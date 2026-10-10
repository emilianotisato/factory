//! One commit per slice, by pathspec (PRD §7 FR-5). The paths are the
//! `file_path` of every `Edit`, `Write` and `NotebookEdit` call in the
//! worker's stream (§4 FR-2: the stream is the record) plus the slice file
//! and its log fragment; the message is `feat: <slice title>`, no body, no
//! trailers, the repo's own `user.*`. Never `git add -A`: another lane may be
//! mid-write in the same tree, and only named paths move. What the worker
//! wrote without naming it (a `Bash` heredoc, a generator) is listed in the
//! fragment under `## Untracked leftovers` and stays in the tree for the
//! owner. When the lanes drain, [`merge_logs`] folds the fragments into
//! `agent-log.md` with `docs log merge` and commits the result the same way.
use std::collections::HashSet;
use std::fs;
use std::path::{Component, Path};
use std::process::{Command, Output};

use crate::docs::{Docs, Status};
use crate::error::io;
use crate::stream::Summary;
use crate::{Error, Result};

/// The tools whose `input.file_path` names a file the worker wrote.
pub const WRITING_TOOLS: &[&str] = &["Edit", "Write", "NotebookEdit"];

/// The merge commit's subject.
pub const MERGE_SUBJECT: &str = "docs: merge slice logs";

/// The three files no worker may write (`deliver/SKILL.md`): they are the
/// orchestrator's, and a worker's edit of `agent-log.md` races the merge.
pub fn protected_files(board: &Status) -> [String; 3] {
    [
        format!("{}/prd.md", board.dir),
        format!("{}/agent-log.md", board.dir),
        "docs/index.md".to_string(),
    ]
}

/// `docs/features/<dir>/log-slice-NN.md`, repo-relative.
pub fn fragment_path(board: &Status, slice: u32) -> String {
    format!("{}/log-slice-{slice:02}.md", board.dir)
}

/// `docs/features/<file>`, repo-relative; `None` off the board.
pub fn slice_path(board: &Status, slice: u32) -> Option<String> {
    board
        .slices
        .iter()
        .find(|s| s.number == slice)
        .map(|s| format!("docs/features/{}", s.file))
}

/// What a slice's commit holds, and what fell out of the stream's paths.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Pathspec {
    /// Repo-relative, deduplicated, in stream order; the slice file and the
    /// fragment last.
    pub paths: Vec<String>,
    /// Named in the stream but one of [`protected_files`]: left out.
    pub protected: Vec<String>,
    /// Named in the stream but outside the repo root: left out.
    pub outside: Vec<String>,
}

impl Pathspec {
    /// The slice's pathspec from its worker's stream and the board.
    pub fn from(summary: &Summary, board: &Status, slice: u32, repo: &Path) -> Pathspec {
        let canon = repo.canonicalize().unwrap_or_else(|_| repo.to_path_buf());
        let protected = protected_files(board);
        let mut spec = Pathspec::default();
        for raw in written_paths(summary) {
            let Some(rel) = relativize(repo, &canon, raw) else {
                push_once(&mut spec.outside, raw.to_string());
                continue;
            };
            if protected.contains(&rel) {
                push_once(&mut spec.protected, rel);
            } else {
                push_once(&mut spec.paths, rel);
            }
        }
        if let Some(p) = slice_path(board, slice) {
            push_once(&mut spec.paths, p);
        }
        push_once(&mut spec.paths, fragment_path(board, slice));
        spec
    }
}

/// The `file_path` of every writing tool call in the stream, as given.
pub fn written_paths(summary: &Summary) -> impl Iterator<Item = &str> {
    summary
        .tool_uses
        .iter()
        .filter(|t| WRITING_TOOLS.contains(&t.name.as_str()))
        .filter_map(|t| t.input.get("file_path").and_then(|v| v.as_str()))
}

fn push_once(v: &mut Vec<String>, s: String) {
    if !v.contains(&s) {
        v.push(s);
    }
}

