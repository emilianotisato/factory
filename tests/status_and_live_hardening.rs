//! `status --json` and `factory live` hardening: bodies, symlink reads, CI waiting, cap/rev, lane labels.
#[path = "harness/mod.rs"]
mod harness;

use std::ffi::CString;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Output;
use std::time::{Duration, Instant};

use factory::events::EventKind;
use factory::live::{CAP, Ctx, Opts, render, snapshot};
use factory::run::Run;
use harness::Fixture;
use libc::{c_char, mkfifo};
use serde_json::{Value, json};

const BIN: &str = env!("CARGO_BIN_EXE_factory");
const FEATURE_DIR: &str = "docs/features/001-fixture";
const QUESTION: &str = "Which storage backs the fixture?";

fn block_key(n: u32) -> String {
    format!("BLOCK-{n}")
}

fn slice_label(n: u32) -> String {
    format!("{} {n:02}", "slice")
}

fn block_body() -> String {
    format!(
        "**Happened:** the stub worker on {} reached a fork the slice does not settle.\n\
**Options:** 1. the Recommended option; 2. the other one.\n\
**Recommended:** 1 — it is the one the fixture expects.",
        slice_label(1)
    )
}

fn cp_dir(n: u32) -> String {
    format!("cp-{n}")
}

fn factory(fx: &Fixture, args: &[&str]) -> Output {
    fx.command(BIN).args(args).output().unwrap()
}

fn factory_in(fx: &Fixture, cwd: &Path, args: &[&str]) -> Output {
    fx.command(BIN)
        .current_dir(cwd)
        .args(args)
        .output()
        .unwrap()
}

fn data_dir(fx: &Fixture) -> PathBuf {
    PathBuf::from(&fx.env()["FACTORY_DATA_DIR"])
}

fn status_json(fx: &Fixture, cwd: &Path, args: &[&str]) -> Value {
    let out = factory_in(fx, cwd, args);
    assert_eq!(out.status.code(), Some(0));
    serde_json::from_str(&String::from_utf8_lossy(&out.stdout)).unwrap()
}

fn append_event(dir: &Path, event: Value) {
    let path = dir.join("events.jsonl");
    let seq = fs::read_to_string(&path)
        .unwrap_or_default()
        .lines()
        .count()
        + 1;
    let mut e = event;
    e["seq"] = json!(seq);
    if e.get("ts").is_none() {
        e["ts"] = json!("2026-11-07T12:00:00Z");
    }
    let mut f = OpenOptions::new()
        .append(true)
        .create(true)
        .open(&path)
        .unwrap();
    writeln!(f, "{e}").unwrap();
}

fn write_run(fx: &Fixture, id: &str, phase: &str, events: &[Value]) -> PathBuf {
    let dir = data_dir(fx).join("runs").join(id);
    fs::create_dir_all(&dir).unwrap();
    let state = json!({
        "id": id,
        "repo": fx.path(),
        "feature": "001",
        "branch": "feat/001-fixture",
        "phase": phase,
        "pid": 0,
        "started_at": "2026-11-07T12:00:00Z",
        "updated_at": "2026-11-07T12:00:00Z",
        "budget_weighted_tokens": 1000,
        "stop_requested": false,
        "pr_url": "https://example.invalid/pr/1",
    });
    fs::write(dir.join("state.json"), state.to_string()).unwrap();
    fs::write(dir.join("events.jsonl"), "").unwrap();
    for e in events {
        append_event(&dir, e.clone());
    }
    dir
}

fn live_json(fx: &Fixture, id: &str, data_dir: Option<&Path>) -> Value {
    let mut base = fx.command(BIN);
    let mut cmd = base.arg("live");
    cmd = cmd.arg(id).arg("--json");
    if let Some(d) = data_dir {
        cmd = cmd.env("FACTORY_DATA_DIR", d);
    }
    let out = cmd.output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    serde_json::from_str(&String::from_utf8_lossy(&out.stdout)).unwrap()
}

