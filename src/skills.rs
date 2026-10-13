//! The skill family, embedded (PRD-004 §3 FR-1). `skills/` in this repo is
//! the source; the binary carries every file under the eight skill folders
//! and `skills/_shared/` as one bundle, listed by hand like the assets in
//! [`crate::pack`]. `skills/README.md` and `skills/bundle.lock` describe the
//! source folder, so neither is bundled. `tests/skills_bundle.rs` walks
//! `skills/` and fails on a file this list misses.

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::symlink;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::io;
use crate::metrics::sha256_hex;
use crate::{Error, Result};

/// Where `init` writes the bundle, relative to the repo root.
pub const SKILLS_DIR: &str = ".claude/skills";
/// Where the Codex links go, one per skill, relative to the repo root.
pub const LINKS_DIR: &str = ".agents/skills";
/// The manifest (FR-3): a loose file at the skills root is no skill to any agent.
pub const MANIFEST: &str = ".claude/skills/factory.toml";

/// The eight skills, the folders that get an `.agents/skills` link.
/// `_shared` is not a skill: it has no `SKILL.md`.
pub const NAMES: [&str; 8] = [
    "prd", "slice", "deliver", "polish", "gate", "harness", "prune", "ux",
];

/// `(path relative to skills/, content)`, sorted by path, `/` separators.
pub const FILES: &[(&str, &str)] = &[
    (
        "_shared/models.md",
        include_str!("../skills/_shared/models.md"),
    ),
    (
        "deliver/SKILL.md",
        include_str!("../skills/deliver/SKILL.md"),
    ),
    ("gate/SKILL.md", include_str!("../skills/gate/SKILL.md")),
    (
        "gate/references/stacks.md",
        include_str!("../skills/gate/references/stacks.md"),
    ),
    (
        "gate/references/tiers.md",
        include_str!("../skills/gate/references/tiers.md"),
    ),
    (
        "harness/SKILL.md",
        include_str!("../skills/harness/SKILL.md"),
    ),
    (
        "harness/references/drivers.md",
        include_str!("../skills/harness/references/drivers.md"),
    ),
    (
        "harness/references/feature-map.md",
        include_str!("../skills/harness/references/feature-map.md"),
    ),
    (
        "harness/references/maintain.md",
        include_str!("../skills/harness/references/maintain.md"),
    ),
    (
        "harness/references/skill-template.md",
        include_str!("../skills/harness/references/skill-template.md"),
    ),
    ("polish/SKILL.md", include_str!("../skills/polish/SKILL.md")),
    ("prd/SKILL.md", include_str!("../skills/prd/SKILL.md")),
    ("prune/SKILL.md", include_str!("../skills/prune/SKILL.md")),
    (
        "prune/references/adapters.md",
        include_str!("../skills/prune/references/adapters.md"),
    ),
    (
        "prune/references/rules.md",
        include_str!("../skills/prune/references/rules.md"),
    ),
    ("slice/SKILL.md", include_str!("../skills/slice/SKILL.md")),
    ("ux/SKILL.md", include_str!("../skills/ux/SKILL.md")),
    (
        "ux/references/inspection.md",
        include_str!("../skills/ux/references/inspection.md"),
    ),
    (
        "ux/references/rubric.md",
        include_str!("../skills/ux/references/rubric.md"),
    ),
];

/// sha256 of one file's bytes, lowercase hex.
pub fn file_hash(content: &str) -> String {
    sha256_hex(content.as_bytes())
}

/// The bundle hash: sha256, lowercase hex, over the entries sorted by path,
/// each fed as `path`, `\0`, `content`, `\0`. The input order does not
/// matter. A manifest's `bundle` and `skills/bundle.lock` record this value,
/// so changing the definition invalidates both.
pub fn hash_of(files: &[(&str, &str)]) -> String {
    let mut sorted: Vec<&(&str, &str)> = files.iter().collect();
    sorted.sort_by_key(|(path, _)| *path);
    let mut bytes = Vec::new();
    for (path, content) in sorted {
        bytes.extend_from_slice(path.as_bytes());
        bytes.push(0);
        bytes.extend_from_slice(content.as_bytes());
        bytes.push(0);
    }
    sha256_hex(&bytes)
}

/// [`hash_of`] the embedded [`FILES`].
pub fn bundle_hash() -> String {
    hash_of(FILES)
}

/// `.claude/skills/factory.toml`. `files` keys are relative to
/// `.claude/skills/`; `links` keys are repo-relative, valued with the target.
/// BTreeMaps keep the file byte-stable across renders.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    pub version: String,
    pub bundle: String,
    #[serde(default)]
    pub files: BTreeMap<String, String>,
    #[serde(default)]
    pub links: BTreeMap<String, String>,
}

