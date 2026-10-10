//! Slice 06 of PRD-001: checkpoints (PRD §6 FR-4, §3 FR-1 bullet 3, §10
//! FR-8). When the board says a checkpoint is due the driver drains the
//! lanes, runs the gate itself (one `gates` row per run, cached on the code
//! tree), spawns a fix worker on every red and commits its files, blocks
//! after three reds, folds the slice logs, hands the green transcript to a
//! checkpoint worker that writes `CP-n`, and commits the checkpoint. A run
//! killed mid-checkpoint resumes at the checkpoint; the end of a run is its
//! last checkpoint.
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
const AGENT_LOG: &str = "docs/features/001-fixture/agent-log.md";
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

/// The kinds from the first `from` on.
fn kinds_from(dir: &Path, from: &str) -> Vec<String> {
    let k = kinds(dir);
    let i = k
        .iter()
        .position(|x| x == from)
        .unwrap_or_else(|| panic!("no {from} in {k:?}"));
    k[i..].to_vec()
}

/// The kinds after the first `after`.
fn kinds_after(dir: &Path, after: &str) -> Vec<String> {
    kinds_from(dir, after)[1..].to_vec()
}

fn db(fx: &Fixture) -> Db {
    Db::open(&data_dir(fx).join("factory.db")).unwrap()
}

/// The driver's gate rows. Since slice 08 a worker's own inner-loop run is
/// a `gates` row too (source `worker`, FR-8); this file is about the driver's.
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

