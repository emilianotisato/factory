//! Where state lives (PRD §15 OPEN-12): `~/.config/factory/` and
//! `~/.local/share/factory/`. Tests must never touch the owner's dirs, so
//! `FACTORY_DATA_DIR` and `FACTORY_CONFIG_DIR` win, then the XDG variables,
//! then the defaults under `$HOME`. What the driver writes there (run
//! state, worker streams, settings) is the owner's alone: a data dir this
//! process creates is mode 0700.
use std::env;
use std::ffi::{OsStr, OsString};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::{Error, Result};

/// `$FACTORY_DATA_DIR`, else `$XDG_DATA_HOME/factory`, else
/// `~/.local/share/factory`. A missing one is created 0700 (best effort:
/// the caller reports what fails); an existing one keeps its mode, since
/// `FACTORY_DATA_DIR` may name a directory the owner shares on purpose.
pub fn data_dir() -> PathBuf {
    let d = dir("FACTORY_DATA_DIR", "XDG_DATA_HOME", ".local/share");
    if !d.exists() {
        let _ = private_dir_all(&d);
    }
    d
}

/// `mkdir -p` with mode 0700 for every directory it creates; an existing
/// one keeps its mode.
pub fn private_dir_all(dir: &Path) -> std::io::Result<()> {
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)
}

/// The running binary's absolute path. Linux names a binary replaced
/// under a live process `<path> (deleted)`; the suffix is dropped, so a
/// rebuild mid-run still hands workers the path the new build lives at.
pub fn self_exe() -> std::io::Result<PathBuf> {
    let exe = env::current_exe()?;
    let bytes = exe.as_os_str().as_bytes();
    Ok(match bytes.strip_suffix(b" (deleted)") {
        Some(live) => PathBuf::from(OsStr::from_bytes(live)),
        None => exe,
    })
}

/// `s` as one shell word: as is when it holds nothing a shell reads
/// specially, else single-quoted.
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

/// How a worker's shell runs this binary: its absolute path (one shell
/// word), which works whether or not `factory` is on `PATH`. A process
/// whose binary is not named `factory` (a test calling the library in
/// process) gets the plain name.
pub fn factory_command() -> String {
    match self_exe() {
        Ok(exe) if exe.file_name() == Some(OsStr::new("factory")) => {
            shell_word(&exe.to_string_lossy())
        }
        _ => "factory".to_string(),
    }
}

/// `$FACTORY_CONFIG_DIR`, else `$XDG_CONFIG_HOME/factory`, else `~/.config/factory`.
pub fn config_dir() -> PathBuf {
    dir("FACTORY_CONFIG_DIR", "XDG_CONFIG_HOME", ".config")
}

/// Always absolute, resolved against this process's cwd: a relative
/// `FACTORY_DATA_DIR` must mean the same directory to a worker or a `git`
/// that runs in the repo (Ln6).
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
