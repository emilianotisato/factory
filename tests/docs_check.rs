//! `factory docs check` and `factory docs doctor` (PRD-002 slice 03, FR-15).
#[path = "harness/mod.rs"]
mod harness;

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

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

fn git_in(dir: &Path, args: &[&str]) -> String {
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
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

/// `doctor` in `dir`: exit 0, the first line, and a tree it left as it was.
fn doctor_in(dir: &Path) -> String {
    let before = git_in(dir, &["status", "--porcelain", "--untracked-files=all"]);
    let out = docs_in(dir, &["doctor"]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}{}",
        stdout(&out),
        stderr(&out)
    );
    let after = git_in(dir, &["status", "--porcelain", "--untracked-files=all"]);
    assert_eq!(before, after, "doctor wrote to the tree");
    stdout(&out)
}

#[test]
fn doctor_names_the_framework_and_never_writes() {
    let fx = Fixture::v3("check-doctor-v3");
    let out = doctor_in(fx.path());
    assert!(out.starts_with("Framework      v3\n"), "{out}");
    assert!(out.contains("gate           ./gate.sh\n"), "{out}");
    assert!(out.contains("drift          clean\n"), "{out}");

    let fx = Fixture::v2("check-doctor-v2");
    let out = doctor_in(fx.path());
    assert!(
        out.starts_with("Framework      v2 (run factory docs migrate --dry)\n"),
        "{out}"
    );
    assert!(out.contains("\ndrift          "), "{out}");

    let v1 = tempfile::tempdir().unwrap();
    git_in(v1.path(), &["init", "-q"]);
    fs::create_dir(v1.path().join("docs")).unwrap();
    fs::write(v1.path().join("docs/index.md"), "# Board\n").unwrap();
    let out = doctor_in(v1.path());
    assert!(out.starts_with("Framework      v1\n"), "{out}");
    assert!(out.contains("\ndrift          "), "{out}");

    let none = tempfile::tempdir().unwrap();
    git_in(none.path(), &["init", "-q"]);
    assert_eq!(
        doctor_in(none.path()),
        "Framework      not on the framework\n"
    );
}

#[test]
fn check_exits_on_errors_not_warnings() {
    let fx = Fixture::v3("check-exits");
    let out = fx.factory_docs_output(&["check"]);
    assert_eq!(
        (out.status.code(), stdout(&out)),
        (Some(0), "docs check: clean\n".into()),
        "{}",
        stderr(&out)
    );

    fs::write(fx.path().join("docs/TODO.md"), "- a\n").unwrap();
    let out = fx.factory_docs_output(&["check"]);
    assert_eq!(out.status.code(), Some(0));
    assert!(
        stderr(&out).contains("! W-SECOND-BOARD"),
        "{}",
        stderr(&out)
    );
    assert!(
        stderr(&out).ends_with("0 errors, 1 warning\n"),
        "{}",
        stderr(&out)
    );
    fs::remove_file(fx.path().join("docs/TODO.md")).unwrap();

    let prd = fx.path().join(FEATURE_DIR).join("prd.md");
    let text = fs::read_to_string(&prd).unwrap();
    fs::write(&prd, text + "\n[gone](slice-09-missing.md)\n").unwrap();
    let out = fx.factory_docs_output(&["check"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(stderr(&out).contains("✗ E-LINK-BROKEN"), "{}", stderr(&out));

    for flag in ["--fix", "--strict"] {
        let out = fx.factory_docs_output(&["check", flag]);
        assert_eq!(out.status.code(), Some(2), "check {flag}");
    }
}

#[test]
fn a_v2_check_goes_to_docs_mjs_while_doctor_stays_native() {
    let fx = Fixture::v2("check-v2-route");
    fs::copy(
        fixture_src().join("stub-docs/docs.mjs"),
        fx.path().join(".docs/bin/docs.mjs"),
    )
    .unwrap();
    let out = fx.factory_docs_output(&["check"]);
    assert_eq!(
        (out.status.code(), stdout(&out)),
        (Some(3), "check\n".into())
    );
    let out = fx.factory_docs_output(&["doctor"]);
    assert_eq!(out.status.code(), Some(0));
    assert!(
        stdout(&out).starts_with("Framework      v2 (run factory docs migrate --dry)\n"),
        "{}",
        stdout(&out)
    );
    // The stub prints its arguments: doctor ran `check`, never `check --fix`.
    assert!(
        stdout(&out).contains("drift          docs.mjs check exited 3\ncheck\n"),
        "{}",
        stdout(&out)
    );
}

/// FR-18, slice 09 task 8: the v2 shim in `src/cli/docs.rs` is the one
/// `node` call site left in `src/` — one line, one file.
#[test]
fn node_is_spawned_in_one_place() {
    fn walk(dir: &Path, hits: &mut Vec<String>) {
        for e in fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(&p, hits);
            } else if p.extension().is_some_and(|x| x == "rs") {
                let text = fs::read_to_string(&p).unwrap();
                let n = text
                    .lines()
                    .filter(|l| l.contains("Command::new(\"node\")"))
                    .count();
                hits.extend(std::iter::repeat_n(p.display().to_string(), n));
            }
        }
    }
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut hits = Vec::new();
    walk(&src, &mut hits);
    let mut hits: Vec<String> = hits
        .into_iter()
        .map(|h| {
            h.trim_start_matches(&format!("{}/", src.display()))
                .to_string()
        })
        .collect();
    hits.sort();
    assert_eq!(hits, ["cli/docs.rs"]);
}
