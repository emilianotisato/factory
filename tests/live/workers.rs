//! `workers[]`: what each worker is doing.
use std::fs;
use std::path::Path;

use serde_json::{Value, json};

use super::harness::Fixture;
use super::{live, run_dir, worker, write_run};

const BUILD: &str = "tests/fixture/recordings/claude-stream-build-mid-bash-2.1.316.jsonl";
const POLISH: &str = "tests/fixture/recordings/claude-stream-polish-skill-2.1.316.jsonl";
const MID_BASH: &str = "toolu_01DAy54snXrasn1qqKV6Jp8Y";
const TOKEN: &str = "ghp_FAKEt0kenN0tRealAbCdEfGh1234567890";

fn recording(rel: &str) -> String {
    fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(rel)).unwrap()
}

/// A worker whose pid is this test process, so `live` sees it alive.
fn alive(session: &str, role: &str) -> Value {
    let mut w = worker(session);
    w["role"] = json!(role);
    w["pid"] = json!(std::process::id());
    w
}

fn finished(session: &str, role: &str) -> Value {
    let mut w = worker(session);
    w["role"] = json!(role);
    w
}

fn stream(fx: &Fixture, session: &str, text: &str) {
    let dir = run_dir(fx).join("workers");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join(format!("{session}.jsonl")), text).unwrap();
}

fn lines(values: &[Value]) -> String {
    values.iter().map(|v| format!("{v}\n")).collect()
}

fn by_session<'a>(v: &'a Value, session: &str) -> &'a Value {
    v["workers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|w| w["sessionId"] == session)
        .unwrap()
}

fn tool_result(id: &str, content: &str) -> Value {
    json!({
        "type": "user",
        "message": {"role": "user", "content": [
            {"tool_use_id": id, "type": "tool_result", "content": content}
        ]},
        "parent_tool_use_id": null,
        "timestamp": "2026-11-07T12:00:09.000Z",
    })
}

fn assistant(content: Value) -> Value {
    json!({
        "type": "assistant",
        "message": {"role": "assistant", "content": content},
        "parent_tool_use_id": null,
        "timestamp": "2026-11-07T12:00:01.000Z",
    })
}

#[test]
fn live_worker_says_what_it_is_doing() {
    let fx = Fixture::new("live-workers-doing");
    write_run(
        &fx,
        json!({ "workers": [
            alive("build", "slice"),
            alive("polish", "polish"),
            alive("polish-early", "polish"),
        ] }),
        &[json!({"kind": "run_started"})],
    );
    stream(&fx, "build", &recording(BUILD));
    let polish = recording(POLISH);
    stream(&fx, "polish", &polish);
    let early: String = polish.lines().take(55).map(|l| format!("{l}\n")).collect();
    stream(&fx, "polish-early", &early);

    let v = live(&fx, &[]);
    let b = by_session(&v, "build");
    assert_eq!(b["alive"], true);
    assert_eq!(b["doing"]["tool"], "Bash");
    assert_eq!(b["doing"]["label"], "Bash");
    assert_eq!(b["doing"]["inSubagent"], false);
    assert!(b["doing"]["at"].is_string(), "{b}");
    assert_eq!(b["running"], json!({"tool": "Bash", "elapsedSeconds": 60}));
    let narration = b["narration"].as_str().unwrap();
    assert!(!narration.is_empty() && narration.chars().count() <= 160);
    assert_eq!(b["outcome"], Value::Null);
    assert_eq!(b["job"], Value::Null);

    let p = by_session(&v, "polish");
    assert_eq!(p["doing"]["tool"], "Bash");
    assert_eq!(p["doing"]["inSubagent"], true);

    let e = by_session(&v, "polish-early");
    assert_eq!(e["doing"]["tool"], "Skill");
    assert_eq!(e["doing"]["label"], "Skill code-review");
    assert_eq!(e["doing"]["inSubagent"], false);
}

#[test]
fn live_worker_running_clears_once_the_tool_returns() {
    let fx = Fixture::new("live-workers-returned");
    write_run(&fx, json!({ "workers": [alive("build", "slice")] }), &[]);
    let mut text = recording(BUILD);
    text.push_str(&lines(&[tool_result(MID_BASH, "exit=0 secs=75")]));
    stream(&fx, "build", &text);

    let b = live(&fx, &[])["workers"][0].clone();
    assert_eq!(b["running"], Value::Null, "{b}");
    assert_eq!(b["doing"]["tool"], "Bash");
}