#[test]
fn status_json_and_live_fixes_body_symlink_ci_waiting_cap_rev_and_lane_labels() {
    let fx = Fixture::with_index("status-live-hardening", &[("checkpoint_every", "10")]);
    fx.write_plan(&[
        json!({"outcome": "blocked", "question": QUESTION}),
        json!({"outcome": "done"}),
        json!({"outcome": "done"}),
        json!({"outcome": "done"}),
        json!({"outcome": "checkpoint"}),
    ]);
    assert_eq!(factory(&fx, &["run", "001"]).status.code(), Some(0));
    let run_dir = fs::read_dir(data_dir(&fx).join("runs"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .next()
        .unwrap();
    let parked_id = run_dir.file_name().unwrap().to_str().unwrap().to_string();
    let deadline = Instant::now() + Duration::from_secs(60);
    while Instant::now() < deadline {
        let st =
            serde_json::from_str::<Value>(&fs::read_to_string(run_dir.join("state.json")).unwrap())
                .unwrap();
        if st["phase"] == "waiting-owner" {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let log_path = Run::open(&data_dir(&fx), &parked_id)
        .unwrap()
        .state()
        .repo
        .join(FEATURE_DIR)
        .join("log.jsonl");
    let log_deadline = Instant::now() + Duration::from_secs(30);
    while Instant::now() < log_deadline {
        if fs::read_to_string(&log_path)
            .ok()
            .is_some_and(|t| t.contains("**Happened:**"))
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(
        fs::read_to_string(&log_path)
            .ok()
            .is_some_and(|t| t.contains("**Happened:**")),
        "block log never landed"
    );
    let v = status_json(&fx, fx.path(), &["status", &parked_id, "--json"]);
    let want_body = block_body();
    let first_block = block_key(1);
    let slice_block = v["runs"][0]["questions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|q| q["id"].as_str() == Some(first_block.as_str()))
        .unwrap();
    assert_eq!(slice_block["body"].as_str().unwrap(), want_body);

    let close_block = block_key(9);
    let mut close_run = Run::create(&data_dir(&fx), fx.path(), "001", None, 1).unwrap();
    close_run
        .emit(EventKind::CloseoutBlocked {
            phase: "ci".into(),
            block_id: close_block.clone(),
            question: "CI stays red".into(),
        })
        .unwrap();
    let close_id = close_run.id().to_string();
    let v = status_json(&fx, fx.path(), &["status", &close_id, "--json"]);
    let closeout = v["runs"][0]["questions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|q| q["id"].as_str() == Some(close_block.as_str()))
        .unwrap();
    assert_eq!(closeout["phase"], "ci");

    // 2. FIFO symlink at prd.md does not hang; title is null
    let fifo_dir = std::env::temp_dir().join(format!("factory-fifo-{}", std::process::id()));
    let _ = fs::remove_dir_all(&fifo_dir);
    fs::create_dir_all(&fifo_dir).unwrap();
    let fifo = fifo_dir.join("prd.fifo");
    let path = CString::new(fifo.to_string_lossy().into_owned()).unwrap();
    assert_eq!(unsafe { mkfifo(path.as_ptr() as *const c_char, 0o600) }, 0);
    let feat_x = fx.path().join("docs/features/001-x");
    fs::create_dir_all(&feat_x).unwrap();
    let prd = feat_x.join("prd.md");
    let _ = fs::remove_file(&prd);
    std::os::unix::fs::symlink(&fifo, &prd).unwrap();
    let prd_fixture = fx.path().join(FEATURE_DIR).join("prd.md");
    let bak = prd_fixture.with_extension("md.bak");
    fs::rename(&prd_fixture, &bak).unwrap();
    std::os::unix::fs::symlink(&fifo, &prd_fixture).unwrap();
    let mut run = Run::create(&data_dir(&fx), fx.path(), "001", None, 1).unwrap();
    run.emit(EventKind::SliceBlocked {
        slice: 1,
        block_id: Some(block_key(1)),
        question: Some("q".into()),
    })
    .unwrap();
    let id = run.id().to_string();
    let start = Instant::now();
    let out = factory_in(&fx, fx.path(), &["status", &id, "--json"]);
    assert!(start.elapsed() < Duration::from_secs(2));
    assert_eq!(out.status.code(), Some(0));
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(v["runs"][0]["title"].is_null());
    let _ = fs::remove_file(&prd_fixture);
    fs::rename(&bak, &prd_fixture).unwrap();

    // 3. closeout CiRed → CiFixCommitted → waiting, no stale excerpt
    let id = "001-20261127-live";
    write_run(
        &fx,
        id,
        "closeout",
        &[
            json!({"kind": "ci_red", "check": "test", "round": 0, "head": "abc"}),
            json!({"kind": "ci_fix_committed", "round": 1, "sha": "def", "files": []}),
        ],
    );
    fs::create_dir_all(data_dir(&fx).join("runs").join(id).join("ci")).unwrap();
    fs::write(
        data_dir(&fx).join("runs").join(id).join("ci/1.log"),
        "FAIL stale\n",
    )
    .unwrap();
    let c = live_json(&fx, id, None)["ci"].clone();
    assert_eq!(c["state"], "waiting");
    assert_eq!(c["checks"], json!([]));
    assert_eq!(c["excerpt"], Value::Null);

    // 4. `--since` cap keeps oldest timeline; post-cap `rev` stable across truncated evidence
    let cap_id = "001-20261127-cap";
    let long = "z".repeat(900);
    let dir = write_run(
        &fx,
        cap_id,
        "building",
        &[json!({"kind": "slice_done", "slice": 1})],
    );
    for _ in 0..600 {
        append_event(&dir, json!({"kind": "judge_noted", "text": long.clone()}));
    }
    let ctx = Ctx::read(&dir).unwrap();
    let opts = Opts {
        since: Some(0),
        evidence_limit: 50,
    };
    let v1 = snapshot(&ctx, &opts);
    assert!(render(&v1).len() <= CAP);
    assert_eq!(v1["timeline"][0]["seq"], 1);
    let rev1 = v1["rev"].as_str().unwrap().to_string();
    let ev = dir.join("evidence").join(cp_dir(1));
    fs::create_dir_all(&ev).unwrap();
    for i in 0..80 {
        fs::write(ev.join(format!("big-{i}.txt")), "q".repeat(12_000)).unwrap();
    }
    let ctx2 = Ctx::read(&dir).unwrap();
    let v2 = snapshot(&ctx2, &opts);
    assert!(render(&v2).len() <= CAP);
    assert_eq!(v2["rev"].as_str().unwrap(), rev1);

    // 5. canonical lane root strips worker tool paths behind symlinks
    let link_fx = Fixture::with_index("status-lane-link", &[]);
    let real_data = link_fx.data_dir();
    let link_parent =
        std::env::temp_dir().join(format!("factory-data-link-{}", std::process::id()));
    let _ = fs::remove_dir_all(&link_parent);
    fs::create_dir_all(&link_parent).unwrap();
    let data_link = link_parent.join("data");
    std::os::unix::fs::symlink(&real_data, &data_link).unwrap();
    let lane_id = "001-20261127-lane";
    write_run(
        &link_fx,
        lane_id,
        "building",
        &[json!({"kind": "run_started"})],
    );
    let run_dir = data_dir(&link_fx).join("runs").join(lane_id);
    let workers = run_dir.join("workers");
    fs::create_dir_all(&workers).unwrap();
    let lane_dir = factory::lanes::dir(&data_dir(&link_fx), link_fx.path(), 1);
    let tool_file = lane_dir.join("src/foo.rs");
    fs::create_dir_all(tool_file.parent().unwrap()).unwrap();
    fs::write(&tool_file, "// x\n").unwrap();
    let canon = fs::canonicalize(&tool_file).unwrap();
    let stream = format!(
        r#"{{"type":"assistant","message":{{"role":"assistant","content":[{{"type":"tool_use","id":"t1","name":"Read","input":{{"file_path":"{}"}}}}]}}}}
{{"type":"user","message":{{"role":"user","content":[{{"type":"tool_result","tool_use_id":"t1","content":"ok"}}]}}}}
"#,
        canon.display()
    );
    fs::write(workers.join("build.jsonl"), stream).unwrap();
    let state_path = run_dir.join("state.json");
    let mut state: Value = serde_json::from_str(&fs::read_to_string(&state_path).unwrap()).unwrap();
    state["workers"] = json!([{
        "slice": 1,
        "lane": 1,
        "role": "slice",
        "session_id": "build",
        "pid": std::process::id(),
        "row_id": 1,
        "spawned_at": "2026-11-07T12:00:00Z",
        "outcome": null,
    }]);
    fs::write(&state_path, state.to_string()).unwrap();
    let w = &live_json(&link_fx, lane_id, Some(&data_link))["workers"][0];
    assert_eq!(w["doing"]["label"], "Read src/foo.rs");
}
