//! The driver's commits in the main tree, by pathspec: never `git add -A` there.
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::Mutex;

use crate::docs::Status;
use crate::{Error, Result};

/// The files no worker may write: they are the orchestrator's.
pub fn protected_files(board: &Status) -> [String; 2] {
    [format!("{}/prd.md", board.dir), "docs/index.md".to_string()]
}

pub fn tool_files(board: &Status) -> [String; 2] {
    [
        format!("{}/state.toml", board.dir),
        format!("{}/log.jsonl", board.dir),
    ]
}

fn push_once(v: &mut Vec<String>, s: String) {
    if !v.contains(&s) {
        v.push(s);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Committed {
    pub sha: String,
    /// The paths staged, in pathspec order.
    pub files: Vec<String>,
}

/// Never-written and ignored paths are dropped first: `git add` refuses either.
pub fn commit(repo: &Path, paths: &[String], message: &str) -> Result<Option<Committed>> {
    if paths.is_empty() {
        return Ok(None);
    }
    if let Some((branch, default)) = held(repo)
        && let Some(why) = branch_refusal(repo, &branch, default.as_deref())?
    {
        return Err(Error::Msg(format!("commit refused: {why}")));
    }
    let mut ls = vec![
        "ls-files",
        "-z",
        "--cached",
        "--others",
        "--exclude-standard",
        "--",
    ];
    ls.extend(paths.iter().map(String::as_str));
    let listed = git(repo, &ls)?;
    let known: HashSet<&str> = listed.split('\0').filter(|s| !s.is_empty()).collect();
    let files: Vec<String> = paths
        .iter()
        .filter(|p| known.contains(p.as_str()))
        .cloned()
        .collect();
    if files.is_empty() {
        return Ok(None);
    }
    let mut add = vec!["add", "--"];
    add.extend(files.iter().map(String::as_str));
    git(repo, &add)?;
    // Same pathspec for the check and the commit: whatever else is staged stays out, and staged.
    let mut staged = vec!["diff", "--cached", "--quiet", "--"];
    staged.extend(files.iter().map(String::as_str));
    if git_code(repo, &staged)? == 0 {
        return Ok(None);
    }
    let mut commit = vec!["commit", "-q", "-m", message, "--"];
    commit.extend(files.iter().map(String::as_str));
    git(repo, &commit)?;
    let sha = git(repo, &["rev-parse", "HEAD"])?.trim().to_string();
    Ok(Some(Committed { sha, files }))
}

pub fn head_branch(repo: &Path) -> Result<Option<String>> {
    let out = run_git(repo, &["symbolic-ref", "-q", "--short", "HEAD"])?;
    let name = String::from_utf8_lossy(&out.stdout).trim().to_string();
    Ok((out.status.success() && !name.is_empty()).then_some(name))
}

pub fn branch_refusal(repo: &Path, branch: &str, default: Option<&str>) -> Result<Option<String>> {
    if default == Some(branch) {
        return Ok(Some(format!(
            "the run's branch {branch} is the default branch — the driver commits only on a \
             feature branch, and the owner decides which one"
        )));
    }
    Ok(match head_branch(repo)? {
        Some(head) if head == branch => None,
        Some(head) => Some(format!(
            "HEAD is on {head}, not the run's branch {branch} — `git switch {branch}`"
        )),
        None => Some(format!(
            "HEAD is detached, not on the run's branch {branch} — `git switch {branch}`"
        )),
    })
}

static HELD: Mutex<Vec<(PathBuf, String, Option<String>)>> = Mutex::new(Vec::new());

/// Catches the owner switching branches between the driver's check and its commit.
pub fn hold_branch(repo: &Path, branch: &str, default: Option<&str>) {
    let key = canonical(repo);
    let mut held = HELD.lock().unwrap_or_else(|p| p.into_inner());
    held.retain(|(r, _, _)| *r != key);
    held.push((key, branch.to_string(), default.map(str::to_string)));
}

pub(crate) fn held(repo: &Path) -> Option<(String, Option<String>)> {
    let key = canonical(repo);
    let held = HELD.lock().unwrap_or_else(|p| p.into_inner());
    held.iter()
        .find(|(r, _, _)| *r == key)
        .map(|(_, b, d)| (b.clone(), d.clone()))
}

fn canonical(repo: &Path) -> PathBuf {
    repo.canonicalize().unwrap_or_else(|_| repo.to_path_buf())
}

/// Directories become their files: [`commit`] stages exact names only.
pub fn expand(repo: &Path, paths: &[String]) -> Result<Vec<String>> {
    if paths.is_empty() {
        return Ok(Vec::new());
    }
    let mut ls = vec![
        "ls-files",
        "-z",
        "--cached",
        "--others",
        "--exclude-standard",
        "--",
    ];
    ls.extend(paths.iter().map(String::as_str));
    let listed = git(repo, &ls)?;
    let mut out = Vec::new();
    for p in listed.split('\0').filter(|s| !s.is_empty()) {
        push_once(&mut out, p.to_string());
    }
    Ok(out)
}

/// Scoped like the builders' own subjects (`feat(018): slice-02 Harness`), so a log search by feature finds it.
pub fn subject(board: &Status, kind: &str, text: &str) -> String {
    format!("{kind}({:03}): {text}", board.number)
}

fn git(repo: &Path, args: &[&str]) -> Result<String> {
    let out = run_git(repo, args)?;
    if !out.status.success() {
        return Err(Error::Tool {
            cmd: format!("git {}", args.join(" ")),
            detail: format!(
                "exit {}: {}",
                out.status.code().unwrap_or(-1),
                String::from_utf8_lossy(&out.stderr).trim()
            ),
        });
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn git_code(repo: &Path, args: &[&str]) -> Result<i32> {
    Ok(run_git(repo, args)?.status.code().unwrap_or(-1))
}

fn run_git(repo: &Path, args: &[&str]) -> Result<Output> {
    Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .map_err(|e| Error::Tool {
            cmd: format!("git {}", args.join(" ")),
            detail: format!("could not start git: {e}"),
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let r = dir.path();
        for args in [
            vec!["init", "-q", "-b", "main"],
            vec!["config", "user.name", "T"],
            vec!["config", "user.email", "t@example.invalid"],
            vec!["config", "core.hooksPath", "/nonexistent-hooks"],
        ] {
            git(r, &args).unwrap();
        }
        fs::write(r.join(".gitignore"), "/ignored/\n").unwrap();
        fs::write(r.join("kept.txt"), "k\n").unwrap();
        fs::write(r.join("gone.txt"), "g\n").unwrap();
        git(r, &["add", "-A"]).unwrap();
        git(r, &["commit", "-q", "-m", "init"]).unwrap();
        dir
    }

    #[test]
    fn commit_stages_only_the_named_paths_and_drops_what_git_would_refuse() {
        let dir = repo();
        let r = dir.path();
        fs::write(r.join("kept.txt"), "k2\n").unwrap();
        fs::remove_file(r.join("gone.txt")).unwrap();
        fs::write(r.join("new.txt"), "n\n").unwrap();
        fs::write(r.join("stray.txt"), "s\n").unwrap();
        fs::create_dir_all(r.join("ignored")).unwrap();
        fs::write(r.join("ignored/x.txt"), "x\n").unwrap();
        assert_eq!(
            git(r, &["ls-files", "--others", "--exclude-standard"]).unwrap(),
            "new.txt\nstray.txt\n"
        );

        let paths: Vec<String> = [
            "kept.txt",
            "gone.txt",
            "new.txt",
            "never-written.txt",
            "ignored/x.txt",
        ]
        .map(str::to_string)
        .to_vec();
        let c = commit(r, &paths, "feat: t").unwrap().unwrap();
        assert_eq!(c.files, ["kept.txt", "gone.txt", "new.txt"]);
        assert_eq!(git(r, &["rev-parse", "HEAD"]).unwrap().trim(), c.sha);
        assert_eq!(
            git(r, &["log", "-1", "--format=%s%n%b"]).unwrap(),
            "feat: t\n\n"
        );
        let mut shown: Vec<String> = git(r, &["show", "--format=", "--name-status", "HEAD"])
            .unwrap()
            .lines()
            .map(str::to_string)
            .collect();
        shown.sort();
        assert_eq!(shown, ["A\tnew.txt", "D\tgone.txt", "M\tkept.txt"]);
        assert_eq!(
            git(r, &["status", "--porcelain"]).unwrap(),
            "?? stray.txt\n"
        );
        assert_eq!(commit(r, &paths, "feat: again").unwrap(), None);
        assert_eq!(commit(r, &[], "feat: none").unwrap(), None);
    }

    #[test]
    fn a_repo_held_to_a_branch_refuses_commits_anywhere_else() {
        let dir = repo();
        let r = dir.path();
        git(r, &["switch", "-q", "-c", "feat/x"]).unwrap();
        hold_branch(r, "feat/x", Some("main"));
        let mine = vec!["kept.txt".to_string()];
        fs::write(r.join("kept.txt"), "k2\n").unwrap();

        git(r, &["switch", "-q", "main"]).unwrap();
        let err = commit(r, &mine, "feat: wrong").unwrap_err().to_string();
        assert!(
            err.contains("HEAD is on main, not the run's branch feat/x"),
            "{err}"
        );
        let head = git(r, &["rev-parse", "HEAD"]).unwrap();
        git(r, &["switch", "-q", "--detach", head.trim()]).unwrap();
        let err = commit(r, &mine, "feat: detached").unwrap_err().to_string();
        assert!(err.contains("HEAD is detached"), "{err}");
        assert_eq!(
            git(r, &["rev-list", "--count", "HEAD"]).unwrap().trim(),
            "1"
        );

        git(r, &["switch", "-q", "feat/x"]).unwrap();
        assert!(commit(r, &mine, "feat: right").unwrap().is_some());

        hold_branch(r, "feat/x", Some("feat/x"));
        fs::write(r.join("kept.txt"), "k3\n").unwrap();
        let err = commit(r, &mine, "feat: default").unwrap_err().to_string();
        assert!(err.contains("is the default branch"), "{err}");
    }

    #[test]
    fn commit_leaves_what_else_is_staged_out_and_staged() {
        let dir = repo();
        let r = dir.path();
        fs::write(r.join("owner.txt"), "o\n").unwrap();
        git(r, &["add", "--", "owner.txt"]).unwrap();
        let mine = vec!["kept.txt".to_string()];
        assert_eq!(commit(r, &mine, "feat: nothing").unwrap(), None);
        assert_eq!(
            git(r, &["rev-list", "--count", "HEAD"]).unwrap().trim(),
            "1"
        );

        fs::write(r.join("kept.txt"), "k2\n").unwrap();
        let c = commit(r, &mine, "feat: mine").unwrap().unwrap();
        assert_eq!(
            git(r, &["show", "--format=", "--name-only", &c.sha]).unwrap(),
            "kept.txt\n"
        );
        assert_eq!(
            git(r, &["status", "--porcelain"]).unwrap(),
            "A  owner.txt\n"
        );
    }
}
