//! `factory review second` and `factory review collect` against a stub `cursor-agent` replaying `tests/stubs/cursor-review.jsonl`.
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
    branch(
        name,
        timeout,
        &[("a", 300), ("b", 250), ("c", 700), ("d", 150), ("e", 100)],
    )
}

/// `feat/review` off `main`, committing each file with its number of lines.
fn branch(name: &str, timeout: u64, files: &[(&str, usize)]) -> Fixture {
    branch_off(name, timeout, &[], files)
}

/// `branch`, after committing each `on_main` file with its text on `main`.
fn branch_off(
    name: &str,
    timeout: u64,
    on_main: &[(&str, &str)],
    files: &[(&str, usize)],
) -> Fixture {
    let fx = Fixture::new(name);
    if !on_main.is_empty() {
        for (f, text) in on_main {
            fs::write(fx.path().join(f), text).unwrap();
            fx.git(&["add", f]);
        }
        fx.git(&["commit", "-q", "-m", "chore: on main"]);
    }
    fx.git(&["checkout", "-q", "-b", "feat/review"]);
    for (f, n) in files {
        let body: String = (1..=*n).map(|i| format!("{f} line {i}\n")).collect();
        let path = fx.path().join(f);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, body).unwrap();
        fx.git(&["add", f]);
    }
    fx.git(&["commit", "-q", "-m", "feat: the branch"]);
    configure(&fx, timeout);
    fx
}

/// `feat/review` off `main`, where `path` holds `before` on `main` and `after` on the branch.
fn edited(name: &str, path: &str, before: &str, after: &str) -> Fixture {
    let fx = Fixture::new(name);
    fs::write(fx.path().join(path), before).unwrap();
    fx.git(&["add", path]);
    fx.git(&["commit", "-q", "-m", "chore: on main"]);
    fx.git(&["checkout", "-q", "-b", "feat/review"]);
    fs::write(fx.path().join(path), after).unwrap();
    fx.git(&["commit", "-q", "-am", "feat: the branch"]);
    configure(&fx, 20);
    fx
}

