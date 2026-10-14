//! Every `gh` call runs in the repo root: the test stub reads its scenario from the cwd.
use std::collections::{BTreeSet, HashSet};
use std::path::Path;
use std::process::Command;

use serde::{Deserialize, Deserializer};

use crate::error::io;
use crate::{Error, Result};

/// Fix rounds on a red PR before the run blocks.
pub const CI_FIX_ROUNDS: u32 = 3;
/// Reruns of a flaky check at one head before the run blocks.
pub const CI_FLAKY_RERUNS: u32 = 1;
/// Seconds between polls; an env var so a test can shorten it without a config key.
pub const INTERVAL_ENV: &str = "FACTORY_CI_INTERVAL";
pub const DEFAULT_INTERVAL: u64 = 60;
/// Seconds the checks may stay pending, counted from a push, a rerun or the watch's start.
pub const TIMEOUT_ENV: &str = "FACTORY_CI_TIMEOUT";
pub const DEFAULT_TIMEOUT: u64 = 6 * 60 * 60;
/// Polls after a push, a rerun or the watch's start during which a missing check means "not registered yet".
pub const GRACE_POLLS: u64 = 5;

fn env_secs(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .filter(|n| *n > 0)
        .unwrap_or(default)
}

pub fn interval() -> u64 {
    env_secs(INTERVAL_ENV, DEFAULT_INTERVAL)
}

pub fn timeout() -> u64 {
    env_secs(TIMEOUT_ENV, DEFAULT_TIMEOUT)
}

pub fn grace() -> u64 {
    GRACE_POLLS * interval()
}

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

pub fn gh(repo: &Path, args: &[&str]) -> Result<Output> {
    exec(repo, "gh", args)
}

pub fn git(repo: &Path, args: &[&str]) -> Result<Output> {
    exec(repo, "git", args)
}

pub fn head(repo: &Path) -> Result<String> {
    let out = git(repo, &["rev-parse", "HEAD"])?;
    if !out.ok() {
        return Err(Error::Msg(format!("git rev-parse HEAD: {}", out.text())));
    }
    Ok(out.stdout.trim().to_string())
}

pub fn head_branch(repo: &Path) -> Result<Option<String>> {
    let out = git(repo, &["symbolic-ref", "--quiet", "--short", "HEAD"])?;
    let name = out.stdout.trim();
    Ok((out.ok() && !name.is_empty()).then(|| name.to_string()))
}

pub fn branch_problem(branch: &str, default: &str, head: Option<&str>) -> Option<String> {
    if branch.is_empty() || branch == "HEAD" {
        return Some("the run names no feature branch".to_string());
    }
    if branch == default {
        return Some(format!("the run's branch `{branch}` is the default branch"));
    }
    match head {
        Some(h) if h == branch => None,
        Some(h) => Some(format!(
            "HEAD is on `{h}`, not the feature branch `{branch}`"
        )),
        None => Some(format!(
            "HEAD is detached, not on the feature branch `{branch}`"
        )),
    }
}

pub fn push(repo: &Path, branch: &str) -> Result<std::result::Result<(), String>> {
    let out = git(repo, &["push", "-u", "origin", branch])?;
    Ok(if out.ok() { Ok(()) } else { Err(out.text()) })
}

