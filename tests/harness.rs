//! Smoke test for the harness itself.
#[path = "harness/mod.rs"]
mod harness;

use std::path::Path;
use std::time::Instant;

use harness::Fixture;

#[test]
fn fixture_is_a_runnable_board_with_stubs_on_path() {
    let t0 = Instant::now();
    let fx = Fixture::new("smoke");
    let built = t0.elapsed();
    assert!(built.as_secs_f64() < 3.0, "Fixture::new took {built:?}");

    let out = fx
        .command("sh")
        .args(["-c", "command -v claude; command -v gh"])
        .output()
        .unwrap();
    let found = String::from_utf8(out.stdout).unwrap();
    let stubs = harness::stubs_dir();
    assert_eq!(found.lines().count(), 2, "{found}");
    for line in found.lines() {
        assert!(
            Path::new(line).starts_with(&stubs),
            "{line} is not under {}",
            stubs.display()
        );
    }

    fx.docs(&["doctor"]);
    assert_eq!(fx.docs(&["gate"]).trim(), "./gate.sh");
    assert_eq!(fx.docs(&["gate", "--inner"]).trim(), "./gate.sh --inner");

    let s = fx.docs_json("status");
    assert_eq!(s["slices"].as_array().unwrap().len(), 3, "{s}");
    assert_eq!(s["ready"].as_array().unwrap().len(), 2, "{s}");
    assert_eq!(s["spawnable"], 2, "{s}");
    assert_eq!(s["checkpointDue"], false, "{s}");
    assert_eq!(s["lanes"], 2, "{s}");
    assert_eq!(s["slices"][2]["depends_on"], serde_json::json!([2]), "{s}");
    let p = fx.docs_json("preflight");
    assert_eq!(p["pass"], true, "{p}");
    assert_eq!(p["ask"], serde_json::json!([]), "{p}");

    fx.script_gate(&["red", "red", "green"]);
    let first = fx.run_gate_output(false);
    assert_eq!(first.status.code(), Some(1));
    assert_eq!(
        String::from_utf8_lossy(&first.stdout).trim(),
        "FAIL tests/fake.rs: expected 2 got 3"
    );
    let codes: Vec<i32> = (0..3).map(|_| fx.run_gate(false)).collect();
    assert_eq!(codes, [1, 0, 0]);
    let log = fx.gate_log();
    assert_eq!(log.iter().map(|r| r.exit).collect::<Vec<_>>(), [1, 1, 0, 0]);
    assert!(log.iter().all(|r| r.kind == "gate"), "{log:?}");

    fx.script_gate(&["red"]);
    fx.script_inner(&["red", "green"]);
    assert_eq!(
        [fx.run_gate(true), fx.run_gate(true), fx.run_gate(true)],
        [1, 0, 0]
    );
    assert_eq!(fx.read("scenario/gate").trim(), "red");
    let log = fx.gate_log();
    assert_eq!(log.len(), 7);
    assert!(log[4..].iter().all(|r| r.kind == "inner"), "{log:?}");
    assert_eq!(fx.run_gate(false), 1);

    // Nothing above dirtied the tree the driver commits from.
    assert_eq!(fx.git(&["status", "--porcelain"]).trim(), "");

    println!(
        "harness smoke: Fixture::new in {built:?}, whole test in {:?}",
        t0.elapsed()
    );
}

#[test]
fn gate_sh_reads_the_main_trees_scenario_from_a_worktree() {
    let fx = Fixture::new("gate-worktree");
    let tmp = tempfile::tempdir().unwrap();
    let wt = tmp.path().join("wt");
    fx.git(&["worktree", "add", "--detach", wt.to_str().unwrap(), "HEAD"]);
    let head = fx.git(&["rev-parse", "HEAD"]).trim().to_string();

    fx.script_gate(&["red"]);
    let out = fx
        .command(wt.join("gate.sh"))
        .current_dir(&wt)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert!(!wt.join("scenario").exists());
    let log = fx.gate_log();
    assert_eq!(log.len(), 1, "{log:?}");
    assert_eq!(log[0].kind, "gate");
    assert_eq!(log[0].exit, 1);
    assert_eq!(log[0].pwd, wt.canonicalize().unwrap());
    assert_eq!(log[0].head, head);

    assert_eq!(fx.run_gate(false), 0);
    let log = fx.gate_log();
    assert_eq!(log.len(), 2, "{log:?}");
    assert_eq!(log[1].pwd, fx.path().canonicalize().unwrap());
    assert_eq!(log[1].head, head);
}
