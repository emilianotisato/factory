#[path = "harness/mod.rs"]
mod harness;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use factory::cli::lanes::detect_shell_clobber;
use factory::docs::{Docs, Status};
use factory::lanes::{self, Lane};
use factory::pack::{self, Pack};
use harness::Fixture;

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
