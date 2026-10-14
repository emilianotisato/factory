//! `factory run NNN [--budget N]` up to the first board read (PRD §3 FR-1).
//!
//! A live run for this repo and feature refuses. An unfinished one resumes —
//! unless HEAD is off the run's branch, which refuses (the driver commits
//! only there): no preflight, `run_resumed`, the board. Otherwise `docs preflight --json`
//! decides: `refuse[]` prints the tool's own lines and exits 1 with no run
//! directory; `ask[]` parks a new run on one `ASK-n` per item (the answer
//! path is slice 04); `judge[]` lines are noted as events and never waited on
//! (BLOCK-201); `pass` creates or switches to the feature branch, creates the
//! run, emits `judge_noted` per line and `run_started`, and prints the ready
//! slices. Then the loop (slice 03): a daemon re-executed as the hidden
//! `factory daemon <id>` and detached with `setsid`, or in this process with
//! `--foreground`. A fresh run makes and seeds its lane worktrees after the
//! refusals and before the run exists (PRD-005 FR-1).
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use clap::Args;

use crate::cli::lanes;
use crate::docs::{Branch, Docs, Preflight};
use crate::error::{io, json};
use crate::events::EventKind;
use crate::run::{Phase, Run};
use crate::{Error, Result, commit, config, daemon, paths, scheduler};

#[derive(Args, Debug, Clone)]
pub struct RunArgs {
    /// The feature: its number (`001`) or slug
    pub feature: String,
    /// Weighted-token budget for this run; overrides `config.toml` and
    /// `docs/project.toml`
    #[arg(long, value_name = "N")]
    pub budget: Option<u64>,
    /// Keep the driver attached to this terminal instead of detaching it
    #[arg(long)]
    pub foreground: bool,
    /// The one line this run tests, stored on its row (PRD-003 FR-23)
    #[arg(long, value_name = "TEXT")]
    pub hypothesis: Option<String>,
}

/// `factory daemon <run>`: the detached driver `factory run` starts. Hidden.
#[derive(Args, Debug, Clone)]
pub struct DaemonArgs {
    pub run: String,
}

