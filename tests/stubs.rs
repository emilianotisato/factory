#[path = "harness/mod.rs"]
mod harness;

use std::fs;
use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::Duration;

use harness::Fixture;
use serde_json::{Value, json};

const SESSION: &str = "11111111-2222-4333-8444-555555555555";

/// The stub with the driver's flags; `pack` is the `--append-system-prompt[-file]` pair.
fn claude(fx: &Fixture, pack: &[&str]) -> Command {
    let mut c = fx.command(harness::stubs_dir().join("claude"));
    c.args([
        "-p",
        "build the slice",
        "--output-format",
        "stream-json",
        "--verbose",
    ])
    .args(["--permission-mode", "bypassPermissions"])
    .args(pack)
    .args([
        "--session-id",
        SESSION,
        "--model",
        "claude-sonnet-4-5",
        "--effort",
        "medium",
    ]);
    c
}

fn tool_use_paths(events: &[Value]) -> Vec<String> {
    events
        .iter()
        .filter(|e| e["type"] == "assistant")
        .flat_map(|e| {
            e["message"]["content"]
                .as_array()
                .cloned()
                .unwrap_or_default()
        })
        .filter(|c| c["type"] == "tool_use")
        // A `Bash` call names no file.
        .filter_map(|c| c["input"]["file_path"].as_str().map(str::to_string))
        .collect()
}

fn bash_commands(events: &[Value]) -> Vec<String> {
    events
        .iter()
        .filter(|e| e["type"] == "assistant")
        .flat_map(|e| {
            e["message"]["content"]
                .as_array()
                .cloned()
                .unwrap_or_default()
        })
        .filter(|c| c["type"] == "tool_use" && c["name"] == "Bash")
        .filter_map(|c| c["input"]["command"].as_str().map(str::to_string))
        .collect()
}

#[test]
fn stub_claude_flips_the_slice_then_replays_a_result() {
    let fx = Fixture::new("stub-claude");
    let slice = fx.slice_path(1);
    let pack_text = format!(
        "{}\n\nBuild slice 01 as the contract says.\n",
        slice.display()
    );
    let pack = fx.stub_dir().join("pack-01.md");
    fs::write(&pack, &pack_text).unwrap();

    let out = claude(
        &fx,
        &["--append-system-prompt-file", pack.to_str().unwrap()],
    )
    .output()
    .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).unwrap();
    let events: Vec<Value> = stdout
        .lines()
        .map(|l| serde_json::from_str(l).expect(l))
        .collect();

    assert_eq!(events[0]["type"], "system");
    assert_eq!(events[0]["subtype"], "init");
    assert_eq!(events[0]["cwd"], fx.path().to_str().unwrap());
    assert_eq!(events[0]["session_id"], SESSION);
    assert_eq!(events[0]["model"], "claude-sonnet-4-5");
    let last = events.last().unwrap();
    assert_eq!(last["type"], "result");
    assert_eq!(last["subtype"], "success");
    assert_eq!(last["session_id"], SESSION);
    let usage = &last["usage"];
    for k in [
        "input_tokens",
        "cache_creation_input_tokens",
        "cache_read_input_tokens",
        "output_tokens",
    ] {
        assert!(usage[k].is_number(), "{k} missing in {usage}");
    }
    assert!(
        usage["output_tokens_details"]["thinking_tokens"].is_number(),
        "{usage}"
    );
    assert!(
        last["modelUsage"].is_object()
            && last["num_turns"].is_number()
            && last["duration_ms"].is_number()
    );
    assert!(
        last["result"]
            .as_str()
            .unwrap()
            .starts_with("STATUS: done\n"),
        "{}",
        last["result"]
    );
    let paths = tool_use_paths(&events);
    assert!(
        paths.contains(
            &fx.path()
                .join("src/slice-01.txt")
                .to_str()
                .unwrap()
                .to_string()
        ),
        "{paths:?}"
    );
    assert!(
        paths.contains(&slice.to_str().unwrap().to_string()),
        "{paths:?}"
    );
    // Three Bash calls: the AD, the inner run, the status.
    let bash = bash_commands(&events);
    assert_eq!(bash.len(), 3, "{bash:?}");
    assert!(
        bash[0].starts_with("factory docs log add 001 --kind decision --slice 1 "),
        "{bash:?}"
    );
    assert_eq!(bash[2], "factory docs set 001 slices.1.status=done");
    assert!(
        !paths
            .iter()
            .any(|p| p.ends_with(".md") && p.contains("/log-")),
        "{paths:?}"
    );
    // One tool_result per tool_use: two Reads, the Write, the AD, the inner run, the status.
    assert_eq!(events.iter().filter(|e| e["type"] == "user").count(), 6);

    assert_eq!(fx.slice_status(1), "done");
    let decisions = fx.log_of("decision");
    assert_eq!(decisions.len(), 1, "{decisions:?}");
    assert_eq!(decisions[0]["id"], "AD-1", "{decisions:?}");
    assert_eq!(decisions[0]["slice"], 1, "{decisions:?}");
    assert!(
        decisions[0]["body"]
            .as_str()
            .unwrap()
            .contains("**Review:** no"),
        "{decisions:?}"
    );
    let feature = fx.path().join("docs/features/001-fixture");
    for f in ["prd.md", "state.toml", "log.jsonl"] {
        assert!(feature.join(f).is_file(), "{f}");
    }
    let names: Vec<String> = fs::read_dir(&feature)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    assert_eq!(
        names.iter().filter(|n| n.starts_with("slice-")).count(),
        3,
        "{names:?}"
    );
    assert!(!names.iter().any(|n| n == "agent-log.md"), "{names:?}");
    assert!(!names.iter().any(|n| n.starts_with("log-")), "{names:?}");
    assert_eq!(fx.read("src/slice-01.txt"), "touched by spawn 1\n");
    assert_eq!(fx.read("src/lib.txt"), "fixture code file\n");
    assert!(
        fs::read_to_string(&slice)
            .unwrap()
            .contains("**Progress**: built by the stub claude (spawn 1)")
    );

    let calls = fx.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0]["n"], 1);
    assert_eq!(calls[0]["pack"], pack_text);
    assert_eq!(calls[0]["pack_source"], "file");
    assert_eq!(calls[0]["prompt"], "build the slice");
    assert_eq!(calls[0]["model"], "claude-sonnet-4-5");
    assert_eq!(calls[0]["effort"], "medium");
    assert_eq!(
        calls[0]["env"]["FACTORY_STUB_DIR"],
        fx.stub_dir().to_str().unwrap()
    );
    assert!(
        calls[0]["argv"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a == "--permission-mode")
    );

    let s = fx.docs_json("status");
    assert_eq!(s["slices"][0]["status"], "done", "{s}");
    assert_eq!(
        s["ready"],
        json!([{ "number": 2, "file": "001-fixture/slice-02-b.md" }]),
        "{s}"
    );
    assert_eq!(s["unverified"], json!([1]), "{s}");
    assert_eq!(s["spawnable"], 1, "{s}");
}

