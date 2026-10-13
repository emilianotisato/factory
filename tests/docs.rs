//! `crate::docs::Docs`, the driver's in-process docs library, against the v3
//! fixture (PRD-002 FR-18, slice 09 task 1). It replaced the `docs.mjs`
//! subprocess wrapper PRD-001 slice 02 tested here.
#[path = "harness/mod.rs"]
mod harness;

use std::fs;

use factory::docs::log::{Entry, Kind};
use factory::docs::{Docs, V2_HINT};
use factory::error::Error;
use harness::Fixture;

#[test]
fn status_and_preflight_come_from_the_fixture() {
    let fx = Fixture::new("docs");
    let docs = Docs::open(fx.path()).unwrap();

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
    // In process and through the binary, one board.
    assert_eq!(serde_json::to_value(&s).unwrap(), fx.docs_json("status"));

    let (p, code) = docs.preflight("001").unwrap();
    assert_eq!(code, 0, "{p:?}");
    assert!(p.pass && p.refuse.is_empty() && p.ask.is_empty(), "{p:?}");
    assert!(!p.judge.is_empty(), "{p:?}");
    assert_eq!(p.branch.current.as_deref(), Some("main"));
    assert_eq!(p.branch.feature.as_deref(), Some("feat/fixture"));
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
fn the_driver_writes_state_and_log_in_process() {
    let fx = Fixture::new("docs-write");
    let docs = Docs::open(fx.path()).unwrap();

    docs.set_slice_status("001", 2, "building").unwrap();
    let s = docs.status("001").unwrap();
    assert_eq!(s.slices[1].status.as_deref(), Some("building"));
    assert_eq!(fx.slice_status(2), "building");

    let mut block = Entry::new(Kind::Block, "Which row?", "**Happened:** x");
    block.phase = Some("checkpoint".into());
    assert_eq!(
        docs.log_add("001", block).unwrap().as_deref(),
        Some("BLOCK-1")
    );
    let s = docs.status("001").unwrap();
    assert_eq!(s.open_blocks.len(), 1, "{:?}", s.open_blocks);
    assert_eq!(s.open_blocks[0].n, 1);
    assert_eq!(s.open_blocks[0].phase.as_deref(), Some("checkpoint"));
    let entries = docs.log_entries("001").unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].id.as_deref(), Some("BLOCK-1"));

    docs.set_reviewed("001", "2026-10-13").unwrap();
    assert_eq!(fx.state().reviewed.as_deref(), Some("2026-10-13"));
}

#[test]
fn a_repo_off_the_framework_is_refused_and_an_unknown_feature_errors() {
    let dir = tempfile::tempdir().unwrap();
    match Docs::open(dir.path()) {
        Err(Error::NotV3(text)) => assert_ne!(text, V2_HINT),
        Err(e) => panic!("no docs at all: {e}"),
        Ok(_) => panic!("a bare dir opened"),
    }
    let fx = Fixture::new("docs-err");
    let docs = Docs::open(fx.path()).unwrap();
    let err = docs.status("999").unwrap_err().to_string();
    assert!(err.contains("999"), "{err}");
}