/// Run the command from the current directory; returns the exit code.
pub fn run(args: &RunArgs, out: &mut dyn Write, err: &mut dyn Write) -> Result<i32> {
    let cwd = std::env::current_dir().map_err(io("current directory"))?;
    let repo = paths::repo_root(&cwd)?;
    let data_dir = paths::data_dir();
    let mut loaded = config::load_with_project(&paths::config_dir(), &repo)?;
    for w in &loaded.warnings {
        writeln!(err, "factory: {w}").map_err(io("stderr"))?;
    }
    if let Some(b) = args.budget {
        loaded.config.budget_weighted_tokens = b;
        loaded.sources.budget = config::Source::Flag;
    }
    let budget = loaded.config.budget_weighted_tokens;
    // A repo not on v3 is refused before preflight (FR-18). `main` maps an
    // `Err` to exit 1, so the hint is printed here and the exit is 2.
    let docs = match Docs::open(&repo) {
        Ok(d) => d,
        Err(Error::NotV3(hint)) => {
            writeln!(err, "factory: {hint}").map_err(io("stderr"))?;
            return Ok(2);
        }
        Err(e) => return Err(e),
    };
    let feature = feature_number(&docs, &args.feature)?;

    if let Some(live) = Run::find_live(&data_dir, &repo, &feature)? {
        hypothesis_ignored(args, live.id().as_str(), err)?;
        writeln!(
            out,
            "run {id} is live (pid {pid}) — stop it first: factory stop {id}",
            id = live.id(),
            pid = live.state().pid
        )
        .map_err(io("stdout"))?;
        return Ok(1);
    }
    if let Some(mut run) = Run::find_resumable(&data_dir, &repo, &feature)? {
        // The driver commits on the run's branch and nowhere else: a tree the
        // owner switched away (or a branch that is the default) refuses the
        // resume before the run is touched.
        if let Some(branch) = run.state().branch.clone() {
            let default = load_preflight(run.dir())?.and_then(|pf| pf.branch.default);
            if let Some(why) = commit::branch_refusal(&repo, &branch, default.as_deref())? {
                writeln!(out, "run {} not resumed: {why}", run.id()).map_err(io("stdout"))?;
                return Ok(1);
            }
        }
        hypothesis_ignored(args, run.id().as_str(), err)?;
        run.adopt()?;
        if let Some(b) = args.budget {
            run.set_budget(b)?;
            budget_knob(&run, b)?;
        }
        run.emit(EventKind::RunResumed)?;
        writeln!(out, "run {} resumed ({})", run.id(), run.state().phase).map_err(io("stdout"))?;
        print_board(&docs, &feature, out)?;
        return launch(&mut run, &docs, &loaded.config, args.foreground, out);
    }

    let (pf, _code) = docs.preflight(&feature)?;
    if !pf.refuse.is_empty() {
        for line in &pf.refuse {
            writeln!(out, "  ✗ {line}").map_err(io("stdout"))?;
        }
        writeln!(out, "\nrefused: {} blocker(s)", pf.refuse.len()).map_err(io("stdout"))?;
        return Ok(1);
    }
    // A fresh run makes and seeds its lanes before any run exists; setup is
    // left to each lane's first spawn (PRD-005 FR-1).
    if !lanes::ensure_all(&data_dir, &crate::lanes::main_tree(&repo)?, out)? {
        writeln!(out, "\nrefused: a lane could not be made").map_err(io("stdout"))?;
        return Ok(1);
    }
    if !pf.ask.is_empty() {
        let mut run = Run::create(&data_dir, &repo, &feature, None, budget)?;
        record_start(&run, &docs, &loaded, args.hypothesis.as_deref(), err)?;
        save_preflight(run.dir(), &pf)?;
        for (i, text) in pf.ask.iter().enumerate() {
            let id = format!("ASK-{}", i + 1);
            writeln!(out, "{id}: {text}").map_err(io("stdout"))?;
            run.emit(EventKind::AskOpened {
                id,
                text: text.clone(),
            })?;
        }
        run.set_phase(Phase::WaitingOwner)?;
        writeln!(
            out,
            "\nrun {} waits for the owner: {} question(s) before it starts — \
             `factory answer ASK-n \"<text>\"`; \"proceed\" is always an option",
            run.id(),
            pf.ask.len()
        )
        .map_err(io("stdout"))?;
        return Ok(0);
    }

    let branch = ensure_branch(&repo, &pf.branch)?;
    let mut run = Run::create(&data_dir, &repo, &feature, Some(branch), budget)?;
    record_start(&run, &docs, &loaded, args.hypothesis.as_deref(), err)?;
    // The closeout reads `branch.default` from here (FR-6: `--base`).
    save_preflight(run.dir(), &pf)?;
    // The driver is the judge: each line is noted for the cockpit and the run
    // goes on (FR-1, BLOCK-201).
    for text in &pf.judge {
        writeln!(out, "  · {text}").map_err(io("stdout"))?;
        run.emit(EventKind::JudgeNoted { text: text.clone() })?;
    }
    begin(&mut run, &docs, out)?;
    launch(&mut run, &docs, &loaded.config, args.foreground, out)
}

/// Hand the run to the loop: in this process with `--foreground`, else to a
/// detached daemon, returning once it owns the run.
pub fn launch(
    run: &mut Run,
    docs: &Docs,
    cfg: &config::Config,
    foreground: bool,
    out: &mut dyn Write,
) -> Result<i32> {
    if foreground {
        scheduler::drive(run, docs, cfg, out)?;
        return Ok(0);
    }
    let pid = daemon::spawn(run)?;
    writeln!(
        out,
        "driver pid {pid} — log {}",
        daemon::log_path(run.dir()).display()
    )
    .map_err(io("stdout"))?;
    Ok(0)
}