#[test]
fn stub_claude_delay_leaves_the_slice_building_mid_spawn() {
    let fx = Fixture::new("stub-delay");
    fx.write_plan(&[
        json!({ "outcome": "done", "delay_ms": 2000, "touch": ["src/lib.txt", "src/other.txt"] }),
    ]);
    let slice = fx.slice_path(2);
    let pack = format!("{}\n", slice.display());

    let child = claude(&fx, &["--append-system-prompt", &pack])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    sleep(Duration::from_millis(500));
    assert_eq!(fx.slice_status(2), "building");

    let out = child.wait_with_output().unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(fx.slice_status(2), "done");
    assert_eq!(fx.read("src/other.txt"), "touched by spawn 1\n");
    let decisions = fx.log_of("decision");
    assert_eq!(decisions.len(), 1, "{decisions:?}");
    assert_eq!(decisions[0]["id"], "AD-1", "{decisions:?}");
    assert_eq!(decisions[0]["slice"], 2, "{decisions:?}");
    assert_eq!(fx.calls()[0]["pack_source"], "inline");
}

#[test]
fn stub_claude_refuses_an_unknown_outcome() {
    let fx = Fixture::new("stub-unknown");
    fx.write_plan(&[json!({ "outcome": "rate_limit" })]);
    let pack = format!("{}\n", fx.slice_path(1).display());
    let out = claude(&fx, &["--append-system-prompt", &pack])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(65));
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("\"rate_limit\" is not one the stub knows")
    );
    assert_eq!(fx.slice_status(1), "pending");
    assert_eq!(
        fx.calls().len(),
        1,
        "the call is recorded before the step is judged"
    );
}

#[test]
fn stub_gh_records_calls_and_answers_pr_create() {
    let fx = Fixture::new("stub-gh");
    let gh = harness::stubs_dir().join("gh");

    let out = fx
        .command(&gh)
        .args(["pr", "create", "--draft", "--body-file", "x"])
        .output()
        .unwrap();
    assert!(out.status.success());
    assert_eq!(
        String::from_utf8(out.stdout).unwrap().trim(),
        "https://example.invalid/pr/1"
    );
    assert!(
        fx.command(&gh)
            .args(["auth", "status"])
            .output()
            .unwrap()
            .status
            .success()
    );
    let view = fx
        .command(&gh)
        .args(["pr", "view", "1", "--json", "number,url"])
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&view.stdout).unwrap();
    assert_eq!(v["number"], 1);
    assert_eq!(v["url"], "https://example.invalid/pr/1");
    let edit = fx
        .command(&gh)
        .args(["pr", "edit", "1", "--title", "two words"])
        .output()
        .unwrap();
    assert!(edit.status.success() && edit.stdout.is_empty());

    let log = fs::read_to_string(fx.stub_dir().join("gh.log")).unwrap();
    let lines: Vec<&str> = log.lines().collect();
    assert_eq!(
        lines,
        [
            "pr create --draft --body-file x",
            "auth status",
            "pr view 1 --json number,url",
            "pr edit 1 --title \"two words\""
        ]
    );
}

