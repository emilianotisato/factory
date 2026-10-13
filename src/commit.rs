//! One commit per slice, by pathspec (PRD §7 FR-5). The paths are the
//! `file_path` of every `Edit` and `Write` call and the `notebook_path` of
//! every `NotebookEdit` call in the worker's stream (§4 FR-2: the stream is the record) plus the slice file,
//! the feature's `state.toml` and `log.jsonl` (PRD-002 FR-18); the message is `feat(NNN): slice-NN <title>`, no body, no
//! trailers, the repo's own `user.*`. Never `git add -A`: another lane may be
//! mid-write in the same tree, and only named paths move. A `Bash` call that
//! types a file out in full (`cat > f <<EOF`) names it too ([`heredoc_paths`]).
//! A file the worker made with a command (a generator, a script) is its own
//! once it `git add`s it by name or by folder ([`added_paths`]); one it never
//! claims is a `note` entry in the feature's log and stays in the tree for
//! the owner. At a checkpoint (FR-4) the same path commits a fix worker's files
//! (`fix(NNN): gate red at checkpoint n`, [`Pathspec::from_stream`]) and the
//! feature folder and the checkpoint worker's files as `docs(NNN): checkpoint n`.
//!
//! With lanes ≥ 2 a slice's commit stages the whole `log.jsonl` and
//! `state.toml`, a sibling's in-flight lines with it: both files are
//! append-only or keyed per slice, so that is correct.
use std::collections::{BTreeMap, HashSet};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Output};
use std::sync::Mutex;

use crate::docs::Status;
use crate::shell;
use crate::stream::Summary;
use crate::{Error, Result};

/// The tools whose input names a file the worker wrote ([`written_paths`]).
pub const WRITING_TOOLS: &[&str] = &["Edit", "Write", "NotebookEdit"];

/// The two files no worker may write (`deliver/SKILL.md`): they are the
/// orchestrator's.
pub fn protected_files(board: &Status) -> [String; 2] {
    [format!("{}/prd.md", board.dir), "docs/index.md".to_string()]
}

/// The feature's `state.toml` and `log.jsonl`, repo-relative: the tool's
/// writes a slice or checkpoint commit carries (FR-18).
pub fn tool_files(board: &Status) -> [String; 2] {
    [
        format!("{}/state.toml", board.dir),
        format!("{}/log.jsonl", board.dir),
    ]
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
    /// Repo-relative, deduplicated, in stream order; the slice file,
    /// `state.toml` and `log.jsonl` last.
    pub paths: Vec<String>,
    /// Named in the stream but one of [`protected_files`]: left out.
    pub protected: Vec<String>,
    /// Named in the stream but outside the repo root: left out.
    pub outside: Vec<String>,
    /// Directories a `git add` named: their new files are the worker's only
    /// while no other worker is live ([`crate::scheduler::unnamed`]).
    pub dirs: Vec<String>,
}

impl Pathspec {
    /// The slice's pathspec from its worker's stream and the board.
    pub fn from(summary: &Summary, board: &Status, slice: u32, repo: &Path) -> Pathspec {
        let mut spec = Pathspec::from_stream(summary, board, repo);
        if let Some(p) = slice_path(board, slice) {
            push_once(&mut spec.paths, p);
        }
        for p in tool_files(board) {
            push_once(&mut spec.paths, p);
        }
        spec
    }

