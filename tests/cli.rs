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

/// `factory hook` with `stdin` piped in, `FACTORY_PACK` at `pack` and
/// `FACTORY_REPO` at `repo` (unset when `None`): (exit code, the decision,
/// the reason, stderr). The decision is `""` when stdout is empty: that is
/// how the hook allows, leaving the call to the permission mode (Ln7).
fn hook_in(
    stdin: &str,
    pack: &std::path::Path,
    repo: Option<&std::path::Path>,
) -> (Option<i32>, String, String, String) {
    let mut cmd = Command::new(BIN);
    cmd.arg("hook").env("FACTORY_PACK", pack);
    match repo {
        Some(r) => cmd.env("FACTORY_REPO", r),
        None => cmd.env_remove("FACTORY_REPO"),
    };
    let mut child = cmd
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
    let err = String::from_utf8_lossy(&out.stderr).into_owned();
    if out.stdout.is_empty() {
        return (out.status.code(), String::new(), String::new(), err);
    }
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    assert_eq!(text.lines().count(), 1, "one decision line: {text:?}");
    let v: Value = serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("hook stdout is not JSON ({e}): {text}"));
    let o = &v["hookSpecificOutput"];
    assert_eq!(o["hookEventName"], "PreToolUse", "{v}");
    (
        out.status.code(),
        o["permissionDecision"].as_str().unwrap_or("").to_string(),
        o["permissionDecisionReason"]
            .as_str()
            .unwrap_or("")
            .to_string(),
        err,
    )
}

fn hook(stdin: &str, pack: &std::path::Path) -> (Option<i32>, String, String, String) {
    hook_in(stdin, pack, None)
}

#[test]
fn hook_denies_repo_greps_and_whole_reads_of_held_files() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    std::fs::create_dir_all(repo.join("src")).unwrap();
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

    // The task's acceptance, one call each. The hint names this binary by
    // its absolute path, so a worker without `factory` on PATH can run it
    // (K1).
    let (code, d, why, _) = hook(&bash("grep -rn foo src").to_string(), &pack);
    assert_eq!((code, d.as_str()), (Some(0), "deny"));
    assert_eq!(
        why,
        format!(
            "use: {} ctx refs <word> (or ctx def <symbol>); grep only for literal strings and log output",
            factory::paths::shell_word(BIN)
        )
    );
    assert!(std::path::Path::new(BIN).is_absolute());
    assert_eq!(
        hook(&bash("grep -n foo src/one.php").to_string(), &pack),
        (Some(0), String::new(), String::new(), String::new()),
        "an allow is no decision: empty stdout, nothing on stderr"
    );
    assert_eq!(hook(&bash("rg foo").to_string(), &pack).1, "deny");
    assert_eq!(
        hook(&bash("cat src/x.php | grep foo").to_string(), &pack).1,
        ""
    );
    let held = format!("{cwd}/{slice}");
    let (_, d, why, _) = hook(&read(held.clone(), json!({})).to_string(), &pack);
    assert_eq!(d, "deny");
    assert_eq!(
        why,
        "that file is in your pack under \"# Slice\"; read a range if you need more"
    );
    let partial = read(held, json!({"offset": 10, "limit": 40}));
    assert_eq!(hook(&partial.to_string(), &pack).1, "");
    let prd_read = read(format!("{cwd}/{prd}"), json!({}));
    assert_eq!(hook(&prd_read.to_string(), &pack).1, "");
    let (_, d, why, _) = hook(
        &read(format!("{cwd}/src/helper.rs"), json!({})).to_string(),
        &pack,
    );
    assert_eq!((d.as_str(), why.contains("\"# Outline\"")), ("deny", true));
    let (code, d, why, err) = hook("{not json", &pack);
    assert_eq!((code, d.as_str(), why.as_str()), (Some(0), "", ""));
    assert!(err.contains("factory hook: cannot parse stdin"), "{err}");
    assert!(err.contains("allowed (no decision)"), "{err}");

    // C3: only the repo's copy of a held file is held. Another checkout
    // with the same relative path reads freely.
    let other = format!("{}/elsewhere/{slice}", tmp.path().display());
    assert_eq!(hook(&read(other, json!({})).to_string(), &pack).1, "");

    // FACTORY_REPO names the repo even when the call runs from a subdir.
    let sub = repo.join("src");
    let from_sub = json!({"tool_name": "Read",
        "tool_input": {"file_path": format!("{cwd}/src/helper.rs")},
        "cwd": sub.to_str().unwrap()});
    assert_eq!(
        hook_in(&from_sub.to_string(), &pack, Some(&repo)).1,
        "deny",
        "the repo is FACTORY_REPO, not the call's cwd"
    );

    // No pack at all: the grep hint still holds, no file is held.
    let none = tmp.path().join("missing.md");
    assert_eq!(hook(&bash("grep -R foo .").to_string(), &none).1, "deny");
    let any = read(format!("{cwd}/{slice}"), json!({}));
    assert_eq!(hook(&any.to_string(), &none).1, "");
}

/// S1 through the binary: a never-list command is denied with the hint
/// that says why, and input the hook cannot parse fails closed when it
/// names `gh` or `push`.
#[test]
fn hook_denies_gh_and_git_push_and_fails_closed() {
    let tmp = tempfile::tempdir().unwrap();
    let pack = tmp.path().join("none.md");
    let cwd = tmp.path().to_str().unwrap();
    let bash = |cmd: &str| {
        json!({"tool_name": "Bash", "tool_input": {"command": cmd}, "cwd": cwd}).to_string()
    };
    let never = factory::cli::hook::NEVER_HINT;
    for cmd in [
        "gh --version",
        "true && gh --version",
        "git -c x.y=z push --dry-run /nonexistent HEAD",
    ] {
        let (code, d, why, _) = hook(&bash(cmd), &pack);
        assert_eq!(
            (code, d.as_str(), why.as_str()),
            (Some(0), "deny", never),
            "{cmd}"
        );
    }
    let (_, d, _, _) = hook(&bash("git status && git log -1"), &pack);
    assert_eq!(d, "", "plain git is no decision");
    let cut = r#"{"tool_name":"Bash","tool_input":{"command":"gh api user"#;
    let (code, d, why, err) = hook(cut, &pack);
    assert_eq!((code, d.as_str(), why.as_str()), (Some(0), "deny", never));
    assert!(err.contains("it names gh or push: denied"), "{err}");
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
