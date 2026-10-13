//! FR-9 (slice 07): a worker that hits a usage limit pauses the run until the
//! stream's `resetsAt` (15 minutes when it gave none), the pause counts
//! toward no cap, and the weighted token sum parks the run at the budget.
//!
//! The pauses are minutes long, so every driver here runs on the scaled
//! clock `src/clock.rs` reads from `FACTORY_CLOCK_SCALE` and
//! `FACTORY_CLOCK_ANCHOR`; the stub worker reads the same two variables, so
//! its `resetsAt` and the driver's `paused_until` agree. Events' `ts` are on
//! that clock too, which is what the assertions compare against.
#[path = "harness/mod.rs"]
mod harness;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;
use std::time::{Duration, Instant};

use factory::clock;
use factory::db::{Db, GateRow};
use factory::run::pid_alive;
use harness::Fixture;
use serde_json::{Value, json};

const BIN: &str = env!("CARGO_BIN_EXE_factory");
const BUILD: &str =
    "Build slice {NN} of PRD-001 as the appended system prompt says. Return the STATUS block.";
const FIX: &str = "Fix the red gate at checkpoint {n} of PRD-001 as the appended system prompt says. Return the STATUS block.";
const CHECKPOINT: &str =
    "Run checkpoint {n} of PRD-001 as the appended system prompt says. Return the STATUS block.";
/// The stub's `result` usage: 18 + 6514 + 0.1 × 41674 + 5 × 182 = 11609.4.
const STUB_WEIGHTED: u64 = 11_609;

/// One scaled clock per test: `scale` virtual seconds per wall second from
/// the wall second the test started. Scale 1 leaves the wall clock alone.
struct Clock {
    scale: u64,
    anchor: u64,
}

impl Clock {
    fn new(scale: u64) -> Clock {
        Clock {
            scale,
            anchor: clock::wall_now(),
        }
    }
}

