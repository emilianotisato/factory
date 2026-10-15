//! `factory review second` against a stub `cursor-agent` replaying `tests/stubs/cursor-review.jsonl`.
#[path = "harness/mod.rs"]
mod harness;

use std::fs;
use std::os::unix::process::ExitStatusExt;
use std::path::PathBuf;
use std::process::{Output, Stdio};
use std::time::{Duration, Instant};

use harness::Fixture;
use serde_json::{Value, json};

const BIN: &str = env!("CARGO_BIN_EXE_factory");
const STREAM: &str = include_str!("stubs/cursor-review.jsonl");

fn fixture(name: &str, timeout: u64) -> Fixture {
    let fx = Fixture::new(name);
    fx.git(&["checkout", "-q", "-b", "feat/review"]);
    for (f, n) in [("a", 300), ("b", 250), ("c", 700), ("d", 150), ("e", 100)] {
        let body: String = (1..=n).map(|i| format!("{f} line {i}\n")).collect();
        fs::write(fx.path().join(f), body).unwrap();
    }
    fx.git(&["add", "a", "b", "c", "d", "e"]);
    fx.git(&["commit", "-q", "-m", "feat: five files"]);
    let cfg = PathBuf::from(&fx.env()["FACTORY_CONFIG_DIR"]);
    fs::write(
        cfg.join("config.toml"),
        format!("[review]\ntimeout_secs = {timeout}\n"),
    )
    .unwrap();
    fx
}

fn scenario(fx: &Fixture, v: Value) {
    fs::write(fx.stub_dir().join("cursor-agent.json"), v.to_string()).unwrap();
}

