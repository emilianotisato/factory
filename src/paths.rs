//! `FACTORY_DATA_DIR` and `FACTORY_CONFIG_DIR` exist so tests never touch the owner's dirs.
use std::env;
use std::ffi::{OsStr, OsString};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::LazyLock;

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

/// A byte that continues a path segment: a home prefix right after one is mid-path, as in `/tmp/home/x/`.
fn in_segment(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-' | b'~')
}

static HOME_PATH: LazyLock<regex::bytes::Regex> = LazyLock::new(|| {
    regex::bytes::Regex::new(r"(?:^|[^A-Za-z0-9._~-])(/(?:home|Users)/[A-Za-z0-9._-]+/)")
        .expect("home path pattern compiles")
});

static ANY_HOME_PATH: LazyLock<regex::bytes::Regex> = LazyLock::new(|| {
    regex::bytes::Regex::new(r"/(?:home|Users)/[A-Za-z0-9._-]+/")
        .expect("home path pattern compiles")
});

/// Byte offset of the first `/home/<name>/` or `/Users/<name>/` that starts a path in `text`.
/// In binary (a NUL byte), a length byte such as `E` can sit right before a path, so any hit counts.
pub fn find_home_path(text: &[u8]) -> Option<usize> {
    if text.contains(&0) {
        return ANY_HOME_PATH.find(text).map(|m| m.start());
    }
    HOME_PATH
        .captures(text)
        .and_then(|c| c.get(1))
        .map(|m| m.start())
}

/// `text` with each path under `home` written as `~/…`; a home of `/` rewrites nothing.
pub fn home_to_tilde(text: &str, home: &Path) -> String {
    let Some(home) = home.to_str().map(|h| h.trim_end_matches('/')) else {
        return text.to_string();
    };
    if home.is_empty() {
        return text.to_string();
    }
    let prefix = format!("{home}/");
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find(&prefix) {
        let starts_path = at == 0 || !in_segment(rest.as_bytes()[at - 1]);
        let take = if starts_path { at } else { at + 1 };
        out.push_str(&rest[..take]);
        if starts_path {
            out.push_str("~/");
            rest = &rest[at + prefix.len()..];
        } else {
            rest = &rest[take..];
        }
    }
    out.push_str(rest);
    out
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_home_path_is_found_only_where_a_path_starts() {
        assert_eq!(find_home_path(b"/home/x/a"), Some(0));
        assert_eq!(find_home_path(b"see /Users/ana.b/notes"), Some(4));
        assert_eq!(find_home_path(b"(`/home/x/`)"), Some(2));
        assert_eq!(find_home_path(b"line\n/home/x-1/"), Some(5));
        assert_eq!(find_home_path(b"/tmp/home/x/ and ~/home/y/"), None);
        assert_eq!(find_home_path(b"/home/x and /home/ and /Users"), None);
        assert_eq!(find_home_path(b"/tmp/home/x/ then /home/y/"), Some(18));
    }

    #[test]
    fn in_binary_a_home_path_after_a_length_byte_is_found() {
        assert_eq!(find_home_path(b"\x00\x01zE/home/x/a"), Some(4));
        assert_eq!(find_home_path(b"zE/home/x/a"), None);
    }

    #[test]
    fn only_the_home_prefix_starting_a_path_becomes_a_tilde() {
        let home = Path::new("/home/x");
        assert_eq!(
            home_to_tilde("log (/home/x/data/runs/1) and /home/x/", home),
            "log (~/data/runs/1) and ~/"
        );
        assert_eq!(
            home_to_tilde("/home/xy/a /home/x", home),
            "/home/xy/a /home/x"
        );
        assert_eq!(home_to_tilde("/tmp/home/x/a", home), "/tmp/home/x/a");
        assert_eq!(
            home_to_tilde("/tmp/home/x/a /home/x/b", home),
            "/tmp/home/x/a ~/b"
        );
        assert_eq!(home_to_tilde("/Users/x/a", Path::new("/Users/x/")), "~/a");
    }

    #[test]
    fn a_home_of_root_rewrites_nothing() {
        assert_eq!(
            home_to_tilde("/home/x/a /tmp/b", Path::new("/")),
            "/home/x/a /tmp/b"
        );
    }
}
