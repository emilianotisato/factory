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

/// The bundle lock (FR-5), relative to the source repo's root.
pub const LOCK: &str = "skills/bundle.lock";

/// `skills/bundle.lock`: the crate version the bundle last shipped under and
/// its [`bundle_hash`]. A skill edit cannot ship under an old version.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Lock {
    pub version: String,
    pub sha256: String,
}

impl Lock {
    pub fn parse(text: &str) -> std::result::Result<Lock, String> {
        toml::from_str(text).map_err(|e| format!("unreadable {LOCK}: {e}"))
    }

    pub fn text(&self) -> String {
        format!(
            "# Written by FACTORY_BUNDLE_WRITE=1 cargo test. Do not edit by hand.\n{}",
            toml::to_string(self).expect("two strings serialize")
        )
    }
}

/// `major.minor.patch` as three integers: `0.10.0` is above `0.9.0`, which
/// a string comparison gets wrong.
pub fn version_parts(v: &str) -> Option<(u64, u64, u64)> {
    let mut parts = v.split('.').map(|p| p.parse::<u64>().ok());
    let triple = (parts.next()??, parts.next()??, parts.next()??);
    parts.next().is_none().then_some(triple)
}

/// `Ok` only when the lock matches both the crate version and the bundle.
/// The error names the lock, each side that differs, and the two lines that
/// fix it.
pub fn check_lock(lock: &Lock, version: &str, hash: &str) -> std::result::Result<(), String> {
    let mut why = Vec::new();
    if lock.sha256 != hash {
        why.push(format!("  bundle changed: {} → {hash}", lock.sha256));
    }
    if lock.version != version {
        why.push(format!(
            "  version: lock {}, Cargo.toml {version}",
            lock.version
        ));
    }
    if why.is_empty() {
        return Ok(());
    }
    Err(format!(
        "{LOCK} is stale:\n{}\nfix:\n  bump version in Cargo.toml\n  FACTORY_BUNDLE_WRITE=1 cargo test",
        why.join("\n")
    ))
}

