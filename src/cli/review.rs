//! `factory review second` (PRD-004 FR-6): `--diff <base> [-- <paths>]` for
//! `/polish`, `--doc <file> --prompt <text>` for `/prd`. One markdown section
//! per pass on stdout, the log dir last. Exit 0 when a pass returned text, 2
//! when none did (the skills' fallback starts), 1 on a usage error.
//!
//! Every flag is optional to clap and the combination is checked here: clap's
//! own usage error exits 2, which the skills read as "unreviewed". A
//! misspelled flag still exits 2 through clap, the safe side.
use std::fs;
use std::io::Write;
use std::path::PathBuf;

use clap::{Args, Subcommand};

use crate::error::io;
use crate::review::{self, Ended, Pass};
use crate::{Result, config, paths};

#[derive(Subcommand, Debug, Clone)]
pub enum ReviewVerb {
    /// The cross-vendor second opinion over a diff or a document
    Second(SecondArgs),
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

const USAGE: &str = "usage: factory review second --diff <base> [--others <file>] [-- <paths>]\n       factory review second --doc <file> --prompt <text> [--others <file>]";

enum Mode {
    Diff(String),
    Doc(PathBuf, String),
}

pub fn review(verb: &ReviewVerb, out: &mut dyn Write, err: &mut dyn Write) -> Result<i32> {
    let ReviewVerb::Second(a) = verb;
    let mode = match (&a.diff, &a.doc, &a.prompt) {
        (Some(base), None, None) => Mode::Diff(base.clone()),
        (None, Some(doc), Some(text)) if a.paths.is_empty() => Mode::Doc(doc.clone(), text.clone()),
        _ => {
            writeln!(err, "{USAGE}").map_err(io("stderr"))?;
            return Ok(1);
        }
    };
    let others = match &a.others {
        Some(p) => Some(fs::read_to_string(p).map_err(io(format!("read {}", p.display())))?),
        None => None,
    };
    let loaded = config::load(&paths::config_dir())?;
    for w in &loaded.warnings {
        writeln!(err, "factory: {w}").map_err(io("stderr"))?;
    }
    let cfg = loaded.config.review;
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

    let mut reviewed = 0;
    for (i, pass) in passes.iter().enumerate() {
        let n = i + 1;
        let prompt = match &mode {
            Mode::Diff(base) => {
                let file = dir.join(format!("review-{n}.diff"));
                fs::write(&file, review::diff(&root, base, &pass.files)?)
                    .map_err(io(format!("write {}", file.display())))?;
                review::diff_prompt(&pass.files, base, &file)
            }
            Mode::Doc(doc, text) => {
                let abs =
                    fs::canonicalize(doc).map_err(io(format!("resolve {}", doc.display())))?;
                review::doc_prompt(text, &abs)
            }
        };
        let prompt = match &others {
            Some(o) => review::with_others(&prompt, o),
            None => prompt,
        };
        writeln!(
            err,
            "review pass {n}/{}: {} ({} lines)",
            passes.len(),
            pass.files.join(", "),
            pass.lines
        )
        .map_err(io("stderr"))?;
        let o = review::run_pass(&cfg, &root, &dir, n, &prompt)?;
        let over = if pass.over {
            format!(" (over {}, alone)", cfg.pass_lines)
        } else {
            String::new()
        };
        let mut section = format!(
            "## Pass {n} — {}{over} ({} lines, {} s, {})\n\n",
            pass.files.join(", "),
            pass.lines,
            o.elapsed.as_secs(),
            cfg.model
        );
        match (o.parsed.text(), o.ended) {
            (Some(text), Ended::Killed) => {
                reviewed += 1;
                section.push_str(&format!(
                    "(timed out after {} s, partial)\n\n{}\n",
                    cfg.timeout_secs,
                    text.trim_end()
                ));
            }
            (Some(text), _) => {
                reviewed += 1;
                section.push_str(&format!("{}\n", text.trim_end()));
            }
            (None, Ended::Killed) => section.push_str(&format!(
                "unreviewed (timed out after {} s with no text)\n",
                cfg.timeout_secs
            )),
            (None, Ended::Exited(code)) => {
                let code = code.map_or("a signal".to_string(), |c| c.to_string());
                let why = if o.stderr.is_empty() {
                    String::new()
                } else {
                    format!(": {}", o.stderr)
                };
                section.push_str(&format!("unreviewed (exit {code}, no text{why})\n"));
            }
        }
        writeln!(out, "{section}").map_err(io("stdout"))?;
        out.flush().map_err(io("stdout"))?;
    }
    writeln!(out, "log: {}", dir.display()).map_err(io("stdout"))?;
    Ok(if reviewed > 0 { 0 } else { 2 })
}
