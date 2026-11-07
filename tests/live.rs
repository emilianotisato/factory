//! `factory live <id> --json` driven through the binary over run dirs written by hand.
#[path = "harness/mod.rs"]
mod harness;

#[path = "live/evidence.rs"]
mod evidence;
#[path = "live/gate.rs"]
mod gate;
#[path = "live/now.rs"]
mod now;
#[path = "live/redact.rs"]
mod redact;
#[path = "live/timeline.rs"]
mod timeline;
#[path = "live/workers.rs"]
mod workers;

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Output, Stdio};
use std::time::{Duration, Instant, SystemTime};

use harness::{FEATURE_DIR, Fixture};
use serde_json::{Value, json};

const BIN: &str = env!("CARGO_BIN_EXE_factory");
const ID: &str = "001-20261107-aaaa";
const STARTED: &str = "2026-11-07T12:00:00Z";

const ENVELOPE: [&str; 19] = [
    "schema",
    "factory",
    "id",
    "feature",
    "phase",
    "repo",
    "branch",
    "seq",
    "rev",
    "at",
    "now",
    "attention",
    "workers",
    "gate",
    "ci",
    "timeline",
    "evidence",
    "decisions",
    "truncated",
];

pub fn factory(fx: &Fixture, args: &[&str]) -> Output {
    let out = fx.command(BIN).args(args).output().unwrap();
    eprintln!(
        "$ factory {}\nexit={:?}\n{}",
        args.join(" "),
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
    out
}

/// The one line `live` printed, parsed; the call must exit 0.
pub fn live(fx: &Fixture, extra: &[&str]) -> Value {
    let mut args = vec!["live", ID, "--json"];
    args.extend_from_slice(extra);
    let out = factory(fx, &args);
    assert_eq!(out.status.code(), Some(0));
    let text = String::from_utf8(out.stdout).unwrap();
    assert_eq!(text.lines().count(), 1, "{text}");
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{e}: {text}"))
}

pub fn run_dir(fx: &Fixture) -> PathBuf {
    fx.data_dir().join("runs").join(ID)
}

/// A `state.json` from `overrides` over a bare building run of the fixture's repo, and one `events.jsonl` line per event.
pub fn write_run(fx: &Fixture, overrides: Value, events: &[Value]) -> PathBuf {
    let dir = run_dir(fx);
    fs::create_dir_all(&dir).unwrap();
    let mut state = json!({
        "id": ID,
        "repo": fx.path(),
        "feature": &ID[..3],
        "branch": "feat/001-fixture",
        "phase": "building",
        "pid": 0,
        "started_at": STARTED,
        "updated_at": STARTED,
        "budget_weighted_tokens": 1000,
        "stop_requested": false,
    });
    for (k, v) in overrides.as_object().unwrap() {
        state[k] = v.clone();
    }
    fs::write(dir.join("state.json"), state.to_string()).unwrap();
    fs::write(dir.join("events.jsonl"), "").unwrap();
    for e in events {
        append(&dir, e.clone());
    }
    dir
}

/// Appends `event` with the next `seq`.
pub fn append(dir: &Path, mut event: Value) {
    let path = dir.join("events.jsonl");
    let seq = fs::read_to_string(&path)
        .unwrap_or_default()
        .lines()
        .count()
        + 1;
    event["seq"] = json!(seq);
    if event.get("ts").is_none() {
        event["ts"] = json!(STARTED);
    }
    let mut f = OpenOptions::new()
        .append(true)
        .create(true)
        .open(&path)
        .unwrap();
    writeln!(f, "{event}").unwrap();
}

pub fn worker(session: &str) -> Value {
    json!({
        "slice": 1,
        "lane": 1,
        "role": "slice",
        "session_id": session,
        "pid": 0,
        "row_id": 1,
        "spawned_at": STARTED,
        "outcome": null,
    })
}

fn seqs(v: &Value) -> Vec<u64> {
    v["timeline"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["seq"].as_u64().unwrap())
        .collect()
}

/// Every path under `root` with its mtime, the dirs included.
fn tree(root: &Path) -> Vec<(PathBuf, SystemTime)> {
    let mut out = vec![(
        root.to_path_buf(),
        fs::metadata(root).unwrap().modified().unwrap(),
    )];
    let mut entries: Vec<_> = fs::read_dir(root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            out.extend(tree(&p));
        } else {
            out.push((p.clone(), fs::metadata(&p).unwrap().modified().unwrap()));
        }
    }
    out
}

