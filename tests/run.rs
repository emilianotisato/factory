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
use factory::run::{Phase, Run, WorkerRef, alive, load_state, pid_alive, start_time};
use harness::Fixture;
use serde_json::{Value, json};

const BIN: &str = env!("CARGO_BIN_EXE_factory");

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

/// `pid` becomes the run's driver, its start time with it.
fn set_pid(dir: &Path, pid: u32) {
    let mut s = state(dir);
    s["pid"] = Value::from(pid);
    s["pid_start"] = json!(start_time(pid));
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
fn a_new_run_directory_and_the_runs_directory_are_private() {
    // A run's events and prompts are the owner's: `runs/` and `runs/<id>/`
    // are 0700 whatever the umask leaves a plain mkdir at.
    use std::os::unix::fs::PermissionsExt;
    let fx = Fixture::new("private-runs");
    let data = data_dir(&fx).join("fresh");
    let run = Run::create(&data, fx.path(), "001", None, 1).unwrap();
    let mode = |p: &Path| fs::metadata(p).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(&data.join("runs")), 0o700);
    assert_eq!(mode(run.dir()), 0o700);
}

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
        [
            "events",
            "gates",
            "runs",
            "schema_version",
            "tool_calls",
            "workers"
        ]
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
            "polish_spawned",
            "polish_done",
            "shipped",
            "pr_opened",
            "ci_green",
            "run_finished"
        ],
        "{k:?}"
    );
    assert_eq!(
        fx.calls().len(),
        7,
        "two checkpoint-1 workers among the seven"
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
    assert_eq!(fx.calls().len(), 6);

    // A finished run is never resumed: the next `run` is a new one, which
    // reads the config file. The driver committed every slice, checkpoint and
    // the ship (slices 05, 06, 09), so the tree is clean for the preflight;
    // with the PRD shipped the new run resumes at the PR it finds open
    // (`gh pr view`), refreshes its body, watches CI and finishes, spawning
    // nothing and opening no second PR.
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
    assert_eq!(fx.calls().len(), 6, "a delivered feature spawns nothing");
    let gh: Vec<String> = fx
        .gh_log()
        .into_iter()
        .filter(|l| l.starts_with("pr create") || l.starts_with("pr edit"))
        .map(|l| l.split(" --body-file ").next().unwrap().to_string())
        .collect();
    assert_eq!(
        gh,
        [
            "pr create --draft --base main --title \"PRD-001: Fixture\"",
            "pr edit 1",
            "pr edit 1"
        ],
        "one PR, refreshed by each later run: {gh:?}"
    );
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
    // A stopped run is over: what continues is the feature, in a new run.
    assert!(
        stdout(&out).contains("factory run 001 resumes the feature in a new run"),
        "{}",
        stdout(&out)
    );
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
        stdout(&out).contains("in flight; the run finishes when"),
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
            "run_stopped"
        ],
        "{k:?}"
    );
    assert_eq!(state(&dir)["stopped"], true);
    let log = fs::read_to_string(dir.join("driver.log")).unwrap();
    assert!(
        log.contains(&format!(
            "run {id} stopped (1 slice(s) done) — factory run 001 resumes the feature in a new run"
        )),
        "{log}"
    );
    // Nothing resumes a finished run: the next `run` is a new one.
    let out = factory(&fx, &["run", "001"]);
    assert!(
        !stdout(&out).contains(&format!("run {id} resumed")),
        "{}",
        stdout(&out)
    );
}

// ── polish: the driver commits on the run's branch and nowhere else ──────────

#[test]
fn resume_refuses_off_the_runs_branch_and_on_the_default_branch() {
    let fx = Fixture::new("resume-branch");
    let data = data_dir(&fx);
    let run = Run::create(&data, fx.path(), "001", Some("feat/fixture".into()), 0).unwrap();
    let dir = run.dir().to_path_buf();
    let id = run.id().to_string();
    drop(run);
    set_pid(&dir, dead_pid());
    let before = state(&dir);

    // HEAD on main, the run on feat/fixture: no adopt, no resume.
    assert_eq!(head(&fx), "main");
    let out = factory(&fx, &["run", "001", "--foreground"]);
    assert_eq!(out.status.code(), Some(1), "{}", stdout(&out));
    assert!(
        stdout(&out).contains(&format!(
            "run {id} not resumed: HEAD is on main, not the run's branch feat/fixture"
        )),
        "{}",
        stdout(&out)
    );
    assert_eq!(state(&dir), before);
    assert!(!kinds(&dir).contains(&"run_resumed".to_string()));

    // A run whose branch is the default branch never resumes.
    fx.git(&["switch", "-q", "-c", "feat/fixture"]);
    fs::write(
        dir.join("preflight.json"),
        json!({"branch": {"current": "feat/fixture", "default": "feat/fixture"}}).to_string(),
    )
    .unwrap();
    let out = factory(&fx, &["run", "001", "--foreground"]);
    assert_eq!(out.status.code(), Some(1), "{}", stdout(&out));
    assert!(
        stdout(&out).contains("the run's branch feat/fixture is the default branch"),
        "{}",
        stdout(&out)
    );
    assert_eq!(state(&dir), before);

    // And a new run refuses to make the default branch its feature branch.
    let same = Branch {
        current: Some("main".into()),
        default: Some("main".into()),
        feature: Some("main".into()),
        alt: None,
        feature_exists: true,
    };
    let err = ensure_branch(fx.path(), &same).unwrap_err().to_string();
    assert!(err.contains("is the default branch"), "{err}");
}

