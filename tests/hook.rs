//! Workers never run `gh` or `git push`, however wrapped; the driver does both.
use std::path::Path;

use factory::cli::hook::{NEVER_HINT, guard, hook_with};
use serde_json::{Value, json};

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
fn a_script_is_denied_only_when_it_runs_a_process_and_names_gh_or_push() {
    for cmd in [
        "python3 - <<'EOF'\nfrom pathlib import Path\np = Path('src/git.rs')\nlines = []\nfor line in p.read_text().splitlines():\n    lines.append(line.replace('line.push(', 'out.push('))\np.write_text('\\n'.join(lines))\nEOF",
        "python3 -c \"print('push')\"",
        "node -e \"const a = []; a.push(1); console.log(a)\"",
        "deno eval \"console.log('gh')\"",
        "ruby -e 'puts \"git push\"'",
        "python3 <<'EOF'\nimport subprocess\nsubprocess.run(['cargo', 'test'])\nEOF",
    ] {
        assert!(!guard(cmd), "guard denies: {cmd}");
        assert_eq!(decide(cmd), None, "{cmd}");
    }
    assert_denied(&[
        "python3 - <<'EOF'\nimport subprocess\nsubprocess.run([\"gh\", \"pr\", \"view\"])\nEOF",
        "python3 -c \"import os; os.system('git push')\"",
        "node -e \"require('child_process').execSync('gh pr list')\"",
        "node --eval=\"require('child_process').execSync('gh pr list')\"",
        "perl -e 'system(\"git push\")'",
        "deno eval \"Deno.Command; spawn('gh')\"",
        "printf 'import os; os.system(\"git push\")' | python3",
        "python3 -c # gh",
    ]);
}

#[test]
fn a_script_that_gets_gh_or_push_from_around_it_is_denied() {
    assert_denied(&[
        "python3 -c 'import os,sys; os.system(sys.argv[1])' 'gh pr list'",
        "C=\"gh pr list\" python3 <<'EOF'\nimport os\nos.system(os.environ['C'])\nEOF",
        "python3 -c 'import sys; exec(sys.stdin.read())' <<'EOF'\nimport os\nos.system(\"gh pr list\")\nEOF",
        "S='import os; os.system(\"gh pr list\")'; python3 -c \"$S\"",
        "node -pe \"require('child_process').execSync('gh pr list')\"",
        "perl -MFile::Copy -e 'system(\"gh pr list\")'",
        "ruby -ropen3 -e 'Open3.capture2(\"gh pr list\")'",
        "php -derror_reporting=E_ALL -r 'system(\"gh pr list\");'",
        "perl -e 'print 1' -e 'system(\"git push\")'",
        "deno eval --ext ts \"spawn('gh')\"",
    ]);
    for cmd in [
        "php -r '$c->push(1); print_r($c);'",
        "python3 -Wignore::DeprecationWarning -c \"print('push')\"",
    ] {
        assert!(!guard(cmd), "guard denies: {cmd}");
        assert_eq!(decide(cmd), None, "{cmd}");
    }
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

// The pack's paths resolve against `FACTORY_REPO`, not the hook's cwd.
#[test]
fn a_whole_read_of_a_held_file_in_a_lane_is_denied() {
    use std::process::{Command, Stdio};
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path().canonicalize().unwrap();
    let (main, lane) = (base.join("main"), base.join("lane-1"));
    let git = |args: &[&str]| {
        let out = Command::new("git")
            .args(["-c", "user.name=T", "-c", "user.email=t@example.invalid"])
            .args(args)
            .current_dir(&main)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .output()
            .unwrap();
        assert!(out.status.success(), "git {args:?}: {out:?}");
    };
    std::fs::create_dir_all(main.join("src")).unwrap();
    git(&["init", "-q", "-b", "main"]);
    std::fs::write(main.join("src/lib.txt"), "lib\n").unwrap();
    git(&["add", "-A"]);
    git(&["commit", "-q", "-m", "init"]);
    git(&["worktree", "add", "-q", "--detach", lane.to_str().unwrap()]);
    let pack = base.join("slice-02-1.md");
    std::fs::write(&pack, "<!-- factory-pack files: src/lib.txt -->\n# Slice\n").unwrap();

    let input = json!({
        "hook_event_name": "PreToolUse",
        "tool_name": "Read",
        "tool_input": {"file_path": lane.join("src/lib.txt")},
        "cwd": lane,
    });
    let hook = |repo: &Path| {
        let mut child = Command::new(env!("CARGO_BIN_EXE_factory"))
            .arg("hook")
            .env(factory::cli::hook::PACK_ENV, &pack)
            .env(factory::worker::REPO_ENV, repo)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        use std::io::Write as _;
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.to_string().as_bytes())
            .unwrap();
        let out = child.wait_with_output().unwrap();
        assert!(out.status.success(), "{out:?}");
        String::from_utf8(out.stdout).unwrap()
    };
    let text = hook(&lane);
    println!("hook, FACTORY_REPO={}: {text}", lane.display());
    let v: Value = serde_json::from_str(&text).unwrap_or_else(|e| panic!("{e}: {text:?}"));
    let o = &v["hookSpecificOutput"];
    assert_eq!(o["permissionDecision"], "deny", "{text}");
    assert_eq!(
        o["permissionDecisionReason"],
        factory::cli::hook::pack_hint("Slice")
    );
    assert_eq!(hook(&main), "", "outside the main tree: no decision");
}
