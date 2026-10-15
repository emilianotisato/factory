//! Listed by hand; `tests/skills_bundle.rs` fails on a file under `skills/` this list misses (README and bundle.lock excepted).

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::symlink;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::io;
use crate::metrics::sha256_hex;
use crate::{Error, Result};

pub const SKILLS_DIR: &str = ".claude/skills";
/// Codex reads skills here.
pub const LINKS_DIR: &str = ".agents/skills";
/// A loose file at the skills root is no skill to any agent.
pub const MANIFEST: &str = ".claude/skills/factory.toml";

/// `_shared` is not one: it has no `SKILL.md`.
pub const NAMES: [&str; 10] = [
    "prd", "slice", "deliver", "polish", "gate", "harness", "harvest", "lanes", "prune", "ux",
];

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
    (
        "harvest/SKILL.md",
        include_str!("../skills/harvest/SKILL.md"),
    ),
    ("lanes/SKILL.md", include_str!("../skills/lanes/SKILL.md")),
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

pub fn file_hash(content: &str) -> String {
    sha256_hex(content.as_bytes())
}

/// Recorded in every manifest and in `skills/bundle.lock`: changing the definition invalidates both.
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

pub fn bundle_hash() -> String {
    hash_of(FILES)
}

pub const LOCK: &str = "skills/bundle.lock";

/// A skill edit cannot ship under an old version.
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

pub fn version_parts(v: &str) -> Option<(u64, u64, u64)> {
    let mut parts = v.split('.').map(|p| p.parse::<u64>().ok());
    let triple = (parts.next()??, parts.next()??, parts.next()??);
    parts.next().is_none().then_some(triple)
}

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

#[derive(Debug, Default)]
pub struct Rendered {
    pub files: Vec<String>,
    pub links: Vec<String>,
    pub shared: bool,
}

impl Rendered {
    pub fn summary(&self) -> String {
        let v = env!("CARGO_PKG_VERSION");
        let n = self.files.len();
        if self.shared {
            format!(
                "skills {v}: {SKILLS_DIR}/ ({n} files, factory.toml), {LINKS_DIR}/ is {SKILLS_DIR}/ (no links)"
            )
        } else {
            format!(
                "skills {v}: {SKILLS_DIR}/ ({n} files, factory.toml), {LINKS_DIR}/ ({} links)",
                self.links.len()
            )
        }
    }

    /// `git add` refuses a path beyond a symlinked `.agents`, and a shared link is the owner's.
    pub fn dirs(&self) -> Vec<&'static str> {
        if self.shared {
            vec![SKILLS_DIR]
        } else {
            vec![SKILLS_DIR, LINKS_DIR]
        }
    }
}

pub fn link_target(name: &str) -> String {
    format!("../../{SKILLS_DIR}/{name}")
}

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

/// A symlink is removed first so the bundle never writes through it.
fn real_dir(path: &Path) -> Result<()> {
    if fs::symlink_metadata(path).is_ok_and(|m| !m.is_dir()) {
        clear(path)?;
    }
    fs::create_dir_all(path).map_err(io(format!("mkdir {}", path.display())))
}

fn write_real(path: &Path, text: &str) -> Result<()> {
    if fs::symlink_metadata(path).is_ok_and(|m| !m.is_file()) {
        clear(path)?;
    }
    fs::write(path, text).map_err(io(format!("write {}", path.display())))
}

fn not_real_dir(root: &Path, rel: &str) -> Option<String> {
    let meta = fs::symlink_metadata(root.join(rel)).ok()?;
    if meta.file_type().is_symlink() {
        Some(format!("{rel} is a symlink"))
    } else if !meta.is_dir() {
        Some(format!("{rel} is not a directory"))
    } else {
        None
    }
}

fn refuse_layout(why: String) -> Error {
    Error::Msg(format!(
        "{why}: the skills are rendered into real directories only, nothing written"
    ))
}

