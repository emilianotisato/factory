//! PRD-004 slice 01: the embedded bundle matches `skills/` on disk, and this
//! repo's `.claude/skills/` link into the live source.

use factory::skills::{FILES, NAMES};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

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
