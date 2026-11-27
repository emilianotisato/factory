#[path = "harness/mod.rs"]
mod harness;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use harness::Fixture;

const BIN: &str = env!("CARGO_BIN_EXE_factory");

fn stdout(o: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}

fn install_gh_stub(dir: &Path) -> (PathBuf, PathBuf) {
    let bin = dir.join("bin");
    fs::create_dir_all(&bin).unwrap();
    let argv = dir.join("gh.argv");
    let stdin = dir.join("gh.stdin");
    let script = format!(
        r#"#!/bin/sh
echo "$*" >> "{}"
cat >> "{}"
case "$*" in
  *"issue list"*) printf '[]' ;;
  *) true ;;
esac
"#,
        argv.display(),
        stdin.display()
    );
    let gh = bin.join("gh");
    fs::write(&gh, script).unwrap();
    fs::set_permissions(&gh, fs::Permissions::from_mode(0o755)).unwrap();
    (argv, stdin)
}

fn path_with_gh(fx: &Fixture, gh_bin: &Path) -> String {
    format!(
        "{}:{}",
        gh_bin.parent().unwrap().display(),
        fx.env()["PATH"]
    )
}

fn factory_upstream(fx: &Fixture, path: &str, args: &[&str], draft: &PathBuf) -> Output {
    fx.command(BIN)
        .env("PATH", path)
        .env_remove("FACTORY_BOARD")
        .args(["upstream"])
        .args(args)
        .arg(draft)
        .output()
        .unwrap()
}

