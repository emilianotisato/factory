//! `factory run NNN [--budget N]` up to the first board read (PRD §3 FR-1).
//!
//! A live run for this repo and feature refuses. An unfinished one resumes:
//! no preflight, `run_resumed`, the board. Otherwise `docs preflight --json`
//! decides: `refuse[]` prints the tool's own lines and exits 1 with no run
//! directory; `ask[]` and `judge[]` park a new run on one `ASK-n` per item
//! (the answer path is slice 04); `pass` creates or switches to the feature
//! branch, creates the run, emits `run_started` and prints the ready slices.
//! Slice 03 replaces the final exit with the loop.
use std::io::Write;
use std::path::Path;
use std::process::Command;

use clap::Args;

use crate::docs::{Branch, Docs};
use crate::error::io;
use crate::events::{AskSource, EventKind};
use crate::run::{Phase, Run};
use crate::{Error, Result, config, paths};

#[derive(Args, Debug, Clone)]
pub struct RunArgs {
    /// The feature: its number (`001`) or slug
    pub feature: String,
    /// Weighted-token budget for this run; overrides `config.toml`
    #[arg(long, value_name = "N")]
    pub budget: Option<u64>,
}

/// Run the command from the current directory; returns the exit code.
pub fn run(args: &RunArgs, out: &mut dyn Write, err: &mut dyn Write) -> Result<i32> {
    let cwd = std::env::current_dir().map_err(io("current directory"))?;
    let repo = paths::repo_root(&cwd)?;
    let data_dir = paths::data_dir();
    let loaded = config::load(&paths::config_dir())?;
    for w in &loaded.warnings {
        writeln!(err, "factory: {w}").map_err(io("stderr"))?;
    }
    let budget = args.budget.unwrap_or(loaded.config.budget_weighted_tokens);
    let docs = Docs::locate(&repo)?;
    let feature = feature_number(&docs, &args.feature)?;

    if let Some(live) = Run::find_live(&data_dir, &repo, &feature)? {
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
        run.adopt()?;
        if let Some(b) = args.budget {
            run.set_budget(b)?;
        }
        run.emit(EventKind::RunResumed)?;
        writeln!(out, "run {} resumed ({})", run.id(), run.state().phase).map_err(io("stdout"))?;
        return print_board(&docs, &feature, out);
    }

    let (pf, _code) = docs.preflight(&feature)?;
    if !pf.refuse.is_empty() {
        for line in &pf.refuse {
            writeln!(out, "  ✗ {line}").map_err(io("stdout"))?;
        }
        writeln!(out, "\nrefused: {} blocker(s)", pf.refuse.len()).map_err(io("stdout"))?;
        return Ok(1);
    }
    let asks: Vec<(AskSource, &String)> = pf
        .ask
        .iter()
        .map(|t| (AskSource::Ask, t))
        .chain(pf.judge.iter().map(|t| (AskSource::Judge, t)))
        .collect();
    if !asks.is_empty() {
        let mut run = Run::create(&data_dir, &repo, &feature, None, budget)?;
        for (i, (source, text)) in asks.iter().enumerate() {
            let id = format!("ASK-{}", i + 1);
            writeln!(out, "{id} ({source}): {text}").map_err(io("stdout"))?;
            run.emit(EventKind::AskOpened {
                id,
                source: *source,
                text: (*text).clone(),
            })?;
        }
        run.set_phase(Phase::WaitingOwner)?;
        writeln!(
            out,
            "\nrun {} waits for the owner: {} question(s) before it starts — \
             `factory answer ASK-n \"<text>\"`; \"proceed\" is always an option",
            run.id(),
            asks.len()
        )
        .map_err(io("stdout"))?;
        return Ok(0);
    }

    let branch = ensure_branch(&repo, &pf.branch)?;
    let mut run = Run::create(&data_dir, &repo, &feature, Some(branch), budget)?;
    begin(&mut run, &docs, out)
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

/// Follow `branch.feature` from preflight (today's `/deliver` §0 rule): on the
/// default branch, create the feature branch or switch to the existing one; on
/// the feature branch already, nothing; anywhere else, refuse naming the two
/// acceptable branches. Returns the branch the repo is now on.
pub fn ensure_branch(repo: &Path, b: &Branch) -> Result<String> {
    let feature = b
        .feature
        .as_deref()
        .ok_or_else(|| Error::Msg("preflight named no feature branch".into()))?;
    let current = b.current.as_deref().unwrap_or("");
    let default = b.default.as_deref().unwrap_or("the default branch");
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
