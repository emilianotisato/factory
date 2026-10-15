//! `check` exits 1 when the draft would leak a home path, this repo's name or an email, 0 otherwise; a failed issue search is one line, not a hit.
//! `post` exits 1 on a hit, else adds the draft as an idea in the `[source]` checkout and commits it, or opens an issue with `gh`.
//! Both refuse inside a run, where the driver owns every GitHub call.
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use clap::Subcommand;

use crate::docs::{ideas, scaffold};
use crate::error::io;
use crate::upstream::{self, Draft, REPO};
use crate::{Error, Result, ci, commit, config, paths, worker};

#[derive(Subcommand, Debug, Clone)]
pub enum UpstreamVerb {
    /// Report what the draft would leak, then the open issues it may repeat
    Check {
        /// A markdown file: its first `# ` line is the title
        draft: PathBuf,
    },
    /// Check again, then file the draft as a factory idea or issue
    Post {
        /// A markdown file: its first `# ` line is the title
        draft: PathBuf,
    },
}

const ORIGIN: &str = "factory upstream";

pub fn upstream(verb: &UpstreamVerb, out: &mut dyn Write, err: &mut dyn Write) -> Result<i32> {
    if std::env::var_os(worker::BOARD_ENV).is_some_and(|v| !v.is_empty()) {
        writeln!(
            err,
            "factory upstream never runs inside a run: the driver is the one that talks to GitHub; run it from your own shell after the run"
        )
        .map_err(io("write stderr"))?;
        return Ok(1);
    }
    let cwd = std::env::current_dir().map_err(io("read the current directory"))?;
    let repo = paths::repo_root(&cwd).unwrap_or(cwd);
    match verb {
        UpstreamVerb::Check { draft } => check(&repo, draft, out),
        UpstreamVerb::Post { draft } => post(&repo, draft, out),
    }
}

fn say(out: &mut dyn Write, text: &str) -> Result<()> {
    writeln!(out, "{text}").map_err(io("write stdout"))
}

/// The draft, and whether anything hit, after printing one line per hit.
fn screen(repo: &Path, file: &Path, out: &mut dyn Write) -> Result<(Draft, bool)> {
    let text = fs::read_to_string(file).map_err(io(format!("read {}", file.display())))?;
    let draft = upstream::parse(&text)?;
    let hits = upstream::hits(&text, &upstream::names(repo));
    for h in &hits {
        say(out, &format!("{}:{}: {}", file.display(), h.line, h.what))?;
    }
    Ok((draft, !hits.is_empty()))
}

fn check(repo: &Path, file: &Path, out: &mut dyn Write) -> Result<i32> {
    let (draft, hit) = screen(repo, file, out)?;
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
    Ok(i32::from(hit))
}

fn post(repo: &Path, file: &Path, out: &mut dyn Write) -> Result<i32> {
    let (draft, hit) = screen(repo, file, out)?;
    if hit {
        say(out, "not posted: rewrite the lines above in general terms")?;
        return Ok(1);
    }
    let loaded = config::load(&paths::config_dir())?;
    match loaded.config.source {
        Some(src) => add_idea(&src.path, draft, out),
        None => open_issue(repo, &draft, out),
    }
}

fn add_idea(checkout: &Path, draft: Draft, out: &mut dyn Write) -> Result<i32> {
    let idea = ideas::Idea {
        title: draft.title,
        state: "open".into(),
        added: scaffold::today(),
        origin: Some(ORIGIN.into()),
        retrigger: None,
        priority: None,
        body: draft.body,
    };
    let id = ideas::add(checkout, &idea)?;
    let rel = format!("{}/{id}.md", ideas::DIR);
    commit::commit(
        checkout,
        std::slice::from_ref(&rel),
        &format!("docs: add idea {id}"),
    )?;
    say(
        out,
        &format!("added {} and committed it", checkout.join(&rel).display()),
    )?;
    Ok(0)
}

fn open_issue(repo: &Path, draft: &Draft, out: &mut dyn Write) -> Result<i32> {
    let o = ci::gh(
        repo,
        &[
            "issue",
            "create",
            "--repo",
            REPO,
            "--title",
            &draft.title,
            "--body",
            &draft.body,
        ],
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