fn stream(out: &std::process::Output) -> Vec<Value> {
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| serde_json::from_str(l).expect(l))
        .collect()
}

#[test]
fn stub_claude_sh_edits_through_one_bash_call_and_no_named_tool() {
    let fx = Fixture::new("stub-sh");
    let script = "git mv src/lib.txt src/moved.txt && printf 'x\\n' > src/printed.txt \
                  && mkdir -p gen && printf 'a\\n' > gen/a.txt && ln -s gen gen-link";
    fx.write_plan(&[json!({ "outcome": "done", "sh": script })]);
    let pack = format!("{}\n", fx.slice_path(1).display());
    let events = stream(
        &claude(&fx, &["--append-system-prompt", &pack])
            .output()
            .unwrap(),
    );

    for f in ["src/moved.txt", "src/printed.txt", "gen/a.txt"] {
        assert!(fx.path().join(f).is_file(), "{f} missing");
    }
    let link = fs::symlink_metadata(fx.path().join("gen-link")).unwrap();
    assert!(link.file_type().is_symlink());
    assert!(!fx.path().join("src/lib.txt").exists());
    let bash = bash_commands(&events);
    assert_eq!(bash.iter().filter(|c| *c == script).count(), 1, "{bash:?}");
    let named = tool_use_paths(&events);
    for f in ["src/moved.txt", "src/printed.txt", "gen/a.txt", "gen-link"] {
        assert!(!named.iter().any(|p| p.ends_with(f)), "{f} in {named:?}");
    }
}

#[test]
fn stub_claude_slice_file_head_and_linger() {
    let fx = Fixture::new("stub-linger");
    fx.write_plan(&[json!({ "outcome": "done", "linger_ms": 1500 })]);
    let pack = format!("{}\n", fx.slice_path(2).display());
    let mut child = claude(&fx, &["--append-system-prompt", &pack])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut seen_done = false;
    for _ in 0..100 {
        if fx.slice_status(2) == "done" {
            seen_done = true;
            break;
        }
        sleep(Duration::from_millis(20));
    }
    assert!(seen_done, "the slice never read done");
    assert!(
        child.try_wait().unwrap().is_none(),
        "the stub exited before its linger"
    );
    stream(&child.wait_with_output().unwrap());
    assert_eq!(fx.read("src/slice-02.txt"), "touched by spawn 1\n");
    assert_eq!(fx.read("src/lib.txt"), "fixture code file\n");
    let head = fx.git(&["rev-parse", "HEAD"]);
    assert_eq!(fx.calls()[0]["head"], head.trim());
}

#[test]
fn stub_claude_a_failing_sh_step_exits_65() {
    let fx = Fixture::new("stub-sh-red");
    fx.write_plan(&[json!({ "outcome": "done", "sh": "echo broken >&2; exit 1" })]);
    let pack = format!("{}\n", fx.slice_path(1).display());
    let out = claude(&fx, &["--append-system-prompt", &pack])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(65));
    assert!(String::from_utf8_lossy(&out.stderr).contains("broken"));
}

#[test]
fn stub_claude_merge_role_resolves_or_blocks_and_leaves_the_status() {
    let fx = Fixture::new("stub-merge");
    let state = fx.slice_path(2).parent().unwrap().join("state.toml");
    let before = fs::read_to_string(&state).unwrap();
    let pack = format!(
        "# Conflict\n\nshared.txt\n\n# Slice\n\n<!-- {} -->\n",
        fx.slice_path(2).display()
    );
    fx.write_plan(&[
        json!({ "outcome": "done", "sh": "printf 'resolved\\n' > shared.txt && git add shared.txt" }),
        json!({ "outcome": "blocked" }),
        json!({ "outcome": "checkpoint" }),
    ]);

    let events = stream(
        &claude(&fx, &["--append-system-prompt", &pack])
            .output()
            .unwrap(),
    );
    assert_eq!(fx.read("shared.txt"), "resolved\n");
    let last = events.last().unwrap();
    assert!(
        last["result"].as_str().unwrap().starts_with("STATUS: done"),
        "{last}"
    );
    assert_eq!(fs::read_to_string(&state).unwrap(), before);

    let events = stream(
        &claude(&fx, &["--append-system-prompt", &pack])
            .output()
            .unwrap(),
    );
    let blocks = fx.log_of("block");
    assert_eq!(blocks.len(), 1, "{blocks:?}");
    assert_eq!(blocks[0]["slice"], 2, "{blocks:?}");
    assert!(
        blocks[0].get("phase").is_none_or(Value::is_null),
        "{blocks:?}"
    );
    assert_eq!(fx.slice_status(2), "pending");
    let last = events.last().unwrap();
    assert!(
        last["result"]
            .as_str()
            .unwrap()
            .starts_with("STATUS: blocked"),
        "{last}"
    );

    let out = claude(&fx, &["--append-system-prompt", &pack])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(65));
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("merge worker"),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