/// The daemon's side: take the run over, then drive it. Errors go to the
/// driver log; the run is already `failed` by the time one returns.
pub fn daemon(args: &DaemonArgs, out: &mut dyn Write, err: &mut dyn Write) -> Result<i32> {
    let mut run = Run::open(&paths::data_dir(), &args.run)?;
    run.adopt()?;
    // The project file of the run's repo, as `factory run` resolved it.
    let loaded = config::load_with_project(&paths::config_dir(), &run.state().repo)?;
    for w in &loaded.warnings {
        writeln!(err, "factory: {w}").map_err(io("stderr"))?;
    }
    let repo = run.state().repo.clone();
    let docs = Docs::open(&repo)?;
    writeln!(
        out,
        "{} driver pid {} took run {}",
        crate::clock::now_rfc3339(),
        std::process::id(),
        run.id()
    )
    .map_err(io("driver log"))?;
    match scheduler::drive(&mut run, &docs, &loaded.config, out) {
        Ok(_) => Ok(0),
        Err(e) => {
            writeln!(err, "factory: {e}").map_err(io("stderr"))?;
            Ok(1)
        }
    }
}

/// A `--hypothesis` given to a run that already exists keeps the stored
/// one: it is written once, at creation (PRD-003 FR-23).
fn hypothesis_ignored(args: &RunArgs, id: &str, err: &mut dyn Write) -> Result<()> {
    if args.hypothesis.is_some() {
        writeln!(err, "hypothesis ignored: run {id} already exists").map_err(io("stderr"))?;
    }
    Ok(())
}

/// A new run's row gets what it resolved (`runs.knobs`) and its
/// hypothesis (PRD-003 FR-23). Lanes come from the board; `null` when it
/// cannot be read, as is the claude version when `claude --version` fails.
fn record_start(
    run: &Run,
    docs: &Docs,
    loaded: &config::Loaded,
    hypothesis: Option<&str>,
    err: &mut dyn Write,
) -> Result<()> {
    let lanes = docs.status(&run.state().feature).ok().map(|s| s.lanes);
    let claude = claude_version(err)?;
    let knobs = config::knobs(loaded, lanes, claude.as_deref());
    let id = run.id().as_str();
    run.db().set_run_knobs(id, &knobs.to_string())?;
    if let Some(h) = hypothesis {
        run.db().set_run_hypothesis(id, h)?;
    }
    Ok(())
}

/// A resume's `--budget` replaces the budget in `runs.knobs`, so the row
/// says what the run went on with, not only what it started with.
fn budget_knob(run: &Run, budget: u64) -> Result<()> {
    let id = run.id().as_str();
    let Some(text) = run.db().run_row(id)?.and_then(|r| r.knobs) else {
        return Ok(());
    };
    let Ok(mut knobs) = serde_json::from_str::<serde_json::Value>(&text) else {
        return Ok(());
    };
    knobs["budget"] = serde_json::json!({"value": budget, "source": config::Source::Flag});
    run.db().set_run_knobs(id, &knobs.to_string())
}

/// The trimmed stdout of `claude --version`; one stderr line and `None`
/// when it cannot be run or fails.
fn claude_version(err: &mut dyn Write) -> Result<Option<String>> {
    let why = match Command::new("claude")
        .arg("--version")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
    {
        Ok(o) if o.status.success() => {
            let v = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if !v.is_empty() {
                return Ok(Some(v));
            }
            "printed nothing".to_string()
        }
        Ok(o) => format!("exited {}", o.status),
        Err(e) => e.to_string(),
    };
    writeln!(
        err,
        "factory: claude --version: {why}; the run records no claude version"
    )
    .map_err(io("stderr"))?;
    Ok(None)
}

