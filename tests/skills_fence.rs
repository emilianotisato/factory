//! The skills fence (PRD-004 FR-3, FR-4, slice 03): `factory docs doctor`
//! prints a `skills` line per verdict, preflight repeats a stale one as a
//! judge line, and `factory skills update` re-renders only what is safe to.
//! Every repo here is built by `factory init`; versions come from
//! `CARGO_PKG_VERSION`, which slice 07 bumps.
#[path = "harness/mod.rs"]
mod harness;

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::symlink;
use std::path::Path;
use std::process::{Command, Output};

use factory::cli::upgrade::INSTALLER;
use factory::skills::{self, FILES, MANIFEST, Manifest};
use harness::Fixture;

const V: &str = env!("CARGO_PKG_VERSION");

fn run(dir: &Path, program: &str, args: &[&str]) -> Output {
    let program = if program == "factory" {
        env!("CARGO_BIN_EXE_factory")
    } else {
        program
    };
    // The config and the install receipt live under `.git`, where git does
    // not see them: never the owner's real `~/.config/factory`.
    Command::new(program)
        .args(args)
        .current_dir(dir)
        .env("FACTORY_CONFIG_DIR", dir.join(".git/fence-config"))
        .env("AXOUPDATER_CONFIG_PATH", dir.join(".git/fence-receipt"))
        .env_remove("AXOUPDATER_CONFIG_WORKING_DIR")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", "Fence")
        .env("GIT_AUTHOR_EMAIL", "fence@example.invalid")
        .env("GIT_COMMITTER_NAME", "Fence")
        .env("GIT_COMMITTER_EMAIL", "fence@example.invalid")
        .output()
        .expect("spawn")
}

fn ok(dir: &Path, program: &str, args: &[&str]) -> String {
    let o = run(dir, program, args);
    assert!(
        o.status.success(),
        "{program} {args:?}: {}{}",
        stdout(&o),
        String::from_utf8_lossy(&o.stderr)
    );
    stdout(&o)
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn commit(dir: &Path, msg: &str) {
    ok(dir, "git", &["add", "-A"]);
    ok(dir, "git", &["commit", "-q", "-m", msg]);
}

/// A git repo after `factory init`, committed.
fn initialised() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    ok(dir.path(), "git", &["init", "-q", "-b", "main"]);
    ok(dir.path(), "factory", &["init"]);
    commit(dir.path(), "chore: factory init");
    dir
}

fn edit_manifest(root: &Path, f: impl FnOnce(&mut Manifest)) {
    let path = root.join(MANIFEST);
    let mut m: Manifest = toml::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    f(&mut m);
    fs::write(&path, toml::to_string(&m).unwrap()).unwrap();
}

fn skill(root: &Path, rel: &str) -> std::path::PathBuf {
    root.join(".claude/skills").join(rel)
}

/// The seven verdict fixtures, by name, from a committed `init`.
fn fixture(kind: &str) -> tempfile::TempDir {
    let dir = initialised();
    let root = dir.path();
    match kind {
        "current" => {}
        "none" => fs::remove_file(root.join(MANIFEST)).unwrap(),
        "behind" => edit_manifest(root, |m| m.version = "0.0.0".into()),
        "ahead" => edit_manifest(root, |m| m.version = "999.0.0".into()),
        "differs" => edit_manifest(root, |m| m.bundle = "0".repeat(64)),
        "hand-edited" => {
            let p = skill(root, "polish/SKILL.md");
            let text = fs::read_to_string(&p).unwrap();
            fs::write(&p, format!("{text}x")).unwrap();
        }
        "missing" => fs::remove_file(skill(root, "ux/SKILL.md")).unwrap(),
        other => panic!("no fixture {other}"),
    }
    dir
}

/// A repo laid out like this one: `.claude/skills/prd` → `../../skills/prd`.
fn source_checkout() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    ok(root, "git", &["init", "-q", "-b", "main"]);
    fs::create_dir_all(root.join("skills/prd")).unwrap();
    fs::write(root.join("skills/prd/SKILL.md"), "live\n").unwrap();
    fs::create_dir_all(root.join(".claude/skills")).unwrap();
    symlink("../../skills/prd", root.join(".claude/skills/prd")).unwrap();
    ok(root, "factory", &["init"]);
    commit(root, "chore: source checkout");
    dir
}