#[test]
fn live_worker_label_never_carries_the_command() {
    let fx = Fixture::new("live-workers-secret");
    write_run(
        &fx,
        json!({ "workers": [alive("w", "slice"), finished("done", "slice")] }),
        &[],
    );
    let id = "toolu_secret";
    let cmd = format!("curl -H 'Authorization: token {TOKEN}' https://api.example?k=unshapedInput");
    let calls = [
        assistant(json!([{"type": "tool_use", "id": id, "name": "Bash",
            "input": {"command": cmd, "timeout": 600000}}])),
        json!({"type": "system", "subtype": "task_started", "tool_use_id": id,
            "description": cmd}),
        json!({"type": "tool_progress", "tool_use_id": format!("{id}-heartbeat-0"),
            "tool_name": "Bash", "parent_tool_use_id": id, "elapsed_time_seconds": 30}),
    ];
    stream(&fx, "w", &lines(&calls));
    let mut returned = calls.to_vec();
    returned.push(tool_result(id, &format!("echoed {TOKEN} unshapedOutput")));
    stream(&fx, "done", &lines(&returned));

    let out = super::factory(&fx, &["live", super::ID, "--json"]);
    assert_eq!(out.status.code(), Some(0));
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(
        !text.contains(TOKEN)
            && !text.contains("FAKEt0ken")
            && !text.contains("unshapedInput")
            && !text.contains("unshapedOutput"),
        "{text}"
    );
    let v: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(by_session(&v, "w")["doing"]["label"], "Bash");
    assert_eq!(
        by_session(&v, "w")["running"],
        json!({"tool": "Bash", "elapsedSeconds": 30})
    );
}

#[test]
fn live_skips_an_oversized_line() {
    let fx = Fixture::new("live-workers-oversized");
    write_run(&fx, json!({ "workers": [alive("w", "slice")] }), &[]);
    let huge = "h".repeat(70 * 1024);
    let text = lines(&[
        assistant(json!([{"type": "text", "text": "Reading the slice first."}])),
        assistant(json!([{"type": "text", "text": huge}])),
        assistant(json!([{"type": "tool_use", "id": "t1", "name": "Bash",
            "input": {"command": "ls", "description": "List the source tree"}}])),
    ]);
    stream(&fx, "w", &text);

    let w = live(&fx, &[])["workers"][0].clone();
    assert_eq!(w["narration"], "Reading the slice first.");
    assert_eq!(w["doing"]["label"], "List the source tree");
}

#[test]
fn live_worker_on_an_unknown_stream_shape_is_null_not_an_error() {
    let fx = Fixture::new("live-workers-unknown");
    write_run(&fx, json!({ "workers": [alive("w", "slice")] }), &[]);
    let mut text = lines(&[
        json!({"type": "mystery", "payload": {"content": [1, 2]}}),
        json!({"type": "assistant", "message": "not an object"}),
        json!({"type": "tool_progress", "elapsed_time_seconds": "soon"}),
        json!({"kind": "no type at all"}),
        json!([1, 2, 3]),
    ]);
    text.push_str(r#"{"type":"assistant","message":{"content":[{"type":"text","text":"half a li"#);
    stream(&fx, "w", &text);

    let w = live(&fx, &[])["workers"][0].clone();
    for k in ["job", "doing", "narration", "running", "outcome"] {
        assert_eq!(w[k], Value::Null, "{k}: {w}");
    }
}

#[test]
fn live_tells_a_ci_fix_from_a_gate_fix() {
    let fx = Fixture::new("live-workers-jobs");
    write_run(
        &fx,
        json!({ "workers": [
            finished("gate-fix", "fix"),
            finished("ci-fix", "fix"),
            finished("build", "slice"),
        ] }),
        &[
            json!({"kind": "fix_spawned", "n": 1, "attempt": 1, "session_id": "gate-fix", "pid": 0}),
            json!({"kind": "ci_fix_spawned", "round": 1, "attempt": 1, "session_id": "ci-fix", "pid": 0}),
        ],
    );
    let v = live(&fx, &[]);
    assert_eq!(by_session(&v, "gate-fix")["job"], "gateFix");
    assert_eq!(by_session(&v, "ci-fix")["job"], "ciFix");
    assert_eq!(by_session(&v, "build")["job"], Value::Null);
}

#[test]
fn live_worker_outcome_reads_the_status_below_the_first_line() {
    let fx = Fixture::new("live-workers-outcome");
    write_run(&fx, json!({ "workers": [finished("w", "slice")] }), &[]);
    let result = "Stopped at the migration.\n\nSTATUS: blocked\nINNER: 2 runs, 1 red, 3 min\nBLOCK: the index forbids soft-delete";
    let text = lines(&[
        assistant(json!([{"type": "text", "text": "Blocking."}])),
        json!({"type": "result", "subtype": "success", "is_error": false, "result": result}),
    ]);
    stream(&fx, "w", &text);

    let w = live(&fx, &[])["workers"][0].clone();
    assert_eq!(
        w["outcome"],
        json!({"status": "blocked", "block": "the index forbids soft-delete"})
    );
    assert_eq!(w["running"], Value::Null);
}