    /// The paths a worker's stream names and nothing else: a fix or checkpoint
    /// worker (FR-4) has no slice file of its own. The checkpoint worker's
    /// caller stages the feature folder whole, so the protected split is only
    /// informative here.
    pub fn from_stream(summary: &Summary, board: &Status, repo: &Path) -> Pathspec {
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
        // A heredoc into /dev/null or /tmp is not a file of the repo's, nor
        // worth reporting as one written outside it.
        for raw in heredoc_paths(summary) {
            match relativize(repo, &canon, &raw) {
                Some(rel) if protected.contains(&rel) => push_once(&mut spec.protected, rel),
                Some(rel) => push_once(&mut spec.paths, rel),
                None => {}
            }
        }
        // `symlink_metadata`, not `is_dir`: a symlink to a folder is one path
        // git stores as a link, and classed as a dir only what changed under
        // it was ever swept, never the link itself.
        for raw in added_paths(summary) {
            match relativize(repo, &canon, &raw) {
                Some(rel) if real_dir(&repo.join(&rel)) => push_once(&mut spec.dirs, rel),
                Some(rel) if protected.contains(&rel) => push_once(&mut spec.protected, rel),
                Some(rel) => push_once(&mut spec.paths, rel),
                None => {}
            }
        }
        spec
    }
}

/// The path every writing tool call in the stream names, as given:
/// `file_path` for `Edit` and `Write`, `notebook_path` for `NotebookEdit`.
pub fn written_paths(summary: &Summary) -> impl Iterator<Item = &str> {
    summary
        .tool_uses
        .iter()
        .filter(|t| WRITING_TOOLS.contains(&t.name.as_str()))
        .filter_map(|t| {
            let key = match t.name.as_str() {
                "NotebookEdit" => "notebook_path",
                _ => "file_path",
            };
            t.input.get(key).and_then(|v| v.as_str())
        })
}

/// The files the stream's `Bash` calls typed out in full: the targets of
/// `cat > f <<EOF` and `tee f <<EOF`, fed by a here-document or here-string.
/// The worker wrote those as surely as with `Write`; left unnamed, a
/// builder's new source files stayed out of its commit (the A/B run of
/// 2026-10-11 lost eleven that way). A call with a `cd` or `pushd` anywhere,
/// or a target spelled by an expansion or a glob, names nothing: where it
/// landed cannot be told. A relative target is read from the repo root,
/// where workers start; one that landed elsewhere names a path that does not
/// exist, which [`commit`] drops.
pub fn heredoc_paths(summary: &Summary) -> Vec<String> {
    let mut out = Vec::new();
    for sc in rooted_scripts(summary) {
        for c in sc.commands.iter().filter(|c| !c.stdin.is_empty()) {
            let targets: Vec<&String> = match program(c) {
                Some("cat") if c.words[1..].iter().all(|a| a == "-") => c.writes.iter().collect(),
                Some("tee") => c.words[1..]
                    .iter()
                    .filter(|a| !a.starts_with('-'))
                    .chain(&c.writes)
                    .collect(),
                _ => continue,
            };
            for p in targets.into_iter().filter(|p| literal(p)) {
                push_once(&mut out, p.clone());
            }
        }
    }
    out
}

/// The paths the stream's `git add` calls name one by one: the worker's
/// claim on a file it made with a command (a generator, a script), which
/// the driver cannot otherwise tell from a scratch file or a secret.
/// `git add -A`, `git add .`, a glob and pathspec magic (`:/`) name nothing
/// in particular, so they claim nothing; nor does `git add -n`, nor a
/// `git -C dir add`, which adds elsewhere.
pub fn added_paths(summary: &Summary) -> Vec<String> {
    let mut out = Vec::new();
    for sc in rooted_scripts(summary) {
        for c in &sc.commands {
            if program(c) != Some("git") || c.words.get(1).map(String::as_str) != Some("add") {
                continue;
            }
            let args = &c.words[2..];
            let end = args.iter().position(|a| a == "--").unwrap_or(args.len());
            if args[..end].iter().any(|a| a == "-n" || a == "--dry-run") {
                continue;
            }
            let named = args[..end]
                .iter()
                .filter(|a| !a.starts_with('-'))
                .chain(args.get(end + 1..).unwrap_or_default());
            let whole_tree = |p: &str| p.split('/').all(|s| s.is_empty() || s == ".");
            for p in named.filter(|p| literal(p) && !p.starts_with(':') && !whole_tree(p)) {
                push_once(&mut out, p.clone());
            }
        }
    }
    out
}

