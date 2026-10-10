//! Slice 10 of PRD-001: the cockpit (PRD §9 FR-7, §3 FR-1 bullet 4).
//! `factory wait` blocks until the next event the owner must see and replays
//! what a dead cockpit missed; `factory status` is the board in ten lines;
//! `factory stop` ends a run cleanly after its in-flight workers and says so;
//! the cockpit skill is under 60 lines. Every `gh` here is the stub in
//! `tests/stubs/`; `origin` is the fixture's bare repo.
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
const BLOCK_LINE: &str = "BLOCK-101 slice 01: Which storage backs the fixture? (answer: factory answer BLOCK-101 \"...\")";

fn factory(fx: &Fixture, args: &[&str]) -> Output {
    let out = fx.command(BIN).args(args).output().unwrap();
    // The command and what it said, for a checkpoint's transcript: `cargo test`
    // keeps it unless `-- --nocapture` (AD-9002).
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

// ── tasks 1 and 2: wait, replay, status ──────────────────────────────────────

#[test]
fn wait_relays_the_block_the_still_running_line_the_dead_driver_and_the_end() {
    // The slice 04 parked fixture: 01 blocks on lane 1 while 02 and 03 build
    // on lane 2; one trailing checkpoint; the polish worker and the PR come
    // from the stubs' defaults (a missing plan step is `done`, a missing
    // `scenario/ci` line is green).
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

    // The first `wait`: the block, one line, exit 0; the cursor is at it.
    let out = factory(&fx, &["wait", &id, "--timeout", "10"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    assert_eq!(stdout(&out), format!("{BLOCK_LINE}\n"));
    assert_eq!(cursor(&dir), Some(seq_first(&dir, "slice_blocked")));

    // Nothing new: `still running` with the status line, exit 3, at the timeout.
    let t = Instant::now();
    let out = factory(&fx, &["wait", &id, "--timeout", "1"]);
    assert_eq!(out.status.code(), Some(3), "{}", stdout(&out));
    assert_eq!(
        stdout(&out),
        "still running: waiting-owner, BLOCK-101 open\n"
    );
    assert!(t.elapsed() < Duration::from_secs(5), "{:?}", t.elapsed());

    // The board in ten lines.
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
        "open: BLOCK-101 (slice 01)",
        "quota: not paused",
        "PR: no PR yet",
        "last: ",
        "run_waiting",
    ] {
        assert!(text.contains(needle), "{needle:?} missing in:\n{text}");
    }
    assert!(!text.contains("driver dead"), "{text}");

    // An unknown run id: exit 1 and the live runs.
    let out = factory(&fx, &["status", "001-20000101-dead"]);
    assert_eq!(out.status.code(), Some(1), "{}", stdout(&out));
    let text = stdout(&out);
    assert!(text.contains("no run 001-20000101-dead"), "{text}");
    assert!(text.contains("live runs:"), "{text}");
    assert!(
        text.contains(&format!("  {id}  waiting-owner  PRD-001")),
        "{text}"
    );
    // No run id at all: the same listing, exit 1.
    let out = factory(&fx, &["status"]);
    assert_eq!(out.status.code(), Some(1), "{}", stdout(&out));
    assert!(stdout(&out).contains(&id), "{}", stdout(&out));

    // The driver killed: `wait` exits 4 with the command that resumes it;
    // `status` says so on its last line.
    // SAFETY: a plain kill(2) on the test's own detached driver.
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

    // The owner answers: a driver is started for the parked run, the slice
    // respawns, the checkpoint and the closeout run; `wait` returns on
    // `run_finished` with the PR URL.
    let out = factory(&fx, &["answer", "BLOCK-101", ANSWER]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    let out = factory(&fx, &["wait", &id, "--timeout", "120"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    assert_eq!(stdout(&out), format!("finished: {PR_URL}\n"));
    assert_eq!(state(&dir)["phase"], "finished");
    assert_eq!(cursor(&dir), Some(seq_first(&dir, "run_finished")));
    // A `wait` on a run that is over says the end again and never blocks.
    let out = factory(&fx, &["wait", &id]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    assert_eq!(stdout(&out), format!("finished: {PR_URL}\n"));

    // A dead cockpit: no cursor replays both lines; `--replay` ignores it.
    fs::remove_file(dir.join("cockpit.cursor")).unwrap();
    let out = factory(&fx, &["wait", &id]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    assert_eq!(stdout(&out), format!("{BLOCK_LINE}\nfinished: {PR_URL}\n"));
    let out = factory(&fx, &["wait", &id, "--replay"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    assert_eq!(stdout(&out), format!("{BLOCK_LINE}\nfinished: {PR_URL}\n"));

    // `status` on the finished run: the PR URL, no open question.
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

#[test]
fn a_budget_park_is_relayed_and_is_not_a_dead_driver() {
    // FR-9's budget park exits the driver by design (AD-703): `wait` relays
    // the park once, then waits with the cap in its status line instead of
    // telling the cockpit to restart a driver that would park again.
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
    let out = factory(&fx, &["wait", &id, "--timeout", "1"]);
    assert_eq!(out.status.code(), Some(3), "{}", stdout(&out));
    let text = stdout(&out);
    assert!(
        text.starts_with("still running: waiting-owner, parked: budget ")
            && text.ends_with("/1, no open question\n"),
        "{text}"
    );
    let out = factory(&fx, &["status", &id]);
    let text = stdout(&out);
    assert!(text.contains("/1 (parked)"), "{text}");
    assert!(!text.contains("driver dead"), "{text}");
}

// ── task 3: factory stop ─────────────────────────────────────────────────────

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
    // The stop lands while the first worker runs (the slice's "300 ms after
    // start", made deterministic: once the worker is on the slice).
    wait_until(&fx, &dir, "slice 01 under a worker", 20, || {
        fx.calls().len() == 1
            && fx.slice_status(1) == "building"
            && state(&dir)["workers"]
                .as_array()
                .is_some_and(|w| w.len() == 1)
    });
    let out = factory(&fx, &["stop", &id]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    assert_eq!(
        stdout(&out),
        format!(
            "stop requested for run {id} (pid {}): 1 worker in flight; the run finishes when it exits\n",
            state(&dir)["pid"]
        )
    );
    // Asked again while it still runs: the remaining count.
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

    // The worker finishes, its slice is committed, the run ends on
    // `run_stopped` with phase `finished` and `stopped: true`.
    wait_until(&fx, &dir, "the run to stop", 30, || {
        state(&dir)["phase"] == "finished"
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
            "run {id} stopped (1 slice(s) done) — factory run 001 resumes"
        )),
        "{}",
        driver_log(&dir)
    );
    // The cockpit sees the stop as the end of the run.
    let out = factory(&fx, &["wait", &id]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    assert_eq!(
        stdout(&out),
        "stopped: 1 slice(s) done — factory run 001 resumes\n"
    );
    let out = factory(&fx, &["stop", &id]);
    assert_eq!(out.status.code(), Some(0));
    assert!(
        stdout(&out).contains("already finished"),
        "{}",
        stdout(&out)
    );

    // `factory run 001` is a new run that builds the other two from the board.
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

// ── task 4: the cockpit skill ────────────────────────────────────────────────

#[test]
fn the_cockpit_skill_is_under_sixty_lines_and_names_the_five_commands() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let text = fs::read_to_string(root.join("skills/factory-deliver/SKILL.md")).unwrap();
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
        // The `/deliver` §8 round shape.
        "Review all n now (Recommended) / Only the d decide ones / Later",
        "four questions",
        "`**Review:** decide` lines first",
        "**Owner ({date}):**",
        // The slice-entry answer about background Bash: the 25-minute poll.
        "--timeout 1500",
        // A dead cockpit reattaches.
        "replays every event",
    ] {
        assert!(text.contains(needle), "{needle:?} missing in:\n{text}");
    }
    // §19: nothing owner-specific.
    for forbidden in ["/home/", "~/Dropbox", "Dropbox", "emiliano", "Emiliano"] {
        assert!(!text.contains(forbidden), "{forbidden:?} in the skill");
    }
    let readme = fs::read_to_string(root.join("skills/README.md")).unwrap();
    assert!(
        readme.contains("ln -s \"$(pwd)/skills/factory-deliver\" ~/.claude/skills/factory-deliver"),
        "{readme}"
    );
}
