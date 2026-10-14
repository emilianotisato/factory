//! Slice 04 of PRD-001: lanes and parking (PRD §5 FR-3, §3 FR-1). Two lanes
//! build two independent slices at once; a blocked slice parks its lane while
//! the rest of the plan goes on; with nothing left to spawn the run parks in
//! `waiting-owner` and the driver stays alive; `factory answer` is the owner's
//! one command back — `BLOCK-n` respawns the slice with the answer in its
//! pack, the last `ASK-n` sends a parked run through branch and board.
//! PRD-005 slice 03 adds lane worktrees: `factory lanes up`, and the run
//! that refuses without `[lanes] seed` and makes its lanes before it starts.
#[path = "harness/mod.rs"]
mod harness;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Duration, Instant};

use factory::db::Db;
use factory::run::pid_alive;
use harness::Fixture;
use serde_json::{Value, json};

const BIN: &str = env!("CARGO_BIN_EXE_factory");
/// The fixture feature's log: blocks, answers, asks and decisions.
const LOG: &str = "docs/features/001-fixture/log.jsonl";
const QUESTION: &str = "Which storage backs the fixture?";
const ANSWER: &str = "Pick the Recommended option";

fn factory(fx: &Fixture, args: &[&str]) -> Output {
    fx.command(BIN).args(args).output().unwrap()
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
        .unwrap()
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

fn db(fx: &Fixture) -> Db {
    Db::open(&data_dir(fx).join("factory.db")).unwrap()
}

fn done() -> Value {
    json!({"outcome": "done"})
}

/// A checkpoint worker's step (slice 06): writes `CP-n` and returns done.
fn checkpoint() -> Value {
    json!({"outcome": "checkpoint"})
}

/// The build calls among the stub's: the checkpoint worker has no slice.
fn builds(calls: &[Value]) -> Vec<&Value> {
    calls
        .iter()
        .filter(|c| {
            c["prompt"]
                .as_str()
                .is_some_and(|p| p.starts_with("Build slice "))
        })
        .collect()
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
                "timed out after {secs}s waiting for {what}\n--- state.json\n{}\n--- events\n{:?}\n--- slices {statuses:?}\n--- calls {:?}\n--- driver.log\n{}",
                state(dir),
                kinds(dir),
                fx.calls()
                    .iter()
                    .map(|c| (c["n"].clone(), c["env"]["FACTORY_LANE"].clone()))
                    .collect::<Vec<_>>(),
                driver_log(dir)
            );
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn driver_log(dir: &Path) -> String {
    fs::read_to_string(dir.join("driver.log")).unwrap_or_default()
}

/// The slice a stub call built, from its `-p` prompt.
fn call_slice(call: &Value) -> u32 {
    let p = call["prompt"].as_str().unwrap();
    p.strip_prefix("Build slice ").unwrap()[..2]
        .parse()
        .unwrap()
}

fn lane_env(call: &Value) -> Option<&str> {
    call["env"]["FACTORY_LANE"].as_str()
}

/// `slices.N.lane` in the feature's `state.toml`.
fn slice_lane(fx: &Fixture, n: u8) -> Option<u32> {
    fx.slice_lane(n)
}

/// The raw `log.jsonl` line of the entry `id`.
fn block_line(log: &str, id: &str) -> String {
    log.lines()
        .find(|l| {
            serde_json::from_str::<Value>(l).is_ok_and(|e| e["id"] == id && e["kind"] == "block")
        })
        .unwrap_or_else(|| panic!("no {id} in {log}"))
        .to_string()
}

/// The `answer` entries of the feature dir's `log.jsonl` whose `ref` is `id`.
fn answers_to(feature_dir: &Path, id: &str) -> Vec<Value> {
    fs::read_to_string(feature_dir.join("log.jsonl"))
        .unwrap_or_default()
        .lines()
        .map(|l| serde_json::from_str::<Value>(l).expect(l))
        .filter(|e| e["kind"] == "answer" && e["ref"] == id)
        .collect()
}

/// `seq` of the first `kind` event on `slice`.
fn seq_of(dir: &Path, kind: &str, slice: u64) -> u64 {
    events(dir)
        .iter()
        .find(|e| e["kind"] == kind && e["slice"] == slice)
        .map(|e| e["seq"].as_u64().unwrap())
        .unwrap_or_else(|| panic!("no {kind} for slice {slice}"))
}

fn seq_first(dir: &Path, kind: &str) -> u64 {
    events(dir)
        .iter()
        .find(|e| e["kind"] == kind)
        .map(|e| e["seq"].as_u64().unwrap())
        .unwrap_or_else(|| panic!("no {kind}"))
}

/// Seconds into the day of an event's `ts` (`YYYY-MM-DDTHH:MM:SSZ`).
fn ts_secs(e: &Value) -> i64 {
    let ts = e["ts"].as_str().unwrap();
    let t = &ts[11..19];
    let h: i64 = t[0..2].parse().unwrap();
    let m: i64 = t[3..5].parse().unwrap();
    let s: i64 = t[6..8].parse().unwrap();
    h * 3600 + m * 60 + s
}

// ── task 1: two lanes ────────────────────────────────────────────────────────

#[test]
fn two_lanes_build_two_slices_at_once_and_the_third_waits_for_its_dependency() {
    let fx = Fixture::with_index("lanes-two", &[("checkpoint_every", "10")]);
    // Lane files as `docs lanes` writes them: an env list and a setup command
    // the driver runs once per lane per run.
    let lanes = fx.path().join("docs/reference/lanes");
    fs::create_dir_all(&lanes).unwrap();
    for n in 1..=2 {
        fs::write(
            lanes.join(format!("lane-{n}.md")),
            format!(
                "---\nlane: {n}\nenv: [FACTORY_LANE_DB=fixture_{n}]\nsetup: echo \"setup $FACTORY_LANE $FACTORY_LANE_DB\" >> \"$(dirname \"$(git rev-parse --path-format=absolute --git-common-dir)\")\"/scenario/lane-setup.log\n---\n\nLane {n} of the fixture.\n"
            ),
        )
        .unwrap();
    }
    fx.git(&["add", "-A"]);
    fx.git(&["commit", "-q", "-m", "chore: lane files"]);
    fx.write_plan(&[
        json!({"outcome": "done", "delay_ms": 1500}),
        json!({"outcome": "done", "delay_ms": 1500}),
        done(),
        checkpoint(),
    ]);
    let out = factory(&fx, &["run", "001"]);
    let text = stdout(&out);
    assert_eq!(out.status.code(), Some(0), "{text}");
    let dir = only_run_dir(&fx);
    let id = run_id(&dir);
    let driver = state(&dir)["pid"].as_u64().unwrap() as u32;

    // Both ready slices get a worker in the same tick, one lane each. The
    // stubs sleep 1.5 s, so the two are alive together while 03 waits on 02.
    wait_until(&fx, &dir, "two workers", 20, || {
        fx.calls().len() == 2
            && state(&dir)["workers"]
                .as_array()
                .is_some_and(|w| w.len() == 2)
    });
    let ws = state(&dir)["workers"].as_array().unwrap().clone();
    for w in &ws {
        assert!(pid_alive(w["pid"].as_u64().unwrap() as u32), "{w}");
    }
    assert_eq!(ws[0]["slice"], 1);
    assert_eq!(ws[1]["slice"], 2);
    assert_eq!(fx.slice_status(1), "building");
    assert_eq!(fx.slice_status(2), "building");
    assert_eq!(fx.slice_status(3), "pending");
    assert_eq!(slice_lane(&fx, 1), Some(1));
    assert_eq!(slice_lane(&fx, 2), Some(2));
    assert_eq!(slice_lane(&fx, 3), None);
    let board = fx.docs_json("status");
    assert_eq!(board["freeLanes"], json!([]), "{board}");
    assert_eq!(board["spawnable"], 0, "{board}");

    wait_until(&fx, &dir, "the run to finish", 60, || {
        state(&dir)["phase"] == "finished"
    });
    wait_until(&fx, &dir, "the driver to exit", 5, || !pid_alive(driver));
    for n in 1..=3 {
        assert_eq!(fx.slice_status(n), "done", "slice {n:02}");
    }
    let calls = fx.calls();
    assert_eq!(
        calls.len(),
        5,
        "three builds, a checkpoint and the polish worker: {calls:?}"
    );
    let builds = builds(&calls);
    assert_eq!(
        builds.iter().map(|c| call_slice(c)).collect::<Vec<_>>(),
        [1, 2, 3]
    );
    // Every worker saw its lane: `FACTORY_LANE` and the lane file's env.
    for call in &builds {
        let n = call_slice(call);
        let lane = slice_lane(&fx, n as u8).unwrap();
        assert_eq!(lane_env(call), Some(lane.to_string().as_str()), "{call}");
        assert_eq!(
            call["env"]["FACTORY_LANE_DB"],
            format!("fixture_{lane}"),
            "{call}"
        );
    }
    assert_eq!(slice_lane(&fx, 1), Some(1));
    assert_eq!(slice_lane(&fx, 2), Some(2));
    assert!(
        matches!(slice_lane(&fx, 3), Some(1 | 2)),
        "{:?}",
        slice_lane(&fx, 3)
    );
    // `setup:` once per lane per run, with the lane's env.
    let setup = fx.read("scenario/lane-setup.log");
    let mut lines: Vec<&str> = setup.lines().collect();
    lines.sort_unstable();
    assert_eq!(lines, ["setup 1 fixture_1", "setup 2 fixture_2"]);

    let evs = events(&dir);
    let spawned: Vec<&Value> = evs
        .iter()
        .filter(|e| e["kind"] == "slice_spawned")
        .collect();
    assert_eq!(spawned.len(), 3);
    assert_eq!(spawned[0]["slice"], 1);
    assert_eq!(spawned[1]["slice"], 2);
    assert_eq!(spawned[2]["slice"], 3);
    let gap = (ts_secs(spawned[0]) - ts_secs(spawned[1])).abs();
    assert!(
        gap <= 1,
        "01 and 02 spawned {gap}s apart: {} {}",
        spawned[0]["ts"],
        spawned[1]["ts"]
    );
    assert!(
        seq_of(&dir, "slice_done", 2) < seq_of(&dir, "slice_spawned", 3),
        "03 spawns only after 02 is done: {:?}",
        kinds(&dir)
    );
    let k = kinds(&dir);
    assert_eq!(count(&k, "slice_done"), 3);
    assert_eq!(count(&k, "worker_dead"), 0);
    assert_eq!(k.last().map(String::as_str), Some("run_finished"));

    let log = driver_log(&dir);
    assert!(log.contains("lane 1 setup ran: echo"), "{log}");
    assert!(log.contains("lane 2 setup ran: echo"), "{log}");
    assert!(log.contains("), lane 1, pack"), "{log}");
    assert!(log.contains("), lane 2, pack"), "{log}");
    let rows = db(&fx).workers(&id).unwrap();
    assert_eq!(rows.len(), 5, "three builds, the checkpoint, the polish");
    assert!(
        rows.iter().all(|r| r.outcome.as_deref() == Some("done")),
        "{rows:?}"
    );
}

// ── tasks 2 and 3: a blocked slice, the parked run, `factory answer BLOCK-n` ──

#[test]
fn a_blocked_slice_parks_its_lane_and_the_answer_respawns_it() {
    let fx = Fixture::with_index("lanes-block", &[("checkpoint_every", "10")]);
    fx.write_plan(&[
        json!({"outcome": "blocked", "question": QUESTION}),
        done(),
        done(),
        done(),
        checkpoint(),
    ]);
    let out = factory(&fx, &["run", "001"]);
    let text = stdout(&out);
    assert_eq!(out.status.code(), Some(0), "{text}");
    let dir = only_run_dir(&fx);
    let id = run_id(&dir);
    let driver = state(&dir)["pid"].as_u64().unwrap() as u32;

    // 01 blocks on lane 1; 02 and then 03 build on lane 2 — the blocked
    // slice keeps its lane. Nothing left to spawn: the run parks, alive.
    wait_until(&fx, &dir, "the run to park", 60, || {
        state(&dir)["phase"] == "waiting-owner"
    });
    assert!(pid_alive(driver), "the parked driver stays alive");
    assert_eq!(fx.slice_status(1), "blocked");
    assert_eq!(fx.slice_status(2), "done");
    assert_eq!(fx.slice_status(3), "done");
    assert_eq!(slice_lane(&fx, 1), Some(1));
    assert_eq!(slice_lane(&fx, 2), Some(2));
    assert_eq!(slice_lane(&fx, 3), Some(2));
    let board = fx.docs_json("status");
    assert_eq!(board["blocked"], json!([1]), "{board}");
    assert_eq!(board["building"], json!([]), "{board}");
    let calls = fx.calls();
    assert_eq!(calls.len(), 3, "{calls:?}");
    assert_eq!(calls.iter().map(call_slice).collect::<Vec<_>>(), [1, 2, 3]);
    assert_eq!(
        calls.iter().map(lane_env).collect::<Vec<_>>(),
        [Some("1"), Some("2"), Some("2")]
    );
    let k = kinds(&dir);
    assert_eq!(count(&k, "slice_spawned"), 3, "{k:?}");
    assert_eq!(count(&k, "slice_done"), 2, "{k:?}");
    assert_eq!(count(&k, "slice_blocked"), 1, "{k:?}");
    assert_eq!(count(&k, "worker_dead"), 0, "{k:?}");
    assert_eq!(k.last().map(String::as_str), Some("run_waiting"), "{k:?}");
    let blocked_ev = events(&dir)
        .into_iter()
        .find(|e| e["kind"] == "slice_blocked")
        .unwrap();
    assert_eq!(blocked_ev["slice"], 1);
    assert_eq!(blocked_ev["block_id"], "BLOCK-1");
    assert_eq!(blocked_ev["question"], QUESTION);
    let waiting = events(&dir)
        .into_iter()
        .find(|e| e["kind"] == "run_waiting")
        .unwrap();
    assert_eq!(waiting["blocked"], json!([1]));
    assert_eq!(waiting["asks"], json!([]));
    let blocks = fx.log_of("block");
    assert_eq!(blocks.len(), 1, "{blocks:?}");
    assert_eq!(blocks[0]["id"], "BLOCK-1");
    assert_eq!(blocks[0]["slice"], 1);
    assert_eq!(blocks[0]["title"], QUESTION);
    assert!(fx.log_of("answer").is_empty(), "the block is open");
    let block = block_line(&fx.read(LOG), "BLOCK-1");
    let log = driver_log(&dir);
    assert!(
        log.contains(&format!(
            "slice 01 blocked by its worker (BLOCK-1: {QUESTION}) — lane 1 parked"
        )),
        "{log}"
    );
    assert!(
        log.contains("parked (waiting-owner): slice 01 blocked"),
        "{log}"
    );
    let rows = db(&fx).workers(&id).unwrap();
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].outcome.as_deref(), Some("blocked"), "{rows:?}");
    assert_eq!(
        db(&fx).run_row(&id).unwrap().unwrap().phase,
        "waiting-owner"
    );

    // The owner answers: an `answer` entry points at the block, whose line
    // stays as written; the slice is `building` again on its lane, the
    // answer is on disk for the driver.
    let out = factory(&fx, &["answer", "BLOCK-1", ANSWER]);
    let text = stdout(&out);
    assert_eq!(out.status.code(), Some(0), "{text}");
    assert!(
        text.contains(&format!("BLOCK-1 resolved ({LOG} line ")),
        "{text}"
    );
    assert!(text.contains("slice 01 → building"), "{text}");
    assert!(
        text.contains(&format!("the driver (pid {driver}) picks it up within 5 s")),
        "{text}"
    );
    let answers = answers_to(&fx.path().join(harness::FEATURE_DIR), "BLOCK-1");
    assert_eq!(answers.len(), 1, "{answers:?}");
    assert_eq!(answers[0]["body"], ANSWER);
    assert_eq!(answers[0]["slice"], 1);
    assert_eq!(
        block_line(&fx.read(LOG), "BLOCK-1"),
        block,
        "the block line stays"
    );
    assert_eq!(fx.slice_status(1), "building");
    assert_eq!(slice_lane(&fx, 1), Some(1));
    let answer: Value =
        serde_json::from_str(&fs::read_to_string(dir.join("answers/BLOCK-1.json")).unwrap())
            .unwrap();
    assert_eq!(answer["id"], "BLOCK-1");
    assert_eq!(answer["slice"], 1);
    assert_eq!(answer["text"], ANSWER);
    assert!(
        answer["at"].as_str().is_some_and(|t| t.ends_with('Z')),
        "{answer}"
    );
    let answered = events(&dir)
        .into_iter()
        .find(|e| e["kind"] == "block_answered")
        .unwrap();
    assert_eq!(answered["id"], "BLOCK-1");
    assert_eq!(answered["slice"], 1);
    assert_eq!(answered["text"], ANSWER);

    // Within the slow tick a fresh worker takes slice 01 on lane 1, with the
    // block and the answer in its pack; the run finishes to the checkpoint.
    wait_until(&fx, &dir, "the answered slice to respawn", 10, || {
        fx.calls().len() == 4
    });
    let calls = fx.calls();
    assert_eq!(call_slice(&calls[3]), 1);
    assert_eq!(lane_env(&calls[3]), Some("1"));
    assert_ne!(
        calls[3]["session_id"], calls[0]["session_id"],
        "a respawn is a new session"
    );
    let pack = calls[3]["pack"].as_str().unwrap();
    let i_slice = pack.find("# Slice\n").unwrap();
    let i_block = pack.find("# Block and answer\n").unwrap();
    let i_prd = pack.find("# PRD sections\n").unwrap();
    assert!(i_slice < i_block && i_block < i_prd, "{pack}");
    let section = &pack[i_block..i_prd];
    assert!(
        section.contains(&format!("<!-- {LOG}: BLOCK-1 -->")),
        "{section}"
    );
    assert!(
        section.contains(&format!("### BLOCK-1 — {QUESTION} · slice-01 · resolved")),
        "{section}"
    );
    assert!(section.contains("**Happened:**"), "{section}");
    assert!(section.contains(&format!("):** {ANSWER}")), "{section}");
    assert!(
        !calls[0]["pack"]
            .as_str()
            .unwrap()
            .contains("# Block and answer")
    );

    wait_until(&fx, &dir, "the run to finish", 60, || {
        state(&dir)["phase"] == "finished"
    });
    wait_until(&fx, &dir, "the driver to exit", 5, || !pid_alive(driver));
    for n in 1..=3 {
        assert_eq!(fx.slice_status(n), "done", "slice {n:02}");
    }
    assert_eq!(fx.calls().len(), 6);
    let k = kinds(&dir);
    assert_eq!(count(&k, "slice_spawned"), 4, "{k:?}");
    assert_eq!(count(&k, "slice_done"), 3, "{k:?}");
    assert_eq!(count(&k, "worker_dead"), 0, "{k:?}");
    assert_eq!(count(&k, "run_waiting"), 1, "{k:?}");
    assert_eq!(count(&k, "block_answered"), 1, "{k:?}");
    assert_eq!(k.last().map(String::as_str), Some("run_finished"), "{k:?}");
    let answered = seq_first(&dir, "block_answered");
    assert!(seq_of(&dir, "slice_blocked", 1) < seq_first(&dir, "run_waiting"));
    assert!(seq_first(&dir, "run_waiting") < answered);
    assert!(seq_of(&dir, "slice_done", 2) < answered && seq_of(&dir, "slice_done", 3) < answered);
    let respawn = events(&dir)
        .iter()
        .filter(|e| e["kind"] == "slice_spawned" && e["slice"] == 1)
        .map(|e| e["seq"].as_u64().unwrap())
        .max()
        .unwrap();
    assert!(answered < respawn, "{k:?}");
    assert!(respawn < seq_of(&dir, "slice_done", 1), "{k:?}");
    let ws = state(&dir)["workers"].as_array().unwrap().clone();
    assert_eq!(ws.len(), 6);
    assert_eq!(ws[0]["outcome"], "blocked");
    assert!(ws[1..].iter().all(|w| w["outcome"] == "done"), "{ws:?}");
    let rows = db(&fx).workers(&id).unwrap();
    assert_eq!(rows.len(), 6);
    assert_eq!(rows[3].slice, Some(1));
    assert_eq!(rows[3].outcome.as_deref(), Some("done"));
    assert_eq!(db(&fx).run_row(&id).unwrap().unwrap().phase, "finished");
    let log = driver_log(&dir);
    assert!(log.contains("lane 1, answered, pack"), "{log}");
    // The run reached the checkpoint: the done workers' ADs are in the log.
    assert!(
        !fx.log_of("decision").is_empty(),
        "the done worker's AD lands too: {:?}",
        fx.log_entries()
    );

    // Answering a resolved block is refused.
    let out = factory(&fx, &["answer", "BLOCK-1", "again"]);
    assert_eq!(out.status.code(), Some(1), "{}", stdout(&out));
    assert!(
        stdout(&out).contains("BLOCK-1 is already resolved"),
        "{}",
        stdout(&out)
    );
}

