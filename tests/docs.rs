//! The `docs.mjs` wrapper against the fixture (PRD-001 slice 02 task 1).
#[path = "harness/mod.rs"]
mod harness;

use std::fs;

use factory::docs::Docs;
use harness::Fixture;

#[test]
fn status_and_preflight_deserialize_from_the_fixture() {
    let fx = Fixture::new("docs");
    let docs = Docs::locate(fx.path()).unwrap();
    assert!(
        docs.tool().starts_with(fx.path()),
        "the vendored tool wins: {}",
        docs.tool().display()
    );

    let s = docs.status("001").unwrap();
    assert_eq!((s.number, s.slug.as_str()), (1, "fixture"));
    assert_eq!(s.slices.len(), 3);
    assert_eq!(s.ready.len(), 2);
    assert_eq!(s.spawnable, 2);
    assert_eq!(s.lanes, 2);
    assert_eq!(s.free_lanes, vec![1, 2]);
    assert_eq!(s.slices[2].depends_on, vec![2]);
    assert_eq!(s.slices[0].status.as_deref(), Some("pending"));
    assert_eq!(s.next.as_ref().map(|n| n.number), Some(1));
    assert_eq!(s.gate.declared.as_deref(), Some("./gate.sh"));
    assert_eq!(s.gate.status, "ok");
    assert!(!s.checkpoint_due && !s.all_done && s.sliced && s.qa_plan);
    assert_eq!(s.branch.as_deref(), Some("main"));
    assert_eq!(s.log.decisions, 0);

    let (p, code) = docs.preflight("001").unwrap();
    assert_eq!(code, 0, "{p:?}");
    assert!(p.pass && p.refuse.is_empty() && p.ask.is_empty(), "{p:?}");
    // The tool always has something to judge (the harness line is unconditional).
    assert!(!p.judge.is_empty(), "{p:?}");
    assert_eq!(p.branch.current.as_deref(), Some("main"));
    assert_eq!(p.branch.default.as_deref(), Some("main"));
    assert_eq!(p.branch.feature.as_deref(), Some("feat/fixture"));
    assert_eq!(p.branch.alt.as_deref(), Some("feat/001-fixture"));
    assert!(!p.branch.feature_exists);
    assert_eq!(p.feature.as_ref().map(|f| f.number), Some(1));

    assert_eq!(docs.gate().unwrap(), "./gate.sh");
    assert_eq!(docs.gate_inner().unwrap(), "./gate.sh --inner");

    // An uncommitted file: exit 1 and a refuse line, still parsed.
    fs::write(fx.path().join("dirty.txt"), "x\n").unwrap();
    let (p, code) = docs.preflight("001").unwrap();
    assert_eq!(code, 1);
    assert!(!p.pass);
    assert!(
        p.refuse
            .iter()
            .any(|r| r.contains("working tree not clean")),
        "{:?}",
        p.refuse
    );
}

#[test]
fn errors_name_the_command_and_quote_stderr() {
    let fx = Fixture::new("docs-err");
    let docs = Docs::locate(fx.path()).unwrap();
    let err = docs.status("999").unwrap_err().to_string();
    assert!(err.contains("docs status 999 --json"), "{err}");
    assert!(err.contains("999"), "{err}");
    // `docs new` creates agent-log.md (AD-503), so a fresh fixture has a log
    // and no fragments: the tool says so and exits 0.
    let log = fx.path().join("docs/features/001-fixture/agent-log.md");
    assert!(log.exists(), "docs new created the log");
    let merge = docs.log_merge("001").unwrap();
    assert_eq!(merge.code, 0, "{merge:?}");
    assert!(merge.stdout.contains("nothing to merge"), "{merge:?}");
    // `log merge` hands back the tool's verdict rather than erroring: with
    // the log gone, the tool says so and exits 2.
    fs::remove_file(&log).unwrap();
    let merge = docs.log_merge("001").unwrap();
    assert_eq!(merge.code, 2, "{merge:?}");
    assert!(merge.stderr.contains("nothing to merge into"), "{merge:?}");

    let missing = Docs::locate(fx.path().join("nowhere").as_path());
    // Either the skill's global copy exists on this machine or nothing does;
    // both are a located tool or a clear error, never a panic.
    if let Err(e) = missing {
        assert!(e.to_string().contains("no docs.mjs"), "{e}");
    }
}
