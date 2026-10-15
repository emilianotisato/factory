#[path = "harness/mod.rs"]
mod harness;

use std::fs::{self, File};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Output, Stdio};
use std::time::{Duration, Instant};

use factory::db::Db;
use factory::docs::Docs;
use factory::pack::{self, Pack};
use factory::run::pid_alive;
use factory::stream::{self, StatusHint, StreamEvent};
use factory::worker::is_uuid_v4;
use harness::{FEATURE_DIR, Fixture};
use serde_json::{Value, json};

const BIN: &str = env!("CARGO_BIN_EXE_factory");
const RECORDING: &str = "tests/fixture/recordings/claude-p-oneshot-2.1.280.jsonl";
const PROMPT: &str =
    "Build slice {NN} of PRD-001 as the appended system prompt says. Return the STATUS block.";
const LOG: &str = "docs/features/001-fixture/log.jsonl";

fn notes(fx: &Fixture, title: &str) -> Vec<(Option<u64>, String)> {
    fx.log_of("note")
        .iter()
        .filter(|e| e["title"] == title)
        .map(|e| {
            (
                e["slice"].as_u64(),
                e["body"].as_str().unwrap_or("").to_string(),
            )
        })
        .collect()
}

fn decisions(fx: &Fixture) -> Vec<(String, u64)> {
    fx.log_of("decision")
        .iter()
        .map(|e| {
            (
                e["id"].as_str().unwrap().to_string(),
                e["slice"].as_u64().unwrap(),
            )
        })
        .collect()
}

fn answers_to(fx: &Fixture, id: &str) -> Vec<Value> {
    fx.log_of("answer")
        .into_iter()
        .filter(|e| e["ref"] == id)
        .collect()
}

fn factory(fx: &Fixture, args: &[&str]) -> Output {
    fx.command(BIN).args(args).output().unwrap()
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn data_dir(fx: &Fixture) -> PathBuf {
    PathBuf::from(&fx.env()["FACTORY_DATA_DIR"])
}

fn run_dirs(fx: &Fixture) -> Vec<PathBuf> {
    let Ok(rd) = fs::read_dir(data_dir(fx).join("runs")) else {
        return Vec::new();
    };
    let mut v: Vec<PathBuf> = rd.map(|e| e.unwrap().path()).collect();
    v.sort();
    v
}

fn only_run_dir(fx: &Fixture) -> PathBuf {
    let dirs = run_dirs(fx);
    assert_eq!(dirs.len(), 1, "{dirs:?}");
    dirs[0].clone()
}

fn run_id(dir: &Path) -> String {
    dir.file_name().unwrap().to_str().unwrap().to_string()
}

fn state(dir: &Path) -> Value {
    serde_json::from_str(&fs::read_to_string(dir.join("state.json")).unwrap()).unwrap()
}

fn events(dir: &Path) -> Vec<Value> {
    fs::read_to_string(dir.join("events.jsonl"))
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).expect(l))
        .collect()
}

fn kinds(dir: &Path) -> Vec<String> {
    events(dir)
        .iter()
        .map(|e| e["kind"].as_str().unwrap().to_string())
        .collect()
}

fn count(kinds: &[String], kind: &str) -> usize {
    kinds.iter().filter(|k| *k == kind).count()
}

fn slices_of(dir: &Path, kind: &str) -> Vec<u64> {
    events(dir)
        .iter()
        .filter(|e| e["kind"] == kind)
        .map(|e| e["slice"].as_u64().unwrap())
        .collect()
}

fn db(fx: &Fixture) -> Db {
    Db::open(&data_dir(fx).join("factory.db")).unwrap()
}

fn done() -> Value {
    json!({"outcome": "done"})
}

fn dead() -> Value {
    json!({"outcome": "dead"})
}

fn checkpoint() -> Value {
    json!({"outcome": "checkpoint"})
}