#[test]
fn a_torn_answer_file_is_said_and_skipped_and_the_run_goes_on() {
    let fx = Fixture::with_index("lanes-torn-answer", &[("checkpoint_every", "10")]);
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
    wait_until(&fx, &dir, "the run to park", 60, || {
        state(&dir)["phase"] == "waiting-owner"
    });
    // A file no `factory answer` finished writing.
    fs::create_dir_all(dir.join("answers")).unwrap();
    fs::write(dir.join("answers/ASK-7.json"), "{\"id\": \"ASK-7\", \"te").unwrap();

    let out = factory(&fx, &["answer", "BLOCK-1", ANSWER]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    wait_until(&fx, &dir, "the run to end", 60, || {
        state(&dir)["phase"] == "finished" || state(&dir)["phase"] == "failed"
    });
    assert_eq!(state(&dir)["phase"], "finished", "{}", driver_log(&dir));
    assert_eq!(
        call_slice(&fx.calls()[3]),
        1,
        "the answered slice respawned"
    );
    let log = driver_log(&dir);
    assert_eq!(
        log.matches("answers/ASK-7.json is not an answer (").count(),
        1,
        "said once: {log}"
    );
    assert!(
        log.contains("is not an answer (") && log.contains(") — skipped"),
        "{log}"
    );
    assert!(!dir.join("answers/.BLOCK-1.json.tmp").exists());
}

// ── task 4: `factory answer ASK-n` ───────────────────────────────────────────

#[test]
fn answering_every_ask_starts_the_parked_run() {
    let fx = Fixture::with_index("lanes-ask", &[("checkpoint_every", "10")]);
    // No `inner` in `[docs]`: preflight asks about the missing inner loop
    // before the run starts.
    let index = fx.path().join("docs/project.toml");
    let text = fs::read_to_string(&index).unwrap();
    assert!(text.contains("\ninner = "), "{text}");
    let without: String = text
        .lines()
        .filter(|l| !l.starts_with("inner ="))
        .map(|l| format!("{l}\n"))
        .collect();
    fs::write(&index, without).unwrap();
    fx.git(&["add", "-A"]);
    fx.git(&["commit", "-q", "-m", "chore: no inner loop"]);
    fx.write_plan(&[done(), done(), done(), checkpoint()]);

    let out = factory(&fx, &["run", "001"]);
    let text = stdout(&out);
    assert_eq!(out.status.code(), Some(0), "{text}");
    assert!(text.contains("ASK-1: "), "{text}");
    let dir = only_run_dir(&fx);
    let id = run_id(&dir);
    assert_eq!(state(&dir)["phase"], "waiting-owner");
    assert_eq!(state(&dir)["branch"], Value::Null);
    assert!(dir.join("preflight.json").is_file());
    let asks: Vec<String> = events(&dir)
        .iter()
        .filter(|e| e["kind"] == "ask_opened")
        .map(|e| e["id"].as_str().unwrap().to_string())
        .collect();
    assert!(!asks.is_empty());
    assert_eq!(asks[0], "ASK-1");
    assert!(fx.calls().is_empty());
    assert_eq!(
        fx.git(&["rev-parse", "--abbrev-ref", "HEAD"]).trim(),
        "main"
    );

    // An unknown ask is refused; every real one takes "proceed".
    let out = factory(&fx, &["answer", "ASK-9", "proceed"]);
    assert_eq!(out.status.code(), Some(1), "{}", stdout(&out));
    assert!(
        stdout(&out).contains("no open run of this repo waits on ASK-9"),
        "{}",
        stdout(&out)
    );
    for (i, ask) in asks.iter().enumerate() {
        let out = factory(&fx, &["answer", ask, "proceed"]);
        let text = stdout(&out);
        assert_eq!(out.status.code(), Some(0), "{text}");
        assert!(
            text.contains(&format!("{ask} answered: proceed — noted in {LOG}")),
            "{text}"
        );
        assert!(dir.join("answers").join(format!("{ask}.json")).is_file());
        if i + 1 < asks.len() {
            assert!(text.contains("still open"), "{text}");
            assert!(!text.contains("driver pid"), "{text}");
        } else {
            assert!(
                text.contains("every ask answered — the run proceeds"),
                "{text}"
            );
            assert!(text.contains("driver pid "), "{text}");
        }
    }
    // One `ask` entry per ask, phase `preflight`, on the AD sequence ahead
    // of any build's decision.
    let logged = fx.log_of("ask");
    assert_eq!(logged.len(), asks.len(), "{logged:?}");
    for (i, (ask, e)) in asks.iter().zip(&logged).enumerate() {
        assert_eq!(e["id"], format!("AD-{}", i + 1), "{e}");
        assert_eq!(e["phase"], "preflight", "{e}");
        assert!(
            e["title"]
                .as_str()
                .is_some_and(|t| t.starts_with(&format!("Preflight {ask}: "))),
            "{e}"
        );
        assert!(
            e["body"]
                .as_str()
                .is_some_and(|b| b.lines().any(|l| l == "**Chose:** proceed")),
            "{e}"
        );
    }

    // The driver takes the run through branch and board, then builds.
    wait_until(&fx, &dir, "the run to finish", 60, || {
        state(&dir)["phase"] == "finished"
    });
    let driver = state(&dir)["pid"].as_u64().unwrap() as u32;
    wait_until(&fx, &dir, "the driver to exit", 5, || !pid_alive(driver));
    assert_eq!(
        fx.git(&["rev-parse", "--abbrev-ref", "HEAD"]).trim(),
        "feat/fixture"
    );
    assert_eq!(state(&dir)["branch"], "feat/fixture");
    for n in 1..=3 {
        assert_eq!(fx.slice_status(n), "done", "slice {n:02}");
    }
    assert_eq!(fx.calls().len(), 5);
    assert_eq!(run_dirs(&fx).len(), 1);
    let k = kinds(&dir);
    assert_eq!(count(&k, "ask_opened"), asks.len(), "{k:?}");
    assert_eq!(count(&k, "ask_answered"), asks.len(), "{k:?}");
    assert_eq!(count(&k, "run_started"), 1, "{k:?}");
    assert_eq!(
        count(&k, "run_waiting"),
        0,
        "an ask-parked run was never re-parked: {k:?}"
    );
    assert_eq!(count(&k, "slice_spawned"), 3, "{k:?}");
    assert_eq!(count(&k, "slice_done"), 3, "{k:?}");
    assert_eq!(k.last().map(String::as_str), Some("run_finished"), "{k:?}");
    let last_answer = k.iter().rposition(|k| k == "ask_answered").unwrap();
    let started = k.iter().position(|k| k == "run_started").unwrap();
    assert!(last_answer < started, "{k:?}");
    let answered_ev = events(&dir)
        .into_iter()
        .find(|e| e["kind"] == "ask_answered")
        .unwrap();
    assert_eq!(answered_ev["id"], "ASK-1");
    assert_eq!(answered_ev["text"], "proceed");
    let row = db(&fx).run_row(&id).unwrap().unwrap();
    assert_eq!(row.phase, "finished");
    assert_eq!(row.branch.as_deref(), Some("feat/fixture"));
    let log = driver_log(&dir);
    assert!(
        log.contains("every ask answered — the run proceeds"),
        "{log}"
    );
    assert!(
        log.contains(&format!("run {id} started on feat/fixture")),
        "{log}"
    );
}

// ── polish: which block `factory answer BLOCK-n` resolves ────────────────────

/// A `block` entry titled "Which store?" on `slice` in the feature dir's
/// `log.jsonl`, as a blocked worker writes it; its id.
fn seed_block(feature_dir: &Path, slice: u32) -> String {
    use factory::docs::log::{Entry, Kind, add};
    let mut e = Entry::new(Kind::Block, "Which store?", "**Happened:** x\n");
    e.slice = Some(slice);
    add(feature_dir, e).unwrap().unwrap()
}

/// An `answer` entry pointing at `id`, as `factory answer` writes it.
fn seed_answer(feature_dir: &Path, id: &str, slice: u32) {
    use factory::docs::log::{Entry, Kind, add};
    let mut e = Entry::new(Kind::Answer, "", "use postgres");
    e.slice = Some(slice);
    e.reference = Some(id.to_string());
    add(feature_dir, e).unwrap();
}

fn log_text(feature_dir: &Path) -> String {
    fs::read_to_string(feature_dir.join("log.jsonl")).unwrap()
}

/// A run of feature 001 parked in `waiting-owner`, its pid this test's.
fn parked_run(fx: &Fixture) -> factory::run::Run {
    let mut run = factory::run::Run::create(
        &data_dir(fx),
        fx.path(),
        harness::FEATURE,
        Some("feat/fixture".into()),
        0,
    )
    .unwrap();
    run.set_phase(factory::run::Phase::WaitingOwner).unwrap();
    run
}

#[test]
fn answer_takes_the_open_block_and_prefers_the_parked_feature() {
    let fx = Fixture::new("answer-pick");
    let one = fx.path().join(harness::FEATURE_DIR);
    // A second feature with its own slice 01: block ids restart per feature,
    // so both can hold a BLOCK-1.
    fx.docs(&["new", "other", "--title", "Other"]);
    fx.docs(&["slice", "002", "x"]);
    let two = fx.path().join("docs/features/002-other");
    fx.docs(&["set", "002", "slices.1.status=blocked"]);
    let status_two = || {
        factory::docs::state::read(&two).unwrap().unwrap().slices[&1]
            .status
            .clone()
    };
    let reseed = |open_in_one: bool| {
        for d in [&one, &two] {
            let _ = fs::remove_file(d.join("log.jsonl"));
            assert_eq!(seed_block(d, 1), "BLOCK-1");
        }
        if !open_in_one {
            seed_answer(&one, "BLOCK-1", 1);
        }
    };

    // 001's BLOCK-1 is resolved, 002's is open: the open one is answered,
    // not refused as "already resolved" on the first folder's match.
    reseed(false);
    let one_before = log_text(&one);
    let two_before = log_text(&two);
    let out = factory(&fx, &["answer", "BLOCK-1", "use sqlite"]);
    let text = stdout(&out);
    assert_eq!(out.status.code(), Some(0), "{text}");
    assert!(
        text.contains("BLOCK-1 resolved (docs/features/002-other/log.jsonl line 1)"),
        "{text}"
    );
    assert!(text.contains("no open run for PRD-002"), "{text}");
    let answered = answers_to(&two, "BLOCK-1");
    assert_eq!(answered.len(), 1, "{answered:?}");
    assert_eq!(answered[0]["body"], "use sqlite");
    assert!(
        log_text(&two).starts_with(&two_before),
        "the block line stays"
    );
    assert_eq!(
        log_text(&one),
        one_before,
        "the resolved block is untouched"
    );
    assert_eq!(status_two(), "building");

    // Two open BLOCK-1s and no parked run: refused with both named,
    // nothing resolved.
    reseed(true);
    let one_before = log_text(&one);
    let two_before = log_text(&two);
    let out = factory(&fx, &["answer", "BLOCK-1", "use sqlite"]);
    let text = stdout(&out);
    assert_eq!(out.status.code(), Some(1), "{text}");
    assert!(text.contains("BLOCK-1 is open in 2 places"), "{text}");
    assert!(
        text.contains("docs/features/001-fixture/log.jsonl line 1 — Which store?"),
        "{text}"
    );
    assert!(
        text.contains("docs/features/002-other/log.jsonl line 1 — Which store?"),
        "{text}"
    );
    assert_eq!(log_text(&one), one_before);
    assert_eq!(log_text(&two), two_before);
    assert!(text.contains("--feature NNN"), "{text}");

    // `--feature` picks one of the two: 002's is answered, 001's untouched.
    let out = factory(&fx, &["answer", "--feature", "2", "BLOCK-1", "use sqlite"]);
    let text = stdout(&out);
    assert_eq!(out.status.code(), Some(0), "{text}");
    assert!(
        text.contains("BLOCK-1 resolved (docs/features/002-other/log.jsonl line 1)"),
        "{text}"
    );
    assert_eq!(log_text(&one), one_before);
    reseed(true);

    // 001's run is parked: its block is the one the owner means. The answer's
    // `ref` lands in 001's log and not in 002's.
    fx.set_slice_status(1, "blocked");
    let run = parked_run(&fx);
    let out = factory(&fx, &["answer", "BLOCK-1", "use sqlite"]);
    let text = stdout(&out);
    assert_eq!(out.status.code(), Some(0), "{text}");
    assert!(
        text.contains("BLOCK-1 resolved (docs/features/001-fixture/log.jsonl line 1)"),
        "{text}"
    );
    assert_eq!(answers_to(&one, "BLOCK-1").len(), 1);
    assert!(
        log_text(&one).starts_with(&one_before),
        "the block line stays"
    );
    assert!(answers_to(&two, "BLOCK-1").is_empty());
    assert_eq!(log_text(&two), two_before);
    assert_eq!(fx.slice_status(1), "building");
    assert!(run.dir().join("answers/BLOCK-1.json").is_file());
    assert_eq!(count(&kinds(run.dir()), "block_answered"), 1);
}

#[test]
fn block_answered_is_logged_before_the_slice_flips() {
    // The flip is what a live driver acts on; an event logged after it can
    // land behind the `slice_spawned` it caused. A read-only feature dir
    // makes the flip fail — `state.toml` lands by a rename from a temp
    // beside it (DEC-3) — while the `answer` entry is still appended to
    // `log.jsonl` in place: whatever was logged before the flip shows.
    let fx = Fixture::new("answer-order");
    let feature_dir = fx.path().join(harness::FEATURE_DIR);
    assert_eq!(seed_block(&feature_dir, 1), "BLOCK-1");
    fx.set_slice_status(1, "blocked");
    let run = parked_run(&fx);
    let mode = |m: u32| {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&feature_dir, fs::Permissions::from_mode(m)).unwrap();
    };
    mode(0o555);
    let out = factory(&fx, &["answer", "BLOCK-1", "use sqlite"]);
    mode(0o755);
    assert_ne!(out.status.code(), Some(0), "{}", stdout(&out));
    let answered: Vec<Value> = events(run.dir())
        .into_iter()
        .filter(|e| e["kind"] == "block_answered")
        .collect();
    assert_eq!(answered.len(), 1, "{:?}", kinds(run.dir()));
    assert_eq!(answered[0]["id"], "BLOCK-1");
    assert_eq!(answered[0]["slice"], 1);
    assert!(run.dir().join("answers/BLOCK-1.json").is_file());
    assert_eq!(fx.slice_status(1), "blocked");
}

