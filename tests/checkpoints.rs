#[path = "harness/mod.rs"]
mod harness;

use std::fs::{self, File};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Output, Stdio};
use std::time::{Duration, Instant};

use factory::db::{Db, GateRow};
use factory::run::pid_alive;
use harness::Fixture;
use serde_json::{Value, json};

const BIN: &str = env!("CARGO_BIN_EXE_factory");
const FEATURE_DIR: &str = "docs/features/001-fixture";
const LOG: &str = "docs/features/001-fixture/log.jsonl";
const BUILD: &str =
    "Build slice {NN} of PRD-001 as the appended system prompt says. Return the STATUS block.";
const FIX: &str = "Fix the red gate at checkpoint {n} of PRD-001 as the appended system prompt says. Return the STATUS block.";
const CHECKPOINT: &str =
    "Run checkpoint {n} of PRD-001 as the appended system prompt says. Return the STATUS block.";
const FAIL_LINE: &str = "FAIL tests/fake.rs: expected 2 got 3";

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
        .unwrap_or_default()
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

fn of_kind(dir: &Path, kind: &str) -> Vec<Value> {
    events(dir)
        .into_iter()
        .filter(|e| e["kind"] == kind)
        .collect()
}

fn kinds_from(dir: &Path, from: &str) -> Vec<String> {
    let k = kinds(dir);
    let i = k
        .iter()
        .position(|x| x == from)
        .unwrap_or_else(|| panic!("no {from} in {k:?}"));
    k[i..].to_vec()
}

fn kinds_after(dir: &Path, after: &str) -> Vec<String> {
    kinds_from(dir, after)[1..].to_vec()
}

fn db(fx: &Fixture) -> Db {
    Db::open(&data_dir(fx).join("factory.db")).unwrap()
}

/// A worker's own inner-loop run is a `gates` row too (source `worker`); this file is about the driver's.
fn gates(fx: &Fixture, dir: &Path) -> Vec<GateRow> {
    db(fx)
        .gates(&run_id(dir))
        .unwrap()
        .into_iter()
        .filter(|g| g.source == "driver")
        .collect()
}

fn driver_log(dir: &Path) -> String {
    fs::read_to_string(dir.join("driver.log")).unwrap_or_default()
}

