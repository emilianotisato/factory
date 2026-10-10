//! The worker hook's never-list (S1): workers never run `gh` and never
//! `git push`; the driver does both. Every such command a worker writes is
//! denied however it is wrapped — compound, nested, behind a wrapper or a
//! git global option — and one the hook cannot read for sure is denied
//! when it names either. Anything else gets no decision (Ln7).
use std::path::Path;

use factory::cli::hook::{NEVER_HINT, guard, hook_with};
use serde_json::{Value, json};

/// The hook's stdout for one Bash call: `None` for no decision, else the
/// decision and its reason, after checking it is one JSON line.
fn decide(cmd: &str) -> Option<(String, String)> {
    let input = json!({"tool_name": "Bash", "tool_input": {"command": cmd}, "cwd": "/tmp"});
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = hook_with(
        &mut input.to_string().as_bytes(),
        None,
        Some(Path::new("/nonexistent/repo")),
        &mut out,
        &mut err,
    )
    .unwrap();
    assert_eq!(code, 0, "{cmd}");
    if out.is_empty() {
        return None;
    }
    let text = String::from_utf8(out).unwrap();
    assert_eq!(text.lines().count(), 1, "{cmd}: {text:?}");
    let v: Value = serde_json::from_str(&text).unwrap();
    let o = &v["hookSpecificOutput"];
    assert_eq!(o["hookEventName"], "PreToolUse");
    Some((
        o["permissionDecision"].as_str().unwrap().to_string(),
        o["permissionDecisionReason"].as_str().unwrap().to_string(),
    ))
}

fn assert_denied(cmds: &[&str]) {
    for cmd in cmds {
        assert!(guard(cmd), "guard misses: {cmd}");
        assert_eq!(
            decide(cmd),
            Some(("deny".to_string(), NEVER_HINT.to_string())),
            "{cmd}"
        );
    }
}

#[test]
fn the_review_cases_are_denied() {
    assert_denied(&[
        "true && gh pr merge 1 --merge",
        "git -c core.hooksPath=/dev/null push origin HEAD:main",
        "git push origin --force",
        "git push origin +main",
        "git push --no-verify --force",
        "env -u CLAUDECODE git push",
    ]);
}

#[test]
fn the_second_pass_cases_are_denied() {
    assert_denied(&[
        "cd /tmp && gh pr merge 1",
        "echo $(gh pr ready 1)",
        "echo `gh pr ready 1`",
        "time gh pr close 1",
        "VAR=x gh api repos/o/r/pulls/1/merge -X PUT",
        "git -C \"$repo\" push --force",
        "git push origin +HEAD:refs/heads/main",
        "git push origin main",
        "git push --force-with-lease",
        // Any gh at all: a create without --draft opens the PR ready.
        "gh pr create --title x --body y",
        "gh pr edit 1 --ready",
        "gh --version",
    ]);
}

#[test]
fn every_segment_and_every_wrapper_is_read() {
    assert_denied(&[
        // compound, grouped, nested, piped, in the background
        "git add -A; git commit -m x; git push",
        "make || gh run rerun 1 --failed",
        "(gh pr merge 1)",
        "{ gh pr merge 1; }",
        "if true; then gh pr merge 1; fi",
        "for p in 1 2; do gh pr merge $p; done",
        "while false; do :; done; git push",
        "case x in x) gh pr merge 1;; esac",
        "gh pr merge 1 2>&1 | tee log",
        "git push &",
        "echo <(gh pr merge 1)",
        "x=$(echo $(gh pr ready 1))",
        "echo \"$(git push)\"",
        "true && gh --version",
        "ls; /usr/bin/gh auth status",
        "(cd x && gh repo view)",
        "echo $(gh api user)",
        "echo \"`gh api user`\"",
        // the program spelled another way
        "/usr/bin/gh pr merge 1",
        "\"gh\" pr merge 1",
        "/usr/bin/git push",
        "GIT_DIR=.git git push",
        // wrappers
        "command gh pr merge 1",
        "exec gh pr merge 1",
        "nohup gh pr merge 1 &",
        "timeout 60 git push",
        "nice -n 5 git push",
        "stdbuf -oL gh pr merge 1",
        "sudo -u me git push",
        "xargs gh pr merge < prs",
        "xargs -I{} git push origin {} < branches",
        "env GH_TOKEN=x gh pr merge 1",
        "env -i PATH=/usr/bin git push",
        "env -S 'gh api' user",
        "timeout -s KILL 30 gh run list",
        "nohup git push &",
        "xargs -n1 gh run view < ids",
        "find . -name x -exec git push \\;",
        "watch -n 5 gh run list",
        // shells and eval
        "sh -c \"gh pr merge 1\"",
        "bash -lc 'git push --force'",
        "bash -c 'cd x; bash -c \"git push\"'",
        "eval \"gh pr merge 1\"",
        "bash <<EOF\ngh pr merge 1\nEOF",
        "sh <<'EOF'\ngit push\nEOF",
        "bash -c 'git push origin HEAD'",
        "sh -ec \"cd x && gh api user\"",
        "bash <<EOF\ngh api user\nEOF",
        "eval 'gh' api user",
        // git's own ways to push
        "git --git-dir=.git --work-tree=. push",
        "git -C . -c a=b push",
        "git --no-pager push",
        "git send-pack origin main",
        "git lfs push origin main",
        "git -c alias.ship=push ship",
        "git -c core.hooksPath=/dev/null push",
        // an alias defined on the line, even one that is not push
        "git -c alias.p=status p",
        "git subtree push --prefix=x origin main",
    ]);
}

#[test]
fn doubt_is_denied_when_it_names_gh_or_push() {
    assert_denied(&[
        // unbalanced quotes
        "echo \"unterminated && gh pr merge 1",
        "echo 'x; git push",
        "echo 'unbalanced; git push",
        // a script the hook cannot see into
        "cat cmds | sh # gh",
        "printf 'git push' | bash",
        "python3 -c 'import os; os.system(\"gh pr merge 1\")'",
        "node -e 'require(\"child_process\").execSync(\"git push\")'",
        "$TOOL pr merge 1 # gh",
        "\"$GH\" pr merge 1",
        "\"$GH\" api user",
        "python3 -c \"import os; os.system('gh api user')\"",
        "printf 'gh api user' | sh",
    ]);
}

#[test]
fn the_rest_gets_no_decision() {
    for cmd in [
        "git status && git log -1",
        "git stash push -m wip",
        "git log --grep=push",
        "git commit -m \"fix: say gh and push in a message\"",
        "git commit -m \"$(cat <<'EOF'\ndocs: gh pr merge is the driver's\nEOF\n)\"",
        "cat <<'EOF' > notes.md\ngit push is the driver's\nEOF",
        "echo gh push",
        "grep -n 'gh pr' notes.md",
        "cargo test --test hook",
        "ls ghost && pushd src",
        "bash scripts/check.sh",
        "sh -c 'cargo fmt --check'",
        "cat cmds | sh",
        "git status && git log --grep push",
        "git commit -m \"$(cat <<'EOF'\nfix: push gh\nEOF\n)\"",
        "cat <<'EOF' > notes.md\ngh pr x\nEOF",
        "grep -rn \"git push\" docs",
        "python3 scripts/push_metrics.py",
        "bash scripts/ci.sh",
        "rg -n 'github' src",
        "pushd src && ls",
    ] {
        assert!(!guard(cmd), "guard denies: {cmd}");
        assert_eq!(decide(cmd), None, "{cmd}");
    }
}