#[test]
fn an_answer_that_stopped_before_the_flip_is_finished_by_a_retry() {
    let fx = Fixture::new("answer-retry");
    let feature_dir = fx.path().join(harness::FEATURE_DIR);
    assert_eq!(seed_block(&feature_dir, 1), "BLOCK-1");
    let block = block_line(&log_text(&feature_dir), "BLOCK-1");
    fx.set_slice_status(1, "blocked");
    let run = parked_run(&fx);
    // A read-only feature dir fails the flip (`state.toml`'s rename,
    // DEC-3), not the `answer` entry's in-place append.
    let mode = |m: u32| {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&feature_dir, fs::Permissions::from_mode(m)).unwrap();
    };
    mode(0o555);
    let out = factory(&fx, &["answer", "BLOCK-1", "use sqlite"]);
    mode(0o755);
    assert_ne!(out.status.code(), Some(0), "{}", stdout(&out));
    assert_eq!(answers_to(&feature_dir, "BLOCK-1").len(), 1);
    assert_eq!(fx.slice_status(1), "blocked");

    // The retry finds the block answered and the answer on file: it
    // finishes the flip instead of refusing an "already resolved" block.
    let out = factory(&fx, &["answer", "BLOCK-1", "use sqlite"]);
    let text = stdout(&out);
    assert_eq!(out.status.code(), Some(0), "{text}");
    assert!(text.contains("holds its answer — finishing it"), "{text}");
    assert!(text.contains("slice 01 → building"), "{text}");
    assert_eq!(fx.slice_status(1), "building");
    assert_eq!(
        answers_to(&feature_dir, "BLOCK-1").len(),
        1,
        "one answer entry"
    );
    assert_eq!(block_line(&log_text(&feature_dir), "BLOCK-1"), block);
    assert!(run.dir().join("answers/BLOCK-1.json").is_file());

    // Once delivered, the block is refused as before.
    let out = factory(&fx, &["answer", "BLOCK-1", "use sqlite"]);
    assert_eq!(out.status.code(), Some(1), "{}", stdout(&out));
    assert!(
        stdout(&out).contains("BLOCK-1 is already resolved"),
        "{}",
        stdout(&out)
    );
}

