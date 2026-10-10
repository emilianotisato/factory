//! The gate cache (PRD §6 FR-4): "a cache keyed on the code paths (hash of
//! the tree minus `docs/**`) skips the rerun when nothing under test
//! changed; only driver runs populate it." The key is the id `git
//! write-tree` gives the working tree — tracked, modified, untracked and not
//! ignored, `docs/` left out — built on a throwaway index so the repo's own
//! index is never touched, plus the gate command. Hits and misses live in
//! `runs/<id>/gate-cache.json`, so a resumed run reuses a green gate.
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

use crate::error::io;
use crate::{Error, Result};

pub const FILE: &str = "gate-cache.json";

/// One cached gate result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    pub red: bool,
    pub minutes: f64,
    pub at: String,
    /// The transcript of the run that produced it.
    pub log: String,
}

/// `runs/<id>/gate-cache.json`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct GateCache {
    path: PathBuf,
    entries: BTreeMap<String, Entry>,
}

impl GateCache {
    /// Load the run's cache, empty when the file is not there yet.
    pub fn load(run_dir: &Path) -> Result<GateCache> {
        let path = run_dir.join(FILE);
        let entries = match fs::read_to_string(&path) {
            Ok(text) => serde_json::from_str(&text)
                .map_err(|e| Error::Msg(format!("{}: {e}", path.display())))?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => BTreeMap::new(),
            Err(e) => return Err(io(format!("read {}", path.display()))(e)),
        };
        Ok(GateCache { path, entries })
    }

    pub fn get(&self, key: &str) -> Option<&Entry> {
        self.entries.get(key)
    }

    /// Remember `entry` under `key` and write the file.
    pub fn put(&mut self, key: &str, entry: Entry) -> Result<()> {
        self.entries.insert(key.to_string(), entry);
        let text = serde_json::to_string_pretty(&self.entries)
            .map_err(|e| Error::Msg(format!("gate cache: {e}")))?;
        fs::write(&self.path, text).map_err(io(format!("write {}", self.path.display())))?;
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// The id of the working tree minus `docs/`, from a throwaway index at
/// `index_path`: `read-tree --empty`, `add -A -- . ':(exclude)docs'`,
/// `write-tree`. Uncommitted and untracked (not ignored) files count; a
/// committed change to `docs/` alone does not change it.
pub fn tree_id(repo: &Path, index_path: &Path) -> Result<String> {
    if let Some(parent) = index_path.parent() {
        fs::create_dir_all(parent).map_err(io(format!("mkdir {}", parent.display())))?;
    }
    let _ = fs::remove_file(index_path);
    let result = (|| {
        git(repo, index_path, &["read-tree", "--empty"])?;
        git(
            repo,
            index_path,
            &["add", "-A", "--", ".", ":(exclude)docs"],
        )?;
        let tree = git(repo, index_path, &["write-tree"])?;
        Ok(tree.trim().to_string())
    })();
    let _ = fs::remove_file(index_path);
    result
}

/// The cache key: [`tree_id`] and the gate command.
pub fn tree_key(repo: &Path, index_path: &Path, gate: &str) -> Result<String> {
    Ok(format!("{} {}", tree_id(repo, index_path)?, gate.trim()))
}

fn git(repo: &Path, index_path: &Path, args: &[&str]) -> Result<String> {
    let out = Command::new("git")
        .args(args)
        .env("GIT_INDEX_FILE", index_path)
        .current_dir(repo)
        .output()
        .map_err(|e| Error::Tool {
            cmd: format!("git {}", args.join(" ")),
            detail: format!("could not start git: {e}"),
        })?;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let run = |args: &[&str]| {
            let out = Command::new("git")
                .args(args)
                .current_dir(dir.path())
                .output()
                .unwrap();
            assert!(
                out.status.success(),
                "git {args:?}: {}",
                String::from_utf8_lossy(&out.stderr)
            );
        };
        run(&["init", "-q", "-b", "main"]);
        run(&["config", "user.email", "t@example.com"]);
        run(&["config", "user.name", "t"]);
        fs::create_dir_all(dir.path().join("src")).unwrap();
        fs::create_dir_all(dir.path().join("docs")).unwrap();
        fs::write(dir.path().join("src/lib.txt"), "code\n").unwrap();
        fs::write(dir.path().join("docs/a.md"), "docs\n").unwrap();
        fs::write(dir.path().join(".gitignore"), "/scenario/\n").unwrap();
        run(&["add", "-A"]);
        run(&["commit", "-q", "-m", "init"]);
        dir
    }

    #[test]
    fn tree_id_follows_the_code_and_ignores_docs_and_ignored_files() {
        let dir = repo();
        let idx = dir.path().join("run").join("gate-index");
        let base = tree_id(dir.path(), &idx).unwrap();
        assert!(!idx.exists(), "the throwaway index is removed");
        // Docs change, committed or not: same key.
        fs::write(dir.path().join("docs/a.md"), "docs 2\n").unwrap();
        fs::write(dir.path().join("docs/new.md"), "new\n").unwrap();
        assert_eq!(tree_id(dir.path(), &idx).unwrap(), base);
        // Ignored files: same key.
        fs::create_dir_all(dir.path().join("scenario")).unwrap();
        fs::write(dir.path().join("scenario/gate"), "red\n").unwrap();
        assert_eq!(tree_id(dir.path(), &idx).unwrap(), base);
        // An uncommitted code change: a new key; committing it keeps that key.
        fs::write(dir.path().join("src/lib.txt"), "code 2\n").unwrap();
        let changed = tree_id(dir.path(), &idx).unwrap();
        assert_ne!(changed, base);
        let out = Command::new("git")
            .args(["commit", "-q", "-am", "code"])
            .current_dir(dir.path())
            .output()
            .unwrap();
        assert!(out.status.success());
        assert_eq!(tree_id(dir.path(), &idx).unwrap(), changed);
        // An untracked code file: a new key.
        fs::write(dir.path().join("src/other.txt"), "x\n").unwrap();
        assert_ne!(tree_id(dir.path(), &idx).unwrap(), changed);
        // The repo's own index was never touched.
        let out = Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir(dir.path())
            .output()
            .unwrap();
        let status = String::from_utf8_lossy(&out.stdout);
        assert!(status.contains("?? src/other.txt"), "{status}");
        assert!(!status.contains("A  "), "{status}");
    }

    #[test]
    fn cache_round_trips_through_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let mut c = GateCache::load(dir.path()).unwrap();
        assert!(c.is_empty());
        c.put(
            "k ./gate.sh",
            Entry {
                red: false,
                minutes: 0.5,
                at: "t".into(),
                log: "gates/1-1.log".into(),
            },
        )
        .unwrap();
        let again = GateCache::load(dir.path()).unwrap();
        assert_eq!(again.len(), 1);
        assert_eq!(again.get("k ./gate.sh").unwrap().log, "gates/1-1.log");
        assert!(again.get("other").is_none());
    }
}
