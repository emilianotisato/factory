#[path = "harness/mod.rs"]
mod harness;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Output;

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
    assert!(argv.contains("--body-file"));
    assert!(argv.contains("-"));
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
    let stale = factory_upstream(&fx, &gh_path, &["post", "--hash", hash], &draft_path);
    assert_eq!(stale.status.code(), Some(1), "{}", stdout(&stale));

    let api_repo = fx.path().parent().unwrap().join("api-app");
    fs::create_dir_all(&api_repo).unwrap();
    git(&api_repo, &["init", "-q"]);
    git(
        &api_repo,
        &["remote", "add", "github", "git@github.com:acme/api.git"],
    );
    let api_draft = fx.path().parent().unwrap().join("api-draft.md");
    fs::write(&api_draft, "# Api note\n\nthe api layer\nmail x@y.com\n").unwrap();
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