#[test]
fn a_block_on_a_missing_slice_file_writes_nothing() {
    let fx = Fixture::new("answer-no-slice");
    let feature_dir = fx.path().join(harness::FEATURE_DIR);
    // A block naming slice 04, which the fixture has no file for.
    assert_eq!(seed_block(&feature_dir, 4), "BLOCK-1");
    let before = log_text(&feature_dir);
    let run = parked_run(&fx);
    let out = factory(&fx, &["answer", "BLOCK-1", "use sqlite"]);
    assert_ne!(out.status.code(), Some(0), "{}", stdout(&out));
    assert_eq!(log_text(&feature_dir), before);
    assert!(!run.dir().join("answers/BLOCK-1.json").exists());
    assert_eq!(count(&kinds(run.dir()), "block_answered"), 0);
}

#[test]
fn an_answer_flips_only_a_blocked_slice() {
    let fx = Fixture::new("answer-done-slice");
    let feature_dir = fx.path().join(harness::FEATURE_DIR);
    assert_eq!(seed_block(&feature_dir, 1), "BLOCK-1");
    let block = block_line(&log_text(&feature_dir), "BLOCK-1");
    fx.set_slice_status(1, "done");
    let run = parked_run(&fx);
    let out = factory(&fx, &["answer", "BLOCK-1", "use sqlite"]);
    let text = stdout(&out);
    assert_eq!(out.status.code(), Some(0), "{text}");
    assert!(text.contains("slice 01 is done: left as is"), "{text}");
    assert_eq!(fx.slice_status(1), "done");
    assert_eq!(answers_to(&feature_dir, "BLOCK-1").len(), 1);
    assert_eq!(block_line(&log_text(&feature_dir), "BLOCK-1"), block);
    assert!(run.dir().join("answers/BLOCK-1.json").is_file());
}

