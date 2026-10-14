//! `factory lanes up` (PRD-005 FR-1): creates the missing lane worktrees
//! `1..=lanes` of this repo, seeds them, runs each lane's setup inside it,
//! and prints one line per lane: its path, how each seed path got there, the
//! setup exit. Any failure still prints the other lanes, then exits 1. A
//! repo with no `[lanes] seed` refuses, naming `/lanes`. A fresh
//! `factory run` runs the create-and-seed half only (`ensure_all`).
//! `up --prove` (FR-9) then runs the inner loop in every lane at once;
//! `prune` (FR-10) removes the lanes above the declared count.
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use clap::Subcommand;

use crate::docs::{Docs, project};
use crate::error::io;
use crate::{Result, lanes, paths, run};

#[derive(Subcommand, Debug, Clone)]
pub enum LanesVerb {
    /// Create, seed and set up every lane, one line each
    Up {
        /// Then run the inner loop in every lane at once; red if any lane is
        #[arg(long)]
        prove: bool,
    },
    /// Remove the lanes above the declared count
    Prune {
        /// Remove every lane of this repo
        #[arg(long)]
        all: bool,
    },
}

pub fn lanes(verb: &LanesVerb, out: &mut dyn Write) -> Result<i32> {
    let cwd = std::env::current_dir().map_err(io("current directory"))?;
    let main = lanes::main_tree(&cwd)?;
    let data = paths::data_dir();
    match verb {
        LanesVerb::Up { prove: false } => up(&data, &main, out),
        LanesVerb::Up { prove: true } => prove(&data, &main, out),
        LanesVerb::Prune { all } => prune(&data, &main, *all, out),
    }
}

fn say(out: &mut dyn Write, line: &str) -> Result<()> {
    writeln!(out, "{line}").map_err(io("stdout"))
}

/// `[docs] lanes` of the main tree, 1 when undeclared.
fn count(main: &Path) -> Result<u32> {
    Ok(project::load(main)?.map_or(1, |p| p.lanes))
}

/// Every lane of `main` made, seeded and set up, one line each.
pub fn up(data: &Path, main: &Path, out: &mut dyn Write) -> Result<i32> {
    let Some(seed) = project::seed(main)? else {
        say(out, &format!("  ✗ {}", lanes::NO_SEED))?;
        return Ok(1);
    };
    let mut failed = false;
    for n in 1..=count(main)? {
        let report = match lanes::ensure(data, main, n, &seed) {
            Ok(r) => r,
            Err(e) => {
                say(out, &format!("lane-{n}  ✗ {e}"))?;
                failed = true;
                continue;
            }
        };
        // The lane file is read from the main tree: a lane holds the
        // commit it was reset to, which may predate an edit to it.
        let (exit, detail) = match lanes::load(main, n).and_then(|l| l.run_setup(&report.dir)) {
            Ok(None) => ("-".to_string(), String::new()),
            Ok(Some(o)) if o.status.success() => ("0".to_string(), String::new()),
            Ok(Some(o)) => {
                failed = true;
                let code = o.status.code().map_or("killed".into(), |c| c.to_string());
                (code, String::from_utf8_lossy(&o.stderr).trim().to_string())
            }
            Err(e) => {
                failed = true;
                ("✗".to_string(), e.to_string())
            }
        };
        say(out, &format!("{}  setup {exit}", report.line()))?;
        for l in detail.lines() {
            say(out, &format!("    {l}"))?;
        }
    }
    Ok(i32::from(failed))
}

