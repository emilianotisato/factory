//! `now` and `attention`: what the run waits on and what the owner is asked.
use std::fs;

use serde_json::{Value, json};

use super::harness::{FEATURE_DIR, Fixture};
use super::{ID, factory, live, worker, write_run};

const RESETS_AT: u64 = 1_794_052_800;

fn block(n: u32) -> String {
    format!("BLOCK-{n}")
}

fn ask(n: u32) -> String {
    format!("ASK-{n}")
}

fn me() -> Value {
    json!(std::process::id())
}

fn alive_worker() -> Value {
    let mut w = worker("s1");
    w["pid"] = me();
    w
}

fn log_entries(fx: &Fixture, entries: &[Value]) {
    let text: String = entries.iter().map(|e| format!("{e}\n")).collect();
    fs::write(fx.path().join(FEATURE_DIR).join("log.jsonl"), text).unwrap();
}

fn by_id<'a>(v: &'a Value, id: &str) -> &'a Value {
    v["attention"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["id"] == id)
        .unwrap_or_else(|| panic!("no {id}: {}", v["attention"]))
}

#[test]
fn live_names_what_the_run_waits_on() {
    let fx = Fixture::new("live-now-waiting-on");
    let cases: Vec<(&str, Value, Vec<Value>)> = vec![
        (
            "done",
            json!({"phase": "finished"}),
            vec![
                json!({"kind": "run_started"}),
                json!({"kind": "run_finished"}),
            ],
        ),
        (
            "owner",
            json!({"phase": "waiting-owner"}),
            vec![
                json!({"kind": "slice_blocked", "slice": 1, "block_id": block(1), "question": "Which store?"}),
                json!({"kind": "run_waiting", "blocked": [1], "asks": [], "reason": "a slice blocked"}),
            ],
        ),
        (
            "budget",
            json!({"phase": "waiting-owner"}),
            vec![
                json!({"kind": "run_started"}),
                json!({"kind": "run_parked", "reason": "budget spent", "weighted": 1200, "budget": 1000}),
            ],
        ),
        (
            "quota",
            json!({"phase": "paused", "pid": me(), "workers": [alive_worker()]}),
            vec![
                json!({"kind": "run_started"}),
                json!({"kind": "run_paused", "literal": "limit reached", "resets_at": RESETS_AT}),
            ],
        ),
        ("driver", json!({}), vec![json!({"kind": "run_started"})]),
        (
            "gate",
            json!({"phase": "checkpoint", "pid": me()}),
            vec![
                json!({"kind": "gate_started", "n": 2, "attempt": 2, "cmd": "./gate.sh", "log": "gates/2-2.log"}),
            ],
        ),
        (
            "review",
            json!({"phase": "closeout", "pid": me()}),
            vec![
                json!({"kind": "collect_started", "passes": 3, "reused": false}),
                json!({"kind": "review_pass_done", "k": 1, "of": 3, "seconds": 40}),
            ],
        ),
        (
            "ci",
            json!({"phase": "closeout", "pid": me()}),
            vec![
                json!({"kind": "ci_red", "check": "test", "round": 0, "head": "abc"}),
                json!({"kind": "ci_waiting", "checks": ["test"]}),
            ],
        ),
        (
            "worker",
            json!({"pid": me(), "workers": [alive_worker()]}),
            vec![json!({"kind": "slice_spawned", "slice": 1, "session_id": "s1", "pid": 0})],
        ),
        (
            "nothing",
            json!({"pid": me()}),
            vec![json!({"kind": "run_started"})],
        ),
    ];
    for (want, state, events) in cases {
        write_run(&fx, state, &events);
        let now = live(&fx, &[])["now"].clone();
        assert_eq!(now["waitingOn"], want, "{now}");
        assert!(!now["label"].as_str().unwrap().is_empty(), "{now}");
        assert!(now["idleSeconds"].is_u64(), "{now}");
        match want {
            "quota" => {
                assert_eq!(now["resumesAt"], "2026-11-07T12:00:00Z", "{now}");
                assert_eq!(now["driverAlive"], true);
            }
            "budget" => assert_eq!(now["resumesAt"], Value::Null, "{now}"),
            "review" => assert_eq!(now["progress"], json!({"done": 1, "of": 3}), "{now}"),
            "gate" => assert_eq!(now["label"], "checkpoint 2 gate, attempt 2"),
            "driver" => assert_eq!(now["driverAlive"], false),
            _ => {}
        }
        if want == "nothing" {
            assert_eq!(now["since"], Value::Null);
        } else {
            assert!(now["since"].is_string(), "{now}");
        }
        if want != "review" {
            assert_eq!(now["progress"], Value::Null, "{now}");
        }
        assert_eq!(now["stopping"], Value::Null);
    }
}

