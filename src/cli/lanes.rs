//! `factory lanes up` (PRD-005 FR-1): creates the missing lane worktrees
//! `1..=lanes` of this repo, seeds them, runs each lane's setup inside it,
//! and prints one line per lane: its path, how each seed path got there, the
//! setup exit. Any failure still prints the other lanes, then exits 1. A
//! repo with no `[lanes] seed` refuses, naming `/lanes`. A fresh
//! `factory run` runs the create-and-seed half only (`ensure_all`).
use std::io::Write;
use std::path::Path;

use clap::Subcommand;

use crate::docs::project;
use crate::error::io;
use crate::{Result, lanes, paths};

#[derive(Subcommand, Debug, Clone)]
pub enum LanesVerb {
    /// Create, seed and set up every lane, one line each
    Up,
}

pub fn lanes(verb: &LanesVerb, out: &mut dyn Write) -> Result<i32> {
    let LanesVerb::Up = verb;
    let cwd = std::env::current_dir().map_err(io("current directory"))?;
    let main = lanes::main_tree(&cwd)?;
    up(&paths::data_dir(), &main, out)
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
