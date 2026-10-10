//! The CI leg of the closeout (PRD §8 FR-6): `gh pr checks --watch` on the
//! draft PR, the failing check and its run from `gh run list`, the one flaky
//! rerun and the log a fix worker reads. Every `gh` call runs in the repo
//! root so the test harness's stub (`tests/stubs/gh`) answers; nothing here
//! merges or marks a PR ready. The caps are today's prose: three fix rounds
//! (`autonomy.md:77`), one rerun (`autonomy.md:78`), a red that is not this
//! branch's blocks at once (`autonomy.md:79`).
use std::path::Path;
use std::process::Command;

use serde::Deserialize;

use crate::error::io;
use crate::{Error, Result};

/// Fix rounds on a red PR before the run blocks (`autonomy.md:77`).
pub const CI_FIX_ROUNDS: u32 = 3;
/// Reruns of a flaky check at one head before the run blocks (`autonomy.md:78`).
pub const CI_FLAKY_RERUNS: u32 = 1;
/// Seconds between `gh pr checks --watch` polls; the env var lets a test
/// shorten it without a config key.
pub const INTERVAL_ENV: &str = "FACTORY_CI_INTERVAL";
pub const DEFAULT_INTERVAL: u64 = 60;

pub fn interval() -> u64 {
    std::env::var(INTERVAL_ENV)
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .filter(|n| *n > 0)
        .unwrap_or(DEFAULT_INTERVAL)
}

/// What a `gh` or `git` call printed and how it exited.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Output {
    pub stdout: String,
    pub stderr: String,
    pub code: i32,
}

impl Output {
    pub fn ok(&self) -> bool {
        self.code == 0
    }

    /// stderr trimmed, or stdout when stderr is empty: the line a block quotes.
    pub fn text(&self) -> String {
        let err = self.stderr.trim();
        if err.is_empty() {
            self.stdout.trim().to_string()
        } else {
            err.to_string()
        }
    }
}

fn exec(repo: &Path, prog: &str, args: &[&str]) -> Result<Output> {
    let out = Command::new(prog)
        .args(args)
        .current_dir(repo)
        .output()
        .map_err(io(format!("start {prog} {}", args.join(" "))))?;
    Ok(Output {
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        code: out.status.code().unwrap_or(-1),
    })
}

/// `gh <args>` in the repo root, whatever it exits.
pub fn gh(repo: &Path, args: &[&str]) -> Result<Output> {
    exec(repo, "gh", args)
}

/// `git <args>` in the repo root, whatever it exits.
pub fn git(repo: &Path, args: &[&str]) -> Result<Output> {
    exec(repo, "git", args)
}

/// `git rev-parse HEAD`.
pub fn head(repo: &Path) -> Result<String> {
    let out = git(repo, &["rev-parse", "HEAD"])?;
    if !out.ok() {
        return Err(Error::Msg(format!("git rev-parse HEAD: {}", out.text())));
    }
    Ok(out.stdout.trim().to_string())
}

/// `git push -u origin <branch>` — plain, never forced. `Err(text)` is the
/// push refusing, for a block; the outer `Result` is git failing to start.
pub fn push(repo: &Path, branch: &str) -> Result<std::result::Result<(), String>> {
    let out = git(repo, &["push", "-u", "origin", branch])?;
    Ok(if out.ok() { Ok(()) } else { Err(out.text()) })
}

/// The verdict of one `gh pr checks <pr> --watch`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Checks {
    /// Exit 0: every check passed.
    Green,
    /// A check failed; `check` is the first `fail` row's name.
    Red { check: String, table: String },
    /// `gh` said no checks are reported on the branch: nothing to watch.
    None,
    /// A non-zero exit with no `fail` row to name: `gh`'s own text.
    Unreadable { text: String },
}