/// `up`, then the inner loop in every lane at once: `sh -c <inner>`, cwd
/// the lane, the lane's env. One line per lane in lane order, a red lane's
/// output tail under its line, and a last line naming the red lanes.
pub fn prove(data: &Path, main: &Path, out: &mut dyn Write) -> Result<i32> {
    let code = up(data, main, out)?;
    if code != 0 {
        return Ok(code);
    }
    let inner = match Docs::open(main).and_then(|d| d.gate_inner()) {
        Ok(cmd) => cmd,
        Err(e) => {
            say(out, &format!("  ✗ {e}"))?;
            return Ok(1);
        }
    };
    let jobs = (1..=count(main)?)
        .map(|n| Ok((lanes::dir(data, main, n), lanes::load(main, n)?)))
        .collect::<Result<Vec<(PathBuf, lanes::Lane)>>>()?;
    // One thread per lane, so every child starts before any is waited on.
    let results = std::thread::scope(|s| {
        let handles: Vec<_> = jobs
            .iter()
            .map(|(dir, lane)| {
                let inner = &inner;
                s.spawn(move || {
                    let start = Instant::now();
                    let res = Command::new("sh")
                        .arg("-c")
                        .arg(inner)
                        .current_dir(dir)
                        .envs(lane.env())
                        .output();
                    (lane.n, res, start.elapsed().as_secs_f64())
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().expect("a prove thread does not panic"))
            .collect::<Vec<_>>()
    });
    let mut red = Vec::new();
    for (n, res, secs) in results {
        let (exit, tail) = match res {
            Ok(o) if o.status.success() => ("0".to_string(), String::new()),
            Ok(o) => {
                let code = o.status.code().map_or("killed".into(), |c| c.to_string());
                let text = format!(
                    "{}{}",
                    String::from_utf8_lossy(&o.stdout),
                    String::from_utf8_lossy(&o.stderr)
                );
                (code, text)
            }
            Err(e) => ("✗".to_string(), format!("could not start sh: {e}")),
        };
        say(out, &format!("lane-{n}  inner  exit {exit}  {secs:.1}s"))?;
        if exit != "0" {
            red.push(format!("lane-{n}"));
            let lines: Vec<&str> = tail.lines().collect();
            for l in &lines[lines.len().saturating_sub(TAIL)..] {
                say(out, &format!("    {l}"))?;
            }
        }
    }
    if red.is_empty() {
        return Ok(0);
    }
    say(out, &format!("✗ inner red in {}", red.join(", ")))?;
    Ok(1)
}

/// How many lines of a red lane's output `--prove` prints.
const TAIL: usize = 20;

/// The lane worktrees above the declared count (every one with `all`)
/// removed, worktree and dir, then `git worktree prune`. Refuses, removing
/// nothing, while a run of this repo is unfinished: a parked run's blocked
/// slice holds its lane.
pub fn prune(data: &Path, main: &Path, all: bool, out: &mut dyn Write) -> Result<i32> {
    let open = run::list_open(data, main)?;
    if !open.is_empty() {
        say(
            out,
            "  ✗ prune refused: this repo has an unfinished run, which holds its lanes",
        )?;
        for s in &open {
            say(
                out,
                &format!("    run {}  feature {}  {}", s.id, s.feature, s.phase),
            )?;
        }
        return Ok(1);
    }
    let keep = if all { 0 } else { count(main)? };
    let mut removed = 0;
    for (n, dir) in lanes::on_disk(data, main)? {
        if n > keep {
            lanes::remove_lane(main, &dir)?;
            say(out, &format!("lane-{n}  {}  removed", dir.display()))?;
            removed += 1;
        }
    }
    lanes::prune(main)?;
    if removed == 0 {
        say(out, &format!("no lane above {keep} to remove"))?;
    }
    Ok(0)
}

/// The create-and-seed half, for a fresh `factory run`: every lane of
/// `main` ensured, one line each; `false` at the first failure, printed.
/// Setup is the run's, at each lane's first spawn.
pub fn ensure_all(data: &Path, main: &Path, out: &mut dyn Write) -> Result<bool> {
    let seed = project::seed(main)?.unwrap_or_default();
    for n in 1..=count(main)? {
        match lanes::ensure(data, main, n, &seed) {
            Ok(r) => say(out, &format!("  {}", r.line()))?,
            Err(e) => {
                say(out, &format!("  ✗ lane-{n}: {e}"))?;
                return Ok(false);
            }
        }
    }
    Ok(true)
}
