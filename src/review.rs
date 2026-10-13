//! `factory review second` (PRD-004 FR-6): the cross-vendor second opinion.
//! The probe spends nothing on a dead auth, the diff is split into passes of
//! at most `pass_lines` changed lines, each pass runs read-only under the
//! timeout with its stream logged to a file, and the stream's text is the
//! pass's findings: the `result` when the run finished, the assistant text
//! it had when it was killed.
//!
//! Every child writes to files, never to a pipe: a stream of 34 KB that
//! nobody drains blocks the reviewer, and the timeout then fires on a
//! working review.
use std::fs::{self, File};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::config::Review;
use crate::error::io;
use crate::{Error, Result, clock, paths};

/// The four flags that separate a review from a silent no-op: `--trust`
/// (or it exits on `Workspace Trust Required`), `--mode ask` (read-only),
/// `stream-json` (text mode buffers to the end) and partial output (or a
/// kill keeps nothing).
pub const FLAGS: [&str; 6] = [
    "--trust",
    "--mode",
    "ask",
    "--output-format",
    "stream-json",
    "--stream-partial-output",
];

/// One pass: the files it covers and their changed lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pass {
    pub files: Vec<String>,
    pub lines: u64,
    /// One file over the cap, alone in its pass.
    pub over: bool,
}

/// Files in order, grouped into passes of at most `cap` changed lines. A
/// file is never split; one larger than the cap is a pass of its own.
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

/// `git diff --numstat -z --no-renames` output: (path, added + deleted). A
/// binary file's `-` counts 0.
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

/// The changed files of `<base>...HEAD` under `paths`, sized, in git's
/// order, relative to the repo root. `cwd` resolves `paths`.
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

/// `git diff <base>...HEAD -- <files>` from the repo root.
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

/// A fresh `<data_dir>/reviews/<YYYYMMDD-HHMMSS>/`, `-2`, `-3`… when taken.
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

/// How a child under the timeout ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ended {
    Exited(Option<i32>),
    Killed,
}

/// Run `cmd` with stdout to `out` and stderr to `err`, killing its process
/// group once `timeout` passes. The elapsed time with it.
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
    let started = Instant::now();
    let mut child = cmd.spawn().map_err(|e| {
        Error::Msg(format!(
            "could not start {}: {e}",
            cmd.get_program().to_string_lossy()
        ))
    })?;
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

/// `<agent> --list-models`: `Err(reason)` when the review cannot run, so no
/// pass starts. Its output goes to `list-models.txt` in `dir`.
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
        _ if !text.split_whitespace().any(|t| t == cfg.model) => {
            Err(format!("model {} not in --list-models", cfg.model))
        }
        _ => Ok(()),
    }
}

/// What a pass's stream said.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Parsed {
    /// The `result` event's text: the run finished.
    pub result: Option<String>,
    /// Every `assistant` event's text items, joined: what a killed run had.
    pub partial: String,
}

impl Parsed {
    /// The pass's text: the `result`, else the assistant text; `None` when
    /// there is neither.
    pub fn text(&self) -> Option<&str> {
        match &self.result {
            Some(r) if !r.trim().is_empty() => Some(r),
            _ if !self.partial.trim().is_empty() => Some(&self.partial),
            _ => None,
        }
    }
}

/// Parse a stream line by line, skipping lines that are not JSON.
pub fn parse_stream(text: &str) -> Parsed {
    let mut p = Parsed::default();
    for line in text.lines() {
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        match v["type"].as_str() {
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

/// One pass's outcome.
#[derive(Debug, Clone)]
pub struct Outcome {
    pub ended: Ended,
    pub elapsed: Duration,
    pub parsed: Parsed,
    /// The first line of its stderr, for an `unreviewed` with no text.
    pub stderr: String,
}

/// Run one review: `<agent> -p <flags> --model <m> <prompt>` in `cwd`, its
/// stream to `<dir>/review-<n>.jsonl` and stderr to `review-<n>.err`.
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

/// The diff-mode prompt, `models.md`'s since 2026-09.
pub fn diff_prompt(files: &[String], base: &str, diff_file: &Path) -> String {
    format!(
        "Adversarial code review. The diff of {} against {base} is in {}: read it, and read any file in the repository you need for context. Report only defects you can point to at file:line with a failure scenario; no praise, no style. Do not edit anything.",
        files.join(" "),
        diff_file.display()
    )
}

/// The doc-mode prompt: the caller's instruction, then the document.
pub fn doc_prompt(text: &str, doc: &Path) -> String {
    format!(
        "{}\n\nThe document is {}: read it, and read any file in the repository you need for context. Do not edit anything.",
        text.trim_end(),
        doc.display()
    )
}

/// `--others`: the other reviewers' findings and the question they raise.
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
}