/// `gh pr checks <pr> --watch --interval <s>`: blocks until the checks
/// settle. The table is `name\tstatus\telapsed\turl`, one row a check.
pub fn watch(repo: &Path, pr: &str) -> Result<Checks> {
    let secs = interval().to_string();
    let out = gh(repo, &["pr", "checks", pr, "--watch", "--interval", &secs])?;
    if out.ok() {
        return Ok(Checks::Green);
    }
    if let Some(check) = failing_check(&out.stdout) {
        return Ok(Checks::Red {
            check,
            table: out.stdout.trim_end().to_string(),
        });
    }
    let text = out.text();
    if text.to_ascii_lowercase().contains("no checks") {
        return Ok(Checks::None);
    }
    Ok(Checks::Unreadable { text })
}

/// The name on the first `fail` row of a `gh pr checks` table.
pub fn failing_check(table: &str) -> Option<String> {
    table.lines().find_map(|l| {
        let mut cells = l.split('\t');
        let name = cells.next()?.trim();
        let status = cells.next()?.trim();
        (status.eq_ignore_ascii_case("fail") && !name.is_empty()).then(|| name.to_string())
    })
}

/// One row of `gh run list --json`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CiRun {
    pub database_id: u64,
    pub name: String,
    pub workflow_name: String,
    pub conclusion: String,
    pub head_sha: String,
}

/// `gh run list --branch <branch> --limit 20 --json …`, newest first.
pub fn runs(repo: &Path, branch: &str) -> Result<Vec<CiRun>> {
    let out = gh(
        repo,
        &[
            "run",
            "list",
            "--branch",
            branch,
            "--limit",
            "20",
            "--json",
            "databaseId,name,workflowName,conclusion,headSha",
        ],
    )?;
    if !out.ok() {
        return Err(Error::Msg(format!("gh run list: {}", out.text())));
    }
    serde_json::from_str(&out.stdout)
        .map_err(|e| Error::Msg(format!("gh run list: not the expected JSON ({e})")))
}

/// The failed run behind `check`: the newest failure whose name or workflow
/// is the check, else the newest failure at all.
pub fn failed_run<'a>(runs: &'a [CiRun], check: &str) -> Option<&'a CiRun> {
    let failed = || runs.iter().filter(|r| r.conclusion == "failure");
    failed()
        .find(|r| r.name == check || r.workflow_name == check)
        .or_else(|| failed().next())
}

/// A green run at `head` beside the red one: the same tree passed once, so
/// the red is the check's, not the tree's (`autonomy.md:78`).
pub fn green_at(runs: &[CiRun], head: &str) -> bool {
    runs.iter()
        .any(|r| r.head_sha == head && r.conclusion == "success")
}

/// `gh run view <id> --log-failed`: the failing steps' log, or `gh`'s text.
pub fn log_failed(repo: &Path, id: u64) -> Result<String> {
    let out = gh(repo, &["run", "view", &id.to_string(), "--log-failed"])?;
    Ok(if out.stdout.trim().is_empty() {
        out.text()
    } else {
        out.stdout
    })
}

/// `gh run rerun <id> --failed` (`autonomy.md:78`).
pub fn rerun(repo: &Path, id: u64) -> Result<std::result::Result<(), String>> {
    let out = gh(repo, &["run", "rerun", &id.to_string(), "--failed"])?;
    Ok(if out.ok() { Ok(()) } else { Err(out.text()) })
}

/// `gh pr create --draft --base <base> --title <title> --body-file <body>`;
/// `Ok(url)` is the last non-empty stdout line.
pub fn pr_create(
    repo: &Path,
    base: &str,
    title: &str,
    body: &Path,
) -> Result<std::result::Result<String, String>> {
    let body = body.to_string_lossy().into_owned();
    let out = gh(
        repo,
        &[
            "pr",
            "create",
            "--draft",
            "--base",
            base,
            "--title",
            title,
            "--body-file",
            &body,
        ],
    )?;
    if !out.ok() {
        return Ok(Err(out.text()));
    }
    Ok(out
        .stdout
        .lines()
        .map(str::trim)
        .rfind(|l| !l.is_empty())
        .map(str::to_string)
        .ok_or_else(|| "gh pr create printed no URL".to_string()))
}

