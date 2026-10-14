use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use super::preflight::git;
use crate::error::io;
use crate::{Error, Result};

/// v2's marker, kept so a repo with the v2 hook reads as installed.
pub const MARKER: &str = "# feature-docs hook";

/// Names `factory`, not this binary's path: the hook outlives a rebuild, and a missing `factory` exits 0.
pub fn body() -> String {
    [
        MARKER,
        "# docs/archive/** is frozen, and docs/ changes must pass `factory docs check`.",
        "root=$(git rev-parse --show-toplevel 2>/dev/null) || exit 0",
        "if git diff --cached --name-only --diff-filter=M | grep -q \"^docs/archive/\"; then",
        "  echo \"docs: docs/archive/** is frozen — move folders, never edit files inside.\" >&2",
        "  exit 1",
        "fi",
        "if git diff --cached --name-only | grep -q \"^docs/\" && [ -f \"$root/docs/index.md\" ]; then",
        "  command -v factory >/dev/null 2>&1 || exit 0",
        "  (cd \"$root\" && factory docs check) || exit 1",
        "fi",
    ]
    .join("\n")
}

#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Printed { hooks_path: String },
    Installed(String),
    Already,
}

impl Outcome {
    pub fn text(&self) -> String {
        match self {
            Outcome::Printed { hooks_path } => format!(
                "git uses core.hooksPath = {hooks_path}, so .git/hooks is ignored.\n\n\
                 Append this to {}/pre-commit — it no-ops in repos with no staged docs/ changes:\n\n\
                 {}\n\n\
                 Or: `factory docs hook --here` points THIS repo at .git/hooks instead — which disables your global hooks for this repo.",
                hooks_path.trim_end_matches('/'),
                body()
            ),
            Outcome::Installed(path) => format!("installed {path}"),
            Outcome::Already => "hook already installed".to_string(),
        }
    }
}

pub fn install(root: &Path, here: bool) -> Result<Outcome> {
    if git(root, &["rev-parse", "--is-inside-work-tree"]).is_none() {
        return Err(Error::Msg("not a git repository".into()));
    }
    let configured = git(root, &["config", "--get", "core.hooksPath"])
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    // Installing under a configured hooksPath would silently do nothing.
    if let Some(hooks_path) = configured
        && !here
    {
        return Ok(Outcome::Printed { hooks_path });
    }
    if here && git(root, &["config", "core.hooksPath", ".git/hooks"]).is_none() {
        return Err(Error::Msg("could not set core.hooksPath".into()));
    }
    let common = git(root, &["rev-parse", "--git-common-dir"])
        .map(|s| s.trim().to_string())
        .ok_or_else(|| Error::Msg("git rev-parse --git-common-dir failed".into()))?;
    // Relative to the repo root in a plain checkout, absolute in a worktree.
    let hooks = root.join(common).join("hooks");
    fs::create_dir_all(&hooks).map_err(io(format!("mkdir {}", hooks.display())))?;
    let file = hooks.join("pre-commit");
    if file.exists() {
        let text = fs::read_to_string(&file).map_err(io(format!("read {}", file.display())))?;
        if text.contains(MARKER) {
            return Ok(Outcome::Already);
        }
        return Err(Error::Msg(format!(
            "{} exists — append this by hand:\n\n{}",
            file.display(),
            body()
        )));
    }
    fs::write(&file, format!("#!/bin/sh\n{}\n", body()))
        .map_err(io(format!("write {}", file.display())))?;
    fs::set_permissions(&file, fs::Permissions::from_mode(0o755))
        .map_err(io(format!("chmod {}", file.display())))?;
    let shown = file.strip_prefix(root).unwrap_or(&file);
    Ok(Outcome::Installed(shown.display().to_string()))
}