/// The stream's `Bash` calls, parsed, less those whose relative paths need
/// not be read from the repo root: an unbalanced one, or one with a `cd` or
/// `pushd` anywhere.
fn rooted_scripts(summary: &Summary) -> impl Iterator<Item = shell::Script> + '_ {
    summary
        .tool_uses
        .iter()
        .filter(|t| t.name == "Bash")
        .filter_map(|t| t.input.get("command").and_then(|v| v.as_str()))
        .map(shell::script)
        .filter(|sc| {
            !sc.unbalanced
                && !sc
                    .commands
                    .iter()
                    .any(|c| matches!(program(c), Some("cd" | "pushd")))
        })
}

fn program(c: &shell::Simple) -> Option<&str> {
    c.words.first().map(|w| shell::program(w))
}

/// A path spelled out, not by an expansion or a glob.
fn literal(p: &str) -> bool {
    !p.is_empty() && !p.contains(['$', '`', '*', '?', '[', '~'])
}

/// A directory itself, not a symlink to one.
fn real_dir(p: &Path) -> bool {
    std::fs::symlink_metadata(p).is_ok_and(|m| m.is_dir())
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

/// A path `git status` reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Changed {
    pub path: String,
    /// In HEAD and changed against it, staged or not; `false` for an
    /// untracked file and for one only staged as new, since `git add -A`
    /// stages every scratch file too ([`added_paths`] names a real claim).
    pub tracked: bool,
}

/// Every changed path in the tree, repo-relative and sorted: tracked files
/// changed against HEAD (both sides of a rename) and untracked, non-ignored
/// files — `git status --porcelain -z --untracked-files=all`.
pub fn changed(repo: &Path) -> Result<Vec<Changed>> {
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
        let tracked = xy != "??" && !xy.starts_with('A');
        if xy == "!!" {
            continue;
        }
        // A rename or copy carries the original path in the next field.
        if xy.contains(['R', 'C']) {
            let orig = entries.next();
            // A copy leaves its original as it was; a rename deletes it.
            if let Some(orig) = orig
                && xy.contains('R')
            {
                files.push(Changed {
                    path: orig.to_string(),
                    tracked: true,
                });
            }
        }
        files.push(Changed {
            path: path.to_string(),
            tracked,
        });
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    files.dedup_by(|a, b| a.path == b.path);
    Ok(files)
}

/// Untracked, non-ignored files in the tree, repo-relative and sorted: the
/// `??` entries of [`changed`].
pub fn untracked(repo: &Path) -> Result<Vec<String>> {
    Ok(changed(repo)?
        .into_iter()
        .filter(|c| !c.tracked)
        .map(|c| c.path)
        .collect())
}

/// Whether the tree has a change outside `docs/`: what the gate runs on is
/// then not what HEAD holds.
pub fn dirty_outside_docs(repo: &Path) -> Result<bool> {
    Ok(changed(repo)?.iter().any(|c| !c.path.starts_with("docs/")))
}

/// The blob id `git hash-object` gives each of `paths` as it stands in the
/// tree, "" for one that is not there (a deletion).
pub fn blobs(repo: &Path, paths: &[String]) -> Result<BTreeMap<String, String>> {
    let mut out: BTreeMap<String, String> =
        paths.iter().map(|p| (p.clone(), String::new())).collect();
    let present: Vec<&str> = paths
        .iter()
        .filter(|p| repo.join(p).is_file())
        .map(String::as_str)
        .collect();
    if present.is_empty() {
        return Ok(out);
    }
    let mut args = vec!["hash-object", "--"];
    args.extend(&present);
    for (p, id) in present.iter().zip(git(repo, &args)?.lines()) {
        out.insert(p.to_string(), id.trim().to_string());
    }
    Ok(out)
}

/// What a commit landed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Committed {
    pub sha: String,
    /// The paths staged, in pathspec order.
    pub files: Vec<String>,
}

