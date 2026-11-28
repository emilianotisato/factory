#[path = "harness/mod.rs"]
mod harness;
#[path = "common/passage.rs"]
mod passage;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use factory::cli::lanes::detect_shell_clobber;
use factory::closeout::local_ci_note_body;
use factory::docs::{Docs, Status};
use factory::lanes::{self, Lane};
use factory::pack::{self, Pack, handed_qa};
use harness::FEATURE_DIR;
use harness::Fixture;
use passage::assert_unique_passage;
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_factory");

fn stub_shell(dir: &std::path::Path, name: &str, body: &str) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, body).unwrap();
    let mut perms = fs::metadata(&path).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&path, perms).unwrap();
    path
}

#[test]
fn lane_env_resolves_target_dir_prefixes_pack_and_warns_on_shell_clobber() {
    let tmp = tempfile::tempdir().unwrap();
    let lane_dir = tmp.path().join("lane-2");
    fs::create_dir_all(lane_dir.join("target/lane-2")).unwrap();
    let lane = Lane {
        n: 2,
        env: vec![("CARGO_TARGET_DIR".to_string(), "target/lane-2".to_string())],
        ..Lane::default()
    };
    let resolved = lane.env_for(&lane_dir);
    let cargo = resolved
        .iter()
        .find(|(k, _)| k == "CARGO_TARGET_DIR")
        .map(|(_, v)| v.clone())
        .unwrap();
    let want = std::path::absolute(lane_dir.join("target/lane-2"))
        .unwrap()
        .to_string_lossy()
        .into_owned();
    assert_eq!(cargo, want);

    let prefixed = pack::env_prefixed(&resolved, "cargo fmt && cargo test -q");
    assert!(prefixed.contains("export CARGO_TARGET_DIR="));
    assert!(prefixed.contains("cargo fmt && cargo test -q"));

    let repo = tmp.path().join("repo");
    let dir = "docs/features/001-x";
    fs::create_dir_all(repo.join(dir)).unwrap();
    fs::write(repo.join("gate.sh"), "").unwrap();
    fs::write(
        repo.join("docs/project.toml"),
        "[docs]\nframework = 3\ngate = \"./gate.sh\"\ninner = \"true\"\n",
    )
    .unwrap();
    fs::write(
        repo.join(dir).join("prd.md"),
        "# PRD\n\n## 1. FR-1\n\nrule\n",
    )
    .unwrap();
    fs::write(
        repo.join(dir).join("slice-01-a.md"),
        "# Slice 01\n\n**Source**: §1\n",
    )
    .unwrap();
    let docs = Docs::open(&repo).unwrap();
    let board = Status {
        number: 1,
        dir: dir.to_string(),
        slices: vec![factory::docs::Slice {
            number: 1,
            file: "001-x/slice-01-a.md".to_string(),
            ..Default::default()
        }],
        ..Default::default()
    };
    let packs = repo.join("packs");
    let gate_log = repo.join("gate.log");
    fs::write(&gate_log, "ok\n").unwrap();
    let cp = Pack::for_checkpoint_env(
        &docs,
        &board,
        1,
        &[1],
        &gate_log,
        false,
        "clean\n",
        &packs,
        1,
        None,
        &resolved,
    )
    .unwrap();
    let cp_text = fs::read_to_string(cp.path).unwrap();
    assert!(cp_text.contains("# Lane env"));
    assert!(cp_text.contains("export CARGO_TARGET_DIR="));

    let stubs = tmp.path();
    let probe_env = vec![
        (lanes::ENV_LANE.to_string(), "2".to_string()),
        ("CARGO_TARGET_DIR".to_string(), want.clone()),
    ];
    let clobber = stub_shell(
        stubs,
        "clobber-stub",
        "#!/bin/sh\nif [ \"$1\" = \"-lic\" ]; then export CARGO_TARGET_DIR=/host/wrong; shift; sh -c \"$1\"; fi\n",
    );
    let got = detect_shell_clobber(&clobber, &probe_env, true);
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].0, "CARGO_TARGET_DIR");
    assert_eq!(got[0].1, want);
    assert_eq!(got[0].2, "/host/wrong");

    let preserve = stub_shell(
        stubs,
        "preserve-stub",
        "#!/bin/sh\nif [ \"$1\" = \"-lic\" ]; then shift; sh -c \"$1\"; fi\n",
    );
    assert!(detect_shell_clobber(&preserve, &probe_env, true).is_empty());

    let hang = stub_shell(
        stubs,
        "hang-stub",
        "#!/bin/sh\nif [ \"$1\" = \"-lic\" ]; then sleep 10; fi\n",
    );
    let start = Instant::now();
    assert!(detect_shell_clobber(&hang, &probe_env, true).is_empty());
    assert!(start.elapsed() < Duration::from_millis(3500));

    let fx = Fixture::new("delivery-hardening-prove-shell");
    let root = fx.path();
    let lanes_dir = root.join("docs/reference/lanes");
    fs::create_dir_all(&lanes_dir).unwrap();
    fs::write(
        lanes_dir.join("lane-1.md"),
        "---\nlane: 1\nenv: [CARGO_TARGET_DIR=target/lane-1]\nsetup:\n---\n",
    )
    .unwrap();
    fx.git(&["add", "docs/reference/lanes/lane-1.md"]);
    fx.git(&["commit", "-q", "-m", "chore: lane-1 cargo target"]);
    let prove_stub = stub_shell(
        fx.stub_dir(),
        "prove-clobber-stub",
        "#!/bin/sh\nif [ \"$1\" = \"-lic\" ]; then export CARGO_TARGET_DIR=/host/wrong; shift; sh -c \"$1\"; fi\n",
    );
    let out = fx
        .command(BIN)
        .args(["lanes", "up", "--prove"])
        .env("FACTORY_PROVE_SHELL", prove_stub.display().to_string())
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert_eq!(out.status.code(), Some(0), "{text}");
    assert!(
        text.contains("! shell") && text.contains("overwrites CARGO_TARGET_DIR"),
        "{text}"
    );
}

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn write_fx(fx: &Fixture, rel: &str, text: &str) {
    let p = fx.path().join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, text).unwrap();
}

