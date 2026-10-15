#[path = "harness/mod.rs"]
mod harness;

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use factory::docs::{Status, state};
use harness::{FEATURE_DIR, Fixture, fixture_src};

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
    let fx = Fixture::new("v3-board");
    let porcelain = fx.git(&["status", "--porcelain"]);
    assert_eq!(
        porcelain, "",
        "the fixture tree is clean, no docs.lock listed"
    );

    let s: Status = serde_json::from_value(fx.docs_json("status")).unwrap();
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
    let fx = Fixture::new("v3-set");
    fx.docs(&["set", "001", "slices.3.status=building", "slices.3.lane=1"]);
    let s: Status = serde_json::from_value(fx.docs_json("status")).unwrap();
    assert_eq!(s.slices[2].status.as_deref(), Some("building"));
    assert_eq!(s.slices[2].lane, Some(1));
    assert_eq!(s.free_lanes, [2]);

    let before = fs::read_to_string(fx.path().join(FEATURE_DIR).join("state.toml")).unwrap();
    let out = fx.docs_output(&["set", "001", "status=later"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(
        stderr(&out).contains("idea | draft | ready | in-progress | shipped | archived | retired")
    );
    let after = fs::read_to_string(fx.path().join(FEATURE_DIR).join("state.toml")).unwrap();
    assert_eq!(before, after);
}

#[test]
fn the_board_never_reads_the_index_prose() {
    let fx = Fixture::new("v3-index");
    let summary = fx.docs(&["status"]);
    assert!(summary.contains("001  ready"), "{summary}");
    assert!(
        summary.contains("     Fixture PRD, approved for the harness run."),
        "{summary}"
    );
    let json = stdout(&fx.docs_output(&["status", "001", "--json"]));

    let index = fx.path().join("docs/index.md");
    let text = fs::read_to_string(&index).unwrap();
    fs::write(&index, format!("{}\n", text.lines().next().unwrap())).unwrap();
    assert_eq!(fx.docs(&["status"]), summary);
    assert_eq!(stdout(&fx.docs_output(&["status", "001", "--json"])), json);
}

#[test]
fn archive_folders_tell_retired_from_archived() {
    let fx = Fixture::new("v3-archive");
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
    let summary = fx.docs(&["status"]);
    assert!(summary.contains("archived: 2 · retired: 002"), "{summary}");
}

#[test]
fn slice_checks_its_dependencies() {
    let fx = Fixture::new("v3-slice");
    let out = fx.docs_output(&["slice", "001", "d", "--depends", "9"]);
    assert_eq!(out.status.code(), Some(2), "{}", stderr(&out));
    assert!(!fx.path().join(FEATURE_DIR).join("slice-04-d.md").exists());

    fx.docs(&["slice", "001", "d", "--depends", "2"]);
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
    let fx = Fixture::v2("v3-route");
    fs::copy(
        fixture_src().join("stub-docs/docs.mjs"),
        fx.path().join(".docs/bin/docs.mjs"),
    )
    .unwrap();
    let out = fx.docs_output(&["status", "001", "--json"]);
    assert_eq!(out.status.code(), Some(3));
    assert_eq!(stdout(&out), "status 001 --json\n");
    let out = fx.docs_output(&["render", "--write"]);
    assert_eq!(
        (out.status.code(), stdout(&out)),
        (Some(3), "render --write\n".into())
    );
}

#[test]
fn v3_repos_refuse_the_removed_verbs() {
    let fx = Fixture::new("v3-removed");
    let out = fx.docs_output(&["status", "001", "--json"]);
    assert_eq!(out.status.code(), Some(0));
    for args in [
        &["render", "--write"][..],
        &["log", "merge", "001"],
        &["vendor"],
    ] {
        let out = fx.docs_output(args);
        assert_eq!(out.status.code(), Some(2), "{args:?}");
        assert!(
            stderr(&out).contains("removed in v3"),
            "{args:?}: {}",
            stderr(&out)
        );
    }
    assert!(stderr(&fx.docs_output(&["render"])).contains("factory docs status"));
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
        "docs.lock\n__pycache__/\n"
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

const DOCS_VERBS: [&str; 19] = [
    "init",
    "new",
    "slice",
    "status",
    "preflight",
    "gate",
    "set",
    "log",
    "idea",
    "ship",
    "archive",
    "harvest",
    "renumber",
    "lanes",
    "check",
    "doctor",
    "hook",
    "migrate",
    "retro",
];

/// A wrapped help line (indented past two) fails here instead of reading as a verb.
fn help_verbs(help: &str) -> std::collections::BTreeSet<String> {
    help.lines()
        .skip_while(|l| !l.starts_with("Commands:"))
        .skip(1)
        .take_while(|l| !l.trim().is_empty())
        .map(|l| {
            assert!(
                l.starts_with("  ") && !l[2..].starts_with(' '),
                "not a verb line: {l:?}"
            );
            l.split_whitespace().next().unwrap().to_string()
        })
        .collect()
}

#[test]
fn help_lists_the_nineteen_verbs_in_and_out_of_a_repo() {
    let fx = Fixture::new("docs-help");
    let none = tempfile::tempdir().unwrap();
    let want: std::collections::BTreeSet<String> =
        DOCS_VERBS.iter().map(|v| v.to_string()).collect();
    for out in [
        fx.docs_output(&["--help"]),
        docs_in(none.path(), &["--help"]),
    ] {
        assert!(out.status.success(), "{}", stderr(&out));
        let help = stdout(&out);
        assert_eq!(help_verbs(&help), want, "in:\n{help}");
        for gone in ["render", "vendor", "merge"] {
            assert!(!help.contains(gone), "{gone} in:\n{help}");
        }
    }
}

const HOOK: &str = ".git/hooks/pre-commit";

#[test]
fn hook_installs_once_prints_under_hooks_path_and_refuses_a_foreign_file() {
    use std::os::unix::fs::PermissionsExt;

    let fx = Fixture::new("hook-install");
    let out = fx.docs_output(&["hook"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert_eq!(stdout(&out).trim(), format!("installed {HOOK}"));
    let file = fx.path().join(HOOK);
    let text = fs::read_to_string(&file).unwrap();
    assert!(
        text.starts_with("#!/bin/sh\n# feature-docs hook\n"),
        "{text}"
    );
    assert_eq!(
        fs::metadata(&file).unwrap().permissions().mode() & 0o777,
        0o755
    );
    assert_eq!(fx.docs(&["hook"]).trim(), "hook already installed");

    // With core.hooksPath set, git never runs .git/hooks.
    fs::remove_file(&file).unwrap();
    fx.git(&["config", "core.hooksPath", "/tmp/x"]);
    let printed = fx.docs(&["hook"]);
    assert!(printed.contains("core.hooksPath = /tmp/x"), "{printed}");
    assert!(
        printed.contains("Append this to /tmp/x/pre-commit"),
        "{printed}"
    );
    assert!(printed.contains("factory docs check"), "{printed}");
    assert!(!file.exists(), "nothing installed under a hooksPath");

    let out = fx.docs(&["hook", "--here"]);
    assert_eq!(out.trim(), format!("installed {HOOK}"));
    assert_eq!(
        fx.git(&["config", "--get", "core.hooksPath"]).trim(),
        ".git/hooks"
    );

    fx.git(&["config", "--unset", "core.hooksPath"]);
    let foreign = "#!/bin/sh\necho mine\n";
    fs::write(&file, foreign).unwrap();
    let out = fx.docs_output(&["hook"]);
    assert_eq!(out.status.code(), Some(2), "{}", stdout(&out));
    assert!(
        stderr(&out).contains("exists — append this by hand"),
        "{}",
        stderr(&out)
    );
    assert!(
        stderr(&out).contains("# feature-docs hook"),
        "{}",
        stderr(&out)
    );
    assert_eq!(fs::read_to_string(&file).unwrap(), foreign);
}

fn commit_with_path(fx: &Fixture, path: &str, msg: &str) -> Output {
    fx.command("git")
        .args(["commit", "-q", "-m", msg])
        .env("PATH", path)
        .output()
        .expect("run git commit")
}

#[test]
fn the_hook_refuses_an_archive_edit_and_a_broken_link_and_skips_without_factory() {
    let fx = Fixture::new("hook-body");
    let archived = fx.path().join("docs/archive/001-x/notes.md");
    fs::create_dir_all(archived.parent().unwrap()).unwrap();
    fs::write(&archived, "frozen\n").unwrap();
    fx.git(&["add", "docs/archive"]);
    fx.git(&["commit", "-q", "-m", "docs: archive a feature"]);
    fx.docs(&["hook"]);

    let bin_dir = Path::new(env!("CARGO_BIN_EXE_factory")).parent().unwrap();
    let with_factory = format!("{}:{}", bin_dir.display(), fx.env()["PATH"]);
    let head = || fx.git(&["rev-parse", "HEAD"]);
    let before = head();

    fs::write(&archived, "thawed\n").unwrap();
    fx.git(&["add", "docs/archive"]);
    let out = commit_with_path(&fx, &with_factory, "docs: edit the archive");
    assert_ne!(out.status.code(), Some(0));
    assert!(
        stderr(&out).contains("docs/archive/** is frozen"),
        "{}",
        stderr(&out)
    );
    assert_eq!(head(), before);
    fx.git(&["reset", "-q", "HEAD", "--", "docs/archive"]);
    fx.git(&["checkout", "--", "docs/archive"]);

    let prd = fx.path().join(FEATURE_DIR).join("prd.md");
    let mut text = fs::read_to_string(&prd).unwrap();
    text.push_str("\nSee [the missing file](missing.md).\n");
    fs::write(&prd, text).unwrap();
    fx.git(&["add", "docs"]);
    let out = commit_with_path(&fx, &with_factory, "docs: link a missing file");
    assert_ne!(out.status.code(), Some(0));
    let said = format!("{}{}", stdout(&out), stderr(&out));
    assert!(said.contains("E-LINK-BROKEN"), "{said}");
    assert_eq!(head(), before);

    let dirs: Vec<String> = ["git", "sh"]
        .iter()
        .map(|t| {
            let p = harness::find_in_path(t).unwrap_or_else(|| panic!("{t} on PATH"));
            p.parent().unwrap().display().to_string()
        })
        .collect();
    let bare = dirs.join(":");
    let probe = Command::new("sh")
        .args(["-c", "command -v factory"])
        .env("PATH", &bare)
        .output()
        .unwrap();
    assert!(
        !probe.status.success(),
        "a factory sits on {bare}: {}",
        stdout(&probe)
    );
    let out = commit_with_path(&fx, &bare, "docs: link a missing file");
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert_ne!(head(), before);
}

fn board_docs(fx: &Fixture, dir: &Path, board: &Path, args: &[&str], stdin: &str) -> Output {
    use std::io::Write as _;
    let mut child = fx
        .command(env!("CARGO_BIN_EXE_factory"))
        .arg("docs")
        .args(args)
        .current_dir(dir)
        .env(factory::worker::BOARD_ENV, board)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("run factory docs");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

fn bytes(p: &Path) -> Vec<u8> {
    fs::read(p).unwrap_or_default()
}

#[test]
fn a_lane_writes_the_main_trees_board_and_ignores_another_repos() {
    let fx = Fixture::new("board-lane");
    let wt = fx.path().with_file_name("wt");
    fx.git(&["worktree", "add", "-q", "--detach", wt.to_str().unwrap()]);
    let (main_state, wt_state) = (
        fx.path().join(FEATURE_DIR).join("state.toml"),
        wt.join(FEATURE_DIR).join("state.toml"),
    );
    let (main_log, wt_log) = (
        fx.path().join(FEATURE_DIR).join("log.jsonl"),
        wt.join(FEATURE_DIR).join("log.jsonl"),
    );
    let (wt_state_before, wt_log_before) = (bytes(&wt_state), bytes(&wt_log));
    let (main_state_before, main_log_before) = (bytes(&main_state), bytes(&main_log));

    let ok = |o: &Output| assert!(o.status.success(), "{}{}", stdout(o), stderr(o));
    let set = ["set", "001", "slices.1.status=building"];
    ok(&board_docs(&fx, &wt, fx.path(), &set, ""));
    assert_ne!(bytes(&main_state), main_state_before);
    assert_eq!(fx.slice_status(1), "building");
    assert_eq!(bytes(&wt_state), wt_state_before, "the lane's state.toml");

    let add = [
        "log",
        "add",
        "001",
        "--kind",
        "note",
        "--slice",
        "1",
        "--title",
        "t",
        "--body-file",
        "-",
    ];
    ok(&board_docs(&fx, &wt, fx.path(), &add, "from the lane\n"));
    let grown = bytes(&main_log);
    assert!(grown.starts_with(&main_log_before) && grown.len() > main_log_before.len());
    assert!(String::from_utf8_lossy(&grown).contains("from the lane"));
    assert_eq!(bytes(&wt_log), wt_log_before, "the lane's log.jsonl");

    let o = board_docs(&fx, &wt, fx.path(), &["status", "001", "--json"], "");
    ok(&o);
    let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
    assert!(v.to_string().contains("building"), "{v}");
    assert_eq!(fx.docs_json("status"), v);

    ok(&board_docs(
        &fx,
        &wt,
        fx.path(),
        &["retro", "001", "--set", "score=4"],
        "",
    ));
    let o = fx.docs_output(&["retro", "001"]);
    ok(&o);
    assert!(stdout(&o).contains("score: 4"), "{}", stdout(&o));

    let other = Fixture::new("board-other");
    let other_state = other.path().join(FEATURE_DIR).join("state.toml");
    let other_before = bytes(&other_state);
    let set = ["set", "001", "slices.2.status=building"];
    ok(&board_docs(&fx, &wt, other.path(), &set, ""));
    assert_eq!(bytes(&other_state), other_before, "the unrelated board");
    let lane = state::read(&wt.join(FEATURE_DIR)).unwrap().unwrap();
    assert_eq!(lane.slices[&2].status, "building", "the lane's own board");
    assert_ne!(fx.slice_status(2), "building");
}
