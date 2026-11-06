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
    assert_eq!(
        run["repo"],
        fx.path().canonicalize().unwrap().display().to_string()
    );
    assert_eq!(run["slug"], "fixture");
    assert_eq!(run["slices"].as_array().unwrap().len(), 3, "{run}");
    assert_eq!(run["slices"][0], json!({"n": 1, "status": "blocked"}));

    assert_eq!(
        run["questions"],
        json!([{
            "id": "BLOCK-1",
            "at": "slice",
            "n": 1,
            "text": QUESTION,
            "answer": ["factory", "answer", "--feature", "001", "BLOCK-1"],
        }]),
        "{run}"
    );

    let out = factory(&fx, &["status", &id, "--line"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    assert_eq!(stdout(&out), "001 · BLOCK-1 open\n");

    let out = answer_as_told(&fx, run, "BLOCK-1", ANSWER);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    assert!(dir.join("answers/BLOCK-1.json").is_file());
    let v = status_json(&fx, fx.path(), &["status", &id, "--json"]);
    assert_eq!(v["runs"][0]["owner"], json!([]), "{v}");
    assert_eq!(v["runs"][0]["questions"], json!([]), "{v}");

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

/// Runs the `answer` argv a run's `questions` entry gives, from the run's repo, with `text` appended.
fn answer_as_told(fx: &Fixture, run: &Value, id: &str, text: &str) -> Output {
    let q = run["questions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|q| q["id"] == id)
        .unwrap_or_else(|| panic!("no {id} in {run}"));
    let argv: Vec<&str> = q["answer"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a.as_str().unwrap())
        .collect();
    assert_eq!(argv[..2], ["factory", "answer"], "{q}");
    let mut args = argv[1..].to_vec();
    args.push(text);
    factory_in(fx, Path::new(run["repo"].as_str().unwrap()), &args)
}

#[test]
fn status_json_gives_a_preflight_ask_the_command_that_answers_it() {
    use factory::events::EventKind;
    use factory::run::{Phase, Run};

    let fx = Fixture::new("cockpit-status-ask");
    let item = "no inner loop declared";
    // The run's pid is this test's, so the answer finds a live driver and spawns none.
    let mut run = Run::create(&data_dir(&fx), fx.path(), "001", None, 1).unwrap();
    run.emit(EventKind::AskOpened {
        id: "ASK-1".into(),
        text: item.into(),
    })
    .unwrap();
    run.set_phase(Phase::WaitingOwner).unwrap();
    let id = run.id().to_string();

    let v = status_json(&fx, fx.path(), &["status", "--json"]);
    let listed = &v["runs"][0];
    assert_eq!(listed["id"], id.as_str(), "{v}");
    assert_eq!(
        listed["questions"],
        json!([{
            "id": "ASK-1",
            "at": "preflight",
            "n": null,
            "text": item,
            "answer": ["factory", "answer", "--run", id, "ASK-1"],
        }]),
        "{listed}"
    );

    let out = answer_as_told(&fx, listed, "ASK-1", "proceed");
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    assert!(run.dir().join("answers/ASK-1.json").is_file());
    let v = status_json(&fx, fx.path(), &["status", &id, "--json"]);
    assert_eq!(v["runs"][0]["questions"], json!([]), "{v}");
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
    let budget = &v["runs"][0]["budget"];
    assert_eq!(budget["limit"], 1, "{v}");
    let used = budget["used"].as_u64().unwrap();
    assert!(used >= 1, "{v}");
    assert!(
        lines(&v["runs"][0]["board"])
            .iter()
            .any(|l| l.contains(&format!(
                "budget: {}/1 (parked)",
                factory::cockpit::tokens(used)
            ))),
        "{v}"
    );
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

const STARTED: &str = "2026-10-30T12:00:00Z";
const ENDED: &str = "2026-10-30T13:30:00Z";

/// A `state.json` from `overrides` over a bare building run, and one `events.jsonl` line per event.
fn write_run(fx: &Fixture, id: &str, repo: &Path, overrides: Value, events: &[Value]) -> PathBuf {
    let dir = data_dir(fx).join("runs").join(id);
    fs::create_dir_all(&dir).unwrap();
    let mut state = json!({
        "id": id,
        "repo": repo.canonicalize().unwrap(),
        "feature": &id[..3],
        "branch": null,
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
    let text: String = events
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let mut e = e.clone();
            e["seq"] = json!(i + 1);
            e["ts"] = json!(STARTED);
            format!("{e}\n")
        })
        .collect();
    fs::write(dir.join("events.jsonl"), text).unwrap();
    dir
}

fn since_started() -> u64 {
    factory::clock::unix_now() - factory::clock::from_rfc3339(STARTED).unwrap()
}

fn assert_elapsed_since_start(run: &Value) {
    let secs = run["elapsedSeconds"].as_u64().unwrap();
    let now = since_started();
    assert!(secs <= now && now - secs < 10, "{secs} vs {now}: {run}");
}

#[test]
fn status_json_says_where_a_run_belongs() {
    let fx = Fixture::new("cockpit-status-where");
    let base = data_dir(&fx).parent().unwrap().to_path_buf();
    let bare = base.join("bare");
    git_init(&fx, &bare);
    let dir = write_run(
        &fx,
        "001-20261030-aaaa",
        fx.path(),
        json!({"branch": "feat/001-fixture"}),
        &[
            json!({"kind": "run_started"}),
            json!({"kind": "slice_spawned", "slice": 1, "session_id": "s-1", "pid": 0}),
            json!({"kind": "slice_done", "slice": 1}),
        ],
    );
    write_run(&fx, "002-20261030-bbbb", &bare, json!({}), &[]);

    let v = status_json(&fx, fx.path(), &["status", "--json"]);
    assert_eq!(v["schema"], 1);
    let runs = v["runs"].as_array().unwrap();
    assert_eq!(runs.len(), 2, "{v}");
    let (run, other) = (&runs[0], &runs[1]);
    assert_eq!(run["id"], "001-20261030-aaaa");

    let state = state(&dir);
    assert_eq!(run["repo"], state["repo"]);
    assert_eq!(
        run["repo"],
        fx.path().canonicalize().unwrap().display().to_string()
    );
    assert_eq!(run["branch"], "feat/001-fixture");
    assert_eq!(run["slug"], "fixture");
    let prd = fs::read_to_string(fx.path().join(harness::FEATURE_DIR).join("prd.md")).unwrap();
    let title = factory::docs::frontmatter::parse(&prd)
        .str("title")
        .unwrap()
        .to_string();
    assert_eq!(run["title"], title.as_str());
    assert_eq!(title, "Fixture");
    assert_eq!(run["startedAt"], STARTED);
    assert_eq!(run["updatedAt"], STARTED);
    assert_elapsed_since_start(run);
    let last = events(&dir).last().unwrap()["seq"].clone();
    assert_eq!(run["seq"], last);
    assert_eq!(run["seq"], 3);

    assert_eq!(other["id"], "002-20261030-bbbb");
    assert_eq!(
        other["repo"],
        bare.canonicalize().unwrap().display().to_string()
    );
    assert_eq!(other["branch"], Value::Null);
    assert_eq!(other["slug"], Value::Null);
    assert_eq!(other["title"], Value::Null);
    assert_eq!(other["seq"], 0);
    assert_eq!(other["slices"], Value::Null);
    assert_eq!(other["line"], "002 · driver dead");
}

struct Want {
    name: &'static str,
    state: Value,
    events: Vec<Value>,
    phase: &'static str,
    checkpoints: Value,
    gate_line: &'static str,
    pr: Option<&'static str>,
    ci: Value,
    paused: bool,
    parked: Value,
    driver_dead: bool,
    stopped: bool,
    over: bool,
}

impl Default for Want {
    fn default() -> Want {
        Want {
            name: "",
            state: json!({}),
            events: vec![],
            phase: "building",
            checkpoints: json!({"done": 0, "lastGate": null}),
            gate_line: "not run yet",
            pr: None,
            ci: Value::Null,
            paused: false,
            parked: Value::Null,
            driver_dead: false,
            stopped: false,
            over: false,
        }
    }
}

#[test]
fn status_json_draws_each_run_state_as_fields() {
    let fx = Fixture::new("cockpit-status-fields");
    fx.set_slice_status(1, "done");
    fx.set_slice_status(2, "building");
    fx.set_slice_status(3, "blocked");
    let alive = std::process::id();
    let until = factory::clock::unix_now() + 3600;
    let started = json!({"kind": "run_started"});
    let pr = json!({"kind": "pr_opened", "url": PR_URL});
    let finished = json!({"kind": "run_finished", "pr_url": PR_URL});
    let gate = |red: bool| json!({"kind": "gate_run", "n": 1, "attempt": 1, "red": red, "minutes": 1.5, "log": "gates/1-1.log"});
    let over = |extra: Value| {
        let mut s = json!({"phase": "finished", "updated_at": ENDED, "branch": "feat/001-fixture"});
        for (k, v) in extra.as_object().unwrap() {
            s[k] = v.clone();
        }
        s
    };
    let wants = [
        Want {
            name: "building",
            state: json!({"pid": alive}),
            events: vec![
                started.clone(),
                gate(false),
                json!({"kind": "checkpoint_done", "n": 1, "sha": "abc"}),
            ],
            checkpoints: json!({"done": 1, "lastGate": "green"}),
            gate_line: "green (checkpoint 1 attempt 1, 1.50 min)",
            ..Want::default()
        },
        Want {
            name: "blocked",
            state: json!({"pid": alive, "phase": "waiting-owner"}),
            events: vec![
                started.clone(),
                gate(true),
                json!({"kind": "slice_blocked", "slice": 3, "block_id": "BLOCK-1", "question": QUESTION}),
                json!({"kind": "run_waiting", "blocked": [3], "asks": []}),
            ],
            phase: "waiting-owner",
            checkpoints: json!({"done": 0, "lastGate": "red"}),
            gate_line: "red (checkpoint 1 attempt 1, 1.50 min)",
            ..Want::default()
        },
        Want {
            name: "parked on budget",
            state: json!({"phase": "waiting-owner"}),
            events: vec![
                started.clone(),
                json!({"kind": "run_parked", "reason": "budget", "weighted": 1200, "budget": 1000}),
            ],
            phase: "waiting-owner",
            parked: json!("budget"),
            ..Want::default()
        },
        Want {
            name: "paused",
            state: json!({"pid": alive, "phase": "paused", "paused_until": until}),
            events: vec![
                started.clone(),
                json!({"kind": "run_paused", "literal": "limit", "resets_at": until}),
            ],
            phase: "paused",
            paused: true,
            ..Want::default()
        },
        Want {
            name: "driver dead",
            events: vec![started.clone()],
            driver_dead: true,
            ..Want::default()
        },
        Want {
            name: "shipped with CI green",
            state: over(json!({"pr_url": PR_URL})),
            events: vec![
                started.clone(),
                json!({"kind": "gate_cached", "n": 1, "attempt": 2, "red": false, "log": "gates/1-1.log"}),
                json!({"kind": "checkpoint_done", "n": 1, "sha": null}),
                pr.clone(),
                json!({"kind": "ci_red", "check": "test", "round": 0, "head": "a"}),
                json!({"kind": "ci_fix_committed", "round": 1, "sha": "b", "files": []}),
                json!({"kind": "ci_green", "head": "b"}),
                finished.clone(),
            ],
            phase: "finished",
            checkpoints: json!({"done": 1, "lastGate": "green"}),
            gate_line: "green (checkpoint 1 attempt 2, cached)",
            pr: Some(PR_URL),
            ci: json!("green"),
            over: true,
            ..Want::default()
        },
        Want {
            name: "CI red",
            state: json!({"pid": alive, "phase": "closeout", "branch": "feat/001-fixture", "pr_url": PR_URL}),
            events: vec![
                started.clone(),
                pr.clone(),
                json!({"kind": "ci_red", "check": "test", "round": 0, "head": "a"}),
            ],
            phase: "closeout",
            pr: Some(PR_URL),
            ci: json!("red"),
            ..Want::default()
        },
        Want {
            name: "finished with no checks",
            state: over(json!({"pr_url": PR_URL})),
            events: vec![started.clone(), pr.clone(), finished.clone()],
            phase: "finished",
            pr: Some(PR_URL),
            ci: json!("none"),
            over: true,
            ..Want::default()
        },
        Want {
            name: "stopped",
            state: over(json!({"stopped": true, "stop_requested": true})),
            events: vec![
                started.clone(),
                json!({"kind": "stop_requested"}),
                json!({"kind": "run_stopped"}),
            ],
            phase: "finished",
            stopped: true,
            over: true,
            ..Want::default()
        },
        Want {
            name: "failed",
            state: over(json!({"phase": "failed"})),
            events: vec![
                started.clone(),
                json!({"kind": "run_failed", "error": "boom"}),
            ],
            phase: "failed",
            over: true,
            ..Want::default()
        },
    ];
    let slices = json!([
        {"n": 1, "status": "done"},
        {"n": 2, "status": "building"},
        {"n": 3, "status": "blocked"},
    ]);

    let ids: Vec<String> = (0..wants.len())
        .map(|i| format!("001-20261030-{i:04}"))
        .collect();
    for (w, id) in wants.iter().zip(&ids) {
        write_run(&fx, id, fx.path(), w.state.clone(), &w.events);
    }

    let live = status_json(&fx, fx.path(), &["status", "--json"]);
    let listed: Vec<&str> = live["runs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].as_str().unwrap())
        .collect();
    let want_live: Vec<&str> = wants
        .iter()
        .zip(&ids)
        .filter(|(w, _)| !w.over)
        .map(|(_, id)| id.as_str())
        .collect();
    assert_eq!(listed, want_live);

    for (w, id) in wants.iter().zip(&ids) {
        let v = status_json(&fx, fx.path(), &["status", id, "--json"]);
        let run = &v["runs"][0];
        let n = w.name;
        assert_eq!(run["phase"], w.phase, "{n}: {run}");
        assert_eq!(run["slices"], slices, "{n}");
        assert_eq!(run["budget"], json!({"used": 0, "limit": 1000}), "{n}");
        assert_eq!(run["checkpoints"], w.checkpoints, "{n}");
        assert_eq!(run["pr"], json!(w.pr), "{n}");
        assert_eq!(run["ci"], w.ci, "{n}");
        let paused = w.paused.then(|| factory::clock::rfc3339(until));
        assert_eq!(run["pausedUntil"], json!(paused), "{n}");
        assert_eq!(run["parked"], w.parked, "{n}");
        assert_eq!(run["driverDead"], w.driver_dead, "{n}");
        assert_eq!(run["stopped"], w.stopped, "{n}");
        assert_eq!(run["seq"], w.events.len(), "{n}");
        if w.over {
            assert_eq!(run["elapsedSeconds"], 5400, "{n}");
        } else {
            assert_elapsed_since_start(run);
        }

        let board = lines(&run["board"]);
        let find = |prefix: &str| {
            board
                .iter()
                .find(|l| l.starts_with(prefix))
                .unwrap_or_else(|| panic!("{n}: no {prefix} line in {board:#?}"))
                .clone()
        };
        assert_eq!(
            find("slices "),
            "slices 3: done: 01 · building: 02 · blocked: 03 · pending: —",
            "{n}"
        );
        assert_eq!(
            find("checkpoints:"),
            format!(
                "checkpoints: {} done · last gate: {}",
                w.checkpoints["done"], w.gate_line
            ),
            "{n}"
        );
        assert!(
            w.gate_line
                .starts_with(w.checkpoints["lastGate"].as_str().unwrap_or("not run")),
            "{n}"
        );
        let quota = find("quota:");
        let budget = format!(
            "budget: {}/{}",
            factory::cockpit::tokens(run["budget"]["used"].as_u64().unwrap()),
            factory::cockpit::tokens(run["budget"]["limit"].as_u64().unwrap())
        );
        assert!(quota.contains(&budget), "{n}: {quota}");
        assert_eq!(
            quota.ends_with(" (parked)"),
            !w.parked.is_null(),
            "{n}: {quota}"
        );
        match &paused {
            Some(at) => assert!(
                quota.starts_with(&format!("quota: paused until {at} · ")),
                "{n}: {quota}"
            ),
            None => assert!(quota.starts_with("quota: not paused · "), "{n}: {quota}"),
        }
        assert_eq!(
            find("PR:"),
            format!("PR: {}", w.pr.unwrap_or("no PR yet")),
            "{n}"
        );
        assert_eq!(
            board.iter().any(|l| l.starts_with("driver dead")),
            w.driver_dead,
            "{n}"
        );
        assert_eq!(board[0].contains("(stopped)"), w.stopped, "{n}");
    }
}

fn days_ago(days: u64) -> String {
    factory::clock::rfc3339(factory::clock::unix_now() - days * 86_400)
}

#[test]
fn recent_adds_runs_that_ended_this_week() {
    let fx = Fixture::new("cockpit-status-recent");
    let outside = data_dir(&fx).parent().unwrap().join("outside");
    fs::create_dir_all(&outside).unwrap();
    let ended = |started: u64, updated: u64| json!({"phase": "finished", "started_at": days_ago(started), "updated_at": days_ago(updated)});
    write_run(
        &fx,
        "001-20261106-live",
        fx.path(),
        json!({"started_at": days_ago(1), "updated_at": days_ago(1)}),
        &[json!({"kind": "run_started"})],
    );
    write_run(&fx, "001-20261104-week", fx.path(), ended(3, 2), &[]);
    write_run(&fx, "001-20261105-long", fx.path(), ended(10, 6), &[]);
    write_run(&fx, "001-20261029-old", fx.path(), ended(9, 8), &[]);
    let ids = |v: &Value| -> Vec<String> {
        v["runs"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["id"].as_str().unwrap().to_string())
            .collect()
    };

    let v = status_json(&fx, fx.path(), &["status", "--json", "--recent"]);
    assert_eq!(
        ids(&v),
        [
            "001-20261105-long",
            "001-20261104-week",
            "001-20261106-live"
        ],
        "{v}"
    );
    assert_eq!(v["runs"][1]["phase"], "finished");
    assert_eq!(v["runs"][1]["questions"], json!([]));
    let v = status_json(&fx, fx.path(), &["status", "--json"]);
    assert_eq!(ids(&v), ["001-20261106-live"], "{v}");
    let v = status_json(&fx, fx.path(), &["status", "--json", "--recent", "--here"]);
    assert_eq!(
        ids(&v),
        [
            "001-20261105-long",
            "001-20261104-week",
            "001-20261106-live"
        ],
        "{v}"
    );
    let v = status_json(&fx, &outside, &["status", "--json", "--recent", "--here"]);
    assert_eq!(v["runs"], json!([]), "{v}");

    for (args, says) in [
        (
            &["status", "--recent"][..],
            "the following required arguments were not provided",
        ),
        (
            &["status", "001-20261106-live", "--json", "--recent"][..],
            "cannot be used with",
        ),
    ] {
        let out = factory(&fx, args);
        assert_eq!(out.status.code(), Some(2), "{args:?}: {}", stdout(&out));
        assert_eq!(stdout(&out), "", "{args:?}");
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(
            err.starts_with("error: ") && err.contains(says),
            "{args:?}: {err}"
        );
    }
}