#[test]
fn upstream_hardening_host_stdin_remotes_variants_allow_and_hash() {
    let fx = Fixture::new("upstream-hardening");
    let scratch = fx.path().parent().unwrap().join("gh-stub");
    let (argv_log, stdin_log) = install_gh_stub(&scratch);
    let gh_path = path_with_gh(&fx, &scratch.join("bin").join("gh"));

    let clean = "# Hardening draft\n\nNothing sensitive here.\n";
    let draft_path = fx.path().parent().unwrap().join("hardening-draft.md");
    fs::write(&draft_path, clean).unwrap();

    let check = factory_upstream(&fx, &gh_path, &["check"], &draft_path);
    assert_eq!(check.status.code(), Some(0), "{}", stdout(&check));
    let check_out = stdout(&check);
    let hash = check_out
        .lines()
        .find_map(|l| l.strip_prefix("hash: "))
        .expect("hash on check");

    fs::write(&argv_log, "").unwrap();
    fs::write(&stdin_log, "").unwrap();
    let post = factory_upstream(&fx, &gh_path, &["post", "--hash", hash], &draft_path);
    assert_eq!(post.status.code(), Some(0), "{}", stdout(&post));
    let argv = fs::read_to_string(&argv_log).unwrap();
    assert!(argv.contains("github.com/emilianotisato/factory"));
    assert!(argv.contains("--body-file -"));
    assert!(!argv.contains("Nothing sensitive"));
    let body = fs::read_to_string(&stdin_log).unwrap();
    assert!(body.contains("Nothing sensitive here."));

    let src = fx.path().parent().unwrap().join("source");
    fs::create_dir_all(&src).unwrap();
    let git = |dir: &PathBuf, args: &[&str]| {
        let out = fx
            .command("git")
            .current_dir(dir)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .args(args)
            .output()
            .unwrap();
        assert!(out.status.success(), "{}", stdout(&out));
    };
    git(&src, &["init", "-q"]);
    git(
        &src,
        &[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "commit",
            "--allow-empty",
            "-qm",
            "init",
        ],
    );
    let cfg = PathBuf::from(&fx.env()["FACTORY_CONFIG_DIR"]).join("config.toml");
    fs::write(&cfg, format!("[source]\npath = \"{}\"\n", src.display())).unwrap();
    fs::write(&argv_log, "").unwrap();
    let post_src = factory_upstream(&fx, &gh_path, &["post", "--hash", hash], &draft_path);
    assert_eq!(post_src.status.code(), Some(0), "{}", stdout(&post_src));
    assert!(
        fs::read_to_string(&argv_log)
            .unwrap()
            .contains("issue create")
    );

    let variant_repo = fx.path().parent().unwrap().join("acme-client");
    fs::create_dir_all(&variant_repo).unwrap();
    git(&variant_repo, &["init", "-q"]);
    git(
        &variant_repo,
        &[
            "remote",
            "add",
            "upstream",
            "git@github.com:acme/acme-api.git",
        ],
    );
    let leak = "# Leak check\n\nWe ship AcmeApi today.\n";
    let leak_path = fx.path().parent().unwrap().join("leak-draft.md");
    fs::write(&leak_path, leak).unwrap();
    let o = fx
        .command(BIN)
        .env("PATH", &gh_path)
        .current_dir(&variant_repo)
        .env_remove("FACTORY_BOARD")
        .args(["upstream", "check"])
        .arg(&leak_path)
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(1), "{}", stdout(&o));
    assert!(stdout(&o).contains("this repo's name: acme-api"));

    fs::write(&argv_log, "").unwrap();
    let no_hash = factory_upstream(&fx, &gh_path, &["post"], &draft_path);
    assert_eq!(no_hash.status.code(), Some(1), "{}", stdout(&no_hash));
    assert!(stdout(&no_hash).contains("--hash missing"));
    let argv = fs::read_to_string(&argv_log).unwrap();
    assert!(!argv.contains("issue create"), "{argv}");

    fs::write(&draft_path, "# Hardening draft\n\nEdited body.\n").unwrap();
    fs::write(&argv_log, "").unwrap();
    let stale = factory_upstream(&fx, &gh_path, &["post", "--hash", hash], &draft_path);
    assert_eq!(stale.status.code(), Some(1), "{}", stdout(&stale));
    let stale_argv = fs::read_to_string(&argv_log).unwrap();
    assert!(!stale_argv.contains("issue create"), "{stale_argv}");

    let api_repo = fx.path().parent().unwrap().join("api-app");
    fs::create_dir_all(&api_repo).unwrap();
    git(&api_repo, &["init", "-q"]);
    git(
        &api_repo,
        &["remote", "add", "github", "git@github.com:acme/api.git"],
    );
    let api_draft = fx.path().parent().unwrap().join("api-draft.md");
    fs::write(
        &api_draft,
        "# Api note\n\nthe api layer\nrun from /home/x\nmail x@y.com\n",
    )
    .unwrap();
    let api_hit = fx
        .command(BIN)
        .env("PATH", &gh_path)
        .current_dir(&api_repo)
        .env_remove("FACTORY_BOARD")
        .args(["upstream", "check"])
        .arg(&api_draft)
        .output()
        .unwrap();
    assert_eq!(api_hit.status.code(), Some(1), "{}", stdout(&api_hit));
    assert!(stdout(&api_hit).contains("this repo's name: api"));
    let allow_check = fx
        .command(BIN)
        .env("PATH", &gh_path)
        .current_dir(&api_repo)
        .env_remove("FACTORY_BOARD")
        .args(["upstream", "check", "--allow", "api"])
        .arg(&api_draft)
        .output()
        .unwrap();
    assert_eq!(
        allow_check.status.code(),
        Some(1),
        "{}",
        stdout(&allow_check)
    );
    assert!(!stdout(&allow_check).contains("this repo's name: api"));
    assert!(stdout(&allow_check).contains("an email address"));
    let allow_out = stdout(&allow_check);
    let allow_hash = allow_out.lines().find_map(|l| l.strip_prefix("hash: "));
    assert!(allow_hash.is_none(), "email hit blocks hash");
    assert!(stdout(&allow_check).contains("a home path:"));

    fs::write(&api_draft, "# Api note\n\nno leaks\n").unwrap();
    let clean_api = fx
        .command(BIN)
        .env("PATH", &gh_path)
        .current_dir(&api_repo)
        .env_remove("FACTORY_BOARD")
        .args(["upstream", "check", "--allow", "api"])
        .arg(&api_draft)
        .output()
        .unwrap();
    assert_eq!(clean_api.status.code(), Some(0), "{}", stdout(&clean_api));
    let clean_out = stdout(&clean_api);
    let api_hash = clean_out
        .lines()
        .find_map(|l| l.strip_prefix("hash: "))
        .expect("hash");
    fs::write(&argv_log, "").unwrap();
    let allow_post = fx
        .command(BIN)
        .env("PATH", &gh_path)
        .current_dir(&api_repo)
        .env_remove("FACTORY_BOARD")
        .args(["upstream", "post", "--allow", "api", "--hash", api_hash])
        .arg(&api_draft)
        .output()
        .unwrap();
    assert_eq!(allow_post.status.code(), Some(0), "{}", stdout(&allow_post));
    assert!(stdout(&allow_post).contains("allowed: api"));
    let allow_argv = fs::read_to_string(&argv_log).unwrap();
    assert!(allow_argv.contains("issue create"), "{allow_argv}");

    let lines = ["acme_api here", "acme api wins", "just acme", "AcmeApi now"];
    for line in lines {
        let text = format!("# T\n\n{line}\n");
        fs::write(&leak_path, &text).unwrap();
        let o = fx
            .command(BIN)
            .env("PATH", &gh_path)
            .current_dir(&variant_repo)
            .env_remove("FACTORY_BOARD")
            .args(["upstream", "check"])
            .arg(&leak_path)
            .output()
            .unwrap();
        assert_eq!(o.status.code(), Some(1), "{line}: {}", stdout(&o));
        let hits = stdout(&o)
            .lines()
            .filter(|l| l.contains("this repo's name: acme-api"))
            .count();
        assert_eq!(hits, 1, "{line}: {}", stdout(&o));
    }
}

