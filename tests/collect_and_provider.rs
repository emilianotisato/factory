//! Reviewer-stamped collect reuse, proceed wording, preflight warnings, PATH-order detection, log titles.
#[path = "harness/mod.rs"]
mod harness;

use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use factory::collect::{self, Options};
use factory::config::{ResolvedReview, Review};
use factory::provider::{self, Provider, ReviewProvider};
use harness::{FEATURE_DIR, Fixture};

const BIN: &str = env!("CARGO_BIN_EXE_factory");
const SKILL: &str = include_str!("../skills/polish/SKILL.md");

fn git(dir: &Path, args: &[&str]) {
    let o = Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .unwrap();
    assert!(
        o.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&o.stderr)
    );
}

fn repo_with_second_opinion(tmp: &Path) -> PathBuf {
    let repo = tmp.join("repo");
    fs::create_dir_all(&repo).unwrap();
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["config", "user.email", "t@t"]);
    git(&repo, &["config", "user.name", "t"]);
    fs::write(repo.join("big.txt"), "start\n").unwrap();
    git(&repo, &["add", "big.txt"]);
    git(&repo, &["commit", "-qm", "base"]);
    git(&repo, &["checkout", "-qb", "work"]);
    fs::write(
        repo.join("big.txt"),
        (0..60).map(|i| format!("{i}\n")).collect::<String>(),
    )
    .unwrap();
    git(&repo, &["add", "big.txt"]);
    git(&repo, &["commit", "-qm", "change"]);
    repo
}

#[test]
fn collect_reuse_preflight_warnings_path_order_and_log_title_file() {
    let head = "abc123";
    let base = ResolvedReview::from(&Review::default());
    let tag = collect::reviewer_tag(&base);
    let good = format!(
        "<!-- head: {head} -->\n<!-- reviewer: {tag} -->\n\n## Plan\n\nsecond opinion: reviewed\n\n## Leaks\n\nverdict: clean\n"
    );
    assert!(collect::reusable_for(&good, head, &base));

    let other = ResolvedReview {
        provider: ReviewProvider::Provider(Provider::Claude),
        model: "opus".into(),
        binary: None,
        effort: None,
        timeout_secs: base.timeout_secs,
        pass_lines: base.pass_lines,
    };
    assert!(!collect::reusable_for(&good, head, &other));

    let other_model = ResolvedReview {
        model: "other-model".into(),
        ..base.clone()
    };
    assert!(!collect::reusable_for(&good, head, &other_model));

    let with_effort = ResolvedReview {
        effort: Some("high".into()),
        ..base.clone()
    };
    assert!(!collect::reusable_for(&good, head, &with_effort));

    let legacy = format!("<!-- head: {head} -->\n\n## Plan\n\n");
    assert!(!collect::reusable_for(&legacy, head, &base));

    let phrase = "skipped (proceed for this run)";
    assert_eq!(
        SKILL.matches(phrase).count(),
        1,
        "skills/polish/SKILL.md must mention proceed skip once"
    );

    let tmp = tempfile::tempdir().unwrap();
    let repo = repo_with_second_opinion(tmp.path());
    let cfg = ResolvedReview {
        provider: ReviewProvider::None,
        binary: None,
        model: String::new(),
        effort: None,
        timeout_secs: 1800,
        pass_lines: 600,
    };
    let mut progress = Vec::new();
    let mut opts = Options::new("main");
    opts.skipped_by_proceed = true;
    let text = collect::collect(&repo, &opts, &cfg, &mut progress, None).unwrap();
    assert!(text.contains("second opinion: skipped (proceed for this run)"));
    assert!(text.contains("skipped: second opinion skipped for this run (proceed)"));
    assert!(!text.contains("disabled in config"));

    opts.skipped_by_proceed = false;
    let text = collect::collect(&repo, &opts, &cfg, &mut progress, None).unwrap();
    assert!(text.contains("second opinion: skipped (disabled in config)"));

    let fx = Fixture::new("collect-preflight-warn");
    let cfg_dir = std::path::PathBuf::from(&fx.env()["FACTORY_CONFIG_DIR"]);
    fs::write(
        cfg_dir.join("config.toml"),
        "[review]\nproivder = \"claude\"\n",
    )
    .unwrap();
    let out = String::from_utf8_lossy(&fx.docs_output(&["preflight", "001"]).stdout).into_owned();
    assert!(
        out.lines().any(|l| l.starts_with("  ? config.toml:")),
        "{out}"
    );

    let dirs = tempfile::tempdir().unwrap();
    let early = dirs.path().join("early");
    let late = dirs.path().join("late");
    fs::create_dir_all(&early).unwrap();
    fs::create_dir_all(&late).unwrap();
    for (dir, name) in [(&early, "claude"), (&late, "codex")] {
        let p = dir.join(name);
        fs::write(&p, "#!/bin/sh\nexit 0\n").unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let path_env = std::env::join_paths([&early, &late]).unwrap();
    let found = provider::detect_installed_in(&path_env);
    assert!(found.len() >= 2, "{found:?}");
    assert_eq!(found[0].provider, Provider::Claude);
    assert!(found.iter().any(|d| d.provider == Provider::Codex));

    if unsafe { libc::geteuid() } != 0 {
        let trap_dir = dirs.path().join("trap");
        fs::create_dir(&trap_dir).unwrap();
        let p = trap_dir.join("claude");
        fs::write(&p, "x\n").unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(0o001)).unwrap();
        let only = std::env::join_paths([&trap_dir]).unwrap();
        assert!(provider::detect_installed_in(&only).is_empty());
    }

    let fx = Fixture::new("collect-log-title-file");
    let title_file = tempfile::NamedTempFile::new().unwrap();
    fs::write(title_file.path(), "fix `git push` shim\n").unwrap();
    let mut child = fx
        .command(BIN)
        .args([
            "docs",
            "log",
            "add",
            "001",
            "--kind",
            "decision",
            "--title-file",
            title_file.path().to_str().unwrap(),
            "--body-file",
            "-",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"body\n").unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let lines = fx.read(&format!("{FEATURE_DIR}/log.jsonl"));
    let last = lines.lines().next_back().expect("log line");
    assert!(
        last.contains(r#""title":"fix `git push` shim""#) || last.contains(r"fix `git push` shim"),
        "{last}"
    );

    let bad = fx
        .command(BIN)
        .args([
            "docs",
            "log",
            "add",
            "001",
            "--kind",
            "decision",
            "--title-file",
            "-",
            "--body-file",
            "-",
        ])
        .output()
        .unwrap();
    assert!(!bad.status.success());
    let err = String::from_utf8_lossy(&bad.stderr);
    assert!(
        err.contains("title-file") && err.contains("body-file"),
        "{err}"
    );
}
