//! Every child writes to files, never a pipe: an undrained 34 KB stream blocks the reviewer.
use std::fs::{self, File};
use std::io::Write;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicI32, Ordering};
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::config::Review;
use crate::error::io;
use crate::{Error, Result, clock, paths};

/// Without these the agent exits on trust, may write, buffers to the end, or leaves nothing when killed.
pub const FLAGS: [&str; 6] = [
    "--trust",
    "--mode",
    "ask",
    "--output-format",
    "stream-json",
    "--stream-partial-output",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pass {
    pub files: Vec<String>,
    pub lines: u64,
    /// One file over the cap, alone in its pass.
    pub over: bool,
}

pub fn group(files: &[(String, u64)], cap: u64) -> Vec<Pass> {
    let mut out = Vec::new();
    let mut cur = Pass {
        files: Vec::new(),
        lines: 0,
        over: false,
    };
    for (file, n) in files {
        if !cur.files.is_empty() && (*n > cap || cur.lines + n > cap) {
            out.push(std::mem::replace(
                &mut cur,
                Pass {
                    files: Vec::new(),
                    lines: 0,
                    over: false,
                },
            ));
        }
        cur.files.push(file.clone());
        cur.lines += n;
        if *n > cap {
            cur.over = true;
            out.push(std::mem::replace(
                &mut cur,
                Pass {
                    files: Vec::new(),
                    lines: 0,
                    over: false,
                },
            ));
        }
    }
    if !cur.files.is_empty() {
        out.push(cur);
    }
    out
}

pub fn parse_numstat(text: &str) -> Vec<(String, u64)> {
    text.split('\0')
        .filter_map(|rec| {
            let mut it = rec.trim_start_matches('\n').splitn(3, '\t');
            let (a, d, path) = (it.next()?, it.next()?, it.next()?);
            let n = |s: &str| s.parse::<u64>().unwrap_or(0);
            (!path.is_empty()).then(|| (path.to_string(), n(a) + n(d)))
        })
        .collect()
}

/// Paths come back relative to the repo root whatever `cwd` is.
pub fn numstat(cwd: &Path, base: &str, paths: &[String]) -> Result<Vec<(String, u64)>> {
    let range = format!("{base}...HEAD");
    let out = git(
        cwd,
        &[
            &["diff", "--numstat", "-z", "--no-renames", &range, "--"],
            &str_refs(paths)[..],
        ]
        .concat(),
    )?;
    Ok(parse_numstat(&out))
}

pub fn diff(root: &Path, base: &str, files: &[String]) -> Result<String> {
    let range = format!("{base}...HEAD");
    git(
        root,
        &[
            &["diff", "--no-renames", &range, "--"],
            &str_refs(files)[..],
        ]
        .concat(),
    )
}

fn str_refs(v: &[String]) -> Vec<&str> {
    v.iter().map(String::as_str).collect()
}

fn git(cwd: &Path, args: &[&str]) -> Result<String> {
    let cmd = format!("git {}", args.join(" "));
    let out = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| Error::Tool {
            cmd: cmd.clone(),
            detail: format!("could not start git: {e}"),
        })?;
    if !out.status.success() {
        return Err(Error::Tool {
            cmd,
            detail: String::from_utf8_lossy(&out.stderr).trim().to_string(),
        });
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

pub fn new_log_dir(data_dir: &Path) -> Result<PathBuf> {
    let parent = std::path::absolute(data_dir.join("reviews"))
        .map_err(io(format!("resolve {}", data_dir.display())))?;
    paths::private_dir_all(&parent).map_err(io(format!("mkdir {}", parent.display())))?;
    let stamp: String = clock::rfc3339(clock::wall_now())
        .chars()
        .filter(char::is_ascii_digit)
        .collect();
    let stamp = format!("{}-{}", &stamp[..8], &stamp[8..]);
    for n in 1.. {
        let dir = if n == 1 {
            parent.join(&stamp)
        } else {
            parent.join(format!("{stamp}-{n}"))
        };
        match fs::create_dir(&dir) {
            Ok(()) => return Ok(dir),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(io(format!("mkdir {}", dir.display()))(e)),
        }
    }
    unreachable!()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ended {
    Exited(Option<i32>),
    Killed,
}

/// Each running reviewer's own process group: a Ctrl-C reaches factory alone, so the handler must kill them.
const SLOTS: usize = 256;
static CHILD_GROUPS: [AtomicI32; SLOTS] = [const { AtomicI32::new(0) }; SLOTS];

/// Holds a slot from before the spawn (as -1, which the handler skips) until the child is done.
struct Watched(&'static AtomicI32);

impl Watched {
    fn claim() -> Result<Watched> {
        CHILD_GROUPS
            .iter()
            .find(|s| {
                s.compare_exchange(0, -1, Ordering::SeqCst, Ordering::SeqCst)
                    .is_ok()
            })
            .map(Watched)
            .ok_or_else(|| Error::Msg(format!("more than {SLOTS} reviewers at once")))
    }
}

impl Drop for Watched {
    fn drop(&mut self) {
        self.0.store(0, Ordering::SeqCst);
    }
}

extern "C" fn forward(sig: libc::c_int) {
    // SAFETY: atomics and async-signal-safe calls only; the re-raise under SIG_DFL ends factory as the signal would.
    unsafe {
        for slot in &CHILD_GROUPS {
            let group = slot.load(Ordering::SeqCst);
            if group > 0 {
                libc::kill(-group, libc::SIGKILL);
            }
        }
        libc::signal(sig, libc::SIG_DFL);
        libc::raise(sig);
    }
}

/// A signal factory started out ignoring (`nohup`) stays ignored.
fn forward_signals() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        for sig in [libc::SIGINT, libc::SIGTERM, libc::SIGHUP] {
            // SAFETY: a zeroed `sigaction` is valid; the handler is async-signal-safe.
            unsafe {
                let mut old: libc::sigaction = std::mem::zeroed();
                libc::sigaction(sig, std::ptr::null(), &mut old);
                if old.sa_sigaction == libc::SIG_IGN {
                    continue;
                }
                let mut sa: libc::sigaction = std::mem::zeroed();
                sa.sa_sigaction = forward as extern "C" fn(libc::c_int) as libc::sighandler_t;
                libc::sigemptyset(&mut sa.sa_mask);
                libc::sigaction(sig, &sa, std::ptr::null_mut());
            }
        }
    });
}

fn run_logged(
    mut cmd: Command,
    out: &Path,
    err: &Path,
    timeout: Duration,
) -> Result<(Ended, Duration)> {
    let file = |p: &Path| File::create(p).map_err(io(format!("create {}", p.display())));
    cmd.stdin(Stdio::null())
        .stdout(Stdio::from(file(out)?))
        .stderr(Stdio::from(file(err)?))
        // Its own group: a wrapper script's children die with it.
        .process_group(0);
    forward_signals();
    let watched = Watched::claim()?;
    let started = Instant::now();
    let mut child = cmd.spawn().map_err(|e| {
        Error::Msg(format!(
            "could not start {}: {e}",
            cmd.get_program().to_string_lossy()
        ))
    })?;
    watched.0.store(child.id() as i32, Ordering::SeqCst);
    loop {
        if let Some(status) = child.try_wait().map_err(io("poll the reviewer"))? {
            return Ok((Ended::Exited(status.code()), started.elapsed()));
        }
        if started.elapsed() >= timeout {
            // SAFETY: a plain syscall on the group this child leads.
            unsafe { libc::kill(-(child.id() as i32), libc::SIGKILL) };
            let _ = child.kill();
            let _ = child.wait();
            return Ok((Ended::Killed, started.elapsed()));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

pub fn probe(cfg: &Review, dir: &Path) -> std::result::Result<(), String> {
    let out = dir.join("list-models.txt");
    let mut cmd = Command::new(&cfg.agent);
    cmd.arg("--list-models");
    let timeout = Duration::from_secs(cfg.timeout_secs);
    let ended = match run_logged(cmd, &out, &out.with_extension("err"), timeout) {
        Ok((ended, _)) => ended,
        Err(e) => return Err(e.to_string()),
    };
    let text = [out.clone(), out.with_extension("err")]
        .iter()
        .filter_map(|p| fs::read_to_string(p).ok())
        .collect::<Vec<_>>()
        .join("\n");
    let first = text
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("no output")
        .to_string();
    match ended {
        Ended::Killed => Err(format!(
            "{} --list-models timed out after {} s",
            cfg.agent, cfg.timeout_secs
        )),
        Ended::Exited(code) if code != Some(0) => Err(format!(
            "{} --list-models exited {}: {first}",
            cfg.agent,
            code.map_or("on a signal".to_string(), |c| c.to_string())
        )),
        _ if text.contains("login") || text.contains("Authentication required") => Err(first),
        _ if !lists_model(&text, &cfg.model) => {
            Err(format!("model {} not in --list-models", cfg.model))
        }
        _ => Ok(()),
    }
}

/// Punctuation is trimmed off each token: the listing's format was never seen working.
pub fn lists_model(text: &str, model: &str) -> bool {
    let id = |c: char| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_');
    text.split_whitespace()
        .any(|t| t.trim_matches(|c| !id(c)) == model)
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Parsed {
    pub result: Option<String>,
    /// The reason a run failed, never findings.
    pub error: Option<String>,
    pub partial: String,
}

impl Parsed {
    pub fn text(&self) -> Option<&str> {
        match &self.result {
            Some(r) if !r.trim().is_empty() => Some(r),
            _ if !self.partial.trim().is_empty() => Some(&self.partial),
            _ => None,
        }
    }
}

pub fn parse_stream(text: &str) -> Parsed {
    let mut p = Parsed::default();
    for line in text.lines() {
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        match v["type"].as_str() {
            Some("result") if v["is_error"] == true => {
                p.error = Some(v["result"].as_str().unwrap_or_default().to_string());
            }
            Some("result") => {
                if let Some(r) = v["result"].as_str() {
                    p.result = Some(r.to_string());
                }
            }
            Some("assistant") => {
                for item in v["message"]["content"].as_array().into_iter().flatten() {
                    if item["type"] == "text"
                        && let Some(t) = item["text"].as_str()
                    {
                        p.partial.push_str(t);
                    }
                }
            }
            _ => {}
        }
    }
    p
}

#[derive(Debug, Clone)]
pub struct Outcome {
    pub ended: Ended,
    pub elapsed: Duration,
    pub parsed: Parsed,
    /// The first line of its stderr, for an `unreviewed` with no text.
    pub stderr: String,
}

pub fn run_pass(cfg: &Review, cwd: &Path, dir: &Path, n: usize, prompt: &str) -> Result<Outcome> {
    let stream = dir.join(format!("review-{n}.jsonl"));
    let err = dir.join(format!("review-{n}.err"));
    let mut cmd = Command::new(&cfg.agent);
    cmd.arg("-p")
        .args(FLAGS)
        .args(["--model", &cfg.model])
        .arg(prompt)
        .current_dir(cwd);
    let (ended, elapsed) = run_logged(cmd, &stream, &err, Duration::from_secs(cfg.timeout_secs))?;
    let text = fs::read(&stream).map_err(io(format!("read {}", stream.display())))?;
    let stderr = fs::read_to_string(&err)
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or_default()
        .to_string();
    Ok(Outcome {
        ended,
        elapsed,
        parsed: parse_stream(&String::from_utf8_lossy(&text)),
        stderr,
    })
}

/// One pass's outcome, or why it never ran, and how long it took.
#[derive(Debug)]
pub struct Ran {
    pub outcome: Result<Outcome>,
    pub elapsed: Duration,
}

/// Starts every pass at once, writing each one's progress line to `progress` as it starts; returns in pass order.
pub fn run_passes(
    cfg: &Review,
    cwd: &Path,
    dir: &Path,
    passes: &[Pass],
    prompt: impl Fn(usize, &Pass) -> Result<String> + Sync,
    progress: &mut dyn Write,
) -> Result<Vec<Ran>> {
    std::thread::scope(|s| {
        let mut running = Vec::with_capacity(passes.len());
        for (i, pass) in passes.iter().enumerate() {
            let n = i + 1;
            writeln!(
                progress,
                "review pass {n}/{}: {} ({} lines)",
                passes.len(),
                pass.files.join(", "),
                pass.lines
            )
            .map_err(io("stderr"))?;
            let prompt = &prompt;
            running.push(s.spawn(move || {
                let started = Instant::now();
                let outcome = prompt(n, pass).and_then(|p| run_pass(cfg, cwd, dir, n, &p));
                let elapsed = outcome.as_ref().map_or(started.elapsed(), |o| o.elapsed);
                Ran { outcome, elapsed }
            }));
        }
        Ok(running
            .into_iter()
            .map(|h| h.join().unwrap_or_else(|p| std::panic::resume_unwind(p)))
            .collect())
    })
}

/// A pass's text and whether it counted; a pass that never ran is unreviewed with why.
pub fn pass_text(ran: &Ran, timeout_secs: u64) -> (String, bool) {
    match &ran.outcome {
        Ok(o) => body(o, timeout_secs),
        Err(e) => (format!("unreviewed (error: {e})\n"), false),
    }
}

/// `Pass n — files (lines, seconds, model)`, the heading of a pass's findings.
pub fn pass_header(n: usize, pass: &Pass, ran: &Ran, cfg: &Review) -> String {
    let over = if pass.over {
        format!(" (over {}, alone)", cfg.pass_lines)
    } else {
        String::new()
    };
    format!(
        "Pass {n} — {}{over} ({} lines, {} s, {})",
        pass.files.join(", "),
        pass.lines,
        ran.elapsed.as_secs(),
        cfg.model
    )
}

/// Writes the pass's slice of `<base>...HEAD` into the log dir and asks the reviewer to read it.
pub fn diff_pass_prompt(
    root: &Path,
    base: &str,
    dir: &Path,
    n: usize,
    pass: &Pass,
) -> Result<String> {
    let file = dir.join(format!("review-{n}.diff"));
    fs::write(&file, diff(root, base, &pass.files)?)
        .map_err(io(format!("write {}", file.display())))?;
    Ok(diff_prompt(&pass.files, base, &file))
}

fn exit_words(code: Option<i32>) -> String {
    code.map_or("killed by a signal".to_string(), |c| format!("exit {c}"))
}

/// A kill's partial counts (it had the whole timeout); an exit's does not, or an opening line stops the fallback.
fn body(o: &Outcome, timeout_secs: u64) -> (String, bool) {
    let p = &o.parsed;
    if let Some(r) = p.result.as_deref().filter(|r| !r.trim().is_empty()) {
        return (format!("{}\n", r.trim_end()), true);
    }
    let partial = Some(p.partial.trim_end()).filter(|t| !t.trim().is_empty());
    let code = match o.ended {
        Ended::Killed => {
            return match partial {
                Some(t) => (
                    format!("(timed out after {timeout_secs} s, partial)\n\n{t}\n"),
                    true,
                ),
                None => (
                    format!("unreviewed (timed out after {timeout_secs} s with no text)\n"),
                    false,
                ),
            };
        }
        Ended::Exited(code) => code,
    };
    let words = exit_words(code);
    let mut text = match &p.error {
        Some(e) if e.trim().is_empty() => format!("unreviewed ({words}, error result)\n"),
        Some(e) => format!("unreviewed ({words}, error: {})\n", e.trim()),
        None if code == Some(0) => match partial {
            Some(t) => return (format!("{t}\n"), true),
            None => String::new(),
        },
        None => String::new(),
    };
    match partial {
        Some(t) => {
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str(&format!("({words}, partial)\n\n{t}\n"));
        }
        None if text.is_empty() => {
            let why = if o.stderr.is_empty() {
                String::new()
            } else {
                format!(": {}", o.stderr)
            };
            text = format!("unreviewed ({words}, no text{why})\n");
        }
        None => {}
    }
    (text, false)
}

pub fn head(cwd: &Path) -> Result<String> {
    Ok(git(cwd, &["rev-parse", "HEAD"])?.trim().to_string())
}

pub fn diff_prompt(files: &[String], base: &str, diff_file: &Path) -> String {
    format!(
        "Adversarial code review. The diff of {} against {base} is in {}: read it, and read any file in the repository you need for context. Report only defects you can point to at file:line with a failure scenario; no praise, no style. Do not edit anything.",
        files.join(" "),
        diff_file.display()
    )
}

pub fn doc_prompt(text: &str, doc: &Path) -> String {
    format!(
        "{}\n\nThe document is {}: read it, and read any file in the repository you need for context. Do not edit anything.",
        text.trim_end(),
        doc.display()
    )
}

pub fn with_others(prompt: &str, others: &str) -> String {
    format!(
        "{prompt}\n\nThe other reviewers found:\n{}\n\nBesides your own review: what would fixing those still miss?",
        others.trim_end()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const STREAM: &str = include_str!("../tests/stubs/cursor-review.jsonl");

    const OPENING: &str = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"I'll read the diff."}]}}"#;
    const DONE: &str = r#"{"type":"result","is_error":false,"result":"a.rs:1 breaks"}"#;
    const FAILED: &str = r#"{"type":"result","is_error":true,"result":"rate limited"}"#;

    fn outcome(ended: Ended, stream: &[&str], stderr: &str) -> Outcome {
        Outcome {
            ended,
            elapsed: Duration::ZERO,
            parsed: parse_stream(&stream.join("\n")),
            stderr: stderr.to_string(),
        }
    }

    #[test]
    fn each_ending_has_its_marker_and_only_text_that_finished_counts() {
        let cases = [
            (
                Ended::Exited(Some(0)),
                vec![OPENING, DONE],
                "a.rs:1 breaks\n",
                true,
            ),
            (
                Ended::Exited(Some(1)),
                vec![OPENING, DONE],
                "a.rs:1 breaks\n",
                true,
            ),
            (
                Ended::Exited(Some(0)),
                vec![OPENING],
                "I'll read the diff.\n",
                true,
            ),
            (
                Ended::Killed,
                vec![OPENING],
                "(timed out after 9 s, partial)\n\nI'll read the diff.\n",
                true,
            ),
            (
                Ended::Exited(Some(1)),
                vec![OPENING],
                "(exit 1, partial)\n\nI'll read the diff.\n",
                false,
            ),
            (
                Ended::Exited(None),
                vec![OPENING],
                "(killed by a signal, partial)\n\nI'll read the diff.\n",
                false,
            ),
            (
                Ended::Exited(Some(1)),
                vec![FAILED],
                "unreviewed (exit 1, error: rate limited)\n",
                false,
            ),
            (
                Ended::Exited(Some(0)),
                vec![OPENING, FAILED],
                "unreviewed (exit 0, error: rate limited)\n\n(exit 0, partial)\n\nI'll read the diff.\n",
                false,
            ),
            (
                Ended::Exited(Some(2)),
                vec![],
                "unreviewed (exit 2, no text: boom)\n",
                false,
            ),
            (
                Ended::Killed,
                vec![],
                "unreviewed (timed out after 9 s with no text)\n",
                false,
            ),
        ];
        for (ended, stream, want, counts) in cases {
            let got = body(&outcome(ended, &stream, "boom"), 9);
            assert_eq!(got, (want.to_string(), counts), "{ended:?} {stream:?}");
        }
    }

    fn sized(ns: &[u64]) -> Vec<(String, u64)> {
        ns.iter()
            .enumerate()
            .map(|(i, n)| (((b'a' + i as u8) as char).to_string(), *n))
            .collect()
    }

    fn names(p: &Pass) -> Vec<&str> {
        p.files.iter().map(String::as_str).collect()
    }

    #[test]
    fn files_group_in_order_and_an_oversized_one_is_alone() {
        let passes = group(&sized(&[300, 250, 700, 150, 100]), 600);
        let got: Vec<_> = passes.iter().map(|p| (names(p), p.lines, p.over)).collect();
        assert_eq!(
            got,
            [
                (vec!["a", "b"], 550, false),
                (vec!["c"], 700, true),
                (vec!["d", "e"], 250, false)
            ]
        );
        let one = group(&sized(&[601]), 600);
        assert_eq!(one.len(), 1);
        assert!(one[0].over && names(&one[0]) == ["a"]);
        assert!(!group(&sized(&[600]), 600)[0].over);
    }

    #[test]
    fn numstat_counts_added_and_deleted_and_a_binary_as_zero() {
        let text = [
            "3\t2\tsrc/a.rs",
            "-\t-\tlogo.png",
            "10\t0\tdir/with space.md",
            "",
        ]
        .join("\0");
        assert_eq!(
            parse_numstat(&text),
            [
                ("src/a.rs".to_string(), 5),
                ("logo.png".to_string(), 0),
                ("dir/with space.md".to_string(), 10)
            ]
        );
    }

    fn line(n: usize) -> Value {
        serde_json::from_str(STREAM.lines().nth(n - 1).unwrap()).unwrap()
    }

    fn text_of(n: usize) -> String {
        line(n)["message"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .to_string()
    }

    #[test]
    fn the_recorded_stream_parses_to_its_result() {
        let p = parse_stream(STREAM);
        assert_eq!(p.result.as_deref(), line(16)["result"].as_str());
        assert_eq!(p.text(), line(16)["result"].as_str());
    }

    #[test]
    fn a_stream_cut_before_its_result_keeps_the_assistant_text() {
        let first = |n: usize| STREAM.lines().take(n).collect::<Vec<_>>().join("\n");
        let p = parse_stream(&first(14));
        assert_eq!(p.result, None);
        assert_eq!(
            p.text(),
            Some(format!("{}{}", text_of(7), text_of(14)).as_str())
        );
        // The three assistant texts joined are the result: the join is right.
        assert_eq!(parse_stream(&first(15)).text(), line(16)["result"].as_str());
        assert_eq!(parse_stream("not json\n{}\n").text(), None);
    }

    #[test]
    fn the_model_is_found_with_punctuation_around_it() {
        for text in [
            "grok-4.7-high - Grok 4.7 High",
            "auto, grok-4.7-high, gpt-5",
            "* grok-4.7-high (current)",
            "[grok-4.7-high]",
        ] {
            assert!(lists_model(text, "grok-4.7-high"), "{text}");
        }
        for text in ["grok-4.7-high-max", "xgrok-4.7-high", "grok-4.7", ""] {
            assert!(!lists_model(text, "grok-4.7-high"), "{text}");
        }
    }
}