#[test]
fn live_answers_one_line_of_schema_1_json() {
    let fx = Fixture::new("live-envelope");
    let dir = write_run(
        &fx,
        json!({ "workers": [worker("s1")] }),
        &[
            json!({"kind": "run_started"}),
            json!({"kind": "slice_spawned", "slice": 1, "session_id": "s1", "pid": 0}),
        ],
    );
    fs::create_dir_all(dir.join("workers")).unwrap();
    fs::write(dir.join("workers/s1.jsonl"), "{}\n").unwrap();
    let log = fx.path().join(FEATURE_DIR).join("log.jsonl");
    let decision = |n: u32| {
        format!(r#"{{"id":"AD-{n}","kind":"decision","ts":"{STARTED}","title":"t","body":"b"}}"#)
    };
    let note = r#"{"kind":"note","ts":"2026-11-07T12:00:00Z","title":"n","body":"b"}"#;
    fs::write(&log, format!("{}\n{note}\n{}\n", decision(1), decision(2))).unwrap();

    let v = live(&fx, &[]);
    let keys: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
    let mut want = ENVELOPE.to_vec();
    want.sort();
    assert_eq!(keys, want, "{v}");
    assert_eq!(v["schema"], 1);
    assert_eq!(v["factory"], env!("CARGO_PKG_VERSION"));
    assert_eq!(v["id"], ID);
    assert_eq!(v["feature"], "001");
    assert_eq!(v["phase"], "building");
    assert_eq!(v["repo"], fx.path().display().to_string());
    assert_eq!(v["branch"], "feat/001-fixture");
    assert_eq!(v["seq"], 2);
    assert!(v["at"].as_str().unwrap().ends_with('Z'), "{v}");
    assert_eq!(v["decisions"], json!({"count": 2}));
    assert_eq!(v["truncated"], json!([]));
    assert_eq!(seqs(&v), [1, 2]);
    let w = &v["workers"][0];
    assert_eq!(w["sessionId"], "s1");
    assert_eq!(w["role"], "slice");
    assert_eq!(w["slice"], 1);
    assert_eq!(w["lane"], 1);
    assert_eq!(w["spawnedAt"], STARTED);
    assert_eq!(w["alive"], false);
    assert!(w["lastWriteAt"].as_str().unwrap().ends_with('Z'), "{w}");
}

#[test]
fn live_writes_nothing() {
    let fx = Fixture::new("live-read-only");
    let dir = write_run(
        &fx,
        json!({ "workers": [worker("s1")] }),
        &[json!({"kind": "run_started"})],
    );
    fs::create_dir_all(dir.join("workers")).unwrap();
    fs::write(dir.join("workers/s1.jsonl"), "{}\n").unwrap();
    let data = fx.data_dir();
    drop(factory::db::Db::open(&factory::run::Run::db_path(&data)).unwrap());
    assert!(data.join("factory.db").is_file());
    let before = tree(&data);
    std::thread::sleep(Duration::from_millis(20));

    live(&fx, &[]);
    live(&fx, &["--wait", "1"]);
    assert_eq!(tree(&data), before);
}

#[test]
fn live_on_an_old_layout_run_is_empty_not_an_error() {
    let fx = Fixture::new("live-old-layout");
    let dir = write_run(&fx, json!({}), &[]);
    fs::remove_file(dir.join("events.jsonl")).unwrap();

    let v = live(&fx, &[]);
    assert_eq!(v["seq"], 0);
    assert_eq!(v["timeline"], json!([]));
    assert_eq!(v["workers"], json!([]));
    assert_eq!(v["attention"], json!([]));
    assert_eq!(v["decisions"], json!({"count": 0}));
    for k in ["now", "gate", "ci"] {
        assert!(v[k].is_null(), "{k}: {v}");
    }
    assert_eq!(v["evidence"]["checkpoints"], json!([]));
}

#[test]
fn live_unknown_id_exits_1() {
    let fx = Fixture::new("live-unknown");
    let out = factory(&fx, &["live", "001-20261107-zzzz", "--json"]);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(
        String::from_utf8_lossy(&out.stderr),
        "no run 001-20261107-zzzz\n"
    );
    assert!(out.stdout.is_empty());
    assert!(!fx.data_dir().join("runs").exists());
}

#[test]
fn live_refuses_a_flag_out_of_range_and_a_call_without_json() {
    let fx = Fixture::new("live-flags");
    write_run(&fx, json!({}), &[]);
    for args in [
        vec!["live", ID, "--json", "--wait", "26"],
        vec!["live", ID, "--json", "--evidence-limit", "301"],
        vec!["live", ID],
    ] {
        assert_eq!(factory(&fx, &args).status.code(), Some(2), "{args:?}");
    }
}

#[test]
fn live_timeline_tones_follow_the_table() {
    let fx = Fixture::new("live-tones");
    let gate = |red| json!({"kind": "gate_run", "n": 1, "attempt": 2, "red": red, "minutes": 1.5, "log": "gates/1-2.log"});
    write_run(
        &fx,
        json!({}),
        &[
            json!({"kind": "ask_opened", "id": "q7", "text": "Which store?"}),
            gate(true),
            gate(false),
            json!({"kind": "ci_waiting", "checks": ["test", "lint"]}),
            json!({"kind": "run_started"}),
            json!({"kind": "a_kind_from_a_newer_binary", "n": 4}),
            json!({"kind": "landing_conflicted", "slice": 3, "paths": ["a", "b"]}),
        ],
    );

    let v = live(&fx, &[]);
    let t = v["timeline"].as_array().unwrap();
    let tones: Vec<&str> = t.iter().map(|e| e["tone"].as_str().unwrap()).collect();
    assert_eq!(
        tones,
        ["owner", "red", "green", "noise", "info", "info", "red"]
    );
    assert_eq!(t[0]["label"], "q7 asks: Which store?");
    assert_eq!(t[0]["data"], json!({"id": "q7", "text": "Which store?"}));
    assert_eq!(t[1]["kind"], "gate_run");
    assert_eq!(t[1]["label"], "gate 1 red, attempt 2");
    assert_eq!(t[1]["ts"], STARTED);
    assert_eq!(t[3]["data"], json!({"checks": 2}));
    assert_eq!(t[5]["kind"], "unknown");
    assert_eq!(t[5]["label"], "an event from a newer factory");
    assert_eq!(t[5]["data"], json!({}));
    assert_eq!(t[6]["label"], "slice 03 conflicted in 2 files");
    assert_eq!(t[6]["data"], json!({"slice": 3, "paths": 2}));
}

#[test]
fn live_since_returns_only_newer_events() {
    let fx = Fixture::new("live-since");
    let started: Vec<Value> = (0..6).map(|_| json!({"kind": "run_resumed"})).collect();
    let dir = write_run(&fx, json!({}), &started);
    assert_eq!(seqs(&live(&fx, &["--since", "3"])), [4, 5, 6]);
    assert_eq!(seqs(&live(&fx, &["--since", "6"])), Vec::<u64>::new());

    for _ in 0..144 {
        append(&dir, json!({"kind": "run_resumed"}));
    }
    let v = live(&fx, &[]);
    assert_eq!(seqs(&v), (51..=150).collect::<Vec<u64>>());
    assert_eq!(v["seq"], 150);
}

#[test]
fn live_wait_returns_when_something_visible_changes() {
    let fx = Fixture::new("live-wait-change");
    let dir = write_run(&fx, json!({}), &[json!({"kind": "run_started"})]);
    let start = Instant::now();
    let child = fx
        .command(BIN)
        .args(["live", ID, "--json", "--wait", "5"])
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    std::thread::sleep(Duration::from_secs(1));
    append(&dir, json!({"kind": "slice_done", "slice": 1}));
    let out = child.wait_with_output().unwrap();
    let took = start.elapsed();
    assert_eq!(out.status.code(), Some(0));
    assert!(took < Duration::from_secs(2), "{took:?}");
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["seq"], 2);
}