/// `gh pr view <branch> --json url,state`: the open PR of `branch`, when
/// there is one (a new run on a shipped feature finds the PR the last run
/// opened); `None` when `gh` knows none or the PR is closed or merged.
pub fn pr_view(repo: &Path, branch: &str) -> Result<Option<String>> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct View {
        url: String,
        #[serde(default)]
        state: String,
    }
    let out = gh(repo, &["pr", "view", branch, "--json", "url,state"])?;
    if !out.ok() {
        return Ok(None);
    }
    let view: View = match serde_json::from_str(out.stdout.trim()) {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };
    Ok((view.state.eq_ignore_ascii_case("open") && !view.url.is_empty()).then_some(view.url))
}

/// `gh pr edit <pr> --body-file <body>`: a resume refreshes the body instead
/// of opening a second PR.
pub fn pr_edit(repo: &Path, pr: &str, body: &Path) -> Result<std::result::Result<(), String>> {
    let body = body.to_string_lossy().into_owned();
    let out = gh(repo, &["pr", "edit", pr, "--body-file", &body])?;
    Ok(if out.ok() { Ok(()) } else { Err(out.text()) })
}

/// The PR number from its URL (`…/pull/12` → `12`); the URL itself when the
/// last segment is not a number, which `gh` also accepts.
pub fn pr_number(url: &str) -> String {
    let last = url.trim_end_matches('/').rsplit('/').next().unwrap_or("");
    if !last.is_empty() && last.chars().all(|c| c.is_ascii_digit()) {
        last.to_string()
    } else {
        url.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_fail_row_names_the_check() {
        let table = "build\tpass\t1m2s\thttps://x/1\ntest\tfail\t3s\thttps://x/2\nlint\tfail\t1s\thttps://x/3\n";
        assert_eq!(failing_check(table).as_deref(), Some("test"));
        assert_eq!(failing_check("build\tpass\t1s\thttps://x/1\n"), None);
        assert_eq!(failing_check("no checks reported\n"), None);
    }

    #[test]
    fn the_failed_run_prefers_the_named_check_then_the_newest_failure() {
        let runs = vec![
            CiRun {
                database_id: 3,
                name: "lint".into(),
                workflow_name: "ci".into(),
                conclusion: "failure".into(),
                head_sha: "a".into(),
            },
            CiRun {
                database_id: 2,
                name: "test".into(),
                workflow_name: "ci".into(),
                conclusion: "failure".into(),
                head_sha: "a".into(),
            },
            CiRun {
                database_id: 1,
                name: "test".into(),
                workflow_name: "ci".into(),
                conclusion: "success".into(),
                head_sha: "a".into(),
            },
        ];
        assert_eq!(failed_run(&runs, "test").map(|r| r.database_id), Some(2));
        assert_eq!(failed_run(&runs, "other").map(|r| r.database_id), Some(3));
        assert!(green_at(&runs, "a"));
        assert!(!green_at(&runs, "b"));
    }

    #[test]
    fn the_pr_number_is_the_last_numeric_segment() {
        assert_eq!(pr_number("https://github.com/o/r/pull/12"), "12");
        assert_eq!(pr_number("https://example.invalid/pr/1/"), "1");
        assert_eq!(pr_number("https://x/pr/abc"), "https://x/pr/abc");
    }

    #[test]
    fn the_interval_defaults_and_reads_the_env() {
        // Not set in the test environment; a bad value falls back too.
        assert_eq!(DEFAULT_INTERVAL, 60);
        assert_eq!(CI_FIX_ROUNDS, 3);
        assert_eq!(CI_FLAKY_RERUNS, 1);
    }
}
