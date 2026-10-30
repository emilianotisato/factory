#[path = "harness/mod.rs"]
mod harness;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;
use std::time::{Duration, Instant};

use factory::run::pid_alive;
use harness::Fixture;
use serde_json::{Value, json};

const BIN: &str = env!("CARGO_BIN_EXE_factory");
const QUESTION: &str = "Which storage backs the fixture?";
const ANSWER: &str = "Pick the Recommended option";
const PR_URL: &str = "https://example.invalid/pr/1";
const BLOCK_LINE: &str =
    "BLOCK-1 slice 01: Which storage backs the fixture? (answer: factory answer BLOCK-1 \"...\")";

fn factory(fx: &Fixture, args: &[&str]) -> Output {
    factory_in(fx, fx.path(), args)
}

fn factory_in(fx: &Fixture, cwd: &Path, args: &[&str]) -> Output {
    let out = fx
        .command(BIN)
        .current_dir(cwd)
        .args(args)
        .output()
        .unwrap();
    // Echoed so a failing test's capture shows every command and what it said.
    eprintln!(
        "$ factory {}\n{}{}exit={}",
        args.join(" "),
        stdout(&out),
        String::from_utf8_lossy(&out.stderr),
        out.status
            .code()
            .map_or("signal".to_string(), |c| c.to_string())
    );
    out
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn data_dir(fx: &Fixture) -> PathBuf {
    PathBuf::from(&fx.env()["FACTORY_DATA_DIR"])
}

fn run_dirs(fx: &Fixture) -> Vec<PathBuf> {
    let Ok(rd) = fs::read_dir(data_dir(fx).join("runs")) else {
        return Vec::new();
    };
    let mut v: Vec<PathBuf> = rd.map(|e| e.unwrap().path()).collect();
    v.sort();
    v
}

fn only_run_dir(fx: &Fixture) -> PathBuf {
    let dirs = run_dirs(fx);
    assert_eq!(dirs.len(), 1, "{dirs:?}");
    dirs[0].clone()
}

fn run_id(dir: &Path) -> String {
    dir.file_name().unwrap().to_str().unwrap().to_string()
}

fn state(dir: &Path) -> Value {
    serde_json::from_str(&fs::read_to_string(dir.join("state.json")).unwrap()).unwrap()
}

fn events(dir: &Path) -> Vec<Value> {
    fs::read_to_string(dir.join("events.jsonl"))
        .unwrap_or_default()
        .lines()
        .map(|l| serde_json::from_str(l).expect(l))
        .collect()
}

fn kinds(dir: &Path) -> Vec<String> {
    events(dir)
        .iter()
        .map(|e| e["kind"].as_str().unwrap().to_string())
        .collect()
}

fn seq_first(dir: &Path, kind: &str) -> u64 {
    events(dir)
        .iter()
        .find(|e| e["kind"] == kind)
        .map(|e| e["seq"].as_u64().unwrap())
        .unwrap_or_else(|| panic!("no {kind}"))
}

fn cursor(dir: &Path) -> Option<u64> {
    fs::read_to_string(dir.join("cockpit.cursor"))
        .ok()
        .map(|t| t.trim().parse().unwrap())
}

fn driver_log(dir: &Path) -> String {
    fs::read_to_string(dir.join("driver.log")).unwrap_or_default()
}

fn wait_until(fx: &Fixture, dir: &Path, what: &str, secs: u64, mut ok: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(secs);
    while !ok() {
        if Instant::now() >= deadline {
            let statuses: Vec<String> = (1..=3)
                .map(|n| format!("{n:02}={}", fx.slice_status(n)))
                .collect();
            panic!(
                "timed out after {secs}s waiting for {what}\n--- state.json\n{}\n--- events\n{:?}\n--- slices {statuses:?}\n--- gh.log\n{:?}\n--- driver.log\n{}",
                state(dir),
                kinds(dir),
                fx.gh_log(),
                driver_log(dir)
            );
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn done() -> Value {
    json!({"outcome": "done"})
}

fn checkpoint() -> Value {
    json!({"outcome": "checkpoint"})
}

// ── wait, replay, status ─────────────────────────────────────────────────────

#[test]
fn wait_relays_the_block_the_still_running_line_the_dead_driver_and_the_end() {
    // The polish worker and the PR come from the stubs' defaults: a missing plan step is `done`.
    let fx = Fixture::with_index("cockpit-wait", &[("checkpoint_every", "10")]);
    fx.write_plan(&[
        json!({"outcome": "blocked", "question": QUESTION}),
        done(),
        done(),
        done(),
        checkpoint(),
    ]);
    let out = factory(&fx, &["run", "001"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    let dir = only_run_dir(&fx);
    let id = run_id(&dir);
    let driver = state(&dir)["pid"].as_u64().unwrap() as u32;
    wait_until(&fx, &dir, "the run to park", 60, || {
        state(&dir)["phase"] == "waiting-owner"
    });

    let out = factory(&fx, &["wait", &id, "--timeout", "10"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    assert_eq!(stdout(&out), format!("{BLOCK_LINE}\n"));
    assert_eq!(cursor(&dir), Some(seq_first(&dir, "slice_blocked")));

    // An open block prints again on every `wait`: a cockpit that died may never have relayed it.
    let t = Instant::now();
    let out = factory(&fx, &["wait", &id, "--timeout", "10"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    assert_eq!(stdout(&out), format!("{BLOCK_LINE}\n"));
    assert_eq!(cursor(&dir), Some(seq_first(&dir, "slice_blocked")));
    assert!(t.elapsed() < Duration::from_secs(5), "{:?}", t.elapsed());

    let out = factory(&fx, &["status", &id]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    let text = stdout(&out);
    let n = text.lines().count();
    assert!((8..=10).contains(&n), "{n} lines:\n{text}");
    for needle in [
        &format!("run {id} · waiting-owner"),
        "done: 02, 03",
        "blocked: 01",
        "lanes: 2 (1 parked) — 1: slice 01 blocked · 2: free",
        "checkpoints: 0 done",
        "open: BLOCK-1 (slice 01)",
        "quota: not paused",
        "PR: no PR yet",
        "last: ",
        "run_waiting",
    ] {
        assert!(text.contains(needle), "{needle:?} missing in:\n{text}");
    }
    assert!(!text.contains("driver dead"), "{text}");

    let out = factory(&fx, &["status", "001-20000101-dead"]);
    assert_eq!(out.status.code(), Some(1), "{}", stdout(&out));
    let text = stdout(&out);
    assert!(text.contains("no run 001-20000101-dead"), "{text}");
    assert!(text.contains("live runs:"), "{text}");
    assert!(
        text.contains(&format!("  {id}  waiting-owner  PRD-001")),
        "{text}"
    );
    let out = factory(&fx, &["status"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    assert!(stdout(&out).contains(&id), "{}", stdout(&out));

    let rc = unsafe { libc::kill(driver as i32, libc::SIGKILL) };
    assert_eq!(rc, 0);
    wait_until(&fx, &dir, "the driver to die", 5, || !pid_alive(driver));
    let out = factory(&fx, &["wait", &id, "--timeout", "1"]);
    assert_eq!(out.status.code(), Some(4), "{}", stdout(&out));
    assert_eq!(stdout(&out), "driver dead; run: factory run 001\n");
    let out = factory(&fx, &["status", &id]);
    let text = stdout(&out);
    assert!(text.lines().count() <= 10, "{text}");
    assert_eq!(
        text.lines().last(),
        Some(format!("driver dead (pid {driver}); run: factory run 001").as_str()),
        "{text}"
    );

    let out = factory(&fx, &["answer", "BLOCK-1", ANSWER]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    let out = factory(&fx, &["wait", &id, "--timeout", "120"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    assert_eq!(stdout(&out), format!("finished: {PR_URL}\n"));
    assert_eq!(state(&dir)["phase"], "finished");
    assert_eq!(cursor(&dir), Some(seq_first(&dir, "run_finished")));
    let out = factory(&fx, &["wait", &id]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    assert_eq!(stdout(&out), format!("finished: {PR_URL}\n"));

    // A missing cursor is a dead cockpit.
    fs::remove_file(dir.join("cockpit.cursor")).unwrap();
    let out = factory(&fx, &["wait", &id]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    assert_eq!(stdout(&out), format!("{BLOCK_LINE}\nfinished: {PR_URL}\n"));
    let out = factory(&fx, &["wait", &id, "--replay"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    assert_eq!(stdout(&out), format!("{BLOCK_LINE}\nfinished: {PR_URL}\n"));

    let out = factory(&fx, &["status", &id]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    let text = stdout(&out);
    assert!(text.lines().count() <= 10, "{text}");
    for needle in [
        &format!("run {id} · finished"),
        "done: 01, 02, 03",
        "lanes: 2 — 1: free · 2: free",
        "checkpoints: 1 done · last gate: green (checkpoint 1 attempt 1, 0.00 min)",
        "open: none",
        &format!("PR: {PR_URL}"),
        "run_finished",
    ] {
        assert!(text.contains(needle), "{needle:?} missing in:\n{text}");
    }
}

fn status_json(fx: &Fixture, cwd: &Path, args: &[&str]) -> Value {
    let out = factory_in(fx, cwd, args);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    let text = stdout(&out);
    assert_eq!(text.lines().count(), 1, "{text}");
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{e}: {text}"))
}

fn lines(v: &Value) -> Vec<String> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|l| l.as_str().unwrap().to_string())
        .collect()
}

#[test]
fn status_json_carries_the_board_and_what_the_owner_owes() {
    let fx = Fixture::with_index("cockpit-status-json", &[("checkpoint_every", "10")]);
    fx.write_plan(&[
        json!({"outcome": "blocked", "question": QUESTION}),
        done(),
        done(),
        done(),
        checkpoint(),
    ]);
    let out = factory(&fx, &["run", "001"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    let dir = only_run_dir(&fx);
    let id = run_id(&dir);
    wait_until(&fx, &dir, "the run to park", 60, || {
        state(&dir)["phase"] == "waiting-owner"
    });

    let plain = stdout(&factory(&fx, &["status", &id]));
    let v = status_json(&fx, fx.path(), &["status", &id, "--json"]);
    assert_eq!(v["schema"], 1);
    assert_eq!(v["factory"], env!("CARGO_PKG_VERSION"));
    let runs = v["runs"].as_array().unwrap();
    assert_eq!(runs.len(), 1, "{v}");
    let run = &runs[0];
    assert_eq!(run["id"], id.as_str());
    assert_eq!(run["feature"], "001");
    assert_eq!(run["phase"], "waiting-owner");
    assert_eq!(run["line"], "001 · BLOCK-1 open");
    // The first line's elapsed counts seconds, so it is compared up to them.
    let board = lines(&run["board"]);
    let plain: Vec<&str> = plain.lines().collect();
    let head = format!("run {id} · waiting-owner · ");
    assert!(
        board[0].starts_with(&head) && plain[0].starts_with(&head),
        "{board:#?}"
    );
    assert_eq!(board[1..], plain[1..]);
    assert_eq!(lines(&run["owner"]), [BLOCK_LINE]);

    let out = factory(&fx, &["status", &id, "--line"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    assert_eq!(stdout(&out), "001 · BLOCK-1 open\n");

    let out = factory(&fx, &["answer", "BLOCK-1", ANSWER]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    let v = status_json(&fx, fx.path(), &["status", &id, "--json"]);
    assert_eq!(v["runs"][0]["owner"], json!([]), "{v}");

    wait_until(&fx, &dir, "the run to finish", 120, || {
        state(&dir)["phase"] == "finished"
    });
    let v = status_json(&fx, fx.path(), &["status", &id, "--json"]);
    assert_eq!(v["runs"][0]["owner"], json!([]), "{v}");
    let line = v["runs"][0]["line"].as_str().unwrap();
    assert!(line.starts_with("001 · finished · 3/3 done · "), "{line}");
    let out = factory(&fx, &["status", &id, "--line"]);
    assert_eq!(stdout(&out), format!("{line}\n"));

    let out = factory(&fx, &["status", "001-20000101-dead", "--json"]);
    assert_eq!(out.status.code(), Some(1), "{}", stdout(&out));
    assert_eq!(stdout(&out), "");
}

fn git_init(fx: &Fixture, dir: &Path) {
    fs::create_dir_all(dir).unwrap();
    let out = fx
        .command("git")
        .current_dir(dir)
        .args(["init", "-q"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
}

fn write_live_run(fx: &Fixture, id: &str, repo: &Path) {
    let dir = data_dir(fx).join("runs").join(id);
    fs::create_dir_all(&dir).unwrap();
    let state = json!({
        "id": id,
        "repo": repo.canonicalize().unwrap(),
        "feature": &id[..3],
        "branch": null,
        "phase": "building",
        "pid": 0,
        "started_at": "2026-10-30T12:00:00Z",
        "updated_at": "2026-10-30T12:00:00Z",
        "budget_weighted_tokens": 1000,
        "stop_requested": false,
    });
    fs::write(dir.join("state.json"), state.to_string()).unwrap();
}

#[test]
fn here_keeps_this_repos_runs_and_a_listing_exits_zero() {
    let fx = Fixture::new("cockpit-status-here");
    let base = data_dir(&fx).parent().unwrap().to_path_buf();
    let mine = base.join("mine");
    let theirs = base.join("theirs");
    let outside = base.join("outside");
    git_init(&fx, &mine);
    git_init(&fx, &theirs);
    let sub = mine.join("deep/sub");
    fs::create_dir_all(&sub).unwrap();
    fs::create_dir_all(&outside).unwrap();
    write_live_run(&fx, "007-20261030-aaaa", &mine);
    write_live_run(&fx, "008-20261030-bbbb", &theirs);

    let v = status_json(&fx, &sub, &["status", "--here", "--json"]);
    let runs = v["runs"].as_array().unwrap();
    assert_eq!(runs.len(), 1, "{v}");
    assert_eq!(runs[0]["id"], "007-20261030-aaaa");
    assert_eq!(
        runs[0]["owner"],
        json!(["driver dead; run: factory run 007"])
    );

    let v = status_json(&fx, &outside, &["status", "--here", "--json"]);
    assert_eq!(v["runs"], json!([]), "{v}");

    let out = factory_in(&fx, &mine, &["status", "--here", "--line"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    assert_eq!(stdout(&out), "007 · driver dead\n");
    let out = factory_in(&fx, &outside, &["status", "--here", "--line"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    assert_eq!(stdout(&out), "");

    let out = factory_in(&fx, &outside, &["status"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    let text = stdout(&out);
    assert!(
        text.contains("007-20261030-aaaa") && text.contains("008-20261030-bbbb"),
        "{text}"
    );
    let out = factory_in(&fx, &outside, &["status", "--here"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    assert_eq!(stdout(&out), "no live runs in this repository\n");
}

/// Unlike `events()`, tolerates the torn line this test plants.
fn readable(dir: &Path) -> Vec<(usize, Value)> {
    fs::read_to_string(dir.join("events.jsonl"))
        .unwrap()
        .lines()
        .enumerate()
        .filter_map(|(i, l)| Some((i + 1, serde_json::from_str(l).ok()?)))
        .collect()
}

#[test]
fn an_unreadable_event_line_is_skipped_and_said_and_the_run_goes_on() {
    let fx = Fixture::with_index("cockpit-torn-event", &[("checkpoint_every", "10")]);
    fx.write_plan(&[
        json!({"outcome": "blocked", "question": QUESTION}),
        done(),
        done(),
        done(),
        checkpoint(),
    ]);
    let out = factory(&fx, &["run", "001"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    let dir = only_run_dir(&fx);
    let id = run_id(&dir);
    wait_until(&fx, &dir, "the run to park", 60, || {
        state(&dir)["phase"] == "waiting-owner"
    });

    // Under the emitters' lock so no driver event lands between the two lines.
    let last = events(&dir).last().unwrap()["seq"].as_u64().unwrap();
    let unknown = last + 1;
    let path = dir.join("events.jsonl");
    let mut f = fs::OpenOptions::new().append(true).open(&path).unwrap();
    f.lock().unwrap();
    std::io::Write::write_all(
        &mut f,
        format!(
            "{{\"seq\":{unknown},\"ts\":\"2026-10-11T00:00:00Z\",\"kind\":\"from_a_newer_factory\",\"x\":1}}\n{{\"seq\":{},\"ts\":\"2026-10-11T00:00:00Z\",\"kind\":\"run_fin",
            unknown + 1
        )
        .as_bytes(),
    )
    .unwrap();
    drop(f);
    let torn = fs::read_to_string(&path).unwrap().lines().count();
    let note = format!(
        "factory: {}: skipped unreadable line {torn}\n",
        path.display()
    );

    let out = factory(&fx, &["status", &id]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    let text = stdout(&out);
    assert!(text.lines().count() <= 10, "{text}");
    assert!(text.contains("open: BLOCK-1 (slice 01)"), "{text}");
    assert!(
        !text.contains("unknown"),
        "an unknown kind is passed over: {text}"
    );
    assert_eq!(String::from_utf8_lossy(&out.stderr), note);

    let out = factory(&fx, &["wait", &id, "--timeout", "10"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    assert_eq!(stdout(&out), format!("{BLOCK_LINE}\n"));
    assert_eq!(String::from_utf8_lossy(&out.stderr), note);

    let out = factory(&fx, &["answer", "BLOCK-1", ANSWER]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    let after = readable(&dir);
    let answered = after
        .iter()
        .find(|(_, e)| e["kind"] == "block_answered")
        .unwrap();
    assert_eq!(answered.0, torn + 1);
    assert_eq!(answered.1["seq"].as_u64(), Some(unknown + 1));
    wait_until(&fx, &dir, "the run to end", 60, || {
        let p = state(&dir)["phase"].clone();
        p == "finished" || p == "failed"
    });
    assert_eq!(state(&dir)["phase"], "finished", "{}", driver_log(&dir));
    let log = driver_log(&dir);
    assert_eq!(
        log.matches(&format!("events.jsonl line {torn} is not an event ("))
            .count(),
        1,
        "said once: {log}"
    );
    assert!(!log.contains(&format!("line {} is not", torn - 1)), "{log}");
    let seqs: Vec<u64> = readable(&dir)
        .iter()
        .map(|(_, e)| e["seq"].as_u64().unwrap())
        .collect();
    assert!(seqs.windows(2).all(|w| w[0] < w[1]), "{seqs:?}");
    let out = factory(&fx, &["status", &id]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    assert!(
        stdout(&out).contains(&format!("run {id} · finished")),
        "{}",
        stdout(&out)
    );
    assert_eq!(String::from_utf8_lossy(&out.stderr), note);
}

#[test]
fn a_budget_park_is_relayed_and_is_not_a_dead_driver() {
    // A budget park exits the driver by design; restarting it would only park again.
    let fx = Fixture::with_index("cockpit-budget", &[("checkpoint_every", "10")]);
    fx.chain();
    let out = factory(&fx, &["run", "001", "--budget", "1"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    let dir = only_run_dir(&fx);
    let id = run_id(&dir);
    wait_until(&fx, &dir, "the parked driver to exit", 60, || {
        let s = state(&dir);
        s["phase"] == "waiting-owner" && !pid_alive(s["pid"].as_u64().unwrap() as u32)
    });

    let out = factory(&fx, &["wait", &id, "--timeout", "5"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    let text = stdout(&out);
    assert!(
        text.starts_with("parked: budget ") && text.ends_with("/1\n") && text.lines().count() == 1,
        "{text}"
    );
    let cursor_at = cursor(&dir);
    let t = Instant::now();
    let again = factory(&fx, &["wait", &id, "--timeout", "10"]);
    assert_eq!(again.status.code(), Some(0), "{}", stdout(&again));
    assert_eq!(stdout(&again), text);
    assert_eq!(cursor(&dir), cursor_at);
    assert!(t.elapsed() < Duration::from_secs(5), "{:?}", t.elapsed());
    let out = factory(&fx, &["status", &id]);
    let text = stdout(&out);
    assert!(text.contains("/1 (parked)"), "{text}");
    assert!(!text.contains("driver dead"), "{text}");
    let parked = stdout(&again);
    let v = status_json(&fx, fx.path(), &["status", &id, "--json"]);
    assert_eq!(v["runs"][0]["line"], "001 · parked: budget", "{v}");
    assert_eq!(lines(&v["runs"][0]["owner"]), [parked.trim_end()]);
}

// ── factory stop ─────────────────────────────────────────────────────────────

#[test]
fn stop_ends_the_run_after_the_worker_and_the_next_run_builds_the_rest() {
    let fx = Fixture::with_index("cockpit-stop", &[("checkpoint_every", "10")]);
    fx.chain();
    fx.write_plan(&[
        json!({"outcome": "done", "delay_ms": 3000}),
        done(),
        done(),
        checkpoint(),
    ]);
    let out = factory(&fx, &["run", "001"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    let dir = only_run_dir(&fx);
    let id = run_id(&dir);
    wait_until(&fx, &dir, "slice 01 under a worker", 20, || {
        fx.calls().len() == 1
            && fx.slice_status(1) == "building"
            && state(&dir)["workers"]
                .as_array()
                .is_some_and(|w| w.len() == 1)
    });
    let out = factory(&fx, &["wait", &id, "--timeout", "0"]);
    assert_eq!(out.status.code(), Some(3), "{}", stdout(&out));
    let text = stdout(&out);
    assert!(
        text.starts_with("still running: ") && text.ends_with(", no open question\n"),
        "{text}"
    );
    assert_eq!(text.lines().count(), 1, "{text}");
    assert_eq!(cursor(&dir), None);
    let out = factory(&fx, &["stop", &id]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    assert_eq!(
        stdout(&out),
        format!(
            "stop requested for run {id} (pid {}): 1 worker in flight; the run finishes when it exits\n",
            state(&dir)["pid"]
        )
    );
    let out = factory(&fx, &["stop", &id]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    assert!(
        stdout(&out).starts_with(&format!("stop already requested for run {id} ")),
        "{}",
        stdout(&out)
    );
    assert!(
        stdout(&out).contains("1 worker in flight"),
        "{}",
        stdout(&out)
    );
    let out = factory(&fx, &["status", &id]);
    let text = stdout(&out);
    assert!(
        text.contains("stop requested: 1 in flight; the run finishes when they exit"),
        "{text}"
    );

    // The phase flips to finished before the driver logs its last line.
    wait_until(&fx, &dir, "the run to stop", 30, || {
        state(&dir)["phase"] == "finished" && driver_log(&dir).contains(" stopped (")
    });
    let s = state(&dir);
    assert_eq!(s["stopped"], true);
    assert_eq!(s["stop_requested"], true);
    assert_eq!(fx.slice_status(1), "done");
    assert_eq!(fx.slice_status(2), "pending");
    assert_eq!(fx.slice_status(3), "pending");
    let k = kinds(&dir);
    assert_eq!(k.last().map(String::as_str), Some("run_stopped"), "{k:?}");
    assert!(k.contains(&"slice_committed".to_string()), "{k:?}");
    assert!(!k.contains(&"run_finished".to_string()), "{k:?}");
    let stopped = events(&dir).pop().unwrap();
    assert_eq!(stopped["slices"], 1, "{stopped}");
    assert!(stopped["duration_minutes"].as_f64().is_some(), "{stopped}");
    assert!(
        driver_log(&dir).contains(&format!(
            "run {id} stopped (1 slice(s) done) — factory run 001 resumes the feature in a new run"
        )),
        "{}",
        driver_log(&dir)
    );
    let out = factory(&fx, &["wait", &id]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    assert_eq!(
        stdout(&out),
        "stopped: 1 slice(s) done — factory run 001 resumes the feature in a new run\n"
    );
    let out = factory(&fx, &["stop", &id]);
    assert_eq!(out.status.code(), Some(0));
    assert!(
        stdout(&out).contains("already finished"),
        "{}",
        stdout(&out)
    );

    let out = factory(&fx, &["run", "001"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    assert!(!stdout(&out).contains("resumed"), "{}", stdout(&out));
    let dirs = run_dirs(&fx);
    assert_eq!(dirs.len(), 2, "{dirs:?}");
    let dir2 = dirs.iter().find(|d| **d != dir).unwrap().clone();
    wait_until(&fx, &dir2, "the second run to finish", 120, || {
        state(&dir2)["phase"] == "finished"
    });
    for n in 1..=3 {
        assert_eq!(fx.slice_status(n), "done", "slice {n:02}");
    }
    assert_eq!(state(&dir2)["stopped"], false);
    let k = kinds(&dir2);
    assert_eq!(k.last().map(String::as_str), Some("run_finished"), "{k:?}");
    assert_eq!(k.iter().filter(|k| *k == "slice_done").count(), 2, "{k:?}");
}

// ── the cockpit skill ────────────────────────────────────────────────────────

#[test]
fn the_cockpit_skill_is_under_sixty_lines_and_names_the_five_commands() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let text = fs::read_to_string(root.join("skills/deliver/SKILL.md")).unwrap();
    let lines = text.lines().count();
    assert!(lines < 60, "{lines} lines");
    for needle in [
        "factory run",
        "factory wait",
        "factory answer",
        "factory status",
        "factory stop",
        "AskUserQuestion",
        "PushNotification",
        // the review round's shape
        "Review all n now (Recommended) / Only the d decide ones / Later",
        "four questions",
        // the log, read and written through the tool
        "factory docs log show NNN --decide",
        "--kind review --ref AD-{n} --verdict kept",
        "--timeout 3300",
        // an unsliced PRD is sliced first, without the write question
        "not sliced",
        "from /deliver",
        // a dead cockpit reattaches
        "replays every event",
        "repeats every question and park still open",
        "prints a question again on every call until it is answered",
        // a template stand-in is relayed, not asked about
        "`stand-in: …`: information",
    ] {
        assert!(text.contains(needle), "{needle:?} missing in:\n{text}");
    }
    assert!(!text.contains("--timeout 1500"), "the old poll");
    // nothing owner-specific
    for forbidden in ["/home/", "~/Dropbox", "Dropbox", "emiliano", "Emiliano"] {
        assert!(!text.contains(forbidden), "{forbidden:?} in the skill");
    }
    let readme = fs::read_to_string(root.join("skills/README.md")).unwrap();
    assert!(readme.contains("`factory init`"), "{readme}");
}