#[test]
fn live_wait_times_out_with_exit_0() {
    let fx = Fixture::new("live-wait-timeout");
    write_run(&fx, json!({}), &[json!({"kind": "run_started"})]);
    let start = Instant::now();
    let v = live(&fx, &["--wait", "1"]);
    assert!(start.elapsed() >= Duration::from_secs(1));
    assert_eq!(v["seq"], 1);
}

#[test]
fn live_rev_ignores_a_growing_stream() {
    let fx = Fixture::new("live-rev");
    let dir = write_run(
        &fx,
        json!({ "workers": [worker("s1")] }),
        &[json!({"kind": "run_started"})],
    );
    fs::create_dir_all(dir.join("workers")).unwrap();
    let stream = dir.join("workers/s1.jsonl");
    fs::write(&stream, "{}\n").unwrap();
    let first = live(&fx, &[]);

    let mut f = OpenOptions::new().append(true).open(&stream).unwrap();
    writeln!(f, "{{\"type\":\"assistant\"}}").unwrap();
    f.set_modified(SystemTime::now() + Duration::from_secs(3600))
        .unwrap();
    drop(f);
    let grown = live(&fx, &[]);
    assert_ne!(
        first["workers"][0]["lastWriteAt"],
        grown["workers"][0]["lastWriteAt"]
    );
    assert_eq!(first["rev"], grown["rev"]);

    append(&dir, json!({"kind": "slice_done", "slice": 1}));
    assert_ne!(live(&fx, &[])["rev"], first["rev"]);
}

#[test]
fn live_caps_output_and_names_what_it_cut() {
    let fx = Fixture::new("live-cap");
    let error = "x".repeat(8000);
    let events: Vec<Value> = (0..120)
        .map(|_| json!({"kind": "run_failed", "error": error}))
        .collect();
    write_run(&fx, json!({}), &events);

    let out = factory(&fx, &["live", ID, "--json"]);
    assert_eq!(out.status.code(), Some(0));
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.trim_end().len() <= 512 * 1024, "{}", text.len());
    let v: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["truncated"], json!(["timeline"]));
    let kept = seqs(&v);
    assert!(kept.len() < 100, "{}", kept.len());
    assert_eq!(kept.last(), Some(&120), "the newest events stay");
}