#[test]
fn a_branch_switch_mid_run_parks_and_nothing_commits_off_the_branch() {
    let fx = Fixture::new("off-branch");
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
    let base = fx.git(&["rev-parse", "main"]);
    wait_until("slice 01 to be building", 20, || {
        fx.calls().len() == 1 && fx.slice_status(1) == "building"
    });
    assert_eq!(fx.git(&["rev-parse", "feat/fixture"]), base);

    // The owner switches away while the worker runs: the run parks, the
    // worker's exit is not routed, nothing commits and nothing spawns.
    fx.git(&["switch", "-q", "main"]);
    wait_until("the run to park off its branch", 20, || {
        state(&dir)["phase"] == "waiting-owner"
    });
    wait_until("the worker to mark slice 01 done", 20, || {
        fx.slice_status(1) == "done"
    });
    std::thread::sleep(std::time::Duration::from_millis(2500));
    assert_eq!(fx.calls().len(), 1);
    assert_eq!(fx.git(&["rev-parse", "main"]), base);
    assert_eq!(fx.git(&["rev-parse", "feat/fixture"]), base);
    assert!(!kinds(&dir).contains(&"slice_committed".to_string()));

    // Back on the branch: the slice commits there, and only there.
    fx.git(&["switch", "-q", "feat/fixture"]);
    wait_until("slice 01 to be committed", 20, || {
        kinds(&dir).contains(&"slice_committed".to_string())
    });
    assert_eq!(fx.git(&["rev-parse", "main"]), base);
    let subjects = fx.git(&["log", "--format=%s", "main..feat/fixture"]);
    assert!(subjects.contains("feat(001): slice-"), "{subjects}");

    let out = factory(&fx, &["stop", &id]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    wait_until("the run to finish", 40, || {
        state(&dir)["phase"] == "finished"
    });
}

// ── finding L11: one writer at a time, and only the owner drives ────────────

fn worker_ref(session: &str) -> WorkerRef {
    WorkerRef {
        slice: Some(1),
        checkpoint: None,
        role: "build".into(),
        session_id: session.into(),
        pid: dead_pid(),
        row_id: 1,
        spawned_at: "2026-10-10T00:00:00Z".into(),
        outcome: None,
    }
}

#[test]
fn a_stale_writer_changes_only_its_own_field_of_state_json() {
    let fx = Fixture::new("state-merge");
    let data = data_dir(&fx);
    let mut driver = Run::create(&data, fx.path(), "001", None, 1).unwrap();
    let dir = driver.dir().to_path_buf();
    // `factory stop`'s copy, read before the driver wrote anything.
    let mut stop = Run::open(&data, driver.id().as_str()).unwrap();
    driver.add_worker(worker_ref("s1")).unwrap();
    driver.set_phase(Phase::Building).unwrap();

    stop.request_stop().unwrap();
    let s = load_state(&dir).unwrap();
    assert!(s.stop_requested);
    assert_eq!(
        s.phase,
        Phase::Building,
        "the stop put back its stale phase"
    );
    assert_eq!(s.workers.len(), 1, "the stop put back its stale workers");
    assert_eq!(stop.state().workers.len(), 1);

    // The driver's stale copy does not undo the stop either.
    driver.set_worker_outcome("s1", "done").unwrap();
    let s = load_state(&dir).unwrap();
    assert!(s.stop_requested);
    assert_eq!(s.workers[0].outcome.as_deref(), Some("done"));
    // Each writer renames its own temp file, and none is left behind.
    let leftovers: Vec<String> = fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".tmp"))
        .collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");
}

