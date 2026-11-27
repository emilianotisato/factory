//! What a draft written in a client repo would leak into the public factory repo, and the open issues it may repeat.
use std::path::Path;
use std::sync::LazyLock;

use serde::{Deserialize, Serialize};

use crate::docs::preflight;
use crate::{Error, Result, ci, metrics, paths};

pub const REPO: &str = "github.com/emilianotisato/factory";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Draft {
    pub title: String,
    pub body: String,
}

/// The first `# ` line is the title; every other line is the body.
pub fn parse(text: &str) -> Result<Draft> {
    let lines: Vec<&str> = text.lines().collect();
    let Some(at) = lines.iter().position(|l| l.starts_with("# ")) else {
        return Err(Error::Msg("the draft has no `# ` title line".into()));
    };
    let title = lines[at][2..].trim().to_string();
    if title.is_empty() {
        return Err(Error::Msg("the draft's `# ` title line is empty".into()));
    }
    let body: Vec<&str> = lines
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != at)
        .map(|(_, l)| *l)
        .collect();
    Ok(Draft {
        title,
        body: body.join("\n").trim().to_string(),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    /// 1-based.
    pub line: usize,
    pub what: String,
}

/// The words that name the repo the draft comes from: its directory, and its origin's owner and name.
pub fn names(repo: &Path) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(dir) = repo.file_name().and_then(|n| n.to_str()) {
        out.push(dir.to_string());
    }
    if let Some(list) = preflight::git(repo, &["remote"]) {
        for remote in list.lines().map(str::trim).filter(|r| !r.is_empty()) {
            if let Some(url) = preflight::git(repo, &["remote", "get-url", remote])
                && let Some((owner, name)) = owner_and_name(url.trim())
            {
                out.push(owner);
                out.push(name);
            }
        }
    }
    out.retain(|n| !n.is_empty());
    out.sort();
    out.dedup();
    out
}

/// Only a host form names an owner: a local path's folders are the machine's, not the project's.
pub fn owner_and_name(url: &str) -> Option<(String, String)> {
    let path = if let Some((scheme, rest)) = url.split_once("://") {
        if !matches!(scheme, "https" | "http" | "ssh" | "git") {
            return None;
        }
        let (host, path) = rest.split_once('/')?;
        if host.is_empty() {
            return None;
        }
        path
    } else {
        let (host, path) = url.split_once(':')?;
        if host.is_empty() || host.contains('/') {
            return None;
        }
        path
    };
    let segments: Vec<&str> = path
        .trim_matches('/')
        .split('/')
        .filter(|s| !s.is_empty())
        .collect();
    let (owner, name) = (segments.first()?, segments.last()?);
    let name = name.strip_suffix(".git").unwrap_or(name);
    (segments.len() >= 2 && !name.is_empty()).then(|| (owner.to_string(), name.to_string()))
}

static EMAIL: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9-]+(?:\.[A-Za-z0-9-]+)*\.[A-Za-z]{2,}")
        .expect("email pattern compiles")
});

/// The person's own home, as `$HOME` names it and as it resolves: one outside `/home` or `/Users`, such as `/var/home/x`, names them too.
pub fn homes() -> Vec<String> {
    let home = paths::home();
    let mut out: Vec<String> = [Some(home.clone()), home.canonicalize().ok()]
        .into_iter()
        .flatten()
        .filter_map(|p| p.to_str().map(|s| s.trim_end_matches('/').to_string()))
        .filter(|s| !s.is_empty())
        .collect();
    out.sort();
    out.dedup();
    out
}

fn word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// No `.`: a home that ends a sentence still hits, and `/home/x.bak` hitting costs a rewrite, not a leak.
fn path_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'~')
}

/// `needle` in `hay` where no `inner` byte touches either end.
fn bounded(hay: &str, needle: &str, inner: fn(u8) -> bool) -> bool {
    let bytes = hay.as_bytes();
    hay.match_indices(needle).any(|(at, m)| {
        let before = at.checked_sub(1).map(|i| bytes[i]);
        let after = bytes.get(at + m.len()).copied();
        !before.is_some_and(inner) && !after.is_some_and(inner)
    })
}

/// Case-insensitive, and only where no letter, digit or `_` touches either end.
fn holds_word(line: &str, word: &str) -> bool {
    bounded(&line.to_lowercase(), &word.to_lowercase(), word_byte)
}