/// A run of `feature` parked on `ASK-1`, its pid this test's.
fn parked_on_ask(fx: &Fixture, feature: &str) -> factory::run::Run {
    let mut run = factory::run::Run::create(&data_dir(fx), fx.path(), feature, None, 1).unwrap();
    run.emit(factory::events::EventKind::AskOpened {
        id: "ASK-1".into(),
        text: format!("inner loop for {feature}?"),
    })
    .unwrap();
    run.set_phase(factory::run::Phase::WaitingOwner).unwrap();
    run
}

#[test]
fn an_ask_two_runs_wait_on_is_refused_by_name_and_run_picks_one() {
    let fx = Fixture::new("ask-two-runs");
    fs::create_dir_all(fx.path().join("docs/features/002-other")).unwrap();
    let one = parked_on_ask(&fx, "001");
    let two = parked_on_ask(&fx, "002");
    let out = factory(&fx, &["answer", "ASK-1", "proceed"]);
    let text = stdout(&out);
    assert_eq!(out.status.code(), Some(1), "{text}");
    assert!(
        text.contains("2 open runs wait on ASK-1 — nothing answered"),
        "{text}"
    );
    assert!(
        text.contains(&format!("{} (PRD-001) — inner loop for 001?", one.id())),
        "{text}"
    );
    assert!(
        text.contains(&format!("{} (PRD-002) — inner loop for 002?", two.id())),
        "{text}"
    );
    for run in [&one, &two] {
        assert!(!run.dir().join("answers/ASK-1.json").exists());
        assert_eq!(count(&kinds(run.dir()), "ask_answered"), 0);
    }

    let out = factory(
        &fx,
        &["answer", "ASK-1", "--run", one.id().as_str(), "proceed"],
    );
    let text = stdout(&out);
    assert_eq!(out.status.code(), Some(0), "{text}");
    assert!(text.contains("ASK-1 answered: proceed"), "{text}");
    assert!(one.dir().join("answers/ASK-1.json").is_file());
    assert!(!two.dir().join("answers/ASK-1.json").exists());

    // One run still waits: no ambiguity left.
    let out = factory(&fx, &["answer", "ASK-1", "proceed"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    assert!(two.dir().join("answers/ASK-1.json").is_file());
}

#[test]
fn an_ask_answer_that_failed_on_the_log_is_retried_whole() {
    let fx = Fixture::new("ask-retry");
    let run = parked_on_ask(&fx, "001");
    // A read-only `log.jsonl` fails the `ask` entry's append.
    let log = fx.path().join(LOG);
    fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log)
        .unwrap();
    let mode = |m: u32| {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&log, fs::Permissions::from_mode(m)).unwrap();
    };
    mode(0o444);
    let out = factory(&fx, &["answer", "ASK-1", "proceed"]);
    mode(0o644);
    assert_ne!(out.status.code(), Some(0), "{}", stdout(&out));
    // The answer file is the last step: the ask is still open.
    assert!(!run.dir().join("answers/ASK-1.json").exists());

    let out = factory(&fx, &["answer", "ASK-1", "proceed"]);
    let text = stdout(&out);
    assert_eq!(out.status.code(), Some(0), "{text}");
    assert!(
        text.contains("every ask answered — the run proceeds"),
        "{text}"
    );
    assert!(run.dir().join("answers/ASK-1.json").is_file());
    let asks = fx.log_of("ask");
    assert_eq!(asks.len(), 1, "{asks:?}");
    assert_eq!(asks[0]["phase"], "preflight");
    assert_eq!(count(&kinds(run.dir()), "ask_answered"), 1);
}

