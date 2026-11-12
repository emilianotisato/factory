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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Pass {
    pub files: Vec<String>,
    pub lines: u64,
    /// One file over the cap, alone in its pass.
    pub over: bool,
    /// The slice of its one file's diff this pass covers, when the file was split.
    pub part: Option<Part>,
}

/// Part `k` of `n` of one file's diff, with its own copy of the file header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Part {
    pub k: usize,
    pub n: usize,
    pub diff: String,
}

pub fn group(files: &[(String, u64)], cap: u64) -> Vec<Pass> {
    let mut out = Vec::new();
    let mut cur = Pass::default();
    for (file, n) in files {
        if !cur.files.is_empty() && (*n > cap || cur.lines + n > cap) {
            out.push(std::mem::take(&mut cur));
        }
        cur.files.push(file.clone());
        cur.lines += n;
        if *n > cap {
            cur.over = true;
            out.push(std::mem::take(&mut cur));
        }
    }
    if !cur.files.is_empty() {
        out.push(cur);
    }
    out
}

/// `group`'s passes, each file over twice `cap` replaced by one pass per part of its `<base>...HEAD` diff.
/// A file whose diff cannot be read stays whole, so its pass reports the error.
pub fn plan(root: &Path, base: &str, files: &[(String, u64)], cap: u64) -> Vec<Pass> {
    let mut out = Vec::new();
    for pass in group(files, cap) {
        let parts = if pass.over && pass.lines > 2 * cap {
            diff(root, base, &pass.files)
                .map(|text| split_diff(&text, cap))
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        if parts.len() < 2 {
            out.push(pass);
            continue;
        }
        let n = parts.len();
        for (i, (diff, lines)) in parts.into_iter().enumerate() {
            let part = Part { k: i + 1, n, diff };
            out.push(Pass {
                files: pass.files.clone(),
                lines,
                over: false,
                part: Some(part),
            });
        }
    }
    out
}

/// One file's diff as parts of whole hunks packed up to `cap` changed lines, each headed by the file header,
/// with each part's changed lines. A hunk over `cap` is first cut into even chunks of at most `cap`.
pub fn split_diff(text: &str, cap: u64) -> Vec<(String, u64)> {
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let starts: Vec<usize> = (0..lines.len())
        .filter(|&i| lines[i].starts_with("@@"))
        .collect();
    let Some(&first) = starts.first() else {
        return vec![(text.to_string(), 0)];
    };
    let header = lines[..first].concat();
    let mut parts: Vec<(String, u64)> = Vec::new();
    for (i, &start) in starts.iter().enumerate() {
        let end = starts.get(i + 1).copied().unwrap_or(lines.len());
        for (hunk, n) in cut_hunk(lines[start], &lines[start + 1..end], cap) {
            match parts.last_mut() {
                Some((text, m)) if *m + n <= cap => {
                    text.push_str(&hunk);
                    *m += n;
                }
                _ => parts.push((format!("{header}{hunk}"), n)),
            }
        }
    }
    parts
}

fn is_change(line: &str) -> bool {
    line.starts_with('+') || line.starts_with('-')
}

fn changed(lines: &[&str]) -> u64 {
    lines.iter().filter(|l| is_change(l)).count() as u64
}

/// A hunk over `cap` changed lines as chunks with recomputed `@@` headers; a `\ No newline` line stays with the
/// line before it, and context after a chunk's last change stays with that chunk.
fn cut_hunk(head: &str, body: &[&str], cap: u64) -> Vec<(String, u64)> {
    let total = changed(body);
    let starts = hunk_starts(head);
    let (Some((mut old, mut new, tail)), true) = (starts, total > cap && cap > 0) else {
        return vec![(format!("{head}{}", body.concat()), total)];
    };
    let size = total.div_ceil(total.div_ceil(cap));
    let mut chunks: Vec<Vec<&str>> = vec![Vec::new()];
    let mut n = 0;
    for line in body {
        if is_change(line) {
            if n == size {
                chunks.push(Vec::new());
                n = 0;
            }
            n += 1;
        }
        chunks.last_mut().expect("one chunk at least").push(line);
    }
    let mut out = Vec::new();
    for (i, chunk) in chunks.iter().enumerate() {
        let olds = chunk
            .iter()
            .filter(|l| !l.starts_with('+') && !l.starts_with('\\'))
            .count() as u64;
        let news = chunk
            .iter()
            .filter(|l| !l.starts_with('-') && !l.starts_with('\\'))
            .count() as u64;
        // An empty side names the line before it, as git writes `-0,0` for an added file.
        let at = |pos: u64, count: u64| {
            if count == 0 {
                pos.saturating_sub(1)
            } else {
                pos
            }
        };
        let tail = if i == 0 { tail } else { "" };
        out.push((
            format!(
                "@@ -{},{olds} +{},{news} @@{tail}\n{}",
                at(old, olds),
                at(new, news),
                chunk.concat()
            ),
            changed(chunk),
        ));
        old += olds;
        new += news;
    }
    out
}

/// The first old and new line a hunk covers, and the text after its closing `@@`.
fn hunk_starts(head: &str) -> Option<(u64, u64, &str)> {
    let (old, rest) = head.strip_prefix("@@ -")?.split_once(" +")?;
    let (new, tail) = rest.split_once(" @@")?;
    let first = |range: &str| -> Option<u64> {
        let (start, count) = match range.split_once(',') {
            Some((s, c)) => (s.parse::<u64>().ok()?, c.parse::<u64>().ok()?),
            None => (range.parse::<u64>().ok()?, 1),
        };
        Some(if count == 0 { start + 1 } else { start })
    };
    Some((first(old)?, first(new)?, tail.trim_end_matches('\n')))
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

/// Changed files split into the ones a reviewer reads and the ones nobody does.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Skip {
    pub kept: Vec<(String, u64)>,
    /// Each skipped path with why: `tool-written`, `vendored` or `generated`.
    pub skipped: Vec<(String, &'static str)>,
    /// Why vendored and generated files went unchecked, when `git check-attr` failed.
    pub unchecked: Option<String>,
}

impl Skip {
    /// The `skipped:` block, one `- path (reason)` per file; empty when nothing was skipped or unchecked.
    pub fn lines(&self) -> String {
        if self.skipped.is_empty() && self.unchecked.is_none() {
            return String::new();
        }
        let mut out = match &self.unchecked {
            Some(e) => format!("skipped: vendored and generated files not checked ({e})\n"),
            None => "skipped:\n".to_string(),
        };
        for (path, why) in &self.skipped {
            out.push_str(&format!("- {} ({why})\n", one_line(path)));
        }
        out
    }

    /// The skipped paths, comma-separated.
    pub fn names(&self) -> String {
        let paths: Vec<String> = self.skipped.iter().map(|(p, _)| p.clone()).collect();
        file_list(&paths)
    }
}

/// Drops the files factory writes itself and the ones `.gitattributes` in `root` marks vendored or generated.
pub fn skip(root: &Path, files: Vec<(String, u64)>) -> Skip {
    let mut out = Skip::default();
    let checked: Vec<&str> = files
        .iter()
        .map(|(p, _)| p.as_str())
        .filter(|p| !tool_written(p))
        .collect();
    let marked = if checked.is_empty() {
        Vec::new()
    } else {
        check_attr(root, &checked).unwrap_or_else(|e| {
            out.unchecked = Some(e.to_string());
            Vec::new()
        })
    };
    for (path, n) in files {
        if tool_written(&path) {
            out.skipped.push((path, "tool-written"));
        } else if let Some((_, why)) = marked.iter().find(|(p, _)| *p == path) {
            out.skipped.push((path, why));
        } else {
            out.kept.push((path, n));
        }
    }
    out
}

/// `docs/features/<dir>/log.jsonl` or `state.toml`, one directory level deep.
fn tool_written(path: &str) -> bool {
    matches!(
        path.split('/').collect::<Vec<_>>()[..],
        ["docs", "features", dir, "log.jsonl" | "state.toml"] if !dir.is_empty()
    )
}

/// The paths among `paths` whose `linguist-vendored` or `linguist-generated` is set, read from the checkout.
fn check_attr(root: &Path, paths: &[&str]) -> Result<Vec<(String, &'static str)>> {
    let cmd = "git check-attr -z --stdin linguist-vendored linguist-generated".to_string();
    let fail = |detail: String| Error::Tool {
        cmd: cmd.clone(),
        detail,
    };
    let mut child = Command::new("git")
        .args([
            "check-attr",
            "-z",
            "--stdin",
            "linguist-vendored",
            "linguist-generated",
        ])
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| fail(format!("could not start git: {e}")))?;
    let input: Vec<u8> = paths.iter().flat_map(|p| p.bytes().chain([0])).collect();
    let mut stdin = child.stdin.take().expect("stdin is piped");
    // A long list fills the pipe both ways: the writer runs beside the reader.
    let writer = std::thread::spawn(move || stdin.write_all(&input));
    let out = child
        .wait_with_output()
        .map_err(|e| fail(format!("could not read git: {e}")))?;
    let written = writer.join().expect("the stdin writer does not panic");
    if !out.status.success() {
        return Err(fail(
            String::from_utf8_lossy(&out.stderr).trim().to_string(),
        ));
    }
    written.map_err(|e| fail(format!("could not write to git: {e}")))?;
    Ok(parse_check_attr(&String::from_utf8_lossy(&out.stdout)))
}

/// `path NUL attr NUL value NUL` triples; `set` or `true` marks the path.
fn parse_check_attr(text: &str) -> Vec<(String, &'static str)> {
    let fields: Vec<&str> = text.split('\0').collect();
    let mut out: Vec<(String, &'static str)> = Vec::new();
    for [path, attr, value] in fields.as_chunks::<3>().0 {
        let why = match *attr {
            "linguist-vendored" => "vendored",
            "linguist-generated" => "generated",
            _ => continue,
        };
        if matches!(*value, "set" | "true") && !out.iter().any(|(p, _)| p == path) {
            out.push((path.to_string(), why));
        }
    }
    out
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

/// Listing the models takes about a second; a hung agent must not hold preflight for a whole pass's timeout.
const PROBE_SECS: u64 = 60;

pub fn probe(cfg: &Review, dir: &Path) -> std::result::Result<(), String> {
    let out = dir.join("list-models.txt");
    let mut cmd = Command::new(&cfg.agent);
    cmd.arg("--list-models");
    let secs = cfg.timeout_secs.min(PROBE_SECS);
    let ended = match run_logged(
        cmd,
        &out,
        &out.with_extension("err"),
        Duration::from_secs(secs),
    ) {
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
            "{} --list-models timed out after {secs} s",
            cfg.agent
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

/// What a review reports while it runs, for a caller that records it as it happens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Progress {
    /// The passes about to run and their log dir; 0 passes and no dir when the second opinion is skipped.
    Started { passes: usize, dir: Option<PathBuf> },
    /// Pass `k` of `of` ended, in finishing order.
    PassDone { k: usize, of: usize, seconds: u64 },
}

/// Where a review sends its [`Progress`]; `None` reports nothing.
pub type Sink<'a> = Option<&'a mut dyn FnMut(Progress) -> Result<()>>;

/// Starts every pass at once, writing each one's progress line to `progress` as it starts; returns in pass order.
/// `sink` hears each pass end as it ends; its first error is returned once every pass has ended.
pub fn run_passes(
    cfg: &Review,
    cwd: &Path,
    dir: &Path,
    passes: &[Pass],
    prompt: impl Fn(usize, &Pass) -> Result<String> + Sync,
    progress: &mut dyn Write,
    mut sink: Sink,
) -> Result<Vec<Ran>> {
    // Written before any spawn: a failed write must not leave reviewers running for nothing.
    for (i, pass) in passes.iter().enumerate() {
        writeln!(
            progress,
            "review pass {}/{}: {} ({} lines)",
            i + 1,
            passes.len(),
            covers(pass),
            pass.lines
        )
        .map_err(io("review progress"))?;
    }
    let of = passes.len();
    let (ended, done) = std::sync::mpsc::channel::<(usize, Duration)>();
    let (ran, failed) = std::thread::scope(|s| {
        let mut running = Vec::with_capacity(passes.len());
        for (i, pass) in passes.iter().enumerate() {
            let n = i + 1;
            let prompt = &prompt;
            let ended = ended.clone();
            running.push(s.spawn(move || {
                let started = Instant::now();
                let outcome = prompt(n, pass).and_then(|p| run_pass(cfg, cwd, dir, n, &p));
                let elapsed = outcome.as_ref().map_or(started.elapsed(), |o| o.elapsed);
                let _ = ended.send((n, elapsed));
                Ran { outcome, elapsed }
            }));
        }
        drop(ended);
        // Received while the passes run, so each is reported as it ends. A sink error stops the
        // reports, never the receiving, so every reviewer has ended when this returns.
        let mut failed = None;
        for (k, elapsed) in done {
            if let (None, Some(sink)) = (&failed, sink.as_mut()) {
                let seconds = elapsed.as_secs();
                failed = sink(Progress::PassDone { k, of, seconds }).err();
            }
        }
        let ran: Vec<Ran> = running
            .into_iter()
            .map(|h| h.join().unwrap_or_else(|p| std::panic::resume_unwind(p)))
            .collect();
        (ran, failed)
    });
    match failed {
        Some(e) => Err(e),
        None => Ok(ran),
    }
}

/// Git allows a newline in a path; written raw, one could forge the headings that follow.
pub fn one_line(path: &str) -> String {
    let mut line = String::with_capacity(path.len());
    for c in path.chars() {
        if c.is_control() {
            line.extend(c.escape_default());
        } else {
            line.push(c);
        }
    }
    line
}

/// The pass's files, and which part of the file's diff when it covers one.
fn covers(pass: &Pass) -> String {
    match &pass.part {
        Some(p) => format!("{} (part {} of {})", file_list(&pass.files), p.k, p.n),
        None => file_list(&pass.files),
    }
}

fn file_list(files: &[String]) -> String {
    files
        .iter()
        .map(|f| one_line(f))
        .collect::<Vec<_>>()
        .join(", ")
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
        covers(pass),
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
    let (text, what) = match &pass.part {
        Some(p) => (
            p.diff.clone(),
            format!(
                "Part {} of {} of the diff of {}",
                p.k,
                p.n,
                pass.files.join(" ")
            ),
        ),
        None => (
            diff(root, base, &pass.files)?,
            format!("The diff of {}", pass.files.join(" ")),
        ),
    };
    fs::write(&file, text).map_err(io(format!("write {}", file.display())))?;
    Ok(diff_prompt(&what, base, &file))
}

fn exit_words(code: Option<i32>) -> String {
    code.map_or("killed by a signal".to_string(), |c| format!("exit {c}"))
}

/// A kill's partial does not count, since the pass did not finish its files; nor does a failed exit's, or an
/// opening line would stop the fallback.
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
                    false,
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

/// `what` names the diff, as `The diff of a.rs b.rs`.
pub fn diff_prompt(what: &str, base: &str, diff_file: &Path) -> String {
    format!(
        "Adversarial code review. {what} against {base} is in {}: read it, and read any file in the repository you need for context. Report only defects you can point to at file:line with a failure scenario; no praise, no style. Do not edit anything.",
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

    #[test]
    fn a_path_stays_on_its_own_line() {
        assert_eq!(one_line("a\n## Leaks\tb.rs"), "a\\n## Leaks\\tb.rs");
        assert_eq!(one_line("dir/é.rs"), "dir/é.rs");
    }

    const STREAM: &str = include_str!("../tests/stubs/cursor-review.jsonl");

    const OPENING: &str = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"I'll read the diff."}]}}"#;
    const DONE: &str = r#"{"type":"result","is_error":false,"result":"the parser breaks"}"#;
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
                "the parser breaks\n",
                true,
            ),
            (
                Ended::Exited(Some(1)),
                vec![OPENING, DONE],
                "the parser breaks\n",
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
                false,
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

    #[test]
    fn tool_written_files_are_one_feature_folder_deep() {
        for (path, want) in [
            ("docs/features/001-x/log.jsonl", true),
            ("docs/features/001-x/state.toml", true),
            ("docs/features/log.jsonl", false),
            ("docs/features//state.toml", false),
            ("docs/features/001-x/sub/log.jsonl", false),
            ("docs/features/001-x/prd.md", false),
            ("src/docs/features/001-x/log.jsonl", false),
        ] {
            assert_eq!(tool_written(path), want, "{path}");
        }
    }

    #[test]
    fn check_attr_marks_set_or_true_and_names_the_first_reason() {
        let out = [
            "a.xsd",
            "linguist-vendored",
            "set",
            "a.xsd",
            "linguist-generated",
            "set",
            "b.rs",
            "linguist-vendored",
            "unspecified",
            "b.rs",
            "linguist-generated",
            "true",
            "c.rs",
            "linguist-vendored",
            "unset",
            "c.rs",
            "linguist-generated",
            "false",
            "",
        ]
        .join("\0");
        assert_eq!(
            parse_check_attr(&out),
            [
                ("a.xsd".to_string(), "vendored"),
                ("b.rs".to_string(), "generated")
            ]
        );
    }

    #[test]
    fn the_skipped_block_lists_each_file_and_says_when_attributes_went_unchecked() {
        let mut s = Skip::default();
        assert_eq!(s.lines(), "");
        s.skipped
            .push(("docs/features/001-x/log.jsonl".into(), "tool-written"));
        s.skipped.push(("t/a.xsd".into(), "vendored"));
        assert_eq!(
            s.lines(),
            "skipped:\n- docs/features/001-x/log.jsonl (tool-written)\n- t/a.xsd (vendored)\n"
        );
        assert_eq!(s.names(), "docs/features/001-x/log.jsonl, t/a.xsd");
        s.skipped.clear();
        s.unchecked = Some("boom".into());
        assert_eq!(
            s.lines(),
            "skipped: vendored and generated files not checked (boom)\n"
        );
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

    fn run_git(dir: &Path, args: &[&str]) -> String {
        let out = Command::new("git")
            .args(args)
            .current_dir(dir)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).into_owned()
    }

    /// `git diff` of `f` in a fresh repo at `dir`: from `before` to `after`, or added as `after` when `before` is `None`.
    fn real_diff(dir: &Path, before: Option<&str>, after: &str) -> String {
        run_git(dir, &["init", "-q"]);
        let f = dir.join("f");
        let diff = [
            "diff",
            "--no-color",
            "--no-ext-diff",
            "-U3",
            "--src-prefix=a/",
            "--dst-prefix=b/",
        ];
        match before {
            Some(b) => {
                fs::write(&f, b).unwrap();
                run_git(dir, &["add", "f"]);
                fs::write(&f, after).unwrap();
                run_git(dir, &diff)
            }
            None => {
                fs::write(&f, after).unwrap();
                run_git(dir, &["add", "f"]);
                run_git(dir, &[&diff[..], &["--cached"]].concat())
            }
        }
    }

    fn numbered(prefix: &str, range: std::ops::RangeInclusive<usize>) -> String {
        range.map(|i| format!("{prefix}{i}\n")).collect()
    }

    /// The `+` and `-` lines after the file header.
    fn changes(text: &str) -> u64 {
        let hunks = &text[text.find("\n@@").unwrap()..];
        changed(&hunks.split_inclusive('\n').collect::<Vec<_>>())
    }

    fn hunk_heads(text: &str) -> Vec<&str> {
        text.lines().filter(|l| l.starts_with("@@")).collect()
    }

    #[test]
    fn a_huge_hunk_splits_with_headers_git_can_apply() {
        let cases = [
            (
                "changes between short runs of context",
                numbered("a", 1..=40),
                [
                    numbered("a", 1..=5),
                    numbered("b", 6..=10),
                    numbered("a", 11..=14),
                    numbered("c", 1..=20),
                    numbered("a", 31..=40),
                ]
                .concat(),
            ),
            (
                "a last line without a newline",
                numbered("a", 1..=30).trim_end().to_string(),
                [numbered("a", 1..=5), numbered("b", 1..=41)]
                    .concat()
                    .trim_end()
                    .to_string(),
            ),
            (
                "lines added between kept ones",
                numbered("a", 1..=20),
                [
                    numbered("a", 1..=10),
                    numbered("n", 1..=35),
                    numbered("a", 11..=20),
                ]
                .concat(),
            ),
        ];
        for (what, before, after) in cases {
            let repo = tempfile::tempdir().unwrap();
            let text = real_diff(repo.path(), Some(&before), &after);
            assert_eq!(hunk_heads(&text).len(), 1, "{what}: {text}");
            let total = changes(&text);
            let parts = split_diff(&text, 10);
            assert!(parts.len() > 1, "{what}: {parts:?}");
            for (part, n) in &parts {
                assert!(*n <= 10, "{what}: {part}");
                assert_eq!(*n, changes(part), "{what}: {part}");
            }
            assert_eq!(parts.iter().map(|(_, n)| n).sum::<u64>(), total, "{what}");

            fs::write(repo.path().join("f"), &before).unwrap();
            let patches = tempfile::tempdir().unwrap();
            for (k, (part, _)) in parts.iter().enumerate() {
                let p = patches.path().join(format!("{k}.diff"));
                fs::write(&p, part).unwrap();
                let p = p.to_str().unwrap();
                run_git(repo.path(), &["apply", "--check", "--unidiff-zero", p]);
                run_git(repo.path(), &["apply", "--unidiff-zero", p]);
            }
            let got = fs::read_to_string(repo.path().join("f")).unwrap();
            assert_eq!(got, after, "{what}: {parts:?}");
        }
    }

    #[test]
    fn whole_hunks_pack_up_to_the_cap() {
        let mut after = String::new();
        for i in 1..=100 {
            let edited = (0..6).any(|e| i == 5 + 15 * e || i == 6 + 15 * e);
            after.push_str(&format!("{}{i}\n", if edited { "b" } else { "a" }));
        }
        let repo = tempfile::tempdir().unwrap();
        let text = real_diff(repo.path(), Some(&numbered("a", 1..=100)), &after);
        let heads = hunk_heads(&text);
        assert_eq!(heads.len(), 6, "{text}");
        let parts = split_diff(&text, 10);
        let got: Vec<(Vec<&str>, u64)> = parts.iter().map(|(p, n)| (hunk_heads(p), *n)).collect();
        assert_eq!(
            got,
            [
                (heads[0..2].to_vec(), 8),
                (heads[2..4].to_vec(), 8),
                (heads[4..6].to_vec(), 8)
            ]
        );
        assert_eq!(split_diff(&text, 24), [(text.clone(), 24)]);
    }

    #[test]
    fn the_file_header_heads_every_part() {
        let repo = tempfile::tempdir().unwrap();
        let text = real_diff(repo.path(), None, &numbered("n", 1..=30));
        let header = &text[..text.find("\n@@").unwrap() + 1];
        assert!(
            header.starts_with("diff --git a/f b/f\nnew file mode "),
            "{header}"
        );
        assert!(header.ends_with("--- /dev/null\n+++ b/f\n"), "{header}");
        let parts = split_diff(&text, 10);
        let heads: Vec<Vec<&str>> = parts.iter().map(|(p, _)| hunk_heads(p)).collect();
        assert_eq!(
            heads,
            [
                ["@@ -0,0 +1,10 @@"],
                ["@@ -0,0 +11,10 @@"],
                ["@@ -0,0 +21,10 @@"]
            ]
        );
        for (part, _) in &parts {
            assert!(part.starts_with(header), "{part}");
        }
        let added: String = parts
            .iter()
            .flat_map(|(p, _)| p.lines().filter_map(|l| l.strip_prefix('+')))
            .filter(|l| !l.starts_with("++ "))
            .map(|l| format!("{l}\n"))
            .collect();
        assert_eq!(added, numbered("n", 1..=30));
    }
}