/// `git add -- <paths>` and `git commit -m <message> -- <paths>` in the
/// repo root, so nothing else staged rides along: no
/// author set, no body, hooks on. Paths that are neither in the index nor in
/// the tree (never written, or written and removed again) and untracked
/// paths the repo ignores are dropped first — `git add` refuses either. A
/// tracked path the worker deleted stays and stages the deletion. `None`
/// when nothing was staged.
///
/// A repo held to a branch ([`hold_branch`]) refuses with an error while
/// HEAD is anywhere else.
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
    // Check and commit by the same pathspec: whatever else sits in the index
    // (an owner's `git add`, a sibling lane's stage) is neither counted nor
    // swept in, and stays staged.
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

/// The branch HEAD is on; `None` when HEAD is detached.
pub fn head_branch(repo: &Path) -> Result<Option<String>> {
    let out = run_git(repo, &["symbolic-ref", "-q", "--short", "HEAD"])?;
    let name = String::from_utf8_lossy(&out.stdout).trim().to_string();
    Ok((out.status.success() && !name.is_empty()).then_some(name))
}

/// Why the driver may not commit for a run on `branch`: the branch is the
/// repo's default branch, or HEAD is on another branch or detached (the
/// owner switched mid-run). `None` when a commit may land.
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

/// The branch each repo's commits are held to in this process.
static HELD: Mutex<Vec<(PathBuf, String, Option<String>)>> = Mutex::new(Vec::new());

/// Hold every later [`commit`] in `repo` by this process to `branch` (and
/// refuse them all when `branch` is `default`). The driver calls it once it
/// knows the run's branch, so a slice, fix, checkpoint, polish, ship or CI
/// commit errors instead of landing on a branch the owner switched to
/// between the driver's check and the commit.
pub fn hold_branch(repo: &Path, branch: &str, default: Option<&str>) {
    let key = canonical(repo);
    let mut held = HELD.lock().unwrap_or_else(|p| p.into_inner());
    held.retain(|(r, _, _)| *r != key);
    held.push((key, branch.to_string(), default.map(str::to_string)));
}

fn held(repo: &Path) -> Option<(String, Option<String>)> {
    let key = canonical(repo);
    let held = HELD.lock().unwrap_or_else(|p| p.into_inner());
    held.iter()
        .find(|(r, _, _)| *r == key)
        .map(|(_, b, d)| (b.clone(), d.clone()))
}

fn canonical(repo: &Path) -> PathBuf {
    repo.canonicalize().unwrap_or_else(|_| repo.to_path_buf())
}

/// What `git` knows under `paths`: tracked files and untracked files it does
/// not ignore, each directory expanded to its files, so [`commit`] (which
/// stages exact names only) can take a folder.
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

