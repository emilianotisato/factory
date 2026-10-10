//! The serial loop end to end (PRD-001 slice 03, FR-1/FR-2): the pack, the
//! stream parser, the worker's argv, three slices through the detached driver
//! and in the foreground, a dead worker respawned once and blocked twice, and
//! a driver killed mid-slice resumed to the end.
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
const RECORDING: &str = "docs/features/001-driver/recordings/claude-p-oneshot-2.1.280.jsonl";
const PROMPT: &str =
    "Build slice {NN} of PRD-001 as the appended system prompt says. Return the STATUS block.";
const FRAGMENT: &str = "docs/features/001-fixture/log-slice-01.md";
const AGENT_LOG: &str = "docs/features/001-fixture/agent-log.md";

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

/// A checkpoint worker's step (slice 06): writes `CP-n` and returns done.
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

/// The session id of the call, after checking FR-2's argv bullet by bullet.
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
    let sid = flag("--session-id");
    assert!(is_uuid_v4(sid), "{sid}");
    assert_eq!(flag("--model"), "opus");
    assert_eq!(flag("--effort"), "high");
    assert_eq!(argv.len(), 15, "{argv:?}");
    assert_eq!(call["cwd"], fx.path().to_str().unwrap());
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
    let docs = Docs::locate(fx.path()).unwrap();
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
    // The slice file byte for byte, the cited section only, the sections in order.
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
    assert!(!text.contains("## 2. Users affected"), "{text}");
    assert!(!text.contains("## 3. FR-"), "{text}");
    // The contract with its placeholders filled; the autonomy copy whole.
    assert!(text.contains("AD-101"), "{text}");
    assert!(text.contains("BLOCK-101"), "{text}");
    assert!(text.contains("one slice of PRD-001"), "{text}");
    assert!(text.contains("slice-01.md"), "{text}");
    for ph in ["{NNN}", "{NN}", "{seed}"] {
        assert!(!text.contains(ph), "{ph} left in pack");
    }
    assert!(text.contains(pack::AUTONOMY.trim_end()), "autonomy copy");
    assert!(
        text.ends_with("\n# Inner loop\n\n./gate.sh --inner\n"),
        "{text}"
    );

    // A cited section the PRD lacks is reported, not fatal; a respawn gets
    // its own file.
    fs::write(&slice_file, scaffold.replacen("§_(n)_", "§1, §9", 1)).unwrap();
    let pack9 = Pack::for_slice(&docs, &board, 1, &packs, 2).unwrap();
    assert_eq!(pack9.missing_sections, vec![9]);
    assert_eq!(pack9.path, packs.join("slice-01-2.md"));
    assert!(pack.path.is_file());

    // A different inner-loop command changes exactly one line of the pack.
    fs::write(&slice_file, &cited).unwrap();
    let idx = fx.path().join("docs/index.md");
    let t = fs::read_to_string(&idx).unwrap();
    assert!(t.contains("inner: ./gate.sh --inner\n"));
    fs::write(
        &idx,
        t.replace(
            "inner: ./gate.sh --inner\n",
            "inner: ./gate.sh --inner --fast\n",
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
    // measure.py:56-67 — max over assistant messages of input + cache writes + cache reads.
    assert_eq!(s.peak_context, Some(24215));
    assert!(s.session_id.is_some());
    assert!(s.model_usage.is_object(), "{}", s.model_usage);
    assert!(!s.is_error);
    assert!(s.cost_usd.is_some());
    // Every tool_use block in the recording is paired with its result size.
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

    // An event type this driver does not know, and a line that is not JSON.
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

    // With every slice done the board says a checkpoint is due (`allDone`);
    // the driver runs it (slice 06) and the run finishes.
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
        4,
        "three builds and a checkpoint worker: {calls:?}"
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
            "log_merged",
            "checkpoint_spawned",
            "checkpoint_done",
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
    assert_eq!(ws.len(), 4, "three builds and the checkpoint worker");
    for (i, w) in ws[..3].iter().enumerate() {
        assert_eq!(w["slice"], i as u64 + 1);
        assert_eq!(w["session_id"], sids[i].as_str());
        assert_eq!(w["outcome"], "done");
        assert_eq!(w["role"], "build");
    }

    let db = db(&fx);
    let rows = db.workers(&id).unwrap();
    assert_eq!(rows.len(), 4);
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
    assert_eq!(fx.calls().len(), 4);
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
    assert_eq!(calls.len(), 5);
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
            "log_merged",
            "checkpoint_spawned",
            "checkpoint_done",
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

    // Merged into agent-log.md at the checkpoint (slice 05); the merge
    // itself refuses a fragment with content above its first heading.
    let frag = fx.read(AGENT_LOG);
    assert!(frag.contains("## Traps\n"), "{frag}");
    assert!(
        frag.contains("- **Dead worker (")
            && frag.contains(&format!(
                "session {sid0} exited 137 with no result; respawned."
            )),
        "{frag}"
    );
    assert_eq!(frag.matches("Dead worker").count(), 1);
    assert!(
        frag.contains("## Decisions\n"),
        "the done worker's AD keeps its heading: {frag}"
    );
    assert!(frag.contains("### AD-101 "), "{frag}");

    let rows = db(&fx).workers(&id).unwrap();
    assert_eq!(rows.len(), 5);
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
    assert_eq!(ws.len(), 5);
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
    // The driver stays alive on a slow tick, watching for the owner (FR-3).
    assert!(pid_alive(driver));
    let log = fs::read_to_string(dir.join("driver.log")).unwrap();
    assert!(log.contains("respawning once"), "{log}");
    assert!(
        log.contains("second death, slice blocked (BLOCK-102)"),
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
    assert_eq!(blocked_ev["block_id"], "BLOCK-102");
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

    let frag = fx.read(FRAGMENT);
    assert!(frag.starts_with("## Traps\n"), "{frag}");
    assert_eq!(frag.matches("- **Dead worker (").count(), 2, "{frag}");
    assert!(frag.contains("; respawned.\n"), "{frag}");
    assert!(
        frag.contains("second death on this slice, blocked (BLOCK-102)."),
        "{frag}"
    );
    assert!(
        frag.contains(
            "\n## Blockers\n\n### BLOCK-102 — Two workers died on slice 01 · slice-01 · open\n"
        ),
        "{frag}"
    );
    for key in ["**Happened:**", "**Options:**", "**Recommended:**"] {
        assert!(frag.contains(key), "{key} missing: {frag}");
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

    // The driver gone (a reboot, say): resuming the parked run parks it again
    // with no second `run_waiting` — the open block still waits for the owner.
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

    // `factory stop` reaches a parked driver on its slow tick.
    let out = factory(&fx, &["stop", &id]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    wait_until("the parked run to finish", 15, || {
        state(&dir)["phase"] == "finished"
    });
    wait_until("the driver to exit", 5, || !pid_alive(driver2));
    let k = kinds(&dir);
    assert_eq!(
        &k[k.len() - 3..],
        ["run_resumed", "stop_requested", "run_finished"],
        "{k:?}"
    );
    assert_eq!(fx.slice_status(1), "blocked");
}

#[test]
fn the_driver_killed_mid_slice_resumes_and_finishes() {
    let fx = Fixture::with_index("resume", &[("checkpoint_every", "10")]);
    fx.chain();
    // Five steps for five spawns: the killed worker's, its respawn's, slices
    // 02 and 03, and the checkpoint worker's.
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

    // A second `factory run` while the driver lives is refused.
    let out = factory(&fx, &["run", "001"]);
    assert_eq!(out.status.code(), Some(1), "{}", stdout(&out));
    assert!(
        stdout(&out).contains(&format!("run {id} is live")),
        "{}",
        stdout(&out)
    );

    // Kill the session: the driver and its worker share the group.
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

    // Resume with the detached driver: the dead worker is settled, the slice
    // gets a fresh one, the run finishes.
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
    assert_eq!(calls.len(), 5);
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

    let frag = fx.read(AGENT_LOG);
    assert!(
        frag.contains(&format!(
            "session {sid0} exited (code unknown) with no result; respawned."
        )),
        "{frag}"
    );
    let rows = db(&fx).workers(&id).unwrap();
    assert_eq!(rows.len(), 5);
    assert_eq!(rows[0].outcome.as_deref(), Some("dead"));
    assert!(rows[0].ended_at.is_some());
    assert!(
        rows[1..]
            .iter()
            .all(|r| r.outcome.as_deref() == Some("done")),
        "{rows:?}"
    );
    let ws = state(&dir)["workers"].as_array().unwrap().clone();
    assert_eq!(ws.len(), 5);
    assert_eq!(ws[0]["outcome"], "dead");
    assert!(ws[1..].iter().all(|w| w["outcome"] == "done"));
}
