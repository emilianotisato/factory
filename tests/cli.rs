//! The `factory` CLI surface (slice 01 task 1): the eight subcommands, and
//! "not built yet" from each one no slice has shaped yet. `run` and `stop`
//! left this list in slice 02 (`tests/run.rs`), `answer` in slice 04
//! (`tests/lanes.rs`).
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_factory");
const SUBCOMMANDS: [&str; 8] = [
    "run", "wait", "answer", "status", "stop", "report", "ctx", "hook",
];

#[test]
fn help_lists_the_eight_subcommands() {
    let out = Command::new(BIN).arg("--help").output().unwrap();
    assert!(out.status.success());
    let help = String::from_utf8(out.stdout).unwrap();
    for sub in SUBCOMMANDS {
        assert!(
            help.lines().any(|l| l.trim_start().starts_with(sub)),
            "{sub} missing in:\n{help}"
        );
    }
}

#[test]
fn every_subcommand_says_not_built_yet_and_exits_2() {
    let calls: [&[&str]; 7] = [
        &["wait"],
        &["status"],
        &["report", "--run", "x"],
        &["ctx", "def", "Foo"],
        &["ctx", "refs", "Foo"],
        &["ctx", "outline", "src/lib.rs"],
        &["hook"],
    ];
    for args in calls {
        let out = Command::new(BIN).args(args).output().unwrap();
        assert_eq!(
            out.status.code(),
            Some(2),
            "{args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(err.contains("not built yet (slice "), "{args:?}: {err}");
    }
}

#[test]
fn answer_refuses_an_id_that_is_neither_block_nor_ask() {
    let out = Command::new(BIN)
        .args(["answer", "1", "2"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("`1` is neither BLOCK-n nor ASK-n"), "{err}");
}
