//! `factory init` (PRD-004 FR-2, slice 02): the docs layout, the skill family
//! byte-identical in `.claude/skills/`, the `.agents/skills` links, the
//! manifest and the convention block, printed with the commit line and
//! committed by nobody.
use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::symlink;
use std::path::Path;
use std::process::{Command, Output};

use factory::skills::{self, FILES, MANIFEST, Manifest, NAMES};

const HEADING: &str = "# Product Docs Convention";

fn factory(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_factory"))
        .args(args)
        .current_dir(dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .expect("run factory")
}

/// `factory init`, asserted to exit 0; its stdout.
fn init(dir: &Path) -> String {
    let out = factory(dir, &["init"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    stdout(&out)
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

fn git(dir: &Path, args: &[&str]) -> Output {
    Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .expect("run git")
}

fn repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    assert!(
        git(dir.path(), &["init", "-q", "-b", "main"])
            .status
            .success()
    );
    dir
}

/// Every file and link under `root` but `.git`: path → `file:<bytes>`,
/// `link:<target>` or `dir`.
fn tree(root: &Path) -> BTreeMap<String, String> {
    fn walk(root: &Path, dir: &Path, into: &mut BTreeMap<String, String>) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let rel = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .into_owned();
            if rel == ".git" {
                continue;
            }
            let meta = fs::symlink_metadata(&path).unwrap();
            if meta.is_symlink() {
                let target = fs::read_link(&path).unwrap();
                into.insert(rel, format!("link:{}", target.display()));
            } else if meta.is_dir() {
                into.insert(rel, "dir".into());
                walk(root, &path, into);
            } else {
                into.insert(rel, format!("file:{}", fs::read_to_string(&path).unwrap()));
            }
        }
    }
    let mut map = BTreeMap::new();
    walk(root, root, &mut map);
    map
}

fn manifest(root: &Path) -> Manifest {
    toml::from_str(&fs::read_to_string(root.join(MANIFEST)).unwrap()).unwrap()
}

fn read(root: &Path, rel: &str) -> String {
    fs::read_to_string(root.join(rel)).unwrap()
}

#[test]
fn init_and_docs_init_write_the_same_tree_and_say_the_same() {
    let (a, b) = (repo(), repo());
    let said_a = init(a.path());
    let out = factory(b.path(), &["docs", "init"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert_eq!(said_a, stdout(&out));
    assert_eq!(tree(a.path()), tree(b.path()));
}

#[test]
fn init_renders_the_bundle_the_links_and_the_manifest() {
    let dir = repo();
    let root = dir.path();
    init(root);
    for (rel, content) in FILES {
        let on_disk = read(root, &format!(".claude/skills/{rel}"));
        assert_eq!(
            skills::file_hash(&on_disk),
            skills::file_hash(content),
            "{rel}"
        );
    }
    for name in NAMES {
        let link = root.join(".agents/skills").join(name);
        assert!(fs::symlink_metadata(&link).unwrap().is_symlink(), "{name}");
        assert_eq!(
            fs::read_link(&link).unwrap(),
            Path::new(&format!("../../.claude/skills/{name}"))
        );
        assert!(link.join("SKILL.md").is_file(), "{name}");
    }
    assert!(fs::symlink_metadata(root.join(".agents/skills/_shared")).is_err());

    assert!(root.join(".claude/skills/lanes/SKILL.md").is_file());
    let m = manifest(root);
    assert_eq!(m.version, env!("CARGO_PKG_VERSION"));
    assert_eq!(m.bundle, skills::bundle_hash());
    assert_eq!(m.files.len(), 20);
    for (rel, hash) in &m.files {
        let on_disk = read(root, &format!(".claude/skills/{rel}"));
        assert_eq!(&skills::file_hash(&on_disk), hash, "{rel}");
    }
    assert_eq!(m.links.len(), 9);
    assert_eq!(
        m.links[".agents/skills/polish"],
        "../../.claude/skills/polish"
    );
}

#[test]
fn init_renders_over_a_hand_written_skill_and_never_through_a_link() {
    let dir = repo();
    let root = dir.path();
    fs::create_dir_all(root.join(".claude/skills/polish")).unwrap();
    fs::write(root.join(".claude/skills/polish/SKILL.md"), "mine\n").unwrap();
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("keep.md"), "outside\n").unwrap();
    symlink(outside.path(), root.join(".claude/skills/ux")).unwrap();

    init(root);
    let polish = read(root, ".claude/skills/polish/SKILL.md");
    let bundled = FILES
        .iter()
        .find(|(p, _)| *p == "polish/SKILL.md")
        .unwrap()
        .1;
    assert_eq!(polish, bundled);
    assert_eq!(
        manifest(root).files["polish/SKILL.md"],
        skills::file_hash(&polish)
    );

    let ux = fs::symlink_metadata(root.join(".claude/skills/ux")).unwrap();
    assert!(ux.is_dir() && !ux.is_symlink());
    assert!(root.join(".claude/skills/ux/SKILL.md").is_file());
    let left: Vec<_> = fs::read_dir(outside.path())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(left, ["keep.md"]);
    assert_eq!(read(outside.path(), "keep.md"), "outside\n");
}

#[test]
fn init_refuses_a_linked_skills_dir_and_writes_nothing() {
    let dir = repo();
    let root = dir.path();
    let outside = tempfile::tempdir().unwrap();
    fs::create_dir(root.join(".claude")).unwrap();
    symlink(outside.path(), root.join(".claude/skills")).unwrap();
    let before = tree(root);
    let out = factory(root, &["init"]);
    assert_ne!(out.status.code(), Some(0), "{}", stdout(&out));
    assert!(
        stderr(&out).contains(
            ".claude/skills is a symlink: the skills are rendered into real directories only, nothing written"
        ),
        "{}",
        stderr(&out)
    );
    assert_eq!(tree(root), before);
    assert_eq!(fs::read_dir(outside.path()).unwrap().count(), 0);
}

/// `.agents/skills -> ../.claude/skills`: Codex reads the skills through the
/// link, so init writes no per-skill links, which would loop.
#[test]
fn init_into_a_shared_agents_dir_writes_no_links() {
    let dir = repo();
    let root = dir.path();
    fs::create_dir(root.join(".agents")).unwrap();
    symlink("../.claude/skills", root.join(".agents/skills")).unwrap();
    let said = init(root);
    let version = env!("CARGO_PKG_VERSION");
    assert!(
        said.contains(&format!(
            "skills {version}: .claude/skills/ (20 files, factory.toml), .agents/skills/ is .claude/skills/ (no links)\n"
        )),
        "{said}"
    );
    assert!(
        said.contains("commit: git add docs .claude/skills CLAUDE.md AGENTS.md && git commit"),
        "{said}"
    );
    for name in NAMES {
        let real = fs::symlink_metadata(root.join(".claude/skills").join(name)).unwrap();
        assert!(real.is_dir() && !real.is_symlink(), "{name}");
        assert!(
            root.join(".agents/skills")
                .join(name)
                .join("SKILL.md")
                .is_file()
        );
    }
    assert!(manifest(root).links.is_empty());
}

#[test]
fn a_convention_file_linking_outside_or_nowhere_is_skipped() {
    let dir = repo();
    let root = dir.path();
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("bashrc"), "export A=1\n").unwrap();
    symlink(outside.path().join("bashrc"), root.join("CLAUDE.md")).unwrap();
    symlink("nowhere.md", root.join("AGENTS.md")).unwrap();
    let said = init(root);
    assert_eq!(read(outside.path(), "bashrc"), "export A=1\n");
    assert!(fs::symlink_metadata(root.join("nowhere.md")).is_err());
    assert!(
        said.contains(
            "convention block: skipped CLAUDE.md (links outside the repo)\nconvention block: skipped AGENTS.md (a dangling link)\n"
        ),
        "{said}"
    );
    assert!(!said.contains("CLAUDE.md &&"), "{said}");
    let again = init(root);
    assert!(
        again.contains("skipped CLAUDE.md") && again.ends_with("already initialised\n"),
        "{again}"
    );
}

/// `CLAUDE.md -> AGENTS.md` before `AGENTS.md` exists dangles on its turn,
/// then resolves once `AGENTS.md` takes the block: no skip line.
#[test]
fn a_link_to_a_not_yet_written_agents_md_is_no_skip() {
    let dir = repo();
    let root = dir.path();
    symlink("AGENTS.md", root.join("CLAUDE.md")).unwrap();
    let said = init(root);
    assert!(!said.contains("skipped"), "{said}");
    assert!(said.contains("convention block: AGENTS.md\n"), "{said}");
    assert_eq!(read(root, "CLAUDE.md").matches(HEADING).count(), 1);
}

#[test]
fn a_crlf_convention_file_gets_no_second_block() {
    let dir = repo();
    let root = dir.path();
    init(root);
    let crlf = read(root, "CLAUDE.md").replace('\n', "\r\n");
    fs::write(root.join("CLAUDE.md"), &crlf).unwrap();
    assert!(init(root).contains("already initialised"));
    assert_eq!(read(root, "CLAUDE.md"), crlf);
}

#[test]
fn the_convention_block_lands_once_in_each_file() {
    let dir = repo();
    let root = dir.path();
    init(root);
    for f in ["CLAUDE.md", "AGENTS.md"] {
        let text = read(root, f);
        assert_eq!(text.matches(HEADING).count(), 1, "{f}");
        assert!(text.starts_with(HEADING), "{f}");
    }
    let before = (read(root, "CLAUDE.md"), read(root, "AGENTS.md"));
    init(root);
    assert_eq!((read(root, "CLAUDE.md"), read(root, "AGENTS.md")), before);

    // No line of the block reads as a gate hint.
    let out = factory(root, &["docs", "gate"]);
    let said = stdout(&out);
    assert!(said.contains("no gate declared"), "{said}");
    assert!(
        !said.contains("CLAUDE.md:") && !said.contains("AGENTS.md:"),
        "{said}"
    );
}

#[test]
fn a_linked_pair_gets_the_block_once_and_an_identical_pair_both() {
    let linked = repo();
    let root = linked.path();
    fs::write(root.join("AGENTS.md"), "# mine\n").unwrap();
    symlink("AGENTS.md", root.join("CLAUDE.md")).unwrap();
    let said = init(root);
    assert_eq!(
        read(root, "AGENTS.md"),
        format!("# mine\n\n{}", factory::docs::scaffold::CONVENTION)
    );
    assert!(
        fs::symlink_metadata(root.join("CLAUDE.md"))
            .unwrap()
            .is_symlink()
    );
    assert!(said.contains("convention block: AGENTS.md\n"), "{said}");

    let twins = repo();
    let root = twins.path();
    for f in ["CLAUDE.md", "AGENTS.md"] {
        fs::write(root.join(f), "# rules\n").unwrap();
    }
    init(root);
    let claude = read(root, "CLAUDE.md");
    assert_eq!(claude, read(root, "AGENTS.md"));
    assert_eq!(claude.matches(HEADING).count(), 1);
}

#[test]
fn a_second_init_changes_nothing_and_leaves_the_skills_to_update() {
    let dir = repo();
    let root = dir.path();
    init(root);
    let before = tree(root);
    assert!(init(root).contains("already initialised"));
    assert_eq!(tree(root), before);

    let edited = root.join(".claude/skills/prd/SKILL.md");
    fs::write(&edited, "my prd\n").unwrap();
    fs::remove_file(root.join("AGENTS.md")).unwrap();
    let said = init(root);
    assert_eq!(fs::read_to_string(&edited).unwrap(), "my prd\n");
    assert!(!said.contains("skills "), "{said}");
    assert!(said.contains("convention block: AGENTS.md\n"), "{said}");
}

#[test]
fn a_source_checkout_keeps_its_live_skills() {
    let dir = repo();
    let root = dir.path();
    fs::create_dir_all(root.join("skills/prd")).unwrap();
    fs::write(root.join("skills/prd/SKILL.md"), "live\n").unwrap();
    fs::create_dir_all(root.join(".claude/skills")).unwrap();
    symlink("../../skills/prd", root.join(".claude/skills/prd")).unwrap();
    init(root);
    assert!(!root.join(MANIFEST).exists());
    assert!(fs::symlink_metadata(root.join(".agents/skills")).is_err());
    let names: Vec<_> = fs::read_dir(root.join(".claude/skills"))
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(names, ["prd"]);
    assert_eq!(read(root, "skills/prd/SKILL.md"), "live\n");
    assert!(read(root, "CLAUDE.md").contains(HEADING));
}

#[test]
fn init_prints_the_commit_line_and_commits_nothing() {
    let dir = repo();
    let root = dir.path();
    let said = init(root);
    let version = env!("CARGO_PKG_VERSION");
    let commit = format!(
        "commit: git add docs .claude/skills .agents/skills CLAUDE.md AGENTS.md && git commit -m \"chore: factory init (skills {version})\""
    );
    let line = said
        .lines()
        .find(|l| l.starts_with("commit: "))
        .unwrap_or_else(|| panic!("no commit line in:\n{said}"));
    assert_eq!(line, commit);
    assert!(said.contains(&format!(
        "skills {version}: .claude/skills/ (20 files, factory.toml), .agents/skills/ (9 links)\n"
    )));
    assert!(said.contains("\nnext, in order: the gate (factory docs gate set \"<cmd>\", or /gate), the harness (/harness), the UX rubric (/ux rubric)\n"), "{said}");

    assert!(!git(root, &["rev-parse", "HEAD"]).status.success());
    let porcelain = stdout(&git(root, &["status", "--porcelain"]));
    let mut untracked: Vec<&str> = porcelain.lines().collect();
    untracked.sort_unstable();
    assert_eq!(
        untracked,
        [
            "?? .agents/",
            "?? .claude/",
            "?? AGENTS.md",
            "?? CLAUDE.md",
            "?? docs/"
        ]
    );

    let run = Command::new("sh")
        .args(["-c", line.trim_start_matches("commit: ")])
        .current_dir(root)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", "T")
        .env("GIT_AUTHOR_EMAIL", "t@example.invalid")
        .env("GIT_COMMITTER_NAME", "T")
        .env("GIT_COMMITTER_EMAIL", "t@example.invalid")
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", stderr(&run));
    assert_eq!(
        stdout(&git(root, &["log", "-1", "--format=%s"])).trim(),
        format!("chore: factory init (skills {version})")
    );
    assert_eq!(stdout(&git(root, &["status", "--porcelain"])), "");
}
