//! The flags are optional to clap: its usage error exits 2, which the skills read as "unreviewed", the safe side.
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

use clap::{Args, Subcommand};

use crate::config::Review;
use crate::error::io;
use crate::review::{self, Ended, Outcome, Pass};
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
        // A base starting with `-` would reach git as an option.
        (Some(base), None, None) if !base.starts_with('-') => Mode::Diff(base.clone()),
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
        writeln!(
            err,
            "review pass {n}/{}: {} ({} lines)",
            passes.len(),
            pass.files.join(", "),
            pass.lines
        )
        .map_err(io("stderr"))?;
        let started = Instant::now();
        let ran = run_one(&mode, &cfg, &root, &dir, others.as_deref(), n, pass);
        let elapsed = ran.as_ref().map_or(started.elapsed(), |o| o.elapsed);
        let (text, counts) = match &ran {
            Ok(o) => body(o, cfg.timeout_secs),
            Err(e) => (format!("unreviewed (error: {e})\n"), false),
        };
        reviewed += usize::from(counts);
        let over = if pass.over {
            format!(" (over {}, alone)", cfg.pass_lines)
        } else {
            String::new()
        };
        writeln!(
            out,
            "## Pass {n} — {}{over} ({} lines, {} s, {})\n\n{text}",
            pass.files.join(", "),
            pass.lines,
            elapsed.as_secs(),
            cfg.model
        )
        .map_err(io("stdout"))?;
        out.flush().map_err(io("stdout"))?;
    }
    writeln!(out, "log: {}", dir.display()).map_err(io("stdout"))?;
    Ok(if reviewed > 0 { 0 } else { 2 })
}

fn run_one(
    mode: &Mode,
    cfg: &Review,
    root: &Path,
    dir: &Path,
    others: Option<&str>,
    n: usize,
    pass: &Pass,
) -> Result<Outcome> {
    let prompt = match mode {
        Mode::Diff(base) => {
            let file = dir.join(format!("review-{n}.diff"));
            fs::write(&file, review::diff(root, base, &pass.files)?)
                .map_err(io(format!("write {}", file.display())))?;
            review::diff_prompt(&pass.files, base, &file)
        }
        Mode::Doc(doc, text) => {
            let abs = fs::canonicalize(doc).map_err(io(format!("resolve {}", doc.display())))?;
            review::doc_prompt(text, &abs)
        }
    };
    let prompt = match others {
        Some(o) => review::with_others(&prompt, o),
        None => prompt,
    };
    review::run_pass(cfg, root, dir, n, &prompt)
}

fn exit_words(code: Option<i32>) -> String {
    code.map_or("killed by a signal".to_string(), |c| format!("exit {c}"))
}

/// A kill's partial counts (it had the whole timeout); an exit's does not, or an opening line stops the fallback.
fn body(o: &Outcome, timeout_secs: u64) -> (String, bool) {
    let p = &o.parsed;
    if let Some(r) = p.result.as_deref().filter(|r| !r.trim().is_empty()) {
        return (format!("{}\n", r.trim_end()), true);
    }
    let partial = Some(p.partial.trim_end()).filter(|t| !t.trim().is_empty());
    let code = match o.ended {
        Ended::Killed => {
            return match partial {
                Some(t) => (
                    format!("(timed out after {timeout_secs} s, partial)\n\n{t}\n"),
                    true,
                ),
                None => (
                    format!("unreviewed (timed out after {timeout_secs} s with no text)\n"),
                    false,
                ),
            };
        }
        Ended::Exited(code) => code,
    };
    let words = exit_words(code);
    let mut text = match &p.error {
        Some(e) if e.trim().is_empty() => format!("unreviewed ({words}, error result)\n"),
        Some(e) => format!("unreviewed ({words}, error: {})\n", e.trim()),
        None if code == Some(0) => match partial {
            Some(t) => return (format!("{t}\n"), true),
            None => String::new(),
        },
        None => String::new(),
    };
    match partial {
        Some(t) => {
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str(&format!("({words}, partial)\n\n{t}\n"));
        }
        None if text.is_empty() => {
            let why = if o.stderr.is_empty() {
                String::new()
            } else {
                format!(": {}", o.stderr)
            };
            text = format!("unreviewed ({words}, no text{why})\n");
        }
        None => {}
    }
    (text, false)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::review::parse_stream;

    const OPENING: &str = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"I'll read the diff."}]}}"#;
    const DONE: &str = r#"{"type":"result","is_error":false,"result":"a.rs:1 breaks"}"#;
    const FAILED: &str = r#"{"type":"result","is_error":true,"result":"rate limited"}"#;

    fn outcome(ended: Ended, stream: &[&str], stderr: &str) -> Outcome {
        Outcome {
            ended,
            elapsed: Duration::ZERO,
            parsed: parse_stream(&stream.join("\n")),
            stderr: stderr.to_string(),
        }
    }

    #[test]
    fn each_ending_has_its_marker_and_only_text_that_finished_counts() {
        let cases = [
            (
                Ended::Exited(Some(0)),
                vec![OPENING, DONE],
                "a.rs:1 breaks\n",
                true,
            ),
            (
                Ended::Exited(Some(1)),
                vec![OPENING, DONE],
                "a.rs:1 breaks\n",
                true,
            ),
            (
                Ended::Exited(Some(0)),
                vec![OPENING],
                "I'll read the diff.\n",
                true,
            ),
            (
                Ended::Killed,
                vec![OPENING],
                "(timed out after 9 s, partial)\n\nI'll read the diff.\n",
                true,
            ),
            (
                Ended::Exited(Some(1)),
                vec![OPENING],
                "(exit 1, partial)\n\nI'll read the diff.\n",
                false,
            ),
            (
                Ended::Exited(None),
                vec![OPENING],
                "(killed by a signal, partial)\n\nI'll read the diff.\n",
                false,
            ),
            (
                Ended::Exited(Some(1)),
                vec![FAILED],
                "unreviewed (exit 1, error: rate limited)\n",
                false,
            ),
            (
                Ended::Exited(Some(0)),
                vec![OPENING, FAILED],
                "unreviewed (exit 0, error: rate limited)\n\n(exit 0, partial)\n\nI'll read the diff.\n",
                false,
            ),
            (
                Ended::Exited(Some(2)),
                vec![],
                "unreviewed (exit 2, no text: boom)\n",
                false,
            ),
            (
                Ended::Killed,
                vec![],
                "unreviewed (timed out after 9 s with no text)\n",
                false,
            ),
        ];
        for (ended, stream, want, counts) in cases {
            let got = body(&outcome(ended, &stream, "boom"), 9);
            assert_eq!(got, (want.to_string(), counts), "{ended:?} {stream:?}");
        }
    }
}
