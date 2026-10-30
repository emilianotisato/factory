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

fn numbered_decisions(rel: &str, text: &str) -> Vec<String> {
    text.lines()
        .enumerate()
        .filter(|(_, line)| line.contains("DEC-"))
        .map(|(i, line)| format!("{rel}:{}: {line}", i + 1))
        .collect()
}

// Decisions are cited by their slug; the old numbered anchors are lowercase `dec-n`.
#[test]
fn no_skill_or_asset_asks_for_a_numbered_decision() {
    let cited = format!("a\nsee {}-1\n", "DEC");
    assert_eq!(numbered_decisions("x.md", &cited).len(), 1);
    let mut hits = Vec::new();
    for tree in ["skills", "assets"] {
        let base = root().join(tree);
        let mut files = BTreeSet::new();
        walk(&base, &base, &mut files);
        for rel in files {
            let Ok(text) = fs::read_to_string(base.join(&rel)) else {
                continue;
            };
            hits.extend(numbered_decisions(&format!("{tree}/{rel}"), &text));
        }
    }
    assert!(
        hits.is_empty(),
        "cite docs/decisions.md#{{slug}}:\n{}",
        hits.join("\n")
    );
}

// `models.md` names roles, never a machine's setup.
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

#[test]
fn polish_collect_starts_from_the_collected_file_with_the_last_resort_beside_it() {
    let text = fs::read_to_string(root().join("skills/polish/SKILL.md")).unwrap();
    let start = text.find("## 1.").expect("§1");
    let end = start + text[start..].find("## 2.").expect("§2");
    let collect = &text[start..end];
    for keep in [
        "factory review collect",
        "factory leaks --base",
        "factory review second --diff",
    ] {
        assert!(collect.contains(keep), "{keep:?} left polish §1");
    }
}

#[test]
fn polish_fixers_run_one_after_another_with_no_lane_rule() {
    let text = fs::read_to_string(root().join("skills/polish/SKILL.md")).unwrap();
    let start = text.find("## 5. Fix").expect("§5");
    let end = start + text[start..].find("## 6.").expect("§6");
    let fix = &text[start..end];
    let lower = fix.to_lowercase();
    for bad in ["lane", "sibling"] {
        assert!(!lower.contains(bad), "{bad:?} in polish §5:\n{fix}");
    }
    for keep in ["factory docs log add", "factory docs gate"] {
        assert!(fix.contains(keep), "{keep:?} left polish §5");
    }
}

#[test]
fn gate_inner_loop_formats_in_write_mode_and_runs_tests_quiet() {
    let text = fs::read_to_string(root().join("skills/gate/SKILL.md")).unwrap();
    let start = text.find("**Round 3").expect("round 3");
    let end = start + text[start..].find("## 4.").expect("§4");
    let round = &text[start..end];
    for keep in ["write mode", "check mode", "`cargo test -q`"] {
        assert!(round.contains(keep), "{keep:?} left gate round 3");
    }
    assert!(!round.contains("same string"), "gate round 3:\n{round}");

    let stacks = fs::read_to_string(root().join("skills/gate/references/stacks.md")).unwrap();
    let rows: Vec<&str> = stacks
        .lines()
        .filter(|l| l.starts_with("| format |"))
        .collect();
    assert!(rows.len() >= 6, "{rows:?}");
    for row in rows {
        let forms = row.split('|').nth(2).unwrap();
        assert!(forms.contains("` / `"), "one format form only: {row}");
    }
}

/// One `OnceLock` so a `FACTORY_BUNDLE_WRITE` rewrite happens once and no reader races it.
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

// A skill edit cannot ship under the old version.
#[test]
fn bundle_lock_matches() {
    if let Err(e) = lock() {
        panic!("{e}");
    }
}

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

#[test]
fn the_prd_skill_offers_the_build_and_drops_the_clear_advice() {
    let text = fs::read_to_string(root().join("skills/prd/SKILL.md")).unwrap();
    for needle in [
        "Build it now (Recommended)",
        "Slice only",
        "Stop here",
        "invoke `/deliver NNN` with the Skill tool, in this session",
    ] {
        assert!(text.contains(needle), "{needle:?} missing");
    }
    assert!(!text.contains("Say the `/clear` part"));
}

#[test]
fn the_slice_skill_skips_the_write_question_only_for_deliver() {
    let text = fs::read_to_string(root().join("skills/slice/SKILL.md")).unwrap();
    for needle in [
        "Write these? [Y/n]",
        "`NNN from /deliver`",
        "**From `/deliver`** → print the board without the `Write these?` line",
        "**Typed by the owner** → ask",
    ] {
        assert!(text.contains(needle), "{needle:?} missing");
    }
    assert!(!text.contains("/factory-deliver"));
}

#[test]
fn every_doctor_step_links_the_stale_tooling_rule() {
    let read = |p: &str| fs::read_to_string(root().join("skills").join(p)).unwrap();
    for skill in ["prd", "slice", "deliver", "lanes"] {
        let text = read(&format!("{skill}/SKILL.md"));
        assert!(
            text.contains("(../_shared/stale-tooling.md)"),
            "{skill} does not link the rule"
        );
    }
    let rule = read("_shared/stale-tooling.md");
    for needle in [
        "**Any `hand-edited:` line** → ask first",
        "**`ahead (…) → <hint>`**",
        "**`behind`, `differs` or `missing`**, no hand edit → `factory skills update`",
        "**`source checkout`**",
    ] {
        assert!(rule.contains(needle), "{needle:?} missing");
    }
}
