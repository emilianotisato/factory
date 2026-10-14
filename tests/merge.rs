//! PRD-005 slice 08: a landing that conflicts spawns a merge worker (FR-4).
//! Slices 01 and 02 start side by side from one tip and rewrite line 3 of
//! `shared.txt`; 01 lands first, so 02's pick conflicts. The driver leaves
//! the main tree untouched, redoes the pick in 02's lane, sets 02 back to
//! `building` and spawns a merge worker there. A clean exit with no marker
//! left lands the lane; markers left, a block the worker logged, or two
//! merge deaths block the slice.
#[path = "harness/mod.rs"]
mod harness;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;
use std::time::{Duration, Instant};

use factory::db::Db;
use harness::Fixture;
use serde_json::{Value, json};

const BIN: &str = env!("CARGO_BIN_EXE_factory");

fn factory(fx: &Fixture, args: &[&str]) -> Output {
    fx.command(BIN).args(args).output().unwrap()
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn data_dir(fx: &Fixture) -> PathBuf {
    PathBuf::from(&fx.env()["FACTORY_DATA_DIR"])
}

fn only_run_dir(fx: &Fixture) -> PathBuf {
    let mut v: Vec<PathBuf> = fs::read_dir(data_dir(fx).join("runs"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    v.sort();
    assert_eq!(v.len(), 1, "{v:?}");
    v.pop().unwrap()
}

fn run_id(dir: &Path) -> String {
    dir.file_name().unwrap().to_str().unwrap().to_string()
}

fn state(dir: &Path) -> Value {
    serde_json::from_str(&fs::read_to_string(dir.join("state.json")).unwrap()).unwrap()
}

fn kinds(dir: &Path) -> Vec<String> {
    fs::read_to_string(dir.join("events.jsonl"))
        .unwrap_or_default()
        .lines()
        .map(|l| {
            serde_json::from_str::<Value>(l).expect(l)["kind"]
                .as_str()
                .unwrap()
                .to_string()
        })
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
                "timed out after {secs}s waiting for {what}\n--- events\n{:?}\n--- slices {statuses:?}\n--- git log\n{}\n--- driver.log\n{}",
                kinds(dir),
                fx.git(&["log", "--format=%h %s"]),
                driver_log(dir)
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

fn stop(fx: &Fixture, dir: &Path) {
    let out = factory(fx, &["stop", &run_id(dir)]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    wait_phase(fx, dir, "finished", 30);
}

const SLICE_01: &str = "feat(001): slice-01 A";
const SLICE_02: &str = "feat(001): slice-02 B";

/// The fixture with a committed `shared.txt`, and a plan whose first two
/// steps rewrite its line 3 in slices 01 and 02; 02 waits, so 01 lands
/// first. `merge` is the steps after.
fn conflicting(name: &str, merge: &[Value]) -> (Fixture, PathBuf) {
    let fx = Fixture::with_index(name, &[("checkpoint_every", "10")]);
    fs::write(fx.path().join("shared.txt"), "1\n2\n3\n4\n").unwrap();
    fx.git(&["add", "shared.txt"]);
    fx.git(&["commit", "-q", "-m", "chore: a shared file"]);
    let mut plan = vec![
        json!({"outcome": "done", "sh": "sed -i '3s/.*/one/' shared.txt"}),
        json!({"outcome": "done", "delay_ms": 1500, "sh": "sed -i '3s/.*/two/' shared.txt"}),
    ];
    plan.extend_from_slice(merge);
    fx.write_plan(&plan);
    let out = factory(&fx, &["run", "001"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    let dir = only_run_dir(&fx);
    (fx, dir)
}

fn head_subject(fx: &Fixture) -> String {
    fx.git(&["log", "-1", "--format=%s"]).trim().to_string()
}

fn factory_refs(fx: &Fixture) -> String {
    fx.git(&["for-each-ref", "refs/factory/"])
}

fn blocks_of(fx: &Fixture, slice: u64) -> Vec<Value> {
    fx.log_of("block")
        .into_iter()
        .filter(|e| e["slice"] == slice)
        .collect()
}

// ── task 2: the merge worker runs in the slice's lane ──────────────────────

#[test]
fn a_conflicting_landing_spawns_a_merge_worker_in_the_slices_lane() {
    let (fx, dir) = conflicting(
        "merge-spawn",
        &[
            json!({"outcome": "done", "delay_ms": 3000, "sh": "printf '1\\n2\\nboth\\n4\\n' > shared.txt"}),
        ],
    );
    wait_until(&fx, &dir, "the merge call", 30, || fx.calls().len() >= 3);
    let calls = fx.calls();
    assert_eq!(calls[2]["cwd"], calls[1]["cwd"], "{calls:?}");
    let pack = calls[2]["pack"].as_str().unwrap();
    let body = pack.split_once("\n\n").unwrap().1;
    assert!(body.starts_with("# Conflict\n"), "{pack}");
    assert!(pack.contains("shared.txt"), "{pack}");
    let session = calls[2]["session_id"].as_str().unwrap();
    let db = Db::open(&data_dir(&fx).join("factory.db")).unwrap();
    let rows = db.workers(&run_id(&dir)).unwrap();
    let row = rows
        .iter()
        .find(|w| w.session_id.as_deref() == Some(session))
        .unwrap_or_else(|| panic!("no workers row for {session}"));
    assert_eq!(row.role, "merge");
    assert_eq!(row.slice, Some(2));

    // While it runs: 02 is building again, and the main tree has 01 only.
    assert_eq!(fx.slice_status(2), "building");
    assert_eq!(head_subject(&fx), SLICE_01);
    assert_eq!(fx.read("shared.txt"), "1\n2\none\n4\n");
    assert!(kinds(&dir).contains(&"landing_conflicted".to_string()));
    assert!(kinds(&dir).contains(&"merge_spawned".to_string()));

    wait_until(&fx, &dir, "slice 02 done", 30, || {
        fx.slice_status(2) == "done"
    });
    stop(&fx, &dir);
}

// ── task 3 (a): a resolution lands ─────────────────────────────────────────

#[test]
fn a_resolved_merge_lands_after_the_slice_it_conflicted_with() {
    let (fx, dir) = conflicting(
        "merge-resolved",
        &[
            json!({"outcome": "done", "sh": "printf '1\\n2\\nboth\\n4\\n' > shared.txt"}),
            json!({"outcome": "done"}),
            json!({"outcome": "checkpoint"}),
        ],
    );
    wait_phase(&fx, &dir, "finished", 60);

    let subjects: Vec<String> = fx
        .git(&["log", "--reverse", "--format=%s"])
        .lines()
        .map(str::to_string)
        .collect();
    let at = subjects
        .iter()
        .position(|s| s == SLICE_01)
        .expect("slice 01");
    assert_eq!(
        subjects.get(at + 1).map(String::as_str),
        Some(SLICE_02),
        "{subjects:?}"
    );
    assert_eq!(
        subjects.iter().filter(|s| *s == SLICE_02).count(),
        1,
        "{subjects:?}"
    );
    assert_eq!(fx.read("shared.txt"), "1\n2\nboth\n4\n");
    let sha = fx.git(&[
        "log",
        "-1",
        "--format=%H",
        "--grep",
        &format!("^{SLICE_02}$"),
    ]);
    let files = fx.git(&[
        "diff-tree",
        "--no-commit-id",
        "--name-only",
        "-r",
        sha.trim(),
    ]);
    assert!(files.lines().any(|f| f == "shared.txt"), "{files}");
    assert!(files.lines().any(|f| f == "src/slice-02.txt"), "{files}");
    assert_eq!(fx.slice_status(2), "done");
    assert_eq!(fx.git(&["status", "--porcelain"]), "");
    assert_eq!(factory_refs(&fx), "");
    assert!(blocks_of(&fx, 2).is_empty(), "{:?}", blocks_of(&fx, 2));
}

// ── task 3 (b): markers left block the slice ───────────────────────────────

#[test]
fn a_merge_that_leaves_markers_blocks_the_slice() {
    let (fx, dir) = conflicting("merge-markers", &[json!({"outcome": "done"})]);
    wait_until(&fx, &dir, "slice 02 to block", 30, || {
        fx.slice_status(2) == "blocked"
    });

    let out = factory(&fx, &["status", &run_id(&dir)]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    assert!(stdout(&out).contains("blocked: 02"), "{}", stdout(&out));
    let blocks = blocks_of(&fx, 2);
    assert_eq!(blocks.len(), 1, "{blocks:?}");
    assert!(
        blocks[0]["title"].as_str().unwrap().contains("shared.txt"),
        "{blocks:?}"
    );
    assert_eq!(head_subject(&fx), SLICE_01);
    let grep = fx
        .command("git")
        .args(["grep", "-n", "^<<<<<<<"])
        .output()
        .unwrap();
    assert_eq!(grep.status.code(), Some(1), "{}", stdout(&grep));
    // The lane keeps the markers for the answer's merge worker.
    let lane = fx.calls()[1]["cwd"].as_str().unwrap().to_string();
    assert!(
        fs::read_to_string(Path::new(&lane).join("shared.txt"))
            .unwrap()
            .contains("<<<<<<<")
    );
    stop(&fx, &dir);
}

// ── task 3 (c): two merge deaths block the slice ───────────────────────────

#[test]
fn two_dead_merge_workers_block_the_slice() {
    let (fx, dir) = conflicting(
        "merge-dead",
        &[json!({"outcome": "dead"}), json!({"outcome": "dead"})],
    );
    wait_until(&fx, &dir, "slice 02 to block", 30, || {
        fx.slice_status(2) == "blocked"
    });
    let calls = fx.calls();
    assert_eq!(calls.len(), 4, "{calls:?}");
    assert_eq!(calls[3]["cwd"], calls[1]["cwd"]);
    assert!(
        calls[3]["pack"]
            .as_str()
            .unwrap()
            .contains("\n# Conflict\n")
    );
    assert_eq!(head_subject(&fx), SLICE_01);
    stop(&fx, &dir);
    assert_eq!(fx.calls().len(), 4, "a third merge worker ran");
}

// ── task 3 (d): a block the merge worker logged ────────────────────────────

#[test]
fn a_block_the_merge_worker_logged_blocks_the_slice() {
    let (fx, dir) = conflicting("merge-blocked", &[json!({"outcome": "blocked"})]);
    // The driver flips the status before it writes its line: wait for both.
    wait_until(&fx, &dir, "slice 02 to block", 30, || {
        fx.slice_status(2) == "blocked"
            && driver_log(&dir).contains("it logged a block, slice blocked")
    });
    // The stub's merge worker never runs `docs set`: the driver flipped it.
    assert_eq!(blocks_of(&fx, 2).len(), 1);
    assert_eq!(head_subject(&fx), SLICE_01);
    stop(&fx, &dir);
}
