//! `second` exits 0 when a pass's text counted, 2 when none did or clap refused a flag (the skills' fallback), 1 on a bad combination.
//! Once the probe passed, a failing pass is its own `unreviewed` line, never the command's error: the other passes keep their findings.
//! `collect` exits 0 when the file was written (or printed, without `--out`), whatever each part found, and 1 on a usage error or when HEAD or the diff cannot be read.
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use clap::{Args, Subcommand};

use crate::error::io;
use crate::review::{self, Pass};
use crate::{Result, collect, config, paths};

#[derive(Subcommand, Debug, Clone)]
pub enum ReviewVerb {
    /// The cross-vendor second opinion over a diff or a document
    Second(SecondArgs),
    /// The plan, the leaks scan and the second opinion of a branch, as one markdown file
    Collect(CollectArgs),
}

#[derive(Args, Debug, Clone)]
pub struct SecondArgs {
    /// Review `<base>...HEAD`, split into passes
    #[arg(long, value_name = "BASE")]
    pub diff: Option<String>,
    /// Review one document, with --prompt
    #[arg(long, value_name = "FILE")]
    pub doc: Option<PathBuf>,
    /// The instruction for --doc
    #[arg(long, value_name = "TEXT")]
    pub prompt: Option<String>,
    /// The other reviewers' one-line findings, asked what fixing them misses
    #[arg(long, value_name = "FILE")]
    pub others: Option<PathBuf>,
    /// Scope --diff to these paths
    #[arg(last = true, value_name = "PATHS")]
    pub paths: Vec<String>,
}

#[derive(Args, Debug, Clone)]
pub struct CollectArgs {
    /// Review `<branch>...HEAD`
    #[arg(long, value_name = "BRANCH", allow_hyphen_values = true)]
    pub base: Option<String>,
    /// Write the file here instead of stdout
    #[arg(long, value_name = "FILE")]
    pub out: Option<PathBuf>,
}

const USAGE: &str = "usage: factory review second --diff <base> [--others <file>] [-- <paths>]\n       factory review second --doc <file> --prompt <text> [--others <file>]\n       factory review collect --base <branch> [--out <file>]";

enum Mode {
    Diff(String),
    Doc(PathBuf, String),
}

pub fn review(verb: &ReviewVerb, out: &mut dyn Write, err: &mut dyn Write) -> Result<i32> {
    match verb {
        ReviewVerb::Second(a) => second(a, out, err),
        ReviewVerb::Collect(a) => collect_file(a, out, err),
    }
}

fn usage(err: &mut dyn Write) -> Result<i32> {
    writeln!(err, "{USAGE}").map_err(io("stderr"))?;
    Ok(1)
}

fn load_review(err: &mut dyn Write) -> Result<config::Review> {
    let loaded = config::load(&paths::config_dir())?;
    for w in &loaded.warnings {
        writeln!(err, "factory: {w}").map_err(io("stderr"))?;
    }
    Ok(loaded.config.review)
}

fn collect_file(a: &CollectArgs, out: &mut dyn Write, err: &mut dyn Write) -> Result<i32> {
    // A base starting with `-` would reach git as an option.
    let Some(base) = a.base.as_deref().filter(|b| !b.starts_with('-')) else {
        return usage(err);
    };
    let cfg = load_review(err)?;
    let cwd = std::env::current_dir().map_err(io("read the current directory"))?;
    let root = paths::repo_root(&cwd)?;
    let text = collect::collect(&root, base, &cfg, err)?;
    match &a.out {
        Some(p) => fs::write(p, text).map_err(io(format!("write {}", p.display())))?,
        None => out.write_all(text.as_bytes()).map_err(io("stdout"))?,
    }
    Ok(0)
}

fn second(a: &SecondArgs, out: &mut dyn Write, err: &mut dyn Write) -> Result<i32> {
    let mode = match (&a.diff, &a.doc, &a.prompt) {
        // A base starting with `-` would reach git as an option.
        (Some(base), None, None) if !base.starts_with('-') => Mode::Diff(base.clone()),
        (None, Some(doc), Some(text)) if a.paths.is_empty() => Mode::Doc(doc.clone(), text.clone()),
        _ => return usage(err),
    };
    let others = match &a.others {
        Some(p) => Some(fs::read_to_string(p).map_err(io(format!("read {}", p.display())))?),
        None => None,
    };
    let cfg = load_review(err)?;
    let cwd = std::env::current_dir().map_err(io("read the current directory"))?;

    // What each pass covers, before anything is spent.
    let (root, passes) = match &mode {
        Mode::Diff(base) => {
            let root = paths::repo_root(&cwd)?;
            let files = review::numstat(&cwd, base, &a.paths)?;
            if files.is_empty() {
                writeln!(out, "unreviewed: no changes against {base}").map_err(io("stdout"))?;
                return Ok(2);
            }
            (root, review::group(&files, cfg.pass_lines))
        }
        Mode::Doc(doc, _) => {
            let text = fs::read_to_string(doc).map_err(io(format!("read {}", doc.display())))?;
            let pass = Pass {
                files: vec![doc.display().to_string()],
                lines: text.lines().count() as u64,
                over: false,
            };
            (paths::repo_root(&cwd).unwrap_or(cwd.clone()), vec![pass])
        }
    };

    let dir = review::new_log_dir(&paths::data_dir())?;
    if let Err(reason) = review::probe(&cfg, &dir) {
        writeln!(out, "unreviewed: {reason}\n\nlog: {}", dir.display()).map_err(io("stdout"))?;
        return Ok(2);
    }

    let ran = review::run_passes(
        &cfg,
        &root,
        &dir,
        &passes,
        |n, pass| prompt(&mode, &root, &dir, others.as_deref(), n, pass),
        err,
    )?;
    let mut reviewed = 0;
    for (i, (pass, ran)) in passes.iter().zip(&ran).enumerate() {
        let (text, counts) = review::pass_text(ran, cfg.timeout_secs);
        reviewed += usize::from(counts);
        let header = review::pass_header(i + 1, pass, ran, &cfg);
        writeln!(out, "## {header}\n\n{text}").map_err(io("stdout"))?;
        out.flush().map_err(io("stdout"))?;
    }
    writeln!(out, "log: {}", dir.display()).map_err(io("stdout"))?;
    Ok(if reviewed > 0 { 0 } else { 2 })
}

fn prompt(
    mode: &Mode,
    root: &Path,
    dir: &Path,
    others: Option<&str>,
    n: usize,
    pass: &Pass,
) -> Result<String> {
    let prompt = match mode {
        Mode::Diff(base) => review::diff_pass_prompt(root, base, dir, n, pass)?,
        Mode::Doc(doc, text) => {
            let abs = fs::canonicalize(doc).map_err(io(format!("resolve {}", doc.display())))?;
            review::doc_prompt(text, &abs)
        }
    };
    Ok(match others {
        Some(o) => review::with_others(&prompt, o),
        None => prompt,
    })
}
