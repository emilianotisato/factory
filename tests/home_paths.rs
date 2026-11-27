#[path = "harness/mod.rs"]
mod harness;

use std::fs;
use std::path::PathBuf;
use std::process::Output;

use factory::paths::{find_home_path, home_paths_to_tilde};
use harness::Fixture;

const BIN: &str = env!("CARGO_BIN_EXE_factory");

fn both(o: &Output) -> String {
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

#[test]
fn home_paths_catch_and_rewrite_no_trailing_slash_wsl_windows_root_and_json() {
    assert_eq!(find_home_path(b"/home/x "), Some(0));
    assert_eq!(find_home_path(b"at EOL /home/x"), Some(7));
    assert_eq!(find_home_path(b"in \"/home/x\""), Some(4));
    assert_eq!(find_home_path(b"cd /home/a /home/b"), Some(3));
    assert_eq!(find_home_path(b"/users/ana/d"), Some(0));
    assert_eq!(find_home_path(b"/mnt/c/Users/jdoe/x"), Some(0));
    assert_eq!(find_home_path(br"C:\Users\jdoe\x"), Some(0));
    assert_eq!(find_home_path(b"/root/.bashrc"), Some(0));
    assert_eq!(find_home_path(br#"\"\/home\/x\/n\""#), Some(2));

    assert_eq!(find_home_path(b"/home/"), None);
    assert_eq!(find_home_path(b"/Users"), None);
    assert_eq!(find_home_path(b"/tmp/home/x/"), None);
    assert_eq!(find_home_path(b"~/home/x/"), None);
    assert_eq!(find_home_path(b"https://example.com/root/a"), None);

    assert_eq!(home_paths_to_tilde("cd /home/a /home/b"), "cd ~ ~");
    assert_eq!(
        home_paths_to_tilde("/users/x,/mnt/c/users/y,C:\\users\\z"),
        "~,~,~"
    );
    assert_eq!(home_paths_to_tilde("/root/a"), "~/a");
    assert_eq!(home_paths_to_tilde(r#"\/home\/x \/home\/y"#), "~ ~");

    let fx = Fixture::new("home-paths-shapes");
    let slice_file = fx.slice_path(1);
    let slice = slice_file
        .strip_prefix(fx.path())
        .unwrap()
        .to_str()
        .unwrap();
    let mut body = fs::read_to_string(&slice_file).unwrap();
    body.push_str("\nrun from /home/x here\n");
    let line = body.lines().count();
    fs::write(&slice_file, &body).unwrap();
    fx.git(&["add", "-A"]);
    fx.git(&["commit", "-qm", "doc with bare home"]);
    let check = fx.docs_output(&["check"]);
    let out = both(&check);
    assert!(
        out.contains("W-HOME-PATH"),
        "expected W-HOME-PATH in:\n{out}"
    );
    assert!(
        out.contains(&format!("{slice}:{line}")),
        "expected line ref in:\n{out}"
    );

    let d = draft(&fx, "# Leak\n\nA note at /home/x before the period.\n");
    let upstream = fx
        .command(BIN)
        .env_remove("FACTORY_BOARD")
        .envs([
            ("GIT_AUTHOR_NAME", "t"),
            ("GIT_AUTHOR_EMAIL", "t@t"),
            ("GIT_COMMITTER_NAME", "t"),
            ("GIT_COMMITTER_EMAIL", "t@t"),
        ])
        .args(["upstream", "check"])
        .arg(&d)
        .output()
        .unwrap();
    assert_eq!(upstream.status.code(), Some(1), "{}", both(&upstream));
    assert!(
        both(&upstream).contains("a home path:"),
        "{}",
        both(&upstream)
    );
}