/// Per line: the first home path, else the person's own home wherever it lives; then every repo name and email.
pub fn hits(text: &str, names: &[String], homes: &[String]) -> Vec<Hit> {
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let mut add = |what: String| out.push(Hit { line: i + 1, what });
        if let Some(at) = paths::find_home_path(line.as_bytes()) {
            let path: String = line[at..]
                .chars()
                .take_while(|c| !c.is_whitespace() && !"`'\"()<>[]".contains(*c))
                .collect();
            add(format!("a home path: {path}"));
        } else if let Some(home) = homes.iter().find(|h| bounded(line, h, path_byte)) {
            add(format!("a home path: {home}"));
        }
        for name in names {
            if name_variants(name).iter().any(|v| holds_word(line, v)) {
                add(format!("this repo's name: {name}"));
            }
        }
        for m in EMAIL.find_iter(line) {
            add(format!("an email address: {}", m.as_str()));
        }
    }
    out
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Issue {
    pub number: u64,
    pub title: String,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PulledAuthor {
    pub login: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PulledLabel {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PulledIssue {
    pub number: u64,
    pub title: String,
    #[serde(default)]
    pub body: String,
    pub url: String,
    #[serde(default)]
    pub labels: Vec<PulledLabel>,
    #[serde(default, rename = "createdAt")]
    pub created_at: String,
    #[serde(default)]
    pub author: Option<PulledAuthor>,
}

fn gh_fail(cmd: &str, out: &ci::Output) -> Error {
    Error::Tool {
        cmd: cmd.into(),
        detail: out.text(),
    }
}

pub fn list_open(repo: &Path) -> Result<Vec<PulledIssue>> {
    let cmd = format!("gh issue list --repo {REPO}");
    let out = ci::gh(
        repo,
        &[
            "issue",
            "list",
            "--repo",
            REPO,
            "--state",
            "open",
            "--limit",
            "50",
            "--json",
            "number,title,body,url,labels,createdAt,author",
        ],
    )?;
    if !out.ok() {
        return Err(gh_fail(&cmd, &out));
    }
    if out.stdout.trim().is_empty() {
        return Ok(Vec::new());
    }
    serde_json::from_str(&out.stdout)
        .map_err(|e| Error::Msg(format!("gh printed no issue list: {e}")))
}

pub fn view_issue(repo: &Path, number: u64) -> Result<PulledIssue> {
    let cmd = format!("gh issue view {number} --repo {REPO}");
    let num = number.to_string();
    let out = ci::gh(
        repo,
        &[
            "issue",
            "view",
            &num,
            "--repo",
            REPO,
            "--json",
            "number,title,body,url",
        ],
    )?;
    if !out.ok() {
        return Err(gh_fail(&cmd, &out));
    }
    serde_json::from_str(&out.stdout)
        .map_err(|e| Error::Msg(format!("gh printed no issue view: {e}")))
}

pub fn comment_issue(repo: &Path, number: u64, body: &str) -> Result<()> {
    let cmd = format!("gh issue comment {number} --repo {REPO}");
    let num = number.to_string();
    let out = ci::gh_stdin(
        repo,
        &["issue", "comment", &num, "--repo", REPO, "--body-file", "-"],
        body.as_bytes(),
    )?;
    if !out.ok() {
        return Err(gh_fail(&cmd, &out));
    }
    Ok(())
}

pub fn close_issue(repo: &Path, number: u64, reason: &str) -> Result<()> {
    let cmd = format!("gh issue close {number} --repo {REPO}");
    let num = number.to_string();
    let out = ci::gh(
        repo,
        &["issue", "close", &num, "--repo", REPO, "--reason", reason],
    )?;
    if !out.ok() {
        return Err(gh_fail(&cmd, &out));
    }
    Ok(())
}

/// Up to five open issues on the factory repo matching `title`; `Err` holds the one line saying why the search failed.
pub fn search(repo: &Path, title: &str) -> std::result::Result<Vec<Issue>, String> {
    let out = ci::gh(
        repo,
        &[
            "issue",
            "list",
            "--repo",
            REPO,
            "--state",
            "open",
            "--search",
            title,
            "--limit",
            "5",
            "--json",
            "number,title,url",
        ],
    )
    .map_err(|e| e.to_string())?;
    if !out.ok() {
        let why = out.text();
        return Err(why
            .lines()
            .next()
            .unwrap_or("gh exited non-zero")
            .to_string());
    }
    if out.stdout.trim().is_empty() {
        return Ok(Vec::new());
    }
    serde_json::from_str(&out.stdout).map_err(|e| format!("gh printed no issue list: {e}"))
}

pub fn draft_hash(text: &str) -> String {
    metrics::sha256_hex(text.as_bytes())[..12].to_string()
}

fn name_variants(name: &str) -> Vec<String> {
    let mut out = vec![name.to_string()];
    if name.contains(['-', '_', ' ']) {
        let parts: Vec<&str> = name
            .split(['-', '_', ' '])
            .filter(|p| !p.is_empty())
            .collect();
        if parts.len() >= 2 {
            out.push(parts.join("-"));
            out.push(parts.join("_"));
            out.push(parts.join(" "));
            out.push(parts.concat());
            for part in parts {
                if part.chars().count() >= 4 {
                    out.push(part.to_string());
                }
            }
        }
    }
    let mut seen = Vec::new();
    let mut deduped = Vec::new();
    for v in out {
        let key = v.to_lowercase();
        if seen.contains(&key) {
            continue;
        }
        seen.push(key);
        deduped.push(v);
    }
    deduped
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(words: &[&str]) -> Vec<String> {
        words.iter().map(|w| w.to_string()).collect()
    }

    fn whats(text: &str, words: &[&str]) -> Vec<(usize, String)> {
        hits(text, &names(words), &[])
            .into_iter()
            .map(|h| (h.line, h.what))
            .collect()
    }

    #[test]
    fn a_home_path_is_a_hit_with_its_line() {
        let got = whats("# t\nsee /home/x/notes.md here", &[]);
        assert_eq!(got, [(2, "a home path: /home/x/notes.md".to_string())]);
        assert!(whats("see /tmp/home/x/a", &[]).is_empty());
    }

    #[test]
    fn the_persons_own_home_hits_wherever_it_lives() {
        let homes = ["/var/home/jdoe".to_string()];
        let got: Vec<String> = hits(
            "ran in /var/home/jdoe/acme\ncwd was /var/home/jdoe.\n/var/home/jdoex/a",
            &[],
            &homes,
        )
        .into_iter()
        .map(|h| format!("{}: {}", h.line, h.what))
        .collect();
        assert_eq!(
            got,
            [
                "1: a home path: /var/home/jdoe",
                "2: a home path: /var/home/jdoe"
            ]
        );
    }

    #[test]
    fn the_repo_name_hits_as_a_whole_word_in_any_case() {
        let got = whats("one\nthe Acme-Api broke", &["acme-api"]);
        assert_eq!(got, [(2, "this repo's name: acme-api".to_string())]);
        assert_eq!(whats("in arca, again", &["arca"]).len(), 1);
        assert_eq!(whats("el ÑANDÚ corre", &["ñandú"]).len(), 1);
    }

    #[test]
    fn a_word_that_only_contains_the_name_does_not_hit() {
        assert!(whats("the repository and repos and prepo", &["repo"]).is_empty());
        assert!(whats("acme_api", &["acme"]).is_empty());
    }

    #[test]
    fn an_email_is_a_hit() {
        let got = whats("mail jane.doe+x@example.co.uk now", &[]);
        assert_eq!(
            got,
            [(1, "an email address: jane.doe+x@example.co.uk".to_string())]
        );
        assert!(whats("a @mention and x@y", &[]).is_empty());
    }

    #[test]
    fn owner_and_name_come_from_both_host_forms() {
        let pair = |o: &str, n: &str| Some((o.to_string(), n.to_string()));
        assert_eq!(
            owner_and_name("git@github.com:acme/widgets.git"),
            pair("acme", "widgets")
        );
        assert_eq!(
            owner_and_name("git@github.com:acme/widgets"),
            pair("acme", "widgets")
        );
        assert_eq!(
            owner_and_name("https://github.com/acme/widgets.git"),
            pair("acme", "widgets")
        );
        assert_eq!(
            owner_and_name("https://github.com/acme/widgets"),
            pair("acme", "widgets")
        );
    }

    #[test]
    fn a_local_origin_names_nothing() {
        for url in [
            "/tmp/abc/origin.git",
            "../origin",
            "file:///tmp/abc/origin.git",
            "origin.git",
        ] {
            assert_eq!(owner_and_name(url), None, "{url}");
        }
    }

    #[test]
    fn an_origin_owner_and_name_from_a_repo_are_hits() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("client");
        std::fs::create_dir(&repo).unwrap();
        let git = |args: &[&str]| {
            let ok = std::process::Command::new("git")
                .args(args)
                .current_dir(&repo)
                .env("GIT_CONFIG_GLOBAL", "/dev/null")
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .output()
                .unwrap()
                .status
                .success();
            assert!(ok, "git {args:?}");
        };
        git(&["init", "-q"]);
        assert_eq!(super::names(&repo), ["client"]);
        git(&["remote", "add", "origin", "git@github.com:acme/widgets.git"]);
        let found = super::names(&repo);
        assert_eq!(found, ["acme", "client", "widgets"]);
        assert_eq!(hits("Acme ships widgets", &found, &[]).len(), 2);
    }

    #[test]
    fn every_configured_remote_names_the_repo() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("widgets");
        std::fs::create_dir(&repo).unwrap();
        let git = |args: &[&str]| {
            let ok = std::process::Command::new("git")
                .args(args)
                .current_dir(&repo)
                .env("GIT_CONFIG_GLOBAL", "/dev/null")
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .output()
                .unwrap()
                .status
                .success();
            assert!(ok, "git {args:?}");
        };
        git(&["init", "-q"]);
        git(&[
            "remote",
            "add",
            "upstream",
            "git@github.com:acme/widgets.git",
        ]);
        let found = super::names(&repo);
        assert!(found.contains(&"acme".to_string()));
        assert!(found.contains(&"widgets".to_string()));
        git(&[
            "remote",
            "add",
            "github",
            "https://github.com/other/spare.git",
        ]);
        let found = super::names(&repo);
        assert!(found.contains(&"other".to_string()));
        assert!(found.contains(&"spare".to_string()));
    }

    #[test]
    fn spelling_variants_hit_once_per_line() {
        let lines = ["AcmeApi ships", "saw acme_api", "in acme api", "only acme"];
        for (i, line) in lines.iter().enumerate() {
            let got = whats(line, &["acme-api"]);
            assert_eq!(got.len(), 1, "line {i}: {got:?}");
            assert_eq!(got[0].1, "this repo's name: acme-api");
        }
        assert!(whats("tiny api part", &["acme-api"]).is_empty());
    }

    #[test]
    fn draft_hash_is_twelve_lowercase_hex() {
        let h = super::draft_hash("hello");
        assert_eq!(h.len(), 12);
        assert!(
            h.chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_uppercase())
        );
        assert_eq!(h, super::draft_hash("hello"));
    }

    #[test]
    fn the_title_is_the_first_heading_line_and_the_rest_the_body() {
        let d = parse("intro\n# The title\n\nthe body\n").unwrap();
        assert_eq!(d.title, "The title");
        assert_eq!(d.body, "intro\n\nthe body");
        assert!(parse("no heading\n## nor this").is_err());
    }

    #[test]
    fn pulled_issue_round_trips_gh_json_field_names() {
        let minimal: PulledIssue =
            serde_json::from_str(r#"{"number":1,"title":"t","url":"https://example.com/1"}"#)
                .unwrap();
        assert_eq!(minimal.body, "");
        assert!(minimal.labels.is_empty());
        assert!(minimal.author.is_none());
        assert!(minimal.created_at.is_empty());
        let raw = r#"{"number":2,"title":"T","body":"B","url":"https://example.com/2","labels":[{"name":"bug"}],"createdAt":"2026-11-27T00:00:00Z","author":{"login":"alice"}}"#;
        let full: PulledIssue = serde_json::from_str(raw).unwrap();
        assert_eq!(full.created_at, "2026-11-27T00:00:00Z");
        assert_eq!(
            full.author.as_ref().map(|a| a.login.as_str()),
            Some("alice")
        );
        let again = serde_json::to_string(&full).unwrap();
        assert!(again.contains("\"createdAt\""));
        let round: PulledIssue = serde_json::from_str(&again).unwrap();
        assert_eq!(round, full);
    }
}