/// On timeout, dumps the run so a failure in the detached driver can be read from the test output.
fn wait_until(fx: &Fixture, dir: &Path, what: &str, secs: u64, mut ok: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(secs);
    while !ok() {
        if Instant::now() >= deadline {
            let statuses: Vec<String> = (1..=3)
                .map(|n| format!("{n:02}={}", fx.slice_status(n)))
                .collect();
            panic!(
                "timed out after {secs}s waiting for {what}\n--- state.json\n{}\n--- events\n{:?}\n--- slices {statuses:?}\n--- git log\n{}\n--- driver.log\n{}\n--- log.jsonl\n{}",
                state(dir),
                kinds(dir),
                fx.git(&["log", "--format=%h %s"]),
                driver_log(dir),
                log_lines(fx).join("\n")
            );
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn wait_phase(fx: &Fixture, dir: &Path, phase: &str, secs: u64) {
    wait_until(fx, dir, &format!("phase {phase}"), secs, || {
        state(dir)["phase"] == phase
    });
}

fn seq_first(dir: &Path, kind: &str) -> u64 {
    events(dir)
        .iter()
        .find(|e| e["kind"] == kind)
        .map(|e| e["seq"].as_u64().unwrap())
        .unwrap_or_else(|| panic!("no {kind}"))
}

fn start(fx: &Fixture) -> PathBuf {
    let out = factory(fx, &["run", "001"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    only_run_dir(fx)
}

fn subjects(fx: &Fixture) -> Vec<String> {
    fx.git(&["log", "--reverse", "--format=%s"])
        .lines()
        .map(str::to_string)
        .collect()
}

fn sha_of(fx: &Fixture, subject: &str) -> String {
    let out = fx.git(&["log", "--format=%H", "--grep", &format!("^{subject}$")]);
    let shas: Vec<&str> = out.lines().collect();
    assert_eq!(shas.len(), 1, "commits titled {subject:?}: {shas:?}");
    shas[0].to_string()
}

fn files_of(fx: &Fixture, sha: &str) -> Vec<String> {
    let mut v: Vec<String> = fx
        .git(&["show", "--format=", "--name-only", sha])
        .lines()
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect();
    v.sort();
    v
}

fn log_lines(fx: &Fixture) -> Vec<String> {
    fs::read_to_string(fx.path().join(LOG))
        .unwrap_or_default()
        .lines()
        .map(str::to_string)
        .collect()
}

/// Read from the file itself: a poll that lands on a line still being written skips it.
fn logged(fx: &Fixture, kind: &str) -> Vec<Value> {
    log_lines(fx)
        .iter()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter(|e| e["kind"] == kind)
        .collect()
}

fn raw_line(fx: &Fixture, id: &str) -> String {
    let needle = format!("\"id\":\"{id}\"");
    let hits: Vec<String> = log_lines(fx)
        .into_iter()
        .filter(|l| l.contains(&needle))
        .collect();
    assert_eq!(hits.len(), 1, "{id} lines: {hits:?}");
    hits[0].clone()
}

fn answers_to(fx: &Fixture, id: &str) -> Vec<Value> {
    fx.log_of("answer")
        .into_iter()
        .filter(|e| e["ref"] == id)
        .collect()
}

fn block(fx: &Fixture, id: &str) -> Value {
    let hits: Vec<Value> = fx
        .log_of("block")
        .into_iter()
        .filter(|e| e["id"] == id)
        .collect();
    assert_eq!(hits.len(), 1, "{id}: {hits:?}");
    hits[0].clone()
}

fn body(e: &Value) -> &str {
    e["body"].as_str().unwrap_or_else(|| panic!("no body: {e}"))
}

fn notes(fx: &Fixture, title: &str) -> Vec<String> {
    fx.log_of("note")
        .iter()
        .filter(|e| e["title"] == title)
        .map(|e| body(e).to_string())
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

fn assert_no_v2_logs(fx: &Fixture) {
    let left: Vec<String> = fs::read_dir(fx.path().join(FEATURE_DIR))
        .unwrap()
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter(|f| f == "agent-log.md" || (f.starts_with("log-") && f.ends_with(".md")))
        .collect();
    assert!(left.is_empty(), "{left:?}");
}

fn done() -> Value {
    json!({"outcome": "done"})
}

fn step(outcome: &str) -> Value {
    json!({"outcome": outcome})
}

fn prompts(fx: &Fixture) -> Vec<String> {
    fx.calls()
        .iter()
        .map(|c| c["prompt"].as_str().unwrap_or("").to_string())
        .collect()
}

fn build(n: u8) -> String {
    BUILD.replace("{NN}", &format!("{n:02}"))
}

fn fix(n: u32) -> String {
    FIX.replace("{n}", &n.to_string())
}

fn checkpoint(n: u32) -> String {
    CHECKPOINT.replace("{n}", &n.to_string())
}

fn polish() -> String {
    "/polish".into()
}

fn pack_of(call: &Value) -> (String, String) {
    let argv: Vec<&str> = call["argv"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    let i = argv
        .iter()
        .position(|a| *a == "--append-system-prompt-file")
        .unwrap_or_else(|| panic!("no pack flag in {argv:?}"));
    let path = PathBuf::from(argv[i + 1]);
    let name = path.file_name().unwrap().to_str().unwrap().to_string();
    (name, fs::read_to_string(&path).unwrap())
}

fn roles(fx: &Fixture, dir: &Path) -> Vec<String> {
    db(fx)
        .workers(&run_id(dir))
        .unwrap()
        .iter()
        .map(|w| w.role.clone())
        .collect()
}

/// The canonical worktrees of the run's lanes, as `pwd -P` prints them.
fn lane_paths(fx: &Fixture) -> Vec<PathBuf> {
    let lanes = data_dir(fx).canonicalize().unwrap().join("lanes");
    fx.git(&["worktree", "list", "--porcelain"])
        .split("\n\n")
        .filter_map(|b| Some(PathBuf::from(b.lines().next()?.strip_prefix("worktree ")?)))
        .filter(|p| p.starts_with(&lanes))
        .collect()
}

fn lane_path(fx: &Fixture, n: u32) -> PathBuf {
    lane_paths(fx)
        .into_iter()
        .find(|p| p.ends_with(format!("lane-{n}")))
        .unwrap_or_else(|| panic!("no lane-{n} worktree"))
}

fn cwd_of(call: &Value) -> PathBuf {
    PathBuf::from(
        call["cwd"]
            .as_str()
            .unwrap_or_else(|| panic!("no cwd: {call}")),
    )
}

fn lanes_with_setup(fx: &Fixture) {
    let lanes = fx.path().join("docs/reference/lanes");
    fs::create_dir_all(&lanes).unwrap();
    for n in 1..=2 {
        fs::write(
            lanes.join(format!("lane-{n}.md")),
            format!(
                "---\nlane: {n}\nenv: [FACTORY_LANE_DB=fixture_{n}]\nsetup: echo \"setup $FACTORY_LANE\" >> \"$(dirname \"$(git rev-parse --path-format=absolute --git-common-dir)\")\"/scenario/lane-setup.log\n---\n\nLane {n} of the fixture.\n"
            ),
        )
        .unwrap();
    }
    fx.git(&["add", "-A"]);
    fx.git(&["commit", "-q", "-m", "chore: lane files"]);
}

fn gate_shape(rows: &[GateRow], n: u32) -> Vec<(bool, bool, u32)> {
    rows.iter()
        .filter(|g| g.checkpoint == Some(n))
        .map(|g| {
            assert_eq!(g.source, "driver", "{g:?}");
            assert_eq!(g.kind, "checkpoint", "{g:?}");
            assert_eq!(g.cmd, "./gate.sh", "{g:?}");
            assert_eq!(g.worker_id, None, "{g:?}");
            if g.cached {
                assert_eq!(g.minutes, 0.0, "{g:?}");
            } else {
                assert!(g.minutes > 0.0, "{g:?}");
            }
            (g.red, g.cached, g.attempt.unwrap())
        })
        .collect()
}

fn assert_cp(fx: &Fixture, n: u32, after: u8, covers: &str, numbers: &[u32]) {
    let id = format!("CP-{n}");
    let cps: Vec<Value> = fx
        .log_of("checkpoint")
        .into_iter()
        .filter(|e| e["id"] == id)
        .collect();
    assert_eq!(cps.len(), 1, "{id}: {:?}", fx.log_entries());
    let cp = &cps[0];
    assert_eq!(cp["slice"], after, "{cp}");
    assert_eq!(cp["covers"], json!(numbers), "{cp}");
    assert_eq!(cp["phase"], "checkpoint", "{cp}");
    assert!(
        cp["title"]
            .as_str()
            .unwrap()
            .starts_with(&format!("after slice-{after:02} · ")),
        "{cp}"
    );
    let entry = body(cp);
    assert!(
        entry.contains(&format!("**Covers:** slices {covers}")),
        "{entry}"
    );
    assert!(entry.contains("**Gate:**"), "{entry}");
    assert!(entry.contains("**Prove:** FR-1"), "{entry}");
    assert!(entry.contains("**Fixed:** none"), "{entry}");
}

fn assert_two_checkpoints(fx: &Fixture, dir: &Path) {
    for n in 1..=3 {
        assert_eq!(fx.slice_status(n), "done", "slice {n:02}");
    }
    assert_cp(fx, 1, 2, "01, 02", &[1, 2]);
    assert_cp(fx, 2, 3, "03", &[3]);
    // One decision per slice, numbered by the tool in commit order.
    assert_eq!(
        decisions(fx),
        [("AD-1".into(), 1), ("AD-2".into(), 2), ("AD-3".into(), 3)]
    );
    assert_no_v2_logs(fx);
    let board = fx.docs_json("status");
    assert_eq!(board["unverified"], json!([]), "{board}");
    assert_eq!(board["checkpointDue"], false, "{board}");
    assert_eq!(
        board["checkpoints"].as_array().map(Vec::len),
        Some(2),
        "{board}"
    );
    let done: Vec<&str> = board["slices"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["status"].as_str().unwrap())
        .collect();
    assert_eq!(done, ["done", "done", "done"], "{board}");
    for n in 1..=2u32 {
        let sha = sha_of(fx, &format!("docs(001): checkpoint {n}"));
        let files = files_of(fx, &sha);
        assert!(files.contains(&LOG.to_string()), "CP-{n}: {files:?}");
        assert!(
            files.iter().all(|f| f.starts_with("docs/")),
            "CP-{n} commit holds code: {files:?}"
        );
        let committed = fx.git(&["show", &format!("{sha}:{LOG}")]);
        assert!(
            committed.contains(&format!("\"id\":\"CP-{n}\"")),
            "{committed}"
        );
        let ev = of_kind(dir, "checkpoint_done")
            .into_iter()
            .find(|e| e["n"] == n)
            .unwrap_or_else(|| panic!("no checkpoint_done {n}"));
        assert_eq!(ev["sha"], sha, "{ev}");
        let rows = db(fx).workers(&run_id(dir)).unwrap();
        let row = rows
            .iter()
            .find(|w| {
                w.role == "checkpoint"
                    && w.checkpoint == Some(n)
                    && w.outcome.as_deref() == Some("done")
            })
            .unwrap_or_else(|| panic!("no done checkpoint row for {n}: {rows:?}"));
        assert_eq!(row.slice, None);
        assert_eq!(row.commit_sha.as_deref(), Some(sha.as_str()));
    }
    let f1 = files_of(fx, &sha_of(fx, "docs(001): checkpoint 1"));
    assert!(
        !f1.iter()
            .any(|f| f == "docs/index.md" || f.contains("/log-slice-")),
        "{f1:?}"
    );
    assert_eq!(fx.git(&["status", "--porcelain"]), "");
    fx.git(&["fsck", "--strict"]);
    assert_eq!(state(dir)["phase"], "finished");
    assert_eq!(kinds(dir).last().map(String::as_str), Some("run_finished"));
    let text = driver_log(dir);
    // Checkpoint 1 reads "started" or, after a kill, "resumed"; the callers check which.
    assert!(text.contains("checkpoint 1 (covers 01, 02) — "), "{text}");
    assert!(
        text.contains("checkpoint 2 (covers 03) — started"),
        "{text}"
    );
    assert!(text.contains("finished (every slice done)"), "{text}");
}

// ── the gate runs before slice 03, CP-1 and CP-2 land ───────────────────────

#[test]
fn the_driver_runs_the_gate_at_each_checkpoint_and_commits_the_cp_entries() {
    let fx = Fixture::new("cp-green");
    fx.chain();
    fx.write_plan(&[
        done(),
        done(),
        step("checkpoint"),
        done(),
        step("checkpoint"),
    ]);
    let dir = start(&fx);
    wait_phase(&fx, &dir, "finished", 60);

    assert_eq!(
        prompts(&fx),
        [
            build(1),
            build(2),
            checkpoint(1),
            build(3),
            checkpoint(2),
            polish()
        ]
    );
    assert_eq!(
        roles(&fx, &dir),
        [
            "build",
            "build",
            "checkpoint",
            "build",
            "checkpoint",
            "polish"
        ]
    );
    assert_eq!(
        subjects(&fx),
        [
            "chore: fixture repo",
            "chore: chain the fixture slices",
            "feat(001): slice-01 A",
            "feat(001): slice-02 B",
            "docs(001): checkpoint 1",
            "feat(001): slice-03 C",
            "docs(001): checkpoint 2",
            "fix(001): polish review fixes",
            "docs(001): ship PRD-001",
        ]
    );

    let rows = gates(&fx, &dir);
    assert_eq!(rows.len(), 2, "{rows:?}");
    assert_eq!(gate_shape(&rows, 1), [(false, false, 1)]);
    assert_eq!(gate_shape(&rows, 2), [(false, false, 1)]);
    let log = fx.gate_log();
    assert_eq!(log.len(), 2, "gate.log");
    assert!(log.iter().all(|g| g.kind == "gate" && g.exit == 0));
    let lanes = lane_paths(&fx);
    for (g, tip) in log
        .iter()
        .zip(["feat(001): slice-02 B", "feat(001): slice-03 C"])
    {
        assert!(lanes.contains(&g.pwd), "{g:?} not in {lanes:?}");
        assert_eq!(g.head, sha_of(&fx, tip), "{g:?}");
    }
    assert!(dir.join("gates/1-1.log").is_file());
    assert!(dir.join("gates/2-1.log").is_file());
    let runs = of_kind(&dir, "gate_run");
    assert_eq!(runs.len(), 2);
    assert_eq!(runs[0]["n"], 1);
    assert_eq!(runs[0]["attempt"], 1);
    assert_eq!(runs[0]["red"], false);
    assert!(runs[0]["minutes"].as_f64().unwrap() > 0.0, "{}", runs[0]);
    assert_eq!(runs[0]["log"], "gates/1-1.log");
    assert_eq!(runs[1]["log"], "gates/2-1.log");

    let calls = fx.calls();
    let (name, pack) = pack_of(&calls[2]);
    assert_eq!(name, "checkpoint-1-3.md");
    assert!(pack.starts_with("<!-- factory-pack files: "), "{pack}");
    for want in [
        "# Gate transcript",
        "<!-- checkpoint 1, green: ",
        "# Covered slices",
        "## slice-01 — A",
        "## slice-02 — B",
        &format!(
            "<!-- {}",
            fx.slice_path(1).strip_prefix(fx.path()).unwrap().display()
        ),
        "**Progress**:",
        "# QA rows",
        "| FR | Proof on the real surface | Evidence | Human? |",
        "| FR-1 | run slice 01 (a) in the fixture",
        "| FR-1 | run slice 02 (b) in the fixture",
        "# Contract",
        "You are running checkpoint 1 of PRD-001",
        "slices 01, 02: each one's Prove task",
        "--kind checkpoint --phase checkpoint --slice 02 --covers 1,2 --title \"after slice-02\"",
        "# Autonomy",
    ] {
        assert!(
            pack.contains(want),
            "{want:?} missing from the checkpoint pack:\n{pack}"
        );
    }
    assert!(!pack.contains("## slice-03"), "{pack}");
    assert_eq!(calls[2]["model"], "opus");
    assert_eq!(calls[2]["effort"], "high");
    let (name, pack) = pack_of(&calls[4]);
    assert_eq!(name, "checkpoint-2-5.md");
    assert!(pack.contains("## slice-03 — C"), "{pack}");
    assert!(!pack.contains("## slice-01"), "{pack}");
    assert!(pack.contains("--title \"after slice-03\""), "{pack}");
    harness::assert_v3_contract(&pack);
    let w = &state(&dir)["workers"][2];
    assert_eq!(w["role"], "checkpoint");
    assert_eq!(w["checkpoint"], 1);
    assert!(w.get("slice").is_none_or(Value::is_null), "{w}");
    assert_eq!(w["outcome"], "done");
    let started = of_kind(&dir, "checkpoint_started");
    assert_eq!(started.len(), 2);
    assert_eq!(started[0]["n"], 1);
    assert_eq!(started[0]["covers"], json!([1, 2]));
    assert_eq!(started[1]["covers"], json!([3]));
    let spawned = of_kind(&dir, "checkpoint_spawned");
    assert_eq!(spawned.len(), 2);
    assert_eq!(spawned[0]["attempt"], 1);
    assert_eq!(spawned[0]["session_id"], calls[2]["session_id"]);
    // No merge step: the slices' entries are in log.jsonl as they land.
    assert_eq!(count(&kinds(&dir), "log_merged"), 0);

    assert_eq!(
        kinds_from(&dir, "slice_spawned"),
        [
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
            "run_finished",
        ]
    );
    assert_two_checkpoints(&fx, &dir);
    assert!(
        driver_log(&dir).contains("checkpoint 1 (covers 01, 02) — started"),
        "{}",
        driver_log(&dir)
    );
}

#[test]
fn checkpoint_evidence_lands_in_the_run_dir_and_never_in_the_branch() {
    let fx = Fixture::new("cp-evidence");
    fx.chain();
    fx.write_plan(&[
        done(),
        done(),
        step("checkpoint"),
        done(),
        step("checkpoint"),
    ]);
    let dir = start(&fx);
    wait_phase(&fx, &dir, "finished", 60);

    let calls = fx.calls();
    assert_eq!(calls[2]["prompt"], checkpoint(1));
    let canon = |p: &Path| fs::canonicalize(p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
    let evidence = canon(&dir.join("evidence"));
    for call in &calls {
        let named = call["env"]["FACTORY_EVIDENCE"]
            .as_str()
            .unwrap_or_else(|| panic!("no FACTORY_EVIDENCE: {}", call["env"]));
        assert_eq!(canon(Path::new(named)), evidence, "{}", call["prompt"]);
    }
    let row = evidence.join("cp-1/row.txt");
    assert!(row.is_file(), "{} missing", row.display());

    let tree = fx.git(&["ls-tree", "-r", "--name-only", "feat/fixture"]);
    assert!(
        !tree.lines().any(|p| p.split('/').any(|s| s == "evidence")),
        "{tree}"
    );
    let shown = fx.docs(&["log", "show", "001"]);
    let cp1 = &shown[shown
        .find("CP-1")
        .unwrap_or_else(|| panic!("no CP-1:\n{shown}"))..];
    let cp1 = &cp1[..cp1.find("CP-2").unwrap_or(cp1.len())];
    assert!(
        cp1.contains("**Prove:** FR-1 — evidence: cp-1/row.txt"),
        "{shown}"
    );
}

// ── leaks at the checkpoint ─────────────────────────────────────────────────

/// A rule's heading, its fix line and its hits, up to the blank line.
fn rule_block<'a>(leaks: &'a str, rule: &str) -> &'a str {
    let start = leaks
        .find(&format!("\n{rule}: "))
        .unwrap_or_else(|| panic!("no {rule} heading in\n{leaks}"))
        + 1;
    let body = &leaks[start..];
    &body[..body.find("\n\n").unwrap_or(body.len())]
}

#[test]
fn the_checkpoint_worker_reads_the_leaks_and_its_fixes_land_with_the_checkpoint() {
    let fx = Fixture::new("cp-leaks");
    fx.chain();
    let project = fx.read("docs/project.toml");
    fs::write(
        fx.path().join("docs/project.toml"),
        format!("{project}\n[leaks]\nallow = [{{ path = \"src/ids.rs\" }}]\n"),
    )
    .unwrap();
    fx.git(&["add", "docs/project.toml"]);
    fx.git(&["commit", "-q", "-m", "chore: allow the id table"]);

    let seed = fx.stub_dir().join("seed");
    for (rel, text) in [
        (
            "src/guard.rs",
            "// FR-3: guard the lane before the merge\n\
             // until slice 4 this runs twice\n\
             pub fn guard() {}\n\
             // The lock stays held across both writes, or a reader sees half a record.\n\
             // git prints rename paths from the repo root, even when run in a subdirectory.\n",
        ),
        ("tests/guard.rs", "#[test]\nfn fr_12_guards_the_lane() {}\n"),
        ("tests/fixture/prd-9/case.jsonl", "{\"a\": 1}\n"),
        ("src/ids.rs", "pub const FIRST: &str = \"BLOCK-1\";\n"),
    ] {
        let p = seed.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, text).unwrap();
    }
    let touch = json!(["src/parse.txt"]);
    fx.write_plan(&[
        json!({"outcome": "done", "touch": touch, "sh": "cp -R \"$FACTORY_STUB_DIR/seed/.\" ."}),
        json!({"outcome": "done", "touch": touch}),
        json!({"outcome": "checkpoint", "leaks": "fix"}),
        json!({"outcome": "done", "touch": touch}),
        json!({"outcome": "checkpoint", "leaks": "fix"}),
    ]);
    let dir = start(&fx);
    wait_phase(&fx, &dir, "finished", 60);

    let calls = fx.calls();
    let first = calls
        .iter()
        .find(|c| c["prompt"] == checkpoint(1))
        .expect("a checkpoint 1 call");
    let (_, pack) = pack_of(first);
    let start = pack.find("\n# Leaks\n").expect("a # Leaks section") + 1;
    let leaks = &pack[start..start + pack[start..].find("\n# Covered slices\n").unwrap()];
    assert!(
        pack[..start].ends_with("```\n\n"),
        "# Leaks follows the gate transcript"
    );
    assert!(
        rule_block(leaks, "id-comment")
            .contains("\n  src/guard.rs:1  // FR-3: guard the lane before the merge"),
        "{leaks}"
    );
    assert!(
        rule_block(leaks, "history")
            .contains("\n  src/guard.rs:2  // until slice 4 this runs twice"),
        "{leaks}"
    );
    let names = rule_block(leaks, "id-name");
    assert!(
        names.contains("\n  tests/guard.rs:2  fn fr_12_guards_the_lane() {}"),
        "{leaks}"
    );
    assert!(
        names
            .lines()
            .any(|l| l == "  tests/fixture/prd-9/case.jsonl"),
        "{leaks}"
    );
    for absent in ["half a record", "subdirectory", "BLOCK-1", "src/ids.rs"] {
        assert!(!leaks.contains(absent), "{absent} in\n{leaks}");
    }

    let planted = [
        "src/guard.rs:1",
        "src/guard.rs:2",
        "tests/guard.rs:2",
        "tests/fixture/prd-9/case.jsonl",
    ];
    let cps = logged(&fx, "checkpoint");
    assert_eq!(cps.len(), 2, "{cps:?}");
    let fixed = body(&cps[0])
        .lines()
        .find(|l| l.starts_with("**Fixed:** "))
        .unwrap_or_else(|| panic!("no Fixed line in {}", cps[0]));
    let mut named: Vec<&str> = fixed["**Fixed:** ".len()..]
        .split(", ")
        .map(|h| h.strip_prefix("leaks: ").unwrap_or(h))
        .collect();
    named.sort();
    let mut want = planted.to_vec();
    want.sort();
    assert_eq!(named, want, "{fixed}");
    assert!(body(&cps[1]).contains("**Fixed:** none"), "{}", cps[1]);
    let last = calls
        .iter()
        .find(|c| c["prompt"] == checkpoint(2))
        .expect("a checkpoint 2 call");
    let (_, pack) = pack_of(last);
    let start = pack.find("\n# Leaks\n").expect("a # Leaks section");
    let leaks = &pack[start..start + pack[start..].find("\n# Covered slices\n").unwrap()];
    assert!(leaks.contains("\nclean\n```"), "{leaks}");

    let sha = sha_of(&fx, "docs(001): checkpoint 1");
    let status = fx.git(&["show", "--format=", "--name-status", &sha]);
    for line in [
        "M\tsrc/guard.rs",
        "M\ttests/guard.rs",
        "D\ttests/fixture/prd-9/case.jsonl",
    ] {
        assert!(status.lines().any(|l| l == line), "{line} in\n{status}");
    }
    let diff = fx.git(&["show", "--format=", &sha, "--", "src/guard.rs"]);
    assert!(diff.contains("\n-// FR-3: guard the lane"), "{diff}");
    assert!(diff.contains("\n-// until slice 4"), "{diff}");

    let out = factory(&fx, &["leaks", "--base", "main"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    assert!(stdout(&out).ends_with("clean\n"), "{}", stdout(&out));
}

// ── the cache on the code tree ──────────────────────────────────────────────

#[test]
fn an_unchanged_code_tree_reuses_the_cached_gate_result() {
    let fx = Fixture::new("cp-cache");
    fx.chain();
    // Slice 03 touches docs only: the tree the gate tests is still checkpoint 1's green one.
    fx.write_plan(&[
        done(),
        done(),
        step("checkpoint"),
        json!({"outcome": "done", "touch": ["docs/notes.txt"]}),
        step("checkpoint"),
    ]);
    let dir = start(&fx);
    wait_phase(&fx, &dir, "finished", 60);

    assert_eq!(fx.gate_log().len(), 1, "gate.sh ran more than once");
    let rows = gates(&fx, &dir);
    assert_eq!(rows.len(), 2, "{rows:?}");
    assert_eq!(gate_shape(&rows, 1), [(false, false, 1)]);
    assert_eq!(gate_shape(&rows, 2), [(false, true, 1)]);
    let cached = of_kind(&dir, "gate_cached");
    assert_eq!(cached.len(), 1, "{cached:?}");
    assert_eq!(cached[0]["n"], 2);
    assert_eq!(cached[0]["attempt"], 1);
    assert_eq!(cached[0]["red"], false);
    assert_eq!(cached[0]["log"], "gates/1-1.log");
    assert_eq!(count(&kinds(&dir), "gate_run"), 1);
    let cache: Value =
        serde_json::from_str(&fs::read_to_string(dir.join("gate-cache.json")).unwrap()).unwrap();
    assert_eq!(cache.as_object().map(|m| m.len()), Some(1), "{cache}");
    let text = driver_log(&dir);
    assert!(
        text.contains(
            "checkpoint 2: gate attempt 1 cached — green (tree unchanged since gates/1-1.log)"
        ),
        "{text}"
    );
    let (_, pack) = pack_of(&fx.calls()[4]);
    assert!(pack.contains("<!-- checkpoint 2, cached green: "), "{pack}");
    let c = files_of(&fx, &sha_of(&fx, "feat(001): slice-03 C"));
    assert!(c.contains(&"docs/notes.txt".to_string()), "{c:?}");
    assert_two_checkpoints(&fx, &dir);
}

// ── red, red, green ─────────────────────────────────────────────────────────

#[test]
fn red_red_green_runs_two_fix_workers_and_three_gates_before_the_cp() {
    let fx = Fixture::new("cp-red");
    fx.chain();
    fx.script_gate(&["red", "red", "green"]);
    fx.write_plan(&[
        done(),
        done(),
        step("fix"),
        step("fix"),
        step("checkpoint"),
        done(),
        step("checkpoint"),
    ]);
    let dir = start(&fx);
    wait_phase(&fx, &dir, "finished", 90);

    assert_eq!(
        prompts(&fx),
        [
            build(1),
            build(2),
            fix(1),
            fix(1),
            checkpoint(1),
            build(3),
            checkpoint(2),
            polish()
        ]
    );
    assert_eq!(
        roles(&fx, &dir),
        [
            "build",
            "build",
            "fix",
            "fix",
            "checkpoint",
            "build",
            "checkpoint",
            "polish"
        ]
    );
    // None cached: each fix changed the tree, and slice 03 touched code.
    let rows = gates(&fx, &dir);
    assert_eq!(rows.len(), 4, "{rows:?}");
    assert_eq!(
        gate_shape(&rows, 1),
        [(true, false, 1), (true, false, 2), (false, false, 3)]
    );
    assert_eq!(gate_shape(&rows, 2), [(false, false, 1)]);
    let log = fx.gate_log();
    assert_eq!(
        log.iter().map(|g| g.exit).collect::<Vec<_>>(),
        [1, 1, 0, 0],
        "gate.log"
    );
    let first = fs::read_to_string(dir.join("gates/1-1.log")).unwrap();
    assert!(first.contains(FAIL_LINE), "{first}");
    let lanes = lane_paths(&fx);
    for call in &fx.calls()[2..4] {
        assert!(lanes.contains(&cwd_of(call)), "{call}");
    }
    assert_eq!(fs::read_to_string(dir.join("gates/1-3.log")).unwrap(), "");

    assert_eq!(
        subjects(&fx),
        [
            "chore: fixture repo",
            "chore: chain the fixture slices",
            "feat(001): slice-01 A",
            "feat(001): slice-02 B",
            "fix(001): gate red at checkpoint 1",
            "fix(001): gate red at checkpoint 1",
            "docs(001): checkpoint 1",
            "feat(001): slice-03 C",
            "docs(001): checkpoint 2",
            "fix(001): polish review fixes",
            "docs(001): ship PRD-001",
        ]
    );
    let fixes: Vec<String> = fx
        .git(&[
            "log",
            "--reverse",
            "--format=%H",
            "--grep",
            "^fix(001): gate red at checkpoint 1$",
        ])
        .lines()
        .map(str::to_string)
        .collect();
    assert_eq!(fixes.len(), 2);
    for sha in &fixes {
        assert_eq!(files_of(&fx, sha), ["src/lib.txt"]);
        assert_eq!(fx.git(&["log", "-1", "--format=%b", sha]), "\n");
    }
    let committed = of_kind(&dir, "fix_committed");
    assert_eq!(committed.len(), 2, "{committed:?}");
    for (i, ev) in committed.iter().enumerate() {
        assert_eq!(ev["n"], 1);
        assert_eq!(ev["attempt"], i as u64 + 1);
        assert_eq!(ev["sha"], fixes[i]);
        assert_eq!(ev["files"], json!(["src/lib.txt"]));
    }
    let workers = db(&fx).workers(&run_id(&dir)).unwrap();
    let fix_rows: Vec<_> = workers.iter().filter(|w| w.role == "fix").collect();
    assert_eq!(fix_rows.len(), 2);
    for (i, w) in fix_rows.iter().enumerate() {
        assert_eq!(w.slice, None);
        assert_eq!(w.checkpoint, Some(1));
        assert_eq!(w.outcome.as_deref(), Some("done"));
        assert_eq!(w.commit_sha.as_deref(), Some(fixes[i].as_str()));
    }
    let out = factory(&fx, &["report", &run_id(&dir), "--json"]);
    let r: Value = serde_json::from_str(&stdout(&out)).expect("one JSON object");
    assert_eq!(r["fix_commits"], 2, "{r}");

    let calls = fx.calls();
    let (name, pack) = pack_of(&calls[2]);
    assert_eq!(name, "fix-1-3.md");
    for want in [
        "# Gate failure",
        "<!-- checkpoint 1, gate attempt 1: ",
        "Gate command: `./gate.sh`",
        "Inner command: `./gate.sh --inner`",
        FAIL_LINE,
        "# Contract",
        "You are fixing a red gate at checkpoint 1 of PRD-001",
        "`fix(001): gate red at checkpoint 1`",
        "--kind block --phase checkpoint",
        "# Autonomy",
        "# Inner loop",
    ] {
        assert!(
            pack.contains(want),
            "{want:?} missing from the fix pack:\n{pack}"
        );
    }
    assert!(!pack.contains("# Slice"), "{pack}");
    harness::assert_v3_contract(&pack);
    let (name, pack) = pack_of(&calls[3]);
    assert_eq!(name, "fix-1-4.md");
    assert!(
        pack.contains("<!-- checkpoint 1, gate attempt 2: "),
        "{pack}"
    );
    let spawned = of_kind(&dir, "fix_spawned");
    assert_eq!(spawned.len(), 2);
    assert_eq!(spawned[0]["attempt"], 1);
    assert_eq!(spawned[1]["attempt"], 2);
    assert_eq!(spawned[0]["session_id"], calls[2]["session_id"]);

    assert_eq!(
        kinds_from(&dir, "checkpoint_started"),
        [
            "checkpoint_started",
            "gate_run",
            "fix_spawned",
            "fix_committed",
            "gate_run",
            "fix_spawned",
            "fix_committed",
            "gate_run",
            "checkpoint_spawned",
            "checkpoint_done",
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
            "run_finished",
        ]
    );
    assert_two_checkpoints(&fx, &dir);
}

// ── three reds block; the answer reruns the gate ────────────────────────────

#[test]
fn three_reds_block_the_run_and_the_answer_reruns_the_gate() {
    let fx = Fixture::new("cp-block");
    fx.chain();
    fx.script_gate(&["red", "red", "red"]);
    fx.write_plan(&[
        done(),
        done(),
        step("fix"),
        step("fix"),
        step("checkpoint"),
        done(),
        step("checkpoint"),
    ]);
    let dir = start(&fx);
    wait_phase(&fx, &dir, "waiting-owner", 90);

    assert_eq!(prompts(&fx), [build(1), build(2), fix(1), fix(1)]);
    let rows = gates(&fx, &dir);
    assert_eq!(
        gate_shape(&rows, 1),
        [(true, false, 1), (true, false, 2), (true, false, 3)]
    );
    assert_eq!(fx.gate_log().len(), 3);
    assert_eq!(
        subjects(&fx)[2..],
        [
            "feat(001): slice-01 A",
            "feat(001): slice-02 B",
            "fix(001): gate red at checkpoint 1",
            "fix(001): gate red at checkpoint 1",
        ]
    );
    assert_eq!(decisions(&fx), [("AD-1".into(), 1), ("AD-2".into(), 2)]);
    assert!(fx.log_of("checkpoint").is_empty(), "{:?}", fx.log_entries());
    assert_no_v2_logs(&fx);
    let title = format!("gate red three times at checkpoint 1: {FAIL_LINE}");
    let blocks = fx.log_of("block");
    assert_eq!(blocks.len(), 1, "{blocks:?}");
    let b = &blocks[0];
    assert_eq!(b["id"], "BLOCK-1", "{b}");
    assert_eq!(b["title"], title.as_str(), "{b}");
    assert_eq!(b["phase"], "checkpoint", "{b}");
    assert!(b.get("slice").is_none_or(Value::is_null), "{b}");
    assert!(answers_to(&fx, "BLOCK-1").is_empty());
    let text = body(b);
    assert!(text.contains("**Happened:** the gate (`./gate.sh`) was red 3 times in a row at checkpoint 1; 2 fix worker(s) ran"), "{text}");
    assert!(text.contains(&format!("  {FAIL_LINE}\n")), "{text}");
    assert!(text.contains("**Recommended:** 1"), "{text}");
    let feature_dir = fx.path().join(FEATURE_DIR);
    assert_eq!(
        factory::checkpoint::open_checkpoint_blocks(&feature_dir).unwrap(),
        [("BLOCK-1".to_string(), title.clone())]
    );
    let block_line = raw_line(&fx, "BLOCK-1");
    let blocked = of_kind(&dir, "checkpoint_blocked");
    assert_eq!(blocked.len(), 1, "{blocked:?}");
    assert_eq!(blocked[0]["n"], 1);
    assert_eq!(blocked[0]["block_id"], "BLOCK-1");
    assert_eq!(blocked[0]["question"], title);
    assert_eq!(
        kinds_from(&dir, "checkpoint_started"),
        [
            "checkpoint_started",
            "gate_run",
            "fix_spawned",
            "fix_committed",
            "gate_run",
            "fix_spawned",
            "fix_committed",
            "gate_run",
            "checkpoint_blocked",
            "run_waiting",
        ]
    );
    let driver = state(&dir)["pid"].as_u64().unwrap() as u32;
    assert!(pid_alive(driver), "driver gone");
    let text = driver_log(&dir);
    assert!(
        text.contains(&format!(
            "parked (waiting-owner): checkpoint blocked — {title}: BLOCK-1 in log.jsonl waits for the owner: `factory answer BLOCK-1 \"<text>\"`"
        )),
        "{text}"
    );
    let open = &fx.docs_json("status")["openBlocks"];
    assert_eq!(open.as_array().map(Vec::len), Some(1), "{open}");
    assert_eq!(open[0]["n"], 1);
    assert_eq!(open[0]["phase"], "checkpoint");

    // The tree is unchanged, yet the gate runs again: a cached red is not trusted past an answer.
    let out = factory(&fx, &["answer", "BLOCK-1", "fixed the runner by hand"]);
    let text = stdout(&out);
    assert_eq!(out.status.code(), Some(0), "{text}");
    assert!(text.starts_with("BLOCK-1 resolved ("), "{text}");
    let answers = answers_to(&fx, "BLOCK-1");
    assert_eq!(answers.len(), 1, "{answers:?}");
    assert_eq!(body(&answers[0]), "fixed the runner by hand");
    assert_eq!(raw_line(&fx, "BLOCK-1"), block_line);
    assert_eq!(
        factory::checkpoint::open_checkpoint_blocks(&feature_dir).unwrap(),
        []
    );
    wait_phase(&fx, &dir, "finished", 90);

    let rows = gates(&fx, &dir);
    assert_eq!(
        gate_shape(&rows, 1),
        [
            (true, false, 1),
            (true, false, 2),
            (true, false, 3),
            (false, false, 4)
        ]
    );
    assert_eq!(gate_shape(&rows, 2), [(false, false, 1)]);
    assert_eq!(
        fx.gate_log().iter().map(|g| g.exit).collect::<Vec<_>>(),
        [1, 1, 1, 0, 0]
    );
    let fourth = of_kind(&dir, "gate_run")
        .into_iter()
        .find(|e| e["n"] == 1 && e["attempt"] == 4)
        .expect("gate attempt 4");
    assert!(seq_first(&dir, "block_answered") < fourth["seq"].as_u64().unwrap());
    assert_eq!(
        kinds_after(&dir, "block_answered"),
        [
            "checkpoint_resumed",
            "gate_run",
            "checkpoint_spawned",
            "checkpoint_done",
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
            "run_finished",
        ]
    );
    let resumed = of_kind(&dir, "checkpoint_resumed");
    assert_eq!(resumed[0]["n"], 1);
    assert_eq!(resumed[0]["covers"], json!([1, 2]));
    assert_eq!(count(&kinds(&dir), "run_waiting"), 1);
    assert_eq!(
        prompts(&fx),
        [
            build(1),
            build(2),
            fix(1),
            fix(1),
            checkpoint(1),
            build(3),
            checkpoint(2),
            polish()
        ]
    );
    assert_eq!(fx.log_of("block").len(), 1);
    assert_eq!(raw_line(&fx, "BLOCK-1"), block_line);
    assert_eq!(answers_to(&fx, "BLOCK-1").len(), 1);
    assert_eq!(fx.docs_json("status")["openBlocks"], json!([]));
    assert_two_checkpoints(&fx, &dir);
}

// ── a run killed under the checkpoint worker resumes there ──────────────────

#[test]
fn a_run_killed_during_the_checkpoint_worker_resumes_at_the_checkpoint() {
    let fx = Fixture::new("cp-resume");
    fx.chain();
    fx.write_plan(&[
        done(),
        done(),
        json!({"outcome": "checkpoint", "delay_ms": 3000}),
        step("checkpoint"),
        json!({"outcome": "done", "touch": ["docs/notes.txt"]}),
        step("checkpoint"),
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

    let deadline = Instant::now() + Duration::from_secs(30);
    while run_dirs(&fx).len() != 1
        || fx.calls().len() != 3
        || !kinds(&run_dirs(&fx)[0])
            .iter()
            .any(|k| k == "checkpoint_spawned")
    {
        assert!(
            Instant::now() < deadline,
            "no checkpoint worker after 30s: calls {}, out {}",
            fx.calls().len(),
            fs::read_to_string(&out_path).unwrap_or_default()
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    let dir = only_run_dir(&fx);
    let id = run_id(&dir);
    let w = state(&dir)["workers"][2].clone();
    assert_eq!(w["role"], "checkpoint");
    let worker = w["pid"].as_u64().unwrap() as u32;
    let sid0 = w["session_id"].as_str().unwrap().to_string();
    assert!(pid_alive(worker));
    assert_eq!(state(&dir)["phase"], "checkpoint");
    assert_eq!(fx.gate_log().len(), 1);

    // SAFETY: a plain kill(2) on the test's own child group.
    let rc = unsafe { libc::kill(-(driver as i32), libc::SIGKILL) };
    assert_eq!(rc, 0);
    let status = child.wait().unwrap();
    assert!(status.code().is_none(), "killed by signal, got {status}");
    let deadline = Instant::now() + Duration::from_secs(5);
    while pid_alive(worker) {
        assert!(Instant::now() < deadline, "the orphaned worker lives on");
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(state(&dir)["phase"], "checkpoint");
    assert_eq!(state(&dir)["workers"][2]["outcome"], Value::Null);
    assert!(logged(&fx, "checkpoint").is_empty());

    let out = factory(&fx, &["run", "001"]);
    let text = stdout(&out);
    assert_eq!(out.status.code(), Some(0), "{text}");
    assert!(
        text.contains(&format!("run {id} resumed (checkpoint)")),
        "{text}"
    );
    assert_eq!(run_dirs(&fx).len(), 1);
    wait_phase(&fx, &dir, "finished", 60);

    assert_eq!(
        kinds_after(&dir, "run_resumed"),
        [
            "worker_dead",
            "checkpoint_resumed",
            "gate_cached",
            "checkpoint_spawned",
            "checkpoint_done",
            "slice_spawned",
            "slice_done",
            "slice_committed",
            "checkpoint_started",
            "gate_cached",
            "checkpoint_spawned",
            "checkpoint_done",
            "polish_spawned",
            "polish_done",
            "shipped",
            "pr_opened",
            "ci_green",
            "run_finished",
        ]
    );
    assert_eq!(fx.gate_log().len(), 1, "the gate ran again");
    let rows = gates(&fx, &dir);
    assert_eq!(gate_shape(&rows, 1), [(false, false, 1), (false, true, 2)]);
    assert_eq!(gate_shape(&rows, 2), [(false, true, 1)]);
    let dead = of_kind(&dir, "worker_dead");
    assert_eq!(dead.len(), 1);
    assert_eq!(dead[0]["session_id"], sid0.as_str());
    assert_eq!(dead[0]["exit_code"], Value::Null);
    assert!(
        dead[0].get("slice").is_none_or(Value::is_null),
        "{}",
        dead[0]
    );
    let resumed = of_kind(&dir, "checkpoint_resumed");
    assert_eq!(resumed[0]["n"], 1);
    assert_eq!(resumed[0]["covers"], json!([1, 2]));
    let spawned = of_kind(&dir, "checkpoint_spawned");
    assert_eq!(spawned.len(), 3);
    assert_eq!(spawned[1]["n"], 1);
    assert_eq!(spawned[1]["attempt"], 2);
    assert_eq!(
        prompts(&fx),
        [
            build(1),
            build(2),
            checkpoint(1),
            checkpoint(1),
            build(3),
            checkpoint(2),
            polish()
        ]
    );
    let workers = db(&fx).workers(&run_id(&dir)).unwrap();
    assert_eq!(workers.len(), 7, "the six and the polish worker");
    assert_eq!(workers[2].outcome.as_deref(), Some("dead"));
    assert_eq!(workers[2].checkpoint, Some(1));
    assert_eq!(workers[3].outcome.as_deref(), Some("done"));
    let dead_notes = notes(&fx, "Dead checkpoint worker");
    assert!(
        dead_notes.iter().any(|n| n.contains(&format!(
            "checkpoint 1, session {sid0} exited (code unknown) with no result."
        ))),
        "{dead_notes:?}"
    );
    let text = driver_log(&dir);
    assert!(
        text.contains("checkpoint 1 (covers 01, 02) — resumed"),
        "{text}"
    );
    assert_two_checkpoints(&fx, &dir);
}

// ── two dead checkpoint workers block; the answer spawns a third ────────────

#[test]
fn two_dead_checkpoint_workers_block_the_run_and_the_answer_spawns_a_third() {
    let fx = Fixture::new("cp-dead");
    fx.chain();
    fx.write_plan(&[
        done(),
        done(),
        step("dead"),
        step("dead"),
        step("checkpoint"),
        done(),
        step("checkpoint"),
    ]);
    let dir = start(&fx);
    wait_phase(&fx, &dir, "waiting-owner", 60);

    assert_eq!(
        prompts(&fx),
        [build(1), build(2), checkpoint(1), checkpoint(1)]
    );
    assert_eq!(
        kinds_from(&dir, "checkpoint_started"),
        [
            "checkpoint_started",
            "gate_run",
            "checkpoint_spawned",
            "worker_dead",
            "checkpoint_spawned",
            "worker_dead",
            "checkpoint_blocked",
            "run_waiting",
        ]
    );
    let spawned = of_kind(&dir, "checkpoint_spawned");
    assert_eq!(spawned[0]["attempt"], 1);
    assert_eq!(spawned[1]["attempt"], 2);
    let dead = of_kind(&dir, "worker_dead");
    assert_eq!(dead[0]["exit_code"], 137);
    let title = "Two checkpoint workers died at checkpoint 1";
    let b = block(&fx, "BLOCK-1");
    assert_eq!(b["title"], title, "{b}");
    assert_eq!(b["phase"], "checkpoint", "{b}");
    assert!(answers_to(&fx, "BLOCK-1").is_empty(), "open");
    let dead_notes = notes(&fx, "Dead checkpoint worker");
    assert_eq!(dead_notes.len(), 2, "{dead_notes:?}");
    assert_eq!(decisions(&fx), [("AD-1".into(), 1), ("AD-2".into(), 2)]);
    let blocked = of_kind(&dir, "checkpoint_blocked");
    assert_eq!(blocked[0]["block_id"], "BLOCK-1");
    assert_eq!(blocked[0]["question"], title);
    assert_eq!(fx.gate_log().len(), 1);
    let workers = db(&fx).workers(&run_id(&dir)).unwrap();
    assert_eq!(workers.len(), 4);
    for w in &workers[2..] {
        assert_eq!(w.role, "checkpoint");
        assert_eq!(w.checkpoint, Some(1));
        assert_eq!(w.outcome.as_deref(), Some("dead"));
    }
    assert_eq!(
        subjects(&fx).last().map(String::as_str),
        Some("feat(001): slice-02 B")
    );

    let out = factory(&fx, &["answer", "BLOCK-1", "logged in again"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    wait_phase(&fx, &dir, "finished", 60);

    assert_eq!(
        kinds_after(&dir, "block_answered"),
        [
            "checkpoint_resumed",
            "gate_cached",
            "checkpoint_spawned",
            "checkpoint_done",
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
            "run_finished",
        ]
    );
    let spawned = of_kind(&dir, "checkpoint_spawned");
    assert_eq!(spawned.len(), 4);
    assert_eq!(spawned[2]["n"], 1);
    assert_eq!(spawned[2]["attempt"], 1, "deaths before the answer count");
    assert_eq!(fx.gate_log().len(), 2);
    let rows = gates(&fx, &dir);
    assert_eq!(gate_shape(&rows, 1), [(false, false, 1), (false, true, 2)]);
    assert_eq!(
        prompts(&fx),
        [
            build(1),
            build(2),
            checkpoint(1),
            checkpoint(1),
            checkpoint(1),
            build(3),
            checkpoint(2),
            polish()
        ]
    );
    assert_eq!(block(&fx, "BLOCK-1")["title"], title);
    let answers = answers_to(&fx, "BLOCK-1");
    assert_eq!(answers.len(), 1, "resolved: {answers:?}");
    assert_eq!(body(&answers[0]), "logged in again");
    assert_two_checkpoints(&fx, &dir);
}

// ── a fix worker's changes, however it ended ────────────────────────────────

#[test]
fn a_dead_fix_workers_changes_are_committed_before_the_gate_runs_again() {
    let fx = Fixture::new("cp-dead-fix");
    fx.chain();
    fx.script_gate(&["red"]);
    fx.write_plan(&[
        done(),
        done(),
        json!({"outcome": "dead", "delay_ms": 3000}),
        step("checkpoint"),
        done(),
        step("checkpoint"),
    ]);
    let dir = start(&fx);
    wait_until(&fx, &dir, "the fix worker", 40, || fx.calls().len() >= 3);
    // The fix worker's change in its lane before it died, through a tool call the stream never flushed.
    let lane = cwd_of(&fx.calls()[2]);
    fs::write(lane.join("src/lib.txt"), "fixed before the death\n").unwrap();
    wait_phase(&fx, &dir, "finished", 60);

    assert_eq!(prompts(&fx)[2], fix(1));
    let sha = sha_of(&fx, "fix(001): gate red at checkpoint 1");
    assert_eq!(files_of(&fx, &sha), ["src/lib.txt"]);
    let committed = of_kind(&dir, "fix_committed");
    assert_eq!(committed.len(), 1, "{committed:?}");
    assert_eq!(committed[0]["attempt"], 1);
    assert_eq!(committed[0]["sha"], sha);
    assert_eq!(of_kind(&dir, "worker_dead").len(), 1);
    assert!(
        seq_first(&dir, "fix_committed") < of_kind(&dir, "gate_run")[1]["seq"].as_u64().unwrap()
    );
    assert_eq!(
        gate_shape(&gates(&fx, &dir), 1),
        [(true, false, 1), (false, false, 2)]
    );
    let subjects = subjects(&fx);
    let at = |s: &str| subjects.iter().position(|x| x == s).unwrap();
    assert!(at("fix(001): gate red at checkpoint 1") < at("docs(001): checkpoint 1"));
}

#[test]
fn a_dirty_main_tree_does_not_pin_the_cache() {
    let fx = Fixture::new("cp-dirty-green");
    fx.chain();
    // Slice 03 touches nothing, so checkpoint 2 sees checkpoint 1's tree.
    fx.write_plan(&[
        done(),
        done(),
        step("dead"),
        step("dead"),
        step("checkpoint"),
        json!({"outcome": "done", "touch": []}),
        step("checkpoint"),
    ]);
    let dir = start(&fx);
    // Written after the clean-tree preflight; the gate runs in a lane reset to HEAD and never sees it.
    fs::write(fx.path().join("notes.txt"), "owner's notes\n").unwrap();
    wait_phase(&fx, &dir, "waiting-owner", 60);
    let out = factory(&fx, &["answer", "BLOCK-1", "logged in again"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    wait_phase(&fx, &dir, "finished", 60);

    assert!(fx.path().join("notes.txt").exists());
    let rows = gates(&fx, &dir);
    assert_eq!(gate_shape(&rows, 1), [(false, false, 1), (false, true, 2)]);
    assert_eq!(gate_shape(&rows, 2), [(false, true, 1)]);
    assert_eq!(fx.gate_log().len(), 1);
    let cache = factory::gatecache::GateCache::load(&dir).unwrap();
    assert_eq!(cache.len(), 1, "{cache:?}");
}

// ── the checkpoint runs in the lowest free lane ─────────────────────────────

#[test]
fn a_checkpoint_due_while_a_blocked_slice_holds_lane_1_runs_in_lane_2() {
    let fx = Fixture::new("cp-lane-2");
    lanes_with_setup(&fx);
    // 01 blocks holding lane 1; 02 and 03 land through lane 2, and checkpoint 1 is due meanwhile.
    fx.write_plan(&[step("blocked"), done(), done(), step("checkpoint")]);
    let dir = start(&fx);
    wait_until(&fx, &dir, "the park after checkpoint 1", 60, || {
        state(&dir)["phase"] == "waiting-owner"
            && kinds(&dir).iter().any(|k| k == "checkpoint_done")
    });

    assert_eq!(prompts(&fx), [build(1), build(2), build(3), checkpoint(1)]);
    assert_eq!(fx.slice_status(1), "blocked");
    assert_eq!(fx.slice_lane(1), Some(1));
    let lane = lane_path(&fx, 2);
    let log = fx.gate_log();
    assert_eq!(log.len(), 1, "{log:?}");
    assert_eq!(log[0].pwd, lane, "{log:?}");
    assert_eq!(cwd_of(&fx.calls()[3]), lane);
    let text = driver_log(&dir);
    assert!(
        text.contains("parked (waiting-owner): slice 01 blocked"),
        "{text}"
    );
    // Lane 2's setup ran once: for slice 02, not again for 03 or the gate.
    let setup = fx.read("scenario/lane-setup.log");
    assert_eq!(
        setup.lines().filter(|l| *l == "setup 2").count(),
        1,
        "{setup}"
    );
    let out = factory(&fx, &["stop", &run_id(&dir)]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
}

#[test]
fn a_file_only_the_main_tree_holds_does_not_turn_the_gate_green() {
    let fx = Fixture::with_index(
        "cp-main-only",
        &[("gate", "\"./gate.sh && test -f needed.txt\"")],
    );
    fx.chain();
    fx.write_plan(&[done(), done(), step("fix"), step("fix")]);
    let dir = start(&fx);
    // Written after the clean-tree preflight and before the first gate; the lane reset to HEAD has no copy.
    fs::write(fx.path().join("needed.txt"), "the owner's\n").unwrap();
    wait_phase(&fx, &dir, "waiting-owner", 90);

    let runs = of_kind(&dir, "gate_run");
    assert_eq!(runs.len(), 3, "{runs:?}");
    assert!(runs.iter().all(|r| r["red"] == true), "{runs:?}");
    let lanes = lane_paths(&fx);
    for g in fx.gate_log() {
        assert_eq!(g.exit, 0, "{g:?}");
        assert!(lanes.contains(&g.pwd), "{g:?}");
    }
    let blocks = fx.log_of("block");
    assert_eq!(blocks.len(), 1, "{blocks:?}");
    assert!(
        blocks[0]["title"]
            .as_str()
            .unwrap()
            .starts_with("gate red three times at checkpoint 1"),
        "{}",
        blocks[0]
    );
    assert!(fx.path().join("needed.txt").exists());
    let out = factory(&fx, &["stop", &run_id(&dir)]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
}

#[test]
fn a_fix_workers_file_no_tool_call_named_lands_in_its_commit() {
    let fx = Fixture::new("cp-fix-unnamed");
    fx.chain();
    fx.script_gate(&["red"]);
    // The file's name is built in the script: no tool call names it.
    fx.write_plan(&[
        done(),
        done(),
        json!({"outcome": "fix", "touch": [], "sh": "n=fixed; echo ok > src/$n.txt"}),
        step("checkpoint"),
        done(),
        step("checkpoint"),
    ]);
    let dir = start(&fx);
    wait_phase(&fx, &dir, "finished", 60);

    let sha = sha_of(&fx, "fix(001): gate red at checkpoint 1");
    assert_eq!(files_of(&fx, &sha), ["src/fixed.txt"]);
    assert_eq!(fx.read("src/fixed.txt"), "ok\n");
    assert_eq!(fx.git(&["status", "--porcelain"]), "");
}

// ── a checkpoint worker that outlives its driver ────────────────────────────

#[test]
fn a_checkpoint_worker_that_outlives_its_driver_gets_its_checkpoint_committed() {
    let fx = Fixture::new("cp-orphan-done");
    fx.chain();
    fx.write_plan(&[
        done(),
        done(),
        json!({"outcome": "checkpoint", "delay_ms": 3000}),
        done(),
        step("checkpoint"),
    ]);
    let dir = start(&fx);
    let driver = state(&dir)["pid"].as_u64().unwrap() as u32;
    wait_until(&fx, &dir, "the checkpoint worker", 30, || {
        fx.calls().len() == 3 && kinds(&dir).iter().any(|k| k == "checkpoint_spawned")
    });
    // SAFETY: a plain kill(2) on the detached driver; its worker lives on.
    assert_eq!(unsafe { libc::kill(driver as i32, libc::SIGKILL) }, 0);
    wait_until(&fx, &dir, "the driver to die", 5, || !pid_alive(driver));
    wait_until(&fx, &dir, "the orphan to write CP-1", 20, || {
        logged(&fx, "checkpoint").iter().any(|e| e["id"] == "CP-1")
    });
    assert!(!subjects(&fx).iter().any(|s| s == "docs(001): checkpoint 1"));

    let out = factory(&fx, &["run", "001"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    wait_phase(&fx, &dir, "finished", 60);

    assert_eq!(
        prompts(&fx),
        [
            build(1),
            build(2),
            checkpoint(1),
            build(3),
            checkpoint(2),
            polish()
        ]
    );
    let subjects = subjects(&fx);
    let at = |s: &str| {
        subjects
            .iter()
            .position(|x| x == s)
            .unwrap_or_else(|| panic!("{s}: {subjects:?}"))
    };
    assert!(
        at("feat(001): slice-02 B") < at("docs(001): checkpoint 1"),
        "{subjects:?}"
    );
    assert!(
        at("docs(001): checkpoint 1") < at("feat(001): slice-03 C"),
        "{subjects:?}"
    );
    let sha = sha_of(&fx, "docs(001): checkpoint 1");
    assert!(
        files_of(&fx, &sha).contains(&LOG.to_string()),
        "{:?}",
        files_of(&fx, &sha)
    );
    let done: Vec<u64> = of_kind(&dir, "checkpoint_done")
        .iter()
        .map(|e| e["n"].as_u64().unwrap())
        .collect();
    assert_eq!(done, [1, 2]);
    assert_eq!(of_kind(&dir, "checkpoint_done")[0]["sha"], sha);
    let workers = db(&fx).workers(&run_id(&dir)).unwrap();
    assert_eq!(workers[2].outcome.as_deref(), Some("done"));
    assert_eq!(workers[2].commit_sha.as_deref(), Some(sha.as_str()));
}

#[test]
fn a_due_checkpoint_runs_before_a_dead_slice_respawns() {
    // 01 is done, its checkpoint due, while 02's worker still runs; then 02 dies.
    let fx = Fixture::with_index("cp-before-respawn", &[("checkpoint_every", "1")]);
    fx.write_plan(&[
        done(),
        json!({"outcome": "dead", "delay_ms": 1500}),
        step("checkpoint"),
        done(),
        step("checkpoint"),
        done(),
        step("checkpoint"),
    ]);
    let dir = start(&fx);
    wait_phase(&fx, &dir, "finished", 90);
    assert_eq!(
        prompts(&fx),
        [
            build(1),
            build(2),
            checkpoint(1),
            build(2),
            checkpoint(2),
            build(3),
            checkpoint(3),
            polish()
        ],
        "{}",
        driver_log(&dir)
    );
}

#[test]
fn a_resumed_checkpoint_counts_the_reds_its_dead_driver_saw() {
    let fx = Fixture::new("cp-reds-resume");
    fx.chain();
    fx.script_gate(&["red", "red", "red", "red", "red"]);
    fx.write_plan(&[
        done(),
        done(),
        step("fix"),
        json!({"outcome": "fix", "delay_ms": 3000}),
        step("fix"),
        step("fix"),
    ]);
    let dir = start(&fx);
    let driver = state(&dir)["pid"].as_u64().unwrap() as u32;
    wait_until(&fx, &dir, "the second fix worker", 60, || {
        count(&kinds(&dir), "fix_spawned") == 2
    });
    assert_eq!(unsafe { libc::kill(driver as i32, libc::SIGKILL) }, 0);
    wait_until(&fx, &dir, "the driver to die", 5, || !pid_alive(driver));
    let fixer = state(&dir)["workers"].as_array().unwrap().last().unwrap()["pid"]
        .as_u64()
        .unwrap() as u32;
    wait_until(&fx, &dir, "the orphan fix worker to exit", 20, || {
        !pid_alive(fixer)
    });

    let out = factory(&fx, &["run", "001"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    wait_phase(&fx, &dir, "waiting-owner", 60);
    assert_eq!(
        count(&kinds(&dir), "fix_spawned"),
        2,
        "{}",
        driver_log(&dir)
    );
    let rows = gates(&fx, &dir);
    assert_eq!(gate_shape(&rows, 1).len(), 3, "{rows:?}");
    let blocks = fx.log_of("block");
    assert_eq!(blocks.len(), 1, "{blocks:?}");
    assert_eq!(blocks[0]["phase"], "checkpoint");
    let log = body(&blocks[0]);
    assert!(
        log.contains("was red 3 times in a row at checkpoint 1; 2 fix worker(s) ran"),
        "{log}"
    );
    assert!(
        log.contains("the gate stayed red through 2 fix worker(s)"),
        "{log}"
    );
    assert_eq!(count(&kinds(&dir), "checkpoint_blocked"), 1);
}