fn wait_until(what: &str, secs: u64, mut ok: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(secs);
    while !ok() {
        assert!(
            Instant::now() < deadline,
            "timed out after {secs}s waiting for {what}"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn check_worker_argv(fx: &Fixture, call: &Value, slice: u32, run_dir: &Path) -> String {
    let argv: Vec<&str> = call["argv"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    let flag = |name: &str| -> &str {
        let i = argv
            .iter()
            .position(|a| *a == name)
            .unwrap_or_else(|| panic!("{name} missing in {argv:?}"));
        argv[i + 1]
    };
    assert_eq!(argv[0], "-p", "{argv:?}");
    assert_eq!(argv[1], PROMPT.replace("{NN}", &format!("{slice:02}")));
    assert_eq!(flag("--output-format"), "stream-json");
    assert!(argv.contains(&"--verbose"), "{argv:?}");
    assert_eq!(flag("--permission-mode"), "bypassPermissions");
    let pack = PathBuf::from(flag("--append-system-prompt-file"));
    assert!(pack.is_file(), "{pack:?}");
    assert!(pack.starts_with(run_dir.join("packs")), "{pack:?}");
    let name = pack.file_name().unwrap().to_str().unwrap();
    assert!(name.starts_with(&format!("slice-{slice:02}-")), "{name}");
    // The pack spells the lookup verbs with the driver's absolute path, so a worker runs the build that wrote it.
    let text = fs::read_to_string(&pack).unwrap();
    let lookup = &text[text.find("\n# Lookup\n").unwrap()..text.find("\n# Contract\n").unwrap()];
    let exe = factory::paths::shell_word(BIN);
    for verb in ["def <symbol>", "refs <word>", "outline <path>"] {
        assert!(
            lookup.contains(&format!("`{exe} ctx {verb}`")),
            "{verb}: {lookup}"
        );
    }
    assert!(!lookup.contains("`factory ctx "), "{lookup}");
    let sid = flag("--session-id");
    assert!(is_uuid_v4(sid), "{sid}");
    assert_eq!(flag("--model"), "opus");
    assert_eq!(flag("--effort"), "high");
    let settings = PathBuf::from(flag("--settings"));
    assert_eq!(call["settings"], settings.to_str().unwrap());
    assert!(
        settings.starts_with(run_dir.join("workers")),
        "{settings:?}"
    );
    assert_eq!(harness::assert_role_scope(call), "build");
    assert_eq!(call["env"]["FACTORY_PACK"], pack.to_str().unwrap());
    let lane = call["env"]["FACTORY_LANE"].as_str().unwrap();
    let lane_dir = PathBuf::from(call["cwd"].as_str().unwrap());
    assert!(
        lane_dir.starts_with(fx.data_dir().canonicalize().unwrap().join("lanes"))
            && lane_dir.ends_with(format!("lane-{lane}")),
        "{lane_dir:?}"
    );
    assert_eq!(lane_dir, lane_dir.canonicalize().unwrap());
    assert_eq!(call["env"]["FACTORY_REPO"], call["cwd"]);
    assert_eq!(call["env"]["FACTORY_BOARD"], fx.path().to_str().unwrap());
    assert_eq!(argv.len(), 21, "{argv:?}");
    assert_eq!(call["pack_source"], "file");
    assert!(
        run_dir
            .join("workers")
            .join(format!("{sid}.jsonl"))
            .is_file(),
        "stream of {sid} on disk"
    );
    sid.to_string()
}

#[test]
fn pack_is_the_slice_the_cited_sections_the_contract_and_the_inner_loop() {
    let fx = Fixture::new("pack");
    let slice_file = fx.slice_path(1);
    let scaffold = fs::read_to_string(&slice_file).unwrap();
    let cited = scaffold.replacen("§_(n)_", "§1", 1);
    assert_ne!(cited, scaffold, "the scaffold's Source line cites §_(n)_");
    fs::write(&slice_file, &cited).unwrap();
    let docs = Docs::open(fx.path()).unwrap();
    let board = docs.status("001").unwrap();
    let packs = fx.path().join("scenario/packs");

    let pack = Pack::for_slice(&docs, &board, 1, &packs, 1).unwrap();
    assert_eq!(pack.path, packs.join("slice-01-1.md"));
    assert_eq!(pack.prompt, PROMPT.replace("{NN}", "01"));
    assert_eq!(
        pack.files,
        vec![
            format!("{FEATURE_DIR}/slice-01-a.md"),
            format!("{FEATURE_DIR}/prd.md")
        ]
    );
    assert!(pack.missing_sections.is_empty());
    let text = fs::read_to_string(&pack.path).unwrap();
    assert!(
        text.starts_with(&format!(
            "<!-- factory-pack files: {FEATURE_DIR}/slice-01-a.md,{FEATURE_DIR}/prd.md -->\n\n# Slice\n\n<!-- {FEATURE_DIR}/slice-01-a.md -->\n\n"
        )),
        "{text}"
    );
    assert!(text.contains(cited.trim_end()), "{text}");
    let at = |s: &str| {
        text.find(s)
            .unwrap_or_else(|| panic!("{s} missing in pack"))
    };
    assert!(at("# Slice") < at("\n# PRD sections\n"));
    assert!(at("\n# PRD sections\n") < at("\n# Contract\n"));
    assert!(at("\n# Contract\n") < at("\n# Autonomy\n"));
    assert!(at("\n# Autonomy\n") < at("\n# Inner loop\n"));
    assert!(
        text.contains(&format!(
            "<!-- {FEATURE_DIR}/prd.md: §1 -->\n\n## 1. Why now"
        )),
        "{text}"
    );
    assert!(!text.contains("## 2. Blast radius"), "{text}");
    assert!(!text.contains("## 3. FR-"), "{text}");
    assert!(
        text.contains("docs set 001 slices.1.status=done`"),
        "{text}"
    );
    assert!(text.contains("--kind block --slice 1 "), "{text}");
    assert!(text.contains("one slice of PRD-001"), "{text}");
    assert!(
        text.contains("`factory docs set 001 slices.1.status=building`"),
        "{text}"
    );
    for ph in ["{NNN}", "{NN}", "{N}", "{seed}", "{factory}"] {
        assert!(!text.contains(ph), "{ph} left in pack");
    }
    assert!(text.contains(pack::AUTONOMY.trim_end()), "autonomy copy");
    assert!(
        text.ends_with("\n# Inner loop\n\n./gate.sh --inner\n"),
        "{text}"
    );

    fs::write(&slice_file, scaffold.replacen("§_(n)_", "§1, §9", 1)).unwrap();
    let pack9 = Pack::for_slice(&docs, &board, 1, &packs, 2).unwrap();
    assert_eq!(pack9.missing_sections, vec![9]);
    assert_eq!(pack9.path, packs.join("slice-01-2.md"));
    assert!(pack.path.is_file());

    fs::write(&slice_file, &cited).unwrap();
    let idx = fx.path().join("docs/project.toml");
    let t = fs::read_to_string(&idx).unwrap();
    assert!(t.contains("inner = \"./gate.sh --inner\"\n"), "{t}");
    fs::write(
        &idx,
        t.replace(
            "inner = \"./gate.sh --inner\"\n",
            "inner = \"./gate.sh --inner --fast\"\n",
        ),
    )
    .unwrap();
    let pack3 = Pack::for_slice(&docs, &board, 1, &packs, 3).unwrap();
    let text3 = fs::read_to_string(&pack3.path).unwrap();
    let a: Vec<&str> = text.lines().collect();
    let b: Vec<&str> = text3.lines().collect();
    assert_eq!(a.len(), b.len());
    let diff: Vec<(&str, &str)> = a
        .iter()
        .zip(&b)
        .filter(|(x, y)| x != y)
        .map(|(x, y)| (*x, *y))
        .collect();
    assert_eq!(
        diff,
        vec![("./gate.sh --inner", "./gate.sh --inner --fast")]
    );
}

#[test]
fn pack_outlines_the_cited_code_and_carries_the_lookup_verbs() {
    let fx = Fixture::new("pack-outline");
    fs::write(
        fx.path().join("src/helper.rs"),
        "/// The helper.\npub fn helper() -> u8 {\n    1\n}\n",
    )
    .unwrap();
    let slice_file = fx.slice_path(1);
    let scaffold = fs::read_to_string(&slice_file).unwrap();
    let cites = scaffold.replacen(
        "`file:line`",
        "`src/lib.txt:1` calls `helper()`; `src/planned.rs` is new; `nowhere()` has no def",
        1,
    );
    assert_ne!(
        cites, scaffold,
        "the scaffold's Current code cites `file:line`"
    );
    fs::write(&slice_file, &cites).unwrap();
    let docs = Docs::open(fx.path()).unwrap();
    let board = docs.status("001").unwrap();
    let packs = fx.path().join("scenario/packs");

    let pack = Pack::for_slice(&docs, &board, 1, &packs, 1).unwrap();
    let text = fs::read_to_string(&pack.path).unwrap();
    assert_eq!(
        pack.files,
        vec![
            format!("{FEATURE_DIR}/slice-01-a.md"),
            format!("{FEATURE_DIR}/prd.md"),
            "src/helper.rs".to_string(),
        ]
    );
    assert!(
        text.starts_with(&format!(
            "<!-- factory-pack files: {FEATURE_DIR}/slice-01-a.md,{FEATURE_DIR}/prd.md,src/helper.rs -->\n"
        )),
        "{text}"
    );
    let at = |s: &str| {
        text.find(s)
            .unwrap_or_else(|| panic!("{s} missing in pack"))
    };
    assert!(at("\n# PRD sections\n") < at("\n# Outline\n"));
    assert!(at("\n# Outline\n") < at("\n# Lookup\n"));
    assert!(at("\n# Lookup\n") < at("\n# Contract\n"));
    let outline = &text[at("\n# Outline\n")..at("\n# Lookup\n")];
    assert!(
        outline.contains(
            "<!-- factory ctx outline src/helper.rs -->\n\n== src/helper.rs ==\n2  fn  pub fn helper() -> u8 {\n"
        ),
        "{outline}"
    );
    assert!(
        outline.contains(
            "<!-- factory ctx def helper -->\n\nsrc/helper.rs:2  fn  pub fn helper() -> u8 {\n        1\n    }\n\n1 definition (searched "
        ),
        "{outline}"
    );
    assert!(
        !outline.contains("lib.txt"),
        "no outlined language: {outline}"
    );
    assert!(
        !outline.contains("planned"),
        "a planned file is skipped: {outline}"
    );
    assert!(
        !outline.contains("nowhere"),
        "a def with no hit is skipped: {outline}"
    );
    let lookup = &text[at("\n# Lookup\n")..at("\n# Contract\n")];
    for verb in ["factory ctx def", "factory ctx refs", "factory ctx outline"] {
        assert!(lookup.contains(verb), "{verb}: {lookup}");
    }
    assert!(
        lookup.contains("a bare name goes to ctx, grep anything else"),
        "{lookup}"
    );
    assert!(lookup.contains("`--vendor`"), "{lookup}");
    assert!(
        !lookup.contains("grep only for literal strings and log output"),
        "{lookup}"
    );
    let again = Pack::for_slice(&docs, &board, 1, &packs, 2).unwrap();
    assert_eq!(fs::read_to_string(&again.path).unwrap(), text);

    let mut many = String::new();
    for n in 0..60 {
        let path = format!("src/m{n:02}.rs");
        let body: String = (0..12)
            .map(|i| format!("pub fn function_number_{i}_in_module_{n}() {{}}\n"))
            .collect();
        fs::write(fx.path().join(&path), body).unwrap();
        many.push_str(&format!("`{path}` "));
    }
    fs::write(&slice_file, scaffold.replacen("`file:line`", &many, 1)).unwrap();
    let big = Pack::for_slice(&docs, &board, 1, &packs, 3).unwrap();
    let text = fs::read_to_string(&big.path).unwrap();
    let at = |s: &str| text.find(s).unwrap();
    let outline = &text[at("\n# Outline\n")..at("\n# Lookup\n")];
    assert!(
        outline.contains("[cut at 8000 chars: run `factory ctx outline <path>` for the rest]"),
        "{outline}"
    );
    assert!(outline.len() < 8000 + 1000, "{}", outline.len());
    assert!(outline.contains("== src/m00.rs =="));
    assert!(!outline.contains("== src/m59.rs =="));
    let held = &big.files[2..];
    assert!(!held.is_empty() && held.len() < 60, "{held:?}");
    for f in held {
        assert!(outline.contains(&format!("== {f} ==")), "{f}");
    }
}

#[test]
fn stream_recording_parses_into_the_workers_row_fields() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(RECORDING);
    let text = fs::read_to_string(&path).unwrap();
    let (evs, s) = stream::parse(&text);
    assert_eq!(evs.len(), 18);
    assert_eq!(s.events, 18);
    assert_eq!(s.bad_lines, 0);
    assert!(s.has_result && !s.is_dead());
    assert_eq!(
        evs.iter()
            .filter(|e| matches!(e, StreamEvent::Result(_)))
            .count(),
        1
    );
    assert!(
        evs.iter().any(|e| matches!(e, StreamEvent::System { subtype, .. } if subtype.as_deref() == Some("init"))),
        "no init line"
    );
    let u = s.usage.as_ref().unwrap();
    assert_eq!(
        (
            u.input_tokens,
            u.cache_creation_input_tokens,
            u.cache_read_input_tokens,
            u.output_tokens,
            u.thinking()
        ),
        (Some(18), Some(6514), Some(41674), Some(182), Some(99))
    );
    assert_eq!(s.num_turns, Some(2));
    assert_eq!(s.duration_ms, Some(3710));
    // Max over assistant messages of input + cache writes + cache reads.
    assert_eq!(s.peak_context, Some(24215));
    assert!(s.session_id.is_some());
    assert!(s.model_usage.is_object(), "{}", s.model_usage);
    assert!(!s.is_error);
    assert!(s.cost_usd.is_some());
    let tool_uses = text
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter(|v| v["type"] == "assistant")
        .flat_map(|v| {
            v["message"]["content"]
                .as_array()
                .cloned()
                .unwrap_or_default()
        })
        .filter(|b| b["type"] == "tool_use")
        .count();
    assert_eq!(s.tool_uses.len(), tool_uses);
    for t in &s.tool_uses {
        if text.contains(&format!("\"tool_use_id\":\"{}\"", t.id)) {
            assert!(t.result_chars.is_some(), "{t:?}");
        }
    }

    // Cut before the result line: a dead worker, the peak still measured.
    let cut: String = text.lines().take(17).map(|l| format!("{l}\n")).collect();
    let (_, d) = stream::parse(&cut);
    assert!(d.is_dead());
    assert_eq!(d.events, 17);
    assert_eq!(d.peak_context, Some(24215));
    assert!(d.usage.is_none());

    let (ev, j) = stream::parse("{\"type\":\"something_new\",\"x\":1}\nnot json\n\n");
    assert_eq!(ev.len(), 1);
    assert!(matches!(ev[0], StreamEvent::Unknown));
    assert_eq!(j.events, 1);
    assert_eq!(j.bad_lines, 1);
    assert!(j.is_dead());

    assert_eq!(
        StatusHint::from_result("built it\n\nSTATUS: done\nCOMMIT: abc\n"),
        Some(StatusHint::Done)
    );
    assert_eq!(
        StatusHint::from_result("STATUS: blocked — BLOCK-301"),
        Some(StatusHint::Blocked)
    );
    assert_eq!(StatusHint::from_result("no status here"), None);
}

#[test]
fn run_drives_three_slices_through_the_detached_driver() {
    let fx = Fixture::with_index("loop", &[("checkpoint_every", "10")]);
    fx.chain();
    fx.write_plan(&[done(), done(), done(), checkpoint()]);
    let t0 = Instant::now();
    let out = factory(&fx, &["run", "001"]);
    let took = t0.elapsed();
    let text = stdout(&out);
    assert_eq!(out.status.code(), Some(0), "{text}");
    assert!(
        took < Duration::from_secs(3),
        "factory run took {took:?}: the command returns once the driver owns the run"
    );
    assert!(text.contains("started on feat/fixture"), "{text}");
    let dir = only_run_dir(&fx);
    let id = run_id(&dir);
    let driver = state(&dir)["pid"].as_u64().unwrap() as u32;
    assert!(text.contains(&format!("driver pid {driver}")), "{text}");
    assert!(text.contains("driver.log"), "{text}");

    wait_until("the run to finish", 40, || {
        state(&dir)["phase"] == "finished"
    });
    wait_until("the driver to exit", 5, || !pid_alive(driver));
    for n in 1..=3 {
        assert_eq!(fx.slice_status(n), "done", "slice {n:02}");
    }
    let calls = fx.calls();
    assert_eq!(
        calls.len(),
        5,
        "three builds, a checkpoint and the polish worker: {calls:?}"
    );
    let sids: Vec<String> = (1..=3u32)
        .map(|n| check_worker_argv(&fx, &calls[n as usize - 1], n, &dir))
        .collect();
    assert_eq!(
        sids.iter().collect::<std::collections::BTreeSet<_>>().len(),
        3
    );

    let k = kinds(&dir);
    assert!(k[..4].iter().all(|k| k == "judge_noted"), "{k:?}");
    assert_eq!(k[4], "run_started");
    assert_eq!(
        k[5..],
        [
            "slice_spawned",
            "slice_done",
            "slice_committed",
            "slice_spawned",
            "slice_done",
            "slice_committed",
            "slice_spawned",
            "slice_done",
            "slice_committed",
            "checkpoint_started",
            "gate_run",
            "checkpoint_spawned",
            "checkpoint_done",
            "polish_spawned",
            "polish_done",
            "shipped",
            "pr_opened",
            "ci_green",
            "run_finished"
        ]
    );
    let started = events(&dir)
        .into_iter()
        .find(|e| e["kind"] == "checkpoint_started")
        .unwrap();
    assert_eq!(started["covers"], json!([1, 2, 3]));
    assert_eq!(slices_of(&dir, "slice_spawned"), [1, 2, 3]);
    assert_eq!(slices_of(&dir, "slice_done"), [1, 2, 3]);
    let spawned_sids: Vec<String> = events(&dir)
        .iter()
        .filter(|e| e["kind"] == "slice_spawned")
        .map(|e| e["session_id"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(spawned_sids, sids);

    let ws = state(&dir)["workers"].as_array().unwrap().clone();
    assert_eq!(
        ws.len(),
        5,
        "three builds, the checkpoint and the polish worker"
    );
    for (i, w) in ws[..3].iter().enumerate() {
        assert_eq!(w["slice"], i as u64 + 1);
        assert_eq!(w["session_id"], sids[i].as_str());
        assert_eq!(w["outcome"], "done");
        assert_eq!(w["role"], "build");
    }

    let db = db(&fx);
    let rows = db.workers(&id).unwrap();
    assert_eq!(rows.len(), 5, "three builds, the checkpoint, the polish");
    for (i, r) in rows[..3].iter().enumerate() {
        assert_eq!(r.slice, Some(i as u32 + 1));
        assert_eq!(r.role, "build");
        assert_eq!(r.session_id.as_deref(), Some(sids[i].as_str()));
        assert_eq!(r.model.as_deref(), Some("opus"));
        assert_eq!(r.effort.as_deref(), Some("high"));
        assert_eq!(r.outcome.as_deref(), Some("done"));
        assert_eq!(
            (r.input, r.cache_write, r.cache_read, r.output, r.thinking),
            (Some(18), Some(6514), Some(41674), Some(182), Some(99)),
            "{r:?}"
        );
        assert!(r.num_turns.is_some() && r.duration_ms.is_some(), "{r:?}");
        assert!(r.peak_context.is_some(), "{r:?}");
        assert!(r.cost_usd.is_some(), "{r:?}");
        assert!(r.ended_at.is_some(), "{r:?}");
    }
    assert_eq!(db.run_row(&id).unwrap().unwrap().phase, "finished");
    assert_eq!(db.events(&id).unwrap().len(), k.len());

    let log = fs::read_to_string(dir.join("driver.log")).unwrap();
    assert!(log.contains(&format!("took run {id}")), "{log}");
    assert!(log.contains("slice 01 → worker "), "{log}");
    assert!(
        log.contains("checkpoint 1 (covers 01, 02, 03) — started"),
        "{log}"
    );
    for (i, sid) in sids.iter().enumerate() {
        let s = fs::read_to_string(dir.join("workers").join(format!("{sid}.jsonl"))).unwrap();
        assert!(
            s.lines().any(|l| l.contains("\"type\":\"result\"")),
            "{sid}"
        );
        assert!(
            dir.join("packs")
                .join(format!("slice-{:02}-{}.md", i + 1, i + 1))
                .is_file()
        );
    }
}

#[test]
fn run_foreground_drives_the_same_loop_in_this_process() {
    let fx = Fixture::with_index("loop-fg", &[("checkpoint_every", "10")]);
    fx.write_plan(&[done(), done(), done(), checkpoint()]);
    let out = factory(&fx, &["run", "001", "--foreground"]);
    let text = stdout(&out);
    assert_eq!(out.status.code(), Some(0), "{text}");
    let dir = only_run_dir(&fx);
    let id = run_id(&dir);
    assert!(!text.contains("driver pid"), "{text}");
    assert!(text.contains("slice 01 → worker "), "{text}");
    assert!(text.contains("slice 03 done"), "{text}");
    assert!(
        text.contains("checkpoint 1 (covers 01, 02, 03) — started"),
        "{text}"
    );
    assert_eq!(state(&dir)["phase"], "finished");
    assert_eq!(db(&fx).run_row(&id).unwrap().unwrap().phase, "finished");
    for n in 1..=3 {
        assert_eq!(fx.slice_status(n), "done");
    }
    assert_eq!(fx.calls().len(), 5);
    let k = kinds(&dir);
    assert_eq!(count(&k, "slice_spawned"), 3);
    assert_eq!(count(&k, "slice_done"), 3);
    assert_eq!(k.last().map(String::as_str), Some("run_finished"));
    assert!(!dir.join("driver.log").exists());
}

#[test]
fn a_dead_worker_is_noted_and_respawned_once() {
    let fx = Fixture::with_index("dead-once", &[("checkpoint_every", "10")]);
    fx.chain();
    fx.write_plan(&[dead(), done(), done(), done(), checkpoint()]);
    let out = factory(&fx, &["run", "001", "--foreground"]);
    let text = stdout(&out);
    assert_eq!(out.status.code(), Some(0), "{text}");
    assert!(
        text.contains("died (exited 137, with no result) — respawning once"),
        "{text}"
    );
    let dir = only_run_dir(&fx);
    let id = run_id(&dir);
    assert_eq!(state(&dir)["phase"], "finished");
    for n in 1..=3 {
        assert_eq!(fx.slice_status(n), "done");
    }
    let calls = fx.calls();
    assert_eq!(calls.len(), 6);
    let sid0 = check_worker_argv(&fx, &calls[0], 1, &dir);
    let sid1 = check_worker_argv(&fx, &calls[1], 1, &dir);
    assert_ne!(sid0, sid1, "a respawn is a new session");
    check_worker_argv(&fx, &calls[2], 2, &dir);
    check_worker_argv(&fx, &calls[3], 3, &dir);

    let k = kinds(&dir);
    assert_eq!(
        k[5..],
        [
            "slice_spawned",
            "worker_dead",
            "slice_spawned",
            "slice_done",
            "slice_committed",
            "slice_spawned",
            "slice_done",
            "slice_committed",
            "slice_spawned",
            "slice_done",
            "slice_committed",
            "checkpoint_started",
            "gate_run",
            "checkpoint_spawned",
            "checkpoint_done",
            "polish_spawned",
            "polish_done",
            "shipped",
            "pr_opened",
            "ci_green",
            "run_finished"
        ]
    );
    let dead_ev = events(&dir)
        .into_iter()
        .find(|e| e["kind"] == "worker_dead")
        .unwrap();
    assert_eq!(dead_ev["slice"], 1);
    assert_eq!(dead_ev["session_id"], sid0.as_str());
    assert_eq!(dead_ev["exit_code"], 137);

    let dead_notes = notes(&fx, "Dead worker");
    assert_eq!(dead_notes.len(), 1, "{dead_notes:?}");
    assert_eq!(dead_notes[0].0, Some(1));
    assert!(
        dead_notes[0].1.contains(&format!(
            "session {sid0} exited 137 with no result; respawned."
        )),
        "{dead_notes:?}"
    );
    assert_eq!(decisions(&fx)[0], ("AD-1".to_string(), 1));

    let rows = db(&fx).workers(&id).unwrap();
    assert_eq!(rows.len(), 6);
    assert_eq!(rows[0].outcome.as_deref(), Some("dead"));
    assert_eq!(rows[0].slice, Some(1));
    assert_eq!(rows[0].session_id.as_deref(), Some(sid0.as_str()));
    assert!(
        rows[0].input.is_none() && rows[0].num_turns.is_none(),
        "{:?}",
        rows[0]
    );
    assert!(rows[0].ended_at.is_some());
    for r in &rows[1..] {
        assert_eq!(r.outcome.as_deref(), Some("done"), "{r:?}");
        assert!(r.input.is_some(), "{r:?}");
    }
    let ws = state(&dir)["workers"].as_array().unwrap().clone();
    assert_eq!(ws.len(), 6);
    assert_eq!(ws[0]["outcome"], "dead");
    let s0 = fs::read_to_string(dir.join("workers").join(format!("{sid0}.jsonl"))).unwrap();
    assert_eq!(
        s0.lines().count(),
        1,
        "the dead worker left its init line only: {s0}"
    );
    assert!(s0.contains("\"subtype\":\"init\""), "{s0}");
    assert!(dir.join("packs/slice-01-1.md").is_file());
    assert!(dir.join("packs/slice-01-2.md").is_file());
}

#[test]
fn two_dead_workers_block_the_slice_and_park_the_run() {
    let fx = Fixture::with_index("dead-twice", &[("checkpoint_every", "10")]);
    fx.chain();
    fx.write_plan(&[dead(), dead()]);
    let out = factory(&fx, &["run", "001"]);
    let text = stdout(&out);
    assert_eq!(out.status.code(), Some(0), "{text}");
    let dir = only_run_dir(&fx);
    let id = run_id(&dir);
    let driver = state(&dir)["pid"].as_u64().unwrap() as u32;
    wait_until("the run to park", 40, || {
        state(&dir)["phase"] == "waiting-owner"
    });
    // A parked driver stays alive, on a slow tick.
    assert!(pid_alive(driver));
    let log = fs::read_to_string(dir.join("driver.log")).unwrap();
    assert!(log.contains("respawning once"), "{log}");
    assert!(
        log.contains("second death, slice blocked (BLOCK-1)"),
        "{log}"
    );
    assert!(log.contains("parked (waiting-owner)"), "{log}");
    assert!(
        log.contains("slice 01 blocked — open blocks wait for the owner"),
        "{log}"
    );
    assert_eq!(fx.slice_status(1), "blocked");
    assert_eq!(fx.slice_status(2), "pending");
    assert_eq!(fx.calls().len(), 2);
    let k = kinds(&dir);
    assert_eq!(
        k[5..],
        [
            "slice_spawned",
            "worker_dead",
            "slice_spawned",
            "worker_dead",
            "slice_blocked",
            "run_waiting"
        ]
    );
    let blocked_ev = events(&dir)
        .into_iter()
        .find(|e| e["kind"] == "slice_blocked")
        .unwrap();
    assert_eq!(blocked_ev["slice"], 1);
    assert_eq!(blocked_ev["block_id"], "BLOCK-1");
    assert_eq!(blocked_ev["question"], "Two workers died on slice 01");
    let waiting = events(&dir)
        .into_iter()
        .find(|e| e["kind"] == "run_waiting")
        .unwrap();
    assert_eq!(waiting["blocked"], json!([1]));
    assert_eq!(waiting["asks"], json!([]));
    let board = fx.docs_json("status");
    assert_eq!(board["blocked"], json!([1]));
    assert_eq!(board["building"], json!([]));

    let dead_notes = notes(&fx, "Dead worker");
    assert_eq!(dead_notes.len(), 2, "{dead_notes:?}");
    assert!(
        dead_notes.iter().all(|(s, _)| *s == Some(1)),
        "{dead_notes:?}"
    );
    assert!(dead_notes[0].1.contains("; respawned."), "{dead_notes:?}");
    assert!(
        dead_notes[1]
            .1
            .contains("second death on this slice, blocked (BLOCK-1)."),
        "{dead_notes:?}"
    );
    let blocks = fx.log_of("block");
    assert_eq!(blocks.len(), 1, "{blocks:?}");
    let b = &blocks[0];
    assert_eq!(b["id"], "BLOCK-1", "{b}");
    assert_eq!(b["title"], "Two workers died on slice 01", "{b}");
    assert_eq!(b["slice"], 1, "{b}");
    assert!(answers_to(&fx, "BLOCK-1").is_empty(), "open");
    let body = b["body"].as_str().unwrap();
    for key in ["**Happened:**", "**Options:**", "**Recommended:**"] {
        assert!(body.contains(key), "{key} missing: {body}");
    }
    let rows = db(&fx).workers(&id).unwrap();
    assert_eq!(rows.len(), 2);
    assert!(
        rows.iter().all(|r| r.outcome.as_deref() == Some("dead")),
        "{rows:?}"
    );
    assert_eq!(
        db(&fx).run_row(&id).unwrap().unwrap().phase,
        "waiting-owner"
    );

    // SAFETY: a plain kill(2) on the test's own detached driver.
    let rc = unsafe { libc::kill(driver as i32, libc::SIGKILL) };
    assert_eq!(rc, 0);
    wait_until("the driver to die", 5, || !pid_alive(driver));
    let out = factory(&fx, &["run", "001"]);
    let text = stdout(&out);
    assert_eq!(out.status.code(), Some(0), "{text}");
    assert!(
        text.contains(&format!("run {id} resumed (waiting-owner)")),
        "{text}"
    );
    assert_eq!(run_dirs(&fx).len(), 1);
    let driver2 = state(&dir)["pid"].as_u64().unwrap() as u32;
    assert_ne!(driver2, driver);
    wait_until("the resumed driver to park", 20, || {
        pid_alive(driver2)
            && fs::read_to_string(dir.join("driver.log"))
                .unwrap()
                .matches("parked (waiting-owner)")
                .count()
                >= 2
    });
    assert_eq!(fx.calls().len(), 2);
    assert_eq!(state(&dir)["phase"], "waiting-owner");
    assert_eq!(kinds(&dir).last().map(String::as_str), Some("run_resumed"));
    assert_eq!(count(&kinds(&dir), "run_waiting"), 1);

    let out = factory(&fx, &["stop", &id]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    wait_until("the parked run to finish", 15, || {
        state(&dir)["phase"] == "finished"
    });
    wait_until("the driver to exit", 5, || !pid_alive(driver2));
    let k = kinds(&dir);
    assert_eq!(
        &k[k.len() - 3..],
        ["run_resumed", "stop_requested", "run_stopped"],
        "{k:?}"
    );
    assert_eq!(fx.slice_status(1), "blocked");
}

#[test]
fn the_driver_killed_mid_slice_resumes_and_finishes() {
    let fx = Fixture::with_index("resume", &[("checkpoint_every", "10")]);
    fx.chain();
    // Five spawns: the killed worker, its respawn, slices 02 and 03, the checkpoint.
    fx.write_plan(&[
        json!({"outcome": "done", "delay_ms": 4000}),
        done(),
        done(),
        done(),
        checkpoint(),
    ]);
    let out_path = fx.path().join("scenario/driver.out");
    let err_path = fx.path().join("scenario/driver.err");
    let mut cmd = fx.command(BIN);
    cmd.args(["run", "001", "--foreground"])
        .stdin(Stdio::null())
        .stdout(File::create(&out_path).unwrap())
        .stderr(File::create(&err_path).unwrap())
        .process_group(0);
    let mut child = cmd.spawn().unwrap();
    let driver = child.id();

    wait_until("slice 01 to be building under a worker", 20, || {
        run_dirs(&fx).len() == 1
            && fx.calls().len() == 1
            && fx.slice_status(1) == "building"
            && state(&run_dirs(&fx)[0])["workers"]
                .as_array()
                .is_some_and(|w| w.len() == 1)
    });
    let dir = only_run_dir(&fx);
    let id = run_id(&dir);
    let worker = state(&dir)["workers"][0]["pid"].as_u64().unwrap() as u32;
    assert!(pid_alive(worker));
    assert_eq!(state(&dir)["pid"], driver);
    assert_eq!(state(&dir)["phase"], "building");

    let out = factory(&fx, &["run", "001"]);
    assert_eq!(out.status.code(), Some(1), "{}", stdout(&out));
    assert!(
        stdout(&out).contains(&format!("run {id} is live")),
        "{}",
        stdout(&out)
    );

    // The driver and its worker share the process group.
    // SAFETY: a plain kill(2) on the test's own child group.
    let rc = unsafe { libc::kill(-(driver as i32), libc::SIGKILL) };
    assert_eq!(rc, 0);
    let status = child.wait().unwrap();
    assert!(status.code().is_none(), "killed by signal, got {status}");
    wait_until("the orphaned worker to be gone", 5, || !pid_alive(worker));
    assert_eq!(fx.slice_status(1), "building");
    assert_eq!(state(&dir)["phase"], "building");
    assert_eq!(state(&dir)["workers"][0]["outcome"], Value::Null);
    assert_eq!(fx.calls().len(), 1);
    let sid0 = state(&dir)["workers"][0]["session_id"]
        .as_str()
        .unwrap()
        .to_string();

    let out = factory(&fx, &["run", "001"]);
    let text = stdout(&out);
    assert_eq!(out.status.code(), Some(0), "{text}");
    assert!(
        text.contains(&format!("run {id} resumed (building)")),
        "{text}"
    );
    assert!(text.contains("driver pid "), "{text}");
    assert_eq!(run_dirs(&fx).len(), 1);
    wait_until("the resumed run to finish", 40, || {
        state(&dir)["phase"] == "finished"
    });
    for n in 1..=3 {
        assert_eq!(fx.slice_status(n), "done", "slice {n:02}");
    }
    let calls = fx.calls();
    assert_eq!(calls.len(), 6);
    assert_eq!(check_worker_argv(&fx, &calls[0], 1, &dir), sid0);
    let sid1 = check_worker_argv(&fx, &calls[1], 1, &dir);
    assert_ne!(sid1, sid0);
    check_worker_argv(&fx, &calls[2], 2, &dir);
    check_worker_argv(&fx, &calls[3], 3, &dir);

    let k = kinds(&dir);
    assert_eq!(count(&k, "run_started"), 1, "{k:?}");
    assert_eq!(count(&k, "run_resumed"), 1, "{k:?}");
    assert_eq!(count(&k, "worker_dead"), 1, "{k:?}");
    assert_eq!(count(&k, "slice_spawned"), 4, "{k:?}");
    assert_eq!(count(&k, "slice_done"), 3, "{k:?}");
    assert_eq!(count(&k, "checkpoint_started"), 1, "{k:?}");
    assert_eq!(k.last().map(String::as_str), Some("run_finished"));
    assert_eq!(slices_of(&dir, "slice_done"), [1, 2, 3]);
    assert_eq!(slices_of(&dir, "slice_spawned"), [1, 1, 2, 3]);
    let dead_ev = events(&dir)
        .into_iter()
        .find(|e| e["kind"] == "worker_dead")
        .unwrap();
    assert_eq!(dead_ev["session_id"], sid0.as_str());
    assert_eq!(
        dead_ev["exit_code"],
        Value::Null,
        "an orphan's exit is unknown"
    );
    let i_resumed = k.iter().position(|k| k == "run_resumed").unwrap();
    let i_dead = k.iter().position(|k| k == "worker_dead").unwrap();
    assert!(i_resumed < i_dead, "{k:?}");

    let dead_notes = notes(&fx, "Dead worker");
    assert!(
        dead_notes.iter().any(|(s, b)| *s == Some(1)
            && b.contains(&format!(
                "session {sid0} exited (code unknown) with no result; respawned."
            ))),
        "{dead_notes:?}"
    );
    let rows = db(&fx).workers(&id).unwrap();
    assert_eq!(rows.len(), 6);
    assert_eq!(rows[0].outcome.as_deref(), Some("dead"));
    assert!(rows[0].ended_at.is_some());
    assert!(
        rows[1..]
            .iter()
            .all(|r| r.outcome.as_deref() == Some("done")),
        "{rows:?}"
    );
    let ws = state(&dir)["workers"].as_array().unwrap().clone();
    assert_eq!(ws.len(), 6);
    assert_eq!(ws[0]["outcome"], "dead");
    assert!(ws[1..].iter().all(|w| w["outcome"] == "done"));
}

// ── the death cap counts from the last answer and survives a kill ───────────

fn blocked_ids(dir: &Path) -> Vec<String> {
    events(dir)
        .iter()
        .filter(|e| e["kind"] == "slice_blocked")
        .map(|e| e["block_id"].as_str().unwrap().to_string())
        .collect()
}

fn parks(dir: &Path) -> usize {
    fs::read_to_string(dir.join("driver.log"))
        .unwrap_or_default()
        .matches("parked (waiting-owner)")
        .count()
}

fn stop_and_wait(fx: &Fixture, dir: &Path) {
    let driver = state(dir)["pid"].as_u64().unwrap() as u32;
    let out = factory(fx, &["stop", &run_id(dir)]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    wait_until("the stopped driver to exit", 15, || !pid_alive(driver));
}

#[test]
fn deaths_count_from_the_last_answer_and_each_block_takes_a_free_id() {
    let fx = Fixture::with_index("dead-answered", &[("checkpoint_every", "10")]);
    fx.chain();
    // A seeded, answered block holds the first number: the death block must not take it again.
    let feature_dir = fx.path().join(FEATURE_DIR);
    let mut seeded = factory::docs::log::Entry::new(
        factory::docs::log::Kind::Block,
        "Which store?",
        "**Happened:** x\n",
    );
    seeded.slice = Some(1);
    let id = factory::docs::log::add(&feature_dir, seeded).unwrap();
    assert_eq!(id.as_deref(), Some("BLOCK-1"));
    let mut answer = factory::docs::log::Entry::new(factory::docs::log::Kind::Answer, "", "sqlite");
    answer.reference = Some("BLOCK-1".into());
    factory::docs::log::add(&feature_dir, answer).unwrap();
    fx.git(&["add", "-A"]);
    fx.git(&["commit", "-q", "-m", "chore: an earlier block"]);
    fx.write_plan(&[dead(), dead(), dead(), dead()]);
    let out = factory(&fx, &["run", "001"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    let dir = only_run_dir(&fx);
    wait_until("the run to park", 40, || parks(&dir) >= 1);
    assert_eq!(blocked_ids(&dir), ["BLOCK-2"]);
    assert_eq!(fx.calls().len(), 2);

    let out = factory(&fx, &["answer", "BLOCK-2", "the login is fixed"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    wait_until("the run to park again", 40, || {
        blocked_ids(&dir).len() == 2 && state(&dir)["phase"] == "waiting-owner"
    });
    assert_eq!(
        fx.calls().len(),
        4,
        "a death after the answer respawns once"
    );
    assert_eq!(blocked_ids(&dir), ["BLOCK-2", "BLOCK-3"]);
    let k = kinds(&dir);
    let answered = k.iter().position(|k| k == "block_answered").unwrap();
    assert_eq!(
        k[answered..]
            .iter()
            .filter(|k| *k == "slice_spawned" || *k == "worker_dead" || *k == "slice_blocked")
            .collect::<Vec<_>>(),
        [
            "slice_spawned",
            "worker_dead",
            "slice_spawned",
            "worker_dead",
            "slice_blocked"
        ],
        "{k:?}"
    );
    let blocks = fx.log_of("block");
    let b3: Vec<&Value> = blocks.iter().filter(|b| b["id"] == "BLOCK-3").collect();
    assert_eq!(b3.len(), 1, "{blocks:?}");
    assert_eq!(b3[0]["title"], "Two workers died on slice 01", "{blocks:?}");
    assert_eq!(b3[0]["slice"], 1, "{blocks:?}");
    assert!(answers_to(&fx, "BLOCK-3").is_empty(), "BLOCK-3 open");
    assert!(
        b3[0]["body"]
            .as_str()
            .unwrap()
            .contains("**Happened:** 2 workers on slice 01 exited"),
        "the count is since the answer: {blocks:?}"
    );
    assert_eq!(
        blocks.iter().filter(|b| b["id"] == "BLOCK-1").count(),
        1,
        "{blocks:?}"
    );
    stop_and_wait(&fx, &dir);
}

#[test]
fn a_kill_between_the_second_death_and_the_block_never_spawns_a_third_worker() {
    let fx = Fixture::with_index("dead-killed", &[("checkpoint_every", "10")]);
    fx.chain();
    fx.write_plan(&[dead(), dead()]);
    let out = factory(&fx, &["run", "001"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    let dir = only_run_dir(&fx);
    let driver = state(&dir)["pid"].as_u64().unwrap() as u32;
    wait_until("the run to park", 40, || parks(&dir) >= 1);
    // SAFETY: a plain kill(2) on the test's own detached driver.
    assert_eq!(unsafe { libc::kill(driver as i32, libc::SIGKILL) }, 0);
    wait_until("the driver to die", 5, || !pid_alive(driver));
    // Rewind to just after the second death was saved and before the block was written.
    let log = fx.read(LOG);
    let kept: String = log
        .lines()
        .filter(|l| serde_json::from_str::<Value>(l).unwrap()["kind"] != "block")
        .map(|l| format!("{l}\n"))
        .collect();
    assert_ne!(kept, log, "the block was logged");
    fs::write(fx.path().join(LOG), kept).unwrap();
    assert!(fx.log_of("block").is_empty());
    assert_eq!(notes(&fx, "Dead worker").len(), 2);
    fx.set_slice_status(1, "building");
    assert_eq!(fx.slice_status(1), "building");

    let out = factory(&fx, &["run", "001"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    wait_until("the resumed driver to park", 30, || parks(&dir) >= 2);
    assert_eq!(fx.calls().len(), 2, "no third worker on a capped slice");
    assert_eq!(fx.slice_status(1), "blocked");
    assert_eq!(blocked_ids(&dir), ["BLOCK-1", "BLOCK-1"]);
    let blocks = fx.log_of("block");
    assert_eq!(blocks.len(), 1, "{blocks:?}");
    assert_eq!(blocks[0]["id"], "BLOCK-1");
    assert_eq!(blocks[0]["title"], "Two workers died on slice 01");
    assert_eq!(blocks[0]["slice"], 1);
    assert!(answers_to(&fx, "BLOCK-1").is_empty(), "open");
    stop_and_wait(&fx, &dir);
}

#[test]
fn a_preflight_answer_logs_one_ask_entry_and_a_retry_adds_none() {
    use factory::events::EventKind;
    use factory::run::{Phase, Run};
    use std::os::unix::fs::PermissionsExt;

    let fx = Fixture::new("preflight-ask");
    let item = "no inner loop declared";
    // The run's pid is this test's, so the answer finds a live driver and spawns none.
    let mut run = Run::create(&data_dir(&fx), fx.path(), "001", None, 1).unwrap();
    run.emit(EventKind::AskOpened {
        id: "ASK-1".into(),
        text: item.into(),
    })
    .unwrap();
    run.set_phase(Phase::WaitingOwner).unwrap();
    let answers = run.dir().join("answers");
    let answer_file = answers.join("ASK-1.json");
    let title = format!("Preflight ASK-1: {item}");
    let asks = |fx: &Fixture| -> Vec<Value> {
        fx.log_of("ask")
            .into_iter()
            .filter(|e| e["title"] == title.as_str())
            .collect()
    };

    fs::create_dir_all(&answers).unwrap();
    fs::set_permissions(&answers, fs::Permissions::from_mode(0o555)).unwrap();
    let out = factory(&fx, &["answer", "ASK-1", "proceed"]);
    fs::set_permissions(&answers, fs::Permissions::from_mode(0o700)).unwrap();
    assert_ne!(out.status.code(), Some(0), "{}", stdout(&out));
    assert!(!answer_file.exists());
    let logged = asks(&fx);
    assert_eq!(logged.len(), 1, "{:?}", fx.log_entries());
    assert_eq!(logged[0]["phase"], "preflight", "{}", logged[0]);
    assert!(
        logged[0]["body"]
            .as_str()
            .unwrap()
            .starts_with("**Chose:** proceed\n"),
        "{}",
        logged[0]
    );
    let line = fx
        .read(LOG)
        .lines()
        .find(|l| l.contains(&title))
        .unwrap()
        .to_string();

    let out = factory(&fx, &["answer", "ASK-1", "proceed"]);
    let text = stdout(&out);
    assert_eq!(out.status.code(), Some(0), "{text}");
    assert!(
        text.contains(&format!("ASK-1 answered: proceed — noted in {LOG}")),
        "{text}"
    );
    assert!(answer_file.is_file());
    assert_eq!(fx.log_of("ask").len(), 1, "{:?}", fx.log_entries());
    assert_eq!(fx.read(LOG).matches(&title).count(), 1);
    assert!(fx.read(LOG).contains(&line));
    let answered = events(run.dir())
        .into_iter()
        .filter(|e| e["kind"] == "ask_answered")
        .count();
    assert_eq!(answered, 1, "one `ask_answered` across both tries");
}
