//! Run state, events, config, the store, `factory run` and `factory stop`
//! (PRD-001 slice 02 tasks 2–6).
#[path = "harness/mod.rs"]
mod harness;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use factory::cli::run::{begin, ensure_branch};
use factory::config;
use factory::db::Db;
use factory::docs::{Branch, Docs};
use factory::events::EventKind;
use factory::run::{Phase, Run, pid_alive};
use harness::Fixture;
use serde_json::{Value, json};

const BIN: &str = env!("CARGO_BIN_EXE_factory");

fn factory(fx: &Fixture, args: &[&str]) -> Output {
    fx.command(BIN).args(args).output().unwrap()
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn data_dir(fx: &Fixture) -> PathBuf {
    PathBuf::from(&fx.env()["FACTORY_DATA_DIR"])
}

fn config_dir(fx: &Fixture) -> PathBuf {
    PathBuf::from(&fx.env()["FACTORY_CONFIG_DIR"])
}

fn run_dirs(fx: &Fixture) -> Vec<PathBuf> {
    let Ok(rd) = fs::read_dir(data_dir(fx).join("runs")) else {
        return Vec::new();
    };
    let mut v: Vec<PathBuf> = rd.map(|e| e.unwrap().path()).collect();
    v.sort();
    v
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

fn set_pid(dir: &Path, pid: u32) {
    let mut s = state(dir);
    s["pid"] = Value::from(pid);
    fs::write(dir.join("state.json"), s.to_string()).unwrap();
}

/// A pid that belonged to a process which has exited and been reaped.
fn dead_pid() -> u32 {
    let mut child = Command::new("true").spawn().unwrap();
    let pid = child.id();
    child.wait().unwrap();
    pid
}

fn wait_until(what: &str, secs: u64, mut ok: impl FnMut() -> bool) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(secs);
    while !ok() {
        assert!(
            std::time::Instant::now() < deadline,
            "timed out after {secs}s waiting for {what}"
        );
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

fn head(fx: &Fixture) -> String {
    fx.git(&["rev-parse", "--abbrev-ref", "HEAD"])
        .trim()
        .to_string()
}

// ── task 2: run dir, state, events ───────────────────────────────────────────

#[test]
fn emit_writes_a_line_and_a_row_each_time() {
    let fx = Fixture::new("emit");
    let data = data_dir(&fx);
    let mut run = Run::create(
        &data,
        fx.path(),
        "001",
        Some("feat/fixture".into()),
        30_000_000,
    )
    .unwrap();
    let id = run.id().as_str().to_string();
    assert!(
        id.starts_with("001-") && id.len() == "001-20261009-abcd".len(),
        "{id}"
    );
    assert_eq!(run.state().phase, Phase::Starting);
    assert_eq!(run.state().pid, std::process::id());
    assert!(run.dir().join("events.jsonl").is_file());

    let first = run.emit(EventKind::RunStarted).unwrap();
    let second = run
        .emit(EventKind::AskOpened {
            id: "ASK-1".into(),
            text: "no inner loop declared".into(),
        })
        .unwrap();
    assert_eq!((first.seq, second.seq), (1, 2));

    let lines = events(run.dir());
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0]["seq"], 1);
    assert_eq!(lines[0]["kind"], "run_started");
    assert!(lines[0]["ts"].as_str().unwrap().ends_with('Z'));
    assert_eq!(lines[1]["seq"], 2);
    assert_eq!(lines[1]["kind"], "ask_opened");
    assert_eq!(lines[1]["id"], "ASK-1");
    assert_eq!(lines[1]["text"], "no inner loop declared");
    assert_eq!(run.events().unwrap(), vec![first, second]);

    let rows = run.db().events(&id).unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!((rows[0].seq, rows[0].kind.as_str()), (1, "run_started"));
    assert_eq!(rows[0].payload, "{}");
    assert_eq!((rows[1].seq, rows[1].kind.as_str()), (2, "ask_opened"));
    let payload: Value = serde_json::from_str(&rows[1].payload).unwrap();
    assert_eq!(payload["id"], "ASK-1");
    assert!(payload.get("kind").is_none() && payload.get("seq").is_none());

    let row = run.db().run_row(&id).unwrap().unwrap();
    assert_eq!(row.phase, "starting");
    assert_eq!(row.budget_weighted_tokens, 30_000_000);
    assert_eq!(row.branch.as_deref(), Some("feat/fixture"));
    assert_eq!(row.finished_at, None);
}

#[test]
fn find_live_sees_only_a_living_pid() {
    let fx = Fixture::new("live");
    let data = data_dir(&fx);
    let run = Run::create(&data, fx.path(), "001", None, 1).unwrap();
    let id = run.id().clone();
    drop(run);

    // This test process is the run's pid, so it is live.
    let live = Run::find_live(&data, fx.path(), "001").unwrap().unwrap();
    assert_eq!(live.id(), &id);
    assert!(Run::find_live(&data, fx.path(), "002").unwrap().is_none());

    // The process gone: not live, but resumable.
    let dir = data.join("runs").join(id.as_str());
    let dead = dead_pid();
    assert!(!pid_alive(dead));
    set_pid(&dir, dead);
    assert!(Run::find_live(&data, fx.path(), "001").unwrap().is_none());
    let resumable = Run::find_resumable(&data, fx.path(), "001")
        .unwrap()
        .unwrap();
    assert_eq!(resumable.id(), &id);

    // Finished: neither.
    let mut run = Run::open(&data, id.as_str()).unwrap();
    run.set_phase(Phase::Finished).unwrap();
    assert!(
        Run::find_resumable(&data, fx.path(), "001")
            .unwrap()
            .is_none()
    );
    assert!(
        run.db()
            .run_row(id.as_str())
            .unwrap()
            .unwrap()
            .finished_at
            .is_some()
    );
    assert!(!pid_alive(0));
}

// ── task 4: config.toml ──────────────────────────────────────────────────────

#[test]
fn config_defaults_file_and_unknown_keys() {
    let dir = tempfile::tempdir().unwrap();
    let loaded = config::load(dir.path()).unwrap();
    assert_eq!(loaded.config, config::Config::default());
    assert_eq!(loaded.config.budget_weighted_tokens, 30_000_000);
    assert_eq!(loaded.config.workers.build.model, "opus");
    assert_eq!(loaded.config.workers.polish.effort, "high");
    assert!(loaded.warnings.is_empty());

    fs::write(
        dir.path().join("config.toml"),
        "budget_weighted_tokens = 5\nbogus = 1\n\n[workers.fix]\nmodel = \"sonnet\"\nnope = 2\n\n[workers.other]\nmodel = \"x\"\n",
    )
    .unwrap();
    let loaded = config::load(dir.path()).unwrap();
    assert_eq!(loaded.config.budget_weighted_tokens, 5);
    assert_eq!(loaded.config.workers.fix.model, "sonnet");
    assert_eq!(loaded.config.workers.fix.effort, "high");
    assert_eq!(loaded.config.workers.build.model, "opus");
    let warned: Vec<&str> = loaded
        .warnings
        .iter()
        .map(|w| w.rsplit('`').nth(1).unwrap())
        .collect();
    assert_eq!(warned, ["bogus", "workers.fix.nope", "workers.other"]);

    fs::write(
        dir.path().join("config.toml"),
        "budget_weighted_tokens = \"many\"\n",
    )
    .unwrap();
    assert!(config::load(dir.path()).is_err());
}

// ── task 5: the store ────────────────────────────────────────────────────────

#[test]
fn opening_the_store_twice_applies_the_schema_once() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("deep/er/factory.db");
    let a = Db::open(&path).unwrap();
    let b = Db::open(&path).unwrap();
    assert_eq!(a.schema_version().unwrap(), 1);
    assert_eq!(b.schema_version().unwrap(), 1);
    let versions: i64 = b
        .conn()
        .query_row("SELECT count(*) FROM schema_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(versions, 1);
    assert_eq!(
        b.tables().unwrap(),
        ["events", "gates", "runs", "schema_version", "workers"]
    );
}

// ── task 3: factory run ──────────────────────────────────────────────────────

#[test]
fn run_refuses_a_dirty_tree_with_no_run_dir_and_no_spawn() {
    let fx = Fixture::new("run-dirty");
    fs::write(fx.path().join("dirty.txt"), "x\n").unwrap();
    let out = factory(&fx, &["run", "001"]);
    assert_eq!(out.status.code(), Some(1), "{}", stdout(&out));
    let text = stdout(&out);
    assert!(
        text.contains("✗ working tree not clean (1 path(s))"),
        "{text}"
    );
    assert!(text.contains("refused: 1 blocker(s)"), "{text}");
    assert!(run_dirs(&fx).is_empty());
    assert!(fx.calls().is_empty());
    assert_eq!(head(&fx), "main");
}

#[test]
fn run_on_the_clean_fixture_starts_notes_the_judge_lines_then_resumes() {
    // The default `checkpoint_every: 2`: the loop builds slices 01 and 02,
    // then the driver runs checkpoint 1 itself (slice 06); its checkpoint
    // worker is slow enough to kill the driver under, and the resumed run
    // spawns another (the fourth step).
    let fx = Fixture::new("run-start");
    fx.write_plan(&[
        json!({"outcome": "done"}),
        json!({"outcome": "done"}),
        json!({"outcome": "checkpoint", "delay_ms": 3000}),
        json!({"outcome": "checkpoint"}),
        json!({"outcome": "done"}),
        json!({"outcome": "checkpoint"}),
    ]);
    let out = factory(&fx, &["run", "1"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    let text = stdout(&out);
    let dirs = run_dirs(&fx);
    assert_eq!(dirs.len(), 1, "{dirs:?}");
    let dir = &dirs[0];
    let id = dir.file_name().unwrap().to_str().unwrap().to_string();
    assert!(id.starts_with("001-"), "{id}");

    // The tool's judge lines are noted, never waited on (FR-1, BLOCK-201):
    // the branch is created, the run starts and the board is printed.
    assert_eq!(head(&fx), "feat/fixture");
    let s = state(dir);
    assert_eq!(s["feature"], "001");
    assert_eq!(s["branch"], "feat/fixture");
    assert_eq!(s["budget_weighted_tokens"], 30_000_000);
    assert_eq!(s["stop_requested"], false);
    let evs = events(dir);
    let k = kinds(dir);
    assert!(k.len() >= 5, "{k:?}");
    assert!(k[..4].iter().all(|k| k == "judge_noted"), "{k:?}");
    assert_eq!(k[4], "run_started");
    for (i, e) in evs.iter().enumerate() {
        assert_eq!(e["seq"], i as u64 + 1);
    }
    assert!(
        evs.iter().any(|e| e["text"]
            .as_str()
            .is_some_and(|t| t.starts_with("harness on disk:"))),
        "{evs:?}"
    );
    assert!(text.contains("  · harness on disk:"), "{text}");
    assert!(text.contains("  · on main — create feat/fixture"), "{text}");
    assert!(
        text.contains(&format!("run {id} started on feat/fixture")),
        "{text}"
    );
    assert!(text.contains("01 001-fixture/slice-01-a.md"), "{text}");
    assert!(text.contains("02 001-fixture/slice-02-b.md"), "{text}");
    assert!(!text.contains("slice-03-c.md"), "{text}");
    assert!(!text.contains("ASK-"), "{text}");
    assert!(text.contains("driver pid "), "{text}");

    // The detached driver builds 01 and 02 — independent, and the index has
    // two lanes: both spawn in the same tick and finish in either order
    // (slice 04) — then runs the gate and hands checkpoint 1 to a worker.
    // (The stub records its call a beat after the spawn event.)
    wait_until("the checkpoint worker", 40, || {
        kinds(dir).last().map(String::as_str) == Some("checkpoint_spawned")
            && state(dir)["workers"]
                .as_array()
                .is_some_and(|w| w.len() == 3)
            && fx.calls().len() == 3
    });
    let k = kinds(dir);
    assert_eq!(k.len(), 15, "{k:?}");
    assert!(k[5..7].iter().all(|k| k == "slice_spawned"), "{k:?}");
    // Each reap commits the slice right after `slice_done` (slice 05).
    assert_eq!(
        k[7..11],
        [
            "slice_done",
            "slice_committed",
            "slice_done",
            "slice_committed"
        ],
        "{k:?}"
    );
    assert_eq!(
        k[11..],
        [
            "checkpoint_started",
            "gate_run",
            "log_merged",
            "checkpoint_spawned"
        ],
        "{k:?}"
    );
    assert_eq!(fx.slice_status(1), "done");
    assert_eq!(fx.slice_status(2), "done");
    assert_eq!(fx.slice_status(3), "pending");
    assert_eq!(fx.calls().len(), 3);
    let started = events(dir)
        .into_iter()
        .find(|e| e["kind"] == "checkpoint_started")
        .unwrap();
    assert_eq!(started["covers"], json!([1, 2]));
    let db = Db::open(&data_dir(&fx).join("factory.db")).unwrap();
    let row = db.run_row(&id).unwrap().unwrap();
    assert_eq!(row.phase, "checkpoint");
    assert_eq!(row.branch.as_deref(), Some("feat/fixture"));
    assert_eq!(db.events(&id).unwrap().len(), 15);

    // A live driver: refused, naming the run and `factory stop`.
    let driver = state(dir)["pid"].as_u64().unwrap() as u32;
    assert!(pid_alive(driver));
    let out = factory(&fx, &["run", "001"]);
    assert_eq!(out.status.code(), Some(1), "{}", stdout(&out));
    let text = stdout(&out);
    assert!(text.contains(&format!("run {id} is live")), "{text}");
    assert!(text.contains(&format!("factory stop {id}")), "{text}");
    assert_eq!(run_dirs(&fx).len(), 1);

    // The driver gone (its session killed under the checkpoint worker): the
    // same run resumes — no preflight, the board printed, the flag taken —
    // and runs the unverified checkpoint first (FR-1) on the cached gate,
    // before anything spawns.
    let worker = state(dir)["workers"][2]["pid"].as_u64().unwrap() as u32;
    // SAFETY: a plain kill(2) on the detached driver's own session group.
    let rc = unsafe { libc::kill(-(driver as i32), libc::SIGKILL) };
    assert_eq!(rc, 0);
    wait_until("the driver and its worker to be gone", 5, || {
        !pid_alive(driver) && !pid_alive(worker)
    });
    assert_eq!(state(dir)["phase"], "checkpoint");
    // A budget the three workers so far fit under: FR-9 (slice 07) parks a
    // resumed run whose weighted tokens already pass it.
    let out = factory(&fx, &["run", "001", "--budget", "90000000"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    let text = stdout(&out);
    assert_eq!(run_dirs(&fx).len(), 1);
    assert!(
        text.contains(&format!("run {id} resumed (checkpoint)")),
        "{text}"
    );
    assert!(
        !text.contains("judge_noted") && !text.contains("  · "),
        "{text}"
    );
    assert_eq!(state(dir)["budget_weighted_tokens"], 90_000_000);
    wait_until("the resumed run to finish", 40, || {
        state(dir)["phase"] == "finished"
    });
    let k = kinds(dir);
    assert_eq!(k[15], "run_resumed", "{k:?}");
    assert_eq!(
        k[16..],
        [
            "worker_dead",
            "checkpoint_resumed",
            "gate_cached",
            "checkpoint_spawned",
            "checkpoint_done",
            "slice_spawned",
            "slice_done",
            "slice_committed",
            "checkpoint_started",
            "gate_run",
            "log_merged",
            "checkpoint_spawned",
            "checkpoint_done",
            "run_finished"
        ],
        "{k:?}"
    );
    assert_eq!(
        fx.calls().len(),
        6,
        "two checkpoint-1 workers among the six"
    );
    assert_eq!(fx.slice_status(3), "done");
    assert_eq!(db.run_row(&id).unwrap().unwrap().phase, "finished");
    // The resumer adopted the run: the killed driver's pid is gone from the
    // state file.
    assert_ne!(state(dir)["pid"], Value::from(driver));
}

#[test]
fn run_parks_on_an_ask_line_and_creates_no_branch() {
    // `lanes: 1` with two slices runnable at once is what preflight asks about.
    let fx = Fixture::with_index("run-ask", &[("lanes", "1")]);
    let out = factory(&fx, &["run", "001"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    let text = stdout(&out);
    let dirs = run_dirs(&fx);
    assert_eq!(dirs.len(), 1, "{dirs:?}");
    let dir = &dirs[0];
    let id = dir.file_name().unwrap().to_str().unwrap().to_string();

    let s = state(dir);
    assert_eq!(s["phase"], "waiting-owner");
    assert_eq!(s["branch"], Value::Null);
    let evs = events(dir);
    assert!(!evs.is_empty(), "{text}");
    for (i, e) in evs.iter().enumerate() {
        assert_eq!(e["kind"], "ask_opened");
        assert_eq!(e["seq"], i as u64 + 1);
        assert_eq!(e["id"], format!("ASK-{}", i + 1));
        assert!(e.get("source").is_none(), "{e}");
    }
    assert!(
        evs.iter()
            .any(|e| e["text"].as_str().unwrap().contains("`lanes: 1`")),
        "{evs:?}"
    );
    assert!(text.contains("ASK-1: "), "{text}");
    assert!(
        text.contains(&format!("run {id} waits for the owner")),
        "{text}"
    );
    assert!(
        !text.contains("run_started") && !text.contains("slice-01-a.md"),
        "{text}"
    );
    assert_eq!(head(&fx), "main", "no branch before the owner answers");
    assert!(fx.calls().is_empty());
    let db = Db::open(&data_dir(&fx).join("factory.db")).unwrap();
    assert_eq!(db.run_row(&id).unwrap().unwrap().phase, "waiting-owner");
    assert_eq!(db.events(&id).unwrap().len(), evs.len());
}

#[test]
fn pass_path_creates_the_branch_and_starts_the_run() {
    let fx = Fixture::new("run-pass");
    let docs = Docs::locate(fx.path()).unwrap();
    // Preflight's `pass` shape on the fixture, fed in directly so each branch
    // rule is exercised on its own.
    let fresh = Branch {
        current: Some("main".into()),
        default: Some("main".into()),
        feature: Some("feat/fixture".into()),
        alt: Some("feat/001-fixture".into()),
        feature_exists: false,
    };
    assert_eq!(ensure_branch(fx.path(), &fresh).unwrap(), "feat/fixture");
    assert_eq!(head(&fx), "feat/fixture");

    let data = data_dir(&fx);
    let mut run = Run::create(&data, fx.path(), "001", Some("feat/fixture".into()), 7).unwrap();
    let mut out = Vec::new();
    assert_eq!(begin(&mut run, &docs, &mut out).unwrap(), 0);
    let text = String::from_utf8(out).unwrap();
    assert!(
        text.starts_with(&format!("run {} started on feat/fixture\n", run.id())),
        "{text}"
    );
    assert!(
        text.contains("01 001-fixture/slice-01-a.md\n02 001-fixture/slice-02-b.md\n"),
        "{text}"
    );
    assert_eq!(kinds(run.dir()), ["run_started"]);
    assert_eq!(run.state().phase, Phase::Starting);
    let row = run.db().run_row(run.id().as_str()).unwrap().unwrap();
    assert_eq!(
        (row.phase.as_str(), row.budget_weighted_tokens),
        ("starting", 7)
    );
    assert_eq!(row.branch.as_deref(), Some("feat/fixture"));
    let rows = run.db().events(run.id().as_str()).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].kind, "run_started");

    // Already on it: nothing to do.
    let on_it = Branch {
        current: Some("feat/fixture".into()),
        feature_exists: true,
        ..fresh.clone()
    };
    assert_eq!(ensure_branch(fx.path(), &on_it).unwrap(), "feat/fixture");
    assert_eq!(head(&fx), "feat/fixture");

    // Back on main with the branch existing: switch, not create.
    fx.git(&["switch", "-q", "main"]);
    let existing = Branch {
        feature_exists: true,
        ..fresh.clone()
    };
    assert_eq!(ensure_branch(fx.path(), &existing).unwrap(), "feat/fixture");
    assert_eq!(head(&fx), "feat/fixture");

    // Anywhere else: refuse, naming the two acceptable branches.
    fx.git(&["switch", "-q", "-c", "other"]);
    let elsewhere = Branch {
        current: Some("other".into()),
        ..existing
    };
    let err = ensure_branch(fx.path(), &elsewhere)
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("on other") && err.contains("main") && err.contains("feat/fixture"),
        "{err}"
    );
    assert_eq!(head(&fx), "other");
}

#[test]
fn budget_comes_from_the_flag_then_the_config_then_the_default() {
    let fx = Fixture::new("run-budget");
    fs::write(
        config_dir(&fx).join("config.toml"),
        "budget_weighted_tokens = 5\nextra = true\n",
    )
    .unwrap();
    // In the foreground the loop runs through both checkpoints before `run`
    // returns, so every step below sees a finished run. The first run's
    // budget has to hold its five workers (FR-9, slice 07: over budget parks);
    // the later runs spawn nothing, so theirs can stay small.
    fx.write_plan(&[
        json!({"outcome": "done"}),
        json!({"outcome": "done"}),
        json!({"outcome": "checkpoint"}),
        json!({"outcome": "done"}),
        json!({"outcome": "checkpoint"}),
    ]);
    let out = factory(&fx, &["run", "001", "--budget", "70000000", "--foreground"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("unknown key `extra` ignored"),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let first = run_dirs(&fx);
    assert_eq!(first.len(), 1, "{first:?}");
    assert_eq!(state(&first[0])["budget_weighted_tokens"], 70_000_000);
    assert_eq!(state(&first[0])["phase"], "finished");
    assert_eq!(fx.calls().len(), 5);

    // A finished run is never resumed: the next `run` is a new one, which
    // reads the config file. The driver committed every slice and checkpoint
    // (slices 05, 06), so the tree is clean for the preflight; with every
    // slice done and verified the new run finishes at once, spawning nothing.
    let newest = |before: &[PathBuf]| {
        let dirs = run_dirs(&fx);
        assert_eq!(dirs.len(), before.len() + 1, "{dirs:?}");
        dirs.into_iter().find(|d| !before.contains(d)).unwrap()
    };
    assert_eq!(fx.git(&["status", "--porcelain"]), "");
    let out = factory(&fx, &["run", "001", "--foreground"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    let second = newest(&first);
    assert_eq!(state(&second)["budget_weighted_tokens"], 5);
    assert_eq!(state(&second)["phase"], "finished");
    assert_eq!(state(&first[0])["budget_weighted_tokens"], 70_000_000);

    // The flag over the config file, on a new run too.
    let out = factory(&fx, &["run", "001", "--budget", "9", "--foreground"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    let third = newest(&[first[0].clone(), second]);
    assert_eq!(state(&third)["budget_weighted_tokens"], 9);
    assert_eq!(fx.calls().len(), 5, "a delivered feature spawns nothing");
}

// ── task 6: factory stop ─────────────────────────────────────────────────────

#[test]
fn stop_finishes_a_dead_run_and_only_flags_a_live_one() {
    // A run parked on a preflight ask has no driver process: an unfinished
    // run whose pid is dead, which is what `stop` tells apart.
    let fx = Fixture::with_index("stop", &[("lanes", "1")]);
    let out = factory(&fx, &["run", "001"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    let dir = run_dirs(&fx).remove(0);
    let id = dir.file_name().unwrap().to_str().unwrap().to_string();
    assert_eq!(state(&dir)["phase"], "waiting-owner");

    // Live: the flag is set, the phase is not.
    set_pid(&dir, std::process::id());
    let out = factory(&fx, &["stop", &id]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    assert!(
        stdout(&out).contains("stop requested for run"),
        "{}",
        stdout(&out)
    );
    let s = state(&dir);
    assert_eq!(s["stop_requested"], true);
    assert_eq!(s["phase"], "waiting-owner");
    assert_eq!(
        kinds(&dir).last().map(String::as_str),
        Some("stop_requested")
    );

    // Dead: finished, and said so.
    set_pid(&dir, dead_pid());
    let out = factory(&fx, &["stop", &id]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    assert!(stdout(&out).contains("marked finished"), "{}", stdout(&out));
    let s = state(&dir);
    assert_eq!(s["phase"], "finished");
    assert_eq!(s["stop_requested"], true);
    let k = kinds(&dir);
    assert_eq!(
        k.iter().filter(|k| *k == "stop_requested").count(),
        2,
        "{k:?}"
    );
    let db = Db::open(&data_dir(&fx).join("factory.db")).unwrap();
    let row = db.run_row(&id).unwrap().unwrap();
    assert_eq!(row.phase, "finished");
    assert!(row.finished_at.is_some());

    // Again: already finished, nothing more written.
    let out = factory(&fx, &["stop", &id]);
    assert_eq!(out.status.code(), Some(0));
    assert!(
        stdout(&out).contains("already finished"),
        "{}",
        stdout(&out)
    );
    assert_eq!(kinds(&dir).len(), k.len());

    // Unknown run: exit 1, says so.
    let out = factory(&fx, &["stop", "001-20000101-dead"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("no run 001-20000101-dead"));
}

#[test]
fn stop_during_a_slice_lets_the_worker_finish_then_ends_the_run() {
    let fx = Fixture::new("stop-live");
    fx.chain();
    fx.write_plan(&[
        json!({"outcome": "done", "delay_ms": 2500}),
        json!({"outcome": "done"}),
        json!({"outcome": "done"}),
    ]);
    let out = factory(&fx, &["run", "001"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    let dir = run_dirs(&fx).remove(0);
    let id = dir.file_name().unwrap().to_str().unwrap().to_string();
    wait_until("slice 01 to be building", 20, || {
        fx.calls().len() == 1 && fx.slice_status(1) == "building"
    });
    let out = factory(&fx, &["stop", &id]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    assert!(
        stdout(&out).contains("finishes its in-flight workers"),
        "{}",
        stdout(&out)
    );
    wait_until("the run to finish", 30, || {
        state(&dir)["phase"] == "finished"
    });
    assert_eq!(fx.slice_status(1), "done");
    assert_eq!(fx.slice_status(2), "pending");
    assert_eq!(fx.slice_status(3), "pending");
    assert_eq!(fx.calls().len(), 1);
    let k = kinds(&dir);
    assert_eq!(
        k[5..],
        [
            "slice_spawned",
            "stop_requested",
            "slice_done",
            "slice_committed",
            "run_finished"
        ],
        "{k:?}"
    );
    // Nothing resumes a finished run: the next `run` is a new one.
    let out = factory(&fx, &["run", "001"]);
    assert!(
        !stdout(&out).contains(&format!("run {id} resumed")),
        "{}",
        stdout(&out)
    );
}
