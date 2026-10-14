//! A lane commits whole (`git add -A`: only its worker writes there); the main tree commits by pathspec.
use std::collections::HashSet;
use std::path::Path;
use std::process::{Command, Output};

use crate::commit;
use crate::{Error, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaneCommit {
    pub sha: String,
    /// Every path the commit touches; a rename is its two paths.
    pub files: Vec<String>,
    pub dropped: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Landing {
    /// The main tree's new HEAD.
    Landed {
        sha: String,
    },
    Conflict {
        paths: Vec<String>,
    },
    Dirty {
        paths: Vec<String>,
    },
}

pub fn commit_lane(
    lane: &Path,
    base: &str,
    board: &[String],
    protected: &[String],
    message: &str,
    ref_name: &str,
) -> Result<Option<LaneCommit>> {
    // `reset --soft` refuses unmerged entries, which a merge worker's lane can hold.
    if !git(lane, &["ls-files", "-u"])?.trim().is_empty() {
        git(lane, &["add", "-A"])?;
    }
    // Detach first: `reset --soft` would move a branch the worker switched to.
    let head = git(lane, &["rev-parse", "HEAD"])?.trim().to_string();
    git(lane, &["update-ref", "--no-deref", "HEAD", &head])?;
    git(lane, &["reset", "-q", "--soft", base])?;
    git(lane, &["add", "-A"])?;
    let drop: Vec<&str> = board.iter().chain(protected).map(String::as_str).collect();
    let staged = if drop.is_empty() {
        Vec::new()
    } else {
        let mut diff = vec![
            "diff",
            "--cached",
            "--name-only",
            "--no-renames",
            "-z",
            "--",
        ];
        diff.extend(&drop);
        nul_list(&git(lane, &diff)?)
    };
    if !staged.is_empty() {
        let mut reset = vec!["reset", "-q", "--"];
        reset.extend(staged.iter().map(String::as_str));
        git(lane, &reset)?;
    }
    if code(lane, &["diff", "--cached", "--quiet"])? == 0 {
        return Ok(None);
    }
    git(lane, &["commit", "-q", "-m", message])?;
    let sha = git(lane, &["rev-parse", "HEAD"])?.trim().to_string();
    git(lane, &["update-ref", ref_name, &sha])?;
    let files = nul_list(&git(
        lane,
        &[
            "diff-tree",
            "--no-commit-id",
            "--name-only",
            "-r",
            "-z",
            &sha,
        ],
    )?);
    let dropped = protected
        .iter()
        .filter(|p| staged.contains(p))
        .cloned()
        .collect();
    Ok(Some(LaneCommit {
        sha,
        files,
        dropped,
    }))
}

/// Board files may be dirty in the main tree: workers write them there by design.
pub fn land(main: &Path, sha: &str, board: &[String]) -> Result<Landing> {
    if let Some((branch, default)) = commit::held(main)
        && let Some(why) = commit::branch_refusal(main, &branch, default.as_deref())?
    {
        return Err(Error::Msg(format!("landing refused: {why}")));
    }
    let parent = format!("{sha}^");
    if code(main, &["diff", "--quiet", &parent, sha])? == 0 {
        return Err(Error::Msg(format!("lane commit {sha} changes nothing")));
    }
    // `merge-tree` works on objects alone: a conflict leaves nothing to abort.
    let merge = run_git(
        main,
        &[
            "merge-tree",
            "--write-tree",
            "--name-only",
            "--no-messages",
            "-z",
            "--merge-base",
            &parent,
            "HEAD",
            sha,
        ],
    )?;
    let out = String::from_utf8_lossy(&merge.stdout);
    let mut fields = nul_list(&out).into_iter();
    let tree = fields.next().unwrap_or_default();
    match merge.status.code() {
        Some(0) => {}
        Some(1) => {
            return Ok(Landing::Conflict {
                paths: fields.collect(),
            });
        }
        _ => return Err(failed("merge-tree", &merge)),
    }
    // Read off the merged tree: a file the tip renamed takes the change under its new name.
    let picked = nul_list(&git(
        main,
        &["diff", "--name-only", "--no-renames", "-z", "HEAD", &tree],
    )?);
    if picked.is_empty() {
        return Err(Error::Msg(format!(
            "lane commit {sha} changes nothing on the feature tip"
        )));
    }
    let dirty: Vec<String> = dirty(main, &picked)?
        .into_iter()
        .filter(|p| !board.contains(p))
        .collect();
    if !dirty.is_empty() {
        return Ok(Landing::Dirty { paths: dirty });
    }
    // `--no-commit` lets the index differ from HEAD: an owner's staged file elsewhere does not stop the pick.
    git(main, &["cherry-pick", "--no-commit", sha])?;
    let known = known(main, board)?;
    let boards: Vec<String> = board
        .iter()
        .filter(|p| known.contains(p.as_str()))
        .cloned()
        .collect();
    if let Err(e) = commit_pick(main, sha, &picked, &boards) {
        undo_pick(main, &picked, &boards);
        return Err(e);
    }
    let head = git(main, &["rev-parse", "HEAD"])?.trim().to_string();
    let refs = git(
        main,
        &[
            "for-each-ref",
            "--points-at",
            sha,
            "--format=%(refname)",
            "refs/factory/",
        ],
    )?;
    for r in refs.lines().filter(|l| !l.is_empty()) {
        git(main, &["update-ref", "-d", r])?;
    }
    Ok(Landing::Landed { sha: head })
}

fn commit_pick(main: &Path, sha: &str, picked: &[String], boards: &[String]) -> Result<()> {
    if !boards.is_empty() {
        let mut add = vec!["add", "--"];
        add.extend(boards.iter().map(String::as_str));
        git(main, &add)?;
    }
    let mut commit = vec!["commit", "-q", "-C", sha, "--"];
    commit.extend(picked.iter().map(String::as_str));
    commit.extend(
        boards
            .iter()
            .filter(|b| !picked.contains(b))
            .map(String::as_str),
    );
    git(main, &commit)?;
    Ok(())
}

/// Best effort: the error that brought us here is the one to report.
fn undo_pick(main: &Path, picked: &[String], boards: &[String]) {
    let mut restore = vec!["restore", "--source=HEAD", "--staged", "--worktree", "--"];
    restore.extend(picked.iter().map(String::as_str));
    let _ = run_git(main, &restore);
    if !boards.is_empty() {
        let mut reset = vec!["reset", "-q", "--"];
        reset.extend(boards.iter().map(String::as_str));
        let _ = run_git(main, &reset);
    }
}

fn dirty(repo: &Path, paths: &[String]) -> Result<Vec<String>> {
    let mut status = vec![
        "status",
        "--porcelain",
        "-z",
        "--untracked-files=all",
        "--ignored=matching",
        "--",
    ];
    status.extend(paths.iter().map(String::as_str));
    let out = git(repo, &status)?;
    let mut fields = out.split('\0').filter(|s| !s.is_empty());
    let mut dirty = Vec::new();
    while let Some(entry) = fields.next() {
        let (xy, path) = entry.split_at(3.min(entry.len()));
        // A rename or copy carries its source as the next field.
        if xy.starts_with(['R', 'C'])
            && let Some(from) = fields.next()
        {
            dirty.push(from.to_string());
        }
        dirty.push(path.to_string());
    }
    Ok(dirty)
}

fn known(repo: &Path, paths: &[String]) -> Result<HashSet<String>> {
    if paths.is_empty() {
        return Ok(HashSet::new());
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
    Ok(nul_list(&git(repo, &ls)?).into_iter().collect())
}

fn nul_list(out: &str) -> Vec<String> {
    out.split('\0')
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

fn git(repo: &Path, args: &[&str]) -> Result<String> {
    let out = run_git(repo, args)?;
    if !out.status.success() {
        return Err(failed(&args.join(" "), &out));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn code(repo: &Path, args: &[&str]) -> Result<i32> {
    Ok(run_git(repo, args)?.status.code().unwrap_or(-1))
}

fn failed(args: &str, out: &Output) -> Error {
    Error::Tool {
        cmd: format!("git {args}"),
        detail: format!(
            "exit {}: {}",
            out.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&out.stderr).trim()
        ),
    }
}

/// Paths are file names, never patterns: a worker's `[a].txt` must not match `a.txt`.
fn run_git(repo: &Path, args: &[&str]) -> Result<Output> {
    Command::new("git")
        .args(args)
        .env("GIT_LITERAL_PATHSPECS", "1")
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
    use std::path::PathBuf;

    const STATE: &str = "docs/features/005-x/state.toml";
    const LOG: &str = "docs/features/005-x/log.jsonl";
    const PRD: &str = "docs/features/005-x/prd.md";
    const INDEX: &str = "docs/index.md";

    fn ok(repo: &Path, args: &[&str]) -> String {
        git(repo, args).unwrap()
    }

    fn write(root: &Path, path: &str, body: &str) {
        let p = root.join(path);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, body).unwrap();
    }

    fn strings(paths: &[&str]) -> Vec<String> {
        paths.iter().map(|p| p.to_string()).collect()
    }

    struct Repo {
        main: tempfile::TempDir,
        lanes: tempfile::TempDir,
    }

    impl Repo {
        fn new() -> Repo {
            let main = tempfile::tempdir().unwrap();
            let r = main.path();
            for args in [
                vec!["init", "-q", "-b", "feat/x"],
                vec!["config", "user.name", "T"],
                vec!["config", "user.email", "t@example.invalid"],
                vec!["config", "core.hooksPath", "/nonexistent-hooks"],
            ] {
                ok(r, &args);
            }
            for p in [STATE, LOG, PRD, INDEX] {
                write(r, p, "board\n");
            }
            write(r, "a.txt", "a\n");
            write(r, "old.txt", "o\n");
            write(r, "unrelated.txt", "u\n");
            write(r, "shared.txt", "1\n2\n3\n4\n5\n");
            ok(r, &["add", "-A"]);
            ok(r, &["commit", "-q", "-m", "init"]);
            Repo {
                main,
                lanes: tempfile::tempdir().unwrap(),
            }
        }

        fn main(&self) -> &Path {
            self.main.path()
        }

        fn lane(&self, name: &str) -> (PathBuf, String) {
            let lane = self.lanes.path().join(name);
            ok(
                self.main(),
                &["worktree", "add", "-q", "--detach", lane.to_str().unwrap()],
            );
            let base = ok(&lane, &["rev-parse", "HEAD"]).trim().to_string();
            (lane, base)
        }
    }

    fn board() -> Vec<String> {
        strings(&[STATE, LOG])
    }

    fn protected() -> Vec<String> {
        strings(&[PRD, INDEX])
    }

    fn commit(lane: &Path, base: &str, message: &str, key: &str) -> Option<LaneCommit> {
        commit_lane(
            lane,
            base,
            &board(),
            &protected(),
            message,
            &format!("refs/factory/005/{key}"),
        )
        .unwrap()
    }

    fn head(repo: &Path) -> String {
        ok(repo, &["rev-parse", "HEAD"]).trim().to_string()
    }

    fn parents(repo: &Path, rev: &str) -> Vec<String> {
        ok(repo, &["rev-list", "--parents", "-n", "1", rev])
            .split_whitespace()
            .skip(1)
            .map(str::to_string)
            .collect()
    }

    #[test]
    fn a_lane_commits_whole_on_its_base() {
        let repo = Repo::new();
        let (lane, base) = repo.lane("l1");
        write(&lane, "a.txt", "a2\n");
        ok(&lane, &["mv", "old.txt", "new.txt"]);
        write(&lane, "dir/inner.txt", "i\n");
        std::os::unix::fs::symlink("dir", lane.join("link")).unwrap();

        let c = commit(&lane, &base, "feat(005): slice-01 a", "slice-01").unwrap();
        assert_eq!(parents(&lane, &c.sha), [base]);
        for p in ["a.txt", "old.txt", "new.txt", "dir/inner.txt", "link"] {
            assert!(c.files.contains(&p.to_string()), "{p} in {:?}", c.files);
        }
        assert!(c.dropped.is_empty());
        assert_eq!(
            ok(repo.main(), &["rev-parse", "refs/factory/005/slice-01"]).trim(),
            c.sha
        );
    }

    #[test]
    fn a_commit_the_worker_made_is_squashed_into_the_lane_commit() {
        let repo = Repo::new();
        let (lane, base) = repo.lane("l1");
        write(&lane, "self.txt", "s\n");
        ok(&lane, &["add", "self.txt"]);
        ok(&lane, &["commit", "-q", "-m", "worker's own"]);
        write(&lane, "loose.txt", "l\n");

        let c = commit(&lane, &base, "feat(005): slice-01 a", "slice-01").unwrap();
        assert_eq!(parents(&lane, &c.sha), [base]);
        assert_eq!(c.files, ["loose.txt", "self.txt"]);
        assert_eq!(
            ok(&lane, &["log", "-1", "--format=%s", &c.sha]).trim(),
            "feat(005): slice-01 a"
        );
    }

    #[test]
    fn board_and_protected_files_stay_out_and_only_protected_ones_are_named() {
        let repo = Repo::new();
        let (lane, base) = repo.lane("l1");
        for p in [STATE, LOG, PRD, INDEX] {
            write(&lane, p, "worker\n");
        }
        write(&lane, "a.txt", "a2\n");

        let c = commit(&lane, &base, "feat(005): slice-01 a", "slice-01").unwrap();
        assert_eq!(c.files, ["a.txt"]);
        assert_eq!(c.dropped, [PRD, INDEX]);
        assert_eq!(fs::read_to_string(lane.join(PRD)).unwrap(), "worker\n");
    }

    #[test]
    fn an_unchanged_lane_commits_nothing_and_makes_no_ref() {
        let repo = Repo::new();
        let (lane, base) = repo.lane("l1");
        write(&lane, STATE, "worker\n");
        assert_eq!(
            commit(&lane, &base, "feat(005): slice-01 a", "slice-01"),
            None
        );
        let refs = ok(repo.main(), &["for-each-ref", "refs/factory/"]);
        assert!(refs.is_empty(), "{refs}");
    }

    #[test]
    fn a_landed_lane_is_one_commit_with_the_main_tree_board() {
        let repo = Repo::new();
        let m = repo.main();
        let (lane, base) = repo.lane("l1");
        write(&lane, "a.txt", "a2\n");
        ok(&lane, &["mv", "old.txt", "new.txt"]);
        write(&lane, "dir/inner.txt", "i\n");
        let c = commit(&lane, &base, "feat(005): slice-01 a", "slice-01").unwrap();
        write(m, STATE, "main board\n");

        let Landing::Landed { sha } = land(m, &c.sha, &board()).unwrap() else {
            panic!("not landed");
        };
        assert_eq!(sha, head(m));
        assert_eq!(parents(m, "HEAD"), [base]);
        for p in ["a.txt", "new.txt", "dir/inner.txt"] {
            assert_eq!(
                ok(m, &["show", &format!("HEAD:{p}")]),
                ok(m, &["show", &format!("{}:{p}", c.sha)])
            );
        }
        assert!(
            run_git(m, &["cat-file", "-e", "HEAD:old.txt"])
                .unwrap()
                .status
                .code()
                != Some(0)
        );
        assert_eq!(ok(m, &["show", &format!("HEAD:{STATE}")]), "main board\n");
        assert_eq!(
            ok(m, &["log", "-1", "--format=%B", "HEAD"]),
            ok(m, &["log", "-1", "--format=%B", &c.sha])
        );
        assert_eq!(ok(m, &["status", "--porcelain"]), "");
        assert!(ok(m, &["for-each-ref", "refs/factory/"]).is_empty());
    }

    #[test]
    fn two_lanes_from_one_base_land_in_turn() {
        let repo = Repo::new();
        let m = repo.main();
        let (l1, base) = repo.lane("l1");
        let (l2, _) = repo.lane("l2");
        write(&l1, "one.txt", "1\n");
        write(&l2, "two.txt", "2\n");
        let c1 = commit(&l1, &base, "feat(005): slice-01 a", "slice-01").unwrap();
        let c2 = commit(&l2, &base, "feat(005): slice-02 b", "slice-02").unwrap();

        let Landing::Landed { sha: first } = land(m, &c1.sha, &board()).unwrap() else {
            panic!("first not landed");
        };
        let Landing::Landed { sha: second } = land(m, &c2.sha, &board()).unwrap() else {
            panic!("second not landed");
        };
        assert_eq!(parents(m, &first), [base]);
        assert_eq!(parents(m, &second), [first]);
        assert_eq!(
            ok(m, &["log", "-1", "--format=%s", &second]).trim(),
            "feat(005): slice-02 b"
        );
    }

    #[test]
    fn a_conflicting_pick_writes_nothing() {
        let repo = Repo::new();
        let m = repo.main();
        let (lane, base) = repo.lane("l1");
        write(m, "shared.txt", "1\n2\nmain\n4\n5\n");
        ok(m, &["commit", "-q", "-am", "main side"]);
        write(&lane, "shared.txt", "1\n2\nlane\n4\n5\n");
        let c = commit(&lane, &base, "feat(005): slice-01 a", "slice-01").unwrap();
        let before = head(m);

        assert_eq!(
            land(m, &c.sha, &board()).unwrap(),
            Landing::Conflict {
                paths: strings(&["shared.txt"])
            }
        );
        assert_eq!(head(m), before);
        assert_eq!(ok(m, &["status", "--porcelain"]), "");
        assert_eq!(
            ok(m, &["rev-parse", "refs/factory/005/slice-01"]).trim(),
            c.sha
        );
    }

    #[test]
    fn an_owner_edit_under_the_pick_parks_it_and_keeps_the_owner_bytes() {
        let repo = Repo::new();
        let m = repo.main();
        let (lane, base) = repo.lane("l1");
        write(&lane, "a.txt", "lane\n");
        write(&lane, "fresh.txt", "lane\n");
        let c = commit(&lane, &base, "feat(005): slice-01 a", "slice-01").unwrap();
        write(m, "a.txt", "owner\n");
        write(m, "fresh.txt", "owner\n");
        write(m, STATE, "main board\n");
        let before = head(m);

        assert_eq!(
            land(m, &c.sha, &board()).unwrap(),
            Landing::Dirty {
                paths: strings(&["a.txt", "fresh.txt"])
            }
        );
        assert_eq!(head(m), before);
        assert_eq!(fs::read_to_string(m.join("a.txt")).unwrap(), "owner\n");
        assert_eq!(fs::read_to_string(m.join("fresh.txt")).unwrap(), "owner\n");
    }

    #[test]
    fn an_owner_staged_edit_elsewhere_stays_staged_and_out() {
        let repo = Repo::new();
        let m = repo.main();
        let (lane, base) = repo.lane("l1");
        write(&lane, "a.txt", "lane\n");
        let c = commit(&lane, &base, "feat(005): slice-01 a", "slice-01").unwrap();
        write(m, "unrelated.txt", "owner\n");
        ok(m, &["add", "unrelated.txt"]);

        assert!(matches!(
            land(m, &c.sha, &board()).unwrap(),
            Landing::Landed { .. }
        ));
        let landed = ok(m, &["show", "--name-only", "--format=", "HEAD"]);
        assert_eq!(landed.trim(), "a.txt");
        assert_eq!(ok(m, &["status", "--porcelain"]), "M  unrelated.txt\n");
    }

    #[test]
    fn a_held_main_tree_off_its_branch_refuses_to_land() {
        let repo = Repo::new();
        let m = repo.main();
        let (lane, base) = repo.lane("l1");
        write(&lane, "a.txt", "lane\n");
        let c = commit(&lane, &base, "feat(005): slice-01 a", "slice-01").unwrap();
        commit::hold_branch(m, "feat/x", Some("main"));
        ok(m, &["switch", "-q", "-c", "other"]);
        let before = head(m);

        let err = land(m, &c.sha, &board()).unwrap_err().to_string();
        assert!(err.contains("feat/x"), "{err}");
        assert_eq!(head(m), before);
        assert_eq!(fs::read_to_string(m.join("a.txt")).unwrap(), "a\n");
    }

    #[test]
    fn a_change_to_a_file_the_tip_renamed_lands_under_its_new_name() {
        let repo = Repo::new();
        let m = repo.main();
        let (lane, base) = repo.lane("l1");
        ok(m, &["mv", "old.txt", "renamed.txt"]);
        ok(m, &["commit", "-q", "-m", "rename on the tip"]);
        write(&lane, "old.txt", "o2\n");
        let c = commit(&lane, &base, "feat(005): slice-01 a", "slice-01").unwrap();

        assert!(matches!(
            land(m, &c.sha, &board()).unwrap(),
            Landing::Landed { .. }
        ));
        assert_eq!(ok(m, &["show", "HEAD:renamed.txt"]), "o2\n");
        assert_eq!(ok(m, &["status", "--porcelain"]), "");
    }

    #[test]
    fn a_commit_that_fails_after_the_pick_leaves_the_main_tree_as_it_was() {
        let repo = Repo::new();
        let m = repo.main();
        let (lane, base) = repo.lane("l1");
        write(&lane, "a.txt", "lane\n");
        write(&lane, "fresh.txt", "lane\n");
        let c = commit(&lane, &base, "feat(005): slice-01 a", "slice-01").unwrap();
        let hooks = repo.lanes.path().join("hooks");
        write(&hooks, "pre-commit", "#!/bin/sh\nexit 1\n");
        let hook = hooks.join("pre-commit");
        let mut perm = fs::metadata(&hook).unwrap().permissions();
        std::os::unix::fs::PermissionsExt::set_mode(&mut perm, 0o755);
        fs::set_permissions(&hook, perm).unwrap();
        ok(m, &["config", "core.hooksPath", hooks.to_str().unwrap()]);
        write(m, STATE, "main board\n");
        let before = head(m);

        assert!(land(m, &c.sha, &board()).is_err());
        assert_eq!(head(m), before);
        assert_eq!(fs::read_to_string(m.join("a.txt")).unwrap(), "a\n");
        assert!(!m.join("fresh.txt").exists());
        assert_eq!(ok(m, &["status", "--porcelain"]), format!(" M {STATE}\n"));
        assert_eq!(
            ok(m, &["rev-parse", "refs/factory/005/slice-01"]).trim(),
            c.sha
        );
    }

    #[test]
    fn an_ignored_file_under_the_pick_parks_it() {
        let repo = Repo::new();
        let m = repo.main();
        write(m, ".gitignore", "gen.txt\n");
        ok(m, &["add", ".gitignore"]);
        ok(m, &["commit", "-q", "-m", "ignore gen"]);
        let (lane, base) = repo.lane("l1");
        write(&lane, "gen.txt", "lane\n");
        ok(&lane, &["add", "-f", "gen.txt"]);
        let c = commit(&lane, &base, "feat(005): slice-01 a", "slice-01").unwrap();
        write(m, "gen.txt", "owner\n");

        assert_eq!(
            land(m, &c.sha, &board()).unwrap(),
            Landing::Dirty {
                paths: strings(&["gen.txt"])
            }
        );
        assert_eq!(fs::read_to_string(m.join("gen.txt")).unwrap(), "owner\n");
    }

    #[test]
    fn a_branch_the_worker_switched_to_does_not_move() {
        let repo = Repo::new();
        let (lane, base) = repo.lane("l1");
        ok(&lane, &["switch", "-q", "-c", "side"]);
        write(&lane, "side.txt", "s\n");
        ok(&lane, &["add", "side.txt"]);
        ok(&lane, &["commit", "-q", "-m", "on side"]);
        let side = head(&lane);

        let c = commit(&lane, &base, "feat(005): slice-01 a", "slice-01").unwrap();
        assert_eq!(c.files, ["side.txt"]);
        assert_eq!(ok(&lane, &["rev-parse", "refs/heads/side"]).trim(), side);
    }

    #[test]
    fn a_path_that_reads_as_a_glob_matches_only_itself() {
        let repo = Repo::new();
        let m = repo.main();
        let (lane, base) = repo.lane("l1");
        write(&lane, "[a].txt", "lane\n");
        let c = commit(&lane, &base, "feat(005): slice-01 a", "slice-01").unwrap();
        write(m, "a.txt", "owner\n");

        assert!(matches!(
            land(m, &c.sha, &board()).unwrap(),
            Landing::Landed { .. }
        ));
        let landed = ok(m, &["show", "--name-only", "--format=", "HEAD"]);
        assert_eq!(landed.trim(), "[a].txt");
        assert_eq!(ok(m, &["status", "--porcelain"]), " M a.txt\n");
    }
}
