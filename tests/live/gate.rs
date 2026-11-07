//! `gate` and `ci`: the running or red gate and the pull request's checks.
use std::fs;

use serde_json::{Value, json};

use super::harness::Fixture;
use super::{append, live, write_run};

const FAILURE: &str = "FAIL: tests/orders.rs expected 2 got 3";

fn hits(excerpt: &Value) -> Vec<&str> {
    excerpt
        .as_array()
        .unwrap()
        .iter()
        .filter(|l| l["hit"] == true)
        .map(|l| l["text"].as_str().unwrap())
        .collect()
}

#[test]
fn live_shows_a_running_gate_tail() {
    let fx = Fixture::new("live-gate-tail");
    let dir = write_run(
        &fx,
        json!({}),
        &[
            json!({"kind": "gate_run", "n": 1, "attempt": 1, "red": true, "minutes": 0.5, "log": "gates/1-1.log"}),
            json!({"kind": "fix_empty", "n": 1, "attempt": 1, "session_id": "s1"}),
            json!({"kind": "gate_started", "n": 1, "attempt": 2, "cmd": "./gate.sh", "log": "gates/1-2.log"}),
        ],
    );
    fs::create_dir_all(dir.join("gates")).unwrap();
    let mut log = String::new();
    for i in 1..=200 {
        match i {
            50 => log.push_str(&format!("\x1b[31m{FAILURE}\x1b[0m\n")),
            190 => log.push_str(&format!("long {}\n", "y".repeat(500))),
            _ => log.push_str(&format!("step {i} \x1b[32mok\x1b[0m\r\n")),
        }
    }
    fs::write(dir.join("gates/1-2.log"), &log).unwrap();

    let g = live(&fx, &[])["gate"].clone();
    assert_eq!(g["n"], 1, "{g}");
    assert_eq!(g["attempt"], 2);
    assert_eq!(g["cmd"], "./gate.sh");
    assert_eq!(g["running"], true);
    assert!(g["startedAt"].as_str().unwrap().ends_with('Z'), "{g}");
    assert_eq!(g["red"], Value::Null);
    assert_eq!(g["minutes"], Value::Null);
    assert_eq!(g["cached"], false);
    assert_eq!(g["log"], "gates/1-2.log");
    assert_eq!(g["bytes"], log.len());
    assert_eq!(g["fixEmpty"], true);
    assert_eq!(g["excerpt"], Value::Null);
    let tail: Vec<&str> = g["tail"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l.as_str().unwrap())
        .collect();
    assert_eq!(tail.len(), 40);
    assert_eq!(tail[0], "step 161 ok");
    assert_eq!(tail[39], "step 200 ok");
    assert!(tail.iter().all(|l| l.chars().count() <= 300), "{tail:?}");
    assert_eq!(tail[29].chars().count(), 300);

    append(
        &dir,
        json!({"kind": "gate_run", "n": 1, "attempt": 2, "red": true, "minutes": 1.5, "log": "gates/1-2.log"}),
    );
    let g = live(&fx, &[])["gate"].clone();
    assert_eq!(g["running"], false);
    assert_eq!(g["red"], true);
    assert_eq!(g["minutes"], 1.5);
    assert_eq!(g["cmd"], "./gate.sh");
    assert_eq!(g["fixEmpty"], true);
    assert_eq!(hits(&g["excerpt"]), [FAILURE], "{g}");

    append(
        &dir,
        json!({"kind": "gate_cached", "n": 2, "attempt": 1, "red": false, "log": "gates/1-2.log"}),
    );
    let g = live(&fx, &[])["gate"].clone();
    assert_eq!(
        (g["n"].clone(), g["cached"].clone()),
        (json!(2), json!(true))
    );
    assert_eq!(g["cmd"], Value::Null);
    assert_eq!(g["fixEmpty"], false);
    assert_eq!(g["excerpt"], Value::Null);
}

#[test]
fn live_shows_the_red_ci_round_and_its_excerpt() {
    let fx = Fixture::new("live-ci-red");
    let dir = write_run(
        &fx,
        json!({}),
        &[
            json!({"kind": "ci_waiting", "checks": ["test", "lint"]}),
            json!({"kind": "ci_red", "check": "test", "round": 0, "head": "abc123"}),
        ],
    );
    fs::create_dir_all(dir.join("ci")).unwrap();
    fs::write(dir.join("ci/1.log"), "an older round\nFAIL old: gone\n").unwrap();
    fs::write(
        dir.join("ci/2.log"),
        "--- run 7 (`ci`), failed: `test`\nFAIL test: expected 2 got 3\n##[error]Process completed with exit code 1.\n",
    )
    .unwrap();

    let c = live(&fx, &[])["ci"].clone();
    assert_eq!(c["state"], "red", "{c}");
    assert_eq!(c["round"], 2);
    assert_eq!(c["head"], "abc123");
    assert_eq!(c["checks"], json!(["test", "lint"]));
    assert!(
        hits(&c["excerpt"]).contains(&"FAIL test: expected 2 got 3"),
        "{c}"
    );

    append(&dir, json!({"kind": "ci_waiting", "checks": ["test"]}));
    let c = live(&fx, &[])["ci"].clone();
    assert_eq!(c["state"], "waiting");
    assert_eq!(c["checks"], json!(["test"]));
    assert_eq!(c["excerpt"], Value::Null);
    assert_eq!(c["head"], "abc123");

    append(&dir, json!({"kind": "ci_green", "head": "def456"}));
    let c = live(&fx, &[])["ci"].clone();
    assert_eq!(
        (c["state"].clone(), c["head"].clone()),
        (json!("green"), json!("def456"))
    );
}

#[test]
fn live_ci_before_any_check_is_none() {
    let fx = Fixture::new("live-ci-none");
    write_run(&fx, json!({}), &[json!({"kind": "run_started"})]);
    let v = live(&fx, &[]);
    assert_eq!(
        v["ci"],
        json!({"state": "none", "round": null, "head": null, "checks": [], "excerpt": null})
    );
    assert_eq!(v["gate"], Value::Null);
}