fn review(fx: &Fixture, args: &[&str]) -> Output {
    fx.command(BIN)
        .args(["review", "second"])
        .args(args)
        .output()
        .unwrap()
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn calls(fx: &Fixture) -> Vec<Vec<String>> {
    match fs::read_to_string(fx.stub_dir().join("cursor-agent.log")) {
        Ok(t) => t
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect(),
        Err(_) => Vec::new(),
    }
}

fn review_calls(fx: &Fixture) -> Vec<Vec<String>> {
    calls(fx)
        .into_iter()
        .filter(|a| a.first().map(String::as_str) == Some("-p"))
        .collect()
}

fn log_dir(out: &str) -> PathBuf {
    let last = out.lines().last().expect("output");
    PathBuf::from(
        last.strip_prefix("log: ")
            .unwrap_or_else(|| panic!("last line: {out}")),
    )
}

fn assistant_text(n: usize) -> String {
    let v: Value = serde_json::from_str(STREAM.lines().nth(n - 1).unwrap()).unwrap();
    v["message"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .to_string()
}

#[test]
fn the_stub_answers_list_models_and_replays_the_recorded_stream() {
    let fx = fixture("review-stub", 20);
    let stub = harness::stubs_dir().join("cursor-agent");
    let list = fx.command(&stub).arg("--list-models").output().unwrap();
    assert_eq!(list.status.code(), Some(0));
    assert!(stdout(&list).contains("grok-4.7-high"));
    scenario(&fx, json!({"list_models": "auth"}));
    let auth = fx.command(&stub).arg("--list-models").output().unwrap();
    assert_eq!(auth.status.code(), Some(1));
    assert!(stdout(&auth).contains("Authentication required"));
    fs::remove_file(fx.stub_dir().join("cursor-agent.json")).unwrap();
    let call = fx.command(&stub).args(["-p", "review"]).output().unwrap();
    assert_eq!(call.status.code(), Some(0));
    assert!(call.stdout == STREAM.as_bytes(), "the replay differs");
    assert_eq!(calls(&fx).len(), 3);
}

#[test]
fn a_diff_is_three_passes_and_the_oversized_file_is_alone() {
    let fx = fixture("review-diff", 20);
    let o = review(&fx, &["--diff", "main"]);
    let out = stdout(&o);
    assert_eq!(o.status.code(), Some(0), "{out}");
    let headers: Vec<&str> = out.lines().filter(|l| l.starts_with("## Pass")).collect();
    assert_eq!(headers.len(), 3, "{out}");
    assert!(
        headers[0].starts_with("## Pass 1 — a, b (550 lines,"),
        "{}",
        headers[0]
    );
    assert!(
        headers[1].starts_with("## Pass 2 — c (over 600, alone) (700 lines,"),
        "{}",
        headers[1]
    );
    assert!(
        headers[2].starts_with("## Pass 3 — d, e (250 lines,"),
        "{}",
        headers[2]
    );
    assert!(
        headers.iter().all(|h| h.ends_with(", grok-4.7-high)")),
        "{headers:?}"
    );
    let result: Value = serde_json::from_str(STREAM.lines().last().unwrap()).unwrap();
    let first = result["result"].as_str().unwrap().lines().next().unwrap();
    assert_eq!(out.matches(first).count(), 3, "{out}");

    let reviews = review_calls(&fx);
    assert_eq!(reviews.len(), 3);
    for argv in &reviews {
        for flag in ["--trust", "--output-format", "--stream-partial-output"] {
            assert!(argv.iter().any(|a| a == flag), "{flag} in {argv:?}");
        }
        let joined = argv.join(" ");
        assert!(joined.contains("--mode ask"), "{joined}");
        assert!(joined.contains("--output-format stream-json"), "{joined}");
        assert!(joined.contains("--model grok-4.7-high"), "{joined}");
    }
    let dir = log_dir(&out);
    for n in 1..=3 {
        for ext in ["diff", "jsonl"] {
            let f = dir.join(format!("review-{n}.{ext}"));
            assert!(f.is_file(), "{} missing", f.display());
        }
    }
    let d2 = fs::read_to_string(dir.join("review-2.diff")).unwrap();
    assert!(d2.contains("+++ b/c") && !d2.contains("+++ b/a"), "{d2}");
    let d2_path = dir.join("review-2.diff").display().to_string();
    assert_eq!(
        reviews
            .iter()
            .filter(|a| a.last().unwrap().contains(&d2_path))
            .count(),
        1,
        "{reviews:?}"
    );
}

#[test]
fn the_passes_run_at_once_and_print_in_pass_order() {
    let fx = fixture("review-parallel", 20);
    scenario(&fx, json!({"sleep_secs": 2}));
    let started = Instant::now();
    let o = review(&fx, &["--diff", "main"]);
    let took = started.elapsed();
    let out = stdout(&o);
    assert_eq!(o.status.code(), Some(0), "{out}");
    assert!(took < Duration::from_secs(4), "took {took:?}");
    let at = |s: &str| out.find(s).unwrap_or_else(|| panic!("no {s:?}: {out}"));
    let order = [
        at("## Pass 1 "),
        at("## Pass 2 "),
        at("## Pass 3 "),
        at("\nlog: "),
    ];
    assert!(order.is_sorted(), "{out}");
    for n in 1..=3 {
        let s = section(&out, n);
        for m in 1..=3 {
            let tag = format!("(replayed for pass {m})");
            assert_eq!(s.contains(&tag), m == n, "pass {n} holding {tag}: {s}");
        }
    }
    let err = String::from_utf8_lossy(&o.stderr);
    for n in 1..=3 {
        assert!(err.contains(&format!("review pass {n}/3: ")), "{err}");
    }
    let headers: Vec<&str> = out.lines().filter(|l| l.starts_with("## Pass")).collect();
    println!(
        "three passes of 2 s each took {took:?}\n{}",
        headers.join("\n")
    );
}

#[test]
fn paths_after_the_separator_scope_the_diff_to_one_pass() {
    let fx = fixture("review-scoped", 20);
    let o = review(&fx, &["--diff", "main", "--", "a", "b"]);
    let out = stdout(&o);
    assert_eq!(o.status.code(), Some(0), "{out}");
    assert_eq!(out.matches("## Pass").count(), 1, "{out}");
    assert!(out.contains("## Pass 1 — a, b (550 lines,"), "{out}");
    assert_eq!(review_calls(&fx).len(), 1);
}

#[test]
fn a_dead_auth_or_a_missing_model_spends_nothing_and_exits_2() {
    for (mode, reason) in [
        ("auth", "Authentication required"),
        ("no-model", "model grok-4.7-high not in --list-models"),
    ] {
        let fx = fixture(&format!("review-{mode}"), 20);
        scenario(&fx, json!({"list_models": mode}));
        let o = review(&fx, &["--diff", "main"]);
        let out = stdout(&o);
        assert_eq!(o.status.code(), Some(2), "{mode}: {out}");
        assert!(out.starts_with("unreviewed: "), "{mode}: {out}");
        assert!(
            out.lines().next().unwrap().contains(reason),
            "{mode}: {out}"
        );
        assert!(!out.contains("## Pass"), "{mode}: {out}");
        assert_eq!(calls(&fx), [vec!["--list-models".to_string()]], "{mode}");
        assert!(log_dir(&out).join("list-models.txt").is_file());
    }
}

#[test]
fn a_timeout_keeps_the_partial_text() {
    let fx = fixture("review-timeout", 2);
    scenario(&fx, json!({"sleep_after_assistant": 2}));
    let started = Instant::now();
    let o = review(&fx, &["--diff", "main", "--", "a", "b"]);
    let took = started.elapsed();
    let out = stdout(&o);
    assert_eq!(o.status.code(), Some(0), "{out}");
    assert!(took < Duration::from_secs(10), "took {took:?}");
    assert!(out.contains("(timed out after 2 s, partial)"), "{out}");
    let partial = format!("{}{}", assistant_text(7), assistant_text(14));
    assert!(out.contains(&partial), "{out}");
    let third: String = assistant_text(15).chars().take(40).collect();
    assert!(!out.contains(&third), "{out}");
}

#[test]
fn doc_mode_is_one_pass_with_the_prompt_and_the_path() {
    let fx = fixture("review-doc", 20);
    let doc = format!("{}/prd.md", harness::FEATURE_DIR);
    let o = review(
        &fx,
        &["--doc", &doc, "--prompt", "find what cannot be true"],
    );
    let out = stdout(&o);
    assert_eq!(o.status.code(), Some(0), "{out}");
    let lines = fx.read(&doc).lines().count();
    assert!(
        out.contains(&format!("## Pass 1 — {doc} ({lines} lines,")),
        "{out}"
    );
    assert_eq!(out.matches("## Pass").count(), 1, "{out}");
    let reviews = review_calls(&fx);
    assert_eq!(reviews.len(), 1);
    let prompt = reviews[0].last().unwrap();
    assert!(prompt.starts_with("find what cannot be true"), "{prompt}");
    let abs = fx.path().canonicalize().unwrap().join(&doc);
    assert!(prompt.contains(&abs.display().to_string()), "{prompt}");
    assert!(!log_dir(&out).join("review-1.diff").exists());
}

#[test]
fn others_go_into_every_pass_prompt() {
    let fx = fixture("review-others", 20);
    let notes = fx.path().join("notes.txt");
    fs::write(&notes, "src/x.rs:3 unchecked unwrap on user input\n").unwrap();
    let o = review(
        &fx,
        &["--diff", "main", "--others", notes.to_str().unwrap()],
    );
    assert_eq!(o.status.code(), Some(0), "{}", stdout(&o));
    let reviews = review_calls(&fx);
    assert_eq!(reviews.len(), 3);
    for argv in &reviews {
        let prompt = argv.last().unwrap();
        assert!(
            prompt.contains("src/x.rs:3 unchecked unwrap on user input"),
            "{prompt}"
        );
        assert!(
            prompt.contains("what would fixing those still miss?"),
            "{prompt}"
        );
    }
}

#[test]
fn a_bad_combination_is_a_usage_error_with_exit_1_and_no_call() {
    let fx = fixture("review-usage", 20);
    for args in [
        &["--doc", "x"][..],
        &["--diff", "main", "--doc", "x", "--prompt", "y"],
        &["--prompt", "y"],
        &[],
    ] {
        let o = review(&fx, args);
        assert_eq!(o.status.code(), Some(1), "{args:?}");
        assert!(String::from_utf8_lossy(&o.stderr).contains("usage: factory review second"));
    }
    assert!(calls(&fx).is_empty());
}

fn section(out: &str, n: usize) -> &str {
    let start = out
        .find(&format!("## Pass {n} "))
        .unwrap_or_else(|| panic!("no pass {n}: {out}"));
    let rest = &out[start..];
    let end = rest[1..]
        .find("\n## Pass ")
        .or_else(|| rest.find("\nlog: "))
        .map_or(rest.len(), |e| e + 1);
    &rest[..end]
}

#[test]
fn a_pass_that_errors_is_unreviewed_and_the_others_still_run() {
    let fx = fixture("review-pass-error", 20);
    scenario(&fx, json!({"unlink_stream_on_call": 2}));
    let o = review(&fx, &["--diff", "main"]);
    let out = stdout(&o);
    assert_eq!(o.status.code(), Some(0), "{out}");
    let two = section(&out, 2);
    assert!(two.contains("\n\nunreviewed (error: read "), "{two}");
    assert!(two.contains("review-2.jsonl"), "{two}");
    let result: Value = serde_json::from_str(STREAM.lines().last().unwrap()).unwrap();
    let first = result["result"].as_str().unwrap().lines().next().unwrap();
    assert!(section(&out, 1).contains(first), "{out}");
    assert!(section(&out, 3).contains(first), "{out}");
    assert!(log_dir(&out).is_dir(), "{out}");

    let fx = fixture("review-pass-error-only", 20);
    scenario(&fx, json!({"unlink_stream_on_call": 1}));
    let o = review(&fx, &["--diff", "main", "--", "a", "b"]);
    let out = stdout(&o);
    assert_eq!(o.status.code(), Some(2), "{out}");
    assert!(section(&out, 1).contains("unreviewed (error: "), "{out}");
    assert!(log_dir(&out).is_dir(), "{out}");
}

#[test]
fn a_non_zero_exit_marks_its_partial_and_does_not_count() {
    let fx = fixture("review-exit-partial", 20);
    scenario(&fx, json!({"exit_after_assistant": 1, "exit_code": 3}));
    let o = review(&fx, &["--diff", "main", "--", "a", "b"]);
    let out = stdout(&o);
    assert_eq!(o.status.code(), Some(2), "{out}");
    let want = format!("(exit 3, partial)\n\n{}\n", assistant_text(7));
    assert!(section(&out, 1).contains(&want), "{out}");
    assert!(log_dir(&out).is_dir(), "{out}");
}

#[test]
fn an_error_result_is_unreviewed_with_its_reason() {
    let fx = fixture("review-error-result", 20);
    scenario(
        &fx,
        json!({"exit_after_assistant": 1, "error_result": "rate limit reached"}),
    );
    let o = review(&fx, &["--diff", "main", "--", "a", "b"]);
    let out = stdout(&o);
    assert_eq!(o.status.code(), Some(2), "{out}");
    let one = section(&out, 1);
    assert!(
        one.contains("\n\nunreviewed (exit 1, error: rate limit reached)\n"),
        "{one}"
    );
    assert!(one.contains("(exit 1, partial)"), "{one}");
}

#[test]
fn a_base_starting_with_a_dash_is_a_usage_error() {
    let fx = fixture("review-dash-base", 20);
    let target = fx.path().join("written-by-git");
    let o = review(&fx, &[&format!("--diff=--output={}", target.display())]);
    assert_eq!(o.status.code(), Some(1), "{}", stdout(&o));
    assert!(String::from_utf8_lossy(&o.stderr).contains("usage: factory review second"));
    assert!(!target.exists());
    assert!(calls(&fx).is_empty());
}

/// A zombie waiting to be reaped by its new parent counts as gone.
fn alive(pid: i32) -> bool {
    // SAFETY: signal 0 only checks that the pid exists.
    if unsafe { libc::kill(pid, 0) } != 0 {
        return false;
    }
    fs::read_to_string(format!("/proc/{pid}/stat")).map_or(true, |s| {
        !s.rsplit(')')
            .next()
            .unwrap_or("")
            .trim_start()
            .starts_with('Z')
    })
}

#[test]
fn a_signal_to_factory_kills_every_reviewer_too() {
    let fx = fixture("review-signal", 60);
    scenario(&fx, json!({"sleep_after_assistant": 1}));
    let mut child = fx
        .command(BIN)
        .args(["review", "second", "--diff", "main"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    let pids: Vec<i32> = (1..=3)
        .map(|n| {
            let pid_file = fx.stub_dir().join(format!("cursor-agent-{n}.pid"));
            loop {
                if let Ok(t) = fs::read_to_string(&pid_file)
                    && let Ok(p) = t.trim().parse::<i32>()
                {
                    break p;
                }
                assert!(
                    Instant::now() < deadline,
                    "the stub for pass {n} never started"
                );
                std::thread::sleep(Duration::from_millis(20));
            }
        })
        .collect();
    std::thread::sleep(Duration::from_millis(200));
    // SAFETY: a plain syscall on the child this test spawned.
    unsafe { libc::kill(child.id() as i32, libc::SIGTERM) };
    let status = child.wait().unwrap();
    assert_eq!(status.signal(), Some(libc::SIGTERM), "{status:?}");
    let deadline = Instant::now() + Duration::from_secs(5);
    while pids.iter().any(|&p| alive(p)) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
    }
    let left: Vec<i32> = pids.into_iter().filter(|&p| alive(p)).collect();
    for &p in &left {
        // SAFETY: as above; a stray stub must not outlive the test.
        unsafe { libc::kill(p, libc::SIGKILL) };
    }
    assert!(left.is_empty(), "the reviewers {left:?} outlived factory");
}