#[test]
fn live_says_driver_after_an_answer_nobody_picked_up() {
    let fx = Fixture::new("live-now-unpicked-answer");
    write_run(
        &fx,
        json!({"phase": "waiting-owner"}),
        &[
            json!({"kind": "slice_blocked", "slice": 1, "block_id": block(1), "question": "Which store?"}),
            json!({"kind": "run_waiting", "blocked": [1], "asks": [], "reason": "a slice blocked"}),
            json!({"kind": "block_answered", "id": block(1), "slice": 1, "text": "B"}),
        ],
    );
    let v = live(&fx, &[]);
    assert_eq!(v["now"]["waitingOn"], "driver", "{v}");
    assert!(
        v["now"]["label"].as_str().unwrap().contains(&block(1)),
        "{v}"
    );
    assert_eq!(v["attention"], json!([]));
}

#[test]
fn live_says_what_a_stop_waits_for() {
    let fx = Fixture::new("live-now-stopping");
    write_run(
        &fx,
        json!({"phase": "checkpoint", "pid": me(), "stop_requested": true, "workers": [alive_worker()]}),
        &[
            json!({"kind": "gate_started", "n": 1, "attempt": 1, "cmd": "./gate.sh", "log": "gates/1-1.log"}),
        ],
    );
    assert_eq!(
        live(&fx, &[])["now"]["stopping"],
        json!({"on": "gate", "what": "checkpoint 1 gate, attempt 1"})
    );
    write_run(
        &fx,
        json!({"pid": me(), "stop_requested": true, "workers": [alive_worker()]}),
        &[json!({"kind": "run_started"})],
    );
    assert_eq!(
        live(&fx, &[])["now"]["stopping"],
        json!({"on": "worker", "what": format!("slice {:02} slice worker", 1)})
    );
}

#[test]
fn live_attention_answer_is_status_answer() {
    let fx = Fixture::new("live-attention-answer");
    write_run(
        &fx,
        json!({"phase": "waiting-owner", "branch": null}),
        &[
            json!({"kind": "ask_opened", "id": ask(1), "text": "no inner loop declared"}),
            json!({"kind": "slice_blocked", "slice": 1, "block_id": block(1), "question": "Which store?\nA or B"}),
        ],
    );
    log_entries(
        &fx,
        &[
            json!({"id": block(1), "kind": "block", "ts": "2026-11-07T12:00:00Z", "title": "Which store?", "body": "**Happened:** two stores"}),
        ],
    );

    let v = live(&fx, &[]);
    let out = factory(&fx, &["status", ID, "--json"]);
    assert_eq!(out.status.code(), Some(0));
    let status: Value = serde_json::from_slice(&out.stdout).unwrap();
    let questions = status["runs"][0]["questions"].as_array().unwrap();
    assert_eq!(questions.len(), 2, "{status}");
    for q in questions {
        let id = q["id"].as_str().unwrap();
        let a = by_id(&v, id);
        assert_eq!(a["title"], q["text"], "{a}");
        assert_eq!(a["answer"], q["answer"], "{a}");
        let kind = if id.starts_with("ASK-") {
            "ask"
        } else {
            "block"
        };
        assert_eq!(a["kind"], kind);
        assert_eq!(a["openedAt"], "2026-11-07T12:00:00Z");
    }
    assert_eq!(by_id(&v, &block(1))["body"], "**Happened:** two stores");
    assert_eq!(by_id(&v, &ask(1))["body"], Value::Null);
    assert_eq!(
        by_id(&v, &block(1))["reason"],
        json!({"source": "driver", "log": null, "excerpt": null})
    );
    assert_eq!(v["now"]["waitingOn"], "owner");
}