/// A pid no process holds: a child that already exited.
fn dead_pid() -> u32 {
    let mut child = std::process::Command::new("true").spawn().unwrap();
    let pid = child.id();
    child.wait().unwrap();
    pid
}

#[test]
fn an_answer_to_a_run_parked_on_its_budget_says_so_and_starts_no_driver() {
    let fx = Fixture::new("answer-budget");
    assert_eq!(
        seed_block(&fx.path().join(harness::FEATURE_DIR), 1),
        "BLOCK-1"
    );
    fx.set_slice_status(1, "blocked");
    // Budget 0 and a worker that spent 10 weighted tokens; no live driver.
    let run = parked_run(&fx);
    run.db()
        .conn()
        .execute(
            "INSERT INTO workers (run_id, role, started_at, input) VALUES (?1, 'build', 'x', 10)",
            [run.id().as_str()],
        )
        .unwrap();
    let mut s: Value =
        serde_json::from_str(&fs::read_to_string(run.dir().join("state.json")).unwrap()).unwrap();
    s["pid"] = json!(dead_pid());
    fs::write(run.dir().join("state.json"), s.to_string()).unwrap();
    let out = factory(&fx, &["answer", "BLOCK-1", "use sqlite"]);
    let text = stdout(&out);
    assert_eq!(out.status.code(), Some(0), "{text}");
    assert!(
        text.contains("is parked on its budget (10 weighted tokens, budget 0) — `factory run 001 --budget <higher>` resumes it"),
        "{text}"
    );
    assert!(!text.contains("driver pid"), "{text}");
    assert!(!run.dir().join("driver.log").exists());
}

