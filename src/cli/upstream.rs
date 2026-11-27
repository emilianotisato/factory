//! `check` exits 1 when the draft would leak a home path, this repo's name or an email, 0 otherwise; a failed issue search is one line, not a hit.
//! `post` exits 1 on a hit, else opens an issue on the factory repo with `gh`.
//! Both refuse inside a run, where the driver owns every GitHub call.
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use clap::{Args, Subcommand};

use crate::commit;
use crate::docs::{ideas, scaffold};
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
    /// List open issues on the factory repo and triage them into ideas
    Pull(PullArgs),
}

#[derive(Args, Debug, Clone)]
pub struct PullArgs {
    #[arg(long, conflicts_with_all = ["idea", "fold", "drop"])]
    pub json: bool,
    #[arg(
        long,
        value_name = "NUMBER",
        conflicts_with_all = ["json", "fold", "drop"]
    )]
    pub idea: Option<u64>,
    #[arg(
        long,
        num_args = 2,
        value_names = ["NUMBER", "IDEA_ID"],
        conflicts_with_all = ["json", "idea", "drop"]
    )]
    pub fold: Option<Vec<String>>,
    #[arg(
        long,
        value_name = "NUMBER",
        requires = "because",
        conflicts_with_all = ["json", "idea", "fold"]
    )]
    pub drop: Option<u64>,
    #[arg(long, value_name = "REASON", requires = "drop")]
    pub because: Option<String>,
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
        UpstreamVerb::Pull(args) => pull(&repo, args, out, err),
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
                "not posted: --hash missing or does not match draft — run `factory upstream check {}` and pass the hash it prints",
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

fn pull(repo: &Path, args: &PullArgs, out: &mut dyn Write, err: &mut dyn Write) -> Result<i32> {
    if let Some(number) = args.idea {
        return pull_idea(repo, number, out, err);
    }
    if let Some(parts) = &args.fold {
        return pull_fold(repo, parts, out, err);
    }
    if let Some(number) = args.drop {
        let because = args.because.as_deref().unwrap_or("");
        return pull_drop(repo, number, because, out, err);
    }
    pull_list(repo, args.json, out)
}

fn pull_list(repo: &Path, json: bool, out: &mut dyn Write) -> Result<i32> {
    let issues = upstream::list_open(repo)?;
    if json {
        let text = serde_json::to_string_pretty(&issues)
            .map_err(|e| Error::Msg(format!("print issue list: {e}")))?;
        say(out, &text)?;
    } else if issues.is_empty() {
        say(out, "no open upstream issues")?;
    } else {
        for i in issues {
            say(out, &format!("#{}  {}  {}", i.number, i.title, i.url))?;
        }
    }
    Ok(0)
}

fn require_ideas_dir(repo: &Path, err: &mut dyn Write) -> Result<()> {
    if repo.join(ideas::DIR).is_dir() {
        return Ok(());
    }
    writeln!(
        err,
        "factory upstream pull needs {} in this checkout",
        ideas::DIR
    )
    .map_err(crate::error::io("write stderr"))?;
    Err(Error::Msg(format!("{} missing", ideas::DIR)))
}

fn pull_idea(repo: &Path, number: u64, out: &mut dyn Write, err: &mut dyn Write) -> Result<i32> {
    if require_ideas_dir(repo, err).is_err() {
        return Ok(1);
    }
    let issue = upstream::view_issue(repo, number)?;
    let idea = ideas::Idea {
        title: issue.title.clone(),
        state: "open".into(),
        added: scaffold::today(),
        origin: Some(issue.url.clone()),
        retrigger: None,
        priority: None,
        body: issue.body.clone(),
    };
    let id = ideas::add(repo, &idea)?;
    let rel = format!("{}/{}.md", ideas::DIR, id);
    let commit_msg = format!("docs: add idea {id} from #{number}");
    match commit::commit(repo, std::slice::from_ref(&rel), &commit_msg) {
        Ok(None) => {
            let _ = fs::remove_file(repo.join(&rel));
            return Err(Error::Msg(format!(
                "commit refused: nothing staged for {rel}"
            )));
        }
        Ok(Some(_)) => {}
        Err(e) => {
            let _ = fs::remove_file(repo.join(&rel));
            return Err(e);
        }
    }
    upstream::comment_issue(repo, number, &format!("Tracked as `{rel}`."))?;
    upstream::close_issue(repo, number, "completed")?;
    say(out, &format!("{rel} tracks #{number}"))?;
    Ok(0)
}

fn pull_fold(
    repo: &Path,
    parts: &[String],
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<i32> {
    let number: u64 = parts[0]
        .parse()
        .map_err(|_| Error::Msg(format!("issue number {:?} is not a number", parts[0])))?;
    let idea_id = &parts[1];
    if require_ideas_dir(repo, err).is_err() {
        return Ok(1);
    }
    let (id, rel) = ideas::locate(repo, idea_id)?;
    let issue = upstream::view_issue(repo, number)?;
    let path = repo.join(&rel);
    let prev = fs::read_to_string(&path).map_err(crate::error::io(format!("read {}", rel)))?;
    let today = scaffold::today();
    let section = format!(
        "\n## From #{number} ({}) ({today})\n\n### {}\n\n{}\n",
        issue.url,
        issue.title.trim(),
        issue.body.trim()
    );
    let mut next = prev.trim_end().to_string();
    next.push_str(&section);
    fs::write(&path, &next).map_err(crate::error::io(format!("write {}", rel)))?;
    let commit_msg = format!("docs: fold #{number} into idea {id}");
    match commit::commit(repo, std::slice::from_ref(&rel), &commit_msg) {
        Ok(None) => {
            fs::write(&path, &prev).map_err(crate::error::io(format!("write {}", rel)))?;
            return Err(Error::Msg(format!(
                "commit refused: nothing staged for {rel}"
            )));
        }
        Ok(Some(_)) => {}
        Err(e) => {
            fs::write(&path, &prev).map_err(crate::error::io(format!("write {}", rel)))?;
            return Err(e);
        }
    }
    upstream::comment_issue(repo, number, &format!("Folded into `{rel}`."))?;
    upstream::close_issue(repo, number, "completed")?;
    say(out, &format!("folded #{number} into {rel}"))?;
    Ok(0)
}

fn pull_drop(
    repo: &Path,
    number: u64,
    because: &str,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<i32> {
    if because.trim().is_empty() {
        writeln!(err, "--because must not be empty").map_err(crate::error::io("write stderr"))?;
        return Ok(1);
    }
    let reason = because.trim();
    upstream::comment_issue(repo, number, &format!("Dropped: {reason}"))?;
    upstream::close_issue(repo, number, "not planned")?;
    say(out, &format!("dropped #{number}"))?;
    Ok(0)
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