fn factory(fx: &Fixture, clk: &Clock, args: &[&str]) -> Output {
    fx.command(BIN)
        .env("FACTORY_CLOCK_SCALE", clk.scale.to_string())
        .env("FACTORY_CLOCK_ANCHOR", clk.anchor.to_string())
        .args(args)
        .output()
        .unwrap()
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

fn count(kinds: &[String], kind: &str) -> usize {
    kinds.iter().filter(|k| *k == kind).count()
}

fn of_kind(dir: &Path, kind: &str) -> Vec<Value> {
    events(dir)
        .into_iter()
        .filter(|e| e["kind"] == kind)
        .collect()
}

/// The kinds from the first `from` on.
fn kinds_from(dir: &Path, from: &str) -> Vec<String> {
    let k = kinds(dir);
    let i = k
        .iter()
        .position(|x| x == from)
        .unwrap_or_else(|| panic!("no {from} in {k:?}"));
    k[i..].to_vec()
}

/// An event's `ts`, in the run's (scaled) epoch seconds.
fn ts(e: &Value) -> u64 {
    clock::from_rfc3339(e["ts"].as_str().unwrap()).unwrap()
}

fn db(fx: &Fixture) -> Db {
    Db::open(&data_dir(fx).join("factory.db")).unwrap()
}

fn gates(fx: &Fixture, dir: &Path) -> Vec<GateRow> {
    db(fx).gates(&run_id(dir)).unwrap()
}

fn driver_log(dir: &Path) -> String {
    fs::read_to_string(dir.join("driver.log")).unwrap_or_default()
}

/// Polls `ok`; on timeout, dumps the run so a failure in the detached
/// driver can be read from the test output.
fn wait_until(fx: &Fixture, dir: &Path, what: &str, secs: u64, mut ok: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(secs);
    while !ok() {
        if Instant::now() >= deadline {
            let statuses: Vec<String> = (1..=3)
                .map(|n| format!("{n:02}={}", fx.slice_status(n)))
                .collect();
            panic!(
                "timed out after {secs}s waiting for {what}\n--- state.json\n{}\n--- events\n{:?}\n--- slices {statuses:?}\n--- calls {}\n--- driver.log\n{}",
                state(dir),
                kinds(dir),
                fx.calls().len(),
                driver_log(dir),
            );
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn wait_phase(fx: &Fixture, dir: &Path, phase: &str, secs: u64) {
    wait_until(fx, dir, &format!("phase {phase}"), secs, || {
        state(dir)["phase"] == phase
    });
}

fn start(fx: &Fixture, clk: &Clock) -> PathBuf {
    let out = factory(fx, clk, &["run", "001"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    only_run_dir(fx)
}

fn done() -> Value {
    json!({"outcome": "done"})
}

fn step(outcome: &str) -> Value {
    json!({"outcome": outcome})
}

fn prompts(fx: &Fixture) -> Vec<String> {
    fx.calls()
        .iter()
        .map(|c| c["prompt"].as_str().unwrap_or("").to_string())
        .collect()
}

fn build(n: u8) -> String {
    BUILD.replace("{NN}", &format!("{n:02}"))
}

fn fix(n: u32) -> String {
    FIX.replace("{n}", &n.to_string())
}

fn checkpoint(n: u32) -> String {
    CHECKPOINT.replace("{n}", &n.to_string())
}

/// The polish worker's prompt: the slash command, nothing else (slice 09).
fn polish() -> String {
    "/polish".into()
}

/// `(role, outcome)` of every worker row, in id order.
fn rows(fx: &Fixture, dir: &Path) -> Vec<(String, String)> {
    db(fx)
        .workers(&run_id(dir))
        .unwrap()
        .iter()
        .map(|w| (w.role.clone(), w.outcome.clone().unwrap_or_default()))
        .collect()
}

fn outcomes(fx: &Fixture, dir: &Path, role: &str) -> Vec<String> {
    rows(fx, dir)
        .into_iter()
        .filter(|(r, _)| r == role)
        .map(|(_, o)| o)
        .collect()
}

/// `(red, cached, attempt)` of the rows for checkpoint `n`, in id order.
fn gate_shape(rows: &[GateRow], n: u32) -> Vec<(bool, bool, u32)> {
    rows.iter()
        .filter(|g| g.checkpoint == Some(n))
        .map(|g| (g.red, g.cached, g.attempt.unwrap()))
        .collect()
}

/// Slice `n`'s entries of `log.jsonl`, of `kind` (any kind when `None`).
fn slice_log(fx: &Fixture, n: u8, kind: Option<&str>) -> Vec<Value> {
    fx.log_entries()
        .into_iter()
        .filter(|e| e["slice"] == json!(n))
        .filter(|e| kind.is_none_or(|k| e["kind"] == k))
        .collect()
}

/// No `Dead worker` note on slice `n`.
fn assert_no_dead_note(fx: &Fixture, n: u8) {
    let notes: Vec<Value> = slice_log(fx, n, Some("note"))
        .into_iter()
        .filter(|e| e["title"] == "Dead worker")
        .collect();
    assert!(notes.is_empty(), "{notes:?}");
}

fn assert_all_done(fx: &Fixture) {
    for n in 1..=3 {
        assert_eq!(fx.slice_status(n), "done", "slice {n:02}");
    }
}

/// The pause's shape: one `run_paused`, then no spawn until `run_resumed_from_pause`.
fn assert_one_pause(dir: &Path) -> (Value, Value) {
    let k = kinds_from(dir, "run_paused");
    let resumed = k
        .iter()
        .position(|x| x == "run_resumed_from_pause")
        .unwrap_or_else(|| panic!("no resume in {k:?}"));
    assert!(
        !k[..resumed].contains(&"slice_spawned".to_string()),
        "a spawn inside the pause: {k:?}"
    );
    let paused = of_kind(dir, "run_paused");
    let resumes = of_kind(dir, "run_resumed_from_pause");
    assert_eq!(paused.len(), 1, "{paused:?}");
    assert_eq!(resumes.len(), 1, "{resumes:?}");
    (paused[0].clone(), resumes[0].clone())
}

#[test]
fn a_rate_limited_worker_pauses_the_run_until_the_reset() {
    let fx = Fixture::with_index("quota-pause", &[("checkpoint_every", "10")]);
    fx.chain();
    fx.write_plan(&[step("rate_limited"), done(), done(), done()]);
    // 120 virtual seconds of pause are 6 wall seconds.
    let clk = Clock::new(20);
    let dir = start(&fx, &clk);

    wait_until(&fx, &dir, "run_paused", 30, || {
        count(&kinds(&dir), "run_paused") == 1
    });
    let paused = of_kind(&dir, "run_paused").remove(0);
    assert_eq!(paused["literal"], "rejected", "{paused}");
    let resets_at = paused["resets_at"].as_u64().unwrap();
    let at = ts(&paused);
    // The stub set `resetsAt` 120 s ahead when it started; the driver read
    // the exit within a poll tick.
    assert!(
        (100..=120).contains(&(resets_at - at)),
        "resets_at {resets_at} vs ts {at}"
    );
    // While paused: phase `paused`, `paused_until` on disk, the slice back to
    // `pending` with its lane freed, and the one call that was limited.
    wait_phase(&fx, &dir, "paused", 10);
    assert_eq!(state(&dir)["paused_until"], json!(resets_at));
    assert_eq!(fx.slice_status(1), "pending");
    assert_eq!(fx.slice_lane(1), None);
    assert_eq!(fx.calls().len(), 1);
    assert_eq!(outcomes(&fx, &dir, "build"), ["rate_limited"]);

    // Nothing spawns before the resume; the calls are read before the events
    // so a spawn racing this poll shows up as a resume, never as a bare call.
    wait_until(&fx, &dir, "run_resumed_from_pause", 60, || {
        let calls = fx.calls().len();
        if count(&kinds(&dir), "run_resumed_from_pause") == 0 {
            assert_eq!(calls, 1, "a spawn during the pause");
            false
        } else {
            true
        }
    });
    wait_phase(&fx, &dir, "finished", 90);

    let (paused, resumed) = assert_one_pause(&dir);
    assert!(
        ts(&resumed) >= resets_at,
        "resumed at {} before resets_at {resets_at}",
        resumed["ts"]
    );
    assert_eq!(paused["resets_at"], json!(resets_at));
    let k = kinds(&dir);
    assert_eq!(count(&k, "slice_spawned"), 4, "{k:?}");
    assert_eq!(
        count(&kinds_from(&dir, "run_resumed_from_pause"), "slice_spawned"),
        3
    );
    assert_eq!(count(&k, "worker_dead"), 0, "{k:?}");
    assert_eq!(count(&k, "slice_blocked"), 0, "{k:?}");
    assert_eq!(count(&k, "run_parked"), 0, "{k:?}");
    assert_all_done(&fx);
    assert_eq!(
        outcomes(&fx, &dir, "build"),
        ["rate_limited", "done", "done", "done"]
    );
    let st = state(&dir);
    assert!(st["paused_until"].is_null(), "{st}");
    assert_eq!(st["phase"], "finished");
    let limited = &st["workers"][0];
    assert_eq!(limited["slice"], 1);
    assert_eq!(limited["outcome"], "rate_limited");
    // Four build calls, the closing checkpoint's worker and the polish worker.
    assert_eq!(fx.calls().len(), 6);
    assert_eq!(outcomes(&fx, &dir, "checkpoint"), ["done"]);
    assert_no_dead_note(&fx, 1);
}

#[test]
fn a_limit_with_no_reset_retries_after_fifteen_minutes() {
    let fx = Fixture::with_index("quota-no-reset", &[("checkpoint_every", "10")]);
    fx.chain();
    fx.write_plan(&[step("rate_limited_no_reset"), done(), done(), done()]);
    // 900 virtual seconds are 3 wall seconds.
    let clk = Clock::new(300);
    let dir = start(&fx, &clk);
    wait_phase(&fx, &dir, "finished", 90);

    let (paused, resumed) = assert_one_pause(&dir);
    assert_eq!(paused["literal"], "rejected", "{paused}");
    let resets_at = paused["resets_at"].as_u64().unwrap();
    let at = ts(&paused);
    // No `resetsAt` in the stream: 15 minutes from the pause itself.
    assert!(
        (880..=900).contains(&(resets_at - at)),
        "resets_at {resets_at} vs ts {at}"
    );
    assert!(ts(&resumed) >= resets_at, "{resumed} before {resets_at}");
    assert_all_done(&fx);
    assert_eq!(
        outcomes(&fx, &dir, "build"),
        ["rate_limited", "done", "done", "done"]
    );
    assert_eq!(count(&kinds(&dir), "worker_dead"), 0);
}

#[test]
fn two_pauses_on_one_slice_count_toward_no_cap() {
    let fx = Fixture::with_index("quota-no-cap", &[("checkpoint_every", "10")]);
    fx.chain();
    fx.write_plan(&[
        step("rate_limited"),
        step("rate_limited"),
        done(),
        done(),
        done(),
    ]);
    let clk = Clock::new(60);
    let dir = start(&fx, &clk);
    wait_phase(&fx, &dir, "finished", 90);

    let k = kinds(&dir);
    assert_eq!(count(&k, "run_paused"), 2, "{k:?}");
    assert_eq!(count(&k, "run_resumed_from_pause"), 2, "{k:?}");
    assert_eq!(count(&k, "slice_spawned"), 5, "{k:?}");
    // Two limited workers on slice 01 are not two deaths (DEATH_CAP = 2).
    assert_eq!(count(&k, "worker_dead"), 0, "{k:?}");
    assert_eq!(count(&k, "slice_blocked"), 0, "{k:?}");
    assert_all_done(&fx);
    assert_eq!(
        outcomes(&fx, &dir, "build"),
        ["rate_limited", "rate_limited", "done", "done", "done"]
    );
    let st = state(&dir);
    let deaths = st["workers"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|w| matches!(w["outcome"].as_str(), Some("dead" | "failed")))
        .count();
    assert_eq!(deaths, 0, "{st}");
    assert_no_dead_note(&fx, 1);
    let blocks = slice_log(&fx, 1, Some("block"));
    assert!(blocks.is_empty(), "{blocks:?}");
    // Each re-limited retry announced its own pause, in order.
    let seq: Vec<String> = kinds_from(&dir, "run_paused")
        .into_iter()
        .filter(|x| x.starts_with("run_") || x == "slice_spawned")
        .collect();
    assert_eq!(
        seq[..5],
        [
            "run_paused",
            "run_resumed_from_pause",
            "slice_spawned",
            "run_paused",
            "run_resumed_from_pause",
        ],
        "{seq:?}"
    );
}

#[test]
fn a_rate_limited_fix_worker_is_not_a_red() {
    let fx = Fixture::new("quota-fix");
    fx.chain();
    fx.script_gate(&["red"]);
    fx.write_plan(&[
        done(),
        done(),
        step("rate_limited"),
        step("fix"),
        step("checkpoint"),
    ]);
    let clk = Clock::new(60);
    let dir = start(&fx, &clk);
    wait_phase(&fx, &dir, "finished", 120);

    // The limited fix attempt is retried on the same gate attempt, with no
    // gate run between; the real fix turns the second gate green.
    assert_eq!(
        prompts(&fx),
        [
            build(1),
            build(2),
            fix(1),
            fix(1),
            checkpoint(1),
            build(3),
            checkpoint(2),
            polish()
        ]
    );
    let fixes = of_kind(&dir, "fix_spawned");
    assert_eq!(fixes.len(), 2, "{fixes:?}");
    assert!(
        fixes.iter().all(|f| f["attempt"] == 1 && f["n"] == 1),
        "{fixes:?}"
    );
    assert_eq!(
        gate_shape(&gates(&fx, &dir), 1),
        [(true, false, 1), (false, false, 2)]
    );
    let k = kinds(&dir);
    assert_eq!(count(&k, "checkpoint_blocked"), 0, "{k:?}");
    assert_eq!(count(&k, "worker_dead"), 0, "{k:?}");
    assert_eq!(count(&k, "checkpoint_done"), 2, "{k:?}");
    let (paused, resumed) = assert_one_pause(&dir);
    assert_eq!(paused["literal"], "rejected");
    assert!(ts(&resumed) >= paused["resets_at"].as_u64().unwrap());
    assert_eq!(outcomes(&fx, &dir, "fix"), ["rate_limited", "done"]);
    assert_eq!(outcomes(&fx, &dir, "checkpoint"), ["done", "done"]);
    assert_all_done(&fx);
    let log = driver_log(&dir);
    assert!(
        log.contains("fix worker") && log.contains("hit a usage limit"),
        "{log}"
    );
}

#[test]
fn a_spent_budget_parks_the_run_and_a_higher_budget_resumes_it() {
    let fx = Fixture::with_index("quota-budget", &[("checkpoint_every", "10")]);
    fx.chain();
    let clk = Clock::new(1);
    let out = factory(&fx, &clk, &["run", "001", "--budget", "1"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    let dir = only_run_dir(&fx);

    // One worker runs (the budget is checked before each spawn, and the sum
    // is 0 until a row lands); its usage passes the budget; nothing else
    // spawns and the driver exits parked.
    wait_phase(&fx, &dir, "waiting-owner", 60);
    let pid = state(&dir)["pid"].as_u64().unwrap() as u32;
    wait_until(&fx, &dir, "the parked driver to exit", 10, || {
        !pid_alive(pid)
    });
    let parked = of_kind(&dir, "run_parked");
    assert_eq!(parked.len(), 1, "{parked:?}");
    assert_eq!(parked[0]["reason"], "budget");
    assert_eq!(parked[0]["weighted"], json!(STUB_WEIGHTED));
    assert_eq!(parked[0]["budget"], 1);
    let k = kinds(&dir);
    assert_eq!(count(&k, "slice_spawned"), 1, "{k:?}");
    assert_eq!(count(&k, "run_paused"), 0, "{k:?}");
    assert_eq!(fx.calls().len(), 1);
    assert_eq!(fx.slice_status(1), "done");
    assert_eq!(fx.slice_status(2), "pending");
    // `weighted` is the hand-computed sum over the workers rows.
    let by_hand: f64 = db(&fx)
        .workers(&run_id(&dir))
        .unwrap()
        .iter()
        .map(|w| {
            w.input.unwrap() as f64
                + w.cache_write.unwrap() as f64
                + 0.1 * w.cache_read.unwrap() as f64
                + 5.0 * w.output.unwrap() as f64
        })
        .sum();
    assert_eq!(by_hand.round() as u64, STUB_WEIGHTED);
    assert_eq!(state(&dir)["budget_weighted_tokens"], 1);

    // A higher budget resumes the same run and it finishes.
    let out = factory(&fx, &clk, &["run", "001", "--budget", "100000000"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    assert_eq!(run_dirs(&fx).len(), 1);
    wait_phase(&fx, &dir, "finished", 60);
    let k = kinds(&dir);
    assert_eq!(count(&k, "run_resumed"), 1, "{k:?}");
    assert_eq!(count(&k, "run_parked"), 1, "{k:?}");
    assert_eq!(count(&k, "slice_spawned"), 3, "{k:?}");
    // Three build calls, the closing checkpoint's worker and the polish worker.
    assert_eq!(fx.calls().len(), 5);
    assert_all_done(&fx);
    assert_eq!(state(&dir)["budget_weighted_tokens"], 100_000_000);
    assert_eq!(outcomes(&fx, &dir, "build"), ["done", "done", "done"]);
}

#[test]
fn a_resumed_driver_holds_on_an_orphans_rate_limit() {
    let fx = Fixture::with_index("quota-orphan-limit", &[("checkpoint_every", "10")]);
    fx.chain();
    fx.write_plan(&[
        json!({"outcome": "done", "delay_ms": 30000}),
        done(),
        done(),
        done(),
    ]);
    let clk = Clock::new(1);
    let dir = start(&fx, &clk);
    let driver = state(&dir)["pid"].as_u64().unwrap() as u32;
    wait_until(&fx, &dir, "slice 01's worker", 20, || {
        state(&dir)["workers"]
            .as_array()
            .is_some_and(|w| w.len() == 1)
    });
    let w = state(&dir)["workers"][0].clone();
    let worker = w["pid"].as_u64().unwrap() as u32;
    let session = w["session_id"].as_str().unwrap();
    // Driver and worker gone; the worker's last word was a usage limit.
    assert_eq!(unsafe { libc::kill(driver as i32, libc::SIGKILL) }, 0);
    assert_eq!(unsafe { libc::kill(worker as i32, libc::SIGKILL) }, 0);
    wait_until(&fx, &dir, "both to die", 10, || {
        !pid_alive(driver) && !pid_alive(worker)
    });
    let stream = dir.join("workers").join(format!("{session}.jsonl"));
    let mut text = fs::read_to_string(&stream).unwrap_or_default();
    text.push_str(&format!(
        "{}\n",
        json!({"type": "rate_limit_event", "session_id": session,
            "rate_limit_info": {"status": "rejected", "resetsAt": clock::wall_now() + 600}})
    ));
    fs::write(&stream, text).unwrap();

    // The resumed driver settles the orphan, which pauses the run: nothing
    // spawns until the reset.
    let out = factory(&fx, &clk, &["run", "001"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    wait_phase(&fx, &dir, "paused", 10);
    assert_eq!(state(&dir)["workers"][0]["outcome"], "rate_limited");
    std::thread::sleep(Duration::from_secs(2));
    let spawned = || state(&dir)["workers"].as_array().unwrap().len();
    assert_eq!(spawned(), 1, "{}", driver_log(&dir));
    assert_eq!(fx.slice_status(1), "pending");
    assert!(state(&dir)["paused_until"].as_u64().is_some());

    let out = factory(&fx, &clk, &["stop", &run_id(&dir)]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    wait_phase(&fx, &dir, "finished", 20);
    assert_eq!(spawned(), 1);
}