/// A commit subject scoped to the feature, as `/deliver`'s builders write
/// theirs: `feat(018): slice-02 Harness`. The A/B of 2026-10-11 found the
/// driver's bare `feat: {title}` gave no feature and no slice to search by.
pub fn subject(board: &Status, kind: &str, text: &str) -> String {
    format!("{kind}({:03}): {text}", board.number)
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
    use std::fs;

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
        // The input key Claude Code sends for each tool.
        let input = match name {
            "NotebookEdit" => json!({ "notebook_path": file_path, "new_source": "x" }),
            _ => json!({ "file_path": file_path }),
        };
        ToolUse {
            id: format!("toolu_{name}"),
            name: name.to_string(),
            input,
            result_chars: None,
            ts: None,
            result_ts: None,
            result_error: false,
            result_head: None,
            in_subagent: false,
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
    fn pathspec_is_the_written_paths_plus_the_slice_file_state_and_log() {
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
        ];
        let spec = Pathspec::from(&summary, &board(), 1, repo);
        assert_eq!(
            spec.paths,
            [
                "src/a.rs",
                "nb.ipynb",
                "docs/features/001-fixture/slice-01-a.md",
                "docs/features/001-fixture/state.toml",
                "docs/features/001-fixture/log.jsonl",
            ]
        );
        assert_eq!(
            spec.protected,
            ["docs/features/001-fixture/prd.md", "docs/index.md"]
        );
        assert_eq!(spec.outside, ["/tmp/elsewhere.txt"]);
    }

    #[test]
    fn a_git_add_claims_only_the_paths_it_spells_out() {
        let added = |cmds: &[&str]| {
            let mut summary = Summary::dead();
            summary.tool_uses = cmds
                .iter()
                .map(|c| ToolUse {
                    input: json!({ "command": c }),
                    ..tool("Bash", "")
                })
                .collect();
            added_paths(&summary)
        };
        assert_eq!(added(&["git add src/a.rs gen"]), ["src/a.rs", "gen"]);
        assert_eq!(added(&["git add -f -- -odd.rs"]), ["-odd.rs"]);
        assert_eq!(added(&["make gen && git add gen/out.rs"]), ["gen/out.rs"]);
        for none in [
            "git add -A",
            "git add .",
            "git add ./",
            "git add --all",
            "git add src/*.rs",
            "git add :/",
            "git add -n src/a.rs",
            "git -C sub add a.rs",
            "cd sub && git add a.rs",
            "git status",
        ] {
            assert!(added(&[none]).is_empty(), "{none}");
        }
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

    #[cfg(unix)]
    #[test]
    fn a_git_add_of_a_symlink_to_a_folder_claims_the_link_as_a_path() {
        let dir = repo();
        let r = dir.path();
        fs::create_dir_all(r.join("skills/prd")).unwrap();
        fs::write(r.join("skills/prd/SKILL.md"), "s\n").unwrap();
        fs::create_dir_all(r.join(".claude/skills")).unwrap();
        std::os::unix::fs::symlink("../../skills/prd", r.join(".claude/skills/prd")).unwrap();
        let mut summary = Summary::dead();
        summary.tool_uses = vec![ToolUse {
            input: json!({ "command": "git add .claude/skills/prd skills" }),
            ..tool("Bash", "")
        }];
        let spec = Pathspec::from_stream(&summary, &board(), r);
        assert_eq!(spec.paths, [".claude/skills/prd"]);
        assert_eq!(spec.dirs, ["skills"]);
        let c = commit(r, &spec.paths, "feat: link").unwrap().unwrap();
        assert_eq!(c.files, [".claude/skills/prd"]);
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

        // A run whose branch is the default branch commits nothing.
        hold_branch(r, "feat/x", Some("feat/x"));
        fs::write(r.join("kept.txt"), "k3\n").unwrap();
        let err = commit(r, &mine, "feat: default").unwrap_err().to_string();
        assert!(err.contains("is the default branch"), "{err}");
    }

    #[test]
    fn changed_names_tracked_and_untracked_paths_apart() {
        let dir = repo();
        let r = dir.path();
        fs::write(r.join("kept.txt"), "k2\n").unwrap();
        fs::write(r.join("new.txt"), "n\n").unwrap();
        fs::create_dir_all(r.join("ignored")).unwrap();
        fs::write(r.join("ignored/x.txt"), "x\n").unwrap();
        git(r, &["mv", "gone.txt", "moved.txt"]).unwrap();
        let got: Vec<(String, bool)> = changed(r)
            .unwrap()
            .into_iter()
            .map(|c| (c.path, c.tracked))
            .collect();
        let want: Vec<(String, bool)> = [
            ("gone.txt", true),
            ("kept.txt", true),
            ("moved.txt", true),
            ("new.txt", false),
        ]
        .map(|(p, t)| (p.to_string(), t))
        .to_vec();
        assert_eq!(got, want);
        assert_eq!(untracked(r).unwrap(), ["new.txt"]);
        assert!(dirty_outside_docs(r).unwrap());
    }

    #[test]
    fn commit_leaves_what_else_is_staged_out_and_staged() {
        let dir = repo();
        let r = dir.path();
        fs::write(r.join("owner.txt"), "o\n").unwrap();
        git(r, &["add", "--", "owner.txt"]).unwrap();
        // The worker's path is unchanged, the owner's is staged: no commit.
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