/// A symlinked `.claude` or `.agents` would carry the writes outside the repo; `.agents/skills` resolving to `.claude/skills` is fine.
fn layout(root: &Path) -> Result<bool> {
    for rel in [".claude", SKILLS_DIR] {
        if let Some(why) = not_real_dir(root, rel) {
            return Err(refuse_layout(why));
        }
    }
    let claude = root.join(".claude");
    let skills = root.join(SKILLS_DIR);
    let (had_claude, had_skills) = (claude.exists(), skills.exists());
    fs::create_dir_all(&skills).map_err(io(format!("mkdir {}", skills.display())))?;
    let shared = match (
        fs::canonicalize(root.join(LINKS_DIR)),
        fs::canonicalize(&skills),
    ) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    };
    if shared {
        return Ok(true);
    }
    for rel in [".agents", LINKS_DIR] {
        if let Some(why) = not_real_dir(root, rel) {
            if !had_skills {
                let _ = fs::remove_dir(&skills);
            }
            if !had_claude {
                let _ = fs::remove_dir(&claude);
            }
            return Err(refuse_layout(why));
        }
    }
    Ok(false)
}

pub fn render(root: &Path) -> Result<Rendered> {
    let shared = layout(root)?;
    let skills = root.join(SKILLS_DIR);
    let mut done = Rendered {
        shared,
        ..Rendered::default()
    };
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
    let names: &[&str] = if shared { &[] } else { &NAMES };
    if !shared {
        fs::create_dir_all(&agents).map_err(io(format!("mkdir {}", agents.display())))?;
    }
    let mut links = BTreeMap::new();
    for name in names {
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

/// This repo's own layout: a session reads the live source, so `init` leaves the skills alone.
pub fn source_checkout(root: &Path) -> bool {
    let (Ok(prd), Ok(src)) = (
        fs::canonicalize(root.join(SKILLS_DIR).join("prd")),
        fs::canonicalize(root.join("skills")),
    ) else {
        return false;
    };
    prd.starts_with(src)
}

#[derive(Debug, Clone, PartialEq)]
pub enum Version {
    Current(String),
    Behind { repo: String, binary: String },
    Ahead { repo: String, binary: String },
    Differs(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Verdict {
    SourceCheckout,
    None,
    Unreadable(String),
    Checked {
        version: Version,
        hand_edited: Vec<String>,
        missing: Vec<String>,
    },
}

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

pub fn verdict(root: &Path) -> Verdict {
    verdict_for(root, env!("CARGO_PKG_VERSION"))
}

fn verdict_for(root: &Path, binary: &str) -> Verdict {
    if source_checkout(root) {
        return Verdict::SourceCheckout;
    }
    let m = match read_manifest(root) {
        Ok(Some(m)) => m,
        Ok(None) => return Verdict::None,
        Err(e) => return Verdict::Unreadable(format!("unreadable {MANIFEST}: {e}")),
    };
    let binary = binary.to_string();
    let Some(repo_v) = version_parts(&m.version) else {
        return Verdict::Unreadable(format!(
            "unreadable {MANIFEST}: version \"{}\" is not three integers",
            m.version
        ));
    };
    // A pre-release build (`0.3.0-rc.1`): the binary is at fault, not the repo.
    let Some(bin_v) = version_parts(&binary) else {
        return Verdict::Unreadable(format!("binary version {binary} is not three integers"));
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
    for (rel, hash) in m.files.iter().filter(|(r, _)| plain(r)) {
        match fs::read(skills.join(rel)) {
            Ok(bytes) if sha256_hex(&bytes) == *hash => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => missing.push(rel.clone()),
            _ => hand_edited.push(rel.clone()),
        }
    }
    let prefix = format!("{LINKS_DIR}/");
    for (rel, target) in &m.links {
        if !rel.strip_prefix(&prefix).is_some_and(plain) {
            continue;
        }
        if fs::read_link(root.join(rel)).ok().as_deref() != Some(Path::new(target)) {
            missing.push(rel.clone());
        }
    }
    Verdict::Checked {
        version,
        hand_edited,
        missing,
    }
}

impl Verdict {
    pub fn lines(&self) -> Vec<String> {
        self.lines_with(crate::cli::upgrade::ahead_hint)
    }

    /// The hint is read only on `ahead`: it looks for the install receipt.
    fn lines_with(&self, ahead_hint: impl FnOnce() -> String) -> Vec<String> {
        let (version, hand_edited, missing) = match self {
            Verdict::SourceCheckout => return vec!["source checkout".into()],
            Verdict::None => return vec!["none → factory init".into()],
            Verdict::Unreadable(line) => return vec![line.clone()],
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

/// A hand-written manifest key must not reach out of the dir it is joined to.
fn plain(rel: &str) -> bool {
    !rel.is_empty()
        && Path::new(rel)
            .components()
            .all(|c| matches!(c, std::path::Component::Normal(_)))
}

/// A `plain` key can still pass through a symlinked dir, so each removal checks what resolves where.
pub fn remove_dropped(root: &Path, old: &Manifest, new: &Manifest) -> Result<Vec<String>> {
    let skills = root.join(SKILLS_DIR);
    let canon = fs::canonicalize(&skills).ok();
    let within = |p: &Path, strictly: bool| match (&canon, fs::canonicalize(p)) {
        (Some(c), Ok(p)) => p.starts_with(c) && !(strictly && p == *c),
        _ => false,
    };
    let mut removed = Vec::new();
    for rel in old.files.keys().filter(|r| !new.files.contains_key(*r)) {
        let path = skills.join(rel);
        if !plain(rel)
            || !fs::symlink_metadata(&path).is_ok_and(|m| !m.is_dir())
            || !path.parent().is_some_and(|p| within(p, false))
        {
            continue;
        }
        fs::remove_file(&path).map_err(io(format!("remove {}", path.display())))?;
        removed.push(format!("{SKILLS_DIR}/{rel}"));
        let mut dir = path.parent();
        while let Some(d) = dir.filter(|d| *d != skills && within(d, true)) {
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
        if !plain(name) || !fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink()) {
            continue;
        }
        fs::remove_file(&path).map_err(io(format!("remove {}", path.display())))?;
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

    fn listing(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(dir)
            .map(|rd| {
                rd.map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default();
        names.sort();
        names
    }

    #[test]
    fn render_refuses_a_linked_or_non_dir_layout_and_writes_nothing() {
        let cases: [(&str, &str, &str); 5] = [
            (".claude", "", "is a symlink"),
            (SKILLS_DIR, ".claude", "is a symlink"),
            (".agents", "", "is a symlink"),
            (LINKS_DIR, ".agents", "is a symlink"),
            (".claude", "file", "is not a directory"),
        ];
        for (at, real_parent, says) in cases {
            let dir = tempfile::tempdir().unwrap();
            let root = dir.path();
            let outside = tempfile::tempdir().unwrap();
            fs::create_dir(outside.path().join("polish")).unwrap();
            fs::write(outside.path().join("polish/keep.md"), "keep").unwrap();
            if real_parent == "file" {
                fs::write(root.join(at), "x").unwrap();
            } else {
                if !real_parent.is_empty() {
                    fs::create_dir(root.join(real_parent)).unwrap();
                }
                symlink(outside.path(), root.join(at)).unwrap();
            }
            let before = listing(root);
            let err = render(root).unwrap_err().to_string();
            assert!(
                err.contains(&format!("{at} {says}"))
                    && err.contains("real directories only, nothing written"),
                "{at}: {err}"
            );
            assert_eq!(listing(root), before, "{at}");
            assert_eq!(listing(outside.path()), ["polish"], "{at}");
            assert_eq!(listing(&outside.path().join("polish")), ["keep.md"], "{at}");
        }
    }

    #[test]
    fn an_agents_link_to_the_skills_dir_shares_them_without_links() {
        for (at, target) in [(LINKS_DIR, "../.claude/skills"), (".agents", ".claude")] {
            let dir = tempfile::tempdir().unwrap();
            let root = dir.path();
            if at == LINKS_DIR {
                fs::create_dir(root.join(".agents")).unwrap();
            }
            symlink(target, root.join(at)).unwrap();
            let done = render(root).unwrap();
            assert!(done.shared && done.links.is_empty(), "{at}");
            assert_eq!(done.files.len(), FILES.len(), "{at}");
            assert!(read_manifest(root).unwrap().unwrap().links.is_empty());
            for name in NAMES {
                let real = fs::symlink_metadata(root.join(SKILLS_DIR).join(name)).unwrap();
                assert!(real.is_dir() && !real.is_symlink(), "{at} {name}");
                assert!(root.join(LINKS_DIR).join(name).join("SKILL.md").is_file());
            }
            assert!(fs::symlink_metadata(root.join(at)).unwrap().is_symlink());
            assert!(verdict(root).is_ok(), "{at}");
            assert!(
                done.summary()
                    .ends_with(".agents/skills/ is .claude/skills/ (no links)")
            );
            assert_eq!(done.dirs(), [SKILLS_DIR]);
        }
    }

    #[test]
    fn remove_dropped_never_reaches_through_a_link_or_takes_a_dir() {
        let dir = rendered();
        let root = dir.path();
        let new = read_manifest(root).unwrap().unwrap();
        let mut old = new.clone();
        let outside = tempfile::tempdir().unwrap();
        fs::create_dir(outside.path().join("sub")).unwrap();
        fs::write(outside.path().join("target.txt"), "t").unwrap();
        fs::write(outside.path().join("sub/x.md"), "x").unwrap();
        symlink(outside.path(), root.join(SKILLS_DIR).join("evil")).unwrap();
        for key in ["evil/target.txt", "evil/sub/x.md", "gate"] {
            old.files.insert(key.into(), file_hash("t"));
        }
        fs::create_dir(root.join(LINKS_DIR).join("real")).unwrap();
        old.links
            .insert(format!("{LINKS_DIR}/real"), link_target("real"));

        assert_eq!(
            remove_dropped(root, &old, &new).unwrap(),
            Vec::<String>::new()
        );
        assert_eq!(listing(outside.path()), ["sub", "target.txt"]);
        assert_eq!(listing(&outside.path().join("sub")), ["x.md"]);
        assert!(root.join(SKILLS_DIR).join("gate/SKILL.md").is_file());
        assert!(root.join(LINKS_DIR).join("real").is_dir());
    }

    #[test]
    fn a_deleted_or_retargeted_link_is_missing() {
        let dir = rendered();
        let root = dir.path();
        fs::remove_file(root.join(LINKS_DIR).join("polish")).unwrap();
        fs::remove_file(root.join(LINKS_DIR).join("ux")).unwrap();
        symlink(link_target("prd"), root.join(LINKS_DIR).join("ux")).unwrap();
        let v = verdict(root);
        assert_eq!(
            v.lines(),
            [format!("missing: {LINKS_DIR}/polish, {LINKS_DIR}/ux")]
        );
        assert!(!v.is_ok());
    }

    #[test]
    fn a_key_leaving_its_dir_is_never_read() {
        let dir = rendered();
        let root = dir.path();
        fs::write(root.join(".claude/outside.md"), "o").unwrap();
        edit_manifest(root, |m| {
            m.files.insert("../outside.md".into(), file_hash("other"));
            m.links
                .insert(format!("{LINKS_DIR}/../../x"), "anything".into());
            m.links.insert("elsewhere/x".into(), "anything".into());
        });
        assert_eq!(verdict(root).lines(), [format!("current ({V})")]);
    }

    #[test]
    fn an_unparseable_binary_version_blames_the_binary() {
        let dir = rendered();
        assert_eq!(
            verdict_for(dir.path(), "0.3.0-rc.1").lines(),
            ["binary version 0.3.0-rc.1 is not three integers"]
        );
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
