//! `timeline[].label`: a short sentence for the human reading the panel; `kind` keeps the machine name.
use serde_json::json;

use super::harness::Fixture;
use super::{live, write_run};

#[test]
fn live_timeline_labels_read_as_sentences() {
    let fx = Fixture::new("live-labels");
    let long = format!("Should the {} store keep orders?", "very ".repeat(30));
    write_run(
        &fx,
        json!({}),
        &[
            json!({"kind": "slice_done", "slice": 1}),
            json!({"kind": "run_waiting", "blocked": [2], "asks": [], "reason": "slice 02 blocked"}),
            json!({"kind": "checkpoint_blocked", "n": 2, "block_id": "BLOCK-3", "question": long}),
            json!({"kind": "gate_run", "n": 3, "attempt": 1, "red": false, "minutes": 1.74, "log": "gates/3-1.log"}),
            json!({"kind": "gate_run", "n": 3, "attempt": 2, "red": true, "minutes": 0.5, "log": "gates/3-2.log"}),
            json!({"kind": "ci_red", "check": "test", "round": 0, "head": "abc1234def"}),
            json!({"kind": "judge_noted", "text": "Screens  stay\nas they are"}),
            json!({"kind": "slice_committed", "slice": 4, "sha": "0123456789abcdef", "files": ["a", "b"]}),
            json!({"kind": "run_finished", "slices": 9, "duration_minutes": 84.4}),
            json!({"kind": "a_kind_from_a_newer_binary"}),
            json!({"kind": "ask_opened", "id": "ASK-1", "text": format!("Use token={} here?", "s3cr3t".repeat(20))}),
        ],
    );

    let v = live(&fx, &[]);
    let t = v["timeline"].as_array().unwrap();
    let labels: Vec<&str> = t.iter().map(|e| e["label"].as_str().unwrap()).collect();
    let checkpoint = format!("checkpoint 2 blocked: {}", &long[..57]);
    let checkpoint = format!("{}…", checkpoint.trim_end_matches(' '));
    assert_eq!(
        labels,
        [
            "slice 01 done",
            "waiting on the owner",
            checkpoint.as_str(),
            "gate 3 green in 1.7 min",
            "gate 3 red, attempt 2",
            "CI red: test",
            "judge: Screens stay as they are",
            "slice 04 committed 0123456 (2 files)",
            "run finished: 9 slices in 84 min",
            "an event from a newer factory",
            "ASK-1 asks: Use token=[redacted] here?",
        ]
    );
    for e in t {
        let label = e["label"].as_str().unwrap();
        assert!(!label.contains('_'), "{label}");
        assert_ne!(label, e["kind"].as_str().unwrap());
        assert!(label.chars().count() <= 80, "{label}");
    }
}