fn install_pull_gh_stub(dir: &Path) -> (PathBuf, PathBuf) {
    let bin = dir.join("bin");
    fs::create_dir_all(&bin).unwrap();
    let argv = dir.join("gh.argv");
    let stdin = dir.join("gh.stdin");
    let script = format!(
        r#"#!/bin/sh
echo "$*" >> "{}"
cat >> "{}"
case "$*" in
  *"issue list"*)
    printf '[{{"number":42,"title":"Track me","body":"Body forty-two","url":"https://github.com/emilianotisato/factory/issues/42","labels":[],"createdAt":"2026-11-27T00:00:00Z","author":{{"login":"alice"}}}},{{"number":43,"title":"Fold me","body":"Body forty-three","url":"https://github.com/emilianotisato/factory/issues/43","labels":[],"createdAt":"2026-11-27T00:00:00Z","author":{{"login":"bob"}}}},{{"number":44,"title":"Drop me","body":"Body forty-four","url":"https://github.com/emilianotisato/factory/issues/44","labels":[],"createdAt":"2026-11-27T00:00:00Z","author":{{"login":"carol"}}}}]'
    ;;
  *"issue view 42"*)
    printf '{{"number":42,"title":"Track me","body":"Body forty-two","url":"https://github.com/emilianotisato/factory/issues/42"}}'
    ;;
  *"issue view 43"*)
    printf '{{"number":43,"title":"Fold me","body":"Body forty-three","url":"https://github.com/emilianotisato/factory/issues/43"}}'
    ;;
  *"issue view 44"*)
    printf '{{"number":44,"title":"Drop me","body":"Body forty-four","url":"https://github.com/emilianotisato/factory/issues/44"}}'
    ;;
  *) true ;;