fn str_or_null<'de, D: Deserializer<'de>>(d: D) -> std::result::Result<String, D::Error> {
    Ok(Option::<String>::deserialize(d)?.unwrap_or_default())
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct CheckRow {
    #[serde(deserialize_with = "str_or_null")]
    pub name: String,
    #[serde(deserialize_with = "str_or_null")]
    pub state: String,
    #[serde(deserialize_with = "str_or_null")]
    pub bucket: String,
    #[serde(deserialize_with = "str_or_null")]
    pub link: String,
    #[serde(deserialize_with = "str_or_null")]
    pub workflow: String,
}

pub const CHECK_FIELDS: &str = "name,state,bucket,link,workflow";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Checks {
    Rows(Vec<CheckRow>),
    None,
    Unreadable { text: String },
}

/// The JSON is read whatever the exit code: `gh` exits 1 on a failed check, 8 on a pending one.
pub fn checks(repo: &Path, pr: &str) -> Result<Checks> {
    let out = gh(repo, &["pr", "checks", pr, "--json", CHECK_FIELDS])?;
    if let Ok(rows) = serde_json::from_str::<Vec<CheckRow>>(out.stdout.trim()) {
        return Ok(if rows.is_empty() {
            Checks::None
        } else {
            Checks::Rows(rows)
        });
    }
    let text = out.text();
    if text.to_ascii_lowercase().contains("no checks reported") {
        return Ok(Checks::None);
    }
    Ok(Checks::Unreadable { text })
}

pub fn pr_head(repo: &Path, pr: &str) -> Result<std::result::Result<String, String>> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct View {
        #[serde(default, deserialize_with = "str_or_null")]
        head_ref_oid: String,
    }
    let out = gh(repo, &["pr", "view", pr, "--json", "headRefOid"])?;
    if !out.ok() {
        return Ok(Err(out.text()));
    }
    Ok(match serde_json::from_str::<View>(out.stdout.trim()) {
        Ok(v) if !v.head_ref_oid.is_empty() => Ok(v.head_ref_oid),
        _ => Err(format!("no headRefOid in: {}", out.text())),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    Pass,
    Fail,
    Pending,
    /// The check never reached a verdict on the code.
    Infra,
}

/// `state` first: `gh` buckets `STARTUP_FAILURE` as pending, which would be waited on forever.
pub fn class(row: &CheckRow) -> Class {
    match row.state.to_ascii_uppercase().as_str() {
        "SUCCESS" | "NEUTRAL" | "SKIPPED" => Class::Pass,
        "FAILURE" | "ERROR" => Class::Fail,
        "TIMED_OUT" | "STARTUP_FAILURE" | "CANCELLED" | "ACTION_REQUIRED" | "STALE" => Class::Infra,
        "QUEUED" | "IN_PROGRESS" | "WAITING" | "PENDING" | "REQUESTED" | "EXPECTED" => {
            Class::Pending
        }
        _ => match row.bucket.as_str() {
            "pass" | "skipping" => Class::Pass,
            "fail" => Class::Fail,
            "cancel" => Class::Infra,
            _ => Class::Pending,
        },
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// The reason, for the driver's log.
    Pending(String),
    Green,
    Red(Vec<CheckRow>),
    Infra(Vec<CheckRow>),
    NoChecks,
}

/// `stale`: job links just reran, their rows still the old attempt's; `expected`: the last settled table's names.
pub fn judge(
    rows: &[CheckRow],
    stale: &HashSet<String>,
    expected: &BTreeSet<String>,
    in_grace: bool,
) -> Verdict {
    if rows.is_empty() {
        return if in_grace {
            Verdict::Pending("no checks reported yet".to_string())
        } else {
            Verdict::NoChecks
        };
    }
    let is_stale = |r: &CheckRow| !r.link.is_empty() && stale.contains(&r.link);
    let infra: Vec<CheckRow> = rows
        .iter()
        .filter(|r| !is_stale(r) && class(r) == Class::Infra)
        .cloned()
        .collect();
    if !infra.is_empty() {
        return Verdict::Infra(infra);
    }
    let rerun: Vec<&str> = rows
        .iter()
        .filter(|r| is_stale(r))
        .map(|r| r.name.as_str())
        .collect();
    if !rerun.is_empty() {
        return Verdict::Pending(format!("the rerun of {} has not registered", names(&rerun)));
    }
    let running: Vec<&str> = rows
        .iter()
        .filter(|r| class(r) == Class::Pending)
        .map(|r| r.name.as_str())
        .collect();
    if !running.is_empty() {
        return Verdict::Pending(format!("{} running", names(&running)));
    }
    if in_grace {
        let seen: BTreeSet<&str> = rows.iter().map(|r| r.name.as_str()).collect();
        let missing: Vec<&str> = expected
            .iter()
            .map(String::as_str)
            .filter(|n| !seen.contains(n))
            .collect();
        if !missing.is_empty() {
            return Verdict::Pending(format!("{} not reported yet", names(&missing)));
        }
    }
    let failed: Vec<CheckRow> = rows
        .iter()
        .filter(|r| class(r) == Class::Fail)
        .cloned()
        .collect();
    if failed.is_empty() {
        Verdict::Green
    } else {
        Verdict::Red(failed)
    }
}

fn names(list: &[&str]) -> String {
    let mut seen = HashSet::new();
    list.iter()
        .filter(|n| seen.insert(**n))
        .map(|n| format!("`{n}`"))
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn check_names(rows: &[CheckRow]) -> String {
    let mut seen = HashSet::new();
    rows.iter()
        .filter(|r| seen.insert(r.name.as_str()))
        .map(|r| r.name.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn table(rows: &[CheckRow]) -> String {
    rows.iter()
        .map(|r| format!("{}\t{}\t{}\n", r.name, r.state, r.link))
        .collect()
}

pub fn run_id(link: &str) -> Option<u64> {
    let (_, rest) = link.split_once("/actions/runs/")?;
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CiRun {
    pub database_id: u64,
    #[serde(deserialize_with = "str_or_null")]
    pub name: String,
    #[serde(deserialize_with = "str_or_null")]
    pub workflow_name: String,
    #[serde(deserialize_with = "str_or_null")]
    pub conclusion: String,
    #[serde(deserialize_with = "str_or_null")]
    pub head_sha: String,
    #[serde(deserialize_with = "str_or_null")]
    pub status: String,
    pub attempt: u64,
}

impl CiRun {
    /// A rerun in flight: the failed checks are still the previous attempt's.
    pub fn in_flight(&self) -> bool {
        !self.status.is_empty() && !self.status.eq_ignore_ascii_case("completed")
    }
}

pub const RUN_FIELDS: &str = "databaseId,name,workflowName,conclusion,headSha,status,attempt";

pub fn run_view(repo: &Path, id: u64) -> Result<std::result::Result<CiRun, String>> {
    let out = gh(
        repo,
        &["run", "view", &id.to_string(), "--json", RUN_FIELDS],
    )?;
    if !out.ok() {
        return Ok(Err(out.text()));
    }
    Ok(serde_json::from_str(out.stdout.trim())
        .map_err(|e| format!("not the expected JSON ({e}): {}", out.text())))
}

/// Newest first, whatever branch or event started them.
pub fn runs_at(
    repo: &Path,
    sha: &str,
    workflow: &str,
) -> Result<std::result::Result<Vec<CiRun>, String>> {
    let out = gh(
        repo,
        &[
            "run",
            "list",
            "--commit",
            sha,
            "--workflow",
            workflow,
            "--limit",
            "100",
            "--json",
            RUN_FIELDS,
        ],
    )?;
    if !out.ok() {
        return Ok(Err(out.text()));
    }
    Ok(serde_json::from_str(out.stdout.trim())
        .map_err(|e| format!("not the expected JSON ({e}): {}", out.text())))
}

/// The same tree went green once, so the red is the check's, not the tree's.
pub fn passed_once(runs: &[CiRun], failed: &CiRun) -> bool {
    runs.iter().any(|r| {
        r.database_id != failed.database_id
            && r.workflow_name == failed.workflow_name
            && r.head_sha == failed.head_sha
            && r.conclusion.eq_ignore_ascii_case("success")
    })
}

pub fn log_failed(repo: &Path, id: u64) -> Result<String> {
    let out = gh(repo, &["run", "view", &id.to_string(), "--log-failed"])?;
    Ok(if out.stdout.trim().is_empty() {
        out.text()
    } else {
        out.stdout
    })
}

pub fn rerun(repo: &Path, id: u64) -> Result<std::result::Result<(), String>> {
    let out = gh(repo, &["run", "rerun", &id.to_string(), "--failed"])?;
    Ok(if out.ok() { Ok(()) } else { Err(out.text()) })
}

pub fn pr_create(
    repo: &Path,
    base: &str,
    branch: &str,
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
            "--head",
            branch,
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

/// A new run on a shipped feature finds the PR the last run opened.
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

/// A resume refreshes the body instead of opening a second PR.
pub fn pr_edit(repo: &Path, pr: &str, body: &Path) -> Result<std::result::Result<(), String>> {
    let body = body.to_string_lossy().into_owned();
    let out = gh(repo, &["pr", "edit", pr, "--body-file", &body])?;
    Ok(if out.ok() { Ok(()) } else { Err(out.text()) })
}

/// A URL whose last segment is not a number comes back whole: `gh` accepts it too.
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

    fn row(name: &str, state: &str, bucket: &str, link: &str) -> CheckRow {
        CheckRow {
            name: name.into(),
            state: state.into(),
            bucket: bucket.into(),
            link: link.into(),
            workflow: "CI".into(),
        }
    }

    fn run(id: u64, workflow: &str, conclusion: &str, sha: &str) -> CiRun {
        CiRun {
            database_id: id,
            name: workflow.into(),
            workflow_name: workflow.into(),
            conclusion: conclusion.into(),
            head_sha: sha.into(),
            status: "completed".into(),
            attempt: 1,
        }
    }

    fn none() -> (HashSet<String>, BTreeSet<String>) {
        (HashSet::new(), BTreeSet::new())
    }

    #[test]
    fn the_verdict_waits_on_running_and_reports_every_failed_row() {
        let (stale, expected) = none();
        let rows = [
            row("test", "FAILURE", "fail", "https://x/actions/runs/1/job/1"),
            row(
                "lint",
                "IN_PROGRESS",
                "pending",
                "https://x/actions/runs/2/job/2",
            ),
        ];
        assert_eq!(
            judge(&rows, &stale, &expected, false),
            Verdict::Pending("`lint` running".into())
        );
        let rows = [
            row("test", "FAILURE", "fail", "https://x/actions/runs/1/job/1"),
            row("build", "SUCCESS", "pass", "https://x/actions/runs/1/job/3"),
            row("lint", "FAILURE", "fail", "https://x/actions/runs/2/job/2"),
        ];
        let Verdict::Red(failed) = judge(&rows, &stale, &expected, false) else {
            panic!("not red")
        };
        assert_eq!(check_names(&failed), "test, lint");
        let rows = [
            row("test", "SUCCESS", "pass", "l1"),
            row("docs", "SKIPPED", "skipping", "l2"),
        ];
        assert_eq!(judge(&rows, &stale, &expected, false), Verdict::Green);
    }

    #[test]
    fn a_timeout_or_a_runner_that_never_started_blocks_even_beside_a_running_check() {
        let (stale, expected) = none();
        for (state, bucket) in [
            ("TIMED_OUT", "fail"),
            ("STARTUP_FAILURE", "pending"),
            ("CANCELLED", "cancel"),
            ("ACTION_REQUIRED", "fail"),
        ] {
            let rows = [
                row("test", state, bucket, "l1"),
                row("lint", "IN_PROGRESS", "pending", "l2"),
            ];
            assert_eq!(
                judge(&rows, &stale, &expected, false),
                Verdict::Infra(vec![row("test", state, bucket, "l1")]),
                "{state}"
            );
        }
        assert_eq!(class(&row("x", "", "cancel", "")), Class::Infra);
        assert_eq!(class(&row("x", "NEW_STATE", "pending", "")), Class::Pending);
    }

    #[test]
    fn a_rerun_row_and_a_check_not_yet_registered_wait_inside_the_grace() {
        let stale: HashSet<String> = ["l1".to_string()].into();
        let expected: BTreeSet<String> = ["lint".to_string(), "test".to_string()].into();
        let red = [row("test", "FAILURE", "fail", "l1")];
        assert_eq!(
            judge(&red, &stale, &BTreeSet::new(), false),
            Verdict::Pending("the rerun of `test` has not registered".into())
        );
        let green = [row("test", "SUCCESS", "pass", "l9")];
        assert_eq!(
            judge(&green, &HashSet::new(), &expected, true),
            Verdict::Pending("`lint` not reported yet".into())
        );
        assert_eq!(
            judge(&green, &HashSet::new(), &expected, false),
            Verdict::Green
        );
        assert_eq!(
            judge(&[], &HashSet::new(), &expected, true),
            Verdict::Pending("no checks reported yet".into())
        );
        assert_eq!(
            judge(&[], &HashSet::new(), &expected, false),
            Verdict::NoChecks
        );
    }

    #[test]
    fn the_run_id_comes_from_the_job_link() {
        assert_eq!(
            run_id("https://github.com/o/r/actions/runs/123456/job/789"),
            Some(123456)
        );
        assert_eq!(
            run_id("https://example.invalid/actions/runs/1100/job/5001"),
            Some(1100)
        );
        assert_eq!(run_id("https://ci.example.com/build/7"), None);
        assert_eq!(run_id(""), None);
    }

    #[test]
    fn flaky_needs_a_green_run_of_the_same_workflow_at_the_same_head() {
        let failed = run(1000, "CI", "failure", "a");
        assert!(!passed_once(
            &[failed.clone(), run(1100, "Lint", "success", "a")],
            &failed
        ));
        assert!(!passed_once(
            &[failed.clone(), run(990, "CI", "success", "b")],
            &failed
        ));
        assert!(!passed_once(std::slice::from_ref(&failed), &failed));
        assert!(passed_once(
            &[failed.clone(), run(500, "CI", "success", "a")],
            &failed
        ));
        let mut queued = failed.clone();
        queued.status = "queued".into();
        assert!(queued.in_flight());
        assert!(!failed.in_flight());
    }

    #[test]
    fn a_run_reads_null_strings_as_empty() {
        let r: CiRun = serde_json::from_str(
            r#"{"databaseId":7,"name":"CI","workflowName":"CI","conclusion":null,"headSha":"a","status":"in_progress","attempt":2}"#,
        )
        .unwrap();
        assert_eq!(r.conclusion, "");
        assert!(r.in_flight());
        let rows: Vec<CheckRow> = serde_json::from_str(
            r#"[{"name":"test","state":"FAILURE","bucket":"fail","link":null,"workflow":"CI"}]"#,
        )
        .unwrap();
        assert_eq!(rows[0].link, "");
    }

    #[test]
    fn the_driver_pushes_only_the_feature_branch_from_the_feature_branch() {
        assert_eq!(branch_problem("feat/x", "main", Some("feat/x")), None);
        assert_eq!(
            branch_problem("main", "main", Some("main")).as_deref(),
            Some("the run's branch `main` is the default branch")
        );
        assert_eq!(
            branch_problem("feat/x", "main", Some("main")).as_deref(),
            Some("HEAD is on `main`, not the feature branch `feat/x`")
        );
        assert_eq!(
            branch_problem("feat/x", "main", None).as_deref(),
            Some("HEAD is detached, not on the feature branch `feat/x`")
        );
        assert!(branch_problem("HEAD", "main", None).is_some());
        assert!(branch_problem("", "main", Some("feat/x")).is_some());
    }

    #[test]
    fn the_pr_number_is_the_last_numeric_segment() {
        assert_eq!(pr_number("https://github.com/o/r/pull/12"), "12");
        assert_eq!(pr_number("https://example.invalid/pr/1/"), "1");
        assert_eq!(pr_number("https://x/pr/abc"), "https://x/pr/abc");
    }

    #[test]
    fn the_interval_defaults_and_reads_the_env() {
        assert_eq!(DEFAULT_INTERVAL, 60);
        assert_eq!(DEFAULT_TIMEOUT, 21_600);
        assert_eq!(GRACE_POLLS, 5);
        assert_eq!(env_secs("FACTORY_CI_NO_SUCH_VAR", 7), 7);
        assert_eq!(CI_FIX_ROUNDS, 3);
        assert_eq!(CI_FLAKY_RERUNS, 1);
    }
}