/// A stream path made repo-relative and lexically normalized; `None` when it
/// points outside the repo. Nothing is touched on disk: a file the worker
/// deleted must still reach the pathspec so the deletion is staged.
pub fn relativize(repo: &Path, canon: &Path, raw: &str) -> Option<String> {
    let p = Path::new(raw);
    let rel = if p.is_absolute() {
        p.strip_prefix(repo)
            .or_else(|_| p.strip_prefix(canon))
            .ok()?
    } else {
        p
    };
    let mut parts: Vec<&str> = Vec::new();
    for c in rel.components() {
        match c {
            Component::Normal(s) => parts.push(s.to_str()?),
            Component::CurDir => {}
            Component::ParentDir => {
                parts.pop()?;
            }
            Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("/"))
    }
}

/// Untracked, non-ignored files in the tree, repo-relative and sorted: the
/// `??` entries of `git status --porcelain -z --untracked-files=all`.
pub fn untracked(repo: &Path) -> Result<Vec<String>> {
    let out = git(
        repo,
        &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
    )?;
    let mut files = Vec::new();
    let mut entries = out.split('\0');
    while let Some(entry) = entries.next() {
        if entry.len() < 4 {
            continue;
        }
        let (xy, path) = (&entry[..2], &entry[3..]);
        // A rename or copy carries the original path in the next field.
        if xy.starts_with('R') || xy.starts_with('C') {
            entries.next();
        }
        if xy == "??" {
            files.push(path.to_string());
        }
    }
    files.sort();
    Ok(files)
}

/// What a commit landed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Committed {
    pub sha: String,
    /// The paths staged, in pathspec order.
    pub files: Vec<String>,
}

/// `git add -- <paths>` and `git commit -m <message>` in the repo root: no
/// author set, no body, hooks on. Paths that are neither in the index nor in
/// the tree (never written, or written and removed again) and untracked
/// paths the repo ignores are dropped first — `git add` refuses either. A
/// tracked path the worker deleted stays and stages the deletion. `None`
/// when nothing was staged.
pub fn commit(repo: &Path, paths: &[String], message: &str) -> Result<Option<Committed>> {
    if paths.is_empty() {
        return Ok(None);
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
    if git_code(repo, &["diff", "--cached", "--quiet"])? == 0 {
        return Ok(None);
    }
    git(repo, &["commit", "-q", "-m", message])?;
    let sha = git(repo, &["rev-parse", "HEAD"])?.trim().to_string();
    Ok(Some(Committed { sha, files }))
}

/// How the drained-lanes merge went.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Merge {
    /// Fragments folded, `render --write` run, the result committed.
    Merged { sha: String, fragments: Vec<String> },
    /// No fragment to fold, or nothing left to commit.
    Nothing,
    /// `docs log merge` refused — an AD clash; `detail` is what it said.
    Refused { detail: String },
}

/// `docs log merge <feature>`, `docs render --write`, then one commit of
/// `agent-log.md`, `prd.md`, `docs/index.md` and the fragments the merge
/// removed: [`MERGE_SUBJECT`]. A missing `agent-log.md` is created with its
/// header first — the tool merges into an existing file only.
pub fn merge_logs(docs: &Docs, repo: &Path, board: &Status, feature: &str) -> Result<Merge> {
    let dir = repo.join(&board.dir);
    let names = fragments_in(&dir)?;
    if names.is_empty() {
        return Ok(Merge::Nothing);
    }
    let log = dir.join("agent-log.md");
    if !log.exists() {
        fs::write(&log, format!("# Agent log — PRD-{feature}\n"))
            .map_err(io(format!("write {}", log.display())))?;
    }
    let out = docs.log_merge(feature)?;
    if out.code != 0 {
        let detail = if out.stderr.trim().is_empty() {
            out.stdout
        } else {
            out.stderr
        };
        return Ok(Merge::Refused {
            detail: detail.trim().to_string(),
        });
    }
    docs.render_write()?;
    let fragments: Vec<String> = names.iter().map(|n| format!("{}/{n}", board.dir)).collect();
    let mut paths = vec![
        format!("{}/agent-log.md", board.dir),
        format!("{}/prd.md", board.dir),
        "docs/index.md".to_string(),
    ];
    paths.extend(fragments.iter().cloned());
    Ok(match commit(repo, &paths, MERGE_SUBJECT)? {
        Some(c) => Merge::Merged {
            sha: c.sha,
            fragments,
        },
        None => Merge::Nothing,
    })
}