esac
"#,
        argv.display(),
        stdin.display()
    );
    let gh = bin.join("gh");
    fs::write(&gh, script).unwrap();
    fs::set_permissions(&gh, fs::Permissions::from_mode(0o755)).unwrap();
    (argv, stdin)
}

fn factory_pull(fx: &Fixture, path: &str, args: &[&str], extra_env: &[(&str, &str)]) -> Output {
    let mut cmd = Command::new(BIN);
    cmd.current_dir(fx.path())
        .args(["upstream", "pull"])
        .args(args);
    for (k, v) in fx.env() {
        if k != "PATH" {
            cmd.env(k, v);
        }
    }
    cmd.env("PATH", path);
    let mut board = false;
    for (k, v) in extra_env {
        if *k == "FACTORY_BOARD" {
            board = true;
        }
        cmd.env(k, v);
    }
    if !board {
        cmd.env_remove("FACTORY_BOARD");
    }
    cmd.output().unwrap()
}

#[test]
fn upstream_pull_lists_and_triages_issues_to_idea_fold_and_drop() {
    let fx = Fixture::new("upstream-pull");
    let scratch = fx.path().parent().unwrap().join("gh-pull-stub");
    let (argv_log, stdin_log) = install_pull_gh_stub(&scratch);
    let gh_path = path_with_gh(&fx, &scratch.join("bin").join("gh"));

    fs::write(&argv_log, "").unwrap();
    fs::write(&stdin_log, "").unwrap();
    let list = factory_pull(&fx, &gh_path, &[], &[]);
    assert_eq!(list.status.code(), Some(0), "{}", stdout(&list));
    let list_out = stdout(&list);
    assert!(list_out.contains("#42"));
    assert!(list_out.contains("github.com/emilianotisato/factory/issues/42"));
    let argv = fs::read_to_string(&argv_log).unwrap();
    assert!(argv.contains("github.com/emilianotisato/factory"));
    assert!(argv.contains("issue list"));

    fs::write(&argv_log, "").unwrap();
    let json = factory_pull(&fx, &gh_path, &["--json"], &[]);
    assert_eq!(json.status.code(), Some(0), "{}", stdout(&json));
    let parsed: serde_json::Value = serde_json::from_str(stdout(&json).trim()).unwrap();
    assert_eq!(parsed.as_array().unwrap().len(), 3);

    fs::write(&argv_log, "").unwrap();
    fs::write(&stdin_log, "").unwrap();
    let idea = factory_pull(&fx, &gh_path, &["--idea", "42"], &[]);
    assert_eq!(idea.status.code(), Some(0), "{}", stdout(&idea));
    let ideas_dir = fx.path().join("docs/ideas");
    let created = fs::read_dir(&ideas_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .find(|n| n.starts_with("20") && n.contains("track-me"))
        .expect("new idea file");
    let idea_text = fs::read_to_string(ideas_dir.join(&created)).unwrap();
    assert!(idea_text.contains("github.com/emilianotisato/factory/issues/42"));
    let status = fx
        .command("git")
        .current_dir(fx.path())
        .args(["status", "--porcelain"])
        .output()
        .unwrap();
    assert!(status.status.success());
    assert!(stdout(&status).trim().is_empty(), "{}", stdout(&status));
    let argv = fs::read_to_string(&argv_log).unwrap();
    assert!(argv.contains("issue comment 42"));
    assert!(argv.contains("--body-file -"));
    assert!(!argv.contains("Tracked as"));
    let body = fs::read_to_string(&stdin_log).unwrap();
    assert!(body.contains("Tracked as `docs/ideas/"));
    assert!(argv.contains("issue close 42"));
    assert!(argv.contains("completed"));

    let fold_id = "2026-01-01-seed-idea";
    let fold_rel = format!("docs/ideas/{fold_id}.md");
    fs::write(
        fx.path().join(&fold_rel),
        "---\ntitle: Seed\nstate: open\nadded: 2026-01-01\n---\n\nseed body\n",
    )
    .unwrap();
    fx.git(&["add", &fold_rel]);
    fx.git(&["commit", "-q", "-m", "chore: seed idea"]);
    fs::write(&argv_log, "").unwrap();
    fs::write(&stdin_log, "").unwrap();
    let fold = factory_pull(&fx, &gh_path, &["--fold", "43", fold_id], &[]);
    assert_eq!(fold.status.code(), Some(0), "{}", stdout(&fold));
    let folded = fs::read_to_string(fx.path().join(&fold_rel)).unwrap();
    assert!(folded.contains("## From #43"));
    assert!(folded.contains("Fold me"));
    assert!(folded.contains("Body forty-three"));
    let status = fx
        .command("git")
        .current_dir(fx.path())
        .args(["status", "--porcelain"])
        .output()
        .unwrap();
    assert!(stdout(&status).trim().is_empty(), "{}", stdout(&status));
    let argv = fs::read_to_string(&argv_log).unwrap();
    assert!(body_comment(&argv, &stdin_log, "43", "Folded into"));
    assert!(argv.contains("issue close 43"));

    fs::write(&argv_log, "").unwrap();
    fs::write(&stdin_log, "").unwrap();
    let head_before = fx
        .command("git")
        .current_dir(fx.path())
        .args(["rev-parse", "HEAD"])
        .output()
        .unwrap();
    assert!(head_before.status.success());
    let drop = factory_pull(
        &fx,
        &gh_path,
        &["--drop", "44", "--because", "duplicate"],
        &[],
    );
    assert_eq!(drop.status.code(), Some(0), "{}", stdout(&drop));
    let argv = fs::read_to_string(&argv_log).unwrap();
    assert!(body_comment(&argv, &stdin_log, "44", "Dropped: duplicate"));
    assert!(argv.contains("not planned"));
    let head_after = fx
        .command("git")
        .current_dir(fx.path())
        .args(["rev-parse", "HEAD"])
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8_lossy(&head_before.stdout).trim(),
        String::from_utf8_lossy(&head_after.stdout).trim(),
        "drop must not commit"
    );

    fs::write(&argv_log, "").unwrap();
    let in_run = factory_pull(&fx, &gh_path, &[], &[("FACTORY_BOARD", "016-001")]);
    assert_eq!(in_run.status.code(), Some(1), "{}", stdout(&in_run));
    assert!(stdout(&in_run).contains("never runs inside a run"));
    assert!(fs::read_to_string(&argv_log).unwrap().is_empty());

    let bare = fx.path().parent().unwrap().join("no-ideas-repo");
    fs::create_dir_all(&bare).unwrap();
    let git = |args: &[&str]| {
        let o = fx
            .command("git")
            .current_dir(&bare)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .args(args)
            .output()
            .unwrap();
        assert!(o.status.success(), "{}", stdout(&o));
    };
    git(&["init", "-q"]);
    git(&[
        "-c",
        "user.name=t",
        "-c",
        "user.email=t@t",
        "commit",
        "--allow-empty",
        "-qm",
        "init",
    ]);
    fs::write(&argv_log, "").unwrap();
    let no_ideas = fx
        .command(BIN)
        .current_dir(&bare)
        .env("PATH", &gh_path)
        .env_remove("FACTORY_BOARD")
        .args(["upstream", "pull", "--idea", "42"])
        .output()
        .unwrap();
    assert_eq!(no_ideas.status.code(), Some(1), "{}", stdout(&no_ideas));
    assert!(stdout(&no_ideas).contains("docs/ideas"));
    assert!(fs::read_to_string(&argv_log).unwrap().is_empty());
}

fn body_comment(argv: &str, stdin_log: &Path, number: &str, needle: &str) -> bool {
    argv.contains(&format!("issue comment {number}"))
        && argv.contains("--body-file -")
        && fs::read_to_string(stdin_log)
            .map(|b| b.contains(needle))
            .unwrap_or(false)
}