fn configure(fx: &Fixture, timeout: u64) {
    let cfg = PathBuf::from(&fx.env()["FACTORY_CONFIG_DIR"]);
    fs::write(
        cfg.join("config.toml"),
        format!("[review]\ntimeout_secs = {timeout}\n"),
    )
    .unwrap();
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
    // Pass 1 finishes last, so printing in finishing order would break the order below.
    scenario(&fx, json!({"sleep_secs": {"1": 2, "2": 1, "3": 1}}));
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
fn a_timeout_is_unreviewed_and_keeps_its_partial_text() {
    let fx = fixture("review-timeout", 2);
    scenario(&fx, json!({"sleep_after_assistant": 2}));
    let started = Instant::now();
    let o = review(&fx, &["--diff", "main", "--", "a", "b"]);
    let took = started.elapsed();
    let out = stdout(&o);
    assert_eq!(o.status.code(), Some(2), "{out}");
    assert!(took < Duration::from_secs(10), "took {took:?}");
    assert!(out.contains("(timed out after 2 s, partial)"), "{out}");
    let partial = format!("{}{}", assistant_text(7), assistant_text(14));
    assert!(out.contains(&partial), "{out}");
    let third: String = assistant_text(15).chars().take(40).collect();
    assert!(!out.contains(&third), "{out}");
}

#[test]
fn a_timeout_beside_a_counted_pass_exits_0() {
    let fx = fixture("review-timeout-beside", 2);
    scenario(&fx, json!({"sleep_after_assistant": {"2": 2}}));
    let o = review(&fx, &["--diff", "main", "--", "a", "b", "d"]);
    let out = stdout(&o);
    assert_eq!(o.status.code(), Some(0), "{out}");
    assert!(section(&out, 1).contains("(replayed for pass 1)"), "{out}");
    let two = section(&out, 2);
    assert!(two.contains("(timed out after 2 s, partial)"), "{two}");
    assert!(two.contains(&assistant_text(7)), "{two}");
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

/// A zombie waiting to be reaped by its new parent counts as gone, and so does one
/// reaped between the two checks: its /proc entry is already unreadable.
fn alive(pid: i32) -> bool {
    // SAFETY: signal 0 only checks that the pid exists.
    if unsafe { libc::kill(pid, 0) } != 0 {
        return false;
    }
    fs::read_to_string(format!("/proc/{pid}/stat")).is_ok_and(|s| {
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

fn collect(fx: &Fixture, args: &[&str]) -> Output {
    fx.command(BIN)
        .args(["review", "collect"])
        .args(args)
        .output()
        .unwrap()
}

/// Runs the collect into `f.md` and returns the file.
fn collected(fx: &Fixture) -> String {
    let o = collect(fx, &["--base", "main", "--out", "f.md"]);
    assert_eq!(
        o.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    assert!(stdout(&o).is_empty(), "{}", stdout(&o));
    fx.read("f.md")
}

/// The second opinion is last and runs to the end: the findings hold headings of their own.
fn part<'a>(file: &'a str, heading: &str) -> &'a str {
    let start = file
        .find(&format!("\n## {heading}\n"))
        .unwrap_or_else(|| panic!("no {heading}: {file}"));
    let rest = &file[start + 1..];
    if heading == "Second opinion" {
        return rest;
    }
    let end = rest.find("\n## ").map_or(rest.len(), |e| e + 1);
    &rest[..end]
}

#[test]
fn collect_writes_the_head_the_plan_the_leaks_and_one_section_per_pass() {
    let fx = branch("collect-two", 20, &[("a", 400), ("b", 300)]);
    let file = collected(&fx);
    let head = fx.git(&["rev-parse", "HEAD"]);
    assert_eq!(
        file.lines().next(),
        Some(format!("<!-- head: {} -->", head.trim()).as_str())
    );
    let plan = part(&file, "Plan");
    assert!(
        plan.contains("changed: 2 files, 700 lines against main...HEAD\n"),
        "{plan}"
    );
    assert!(
        plan.contains("\n- a (400 lines)\n- b (300 lines)\n"),
        "{plan}"
    );
    assert!(
        plan.contains("\nsize: local review and second opinion\n"),
        "{plan}"
    );
    assert!(
        plan.contains("\nsecond opinion: reviewed, 2 of 2 passes counted\n"),
        "{plan}"
    );
    assert!(
        part(&file, "Leaks").contains("\nverdict: clean\n"),
        "{file}"
    );
    let second = part(&file, "Second opinion");
    assert!(second.contains("\nexit: 0\n"), "{second}");
    let passes: Vec<&str> = second
        .lines()
        .filter(|l| l.starts_with("### Pass"))
        .collect();
    assert_eq!(passes.len(), 2, "{second}");
    assert!(
        passes[0].starts_with("### Pass 1 — a (400 lines,"),
        "{}",
        passes[0]
    );
    assert!(
        passes[1].starts_with("### Pass 2 — b (300 lines,"),
        "{}",
        passes[1]
    );
    assert!(
        passes.iter().all(|p| p.ends_with(" — counted")),
        "{passes:?}"
    );
    let bodies: Vec<&str> = second.split("\n### Pass ").skip(1).collect();
    for (i, body) in bodies.iter().enumerate() {
        for m in 1..=2 {
            let tag = format!("(replayed for pass {m})");
            assert_eq!(
                body.contains(&tag),
                m == i + 1,
                "pass {} holding {tag}: {body}",
                i + 1
            );
        }
    }
    assert_eq!(review_calls(&fx).len(), 2);
}

#[test]
fn collect_logged_out_is_unreviewed_and_owes_the_fallback() {
    let fx = branch("collect-auth", 20, &[("a", 400), ("b", 300)]);
    scenario(&fx, json!({"list_models": "auth"}));
    let file = collected(&fx);
    let plan = part(&file, "Plan");
    assert!(
        plan.contains(
            "\nsecond opinion: fallback owed — unreviewed: cursor-agent --list-models exited 1: Error: Authentication required."
        ),
        "{plan}"
    );
    let second = part(&file, "Second opinion");
    assert!(
        second.contains("\nexit: 2\nunreviewed: cursor-agent --list-models exited 1: Error: Authentication required."),
        "{second}"
    );
    assert!(!second.contains("### Pass"), "{second}");
    assert_eq!(calls(&fx), [vec!["--list-models".to_string()]]);
}

#[test]
fn collect_skips_the_second_opinion_of_one_small_file_without_calling_the_reviewer() {
    let fx = branch("collect-small", 20, &[("a", 49)]);
    let file = collected(&fx);
    let plan = part(&file, "Plan");
    assert!(plan.contains("\n- a (49 lines)\n"), "{plan}");
    assert!(
        plan.contains("\nsize: local review only\nsecond opinion: skipped by size\n"),
        "{plan}"
    );
    assert!(
        part(&file, "Second opinion").contains("skipped by size: 1 file under 50 changed lines"),
        "{file}"
    );
    assert!(calls(&fx).is_empty(), "{:?}", calls(&fx));
}

#[test]
fn collect_without_a_base_or_with_a_dash_base_is_a_usage_error() {
    let fx = branch("collect-usage", 20, &[("a", 400), ("b", 300)]);
    for args in [&["--out", "f.md"][..], &["--base", "-x"], &[]] {
        let o = collect(&fx, args);
        assert_eq!(o.status.code(), Some(1), "{args:?}");
        assert!(
            String::from_utf8_lossy(&o.stderr)
                .contains("factory review collect --base <branch> [--out <file>]"),
            "{args:?}"
        );
    }
    assert!(!fx.path().join("f.md").exists());
    assert!(calls(&fx).is_empty());
}

#[test]
fn collect_owes_the_fallback_when_every_pass_timed_out() {
    let fx = branch("collect-timeout-one", 2, &[("a", 400), ("b", 300)]);
    scenario(&fx, json!({"sleep_after_assistant": {"2": 2}}));
    let file = collected(&fx);
    let plan = part(&file, "Plan");
    assert!(
        plan.contains("\nsecond opinion: reviewed, 1 of 2 passes counted\n"),
        "{plan}"
    );
    let second = part(&file, "Second opinion");
    let passes: Vec<&str> = second
        .lines()
        .filter(|l| l.starts_with("### Pass"))
        .collect();
    assert_eq!(passes.len(), 2, "{second}");
    assert!(passes[0].ends_with(" — counted"), "{}", passes[0]);
    assert!(passes[1].ends_with(" — unreviewed"), "{}", passes[1]);
    assert!(
        second.contains("(timed out after 2 s, partial)"),
        "{second}"
    );

    let fx = branch("collect-timeout-all", 2, &[("a", 400), ("b", 300)]);
    scenario(&fx, json!({"sleep_after_assistant": 2}));
    let file = collected(&fx);
    let plan = part(&file, "Plan");
    assert!(
        plan.contains("\nsecond opinion: fallback owed — unreviewed: no pass of 2 counted\n"),
        "{plan}"
    );
    let second = part(&file, "Second opinion");
    assert!(second.contains("\nexit: 2\n"), "{second}");
    assert_eq!(
        second.matches("(timed out after 2 s, partial)").count(),
        2,
        "{second}"
    );
    assert!(
        second
            .lines()
            .filter(|l| l.starts_with("### Pass"))
            .all(|l| l.ends_with(" — unreviewed")),
        "{second}"
    );
}

const LOG: &str = "docs/features/001-x/log.jsonl";
const STATE: &str = "docs/features/001-x/state.toml";
const VENDORED: &[(&str, &str)] = &[(".gitattributes", "testdata/** linguist-vendored\n")];

#[test]
fn collect_skips_tool_written_and_vendored_files_and_lists_them() {
    let fx = branch_off(
        "collect-skip",
        20,
        VENDORED,
        &[
            (LOG, 10),
            (STATE, 5),
            ("testdata/a.xsd", 30),
            ("src/a.rs", 400),
        ],
    );
    let file = collected(&fx);
    let plan = part(&file, "Plan");
    assert!(
        plan.contains(
            "changed: 1 file, 400 lines against main...HEAD\n- src/a.rs (400 lines)\nskipped:\n\
             - docs/features/001-x/log.jsonl (tool-written)\n\
             - docs/features/001-x/state.toml (tool-written)\n\
             - testdata/a.xsd (vendored)\nsize: "
        ),
        "{plan}"
    );
    assert!(
        plan.contains("\nsecond opinion: reviewed, 1 of 1 passes counted\n"),
        "{plan}"
    );
    let reviews = review_calls(&fx);
    assert_eq!(reviews.len(), 1, "{reviews:?}");
    let second = part(&file, "Second opinion");
    let dir = PathBuf::from(
        second
            .lines()
            .find_map(|l| l.strip_prefix("log: "))
            .unwrap_or_else(|| panic!("no log: {second}")),
    );
    let diff = fs::read_to_string(dir.join("review-1.diff")).unwrap();
    assert!(diff.contains("+++ b/src/a.rs"), "{diff}");
    for f in [LOG, STATE, "testdata/a.xsd"] {
        assert!(!diff.contains(f), "{f} in {diff}");
    }

    let fx = branch_off("collect-skip-all", 20, VENDORED, &[(LOG, 10), (STATE, 5)]);
    let file = collected(&fx);
    let plan = part(&file, "Plan");
    assert!(
        plan.contains("\nsize: local review only\nsecond opinion: skipped by size\n"),
        "{plan}"
    );
    assert!(
        part(&file, "Second opinion").contains(&format!(
            "skipped by size: no file left after skipping {LOG}, {STATE}\n"
        )),
        "{file}"
    );
    assert!(calls(&fx).is_empty(), "{:?}", calls(&fx));
}

#[test]
fn second_skips_the_same_files_and_lists_them_before_the_passes() {
    let fx = branch_off(
        "review-skip",
        20,
        VENDORED,
        &[(LOG, 10), ("testdata/a.xsd", 30), ("src/a.rs", 400)],
    );
    let o = review(&fx, &["--diff", "main"]);
    let out = stdout(&o);
    assert_eq!(o.status.code(), Some(0), "{out}");
    assert!(
        out.starts_with(&format!(
            "skipped:\n- {LOG} (tool-written)\n- testdata/a.xsd (vendored)\n\n## Pass 1 — src/a.rs (400 lines,"
        )),
        "{out}"
    );
    assert_eq!(review_calls(&fx).len(), 1);

    let fx = branch_off("review-skip-all", 20, VENDORED, &[(LOG, 10)]);
    let o = review(&fx, &["--diff", "main"]);
    let out = stdout(&o);
    assert_eq!(o.status.code(), Some(2), "{out}");
    assert_eq!(
        out,
        format!("unreviewed: no changes against main left after skipping {LOG}\n")
    );
    assert!(calls(&fx).is_empty(), "{:?}", calls(&fx));
}

/// The second opinion's pass headings and its log dir.
fn passes_and_dir(file: &str) -> (Vec<&str>, PathBuf) {
    let second = part(file, "Second opinion");
    let heads = second
        .lines()
        .filter(|l| l.starts_with("### Pass"))
        .collect();
    let dir = second
        .lines()
        .find_map(|l| l.strip_prefix("log: "))
        .unwrap_or_else(|| panic!("no log: {second}"));
    (heads, PathBuf::from(dir))
}

fn hunk_heads(diff: &str) -> Vec<&str> {
    diff.lines().filter(|l| l.starts_with("@@")).collect()
}

/// The `+` and `-` lines below the file header.
fn changes(diff: &str) -> usize {
    let hunks = &diff[diff
        .find("\n@@")
        .unwrap_or_else(|| panic!("no hunk: {diff}"))..];
    hunks
        .lines()
        .filter(|l| l.starts_with('+') || l.starts_with('-'))
        .count()
}

#[test]
fn a_huge_file_splits_at_hunks() {
    let blocks = |word: &str| -> String {
        (1..=10)
            .flat_map(|b| {
                let changed = (1..=100).map(move |i| format!("{word} {b} {i}\n"));
                changed.chain((1..=10).map(move |i| format!("keep {b} {i}\n")))
            })
            .collect()
    };
    let fx = edited("split-hunks", "big", &blocks("old"), &blocks("new"));
    let file = collected(&fx);
    assert!(
        part(&file, "Plan").contains("\n- big (2000 lines)\n"),
        "{file}"
    );
    let (heads, dir) = passes_and_dir(&file);
    assert_eq!(heads.len(), 4, "{file}");
    for (k, (head, lines)) in heads.iter().zip([600, 600, 600, 200]).enumerate() {
        let k = k + 1;
        let want = format!("### Pass {k} — big (part {k} of 4) ({lines} lines, ");
        assert!(head.starts_with(&want), "{head}");
        assert!(head.ends_with(" — counted"), "{head}");
    }
    assert_eq!(review_calls(&fx).len(), 4);

    let whole = fx.git(&["diff", "--no-renames", "main...HEAD", "--", "big"]);
    let header = &whole[..whole.find("\n@@").unwrap() + 1];
    let mut seen = Vec::new();
    for n in 1..=4 {
        let diff = fs::read_to_string(dir.join(format!("review-{n}.diff"))).unwrap();
        assert!(diff.starts_with(header), "part {n}: {diff}");
        assert!(changes(&diff) <= 600, "part {n}: {}", changes(&diff));
        for head in hunk_heads(&diff) {
            let at = whole.find(&format!("\n{head}\n")).unwrap();
            let rest = &whole[at + 1..];
            let hunk = &rest[..rest[1..].find("\n@@").map_or(rest.len(), |e| e + 2)];
            assert!(diff.contains(hunk), "part {n} cuts {head}: {diff}");
            seen.extend([head.to_string()]);
        }
    }
    assert_eq!(seen, hunk_heads(&whole), "{whole}");
    assert_eq!(seen.len(), 10);
}

#[test]
fn a_file_under_twice_the_cap_stays_whole() {
    for n in [900, 1200] {
        let fx = branch(&format!("split-under-{n}"), 20, &[("a", n)]);
        let file = collected(&fx);
        let (heads, dir) = passes_and_dir(&file);
        assert_eq!(heads.len(), 1, "{file}");
        let want = format!("### Pass 1 — a (over 600, alone) ({n} lines, ");
        assert!(heads[0].starts_with(&want), "{}", heads[0]);
        let diff = fs::read_to_string(dir.join("review-1.diff")).unwrap();
        assert_eq!(hunk_heads(&diff), [format!("@@ -0,0 +1,{n} @@")], "{diff}");
        assert_eq!(review_calls(&fx).len(), 1);
    }
}

#[test]
fn an_added_huge_file_splits_at_lines() {
    let fx = branch("split-lines", 20, &[("big", 1500)]);
    let file = collected(&fx);
    let (heads, dir) = passes_and_dir(&file);
    assert_eq!(heads.len(), 3, "{file}");
    let mut added = Vec::new();
    for k in 1..=3 {
        let want = format!("### Pass {k} — big (part {k} of 3) (500 lines, ");
        assert!(heads[k - 1].starts_with(&want), "{}", heads[k - 1]);
        let diff = fs::read_to_string(dir.join(format!("review-{k}.diff"))).unwrap();
        assert!(
            diff.starts_with("diff --git a/big b/big\nnew file mode "),
            "{diff}"
        );
        let first = 1 + 500 * (k - 1);
        assert_eq!(
            hunk_heads(&diff),
            [format!("@@ -0,0 +{first},500 @@")],
            "{diff}"
        );
        let hunk = &diff[diff.find("\n@@").unwrap() + 1..];
        added.extend(
            hunk.lines()
                .skip(1)
                .map(|l| format!("{}\n", l.strip_prefix('+').unwrap_or_else(|| panic!("{l}")))),
        );
    }
    let body: Vec<String> = (1..=1500).map(|i| format!("big line {i}\n")).collect();
    assert_eq!(added, body);
    assert_eq!(review_calls(&fx).len(), 3);
}