/// Polls `ok`; on timeout, dumps the run so a failure in the detached
/// driver can be read from the test output.
fn wait_until(fx: &Fixture, dir: &Path, what: &str, secs: u64, mut ok: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(secs);
    while !ok() {
        if Instant::now() >= deadline {
            let statuses: Vec<String> = (1..=3)
                .map(|n| format!("{n:02}={}", fx.slice_status(n)))
                .collect();
            panic!(
                "timed out after {secs}s waiting for {what}\n--- state.json\n{}\n--- events\n{:?}\n--- slices {statuses:?}\n--- git log\n{}\n--- driver.log\n{}\n--- agent-log.md\n{}",
                state(dir),
                kinds(dir),
                fx.git(&["log", "--format=%h %s"]),
                driver_log(dir),
                fs::read_to_string(fx.path().join(AGENT_LOG)).unwrap_or_default()
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

/// Subjects, oldest first.
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

/// The files a commit touches, sorted.
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

fn fragment(n: u8) -> String {
    format!("{FEATURE_DIR}/log-slice-{n:02}.md")
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

/// The pack file a call was given, and its text.
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

/// `(red, cached, attempt)` of the rows for checkpoint `n`, in id order; every
/// row is the driver's checkpoint row with the fixture's gate command.
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

fn assert_cp(log: &str, n: u32, after: u8, covers: &str) {
    let heading = format!("### CP-{n} — after slice-{after:02} · ");
    let i = log
        .find(&heading)
        .unwrap_or_else(|| panic!("{heading} missing from\n{log}"));
    let entry = &log[i..];
    let entry = &entry[..entry[4..].find("\n### ").map_or(entry.len(), |j| j + 4)];
    assert!(
        entry.contains(&format!("**Covers:** slices {covers}")),
        "{entry}"
    );
    assert!(entry.contains("**Gate:**"), "{entry}");
    assert!(entry.contains("**Prove:** FR-1"), "{entry}");
    assert!(entry.contains("**Fixed:** none"), "{entry}");
}

/// What every finished fixture run with two checkpoints has in common.
fn assert_two_checkpoints(fx: &Fixture, dir: &Path) {
    for n in 1..=3 {
        assert_eq!(fx.slice_status(n), "done", "slice {n:02}");
    }
    let log = fx.read(AGENT_LOG);
    assert_cp(&log, 1, 2, "01, 02");
    assert_cp(&log, 2, 3, "03");
    for ad in ["AD-101", "AD-201", "AD-301"] {
        assert!(
            log.contains(&format!("### {ad} —")),
            "{ad} missing from\n{log}"
        );
    }
    for n in 1..=3 {
        assert!(
            !fx.path().join(fragment(n)).exists(),
            "fragment {n} survived"
        );
    }
    let board = fx.docs_json("status");
    assert_eq!(board["unverified"], json!([]), "{board}");
    assert_eq!(board["checkpointDue"], false, "{board}");
    assert_eq!(
        board["checkpoints"].as_array().map(Vec::len),
        Some(2),
        "{board}"
    );
    let prd = fx.read(&format!("{FEATURE_DIR}/prd.md"));
    assert_eq!(prd.matches("| ✅ done |").count(), 3, "{prd}");
    for n in 1..=2u32 {
        let sha = sha_of(fx, &format!("docs: checkpoint {n}"));
        let files = files_of(fx, &sha);
        assert!(files.contains(&AGENT_LOG.to_string()), "CP-{n}: {files:?}");
        assert!(
            files.iter().all(|f| f.starts_with("docs/")),
            "CP-{n} commit holds code: {files:?}"
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
    let f1 = files_of(fx, &sha_of(fx, "docs: checkpoint 1"));
    assert!(
        f1.contains(&fragment(1)) && f1.contains(&fragment(2)),
        "{f1:?}"
    );
    assert!(f1.contains(&format!("{FEATURE_DIR}/prd.md")), "{f1:?}");
    assert_eq!(fx.git(&["status", "--porcelain"]), "");
    fx.git(&["fsck", "--strict"]);
    assert_eq!(state(dir)["phase"], "finished");
    assert_eq!(kinds(dir).last().map(String::as_str), Some("run_finished"));
    let text = driver_log(dir);
    // Checkpoint 1 reads "started" on a straight run and "resumed" after a
    // kill; the callers check which.
    assert!(text.contains("checkpoint 1 (covers 01, 02) — "), "{text}");
    assert!(
        text.contains("checkpoint 2 (covers 03) — started"),
        "{text}"
    );
    assert!(text.contains("finished (every slice done)"), "{text}");
}

// ── tasks 1 and 4: the gate runs before slice 03, CP-1 and CP-2 land ────────

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

    // Spawn order: the checkpoint sits between slice 02 and slice 03.
    assert_eq!(
        prompts(&fx),
        [build(1), build(2), checkpoint(1), build(3), checkpoint(2)]
    );
    assert_eq!(
        roles(&fx, &dir),
        ["build", "build", "checkpoint", "build", "checkpoint"]
    );
    assert_eq!(
        subjects(&fx),
        [
            "chore: fixture repo",
            "chore: chain the fixture slices",
            "feat: A",
            "feat: B",
            "docs: checkpoint 1",
            "feat: C",
            "docs: checkpoint 2",
        ]
    );

    // One gate run per checkpoint, each a `gates` row with a transcript.
    let rows = gates(&fx, &dir);
    assert_eq!(rows.len(), 2, "{rows:?}");
    assert_eq!(gate_shape(&rows, 1), [(false, false, 1)]);
    assert_eq!(gate_shape(&rows, 2), [(false, false, 1)]);
    let log = fx.gate_log();
    assert_eq!(log.len(), 2, "gate.log");
    assert!(log.iter().all(|g| g.kind == "gate" && g.exit == 0));
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

    // The checkpoint worker: its argv, its pack, its rows.
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
        "`### CP-1 — after slice-02 · {date}`",
        "`docs/features/001-fixture/agent-log.md`",
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
    assert!(
        pack.contains("`### CP-2 — after slice-03 · {date}`"),
        "{pack}"
    );
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
    let merged = of_kind(&dir, "log_merged");
    assert_eq!(merged.len(), 2);
    assert_eq!(merged[0]["fragments"], json!([fragment(1), fragment(2)]));
    assert_eq!(merged[1]["fragments"], json!([fragment(3)]));
    assert!(merged[0].get("sha").is_none(), "{}", merged[0]);

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
            "log_merged",
            "checkpoint_spawned",
            "checkpoint_done",
            "slice_spawned",
            "slice_done",
            "slice_committed",
            "checkpoint_started",
            "gate_run",
            "log_merged",
            "checkpoint_spawned",
            "checkpoint_done",
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

// ── task 2: the cache on the code tree ──────────────────────────────────────

#[test]
fn an_unchanged_code_tree_reuses_the_cached_gate_result() {
    let fx = Fixture::new("cp-cache");
    fx.chain();
    // Slice 03 touches docs only: the tree the gate tests is the one that was
    // green at checkpoint 1.
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
    let c = files_of(&fx, &sha_of(&fx, "feat: C"));
    assert!(c.contains(&"docs/notes.txt".to_string()), "{c:?}");
    assert_two_checkpoints(&fx, &dir);
}

// ── task 3: red, red, green ─────────────────────────────────────────────────

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
            checkpoint(2)
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
            "checkpoint"
        ]
    );
    // Three gate runs at checkpoint 1, one at checkpoint 2 (slice 03 touched
    // code); every one a row, none cached — each fix changed the tree.
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
    assert_eq!(fs::read_to_string(dir.join("gates/1-3.log")).unwrap(), "");

    // Two fix commits, each only the file the fix worker named.
    assert_eq!(
        subjects(&fx),
        [
            "chore: fixture repo",
            "chore: chain the fixture slices",
            "feat: A",
            "feat: B",
            "fix: gate red at checkpoint 1",
            "fix: gate red at checkpoint 1",
            "docs: checkpoint 1",
            "feat: C",
            "docs: checkpoint 2",
        ]
    );
    let fixes: Vec<String> = fx
        .git(&[
            "log",
            "--reverse",
            "--format=%H",
            "--grep",
            "^fix: gate red at checkpoint 1$",
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

    // The fix pack: the failing tail, both commands, the fix contract.
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
        "`fix: gate red at checkpoint 1`",
        "start at AD-9101",
        "# Autonomy",
        "# Inner loop",
    ] {
        assert!(
            pack.contains(want),
            "{want:?} missing from the fix pack:\n{pack}"
        );
    }
    assert!(!pack.contains("# Slice"), "{pack}");
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
            "log_merged",
            "checkpoint_spawned",
            "checkpoint_done",
            "slice_spawned",
            "slice_done",
            "slice_committed",
            "checkpoint_started",
            "gate_run",
            "log_merged",
            "checkpoint_spawned",
            "checkpoint_done",
            "run_finished",
        ]
    );
    assert_two_checkpoints(&fx, &dir);
}

// ── task 3: three reds block; the answer reruns the gate ────────────────────

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

    // Two fix workers ran (the third red blocks instead), three rows, no CP.
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
            "feat: A",
            "feat: B",
            "fix: gate red at checkpoint 1",
            "fix: gate red at checkpoint 1",
        ]
    );
    for n in 1..=2 {
        assert!(
            fx.path().join(fragment(n)).exists(),
            "fragment {n} merged before green"
        );
    }
    let log = fx.read(AGENT_LOG);
    assert!(log.starts_with("# Agent log — PRD-001\n"), "{log}");
    let title = format!("gate red three times at checkpoint 1: {FAIL_LINE}");
    assert!(
        log.contains(&format!("### BLOCK-1 — {title} · checkpoint · open")),
        "{log}"
    );
    assert!(log.contains("**Happened:** the gate (`./gate.sh`) was red 3 times in a row at checkpoint 1; 2 fix worker(s) ran"), "{log}");
    assert!(log.contains(&format!("  {FAIL_LINE}\n")), "{log}");
    assert!(log.contains("**Recommended:** 1"), "{log}");
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
            "parked (waiting-owner): checkpoint blocked — {title}: BLOCK-1 in agent-log.md waits for the owner: `factory answer BLOCK-1 \"<text>\"`"
        )),
        "{text}"
    );
    let open = &fx.docs_json("status")["openBlocks"];
    assert_eq!(open.as_array().map(Vec::len), Some(1), "{open}");
    assert_eq!(open[0]["n"], 1);
    assert_eq!(open[0]["phase"], "checkpoint");

    // The owner answers without touching the tree: the gate runs again all
    // the same (a cached red is not trusted past an answer), green now.
    let out = factory(&fx, &["answer", "BLOCK-1", "fixed the runner by hand"]);
    let text = stdout(&out);
    assert_eq!(out.status.code(), Some(0), "{text}");
    assert!(text.starts_with("BLOCK-1 resolved ("), "{text}");
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
            "log_merged",
            "checkpoint_spawned",
            "checkpoint_done",
            "slice_spawned",
            "slice_done",
            "slice_committed",
            "checkpoint_started",
            "gate_run",
            "log_merged",
            "checkpoint_spawned",
            "checkpoint_done",
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
            checkpoint(2)
        ]
    );
    let log = fx.read(AGENT_LOG);
    assert!(
        log.contains(&format!("### BLOCK-1 — {title} · checkpoint · resolved")),
        "{log}"
    );
    assert!(log.contains("**Resolution ("), "{log}");
    assert_two_checkpoints(&fx, &dir);
}

