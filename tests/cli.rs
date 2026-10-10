//! The `factory` CLI surface (slice 01 task 1): the eight subcommands. Each
//! said "not built yet" until its slice shaped it; `hook`, the last, left
//! that list in slice 13 and is tested here by piping PreToolUse JSON into
//! the binary with `FACTORY_PACK` set (slice 13 task 1).
use std::io::Write;
use std::process::{Command, Stdio};

use serde_json::{Value, json};

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

/// `factory hook` with `stdin` piped in and `FACTORY_PACK` at `pack`:
/// (exit code, the decision, the reason, stderr).
fn hook(stdin: &str, pack: &std::path::Path) -> (Option<i32>, String, String, String) {
    let mut child = Command::new(BIN)
        .arg("hook")
        .env("FACTORY_PACK", pack)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    let v: Value = serde_json::from_slice(&out.stdout).unwrap_or_else(|e| {
        panic!(
            "hook stdout is not JSON ({e}): {}",
            String::from_utf8_lossy(&out.stdout)
        )
    });
    let o = &v["hookSpecificOutput"];
    assert_eq!(o["hookEventName"], "PreToolUse", "{v}");
    (
        out.status.code(),
        o["permissionDecision"].as_str().unwrap_or("").to_string(),
        o["permissionDecisionReason"]
            .as_str()
            .unwrap_or("")
            .to_string(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn hook_denies_repo_greps_and_whole_reads_of_held_files() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    let slice = "docs/features/001-x/slice-01-a.md";
    let prd = "docs/features/001-x/prd.md";
    let pack = tmp.path().join("slice-01-1.md");
    std::fs::write(
        &pack,
        format!("<!-- factory-pack files: {slice},{prd},src/helper.rs -->\n\n# Slice\n"),
    )
    .unwrap();
    let cwd = repo.to_str().unwrap();
    let bash = |cmd: &str| json!({"tool_name": "Bash", "tool_input": {"command": cmd}, "cwd": cwd});
    let read = |file: String, extra: Value| {
        let mut input = json!({"file_path": file});
        for (k, v) in extra.as_object().unwrap() {
            input[k] = v.clone();
        }
        json!({"tool_name": "Read", "tool_input": input, "cwd": cwd})
    };

    // The task's acceptance, one call each.
    let (code, d, why, _) = hook(&bash("grep -rn foo src").to_string(), &pack);
    assert_eq!((code, d.as_str()), (Some(0), "deny"));
    assert!(why.contains("factory ctx refs"), "{why}");
    assert_eq!(
        why,
        "use: factory ctx refs <word> (or ctx def <symbol>); grep only for literal strings and log output"
    );
    assert_eq!(
        hook(&bash("grep -n foo src/one.php").to_string(), &pack).1,
        "allow"
    );
    assert_eq!(hook(&bash("rg foo").to_string(), &pack).1, "deny");
    assert_eq!(
        hook(&bash("cat src/x.php | grep foo").to_string(), &pack).1,
        "allow"
    );
    let held = format!("{cwd}/{slice}");
    let (_, d, why, _) = hook(&read(held.clone(), json!({})).to_string(), &pack);
    assert_eq!(d, "deny");
    assert_eq!(
        why,
        "that file is in your pack under \"# Slice\"; read a range if you need more"
    );
    let partial = read(held, json!({"offset": 10, "limit": 40}));
    assert_eq!(hook(&partial.to_string(), &pack).1, "allow");
    let prd_read = read(format!("{cwd}/{prd}"), json!({}));
    assert_eq!(hook(&prd_read.to_string(), &pack).1, "allow");
    let (_, d, why, _) = hook(
        &read(format!("{cwd}/src/helper.rs"), json!({})).to_string(),
        &pack,
    );
    assert_eq!((d.as_str(), why.contains("\"# Outline\"")), ("deny", true));
    let (code, d, why, err) = hook("{not json", &pack);
    assert_eq!((code, d.as_str(), why.as_str()), (Some(0), "allow", ""));
    assert!(err.contains("factory hook: cannot parse stdin"), "{err}");

    // No pack at all: the grep hint still holds, no file is held.
    let none = tmp.path().join("missing.md");
    assert_eq!(hook(&bash("grep -R foo .").to_string(), &none).1, "deny");
    let any = read(format!("{cwd}/{slice}"), json!({}));
    assert_eq!(hook(&any.to_string(), &none).1, "allow");
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
