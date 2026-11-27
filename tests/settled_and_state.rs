#[path = "harness/mod.rs"]
mod harness;

use std::io::Write;
use std::process::{Output, Stdio};

use harness::Fixture;

const BIN: &str = env!("CARGO_BIN_EXE_factory");

fn review_settled(fx: &Fixture, feature: &str) -> Output {
    fx.command(BIN)
        .args(["review", "settled", feature])
        .output()
        .unwrap()
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn log_stdin(fx: &Fixture, args: &[&str], body: &str) -> Output {
    let mut child = fx
        .command(BIN)
        .arg("docs")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn factory docs");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(body.as_bytes())
        .unwrap();
    child.wait_with_output().expect("wait factory docs")
}

#[test]
fn settled_includes_answered_asks_and_prints_via_cli() {
    let fx = Fixture::new("settled-asks-cli");
    log_stdin(
        &fx,
        &[
            "log",
            "add",
            "001",
            "--kind",
            "decision",
            "--title",
            "Keep the queue",
            "--body-file",
            "-",
        ],
        "**Chose:** in memory\n",
    );
    fx.docs(&[
        "log",
        "add",
        "001",
        "--kind",
        "ask",
        "--title",
        "Ship without Redis?",
    ]);
    let ask_id = fx.log_of("ask")[0]["id"]
        .as_str()
        .expect("ask id")
        .to_string();
    log_stdin(
        &fx,
        &[
            "log",
            "add",
            "001",
            "--kind",
            "answer",
            "--ref",
            &ask_id,
            "--title",
            "a",
            "--body-file",
            "-",
        ],
        "lane-local only\n",
    );
    fx.docs(&[
        "log",
        "add",
        "001",
        "--kind",
        "ask",
        "--title",
        "Still open",
    ]);

    let want = factory::settled::for_feature(fx.path(), "001").expect("settled block");
    let o = review_settled(&fx, "001");
    assert_eq!(o.status.code(), Some(0), "{}", stdout(&o));
    assert_eq!(stdout(&o), want);
    assert!(want.contains("Keep the queue"));
    assert!(want.contains(&format!(
        "{ask_id} — Ship without Redis?: answered — lane-local only"
    )));
    assert!(!want.contains("Still open"));

    let bad = review_settled(&fx, "999");
    assert_eq!(bad.status.code(), Some(1));
}

#[test]
fn reviewed_head_controls_last_reviewed_and_clears_with_reviewed() {
    use harness::FEATURE_DIR;

    let fx = Fixture::new("reviewed-head");
    let head = fx.git(&["rev-parse", "HEAD"]).trim().to_string();
    fx.docs(&[
        "set",
        "001",
        "reviewed=2026-11-26",
        &format!("reviewed_head={head}"),
    ]);
    fx.git(&["add", "-A"]);
    fx.git(&["commit", "-q", "-m", "docs: reviewed with head"]);
    std::fs::write(fx.path().join("later.txt"), "x\n").unwrap();
    fx.git(&["add", "later.txt"]);
    fx.git(&["commit", "-q", "-m", "chore: after reviewed stamp"]);
    assert_eq!(
        factory::settled::last_reviewed(fx.path(), "001"),
        Some(head)
    );

    let bad = fx.docs_output(&["set", "001", "reviewed_head=not-a-sha"]);
    assert!(!bad.status.success());
    assert!(
        String::from_utf8_lossy(&bad.stderr).contains("not a commit sha"),
        "{}",
        String::from_utf8_lossy(&bad.stderr)
    );

    fx.docs(&["set", "001", "reviewed="]);
    fx.git(&["add", "-A"]);
    fx.git(&["commit", "-q", "-m", "docs: clear reviewed"]);
    assert_eq!(factory::settled::last_reviewed(fx.path(), "001"), None);
    let state = std::fs::read_to_string(fx.path().join(FEATURE_DIR).join("state.toml")).unwrap();
    assert!(!state.contains("reviewed_head"));
}