/// Doctor's `skills` lines, asserting git sees no change from running it.
fn doctor_skills(root: &Path) -> Vec<String> {
    let before = ok(root, "git", &["status", "--porcelain"]);
    let out = ok(root, "factory", &["docs", "doctor"]);
    assert_eq!(ok(root, "git", &["status", "--porcelain"]), before);
    out.lines()
        .filter(|l| l.starts_with("skills "))
        .map(str::to_string)
        .collect()
}

fn padded(value: &str) -> String {
    format!("{:<15}{value}", "skills")
}

/// Every file and link under `.claude/skills` and `.agents/skills`.
fn snapshot(root: &Path) -> BTreeMap<String, String> {
    fn walk(root: &Path, dir: &Path, into: &mut BTreeMap<String, String>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries {
            let path = entry.unwrap().path();
            let rel = path.strip_prefix(root).unwrap().display().to_string();
            let meta = fs::symlink_metadata(&path).unwrap();
            if meta.is_symlink() {
                into.insert(
                    rel,
                    format!("link:{}", fs::read_link(&path).unwrap().display()),
                );
            } else if meta.is_dir() {
                into.insert(rel, "dir".into());
                walk(root, &path, into);
            } else {
                into.insert(rel, format!("file:{}", fs::read_to_string(&path).unwrap()));
            }
        }
    }
    let mut map = BTreeMap::new();
    walk(root, &root.join(".claude/skills"), &mut map);
    walk(root, &root.join(".agents/skills"), &mut map);
    map
}

fn update(root: &Path) -> (i32, String) {
    let o = run(root, "factory", &["skills", "update"]);
    (o.status.code().unwrap_or(-1), stdout(&o))
}

fn commits(root: &Path) -> String {
    ok(root, "git", &["rev-list", "--count", "HEAD"])
}

// ── doctor ───────────────────────────────────────────────────────────────────

#[test]
fn doctor_prints_one_line_per_verdict() {
    let cases = [
        ("current", format!("current ({V})")),
        ("none", "none → factory init".to_string()),
        (
            "behind",
            format!("behind (repo 0.0.0, binary {V}) → factory skills update"),
        ),
        (
            "ahead",
            format!("ahead (repo 999.0.0, binary {V}) → {INSTALLER}"),
        ),
        (
            "differs",
            format!("differs ({V} both, bundle differs: a dev build) → factory skills update"),
        ),
        ("hand-edited", "hand-edited: polish/SKILL.md".to_string()),
        ("missing", "missing: ux/SKILL.md".to_string()),
    ];
    for (kind, line) in cases {
        let dir = fixture(kind);
        assert_eq!(doctor_skills(dir.path()), [padded(&line)], "{kind}");
    }
    let dir = source_checkout();
    assert_eq!(doctor_skills(dir.path()), [padded("source checkout")]);
}

#[test]
fn doctor_puts_the_skills_line_after_gate() {
    let dir = fixture("current");
    let out = ok(dir.path(), "factory", &["docs", "doctor"]);
    let keys: Vec<&str> = out
        .lines()
        .filter_map(|l| l.split_whitespace().next())
        .collect();
    assert_eq!(keys[..4], ["Framework", "gate", "skills", "drift"], "{out}");
}

// ── preflight ────────────────────────────────────────────────────────────────

fn preflight(fx: &Fixture) -> (serde_json::Value, Option<i32>) {
    let o = fx.docs_output(&["preflight", "001", "--json"]);
    (serde_json::from_slice(&o.stdout).unwrap(), o.status.code())
}

fn skills_lines(v: &serde_json::Value, bucket: &str) -> Vec<String> {
    v[bucket]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|l| l.as_str())
        .filter(|l| l.starts_with("skills "))
        .map(str::to_string)
        .collect()
}

#[test]
fn preflight_notes_current_as_ok_and_behind_as_a_judge_line() {
    let fx = Fixture::new("skills-fence-pf");
    let (clean, clean_code) = preflight(&fx);
    assert_eq!(
        skills_lines(&clean, "ok"),
        [format!("skills current ({V})")]
    );
    assert!(skills_lines(&clean, "judge").is_empty(), "{clean}");

    edit_manifest(fx.path(), |m| m.version = "0.0.0".into());
    fx.git(&["commit", "-qam", "test: a behind manifest"]);
    let (behind, code) = preflight(&fx);
    assert_eq!(code, clean_code, "{behind}");
    assert_eq!(
        skills_lines(&behind, "judge"),
        [format!(
            "skills behind (repo 0.0.0, binary {V}) → factory skills update"
        )]
    );
    assert!(skills_lines(&behind, "ok").is_empty(), "{behind}");
}

