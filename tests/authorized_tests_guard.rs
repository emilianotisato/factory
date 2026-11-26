#[path = "harness/mod.rs"]
mod harness;

use std::fs;
use std::path::Path;
use std::process::Command;

use factory::docs::Status;
use factory::land::commit_lane;
use factory::run::Run;
use factory::testguard::{self, BLOCK_PREFIX};
use harness::Fixture;

const AUTONOMY_LINE: &str = "skip, delete or weaken a test this run did not write — unless the PRD's frontmatter `authorized_tests` list (snapshotted at run start) names it";

fn data_dir(fx: &Fixture) -> std::path::PathBuf {
    std::path::PathBuf::from(&fx.env()["FACTORY_DATA_DIR"])
}

fn insert_prd_line(fx: &Fixture, line: &str) {
    let p = fx.path().join(harness::FEATURE_DIR).join("prd.md");
    let mut t = fs::read_to_string(&p).unwrap();
    let end = t.find("\n---\n").expect("frontmatter");
    t.insert_str(end, &format!("\n{line}"));
    fs::write(&p, t).unwrap();
}

#[test]
fn authorized_tests_snapshot_blocks_unlisted_test_edits_and_persists_allow_for_run() {
    let autonomy =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/autonomy.md"))
            .unwrap();
    assert!(autonomy.contains(AUTONOMY_LINE), "autonomy clause");

    let fx = Fixture::new("authorized-tests-guard");
    fs::create_dir_all(fx.path().join("tests")).unwrap();
    fs::write(
        fx.path().join("tests/existing_test.rs"),
        "#[test]\nfn sample_test() {\n    assert_eq!(1, 1);\n}\n",
    )
    .unwrap();
    fx.git(&["add", "tests/existing_test.rs"]);
    fx.git(&["commit", "-q", "-m", "seed existing test"]);

    insert_prd_line(&fx, "authorized_tests: [\"tests/knobs.rs\"]");
    fx.git(&["add", "-A"]);
    fx.git(&["commit", "-q", "-m", "prd authorized list"]);

    let base = fx.git(&["rev-parse", "HEAD"]).trim().to_string();
    fs::write(
        fx.path().join("tests/existing_test.rs"),
        "#[test]\nfn sample_test() {\n    assert_eq!(1, 2);\n}\n",
    )
    .unwrap();
    fx.git(&["add", "tests/existing_test.rs"]);
    fx.git(&["commit", "-q", "-m", "weaken existing test"]);

    let violations = testguard::check(fx.path(), &base, &[]).unwrap();
    assert_eq!(violations.len(), 1);
    assert_eq!(violations[0].path, "tests/existing_test.rs");

    let feature_dir = fx.path().join(harness::FEATURE_DIR);
    let mut run = Run::create(&data_dir(&fx), fx.path(), "001", None, 1).unwrap();
    run.set_authorized_tests(testguard::prd_authorized_tests(&feature_dir.join("prd.md")).unwrap())
        .unwrap();
    let state: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(run.dir().join("state.json")).unwrap()).unwrap();
    assert_eq!(
        state["authorized_tests"],
        serde_json::json!(["tests/knobs.rs"])
    );

    let body = testguard::format_block_body(&violations);
    let title = factory::answers::block_title(&format!("{} tests/existing_test.rs", BLOCK_PREFIX));
    let block_id =
        factory::answers::add_block(&feature_dir, None, Some("checkpoint"), &title, &body).unwrap();
    factory::answers::resolve(
        &feature_dir,
        &factory::answers::find(&feature_dir, &block_id)
            .unwrap()
            .unwrap(),
        "allow",
    )
    .unwrap();
    testguard::sync_allow_answers(&mut run, &feature_dir).unwrap();
    let state: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(run.dir().join("state.json")).unwrap()).unwrap();
    assert!(
        state["authorized_tests"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v == "tests/existing_test.rs"),
        "{state}"
    );
    assert!(
        testguard::check(fx.path(), &base, &run.state().authorized_tests)
            .unwrap()
            .is_empty()
    );

    fs::write(
        fx.path().join("tests/existing_test.rs"),
        "#[test]\nfn sample_test() {\n    assert_eq!(1, 1);\n}\n#[test]\nfn more() {}\n",
    )
    .unwrap();
    fx.git(&["add", "tests/existing_test.rs"]);
    fx.git(&["commit", "-q", "-m", "another change"]);
    let base2 = fx.git(&["rev-parse", "HEAD~1"]).trim().to_string();
    assert!(
        testguard::check(fx.path(), &base2, &run.state().authorized_tests)
            .unwrap()
            .is_empty(),
        "closeout-style check after allow"
    );

    insert_prd_line(&fx, "authorized_tests: [\"tests/existing_test.rs\"]");
    let listed = testguard::prd_authorized_tests(&feature_dir.join("prd.md")).unwrap();
    let base3 = fx.git(&["rev-parse", "HEAD"]).trim().to_string();
    fs::write(
        fx.path().join("tests/existing_test.rs"),
        "#[test]\nfn sample_test() {}\n",
    )
    .unwrap();
    fx.git(&["add", "tests/existing_test.rs"]);
    fx.git(&["commit", "-q", "-m", "listed weaken"]);
    assert!(
        testguard::check(fx.path(), &base3, &listed)
            .unwrap()
            .is_empty(),
        "listed path passes"
    );

    let board = Status {
        number: 1,
        dir: harness::FEATURE_DIR.to_string(),
        ..Default::default()
    };
    let main = fx.path();
    let lane = tempfile::tempdir().unwrap();
    let lane_path = lane.path();
    Command::new("git")
        .args([
            "-c",
            "user.name=T",
            "-c",
            "user.email=t@example.invalid",
            "worktree",
            "add",
            "-q",
            "--detach",
        ])
        .arg(lane_path)
        .current_dir(main)
        .status()
        .unwrap();
    let lane_base = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(lane_path)
        .output()
        .unwrap();
    let lane_base = String::from_utf8_lossy(&lane_base.stdout)
        .trim()
        .to_string();
    let prd = lane_path.join(harness::FEATURE_DIR).join("prd.md");
    let mut prd_text = fs::read_to_string(&prd).unwrap();
    let end = prd_text.find("\n---\n").unwrap();
    prd_text.insert_str(end, "\nauthorized_tests: [\"tests/tampered.rs\"]");
    fs::write(&prd, &prd_text).unwrap();
    fs::write(lane_path.join("polish.txt"), "x\n").unwrap();
    let board_files = vec![
        format!("{}/state.toml", board.dir),
        format!("{}/log.jsonl", board.dir),
    ];
    let mut protected = vec!["docs/index.md".to_string(), "docs/project.toml".to_string()];
    let lane_at = testguard::prd_authorized_tests(&prd).unwrap();
    let repo_at =
        testguard::prd_authorized_tests(&main.join(harness::FEATURE_DIR).join("prd.md")).unwrap();
    if lane_at != repo_at {
        protected.push(format!("{}/prd.md", board.dir));
    }
    let c = commit_lane(
        lane_path,
        &lane_base,
        &board_files,
        &protected,
        "polish",
        "refs/factory/001/polish",
    )
    .unwrap()
    .unwrap();
    assert!(c.dropped.iter().any(|p| p.ends_with("prd.md")));
    assert_eq!(
        testguard::prd_authorized_tests(&main.join(harness::FEATURE_DIR).join("prd.md")).unwrap(),
        repo_at
    );

    fx.docs(&["check"]);

    let empty = Run::create(&data_dir(&fx), fx.path(), "001", None, 1).unwrap();
    let raw = fs::read_to_string(empty.dir().join("state.json")).unwrap();
    assert!(!raw.contains("authorized_tests"));
    drop(empty);
    let mut with = Run::create(&data_dir(&fx), fx.path(), "001", None, 1).unwrap();
    with.set_authorized_tests(vec!["tests/a.rs".into()])
        .unwrap();
    with.add_authorized_tests(&["tests/a.rs".into(), "tests/b.rs".into()])
        .unwrap();
    let raw = fs::read_to_string(with.dir().join("state.json")).unwrap();
    let state: serde_json::Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(
        state["authorized_tests"],
        serde_json::json!(["tests/a.rs", "tests/b.rs"])
    );
}