fn commit_fx(fx: &Fixture, msg: &str) {
    fx.git(&["add", "-A"]);
    fx.git(&["commit", "-q", "-m", msg]);
}

fn docs_fx(fx: &Fixture, args: &[&str]) -> (Option<i32>, String, String) {
    let out = fx.command(BIN).arg("docs").args(args).output().unwrap();
    (
        out.status.code(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

fn ctx_fx(fx: &Fixture, args: &[&str]) -> (Option<i32>, String) {
    let out = fx.command(BIN).arg("ctx").args(args).output().unwrap();
    (
        out.status.code(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
    )
}

fn leaks_fx(fx: &Fixture, args: &[&str]) -> (Option<i32>, String) {
    let out = fx.command(BIN).arg("leaks").args(args).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let err = String::from_utf8_lossy(&out.stderr);
    (out.status.code(), format!("{text}{err}"))
}

#[test]
fn checkpoint_defers_pending_slice_frs_and_bundle_carries_prove_rules() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path();
    let dir = "docs/features/001-x";
    fs::create_dir_all(repo.join(dir)).unwrap();
    let prd = "# PRD\n\n| FR | Proof | Evidence | Human? |\n|---|---|---|---|\n\
        | FR-3 | run t | transcript | no |\n| FR-4 | run u | transcript | no |\n";
    fs::write(repo.join(dir).join("prd.md"), prd).unwrap();
    fs::write(
        repo.join(dir).join("slice-02.md"),
        "# S\n\n### 3. Prove\n§8, the FR-3 and FR-4 rows.\n",
    )
    .unwrap();
    fs::write(
        repo.join(dir).join("slice-03.md"),
        "# S\n\n**Source**: §6 FR-2\n**Depends on**: Slice 02 (so FR-4 lands)\n",
    )
    .unwrap();
    fs::write(
        repo.join(dir).join("slice-04.md"),
        "# S\n\n### 3. Prove\n§8, the FR-3 row.\n",
    )
    .unwrap();
    let board = Status {
        dir: dir.to_string(),
        all_done: false,
        slices: vec![
            factory::docs::Slice {
                number: 2,
                file: "001-x/slice-02.md".into(),
                status: Some("done".into()),
                ..Default::default()
            },
            factory::docs::Slice {
                number: 3,
                file: "001-x/slice-03.md".into(),
                status: Some("pending".into()),
                ..Default::default()
            },
            factory::docs::Slice {
                number: 4,
                file: "001-x/slice-04.md".into(),
                status: Some("building".into()),
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    let (text, rows) = handed_qa(repo, &board, &[2], prd).unwrap();
    assert_eq!(
        rows.iter().map(|r| r.key.as_str()).collect::<Vec<_>>(),
        ["FR-4"]
    );
    assert!(
        text.contains("deferred until remaining slice(s) finish: FR-3"),
        "{text}"
    );

    let board_done = Status {
        all_done: true,
        ..board
    };
    let (_, rows) = handed_qa(repo, &board_done, &[2], prd).unwrap();
    assert_eq!(
        rows.iter().map(|r| r.key.as_str()).collect::<Vec<_>>(),
        ["FR-3", "FR-4"]
    );

    let slice = fs::read_to_string(crate_root().join("skills/slice/SKILL.md")).unwrap();
    assert_unique_passage(
        "slice/SKILL.md narrow form",
        &slice,
        "Each form a slice adds gets one empty submit and one narrow-screen drive (375 px).",
    );
    assert_unique_passage(
        "slice/SKILL.md option seed",
        &slice,
        "A Prove row that picks between options seeds a state where the options give different results.",
    );
    let checkpoint =
        fs::read_to_string(crate_root().join("assets/contract-checkpoint.md")).unwrap();
    assert_unique_passage(
        "contract-checkpoint gate log",
        &checkpoint,
        "you may cite that test from the **full** gate log file named in `# Gate transcript`'s header comment — not only the 200-line tail in the pack",
    );
}

#[test]
fn docs_check_feature_ctx_def_generated_leaks_attributes_and_local_pr_note() {
    let fx = Fixture::new("delivery-hardening-fr4");
    let root = fx.path();
    let second = root.join("docs/features/002-second");
    fs::create_dir_all(&second).unwrap();
    fs::write(
        second.join("prd.md"),
        "---\nnumber: 2\nslug: second\ntitle: Second\ntype: feature\ncreated: 2026-01-01\nauthor: t\nsummary: |\n  x\n---\n",
    )
    .unwrap();
    fs::write(second.join("state.toml"), "status = \n").unwrap();
    commit_fx(&fx, "second feature");
    write_fx(&fx, "docs/ideas/bad name.md", "# bad\n");

    let (code, out, err) = docs_fx(&fx, &["check", "001"]);
    assert_eq!(code, Some(0), "{out}{err}");
    assert_eq!(out.trim(), "docs check: clean");

    let (code, _, err) = docs_fx(&fx, &["check"]);
    assert_eq!(code, Some(1), "{err}");
    assert!(err.contains("E-STATE-INVALID"), "{err}");
    assert!(err.contains("E-IDEA-NAME"), "{err}");

    write_fx(
        &fx,
        ".gitattributes",
        "src/hidden_gen.rs linguist-generated\n",
    );
    write_fx(&fx, "src/live.rs", "pub fn delivery_hardening_def() {}\n");
    write_fx(
        &fx,
        "src/hidden_gen.rs",
        "pub fn delivery_hardening_def() {}\n",
    );
    commit_fx(&fx, "sources");

    let (_, ctx_out) = ctx_fx(&fx, &["def", "delivery_hardening_def"]);
    assert!(ctx_out.contains("src/live.rs"), "{ctx_out}");
    assert!(!ctx_out.contains("hidden_gen.rs"), "{ctx_out}");

    write_fx(&fx, ".gitattributes", "src/gen.rs linguist-generated\n");
    write_fx(
        &fx,
        "src/gen.rs",
        "// main-attrs-only: on main attrs only\n",
    );
    commit_fx(&fx, "main: attributes");

    let attrs = fx.path().join("local-attrs");
    fs::write(&attrs, "src/leak.rs linguist-generated\n").unwrap();
    fx.git(&["config", "core.attributesFile", attrs.to_str().unwrap()]);

    fx.git(&["checkout", "-q", "-b", "feat/leak"]);
    write_fx(
        &fx,
        "src/leak.rs",
        "const _ = \"FR-99: must stay visible\";\n",
    );
    commit_fx(&fx, "branch leak");

    let (code, leaks_out) = leaks_fx(&fx, &["--base", "main"]);
    assert_eq!(code, Some(1), "{leaks_out}");
    assert!(leaks_out.contains("src/leak.rs"), "{leaks_out}");

    let body = "## PRD-031 — x\n\nBody.\n";
    let noted = local_ci_note_body(body);
    assert!(
        noted.contains("squash-merge commit message on merge"),
        "{noted}"
    );
    assert!(!noted.contains("[skip ci]"), "{noted}");
    assert_eq!(local_ci_note_body(&noted), noted);

    let deliver = fs::read_to_string(crate_root().join("skills/deliver/SKILL.md")).unwrap();
    assert_unique_passage(
        "deliver/SKILL.md squash merge",
        &deliver,
        "Also strip any skip-CI lines from GitHub's default squash-merge commit message so `main` workflows still run on merge.",
    );
    assert!(
        fs::read_to_string(crate_root().join("mods/cockpit/hooks/register.tsx"))
            .unwrap()
            .contains("0.20.0")
    );
}

#[test]
fn docs_set_clears_stale_reviewed_head_resolves_head_and_log_add_explains_main_checkout() {
    let fx = Fixture::new("delivery-fr3-reviewed");
    let main = fx.path();

    fx.docs(&["set", "001", "reviewed=2026-11-26", "reviewed_head=abc1234"]);
    fx.docs(&["set", "001", "reviewed=2026-11-27"]);
    let state = fs::read_to_string(main.join(FEATURE_DIR).join("state.toml")).unwrap();
    assert!(state.contains("reviewed = \"2026-11-27\""));
    assert!(!state.contains("reviewed_head"));
    fx.git(&["add", "-A"]);
    fx.git(&["commit", "-q", "-m", "docs: clear stale reviewed_head"]);
    let cleared = fx.git(&["rev-parse", "HEAD"]).trim().to_string();
    assert_eq!(factory::settled::last_reviewed(main, "001"), Some(cleared));

    fx.docs(&["set", "001", "reviewed_head=beef123", "reviewed=2026-11-28"]);
    let pinned = fs::read_to_string(main.join(FEATURE_DIR).join("state.toml")).unwrap();
    assert!(pinned.contains("reviewed_head = \"beef123\""));

    fx.git(&["checkout", "-q", "-b", "feat/lane-tip"]);
    write_fx(&fx, "lane-only.txt", "lane\n");
    commit_fx(&fx, "feat: lane-only");
    let lane_sha = fx.git(&["rev-parse", "HEAD"]).trim().to_string();

    fx.git(&["checkout", "-q", "main"]);
    write_fx(&fx, "main-only.txt", "main\n");
    commit_fx(&fx, "chore: main-only");
    let main_sha = fx.git(&["rev-parse", "HEAD"]).trim().to_string();
    assert_ne!(lane_sha, main_sha);

    let lane_dir = fx.stub_dir().join("lane-wt");
    fx.git(&[
        "worktree",
        "add",
        "-q",
        lane_dir.to_str().unwrap(),
        "feat/lane-tip",
    ]);
    let out = fx
        .command(BIN)
        .current_dir(&lane_dir)
        .env("FACTORY_BOARD", main)
        .args(["docs", "set", "001", "reviewed_head=HEAD"])
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let state = fs::read_to_string(main.join(FEATURE_DIR).join("state.toml")).unwrap();
    assert!(
        state.contains(&format!("reviewed_head = \"{lane_sha}\"")),
        "{state}"
    );

    let (code, _, err) = {
        let out = fx
            .command(BIN)
            .current_dir(&lane_dir)
            .env("FACTORY_BOARD", main)
            .args([
                "docs",
                "log",
                "add",
                "001",
                "--kind",
                "note",
                "--title",
                "lane note",
                "--body-file",
                "-",
            ])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let mut child = out;
        use std::io::Write;
        child
            .stdin
            .take()
            .unwrap()
            .write_all(b"lane body\n")
            .unwrap();
        let out = child.wait_with_output().unwrap();
        (
            out.status.code(),
            String::from_utf8_lossy(&out.stdout).into_owned(),
            String::from_utf8_lossy(&out.stderr).into_owned(),
        )
    };
    assert_eq!(code, Some(0), "{err}");
    assert!(
        err.contains(
            "main checkout; the driver commits the log, so this lane's git status stays clean"
        ),
        "{err}"
    );
    let lane_porcelain = Command::new("git")
        .current_dir(&lane_dir)
        .args(["status", "--porcelain"])
        .output()
        .unwrap();
    assert!(
        lane_porcelain.stdout.is_empty(),
        "{}",
        String::from_utf8_lossy(&lane_porcelain.stdout)
    );

    let slice_contract = fs::read_to_string(crate_root().join("assets/contract-slice.md")).unwrap();
    let polish_contract =
        fs::read_to_string(crate_root().join("assets/contract-polish.md")).unwrap();
    let note = "write to the main checkout (the driver commits them, so this lane's `git status` stays clean)";
    assert_eq!(slice_contract.matches(note).count(), 1);
    assert_eq!(polish_contract.matches(note).count(), 1);
}