/// The lock `FACTORY_BUNDLE_WRITE=1` writes. Refuses when the bundle changed
/// and `version` is not above the old lock's: a skill edit needs a bump. A
/// bump over an unchanged bundle (a binary-only release) re-locks freely.
pub fn relock(old: Option<&Lock>, version: &str, hash: &str) -> std::result::Result<Lock, String> {
    let new = Lock {
        version: version.to_string(),
        sha256: hash.to_string(),
    };
    let Some(old) = old.filter(|o| o.sha256 != hash) else {
        return Ok(new);
    };
    let parts = |v: &str| version_parts(v).ok_or(format!("version {v} is not three integers"));
    if parts(version)? > parts(&old.version)? {
        Ok(new)
    } else {
        Err(format!(
            "skills changed since {0}: bump Cargo.toml past {0} first",
            old.version
        ))
    }
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

/// The manifest's version against the binary's.
#[derive(Debug, Clone, PartialEq)]
pub enum Version {
    Current(String),
    Behind { repo: String, binary: String },
    Ahead { repo: String, binary: String },
    Differs(String),
}

/// What a repo's skills are against this binary's bundle (FR-3).
#[derive(Debug, Clone, PartialEq)]
pub enum Verdict {
    /// `.claude/skills/prd` resolves inside `<root>/skills/` (FR-2).
    SourceCheckout,
    /// No manifest.
    None,
    /// A manifest that does not parse, or whose version is not three integers.
    Unreadable(String),
    Checked {
        version: Version,
        /// Manifest files whose bytes differ, relative to `.claude/skills/`.
        hand_edited: Vec<String>,
        /// Manifest files absent on disk, relative to `.claude/skills/`.
        missing: Vec<String>,
    },
}

/// The manifest at `root`, `None` when there is none.
pub fn read_manifest(root: &Path) -> std::result::Result<Option<Manifest>, String> {
    let text = match fs::read_to_string(root.join(MANIFEST)) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    toml::from_str(&text)
        .map(Some)
        .map_err(|e| e.message().trim().to_string())
}

/// Compares the repo's skills with the binary's. Reads only.
pub fn verdict(root: &Path) -> Verdict {
    verdict_for(root, env!("CARGO_PKG_VERSION"))
}

/// [`verdict`] against a given binary version.
fn verdict_for(root: &Path, binary: &str) -> Verdict {
    if source_checkout(root) {
        return Verdict::SourceCheckout;
    }
    let m = match read_manifest(root) {
        Ok(Some(m)) => m,
        Ok(None) => return Verdict::None,
        Err(e) => return Verdict::Unreadable(e),
    };
    let binary = binary.to_string();
    let (Some(repo_v), Some(bin_v)) = (version_parts(&m.version), version_parts(&binary)) else {
        return Verdict::Unreadable(format!("version \"{}\" is not three integers", m.version));
    };
    let version = match repo_v.cmp(&bin_v) {
        std::cmp::Ordering::Less => Version::Behind {
            repo: m.version,
            binary,
        },
        std::cmp::Ordering::Greater => Version::Ahead {
            repo: m.version,
            binary,
        },
        std::cmp::Ordering::Equal if m.bundle != bundle_hash() => Version::Differs(binary),
        std::cmp::Ordering::Equal => Version::Current(binary),
    };
    let skills = root.join(SKILLS_DIR);
    let (mut hand_edited, mut missing) = (Vec::new(), Vec::new());
    for (rel, hash) in &m.files {
        match fs::read(skills.join(rel)) {
            Ok(bytes) if sha256_hex(&bytes) == *hash => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => missing.push(rel.clone()),
            _ => hand_edited.push(rel.clone()),
        }
    }
    Verdict::Checked {
        version,
        hand_edited,
        missing,
    }
}

impl Verdict {
    /// Doctor's `skills` lines: the version line unless current, then the
    /// hand-edit and missing lines; `current (<v>)` only when none applies.
    /// `ahead` points at `factory upgrade` or, for a binary built from
    /// source, the line that rebuilds it (FR-8).
    pub fn lines(&self) -> Vec<String> {
        self.lines_with(crate::cli::upgrade::ahead_hint)
    }

    /// [`Verdict::lines`] with the `ahead` hint given; read only on `ahead`,
    /// since it looks for the install receipt.
    fn lines_with(&self, ahead_hint: impl FnOnce() -> String) -> Vec<String> {
        let (version, hand_edited, missing) = match self {
            Verdict::SourceCheckout => return vec!["source checkout".into()],
            Verdict::None => return vec!["none → factory init".into()],
            Verdict::Unreadable(e) => return vec![format!("unreadable {MANIFEST}: {e}")],
            Verdict::Checked {
                version,
                hand_edited,
                missing,
            } => (version, hand_edited, missing),
        };
        let mut out = Vec::new();
        match version {
            Version::Current(_) => {}
            Version::Behind { repo, binary } => out.push(format!(
                "behind (repo {repo}, binary {binary}) → factory skills update"
            )),
            Version::Ahead { repo, binary } => out.push(format!(
                "ahead (repo {repo}, binary {binary}) → {}",
                ahead_hint()
            )),
            Version::Differs(v) => out.push(format!(
                "differs ({v} both, bundle differs: a dev build) → factory skills update"
            )),
        }
        if !hand_edited.is_empty() {
            out.push(format!("hand-edited: {}", hand_edited.join(", ")));
        }
        if !missing.is_empty() {
            out.push(format!("missing: {}", missing.join(", ")));
        }
        if let (true, Version::Current(v)) = (out.is_empty(), version) {
            out.push(format!("current ({v})"));
        }
        out
    }

    /// Preflight's `ok` side: `current` and `source checkout`. Every other
    /// verdict is a judge line.
    pub fn is_ok(&self) -> bool {
        match self {
            Verdict::SourceCheckout => true,
            Verdict::Checked {
                version: Version::Current(_),
                hand_edited,
                missing,
            } => hand_edited.is_empty() && missing.is_empty(),
            _ => false,
        }
    }
}

/// A manifest key that stays under the dir it is joined to: plain names
/// only, no `..`, no root. A hand-written manifest must not reach out of it.
fn plain(rel: &str) -> bool {
    !rel.is_empty()
        && Path::new(rel)
            .components()
            .all(|c| matches!(c, std::path::Component::Normal(_)))
}

/// After a [`render`]: removes what `old` lists and `new` lacks, files under
/// `.claude/skills/` and links under `.agents/skills/`, then the directories
/// under `.claude/skills/` the removals left empty. Returns the repo-relative
/// paths removed.
pub fn remove_dropped(root: &Path, old: &Manifest, new: &Manifest) -> Result<Vec<String>> {
    let skills = root.join(SKILLS_DIR);
    let mut removed = Vec::new();
    for rel in old.files.keys().filter(|r| !new.files.contains_key(*r)) {
        let path = skills.join(rel);
        if !plain(rel) || fs::symlink_metadata(&path).is_err() {
            continue;
        }
        clear(&path)?;
        removed.push(format!("{SKILLS_DIR}/{rel}"));
        let mut dir = path.parent();
        while let Some(d) = dir.filter(|d| *d != skills && d.starts_with(&skills)) {
            if fs::remove_dir(d).is_err() {
                break;
            }
            dir = d.parent();
        }
    }
    let prefix = format!("{LINKS_DIR}/");
    for rel in old.links.keys().filter(|r| !new.links.contains_key(*r)) {
        let Some(name) = rel.strip_prefix(&prefix) else {
            continue;
        };
        let path = root.join(LINKS_DIR).join(name);
        if !plain(name) || fs::symlink_metadata(&path).is_err() {
            continue;
        }
        clear(&path)?;
        removed.push(rel.clone());
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;

    const V: &str = env!("CARGO_PKG_VERSION");

    fn rendered() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        render(dir.path()).unwrap();
        dir
    }

    fn edit_manifest(root: &Path, f: impl FnOnce(&mut Manifest)) {
        let mut m = read_manifest(root).unwrap().unwrap();
        f(&mut m);
        fs::write(root.join(MANIFEST), toml::to_string(&m).unwrap()).unwrap();
    }

    fn append(root: &Path, rel: &str) {
        let path = root.join(SKILLS_DIR).join(rel);
        let text = fs::read_to_string(&path).unwrap();
        fs::write(&path, format!("{text}x")).unwrap();
    }

    #[test]
    fn a_fresh_render_is_current() {
        let dir = rendered();
        let v = verdict(dir.path());
        assert_eq!(v.lines(), [format!("current ({V})")]);
        assert!(v.is_ok());
    }

    #[test]
    fn no_manifest_is_none() {
        let dir = rendered();
        fs::remove_file(dir.path().join(MANIFEST)).unwrap();
        let v = verdict(dir.path());
        assert_eq!(v.lines(), ["none → factory init"]);
        assert!(!v.is_ok());
    }

    #[test]
    fn behind_with_a_hand_edit_prints_both_version_first() {
        let dir = rendered();
        edit_manifest(dir.path(), |m| m.version = "0.0.0".into());
        append(dir.path(), "polish/SKILL.md");
        assert_eq!(
            verdict(dir.path()).lines(),
            [
                format!("behind (repo 0.0.0, binary {V}) → factory skills update"),
                "hand-edited: polish/SKILL.md".to_string(),
            ]
        );
    }

    #[test]
    fn a_newer_manifest_is_ahead() {
        let dir = rendered();
        edit_manifest(dir.path(), |m| m.version = "999.0.0".into());
        assert_eq!(
            verdict(dir.path()).lines_with(|| "factory upgrade".into()),
            [format!(
                "ahead (repo 999.0.0, binary {V}) → factory upgrade"
            )]
        );
    }

    #[test]
    fn versions_compare_as_integers_not_text() {
        let dir = rendered();
        edit_manifest(dir.path(), |m| m.version = "0.10.0".into());
        assert_eq!(
            verdict_for(dir.path(), "0.9.0").lines_with(|| "factory upgrade".into()),
            ["ahead (repo 0.10.0, binary 0.9.0) → factory upgrade"]
        );
    }

    #[test]
    fn same_version_other_bundle_differs() {
        let dir = rendered();
        edit_manifest(dir.path(), |m| m.bundle = "0".repeat(64));
        let v = verdict(dir.path());
        assert_eq!(
            v.lines(),
            [format!(
                "differs ({V} both, bundle differs: a dev build) → factory skills update"
            )]
        );
        assert!(!v.is_ok());
    }

    #[test]
    fn an_edited_file_alone_prints_hand_edited_alone() {
        let dir = rendered();
        append(dir.path(), "polish/SKILL.md");
        append(dir.path(), "gate/SKILL.md");
        let v = verdict(dir.path());
        assert_eq!(v.lines(), ["hand-edited: gate/SKILL.md, polish/SKILL.md"]);
        assert!(!v.is_ok());
    }

    #[test]
    fn a_deleted_file_is_missing() {
        let dir = rendered();
        fs::remove_file(dir.path().join(SKILLS_DIR).join("ux/SKILL.md")).unwrap();
        assert_eq!(verdict(dir.path()).lines(), ["missing: ux/SKILL.md"]);
    }

    #[test]
    fn a_link_into_skills_is_a_source_checkout() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("skills/prd")).unwrap();
        fs::create_dir_all(dir.path().join(SKILLS_DIR)).unwrap();
        symlink("../../skills/prd", dir.path().join(SKILLS_DIR).join("prd")).unwrap();
        let v = verdict(dir.path());
        assert_eq!(v.lines(), ["source checkout"]);
        assert!(v.is_ok());
    }

    #[test]
    fn a_bad_manifest_is_unreadable_not_guessed() {
        let dir = rendered();
        edit_manifest(dir.path(), |m| m.version = "0.2".into());
        assert_eq!(
            verdict(dir.path()).lines(),
            [format!(
                "unreadable {MANIFEST}: version \"0.2\" is not three integers"
            )]
        );
        fs::write(dir.path().join(MANIFEST), "version = 3\n").unwrap();
        let lines = verdict(dir.path()).lines();
        assert!(
            lines.len() == 1 && lines[0].starts_with(&format!("unreadable {MANIFEST}: ")),
            "{lines:?}"
        );
    }

    #[test]
    fn remove_dropped_takes_files_links_and_empty_dirs_only() {
        let dir = rendered();
        let root = dir.path();
        let new = read_manifest(root).unwrap().unwrap();
        let mut old = new.clone();
        fs::create_dir_all(root.join(SKILLS_DIR).join("old/references")).unwrap();
        fs::write(root.join(SKILLS_DIR).join("old/references/a.md"), "a").unwrap();
        old.files
            .insert("old/references/a.md".into(), file_hash("a"));
        old.files.insert("../outside.md".into(), file_hash("o"));
        fs::write(root.join("outside.md"), "o").unwrap();
        symlink(link_target("old"), root.join(LINKS_DIR).join("old")).unwrap();
        old.links
            .insert(format!("{LINKS_DIR}/old"), link_target("old"));
        let removed = remove_dropped(root, &old, &new).unwrap();
        assert_eq!(
            removed,
            [
                format!("{SKILLS_DIR}/old/references/a.md"),
                format!("{LINKS_DIR}/old")
            ]
        );
        assert!(fs::symlink_metadata(root.join(SKILLS_DIR).join("old")).is_err());
        assert!(fs::symlink_metadata(root.join(LINKS_DIR).join("old")).is_err());
        assert!(root.join("outside.md").is_file());
        assert!(root.join(SKILLS_DIR).join("prd/SKILL.md").is_file());
    }

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

    fn lock(version: &str, sha256: &str) -> Lock {
        Lock {
            version: version.to_string(),
            sha256: sha256.to_string(),
        }
    }

    #[test]
    fn a_lock_matching_both_sides_passes() {
        assert_eq!(check_lock(&lock("0.2.0", "aa"), "0.2.0", "aa"), Ok(()));
    }

    #[test]
    fn a_skill_edit_without_a_bump_fails_and_cannot_relock() {
        let old = lock("0.2.0", "aa");
        let err = check_lock(&old, "0.2.0", "bb").unwrap_err();
        assert!(err.starts_with("skills/bundle.lock is stale:"), "{err}");
        assert!(err.contains("bundle changed: aa → bb"), "{err}");
        assert!(!err.contains("version: lock"), "{err}");
        assert!(
            err.ends_with("bump version in Cargo.toml\n  FACTORY_BUNDLE_WRITE=1 cargo test"),
            "{err}"
        );
        assert_eq!(
            relock(Some(&old), "0.2.0", "bb"),
            Err("skills changed since 0.2.0: bump Cargo.toml past 0.2.0 first".to_string())
        );
        assert!(relock(Some(&old), "0.1.9", "bb").is_err());
    }

    #[test]
    fn a_skill_edit_with_a_bump_relocks() {
        let old = lock("0.2.0", "aa");
        assert_eq!(relock(Some(&old), "0.2.1", "bb"), Ok(lock("0.2.1", "bb")));
        assert_eq!(relock(None, "0.2.0", "aa"), Ok(lock("0.2.0", "aa")));
    }

    #[test]
    fn a_binary_only_bump_fails_the_check_and_relocks_freely() {
        let old = lock("0.2.0", "aa");
        let err = check_lock(&old, "0.2.1", "aa").unwrap_err();
        assert!(
            err.contains("version: lock 0.2.0, Cargo.toml 0.2.1"),
            "{err}"
        );
        assert!(!err.contains("bundle changed"), "{err}");
        assert_eq!(relock(Some(&old), "0.2.1", "aa"), Ok(lock("0.2.1", "aa")));
    }

    #[test]
    fn versions_compare_as_integers() {
        assert_eq!(version_parts("0.10.0"), Some((0, 10, 0)));
        assert!(version_parts("0.10.0") > version_parts("0.9.0"));
        assert!(relock(Some(&lock("0.9.0", "aa")), "0.10.0", "bb").is_ok());
        for bad in ["0.2", "0.2.0.1", "0.2.x", "", "0.2.0-rc1"] {
            assert_eq!(version_parts(bad), None, "{bad}");
        }
    }

    #[test]
    fn a_lock_round_trips_through_its_text() {
        let l = lock("0.2.0", "aa");
        assert_eq!(Lock::parse(&l.text()), Ok(l));
    }

    #[test]
    fn every_name_has_its_skill_file() {
        for name in NAMES {
            let path = format!("{name}/SKILL.md");
            assert!(FILES.iter().any(|(p, _)| *p == path), "{path} missing");
        }
    }
}
