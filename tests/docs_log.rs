//! `factory docs log add|show` and the board's log fields (PRD-002 FR-14,
//! slice 02).
#[path = "harness/mod.rs"]
mod harness;

use std::collections::BTreeSet;
use std::io::Write;
use std::process::{Output, Stdio};

use factory::docs::Status;
use factory::docs::log::Entry;
use harness::{FEATURE_DIR, Fixture};

/// `factory docs <args>` with `stdin` piped in.
fn docs_stdin(fx: &Fixture, args: &[&str], stdin: &str) -> Output {
    let mut child = fx
        .command(env!("CARGO_BIN_EXE_factory"))
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
        .write_all(stdin.as_bytes())
        .unwrap();
    child.wait_with_output().expect("wait factory docs")
}

fn status(fx: &Fixture) -> Status {
    serde_json::from_value(fx.docs_json("status")).unwrap()
}

fn log_lines(fx: &Fixture) -> Vec<String> {
    fx.read(&format!("{FEATURE_DIR}/log.jsonl"))
        .lines()
        .map(String::from)
        .collect()
}

#[test]
fn two_processes_append_fifty_each_without_a_gap_or_a_duplicate() {
    let fx = Fixture::new("log-concurrent");
    let args = [
        "log",
        "add",
        "001",
        "--kind",
        "decision",
        "--title",
        "t",
        "--body-file",
        "-",
    ];
    std::thread::scope(|s| {
        let workers: Vec<_> = (0..2)
            .map(|w| {
                let fx = &fx;
                s.spawn(move || {
                    for i in 0..50 {
                        let o = docs_stdin(fx, &args, &format!("worker {w} entry {i}\n"));
                        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
                    }
                })
            })
            .collect();
        for w in workers {
            w.join().unwrap();
        }
    });
    let lines = log_lines(&fx);
    assert_eq!(lines.len(), 100);
    let ids: BTreeSet<String> = lines
        .iter()
        .map(|l| serde_json::from_str::<Entry>(l).unwrap().id.unwrap())
        .collect();
    let want: BTreeSet<String> = (1..=100).map(|n| format!("AD-{n}")).collect();
    assert_eq!(ids, want);
    assert_eq!(
        fx.git(&["status", "--porcelain"]),
        format!("?? {FEATURE_DIR}/log.jsonl\n"),
        "docs.lock stays ignored"
    );
}

#[test]
fn a_body_with_a_title_and_preamble_round_trips() {
    let fx = Fixture::new("log-body");
    let body = "# A title\n\nprose before any heading\n\n## A heading\n\ntext";
    let o = docs_stdin(
        &fx,
        &[
            "log",
            "add",
            "001",
            "--kind",
            "decision",
            "--title",
            "t",
            "--body-file",
            "-",
        ],
        body,
    );
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    assert_eq!(String::from_utf8_lossy(&o.stdout), "AD-1\n");

    let json: Vec<Entry> =
        serde_json::from_str(&fx.docs(&["log", "show", "001", "--json"])).unwrap();
    assert_eq!(json[0].body, body);
    let shown = fx.docs(&["log", "show", "001"]);
    let entry = shown
        .split("### AD-1 — t · ")
        .nth(1)
        .expect("AD-1 in log show");
    assert!(entry.contains(body), "body verbatim in:\n{shown}");
}

#[test]
fn the_board_reads_its_log_fields_from_log_jsonl() {
    let fx = Fixture::new("log-board");
    fx.docs(&["set", "001", "slices.1.status=done", "slices.2.status=done"]);
    let s = status(&fx);
    assert_eq!(s.unverified, [1, 2]);
    assert!(s.checkpoint_due);
    assert_eq!(s.spawnable, 0);

    assert_eq!(
        fx.docs(&[
            "log",
            "add",
            "001",
            "--kind",
            "checkpoint",
            "--slice",
            "2",
            "--title",
            "after 2"
        ]),
        "CP-1\n"
    );
    let s = status(&fx);
    assert_eq!(
        s.checkpoints
            .iter()
            .map(|c| (c.n, c.after))
            .collect::<Vec<_>>(),
        [(1, Some(2))]
    );
    assert!(s.unverified.is_empty());
    assert!(!s.checkpoint_due);

    let add = |args: &[&str]| {
        let mut a = vec!["log", "add", "001"];
        a.extend_from_slice(args);
        fx.docs(&a)
    };
    assert_eq!(
        add(&["--kind", "block", "--phase", "checkpoint", "--title", "red"]),
        "BLOCK-1\n"
    );
    let s = status(&fx);
    assert_eq!(s.open_blocks.len(), 1);
    assert_eq!(s.open_blocks[0].n, 1);
    assert_eq!(s.open_blocks[0].phase.as_deref(), Some("checkpoint"));
    assert_eq!(s.open_blocks[0].title.as_deref(), Some("red"));
    assert_eq!(
        add(&["--kind", "answer", "--ref", "BLOCK-1", "--title", "a"]),
        ""
    );
    assert!(status(&fx).open_blocks.is_empty());

    add(&["--kind", "decision", "--flag", "decide", "--title", "d"]);
    add(&["--kind", "decision", "--title", "plain"]);
    add(&["--kind", "ask", "--phase", "preflight", "--title", "q"]);
    let summary = |s: Status| (s.log.decisions, s.log.flagged, s.log.decide, s.log.blocks);
    assert_eq!(summary(status(&fx)), (3, vec![1], vec![1], 1));
    add(&[
        "--kind",
        "review",
        "--ref",
        "AD-1",
        "--verdict",
        "kept",
        "--title",
        "r",
    ]);
    assert_eq!(
        summary(status(&fx)),
        (3, vec![1], vec![1], 1),
        "a review leaves decide alone"
    );
    assert!(
        fx.docs(&["log", "show", "001", "--decide"])
            .contains("_(none)_"),
        "but log show --decide drops the reviewed one"
    );
}

#[test]
fn refusals_exit_2_and_log_merge_points_at_log_add() {
    let fx = Fixture::new("log-refuse");
    let o = fx.docs_output(&[
        "log", "add", "001", "--kind", "answer", "--ref", "BLOCK-9", "--title", "a",
    ]);
    assert_eq!(o.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&o.stderr).contains("BLOCK-9"));
    let o = fx.docs_output(&["log", "merge", "001"]);
    assert_eq!(o.status.code(), Some(2));
    let err = String::from_utf8_lossy(&o.stderr);
    assert!(err.contains("removed in v3"), "{err}");
    assert!(err.contains("factory docs log add"), "{err}");
}
