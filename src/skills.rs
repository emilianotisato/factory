//! The skill family, embedded (PRD-004 §3 FR-1). `skills/` in this repo is
//! the source; the binary carries every file under the eight skill folders
//! and `skills/_shared/` as one bundle, listed by hand like the assets in
//! [`crate::pack`]. `skills/README.md` and `skills/bundle.lock` describe the
//! source folder, so neither is bundled. `tests/skills_bundle.rs` walks
//! `skills/` and fails on a file this list misses.

use crate::metrics::sha256_hex;

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