/// Lane files for lanes 1 and 2 whose setup appends a line to
/// `scenario/lane-setup.log`, committed.
fn lanes_with_setup(fx: &Fixture) {
    let lanes = fx.path().join("docs/reference/lanes");
    fs::create_dir_all(&lanes).unwrap();
    for n in 1..=2 {
        fs::write(
            lanes.join(format!("lane-{n}.md")),
            format!(
                "---\nlane: {n}\nenv: [FACTORY_LANE_DB=fixture_{n}]\nsetup: echo \"setup $FACTORY_LANE\" >> \"$(dirname \"$(git rev-parse --path-format=absolute --git-common-dir)\")\"/scenario/lane-setup.log\n---\n\nLane {n} of the fixture.\n"
            ),
        )
        .unwrap();
    }
    fx.git(&["add", "-A"]);
    fx.git(&["commit", "-q", "-m", "chore: lane files"]);
}

#[test]
fn a_resumed_run_does_not_run_a_lanes_setup_again() {
    let fx = Fixture::with_index("lanes-setup-resume", &[("checkpoint_every", "10")]);
    lanes_with_setup(&fx);
    fx.chain();
    fx.write_plan(&[
        json!({"outcome": "done", "delay_ms": 3000}),
        json!({"outcome": "done", "delay_ms": 3000}),
    ]);
    let out = factory(&fx, &["run", "001"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    let dir = only_run_dir(&fx);
    let driver = state(&dir)["pid"].as_u64().unwrap() as u32;
    // The stub logs its call as it starts, before the driver records the
    // worker in state.json: a kill in between leaves an orphan the resumed
    // driver cannot see, and it builds slice 01 a second time.
    wait_until(&fx, &dir, "slice 01's worker", 20, || {
        fx.calls().len() == 1
            && state(&dir)["workers"]
                .as_array()
                .is_some_and(|w| w.len() == 1)
            && fx.slice_status(1) == "building"
    });
    assert_eq!(fx.read("scenario/lane-setup.log"), "setup 1\n");
    assert_eq!(state(&dir)["lanes_set_up"], json!([1]));
    assert_eq!(unsafe { libc::kill(driver as i32, libc::SIGKILL) }, 0);
    wait_until(&fx, &dir, "the driver to die", 5, || !pid_alive(driver));

    // The resumed driver settles 01's orphan and spawns 02 on lane 1 again:
    // the lane is set up for this run already.
    let out = factory(&fx, &["run", "001"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    wait_until(&fx, &dir, "slice 02's worker", 30, || {
        fx.calls().len() == 2 && fx.slice_status(2) == "building"
    });
    assert_eq!(slice_lane(&fx, 2), Some(1));
    assert_eq!(fx.read("scenario/lane-setup.log"), "setup 1\n");
    assert_eq!(state(&dir)["lanes_set_up"], json!([1]));

    let out = factory(&fx, &["stop", &run_id(&dir)]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    wait_until(&fx, &dir, "the run to stop", 30, || {
        state(&dir)["phase"] == "finished"
    });
}

#[test]
fn a_failing_driver_kills_and_settles_its_live_worker() {
    let fx = Fixture::with_index("lanes-abandon", &[("checkpoint_every", "10")]);
    fx.chain();
    fx.write_plan(&[json!({"outcome": "done", "delay_ms": 20000})]);
    let out = factory(&fx, &["run", "001"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    let dir = only_run_dir(&fx);
    wait_until(&fx, &dir, "slice 01's worker", 20, || {
        fx.calls().len() == 1
            && state(&dir)["workers"]
                .as_array()
                .is_some_and(|w| w.len() == 1)
            && fx.slice_status(1) == "building"
    });
    let worker = state(&dir)["workers"][0]["pid"].as_u64().unwrap() as u32;
    assert!(pid_alive(worker));

    // The board can no longer be read — `state.toml` no longer parses: the
    // driver's next tick fails the run.
    fs::write(
        fx.path().join(harness::FEATURE_DIR).join("state.toml"),
        "status = [ not toml\n",
    )
    .unwrap();
    let started = Instant::now();
    wait_until(&fx, &dir, "the run to fail", 15, || {
        state(&dir)["phase"] == "failed"
    });
    // Killed, not left to build on unwatched for the rest of its 20 s.
    wait_until(&fx, &dir, "the worker to die", 5, || !pid_alive(worker));
    assert!(started.elapsed() < Duration::from_secs(15));
    assert_eq!(state(&dir)["workers"][0]["outcome"], "failed");
    let rows = db(&fx).workers(&run_id(&dir)).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].outcome.as_deref(), Some("failed"));
    assert!(rows[0].ended_at.is_some(), "{:?}", rows[0]);
    let log = driver_log(&dir);
    assert!(log.contains("slice 01: worker"), "{log}");
}

#[test]
fn a_rate_limited_slice_goes_back_to_pending_with_no_lane_and_its_file_untouched() {
    // Slice 09 task 2 of PRD-002: the reset is a `state.toml` write alone —
    // `status = "pending"`, the `lane` key gone — and the slice file is
    // never written.
    let fx = Fixture::with_index("lanes-rate-limited", &[("checkpoint_every", "10")]);
    fx.chain();
    let slice_file = fx.slice_path(1);
    let before = fs::read(&slice_file).unwrap();
    fx.write_plan(&[json!({"outcome": "rate_limited", "reset_s": 600})]);
    let out = factory(&fx, &["run", "001"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    let dir = only_run_dir(&fx);
    wait_until(&fx, &dir, "the run to pause", 30, || {
        state(&dir)["phase"] == "paused"
    });
    assert_eq!(fx.calls().len(), 1);
    let raw = fx.read(&format!("{}/state.toml", harness::FEATURE_DIR));
    let table: toml::Table = toml::from_str(&raw).unwrap();
    let one = table["slices"]["1"].as_table().unwrap();
    assert_eq!(one["status"].as_str(), Some("pending"), "{raw}");
    assert!(!one.contains_key("lane"), "{raw}");
    assert_eq!(
        fs::read(&slice_file).unwrap(),
        before,
        "the slice file is untouched"
    );

    let out = factory(&fx, &["stop", &run_id(&dir)]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    wait_until(&fx, &dir, "the run to stop", 30, || {
        state(&dir)["phase"] == "finished"
    });
    assert_eq!(
        fs::read(&slice_file).unwrap(),
        before,
        "the slice file is untouched"
    );
    assert_eq!(fx.slice_status(1), "pending");
}

// ── PRD-005 slice 03: lane worktrees ─────────────────────────────────────────

/// What `lanes up` labels a seed path in this fixture: `clone` when `cp`
/// can clone a scratch file in the fixture's tempdir, else `copy`. Test
/// tempdirs are tmpfs here and ext4 on CI, so the probe decides.
fn seed_label(fx: &Fixture) -> &'static str {
    let base = data_dir(fx).parent().unwrap().to_path_buf();
    let probe = base.join("probe");
    fs::write(&probe, "p").unwrap();
    let flag = if cfg!(target_os = "macos") {
        "-c"
    } else {
        "--reflink=always"
    };
    let cloned = Command::new("cp")
        .arg(flag)
        .arg(&probe)
        .arg(base.join("probe-clone"))
        .output()
        .unwrap()
        .status
        .success();
    if cloned { "clone" } else { "copy" }
}

/// The worktrees under `$FACTORY_DATA_DIR/lanes/`, each with its porcelain
/// block.
fn lane_worktrees(fx: &Fixture) -> Vec<(PathBuf, String)> {
    let lanes = data_dir(fx).canonicalize().unwrap().join("lanes");
    fx.git(&["worktree", "list", "--porcelain"])
        .split("\n\n")
        .filter_map(|b| {
            let path = PathBuf::from(b.lines().next()?.strip_prefix("worktree ")?);
            path.starts_with(&lanes).then(|| (path, b.to_string()))
        })
        .collect()
}

#[test]
fn lanes_up_makes_two_detached_seeded_lanes_and_runs_setup_in_each() {
    let fx = Fixture::new("lanes-up");
    let root = fx.path().to_path_buf();
    let ignore = fs::read_to_string(root.join(".gitignore")).unwrap();
    fs::write(root.join(".gitignore"), format!("{ignore}/big/\n/.env\n")).unwrap();
    fs::create_dir_all(root.join("big/nested")).unwrap();
    fs::write(root.join("big/one.bin"), "one").unwrap();
    fs::write(root.join("big/nested/two.bin"), "two").unwrap();
    fs::write(root.join(".env"), "SECRET=fixture\n").unwrap();
    let lane_files = root.join("docs/reference/lanes");
    fs::create_dir_all(&lane_files).unwrap();
    for n in 1..=2 {
        fs::write(
            lane_files.join(format!("lane-{n}.md")),
            format!("---\nlane: {n}\nenv: []\nsetup: pwd -P > setup-pwd\n---\n"),
        )
        .unwrap();
    }
    fx.git(&["add", "-A"]);
    fx.git(&["commit", "-q", "-m", "chore: lane files and ignored state"]);
    fx.set_seed(&["big", ".env"]);

    let out = factory(&fx, &["lanes", "up"]);
    let text = stdout(&out);
    assert_eq!(out.status.code(), Some(0), "{text}");
    let label = seed_label(&fx);
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 2, "{text}");
    for (n, line) in (1..=2).zip(&lines) {
        assert!(line.starts_with(&format!("lane-{n}  ")), "{line}");
        assert!(
            line.contains(&format!("big {label} · .env {label}")),
            "{line}"
        );
        assert!(line.ends_with("setup 0"), "{line}");
    }

    let lanes = lane_worktrees(&fx);
    assert_eq!(lanes.len(), 2, "{lanes:?}");
    for (path, block) in &lanes {
        assert!(block.lines().any(|l| l == "detached"), "{block}");
        for f in ["big/one.bin", "big/nested/two.bin", ".env"] {
            assert_eq!(
                fs::read(path.join(f)).unwrap(),
                fs::read(root.join(f)).unwrap(),
                "{f} in {}",
                path.display()
            );
        }
        assert_eq!(
            fs::read_to_string(path.join("setup-pwd")).unwrap(),
            format!("{}\n", path.canonicalize().unwrap().display())
        );
    }

    // Again: nothing re-seeded, setup run in every lane each time.
    fs::remove_file(lanes[0].0.join("setup-pwd")).unwrap();
    let out = factory(&fx, &["lanes", "up"]);
    let text = stdout(&out);
    assert_eq!(out.status.code(), Some(0), "{text}");
    assert!(text.contains("big kept · .env kept"), "{text}");
    assert!(lanes[0].0.join("setup-pwd").is_file());
    assert_eq!(lane_worktrees(&fx).len(), 2);
}

#[test]
fn lanes_up_without_a_seed_refuses_naming_lanes() {
    let fx = Fixture::new("lanes-up-no-seed");
    fx.drop_lanes_table();
    let out = factory(&fx, &["lanes", "up"]);
    let text = stdout(&out);
    assert_eq!(out.status.code(), Some(1), "{text}");
    assert!(text.contains("/lanes"), "{text}");
    assert!(lane_worktrees(&fx).is_empty());
}

#[test]
fn a_run_without_a_lanes_seed_refuses_naming_lanes_and_creates_nothing() {
    let fx = Fixture::new("lanes-run-no-seed");
    fx.drop_lanes_table();
    fx.write_plan(&[done()]);
    let out = factory(&fx, &["run", "001"]);
    let text = stdout(&out);
    assert_eq!(out.status.code(), Some(1), "{text}");
    assert!(text.contains("no lanes declared — /lanes"), "{text}");
    assert!(!fx.stub_dir().join("calls").exists(), "no worker spawned");
    assert!(run_dirs(&fx).is_empty());
    assert!(lane_worktrees(&fx).is_empty());
}

#[test]
fn a_fresh_run_makes_its_lanes_and_workers_still_run_in_the_main_tree() {
    let fx = Fixture::with_index("lanes-run-makes", &[("checkpoint_every", "10")]);
    fx.write_plan(&[done(), done(), done(), checkpoint()]);
    let out = factory(&fx, &["run", "001", "--foreground"]);
    let text = stdout(&out);
    assert_eq!(out.status.code(), Some(0), "{text}");
    let lanes = lane_worktrees(&fx);
    assert_eq!(lanes.len(), 2, "{lanes:?}");
    for n in 1..=2 {
        assert!(
            text.lines()
                .any(|l| l.trim_start().starts_with(&format!("lane-{n}  "))),
            "{text}"
        );
    }
    // Slice 07 moves the workers into the lanes; until then, the main tree.
    let calls = fx.calls();
    assert!(!calls.is_empty());
    for c in &calls {
        assert_eq!(c["cwd"], fx.path().to_str().unwrap(), "call {}", c["n"]);
    }
}
