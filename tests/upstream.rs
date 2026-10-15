#[path = "harness/mod.rs"]
mod harness;

use std::fs;
use std::path::PathBuf;
use std::process::Output;

use harness::Fixture;

const BIN: &str = env!("CARGO_BIN_EXE_factory");
const CLEAN: &str =
    "# The gate forgets its cache\n\nA second run after a clean build reruns every tier.\n";

fn stdout(o: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}

fn draft(fx: &Fixture, text: &str) -> PathBuf {
    let p = fx.path().parent().unwrap().join("draft.md");
    fs::write(&p, text).unwrap();
    p
}

fn upstream(fx: &Fixture, verb: &str, draft: &PathBuf) -> Output {
    fx.command(BIN)
        .env_remove("FACTORY_BOARD")
        .envs([
            ("GIT_AUTHOR_NAME", "t"),
            ("GIT_AUTHOR_EMAIL", "t@t"),
            ("GIT_COMMITTER_NAME", "t"),
            ("GIT_COMMITTER_EMAIL", "t@t"),
        ])
        .args(["upstream", verb])
        .arg(draft)
        .output()
        .unwrap()
}

fn source_repo(fx: &Fixture) -> PathBuf {
    let src = fx.path().parent().unwrap().join("skills");
    fs::create_dir_all(&src).unwrap();
    let git = |args: &[&str]| {
        let out = fx
            .command("git")
            .current_dir(&src)
            .args(["-c", "user.name=t", "-c", "user.email=t@t"])
            .args(args)
            .output()
            .unwrap();
        assert!(out.status.success(), "{}", stdout(&out));
    };
    git(&["init", "-q"]);
    let init = fx
        .command(BIN)
        .current_dir(&src)
        .args(["docs", "init"])
        .output()
        .unwrap();
    assert!(init.status.success(), "{}", stdout(&init));
    git(&["add", "-A"]);
    git(&["commit", "-qm", "init"]);
    let cfg = PathBuf::from(&fx.env()["FACTORY_CONFIG_DIR"]).join("config.toml");
    fs::write(&cfg, format!("[source]\npath = \"{}\"\n", src.display())).unwrap();
    src
}

fn git_in(fx: &Fixture, dir: &PathBuf, args: &[&str]) -> String {
    let out = fx
        .command("git")
        .current_dir(dir)
        .args(args)
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", stdout(&out));
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn calls(fx: &Fixture, prefix: &str) -> usize {
    fx.gh_log().iter().filter(|l| l.contains(prefix)).count()
}

#[test]
fn check_names_a_home_path_and_searches_once() {
    let fx = Fixture::new("upstream-home");
    let d = draft(
        &fx,
        "# Slow gate\n\nThe log sits at /home/x/notes.txt today.\n",
    );
    let o = upstream(&fx, "check", &d);
    assert_eq!(o.status.code(), Some(1), "{}", stdout(&o));
    assert!(
        stdout(&o).contains(":3: a home path: /home/x/notes.txt"),
        "{}",
        stdout(&o)
    );
    assert_eq!(calls(&fx, "issue list"), 1, "{:?}", fx.gh_log());
}

#[test]
fn check_names_the_repo_name() {
    let fx = Fixture::new("upstream-name");
    let d = draft(&fx, "# Slow gate\n\nIn the Repo the gate is slow.\n");
    let o = upstream(&fx, "check", &d);
    assert_eq!(o.status.code(), Some(1), "{}", stdout(&o));
    assert!(
        stdout(&o).contains(":3: this repo's name: repo"),
        "{}",
        stdout(&o)
    );
    assert_eq!(calls(&fx, "issue list"), 1, "{:?}", fx.gh_log());
}

#[test]
fn a_failed_issue_search_is_one_line_and_not_a_hit() {
    let fx = Fixture::new("upstream-search-fails");
    fs::write(fx.path().join("scenario/gh-fail"), "issue list\n").unwrap();
    let d = draft(&fx, CLEAN);
    let o = upstream(&fx, "check", &d);
    assert_eq!(o.status.code(), Some(0), "{}", stdout(&o));
    assert!(
        stdout(&o).contains("issue search on emilianotisato/factory failed"),
        "{}",
        stdout(&o)
    );
}

#[test]
fn post_without_a_source_checkout_opens_an_issue() {
    let fx = Fixture::new("upstream-issue");
    let d = draft(&fx, CLEAN);
    let o = upstream(&fx, "post", &d);
    assert_eq!(o.status.code(), Some(0), "{}", stdout(&o));
    let created: Vec<String> = fx
        .gh_log()
        .into_iter()
        .filter(|l| l.contains("issue create"))
        .collect();
    assert_eq!(created.len(), 1, "{:?}", fx.gh_log());
    assert!(
        created[0].contains(
            r#"issue create --repo emilianotisato/factory --title "The gate forgets its cache""#
        ),
        "{created:?}"
    );
}

#[test]
fn post_with_a_source_checkout_commits_one_idea_and_opens_no_issue() {
    let fx = Fixture::new("upstream-idea");
    let src = source_repo(&fx);
    let before = git_in(&fx, &src, &["rev-parse", "HEAD"]);
    let d = draft(&fx, CLEAN);
    let o = upstream(&fx, "post", &d);
    assert_eq!(o.status.code(), Some(0), "{}", stdout(&o));
    let added = git_in(&fx, &src, &["diff", "--name-only", before.trim(), "HEAD"]);
    let added: Vec<&str> = added.lines().collect();
    assert_eq!(added.len(), 1, "{added:?}");
    assert!(
        added[0].starts_with("docs/ideas/") && added[0].ends_with("-the-gate-forgets-its-cache.md"),
        "{added:?}"
    );
    let idea = fs::read_to_string(src.join(added[0])).unwrap();
    assert!(idea.contains("origin: \"factory upstream\""), "{idea}");
    assert!(idea.contains("A second run after a clean build"), "{idea}");
    assert_eq!(git_in(&fx, &src, &["status", "--porcelain"]), "");
    assert_eq!(calls(&fx, "issue create"), 0, "{:?}", fx.gh_log());
}

#[test]
fn post_refuses_a_draft_with_a_hit() {
    let fx = Fixture::new("upstream-post-hit");
    let d = draft(&fx, "# Mail me\n\nWrite to jane@example.com.\n");
    let o = upstream(&fx, "post", &d);
    assert_eq!(o.status.code(), Some(1), "{}", stdout(&o));
    assert!(stdout(&o).contains("an email address: jane@example.com"));
    assert_eq!(calls(&fx, "issue create"), 0, "{:?}", fx.gh_log());
}

#[test]
fn inside_a_run_check_and_post_refuse_without_calling_gh() {
    let fx = Fixture::new("upstream-in-run");
    let d = draft(&fx, CLEAN);
    for verb in ["check", "post"] {
        let o = fx
            .command(BIN)
            .env("FACTORY_BOARD", fx.path())
            .args(["upstream", verb])
            .arg(&d)
            .output()
            .unwrap();
        assert_eq!(o.status.code(), Some(1), "{verb}: {}", stdout(&o));
        assert!(
            stdout(&o).contains("never runs inside a run"),
            "{}",
            stdout(&o)
        );
    }
    assert!(fx.gh_log().is_empty(), "{:?}", fx.gh_log());
}
