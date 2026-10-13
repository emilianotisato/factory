//! PRD-004 slice 01: the embedded bundle matches `skills/` on disk, and this
//! repo's `.claude/skills/` link into the live source. Slice 07: the bundle
//! lock matches `Cargo.toml` and the bundle.

use factory::skills::{self, FILES, LOCK, Lock, Manifest, NAMES, bundle_hash, check_lock, relock};
use std::collections::BTreeSet;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Every file under `dir`, relative to `base`, `/` separated.
fn walk(base: &Path, dir: &Path, out: &mut BTreeSet<String>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            walk(base, &path, out);
        } else {
            let rel = path.strip_prefix(base).unwrap();
            let parts: Vec<_> = rel.iter().map(|p| p.to_string_lossy()).collect();
            out.insert(parts.join("/"));
        }
    }
}

#[test]
fn every_file_under_skills_is_in_the_bundle_byte_for_byte() {
    let skills = root().join("skills");
    let mut on_disk = BTreeSet::new();
    walk(&skills, &skills, &mut on_disk);
    // They describe the source folder, not the bundle.
    on_disk.remove("README.md");
    on_disk.remove("bundle.lock");

    let bundled: BTreeSet<String> = FILES.iter().map(|(p, _)| p.to_string()).collect();
    let missing: Vec<_> = on_disk.difference(&bundled).collect();
    assert!(
        missing.is_empty(),
        "on disk, not in the bundle: {missing:?}"
    );
    let gone: Vec<_> = bundled.difference(&on_disk).collect();
    assert!(gone.is_empty(), "in the bundle, not on disk: {gone:?}");

    for (path, content) in FILES {
        let bytes = fs::read(skills.join(path)).unwrap();
        assert!(bytes == content.as_bytes(), "{path} differs from its entry");
    }
}

#[test]
fn this_repos_claude_skills_link_into_the_source() {
    let names = NAMES.iter().copied().chain(["_shared"]);
    for name in names {
        let link = root().join(".claude/skills").join(name);
        let target = fs::read_link(&link).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(target, Path::new("../../skills").join(name), "{name}");
        let file = if name == "_shared" {
            "models.md"
        } else {
            "SKILL.md"
        };
        assert!(link.join(file).is_file(), "{name}/{file} does not resolve");
    }
}

/// PRD-004 FR-1 and FR-6: `models.md` names roles, not a machine's setup, and
/// the two skills that run the second opinion call the command.
#[test]
fn models_md_holds_no_invocation_and_the_skills_call_review_second() {
    let read = |p: &str| fs::read_to_string(root().join("skills").join(p)).unwrap();
    let models = read("_shared/models.md");
    for bad in [
        "```bash",
        "grok-",
        "gpt-",
        "gemini-",
        "~/.local/bin",
        "--trust",
        "cursor-agent -p",
    ] {
        assert!(!models.contains(bad), "{bad:?} in models.md");
    }
    for skill in ["polish/SKILL.md", "prd/SKILL.md"] {
        assert!(read(skill).contains("factory review second"), "{skill}");
    }
    let mut files = BTreeSet::new();
    let skills = root().join("skills");
    walk(&skills, &skills, &mut files);
    for f in files {
        let text = fs::read_to_string(skills.join(&f)).unwrap_or_default();
        assert!(!text.contains("\"A diff\" invocation"), "{f}");
    }
}

/// The lock this run checks against. With `FACTORY_BUNDLE_WRITE` set, the
/// first caller rewrites it (FR-5's refusal applies) and every test in this
/// binary sees the same result, so no reader races the write.
fn lock() -> &'static Result<Lock, String> {
    static LOCKED: OnceLock<Result<Lock, String>> = OnceLock::new();
    LOCKED.get_or_init(|| {
        let path = root().join(LOCK);
        let old = match fs::read_to_string(&path) {
            Ok(text) => Some(Lock::parse(&text)?),
            Err(e) if e.kind() == ErrorKind::NotFound => None,
            Err(e) => return Err(format!("read {LOCK}: {e}")),
        };
        let (version, hash) = (env!("CARGO_PKG_VERSION"), bundle_hash());
        if std::env::var_os("FACTORY_BUNDLE_WRITE").is_some() {
            let new = relock(old.as_ref(), version, &hash)?;
            fs::write(&path, new.text()).map_err(|e| format!("write {LOCK}: {e}"))?;
            return Ok(new);
        }
        let old = old.ok_or(format!("no {LOCK}: run FACTORY_BUNDLE_WRITE=1 cargo test"))?;
        check_lock(&old, version, &hash)?;
        Ok(old)
    })
}

/// PRD-004 FR-5: a skill edit cannot ship under the old version.
#[test]
fn bundle_lock_matches() {
    if let Err(e) = lock() {
        panic!("{e}");
    }
}

/// PRD-004 FR-5: the lock records what `init` writes into a manifest.
#[test]
fn the_lock_records_what_a_manifest_carries() {
    let lock = lock().as_ref().unwrap_or_else(|e| panic!("{e}"));
    let repo = tempfile::tempdir().unwrap();
    let ok = Command::new("git")
        .args(["init", "-q"])
        .current_dir(repo.path())
        .status()
        .unwrap();
    assert!(ok.success());
    skills::render(repo.path()).unwrap();
    let text = fs::read_to_string(repo.path().join(skills::MANIFEST)).unwrap();
    let manifest: Manifest = toml::from_str(&text).unwrap();
    assert_eq!(manifest.bundle, lock.sha256);
    assert_eq!(manifest.version, lock.version);
}