// ── update ───────────────────────────────────────────────────────────────────

#[test]
fn update_renders_a_behind_repo_and_drops_what_the_bundle_lost() {
    let dir = fixture("behind");
    let root = dir.path();
    fs::create_dir_all(skill(root, "old")).unwrap();
    fs::write(skill(root, "old/SKILL.md"), "retired\n").unwrap();
    symlink("../../.claude/skills/old", root.join(".agents/skills/old")).unwrap();
    edit_manifest(root, |m| {
        m.files
            .insert("old/SKILL.md".into(), skills::file_hash("retired\n"));
        m.links.insert(
            ".agents/skills/old".into(),
            "../../.claude/skills/old".into(),
        );
    });
    commit(root, "test: behind with a retired skill");
    let before = commits(root);

    let (code, out) = update(root);
    assert_eq!(code, 0, "{out}");
    for (rel, content) in FILES {
        assert_eq!(
            &fs::read_to_string(skill(root, rel)).unwrap(),
            content,
            "{rel}"
        );
    }
    let m: Manifest = toml::from_str(&fs::read_to_string(root.join(MANIFEST)).unwrap()).unwrap();
    assert_eq!(m.version, V);
    assert!(!m.files.contains_key("old/SKILL.md"));
    assert!(fs::symlink_metadata(skill(root, "old")).is_err(), "{out}");
    assert!(
        fs::symlink_metadata(root.join(".agents/skills/old")).is_err(),
        "{out}"
    );
    assert!(
        out.contains(&format!(
            "commit: git add .claude/skills .agents/skills && git commit -m \"chore: skills {V}\""
        )),
        "{out}"
    );
    assert_eq!(commits(root), before);
    assert_eq!(update(root), (0, "already current\n".to_string()));
}

#[test]
fn update_refuses_ahead_and_writes_nothing() {
    let dir = fixture("ahead");
    let before = snapshot(dir.path());
    let (code, out) = update(dir.path());
    assert_eq!(code, 1);
    assert_eq!(
        out,
        format!("ahead (repo 999.0.0, binary {V}) → {INSTALLER}\n")
    );
    assert_eq!(snapshot(dir.path()), before);
}

/// A binary built from source with `[source] path` set: doctor and `skills
/// update` both point at `cargo install --path` (FR-8).
#[test]
fn ahead_from_source_points_at_the_checkout() {
    let dir = fixture("ahead");
    let root = dir.path();
    let config = root.join(".git/fence-config");
    fs::create_dir_all(&config).unwrap();
    fs::write(
        config.join("config.toml"),
        "[source]\npath = \"/src/factory\"\n",
    )
    .unwrap();
    let line = format!("ahead (repo 999.0.0, binary {V}) → cargo install --path /src/factory");
    assert_eq!(doctor_skills(root), [padded(&line)]);
    assert_eq!(update(root), (1, format!("{line}\n")));
}

#[test]
fn update_refuses_a_hand_edit_until_git_restores_it() {
    let dir = fixture("hand-edited");
    let root = dir.path();
    let before = snapshot(root);
    let (code, out) = update(root);
    assert_eq!(code, 1);
    assert!(
        out.contains("git checkout -- .claude/skills/polish/SKILL.md"),
        "{out}"
    );
    assert_eq!(snapshot(root), before);
    ok(
        root,
        "git",
        &["checkout", "--", ".claude/skills/polish/SKILL.md"],
    );
    assert_eq!(update(root), (0, "already current\n".to_string()));
}

#[test]
fn update_restores_a_missing_file() {
    let dir = fixture("missing");
    let (code, out) = update(dir.path());
    assert_eq!(code, 0, "{out}");
    let ux = FILES.iter().find(|(p, _)| *p == "ux/SKILL.md").unwrap().1;
    assert_eq!(
        fs::read_to_string(skill(dir.path(), "ux/SKILL.md")).unwrap(),
        ux
    );
}

#[test]
fn update_on_current_none_and_a_source_checkout() {
    let dir = fixture("current");
    assert_eq!(update(dir.path()), (0, "already current\n".to_string()));

    for dir in [fixture("none"), source_checkout()] {
        let before = snapshot(dir.path());
        let (code, out) = update(dir.path());
        assert_eq!(code, 1, "{out}");
        assert_eq!(snapshot(dir.path()), before);
    }
}