/// What [`render`] wrote, for `init`'s output.
#[derive(Debug, Default)]
pub struct Rendered {
    pub files: Vec<String>,
    pub links: Vec<String>,
}

/// `../../.claude/skills/<name>`, the target of `.agents/skills/<name>`.
pub fn link_target(name: &str) -> String {
    format!("../../{SKILLS_DIR}/{name}")
}

/// Removes whatever sits at `path` without following a symlink: a link or a
/// file is unlinked, a real directory removed whole.
fn clear(path: &Path) -> Result<()> {
    let Ok(meta) = fs::symlink_metadata(path) else {
        return Ok(());
    };
    let what = format!("remove {}", path.display());
    if meta.is_dir() {
        fs::remove_dir_all(path).map_err(io(what))
    } else {
        fs::remove_file(path).map_err(io(what))
    }
}

/// A real directory at `path`. A symlink there is removed first, so the
/// bundle never writes through it into the link's target.
fn real_dir(path: &Path) -> Result<()> {
    if fs::symlink_metadata(path).is_ok_and(|m| !m.is_dir()) {
        clear(path)?;
    }
    fs::create_dir_all(path).map_err(io(format!("mkdir {}", path.display())))
}

/// `fs::write` that never follows a symlink at `path`.
fn write_real(path: &Path, text: &str) -> Result<()> {
    if fs::symlink_metadata(path).is_ok_and(|m| !m.is_file()) {
        clear(path)?;
    }
    fs::write(path, text).map_err(io(format!("write {}", path.display())))
}

/// Writes every [`FILES`] entry to `.claude/skills/`, byte-identical and over
/// whatever is there, then the eight `.agents/skills` links, then the
/// manifest.
pub fn render(root: &Path) -> Result<Rendered> {
    let skills = root.join(SKILLS_DIR);
    let mut done = Rendered::default();
    let mut files = BTreeMap::new();
    for (rel, content) in FILES {
        let parts: Vec<&str> = rel.split('/').collect();
        let mut dir = skills.clone();
        for part in &parts[..parts.len() - 1] {
            dir.push(part);
            real_dir(&dir)?;
        }
        write_real(&skills.join(rel), content)?;
        files.insert((*rel).to_string(), file_hash(content));
        done.files.push(format!("{SKILLS_DIR}/{rel}"));
    }
    let agents = root.join(LINKS_DIR);
    fs::create_dir_all(&agents).map_err(io(format!("mkdir {}", agents.display())))?;
    let mut links = BTreeMap::new();
    for name in NAMES {
        let path = agents.join(name);
        clear(&path)?;
        let target = link_target(name);
        symlink(&target, &path).map_err(io(format!("link {}", path.display())))?;
        let rel = format!("{LINKS_DIR}/{name}");
        links.insert(rel.clone(), target);
        done.links.push(rel);
    }
    let manifest = Manifest {
        version: env!("CARGO_PKG_VERSION").to_string(),
        bundle: bundle_hash(),
        files,
        links,
    };
    let text =
        toml::to_string(&manifest).map_err(|e| Error::Msg(format!("serialize {MANIFEST}: {e}")))?;
    write_real(&root.join(MANIFEST), &text)?;
    Ok(done)
}

/// This repo's own layout: `.claude/skills/prd` resolves inside
/// `<root>/skills/`, so a session reads the live source and `init` leaves the
/// skills alone.
pub fn source_checkout(root: &Path) -> bool {
    let (Ok(prd), Ok(src)) = (
        fs::canonicalize(root.join(SKILLS_DIR).join("prd")),
        fs::canonicalize(root.join("skills")),
    ) else {
        return false;
    };
    prd.starts_with(src)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_changed_byte_changes_the_hash() {
        let mut files = FILES.to_vec();
        let changed = format!("{}x", files[3].1);
        files[3].1 = &changed;
        assert_ne!(hash_of(&files), bundle_hash());
    }

    #[test]
    fn input_order_does_not_change_the_hash() {
        let mut files = FILES.to_vec();
        files.reverse();
        assert_eq!(hash_of(&files), bundle_hash());
    }

    #[test]
    fn a_path_and_content_cannot_trade_bytes() {
        assert_ne!(hash_of(&[("a", "bc")]), hash_of(&[("ab", "c")]));
    }

    #[test]
    fn files_are_sorted_without_duplicates() {
        for pair in FILES.windows(2) {
            assert!(pair[0].0 < pair[1].0, "{} then {}", pair[0].0, pair[1].0);
        }
    }

    #[test]
    fn every_name_has_its_skill_file() {
        for name in NAMES {
            let path = format!("{name}/SKILL.md");
            assert!(FILES.iter().any(|(p, _)| *p == path), "{path} missing");
        }
    }
}
