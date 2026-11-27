//! Structured claims index in collect output and unanchored `# Collected`.
#![cfg(unix)]
#[path = "harness/mod.rs"]
mod harness;

use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

use factory::config::ResolvedReview;
use factory::provider::{Provider, ReviewProvider};
use harness::{FEATURE, FEATURE_DIR, Fixture};

const BIN: &str = env!("CARGO_BIN_EXE_factory");

fn branch_two_pass(name: &str) -> Fixture {
    let fx = Fixture::new(name);
    fx.git(&["checkout", "-q", "-b", "feat/review"]);
    for (f, n) in [("a", 400_usize), ("b", 300)] {
        let body: String = (1..=n).map(|i| format!("{f} line {i}\n")).collect();
        fs::write(fx.path().join(f), body).unwrap();
        fx.git(&["add", f]);
    }
    fx.git(&["commit", "-q", "-m", "feat: the branch"]);
    let cfg = fx.env()["FACTORY_CONFIG_DIR"].clone();
    fs::write(
        format!("{cfg}/config.toml"),
        "[review]\ntimeout_secs = 20\n",
    )
    .unwrap();
    fx
}

fn write_claims_stub(bin: &Path) {
    fs::create_dir_all(bin).unwrap();
    let script = r#"#!/usr/bin/env node
'use strict'
const argv = process.argv.slice(2)
if (argv.includes('--list-models')) {
  console.log('Available models\n\ngrok-4.7-high - Grok 4.7 High')
  process.exit(0)
}
const m = /review-(\d+)\.diff/.exec(argv[argv.length - 1] ?? '')
const pass = m ? Number(m[1]) : 1
const prose = process.env.CLAIMS_STUB_MODE === 'prose'
let text
if (prose) {
  text = 'Plain review with no summary table.\n'
} else if (pass === 1) {
  text = [
    'Pass one prose.',
    '',
    '### Summary',
    '',
    '| file:line | severity | claim |',
    '|---|---|---|',
    '| src/a.rs:1 | low | low claim pass1 |',
    '| src/b.rs:2 | medium | medium claim pass1 |',
    '',
  ].join('\n')
} else {
  text = [
    'Pass two prose.',
    '',
    '### Summary',
    '',
    '| file:line | severity | claim |',
    '|---|---|---|',
    '| src/c.rs:3 | high | high claim pass2 |',
    '| src/d.rs:4 | low | low claim pass2 |',
    '',
  ].join('\n')
}
const assistant = {
  type: 'assistant',
  message: { role: 'assistant', content: [{ type: 'text', text }] },
}
const result = { type: 'result', result: text }
process.stdout.write(`${JSON.stringify(assistant)}\n${JSON.stringify(result)}\n`)
"#;
    fs::write(bin.join("cursor-agent"), script).unwrap();
    fs::set_permissions(bin.join("cursor-agent"), fs::Permissions::from_mode(0o755)).unwrap();
}