// ── task 5: a run killed under the checkpoint worker resumes there ──────────

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
    assert!(!fx.read(AGENT_LOG).contains("### CP-1"));

    // Resume: the orphan is settled, the gate is a cache hit, a fresh
    // checkpoint worker writes CP-1, and the run goes on to the end.
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
            "log_merged",
            "checkpoint_spawned",
            "checkpoint_done",
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
            checkpoint(2)
        ]
    );
    let workers = db(&fx).workers(&run_id(&dir)).unwrap();
    assert_eq!(workers.len(), 6);
    assert_eq!(workers[2].outcome.as_deref(), Some("dead"));
    assert_eq!(workers[2].checkpoint, Some(1));
    assert_eq!(workers[3].outcome.as_deref(), Some("done"));
    let log = fx.read(AGENT_LOG);
    assert!(
        log.contains("- **Dead checkpoint worker (")
            && log.contains(&format!(
                "checkpoint 1, session {sid0} exited (code unknown) with no result."
            )),
        "{log}"
    );
    let text = driver_log(&dir);
    assert!(
        text.contains("checkpoint 1 (covers 01, 02) — resumed"),
        "{text}"
    );
    assert_two_checkpoints(&fx, &dir);
}

// ── task 4: two dead checkpoint workers block; the answer spawns a third ────

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
            "log_merged",
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
    let log = fx.read(AGENT_LOG);
    let title = "Two checkpoint workers died at checkpoint 1";
    assert!(
        log.contains(&format!("### BLOCK-1 — {title} · checkpoint · open")),
        "{log}"
    );
    assert_eq!(
        log.matches("- **Dead checkpoint worker (").count(),
        2,
        "{log}"
    );
    assert!(
        log.contains("### AD-101 —") && log.contains("### AD-201 —"),
        "{log}"
    );
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
    assert_eq!(subjects(&fx).last().map(String::as_str), Some("feat: B"));

    // The answer: the two deaths before it no longer count, a third worker
    // reads the same (cached) transcript and writes CP-1.
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
            "log_merged",
            "checkpoint_spawned",
            "checkpoint_done",
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
            checkpoint(2)
        ]
    );
    let log = fx.read(AGENT_LOG);
    assert!(
        log.contains(&format!("### BLOCK-1 — {title} · checkpoint · resolved")),
        "{log}"
    );
    assert_two_checkpoints(&fx, &dir);
}