#[test]
fn a_red_gate_block_carries_the_gate_excerpt_and_its_log_body() {
    let fx = Fixture::new("live-attention-gate");
    let dir = write_run(
        &fx,
        json!({"phase": "waiting-owner"}),
        &[
            json!({"kind": "gate_run", "n": 1, "attempt": 3, "red": true, "minutes": 1.0, "log": "gates/1-3.log"}),
            json!({"kind": "checkpoint_blocked", "n": 1, "block_id": block(2), "question": "The gate stays red"}),
        ],
    );
    fs::create_dir_all(dir.join("gates")).unwrap();
    fs::write(
        dir.join("gates/1-3.log"),
        "step ok\nFAIL: tests/orders.rs expected 2 got 3\nstep ok\n",
    )
    .unwrap();
    let body = "**Happened:** three red gates on tests/orders.rs";
    log_entries(
        &fx,
        &[
            json!({"id": block(2), "kind": "block", "ts": "2026-11-07T12:00:00Z", "title": "red", "body": body}),
        ],
    );

    let v = live(&fx, &[]);
    let a = by_id(&v, &block(2));
    assert_eq!(a["kind"], "block");
    assert_eq!(a["title"], "The gate stays red");
    assert_eq!(a["body"], body);
    assert_eq!(a["reason"]["source"], "gate", "{a}");
    assert_eq!(a["reason"]["log"], "gates/1-3.log");
    let excerpt = a["reason"]["excerpt"].as_array().unwrap();
    assert!(
        excerpt
            .iter()
            .any(|l| l["hit"] == true && l["text"] == "FAIL: tests/orders.rs expected 2 got 3"),
        "{a}"
    );
}

#[test]
fn a_ci_block_carries_the_ci_excerpt_and_warnings_follow_the_questions() {
    let fx = Fixture::new("live-attention-ci");
    let dir = write_run(
        &fx,
        json!({"phase": "waiting-owner"}),
        &[
            json!({"kind": "pack_section_missing", "slice": 2, "section": 3}),
            json!({"kind": "ci_red", "check": "test", "round": 2, "head": "abc", "ts": "2026-11-01T00:00:00Z"}),
            json!({"kind": "closeout_blocked", "phase": "ci", "block_id": block(3), "question": "CI stays red"}),
            json!({"kind": "polish_findings_missing", "session_id": "s9"}),
        ],
    );
    fs::create_dir_all(dir.join("ci")).unwrap();
    fs::write(
        dir.join("ci/2.log"),
        "FAIL test: expected 2 got 3\n##[error]Process completed with exit code 1.\n",
    )
    .unwrap();

    let v = live(&fx, &[]);
    let kinds: Vec<&str> = v["attention"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["kind"].as_str().unwrap())
        .collect();
    assert_eq!(kinds, ["block", "warn", "warn"], "{v}");
    let a = by_id(&v, &block(3));
    assert_eq!(a["reason"]["source"], "ci", "{a}");
    assert_eq!(a["reason"]["log"], "ci/2.log");
    assert!(
        !a["reason"]["excerpt"].as_array().unwrap().is_empty(),
        "{a}"
    );
    let warns: Vec<&Value> = v["attention"].as_array().unwrap()[1..].iter().collect();
    for w in &warns {
        assert_eq!(w["answer"], Value::Null);
        assert!(!w["title"].as_str().unwrap().is_empty());
    }
    assert!(warns[0]["title"].as_str().unwrap().contains("02"), "{v}");
    assert!(warns[1]["title"].as_str().unwrap().contains("s9"), "{v}");
}

#[test]
fn a_park_entry_needs_a_park_with_nothing_after_it() {
    let fx = Fixture::new("live-attention-park");
    let park = json!({"kind": "run_waiting", "blocked": [], "asks": [], "reason": "the landing cannot go on: a.txt changed in the main tree"});
    write_run(
        &fx,
        json!({"phase": "waiting-owner"}),
        std::slice::from_ref(&park),
    );
    let v = live(&fx, &[]);
    let a = &v["attention"][0];
    assert_eq!(a["id"], "park-1");
    assert_eq!(a["kind"], "park");
    assert_eq!(a["title"], park["reason"]);
    assert_eq!(
        a["reason"],
        json!({"source": "land", "log": null, "excerpt": null})
    );
    assert_eq!(a["answer"], Value::Null);
    assert_eq!(v["now"]["waitingOn"], "owner");

    write_run(
        &fx,
        json!({"phase": "waiting-owner"}),
        &[
            park,
            json!({"kind": "run_resumed"}),
            json!({"kind": "run_parked", "reason": "budget spent", "weighted": 2, "budget": 1}),
        ],
    );
    let v = live(&fx, &[]);
    assert_eq!(v["attention"], json!([]));
    assert_eq!(v["now"]["waitingOn"], "budget");
}