fn collect_with_stub(fx: &Fixture, mode: Option<&str>) -> String {
    let bin = fx.stub_dir().parent().unwrap().join("bin");
    write_claims_stub(&bin);
    let path = format!("{}:{}", bin.display(), fx.env()["PATH"]);
    let mut cmd = fx.command(BIN);
    cmd.env("PATH", &path);
    if let Some(m) = mode {
        cmd.env("CLAIMS_STUB_MODE", m);
    }
    let o = cmd
        .args(["review", "collect", "--base", "main", "--out", "collect.md"])
        .output()
        .unwrap();
    assert_eq!(
        o.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    fx.read("collect.md")
}

fn second_opinion(file: &str) -> &str {
    let start = file
        .find("\n## Second opinion\n")
        .expect("second opinion section");
    &file[start + 1..]
}

#[test]
fn collect_merges_structured_claims_index_sorted_by_severity() {
    let fx = branch_two_pass("claims-merge");
    let file = collect_with_stub(&fx, None);
    let second = second_opinion(&file);
    let idx = second.find("### Claims index").expect("claims index");
    let pass1 = second.find("### Pass 1").expect("pass 1");
    assert!(idx < pass1, "claims index must precede pass 1:\n{second}");

    let table = &second[idx..pass1];
    let rows: Vec<&str> = table.lines().filter(|l| l.starts_with("| high")).collect();
    assert_eq!(rows.len(), 1);
    assert!(rows[0].contains("| 2 |"));
    assert!(rows[0].contains("high claim pass2"));

    let medium = table
        .lines()
        .find(|l| l.starts_with("| medium"))
        .expect("medium row");
    assert!(medium.contains("medium claim pass1"));
    assert!(medium.contains("| 1 |"));

    let lows: Vec<&str> = table.lines().filter(|l| l.starts_with("| low")).collect();
    assert_eq!(lows.len(), 2);

    let packed = factory::pack::collected(&file, Path::new("collect.md"));
    assert!(packed.contains("- Pass 1"));
    assert!(packed.contains("- Pass 2"));
    assert!(!packed.contains("### Claims index"));
    assert!(!packed.contains("high claim pass2"));
    assert!(!packed.contains("medium claim pass1"));
    assert!(!packed.contains("low claim pass1"));
    assert!(!packed.contains("low claim pass2"));

    let fx_prose = branch_two_pass("claims-prose");
    let prose_file = collect_with_stub(&fx_prose, Some("prose"));
    let prose = second_opinion(&prose_file);
    assert!(!prose.contains("### Claims index"));
    assert!(prose.contains("### Pass 1"));
    assert!(prose.contains("Plain review with no summary table."));
}

fn plan_section(file: &str) -> &str {
    let start = file.find("\n## Plan\n").expect("plan");
    let end = file[start..].find("\n## Leaks\n").expect("leaks");
    &file[start..start + end]
}

fn write_cause_stub(bin: &Path) {
    fs::create_dir_all(bin).unwrap();
    let script = r#"#!/usr/bin/env node
'use strict'
const argv = process.argv.slice(2)
if (argv.includes('--list-models')) {
  const models = process.env.LIST_MODELS ?? 'grok-4.7-high - Grok 4.7 High'
  console.log(`Available models\n\n${models}`)
  process.exit(0)
}
const msg = process.env.STUB_FAILURE ?? ''
const assistant = { type: 'assistant', message: { role: 'assistant', content: [{ type: 'text', text: 'reviewing' }] } }
const result = msg
  ? { type: 'result', is_error: true, result: msg }
  : { type: 'result', is_error: false, result: 'ok' }
process.stdout.write(`${JSON.stringify(assistant)}\n${JSON.stringify(result)}\n`)
process.exit(msg ? 1 : 0)
"#;
    fs::write(bin.join("cursor-agent"), script).unwrap();
    fs::set_permissions(bin.join("cursor-agent"), fs::Permissions::from_mode(0o755)).unwrap();
}

fn collect_with_env(fx: &Fixture, extra_args: &[&str], env: &[(&str, &str)]) -> String {
    let bin = fx.stub_dir().parent().unwrap().join("causes-bin");
    write_cause_stub(&bin);
    let path = format!("{}:{}", bin.display(), fx.env()["PATH"]);
    let mut cmd = fx.command(BIN);
    cmd.env("PATH", &path);
    for (k, v) in env {
        cmd.env(k, v);
    }
    let mut args = vec!["review", "collect", "--base", "main", "--out", "collect.md"];
    args.extend(extra_args);
    let o = cmd.args(&args).output().unwrap();
    assert_eq!(
        o.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    fx.read("collect.md")
}

fn review_settled(fx: &Fixture, feature: &str) -> Output {
    fx.command(BIN)
        .args(["review", "settled", feature])
        .output()
        .unwrap()
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

#[test]
fn collect_classifies_unreviewed_causes_and_since_ignores_merged_main_and_uses_reviewed_head() {
    let fx = branch_two_pass("causes-classify");
    for (msg, want) in [
        (
            "You've hit your usage limit",
            "usage limit (no pass of 2 counted)",
        ),
        ("Authentication required", "auth (no pass of 2 counted)"),
        ("model not found", "no such model (no pass of 2 counted)"),
    ] {
        let file = collect_with_env(&fx, &[], &[("STUB_FAILURE", msg)]);
        let plan = plan_section(&file);
        assert!(
            plan.contains(&format!("fallback owed — unreviewed: {want}")),
            "{msg}: {plan}"
        );
    }

    let probe_dir = tempfile::tempdir().unwrap();
    let stub_bin = probe_dir.path().join("bin");
    write_cause_stub(&stub_bin);
    let cfg = ResolvedReview {
        provider: ReviewProvider::Provider(Provider::Cursor),
        binary: Some(stub_bin.join("cursor-agent").to_string_lossy().into_owned()),
        model: "grok-4.7-high".into(),
        effort: None,
        timeout_secs: 20,
        pass_lines: 600,
    };
    // SAFETY: single-threaded test; cleared before the next case on the same thread.
    unsafe {
        std::env::set_var(
            "LIST_MODELS",
            "model-429-quota - Quota model\ngrok-4.7-high - Grok 4.7 High",
        );
    }
    assert!(
        factory::review::probe_resolved(&cfg, probe_dir.path(), None).is_ok(),
        "probe must accept model-429-quota in the listing"
    );

    let fx2 = Fixture::new("since-intersect");
    fx2.git(&["checkout", "-q", "-b", "feat/review"]);
    let lines = |f: &str| -> String { (1..=100).map(|i| format!("{f} line {i}\n")).collect() };
    fs::write(fx2.path().join("seed.txt"), lines("seed")).unwrap();
    fx2.git(&["add", "seed.txt"]);
    fx2.git(&["commit", "-q", "-m", "feat: seed"]);
    let r = fx2.git(&["rev-parse", "HEAD"]).trim().to_string();
    fs::write(fx2.path().join("polish.txt"), lines("polish")).unwrap();
    fx2.git(&["add", "polish.txt"]);
    fx2.git(&["commit", "-q", "-m", "fix: polish after review stamp"]);
    fx2.docs(&[
        "set",
        FEATURE,
        "reviewed=2026-11-26",
        &format!("reviewed_head={r}"),
    ]);
    fx2.git(&["add", "-A"]);
    fx2.git(&["commit", "-q", "-m", "docs: reviewed with head"]);
    let cfg_dir = PathBuf::from(&fx2.env()["FACTORY_CONFIG_DIR"]);
    fs::write(cfg_dir.join("config.toml"), "[review]\ntimeout_secs = 20\n").unwrap();
    fx2.git(&["checkout", "-q", "main"]);
    fs::write(fx2.path().join("main-only.txt"), "only on main\n").unwrap();
    fx2.git(&["add", "main-only.txt"]);
    fx2.git(&["commit", "-q", "-m", "chore: main only"]);
    fx2.git(&["checkout", "-q", "feat/review"]);
    fx2.git(&["merge", "-q", "--no-edit", "main"]);
    fs::write(
        fx2.path().join("polish.txt"),
        format!("{}{}", lines("polish"), "extra line\n"),
    )
    .unwrap();
    fs::write(fx2.path().join("big-a.txt"), lines("a")).unwrap();
    fx2.git(&["add", "-A"]);
    fx2.git(&["commit", "-q", "-m", "fix: more polish"]);
    let file = collect_with_env(
        &fx2,
        &["--since", &r],
        &[("STUB_FAILURE", "Authentication required")],
    );
    let plan = plan_section(&file);
    assert!(plan.contains("polish.txt"), "{plan}");
    assert!(!plan.contains("main-only.txt"), "{plan}");
    assert_eq!(
        factory::settled::last_reviewed(fx2.path(), FEATURE),
        Some(r.clone())
    );
    fx2.docs(&["set", FEATURE, "reviewed="]);
    fx2.git(&["add", "-A"]);
    fx2.git(&["commit", "-q", "-m", "docs: clear reviewed"]);
    let state = fs::read_to_string(fx2.path().join(FEATURE_DIR).join("state.toml")).unwrap();
    assert!(!state.contains("reviewed_head"));
    assert_eq!(factory::settled::last_reviewed(fx2.path(), FEATURE), None);

    let o = fx2
        .command(BIN)
        .args([
            "review",
            "collect",
            "--base",
            "main",
            "--since",
            "deadbeefdeadbeef",
            "--out",
            "f.md",
        ])
        .output()
        .unwrap();
    assert_eq!(
        o.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    let fallback = fx2.read("f.md");
    let plan = plan_section(&fallback);
    assert!(plan.contains("main...HEAD"), "{plan}");

    let skill =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("skills/polish/SKILL.md"))
            .unwrap();
    assert_eq!(skill.matches("reviewed_head").count(), 1);
    assert_eq!(skill.matches("### Claims index").count(), 1);
}

fn log_stdin(fx: &Fixture, args: &[&str], body: &str) -> Output {
    let mut child = fx
        .command(BIN)
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
        .write_all(body.as_bytes())
        .unwrap();
    child.wait_with_output().expect("wait factory docs")
}

fn grandchild_gone(pid: i32) -> bool {
    // SAFETY: pid existence check only.
    unsafe { libc::kill(pid, 0) != 0 }
}

#[allow(clippy::zombie_processes)]
fn signals_test_child(ready: &Path) {
    factory::signals::install();
    let g1 = factory::signals::Guard::claim().expect("guard 1");
    let g2 = factory::signals::Guard::claim().expect("guard 2");
    let mut s1 = Command::new("sleep");
    s1.arg("60").process_group(0);
    let c1 = s1.spawn().expect("sleep 1");
    let mut s2 = Command::new("sleep");
    s2.arg("60").process_group(0);
    let c2 = s2.spawn().expect("sleep 2");
    let p1 = c1.id() as i32;
    let p2 = c2.id() as i32;
    g1.set(p1);
    g2.set(p2);
    let mut f = fs::File::create(ready).expect("ready");
    writeln!(f, "{p1}\n{p2}").unwrap();
    drop(f);
    loop {
        std::thread::sleep(Duration::from_secs(60));
    }
}

#[test]
fn settled_includes_answered_asks_prints_via_cli_and_signals_are_unified() {
    let review = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/review.rs"));
    let checkpoint = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/checkpoint.rs"));
    assert!(!review.contains("sigaction"));
    assert!(!checkpoint.contains("sigaction"));

    if std::env::var_os("FACTORY_SIGNALS_TEST_CHILD").is_some() {
        let ready = std::env::var_os("FACTORY_SIGNALS_READY").expect("FACTORY_SIGNALS_READY");
        signals_test_child(Path::new(&ready));
        return;
    }

    let fx = Fixture::new("settled-signals");
    log_stdin(
        &fx,
        &[
            "log",
            "add",
            FEATURE,
            "--kind",
            "decision",
            "--title",
            "Keep the queue",
            "--body-file",
            "-",
        ],
        "**Chose:** in memory\n",
    );
    fx.docs(&[
        "log",
        "add",
        FEATURE,
        "--kind",
        "ask",
        "--title",
        "Ship without Redis?",
    ]);
    let ask_id = fx.log_of("ask")[0]["id"]
        .as_str()
        .expect("ask id")
        .to_string();
    log_stdin(
        &fx,
        &[
            "log",
            "add",
            FEATURE,
            "--kind",
            "answer",
            "--ref",
            &ask_id,
            "--title",
            "a",
            "--body-file",
            "-",
        ],
        "lane-local only\n",
    );
    fx.docs(&[
        "log",
        "add",
        FEATURE,
        "--kind",
        "ask",
        "--title",
        "Still open",
    ]);

    let want = factory::settled::for_feature(fx.path(), FEATURE).expect("settled block");
    let o = review_settled(&fx, FEATURE);
    assert_eq!(o.status.code(), Some(0), "{}", stdout(&o));
    assert_eq!(stdout(&o), want);
    assert!(want.contains(&format!(
        "{ask_id} — Ship without Redis?: answered — lane-local only"
    )));
    assert!(!want.contains("Still open"));

    let bad = review_settled(&fx, "999");
    assert_eq!(bad.status.code(), Some(1));

    let skill =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("skills/polish/SKILL.md"))
            .unwrap();
    assert_eq!(skill.matches("factory review settled <NNN>").count(), 1);

    let ready = tempfile::NamedTempFile::new().unwrap();
    let ready_path = ready.path().to_owned();
    let exe = std::env::current_exe().unwrap();
    let mut child = Command::new(&exe)
        .arg("--exact")
        .arg("settled_includes_answered_asks_prints_via_cli_and_signals_are_unified")
        .env("FACTORY_SIGNALS_TEST_CHILD", "1")
        .env("FACTORY_SIGNALS_READY", &ready_path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();

    let deadline = Instant::now() + Duration::from_secs(10);
    let (p1, p2) = loop {
        if let Ok(t) = fs::read_to_string(&ready_path) {
            let lines: Vec<&str> = t.lines().filter(|l| !l.is_empty()).collect();
            if lines.len() >= 2
                && let (Ok(a), Ok(b)) = (lines[0].parse::<i32>(), lines[1].parse::<i32>())
            {
                break (a, b);
            }
        }
        assert!(Instant::now() < deadline, "child never wrote ready file");
        std::thread::sleep(Duration::from_millis(20));
    };
    std::thread::sleep(Duration::from_millis(100));
    // SAFETY: signal the helper subprocess under test.
    unsafe { libc::kill(child.id() as i32, libc::SIGTERM) };
    let status = child.wait().unwrap();
    assert_eq!(status.signal(), Some(libc::SIGTERM), "{status:?}");
    let deadline = Instant::now() + Duration::from_secs(5);
    while (!grandchild_gone(p1) || !grandchild_gone(p2)) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(grandchild_gone(p1), "sleep-1 pid {p1} still alive");
    assert!(grandchild_gone(p2), "sleep-2 pid {p2} still alive");
}
