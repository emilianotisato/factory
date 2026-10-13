//! PRD-004 FR-6, slice 04: `factory review second` against the stub
//! `cursor-agent`, which replays `tests/stubs/cursor-review.jsonl`.
#[path = "harness/mod.rs"]
mod harness;

use std::fs;
use std::path::PathBuf;
use std::process::Output;
use std::time::{Duration, Instant};

use harness::Fixture;
use serde_json::{Value, json};

const BIN: &str = env!("CARGO_BIN_EXE_factory");
const STREAM: &str = include_str!("stubs/cursor-review.jsonl");

/// The fixture on a branch adding `a` to `e`: 300, 250, 700, 150 and 100
/// lines over `main`, with `[review] timeout_secs` set.
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

/// The stub's argv, one array per call.
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

/// The `log: <dir>` line, which comes last.
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
    assert!(
        reviews[1]
            .last()
            .unwrap()
            .contains(&dir.join("review-2.diff").display().to_string())
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
