//! Where state lives (PRD §15 OPEN-12): `~/.config/factory/` and
//! `~/.local/share/factory/`. Tests must never touch the owner's dirs, so
//! `FACTORY_DATA_DIR` and `FACTORY_CONFIG_DIR` win, then the XDG variables,
//! then the defaults under `$HOME`.
use std::env;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::{Error, Result};

/// `$FACTORY_DATA_DIR`, else `$XDG_DATA_HOME/factory`, else `~/.local/share/factory`.
pub fn data_dir() -> PathBuf {
    dir("FACTORY_DATA_DIR", "XDG_DATA_HOME", ".local/share")
}

/// `$FACTORY_CONFIG_DIR`, else `$XDG_CONFIG_HOME/factory`, else `~/.config/factory`.
pub fn config_dir() -> PathBuf {
    dir("FACTORY_CONFIG_DIR", "XDG_CONFIG_HOME", ".config")
}

fn dir(ours: &str, xdg: &str, fallback: &str) -> PathBuf {
    if let Some(p) = non_empty(ours) {
        return PathBuf::from(p);
    }
    if let Some(x) = non_empty(xdg) {
        return PathBuf::from(x).join("factory");
    }
    home().join(fallback).join("factory")
}

fn non_empty(var: &str) -> Option<OsString> {
    env::var_os(var).filter(|v| !v.is_empty())
}

/// `$HOME`, or `/` when unset.
pub fn home() -> PathBuf {
    non_empty("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"))
}

/// The git toplevel of `from`, canonical.
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
