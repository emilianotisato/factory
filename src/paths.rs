//! `FACTORY_DATA_DIR` and `FACTORY_CONFIG_DIR` exist so tests never touch the owner's dirs.
use std::env;
use std::ffi::{OsStr, OsString};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::{Error, Result};

/// An existing dir keeps its mode: `FACTORY_DATA_DIR` may name one the owner shares on purpose.
pub fn data_dir() -> PathBuf {
    let d = dir("FACTORY_DATA_DIR", "XDG_DATA_HOME", ".local/share");
    if !d.exists() {
        let _ = private_dir_all(&d);
    }
    d
}

pub fn private_dir_all(dir: &Path) -> std::io::Result<()> {
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)
}

/// Linux names a binary replaced under a live process `<path> (deleted)`; a rebuild mid-run keeps the path.
pub fn self_exe() -> std::io::Result<PathBuf> {
    let exe = env::current_exe()?;
    let bytes = exe.as_os_str().as_bytes();
    Ok(match bytes.strip_suffix(b" (deleted)") {
        Some(live) => PathBuf::from(OsStr::from_bytes(live)),
        None => exe,
    })
}

pub fn shell_word(s: &str) -> String {
    let plain = s
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "/._-+:@%,".contains(c));
    if plain && !s.is_empty() {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
}

/// Absolute so workers need no `PATH`; a binary not named `factory` (an in-process test) gets the plain name.
pub fn factory_command() -> String {
    match self_exe() {
        Ok(exe) if exe.file_name() == Some(OsStr::new("factory")) => {
            shell_word(&exe.to_string_lossy())
        }
        _ => "factory".to_string(),
    }
}

pub fn config_dir() -> PathBuf {
    dir("FACTORY_CONFIG_DIR", "XDG_CONFIG_HOME", ".config")
}

/// Absolute: a relative `FACTORY_DATA_DIR` must mean the same directory to a worker or `git` in the repo.
fn dir(ours: &str, xdg: &str, fallback: &str) -> PathBuf {
    let d = if let Some(p) = non_empty(ours) {
        PathBuf::from(p)
    } else if let Some(x) = non_empty(xdg) {
        PathBuf::from(x).join("factory")
    } else {
        home().join(fallback).join("factory")
    };
    std::path::absolute(&d).unwrap_or(d)
}

fn non_empty(var: &str) -> Option<OsString> {
    env::var_os(var).filter(|v| !v.is_empty())
}

pub fn home() -> PathBuf {
    non_empty("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"))
}

pub fn claude_config_dir() -> PathBuf {
    non_empty("CLAUDE_CONFIG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".claude"))
}

/// One per repo, shared by its linked worktrees.
pub fn git_common_dir(from: &Path) -> Option<PathBuf> {
    let out = Command::new("git")
        .args(["rev-parse", "--path-format=absolute", "--git-common-dir"])
        .current_dir(from)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let dir = String::from_utf8_lossy(&out.stdout).trim().to_string();
    PathBuf::from(dir).canonicalize().ok()
}

pub fn repo_root(from: &Path) -> Result<PathBuf> {
    let out = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(from)
        .output()
        .map_err(|e| Error::Tool {
            cmd: "git rev-parse --show-toplevel".into(),
            detail: format!("could not start git: {e}"),
        })?;
    if !out.status.success() {
        return Err(Error::Msg(format!(
            "{} is not inside a git repository",
            from.display()
        )));
    }
    let root = String::from_utf8_lossy(&out.stdout).trim().to_string();
    PathBuf::from(&root)
        .canonicalize()
        .map_err(crate::error::io(format!("canonicalize {root}")))
}
