//! What the snapshot hides before it prints.
use std::fs;
use std::path::Path;

use serde_json::{Value, json};

use super::harness::{FEATURE_DIR, Fixture};
use super::{BIN, ID, worker, write_run};

const MARKER: &str = "[redacted]";
const SHA: &str = "0123456789abcdef0123456789abcdef01234567";

/// Each line plants one token shape; the second field is the value that must not print.
const PLANTED: [(&str, &str); 13] = [
    (
        "key sk-ant-api03-PlantedAnthropicKey0123",
        "PlantedAnthropicKey0123",
    ),
    (
        "push ghp_PlantedGithubToken0123456789abcdef",
        "PlantedGithubToken0123456789abcdef",
    ),
    (
        "pat github_pat_11PLANTED0_PlantedFineGrainedPat",
        "PlantedFineGrainedPat",
    ),
    ("aws AKIAPLANTED0AWS0KEY1", "PLANTED0AWS0KEY1"),
    (
        "slack xoxb-1111-2222-PlantedSlackToken",
        "PlantedSlackToken",
    ),
    (
        "jwt eyJhbGciOiJIUzI1NiJ9.eyJwbGFudGVkIjoxfQ.PlantedJwtSignature_x-y",
        "eyJwbGFudGVkIjoxfQ",
    ),
    (
        "Authorization: Bearer PlantedBearerValue123",
        "PlantedBearerValue123",
    ),
    (
        "export DB_PASSWORD=PlantedEnvPassword",
        "PlantedEnvPassword",
    ),
    ("with api_key=PlantedLowerApiKey set", "PlantedLowerApiKey"),
    (
        r#"body {"token": "PlantedJsonToken", "ok": false}"#,
        "PlantedJsonToken",
    ),
    ("X-API-Key: PlantedApiHeader", "PlantedApiHeader"),
    (
        "hook https://hooks.example/services/PlantedUrlPathToken0123456789abcdefABCDEFGH",
        "PlantedUrlPathToken0123456789abcdefABCDEFGH",
    ),
    ("read {home}/planted/secret.txt", "{home}/"),
];

fn home(fx: &Fixture) -> String {
    fx.path().parent().unwrap().to_str().unwrap().to_string()
}

fn planted(fx: &Fixture) -> Vec<(String, String)> {
    let h = home(fx);
    PLANTED
        .iter()
        .map(|(line, secret)| (line.replace("{home}", &h), secret.replace("{home}", &h)))
        .collect()
}

/// `live --json` with `HOME` set to the fixture's base, so the repo sits under it.
fn live_at_home(fx: &Fixture) -> (String, Value) {
    let out = fx
        .command(BIN)
        .env("HOME", home(fx))
        .args(["live", ID, "--json"])
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = String::from_utf8(out.stdout).unwrap();
    let v = serde_json::from_str(&text).unwrap_or_else(|e| panic!("{e}: {text}"));
    (text, v)
}

fn stream(dir: &Path, session: &str, text: &str) {
    let line = json!({
        "type": "assistant",
        "message": {"role": "assistant", "content": [{"type": "text", "text": text}]},
        "parent_tool_use_id": null,
        "timestamp": "2026-11-07T12:00:01.000Z",
    });
    fs::create_dir_all(dir.join("workers")).unwrap();
    fs::write(
        dir.join("workers").join(format!("{session}.jsonl")),
        format!("{line}\n"),
    )
    .unwrap();
}

#[test]
fn live_redacts_every_string() {
    let fx = Fixture::new("live-redact-all");
    let lines = planted(&fx);
    let text: Vec<&str> = lines.iter().map(|(l, _)| l.as_str()).collect();
    let all = text.join("\n");
    let failures: String = text.iter().map(|l| format!("FAIL: {l}\n")).collect();

    let workers: Vec<Value> = (0..lines.len())
        .map(|i| {
            let mut w = worker(&format!("w{i}"));
            w["slice"] = json!(i + 1);
            w
        })
        .collect();
    let block = format!("BLOCK-{}", 1);
    let dir = write_run(
        &fx,
        json!({"phase": "waiting-owner", "workers": workers}),
        &[
            json!({"kind": "run_failed", "error": all}),
            json!({"kind": "gate_run", "n": 1, "attempt": 1, "red": true, "minutes": 1.0, "log": "gates/1-1.log"}),
            json!({"kind": "ci_red", "check": "test", "round": 0, "head": SHA}),
            json!({"kind": "slice_blocked", "slice": 1, "block_id": block, "question": "Which store?"}),
        ],
    );
    fs::create_dir_all(dir.join("gates")).unwrap();
    fs::write(dir.join("gates/1-1.log"), &failures).unwrap();
    fs::create_dir_all(dir.join("ci")).unwrap();
    fs::write(dir.join("ci/1.log"), &failures).unwrap();
    let entry = json!({"id": block, "kind": "block", "ts": "2026-11-07T12:00:00Z", "title": "Which store?", "body": all});
    fs::write(
        fx.path().join(FEATURE_DIR).join("log.jsonl"),
        format!("{entry}\n"),
    )
    .unwrap();
    for (i, l) in text.iter().enumerate() {
        stream(&dir, &format!("w{i}"), l);
    }

    let (out, mut v) = live_at_home(&fx);
    for (_, secret) in &lines[..lines.len() - 1] {
        assert!(!out.contains(secret.as_str()), "{secret} printed: {out}");
    }
    assert_eq!(v["repo"], fx.path().to_str().unwrap());

    let failed = v["timeline"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["kind"] == "run_failed")
        .unwrap()["data"]["error"]
        .clone();
    let body = v["attention"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["id"] == block.as_str())
        .unwrap_or_else(|| panic!("no block: {}", v["attention"]))["body"]
        .clone();
    let narrations: Vec<Value> = v["workers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|w| w["narration"].clone())
        .collect();
    for (section, x) in [
        ("run_failed", failed),
        ("gate", v["gate"]["excerpt"].clone()),
        ("ci", v["ci"]["excerpt"].clone()),
        ("block body", body),
        ("narration", json!(narrations)),
    ] {
        let s = x.to_string();
        assert_eq!(
            s.matches(MARKER).count(),
            PLANTED.len() - 1,
            "{section}: {s}"
        );
        assert!(s.contains("~/planted/secret.txt"), "{section}: {s}");
    }

    v["repo"] = Value::Null;
    assert!(!v.to_string().contains(&home(&fx)), "{v}");
}

#[test]
fn live_keeps_a_git_sha() {
    let fx = Fixture::new("live-redact-sha");
    let kept = format!("https://github.com/o/r/commit/{SHA}");
    let long = format!("https://github.com/o/r/commit/{SHA}9");
    write_run(
        &fx,
        json!({}),
        &[
            json!({"kind": "ci_green", "head": SHA}),
            json!({"kind": "run_failed", "error": format!("see {kept} and {long}")}),
        ],
    );

    let (_, v) = live_at_home(&fx);
    assert_eq!(v["ci"]["head"], SHA, "{}", v["ci"]);
    let green = v["timeline"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["kind"] == "ci_green")
        .unwrap();
    assert_eq!(green["data"]["head"], SHA, "{green}");
    let failed = v["timeline"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["kind"] == "run_failed")
        .unwrap();
    assert_eq!(
        failed["data"]["error"],
        format!("see {kept} and https://github.com/o/r/commit/{MARKER}"),
    );
}
