//! `review::skip` feature-doc filtering and shared signal forwarding.
#![cfg(unix)]
#[path = "harness/mod.rs"]
mod harness;

use std::fs;
use std::io::Write;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use harness::Fixture;

const BIN: &str = env!("CARGO_BIN_EXE_factory");
const LOG: &str = "docs/features/acme/log.jsonl";
const PRD: &str = "docs/features/acme/prd.md";
const BRIEF: &str = "docs/features/acme/brief.md";

fn configure(fx: &Fixture, timeout: u64) {
    let cfg = PathBuf::from(&fx.env()["FACTORY_CONFIG_DIR"]);
    fs::write(
        cfg.join("config.toml"),
        format!("[review]\ntimeout_secs = {timeout}\n"),
    )
    .unwrap();
}

fn branch_off(name: &str, timeout: u64, files: &[(&str, usize)]) -> Fixture {
    let fx = Fixture::new(name);
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

fn collected(fx: &Fixture) -> String {
    let o = fx
        .command(BIN)
        .args(["review", "collect", "--base", "main", "--out", "f.md"])
        .output()
        .unwrap();
    assert_eq!(
        o.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    fx.read("f.md")
}

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

fn review_calls(fx: &Fixture) -> Vec<Vec<String>> {
    let calls: Vec<Vec<String>> = match fs::read_to_string(fx.stub_dir().join("cursor-agent.log")) {
        Ok(t) => t
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect(),
        Err(_) => vec![],
    };
    calls
        .into_iter()
        .filter(|a| a.first().map(String::as_str) == Some("-p"))
        .collect()
}

#[test]
fn review_skip_drops_feature_docs_when_code_changed_but_keeps_docs_only_diffs() {
    let mixed = branch_off(
        "collect-feature-doc-mixed",
        20,
        &[(LOG, 1), (PRD, 10), (BRIEF, 10), ("src/a.rs", 400)],
    );
    let file = collected(&mixed);
    let plan = part(&file, "Plan");
    assert!(plan.contains(&format!("- {PRD} (feature-doc)")), "{plan}");
    assert!(plan.contains(&format!("- {BRIEF} (feature-doc)")), "{plan}");
    assert!(plan.contains(&format!("- {LOG} (tool-written)")), "{plan}");
    assert!(
        plan.contains("changed: 1 file, 400 lines against main...HEAD\n- src/a.rs (400 lines)"),
        "{plan}"
    );
    let second = part(&file, "Second opinion");
    let dir = PathBuf::from(
        second
            .lines()
            .find_map(|l| l.strip_prefix("log: "))
            .unwrap_or_else(|| panic!("no log: {second}")),
    );
    let diff = fs::read_to_string(dir.join("review-1.diff")).unwrap();
    assert!(diff.contains("+++ b/src/a.rs"), "{diff}");
    for f in [PRD, BRIEF, LOG] {
        assert!(!diff.contains(f), "{f} in {diff}");
    }

    let docs_only = branch_off(
        "collect-feature-doc-only",
        20,
        &[(LOG, 1), (PRD, 10), (BRIEF, 10)],
    );
    let file = collected(&docs_only);
    let plan = part(&file, "Plan");
    assert!(plan.contains(&format!("- {LOG} (tool-written)")), "{plan}");
    assert!(plan.contains(&format!("- {PRD} (10 lines)")), "{plan}");
    assert!(plan.contains(&format!("- {BRIEF} (10 lines)")), "{plan}");
    let reviews = review_calls(&docs_only);
    assert_eq!(reviews.len(), 1, "{reviews:?}");
    let second = part(&file, "Second opinion");
    let dir = PathBuf::from(
        second
            .lines()
            .find_map(|l| l.strip_prefix("log: "))
            .unwrap_or_else(|| panic!("no log: {second}")),
    );
    let diff = fs::read_to_string(dir.join("review-1.diff")).unwrap();
    assert!(diff.contains(&format!("+++ b/{PRD}")), "{diff}");
    assert!(diff.contains(&format!("+++ b/{BRIEF}")), "{diff}");
}

fn grandchild_gone(pid: i32) -> bool {
    // SAFETY: signal 0 only checks that the pid exists.
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
fn signals_unified_kills_both_child_groups() {
    let review = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/review.rs"));
    let checkpoint = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/checkpoint.rs"));
    assert!(
        !review.contains("sigaction"),
        "review.rs still calls sigaction"
    );
    assert!(
        !checkpoint.contains("sigaction"),
        "checkpoint.rs still calls sigaction"
    );

    if std::env::var_os("FACTORY_SIGNALS_TEST_CHILD").is_some() {
        let ready = std::env::var_os("FACTORY_SIGNALS_READY").expect("FACTORY_SIGNALS_READY");
        signals_test_child(Path::new(&ready));
        return;
    }

    let ready = tempfile::NamedTempFile::new().unwrap();
    let ready_path = ready.path().to_owned();
    let exe = std::env::current_exe().unwrap();
    let mut child = Command::new(&exe)
        .arg("--exact")
        .arg("signals_unified_kills_both_child_groups")
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
    // SAFETY: plain syscall on the helper subprocess.
    unsafe { libc::kill(child.id() as i32, libc::SIGTERM) };
    let status = child.wait().unwrap();
    assert_eq!(status.signal(), Some(libc::SIGTERM), "{status:?}");

    let deadline = Instant::now() + Duration::from_secs(5);
    while (!grandchild_gone(p1) || !grandchild_gone(p2)) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
    }
    for (label, pid) in [("sleep-1", p1), ("sleep-2", p2)] {
        assert!(grandchild_gone(pid), "{label} pid {pid} still alive");
    }
}