#[test]
fn a_driver_whose_run_was_taken_over_writes_nothing_and_a_live_one_is_not_adopted() {
    let fx = Fixture::new("state-owner");
    let data = data_dir(&fx);
    let mut driver = Run::create(&data, fx.path(), "001", None, 1).unwrap();
    let dir = driver.dir().to_path_buf();
    let id = driver.id().as_str().to_string();
    let mut other = Command::new("sleep").arg("30").spawn().unwrap();
    let pid = other.id();

    // Another process took the run: the old owner's writes fail, untouched.
    set_pid(&dir, pid);
    assert_eq!(driver.lost_to(), Some(pid));
    let err = driver.set_phase(Phase::Building).unwrap_err().to_string();
    assert!(err.contains(&format!("taken over by pid {pid}")), "{err}");
    assert_eq!(driver.taken_over(), Some(pid));
    let s = load_state(&dir).unwrap();
    assert_eq!((s.pid, s.phase), (pid, Phase::Starting));

    // A live driver keeps its run.
    let mut resume = Run::open(&data, &id).unwrap();
    let err = resume.adopt().unwrap_err().to_string();
    assert!(err.contains(&format!("driven by pid {pid}")), "{err}");
    assert_eq!(load_state(&dir).unwrap().pid, pid);

    // Gone, it is adopted.
    other.kill().unwrap();
    other.wait().unwrap();
    resume.adopt().unwrap();
    assert_eq!(load_state(&dir).unwrap().pid, std::process::id());
    resume.set_phase(Phase::Building).unwrap();
}

