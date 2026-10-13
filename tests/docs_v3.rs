//! `factory docs` on the v3 layout and the v2 routing shim (PRD-002 slice 01).
#[path = "harness/mod.rs"]
mod harness;

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use factory::docs::{Status, state};
use harness::{FEATURE_DIR, Fixture, fixture_src};

/// `factory docs <args>` in `dir`, outside any fixture: no global git config.
fn docs_in(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_factory"))
        .arg("docs")
        .args(args)
        .current_dir(dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .expect("run factory docs")
}

fn git_in(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .args(["-c", "user.name=T", "-c", "user.email=t@example.invalid"])
        .args(args)
        .current_dir(dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .expect("run git");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

#[test]
fn the_v3_board_deserializes_into_status() {
    let fx = Fixture::v3("v3-board");
    let porcelain = fx.git(&["status", "--porcelain"]);
    assert_eq!(
        porcelain, "",
        "the fixture tree is clean, no docs.lock listed"
    );

    let s: Status = serde_json::from_value(fx.factory_docs_json("status")).unwrap();
    assert_eq!((s.number, s.slug.as_str()), (1, "fixture"));
    assert_eq!(s.status.as_deref(), Some("ready"));
    assert_eq!(s.slices.len(), 3);
    assert_eq!(s.slices[2].depends_on, [2]);
    assert_eq!(s.slices[0].file, "001-fixture/slice-01-a.md");
    assert_eq!(s.slices[0].title.as_deref(), Some("A"));
    assert_eq!(s.ready.len(), 2);
    assert_eq!(s.spawnable, 2);
    assert_eq!(s.lanes, 2);
    assert_eq!(s.free_lanes, [1, 2]);
    assert_eq!(s.next.as_ref().map(|n| n.number), Some(1));
    assert!(s.qa_plan);
    assert_eq!(s.branch.as_deref(), Some("main"));
    assert!(s.retro.is_null());
}

#[test]
fn set_rewrites_state_and_refuses_outside_the_vocabulary() {
    let fx = Fixture::v3("v3-set");
    fx.factory_docs(&["set", "001", "slices.3.status=building", "slices.3.lane=1"]);
    let s: Status = serde_json::from_value(fx.factory_docs_json("status")).unwrap();
    assert_eq!(s.slices[2].status.as_deref(), Some("building"));
    assert_eq!(s.slices[2].lane, Some(1));
    assert_eq!(s.free_lanes, [2]);

    let before = fs::read_to_string(fx.path().join(FEATURE_DIR).join("state.toml")).unwrap();
    let out = fx.factory_docs_output(&["set", "001", "status=later"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(
        stderr(&out).contains("idea | draft | ready | in-progress | shipped | archived | retired")
    );
    let after = fs::read_to_string(fx.path().join(FEATURE_DIR).join("state.toml")).unwrap();
    assert_eq!(before, after);
}

#[test]
fn the_board_never_reads_the_index_prose() {
    let fx = Fixture::v3("v3-index");
    let summary = fx.factory_docs(&["status"]);
    assert!(summary.contains("001  ready"), "{summary}");
    assert!(
        summary.contains("     Fixture PRD, approved for the harness run."),
        "{summary}"
    );
    let json = stdout(&fx.factory_docs_output(&["status", "001", "--json"]));

    let index = fx.path().join("docs/index.md");
    let text = fs::read_to_string(&index).unwrap();
    fs::write(&index, format!("{}\n", text.lines().next().unwrap())).unwrap();
    assert_eq!(fx.factory_docs(&["status"]), summary);
    assert_eq!(
        stdout(&fx.factory_docs_output(&["status", "001", "--json"])),
        json
    );
}

#[test]
fn archive_folders_tell_retired_from_archived() {
    let fx = Fixture::v3("v3-archive");
    let archive = fx.path().join("docs/archive");
    fs::create_dir_all(archive.join("002-gone")).unwrap();
    fs::write(
        archive.join("002-gone/state.toml"),
        "status = \"retired\"\n",
    )
    .unwrap();
    fs::create_dir_all(archive.join("003-old")).unwrap();
    fs::write(
        archive.join("003-old/prd.md"),
        "---\nnumber: 3\nslug: old\nstatus: archived\n---\n\n# PRD-003 — Old\n",
    )
    .unwrap();
    let summary = fx.factory_docs(&["status"]);
    assert!(summary.contains("archived: 2 · retired: 002"), "{summary}");
}

#[test]
fn slice_checks_its_dependencies() {
    let fx = Fixture::v3("v3-slice");
    let out = fx.factory_docs_output(&["slice", "001", "d", "--depends", "9"]);
    assert_eq!(out.status.code(), Some(2), "{}", stderr(&out));
    assert!(!fx.path().join(FEATURE_DIR).join("slice-04-d.md").exists());

    fx.factory_docs(&["slice", "001", "d", "--depends", "2"]);
    let file = fs::read_to_string(fx.path().join(FEATURE_DIR).join("slice-04-d.md")).unwrap();
    assert!(file.starts_with("# Slice 04 — D\n"), "{file}");
    assert!(
        file.contains("`./gate.sh --inner`"),
        "the Verify task names the inner loop"
    );
    let st = state::read(&fx.path().join(FEATURE_DIR)).unwrap().unwrap();
    assert_eq!(st.slices[&4].depends_on, [2]);
    assert_eq!(st.slices[&4].status, "pending");
    assert_eq!(st.slices[&3].depends_on, [2]);
}

#[test]
fn v2_repos_pass_through_to_docs_mjs() {
    let fx = Fixture::new("v3-route");
    fs::copy(
        fixture_src().join("stub-docs/docs.mjs"),
        fx.path().join(".docs/bin/docs.mjs"),
    )
    .unwrap();
    let out = fx.factory_docs_output(&["status", "001", "--json"]);
    assert_eq!(out.status.code(), Some(3));
    assert_eq!(stdout(&out), "status 001 --json\n");
    // v2 keeps the verbs v3 removed: the format decides, not the shim.
    let out = fx.factory_docs_output(&["render", "--write"]);
    assert_eq!(
        (out.status.code(), stdout(&out)),
        (Some(3), "render --write\n".into())
    );
}

#[test]
fn v3_repos_refuse_the_removed_verbs() {
    let fx = Fixture::v3("v3-removed");
    let out = fx.factory_docs_output(&["status", "001", "--json"]);
    assert_eq!(out.status.code(), Some(0));
    for args in [
        &["render", "--write"][..],
        &["log", "merge", "001"],
        &["vendor"],
    ] {
        let out = fx.factory_docs_output(args);
        assert_eq!(out.status.code(), Some(2), "{args:?}");
        assert!(
            stderr(&out).contains("removed in v3"),
            "{args:?}: {}",
            stderr(&out)
        );
    }
    assert!(stderr(&fx.factory_docs_output(&["render"])).contains("factory docs status"));
}

#[test]
fn a_repo_with_no_docs_is_off_the_framework_until_init() {
    let dir = tempfile::tempdir().unwrap();
    git_in(dir.path(), &["init", "-q", "-b", "main"]);
    let out = docs_in(dir.path(), &["status"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(
        stderr(&out).contains("not on the framework"),
        "{}",
        stderr(&out)
    );

    let out = docs_in(dir.path(), &["init"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    for p in [
        "docs/project.toml",
        "docs/index.md",
        "docs/features",
        "docs/ideas",
        "docs/archive",
        "docs/.gitignore",
    ] {
        assert!(dir.path().join(p).exists(), "{p}");
    }
    assert_eq!(
        fs::read_to_string(dir.path().join("docs/.gitignore")).unwrap(),
        "docs.lock\n"
    );
    let snapshot = |root: &Path| -> Vec<(String, String)> {
        ["project.toml", "index.md", ".gitignore"]
            .iter()
            .map(|f| {
                (
                    f.to_string(),
                    fs::read_to_string(root.join("docs").join(f)).unwrap(),
                )
            })
            .collect()
    };
    let before = snapshot(dir.path());
    let out = docs_in(dir.path(), &["init"]);
    assert_eq!(out.status.code(), Some(0));
    assert!(stdout(&out).contains("already initialised"));
    assert_eq!(snapshot(dir.path()), before);
}

#[test]
fn new_scaffolds_the_prd_and_draft_state_and_worktrees_never_share_a_number() {
    let dir = tempfile::tempdir().unwrap();
    let main = dir.path().join("main");
    fs::create_dir_all(&main).unwrap();
    git_in(&main, &["init", "-q", "-b", "main"]);
    assert!(docs_in(&main, &["init"]).status.success());
    git_in(&main, &["add", "-A"]);
    git_in(&main, &["commit", "-q", "-m", "init"]);
    let other = dir.path().join("other");
    git_in(
        &main,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "other",
            other.to_str().unwrap(),
        ],
    );

    let out = docs_in(&main, &["new", "fixture", "--title", "Fixture"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let out = docs_in(&other, &["new", "second"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(main.join("docs/features/001-fixture").is_dir());
    assert!(other.join("docs/features/002-second").is_dir());

    let prd = fs::read_to_string(main.join("docs/features/001-fixture/prd.md")).unwrap();
    let fm = factory::docs::frontmatter::parse(&prd);
    let keys: Vec<&str> = fm
        .data
        .as_ref()
        .unwrap()
        .iter()
        .map(|(k, _)| k.as_str())
        .collect();
    assert_eq!(
        keys,
        [
            "number", "slug", "title", "type", "created", "author", "summary"
        ]
    );
    assert_eq!(fm.str("title"), Some("Fixture"));
    let st = state::read(&main.join("docs/features/001-fixture"))
        .unwrap()
        .unwrap();
    assert_eq!(st.status, "draft");
    assert!(!main.join("docs/features/001-fixture/agent-log.md").exists());
}

#[test]
fn concurrent_sets_on_two_slices_never_lose_one() {
    let dir = tempfile::tempdir().unwrap();
    let feature = dir.path().to_path_buf();
    fs::write(
        feature.join("state.toml"),
        "status = \"in-progress\"\n\n[slices.1]\nstatus = \"pending\"\n\n[slices.2]\nstatus = \"pending\"\n",
    )
    .unwrap();
    let threads: Vec<_> = [1u32, 2]
        .into_iter()
        .map(|n| {
            let feature = feature.clone();
            std::thread::spawn(move || {
                let key = format!("slices.{n}.status");
                for i in 0..50 {
                    let v = if i % 2 == 0 { "done" } else { "building" };
                    state::set(&feature, &[(&key, v)]).unwrap();
                    let st = state::read(&feature).unwrap().unwrap();
                    assert_eq!(st.slices.len(), 2, "both slices survive every write");
                }
            })
        })
        .collect();
    for t in threads {
        t.join().unwrap();
    }
    let st = state::read(&feature).unwrap().unwrap();
    assert_eq!(st.slices[&1].status, "building");
    assert_eq!(st.slices[&2].status, "building");
}