/// The `pass` path once the branch is right: `run_started`, then the board.
/// Slice 04 calls it again when the last `ASK-n` is answered.
pub fn begin(run: &mut Run, docs: &Docs, out: &mut dyn Write) -> Result<i32> {
    run.emit(EventKind::RunStarted)?;
    writeln!(
        out,
        "run {} started on {}",
        run.id(),
        run.state().branch.as_deref().unwrap_or("?")
    )
    .map_err(io("stdout"))?;
    print_board(docs, &run.state().feature.clone(), out)
}

/// `runs/<id>/preflight.json`: the preflight a run parked on asks was created
/// from, so the ask→branch step (slice 04) follows its `branch` instead of
/// re-running preflight against a tree the owner may since have touched.
pub const PREFLIGHT_FILE: &str = "preflight.json";

pub fn save_preflight(run_dir: &Path, pf: &Preflight) -> Result<()> {
    let path = run_dir.join(PREFLIGHT_FILE);
    let text = serde_json::to_string_pretty(pf).map_err(json("serialize preflight"))?;
    fs::write(&path, text + "\n").map_err(io(format!("write {}", path.display())))
}

pub fn load_preflight(run_dir: &Path) -> Result<Option<Preflight>> {
    let path = run_dir.join(PREFLIGHT_FILE);
    match fs::read_to_string(&path) {
        Ok(t) => Ok(Some(
            serde_json::from_str(&t).map_err(json(format!("parse {}", path.display())))?,
        )),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(io(format!("read {}", path.display()))(e)),
    }
}

/// Follow `branch.feature` from preflight (today's `/deliver` §0 rule): on the
/// default branch, create the feature branch or switch to the existing one; on
/// the feature branch already, nothing; anywhere else, refuse naming the two
/// acceptable branches. A feature branch that is the default branch refuses:
/// the driver never commits there. Returns the branch the repo is now on.
pub fn ensure_branch(repo: &Path, b: &Branch) -> Result<String> {
    let feature = b
        .feature
        .as_deref()
        .ok_or_else(|| Error::Msg("preflight named no feature branch".into()))?;
    let current = b.current.as_deref().unwrap_or("");
    let default = b.default.as_deref().unwrap_or("the default branch");
    if b.default.as_deref() == Some(feature) {
        return Err(Error::Msg(format!(
            "the feature branch {feature} is the default branch — the driver commits only on a \
             feature branch, and the owner decides which one"
        )));
    }
    if current == feature {
        return Ok(feature.to_string());
    }
    if Some(current) == b.default.as_deref() {
        let args: &[&str] = if b.feature_exists {
            &["switch", feature]
        } else {
            &["switch", "-c", feature]
        };
        git(repo, args)?;
        return Ok(feature.to_string());
    }
    Err(Error::Msg(format!(
        "on {current}, which is neither {default} nor {feature} — the owner decides which branch this feature lives on"
    )))
}

fn git(repo: &Path, args: &[&str]) -> Result<()> {
    let out = Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .map_err(|e| Error::Tool {
            cmd: format!("git {}", args.join(" ")),
            detail: format!("could not start git: {e}"),
        })?;
    if !out.status.success() {
        return Err(Error::Tool {
            cmd: format!("git {}", args.join(" ")),
            detail: String::from_utf8_lossy(&out.stderr).trim().to_string(),
        });
    }
    Ok(())
}

/// The ready slices as `<number> <file>`, one per line.
fn print_board(docs: &Docs, feature: &str, out: &mut dyn Write) -> Result<i32> {
    let board = docs.status(feature)?;
    if board.ready.is_empty() {
        writeln!(out, "nothing ready").map_err(io("stdout"))?;
    }
    for s in &board.ready {
        writeln!(out, "{:02} {}", s.number, s.file).map_err(io("stdout"))?;
    }
    Ok(0)
}

/// `001` as given or from `1`; a slug is resolved through the board.
fn feature_number(docs: &Docs, arg: &str) -> Result<String> {
    if let Ok(n) = arg.parse::<u32>() {
        return Ok(format!("{n:03}"));
    }
    Ok(format!("{:03}", docs.status(arg)?.number))
}