#[test]
fn a_detached_driver_whose_run_is_taken_over_stops_without_failing_it() {
    let fx = Fixture::new("driver-lost");
    fx.chain();
    fx.write_plan(&[json!({"outcome": "done", "delay_ms": 3000})]);
    let out = factory(&fx, &["run", "001"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    let dir = run_dirs(&fx).remove(0);
    let driver = state(&dir)["pid"].as_u64().unwrap() as u32;
    wait_until("slice 01 to be building", 20, || {
        fx.calls().len() == 1 && fx.slice_status(1) == "building"
    });

    let mut other = Command::new("sleep").arg("30").spawn().unwrap();
    let pid = other.id();
    wait_until("the driver to stop", 20, || {
        if state(&dir)["pid"] != pid {
            set_pid(&dir, pid);
        }
        !pid_alive(driver)
    });
    let s = state(&dir);
    assert_eq!(s["pid"], pid);
    assert_ne!(s["phase"], "failed");
    assert!(!kinds(&dir).iter().any(|k| k == "run_failed"));
    let log = fs::read_to_string(dir.join("driver.log")).unwrap();
    assert!(
        log.contains(&format!("was taken over by pid {pid}")),
        "{log}"
    );
    other.kill().unwrap();
    other.wait().unwrap();
}

#[test]
fn a_detached_driver_started_from_a_subdirectory_finds_a_relative_data_dir() {
    // `factory run` from `docs/` with data and config dirs relative to it:
    // the daemon runs in the repo, so it is handed the absolute paths, or it
    // looks for its run directory somewhere else and never takes it over.
    let fx = Fixture::new("daemon-dirs");
    fx.chain();
    fx.write_plan(&[json!({"outcome": "done", "delay_ms": 3000})]);
    let out = fx
        .command(BIN)
        .args(["run", "001"])
        .current_dir(fx.path().join("docs"))
        .env("FACTORY_DATA_DIR", "../../data")
        .env("FACTORY_CONFIG_DIR", "../../config")
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}{}",
        stdout(&out),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(stdout(&out).contains("driver pid "), "{}", stdout(&out));
    let dir = run_dirs(&fx).remove(0);
    let driver = state(&dir)["pid"].as_u64().unwrap() as u32;
    wait_until("slice 01 to be building", 20, || {
        fx.calls().len() == 1 && fx.slice_status(1) == "building"
    });
    let id = dir.file_name().unwrap().to_str().unwrap().to_string();
    let out = factory(&fx, &["stop", &id]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    wait_until("the driver to stop", 30, || !pid_alive(driver));
    assert!(committed(&dir, 1), "{:?}", kinds(&dir));
    assert_eq!(state(&dir)["stopped"], true);
}

// ── finding L10: emitters in several processes share one sequence ───────────

#[test]
fn concurrent_emitters_never_share_a_seq_or_tear_a_line() {
    let fx = Fixture::new("emit-race");
    let data = data_dir(&fx);
    let run = Run::create(&data, fx.path(), "001", None, 1).unwrap();
    let id = run.id().as_str().to_string();
    let dir = run.dir().to_path_buf();
    drop(run);
    // Each thread is its own emitter with its own file handle and store
    // connection, as `factory stop` and `factory answer` are beside the driver.
    let threads: Vec<_> = (0..4)
        .map(|t| {
            let (data, id) = (data.clone(), id.clone());
            std::thread::spawn(move || {
                let mut run = Run::open(&data, &id).unwrap();
                for i in 0..40 {
                    run.emit(EventKind::JudgeNoted {
                        text: format!("thread {t} event {i} {}", "x".repeat(2000)),
                    })
                    .unwrap();
                }
            })
        })
        .collect();
    for t in threads {
        t.join().unwrap();
    }
    let seqs: Vec<u64> = events(&dir)
        .iter()
        .map(|e| e["seq"].as_u64().unwrap())
        .collect();
    assert_eq!(seqs, (1..=160).collect::<Vec<u64>>());
    let db = Db::open(&data.join("factory.db")).unwrap();
    assert_eq!(db.events(&id).unwrap().len(), 160);
}

// ── finding Xc6: a pid is a process only with its start time ─────────────────

fn committed(dir: &Path, slice: u64) -> bool {
    events(dir)
        .iter()
        .any(|e| e["kind"] == "slice_committed" && e["slice"] == slice)
}

#[test]
fn a_pid_reused_by_another_process_is_not_the_runs_driver() {
    let me = std::process::id();
    let start = start_time(me).unwrap();
    assert!(alive(me, Some(start)));
    assert!(alive(me, None));
    assert!(!alive(me, Some(start + 1)));
    assert_eq!(start_time(dead_pid()), None);

    let fx = Fixture::with_index("pid-reused", &[("lanes", "1")]);
    let data = data_dir(&fx);
    let run = Run::create(&data, fx.path(), "001", None, 1).unwrap();
    let dir = run.dir().to_path_buf();
    let id = run.id().as_str().to_string();
    assert_eq!(run.state().pid_start, Some(start));
    drop(run);

    // The driver's pid now names a process that started after it.
    let mut other = Command::new("sleep").arg("30").spawn().unwrap();
    let reuse = |pid: u32| {
        let mut s = state(&dir);
        s["pid"] = Value::from(pid);
        fs::write(dir.join("state.json"), s.to_string()).unwrap();
    };
    reuse(other.id());
    assert!(pid_alive(other.id()));
    assert!(!load_state(&dir).unwrap().driver_alive());
    assert!(Run::find_live(&data, fx.path(), "001").unwrap().is_none());
    let mut resume = Run::open(&data, &id).unwrap();
    resume.adopt().unwrap();
    assert_eq!(load_state(&dir).unwrap().pid, me);
    drop(resume);

    // `factory stop` does not take it for a live driver either.
    let mut s = state(&dir);
    s["pid_start"] = json!(start);
    fs::write(dir.join("state.json"), s.to_string()).unwrap();
    reuse(other.id());
    let out = factory(&fx, &["stop", &id]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    assert!(stdout(&out).contains("no live process"), "{}", stdout(&out));
    other.kill().unwrap();
    other.wait().unwrap();
}

#[test]
fn a_resumed_driver_settles_an_orphan_whose_pid_was_reused() {
    let fx = Fixture::with_index("orphan-reused", &[("checkpoint_every", "10")]);
    fx.chain();
    fx.write_plan(&[json!({"outcome": "done", "delay_ms": 2000})]);
    let out = factory(&fx, &["run", "001"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    let dir = run_dirs(&fx).remove(0);
    let driver = state(&dir)["pid"].as_u64().unwrap() as u32;
    wait_until("slice 01's worker", 20, || {
        state(&dir)["workers"]
            .as_array()
            .is_some_and(|w| w.len() == 1)
    });
    let s = state(&dir);
    let session = s["workers"][0]["session_id"].as_str().unwrap().to_string();
    let worker = s["workers"][0]["pid"].as_u64().unwrap() as u32;
    assert_eq!(
        s["worker_starts"][&session].as_u64(),
        start_time(worker),
        "{s}"
    );
    assert_eq!(unsafe { libc::kill(driver as i32, libc::SIGKILL) }, 0);
    wait_until("the orphan to finish slice 01", 20, || {
        fx.slice_status(1) == "done" && !pid_alive(worker)
    });

    // The worker's pid now names another process.
    let mut other = Command::new("sleep").arg("30").spawn().unwrap();
    let mut s = state(&dir);
    s["workers"][0]["pid"] = Value::from(other.id());
    fs::write(dir.join("state.json"), s.to_string()).unwrap();

    let out = factory(&fx, &["run", "001"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    wait_until("slice 01 to be committed", 15, || committed(&dir, 1));
    // The commit lands before the outcome: wait for the settle too.
    wait_until("slice 01's worker to be settled", 15, || {
        !state(&dir)["workers"][0]["outcome"].is_null()
    });
    assert_eq!(state(&dir)["workers"][0]["outcome"], "done");
    assert!(pid_alive(other.id()));
    other.kill().unwrap();
    other.wait().unwrap();
    let out = factory(&fx, &["stop", dir.file_name().unwrap().to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    wait_until("the run to stop", 30, || state(&dir)["phase"] == "finished");
}

#[test]
fn a_resumed_driver_settles_a_dead_orphan_beside_a_live_one() {
    let fx = Fixture::with_index("orphans-mixed", &[("checkpoint_every", "10")]);
    // 01 and 02 are independent: both build at once, 01 until it is killed.
    fx.write_plan(&[
        json!({"outcome": "done", "delay_ms": 120000}),
        json!({"outcome": "done", "delay_ms": 1000}),
    ]);
    let out = factory(&fx, &["run", "001"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    let dir = run_dirs(&fx).remove(0);
    let driver = state(&dir)["pid"].as_u64().unwrap() as u32;
    wait_until("two workers", 20, || {
        state(&dir)["workers"]
            .as_array()
            .is_some_and(|w| w.len() == 2)
    });
    assert_eq!(unsafe { libc::kill(driver as i32, libc::SIGKILL) }, 0);
    let s = state(&dir);
    assert_eq!(s["workers"][0]["slice"], 1);
    let first = s["workers"][0]["pid"].as_u64().unwrap() as u32;
    wait_until("the orphan to finish slice 02", 20, || {
        fx.slice_status(2) == "done"
    });

    // Slice 02's orphan is settled and committed while 01's still runs.
    let out = factory(&fx, &["run", "001"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    wait_until("slice 02 to be committed", 20, || committed(&dir, 2));
    assert!(pid_alive(first), "slice 01's worker outlived the commit");
    // The commit lands before the outcome: wait for the settle too.
    wait_until("slice 02's worker to be settled", 20, || {
        !state(&dir)["workers"][1]["outcome"].is_null()
    });
    assert_eq!(state(&dir)["workers"][1]["outcome"], "done");

    // A few ticks of waiting on 01, said once.
    std::thread::sleep(std::time::Duration::from_millis(2500));
    let log = fs::read_to_string(dir.join("driver.log")).unwrap();
    let waits = log
        .lines()
        .filter(|l| l.contains("of an earlier driver is still running"))
        .count();
    assert_eq!(waits, 1, "{log}");
    assert_eq!(state(&dir)["workers"][0]["outcome"], Value::Null);

    let out = factory(&fx, &["stop", dir.file_name().unwrap().to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    assert_eq!(unsafe { libc::kill(first as i32, libc::SIGKILL) }, 0);
    wait_until("the run to stop", 30, || state(&dir)["phase"] == "finished");
    assert_eq!(state(&dir)["workers"][0]["outcome"], "dead");
}

// ── finding K2: the final event lands before the final phase ─────────────────

#[test]
fn a_stop_that_cannot_save_the_phase_has_logged_run_stopped_already() {
    let fx = Fixture::new("stop-order");
    let data = data_dir(&fx);
    let mut run = Run::create(&data, fx.path(), "001", None, 1).unwrap();
    run.set_phase(Phase::Building).unwrap();
    let dir = run.dir().to_path_buf();
    // A run dir that refuses new files: state.json's temp file cannot be
    // made, events.jsonl can still be appended to.
    let mode = |m: u32| {
        fs::set_permissions(&dir, std::os::unix::fs::PermissionsExt::from_mode(m)).unwrap()
    };
    mode(0o555);
    if fs::write(dir.join("probe"), "").is_ok() {
        // Root ignores the mode: nothing to see here.
        mode(0o755);
        return;
    }
    let res = factory::scheduler::mark_stopped(&mut run);
    mode(0o755);
    assert!(res.is_err());
    assert_eq!(kinds(&dir).last().map(String::as_str), Some("run_stopped"));
    assert_eq!(load_state(&dir).unwrap().phase, Phase::Building);

    // The retry saves the phase and does not log the stop twice.
    factory::scheduler::mark_stopped(&mut run).unwrap();
    let s = load_state(&dir).unwrap();
    assert_eq!((s.phase, s.stopped), (Phase::Finished, true));
    assert_eq!(
        kinds(&dir).iter().filter(|k| *k == "run_stopped").count(),
        1
    );
}
