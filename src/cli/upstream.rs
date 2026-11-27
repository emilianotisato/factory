//! `check` exits 1 when the draft would leak a home path, this repo's name or an email, 0 otherwise; a failed issue search is one line, not a hit.
//! `post` exits 1 on a hit, else opens an issue on the factory repo with `gh`.
//! Both refuse inside a run, where the driver owns every GitHub call.
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use clap::Subcommand;

use crate::upstream::{self, Draft, REPO};
use crate::{Error, Result, ci, paths, worker};

#[derive(Subcommand, Debug, Clone)]
pub enum UpstreamVerb {
    /// Report what the draft would leak, then the open issues it may repeat
    Check {
        /// A markdown file: its first `# ` line is the title
        draft: PathBuf,
        #[arg(long = "allow", value_name = "WORD")]
        allow: Vec<String>,
    },
    /// Check again, then file the draft as a factory idea or issue
    Post {
        /// A markdown file: its first `# ` line is the title
        draft: PathBuf,
        #[arg(long = "allow", value_name = "WORD")]
        allow: Vec<String>,
        #[arg(long)]
        hash: Option<String>,
    },
}

pub fn upstream(verb: &UpstreamVerb, out: &mut dyn Write, err: &mut dyn Write) -> Result<i32> {
    if std::env::var_os(worker::BOARD_ENV).is_some_and(|v| !v.is_empty()) {
        writeln!(
            err,
            "factory upstream never runs inside a run: the driver is the one that talks to GitHub; run it from your own shell after the run"
        )
        .map_err(crate::error::io("write stderr"))?;
        return Ok(1);
    }
    let cwd = std::env::current_dir().map_err(crate::error::io("read the current directory"))?;
    let repo = paths::repo_root(&cwd).unwrap_or(cwd);
    match verb {
        UpstreamVerb::Check { draft, allow } => check(&repo, draft, allow, out),
        UpstreamVerb::Post { draft, allow, hash } => {
            post(&repo, draft, allow, hash.as_deref(), out)
        }
    }
}

fn say(out: &mut dyn Write, text: &str) -> Result<()> {
    writeln!(out, "{text}").map_err(crate::error::io("write stdout"))
}

fn screen(
    repo: &Path,
    file: &Path,
    allow: &[String],
    out: &mut dyn Write,
) -> Result<(Draft, String, bool)> {
    let text =
        fs::read_to_string(file).map_err(crate::error::io(format!("read {}", file.display())))?;
    let draft = upstream::parse(&text)?;
    let mut repo_names = upstream::names(repo);
    if !allow.is_empty() {
        let skip: Vec<String> = allow.iter().map(|a| a.to_lowercase()).collect();
        repo_names.retain(|n| !skip.contains(&n.to_lowercase()));
    }
    let hits = upstream::hits(&text, &repo_names, &upstream::homes());
    for h in &hits {
        say(out, &format!("{}:{}: {}", file.display(), h.line, h.what))?;
    }
    Ok((draft, text, !hits.is_empty()))
}

fn check(repo: &Path, file: &Path, allow: &[String], out: &mut dyn Write) -> Result<i32> {
    let (draft, text, hit) = screen(repo, file, allow, out)?;
    match upstream::search(repo, &draft.title) {
        Err(why) => say(out, &format!("issue search on {REPO} failed: {why}"))?,
        Ok(found) if found.is_empty() => say(out, &format!("no open issue on {REPO} matches"))?,
        Ok(found) => {
            say(
                out,
                &format!("open issues on {REPO} that may match; comment there instead of posting:"),
            )?;
            for i in found {
                say(out, &format!("  #{} {}  {}", i.number, i.title, i.url))?;
            }
        }
    }
    if !hit {
        say(out, &format!("hash: {}", upstream::draft_hash(&text)))?;
    }
    Ok(i32::from(hit))
}

fn post(
    repo: &Path,
    file: &Path,
    allow: &[String],
    hash: Option<&str>,
    out: &mut dyn Write,
) -> Result<i32> {
    let (draft, text, hit) = screen(repo, file, allow, out)?;
    if hit {
        say(out, "not posted: rewrite the lines above in general terms")?;
        return Ok(1);
    }
    let expected = upstream::draft_hash(&text);
    if hash != Some(expected.as_str()) {
        say(
            out,
            &format!(
                "not posted: --hash missing or does not match draft — run `factory upstream check {}` and pass --hash {expected}",
                file.display()
            ),
        )?;
        return Ok(1);
    }
    if !allow.is_empty() {
        say(out, &format!("allowed: {}", allow.join(", ")))?;
    }
    open_issue(repo, &draft, out)
}

fn open_issue(repo: &Path, draft: &Draft, out: &mut dyn Write) -> Result<i32> {
    let o = ci::gh_stdin(
        repo,
        &[
            "issue",
            "create",
            "--repo",
            REPO,
            "--title",
            &draft.title,
            "--body-file",
            "-",
        ],
        draft.body.as_bytes(),
    )?;
    if !o.ok() {
        return Err(Error::Tool {
            cmd: format!("gh issue create --repo {REPO}"),
            detail: o.text(),
        });
    }
    let text = o.text();
    say(
        out,
        if text.is_empty() {
            "issue created"
        } else {
            &text
        },
    )?;
    Ok(0)
}