/// `log-slice-NN.md` and `log-fix-N.md` in the feature folder, by name.
fn fragments_in(dir: &Path) -> Result<Vec<String>> {
    let rd = match fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(io(format!("read {}", dir.display()))(e)),
    };
    let mut out: Vec<String> = rd
        .filter_map(|e| e.ok())
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| is_fragment(n))
        .collect();
    out.sort();
    Ok(out)
}

fn is_fragment(name: &str) -> bool {
    let Some(rest) = name.strip_prefix("log-") else {
        return false;
    };
    let Some(rest) = rest
        .strip_prefix("slice-")
        .or_else(|| rest.strip_prefix("fix-"))
    else {
        return false;
    };
    let Some(digits) = rest.strip_suffix(".md") else {
        return false;
    };
    !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit())
}

/// `git <args>` in the repo root; a non-zero exit is an error quoting stderr.
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

/// `git <args>`, the exit code alone (`diff --quiet` answers with it).
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
    use crate::docs::Slice;
    use crate::stream::ToolUse;
    use serde_json::json;

    fn board() -> Status {
        Status {
            dir: "docs/features/001-fixture".to_string(),
            slices: vec![Slice {
                number: 1,
                file: "001-fixture/slice-01-a.md".to_string(),
                title: Some("a".to_string()),
                status: Some("done".to_string()),
                lane: None,
                depends_on: Vec::new(),
            }],
            ..Status::default()
        }
    }

    fn tool(name: &str, file_path: &str) -> ToolUse {
        ToolUse {
            id: format!("toolu_{name}"),
            name: name.to_string(),
            input: json!({ "file_path": file_path }),
            result_chars: None,
        }
    }

    #[test]
    fn relativize_keeps_the_repo_and_drops_the_rest() {
        let repo = Path::new("/r");
        let rel = |raw: &str| relativize(repo, repo, raw);
        assert_eq!(rel("/r/src/a.rs").as_deref(), Some("src/a.rs"));
        assert_eq!(rel("src/./a/../b.rs").as_deref(), Some("src/b.rs"));
        assert_eq!(rel("./x").as_deref(), Some("x"));
        assert_eq!(rel("/elsewhere/a.rs"), None);
        assert_eq!(rel("../a.rs"), None);
        assert_eq!(rel("/r"), None);
        assert_eq!(rel(""), None);
    }

    #[test]
    fn pathspec_is_the_written_paths_plus_the_slice_file_and_fragment() {
        let repo = Path::new("/r");
        let mut summary = Summary::dead();
        summary.tool_uses = vec![
            tool("Read", "/r/src/read.rs"),
            tool("Write", "/r/src/a.rs"),
            tool("Edit", "src/a.rs"),
            tool("NotebookEdit", "/r/nb.ipynb"),
            tool("Write", "/r/docs/features/001-fixture/prd.md"),
            tool("Edit", "/r/docs/index.md"),
            tool("Write", "/tmp/elsewhere.txt"),
            tool("Write", "/r/docs/features/001-fixture/log-slice-01.md"),
        ];
        let spec = Pathspec::from(&summary, &board(), 1, repo);
        assert_eq!(
            spec.paths,
            [
                "src/a.rs",
                "nb.ipynb",
                "docs/features/001-fixture/log-slice-01.md",
                "docs/features/001-fixture/slice-01-a.md",
            ]
        );
        assert_eq!(
            spec.protected,
            ["docs/features/001-fixture/prd.md", "docs/index.md"]
        );
        assert_eq!(spec.outside, ["/tmp/elsewhere.txt"]);
    }

    #[test]
    fn fragment_names() {
        assert!(is_fragment("log-slice-01.md"));
        assert!(is_fragment("log-fix-3.md"));
        assert!(!is_fragment("log-slice-.md"));
        assert!(!is_fragment("agent-log.md"));
        assert!(!is_fragment("log-slice-01.md.bak"));
    }

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
        assert_eq!(untracked(r).unwrap(), ["new.txt", "stray.txt"]);

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
        // Nothing changed: no commit.
        assert_eq!(commit(r, &paths, "feat: again").unwrap(), None);
        assert_eq!(commit(r, &[], "feat: none").unwrap(), None);
    }
}
