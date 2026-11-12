//! One branch's mechanical review as a markdown file: the plan, the leaks scan and the second opinion.
//!
//! The file starts with `<!-- head: <sha> -->`. Each part sits under a fixed `## Plan`, `## Leaks` or
//! `## Second opinion` heading, in that order; the second opinion is last and runs to the end, because
//! the findings hold headings of their own. The plan's `size:` and `second opinion:` rows, the leaks
//! `verdict:` and, when the second opinion ran, its `exit:` are lines of their own, and each pass is a
//! `### Pass n — … — counted` (or `— unreviewed`) heading above its findings. Files no reviewer reads are a
//! `skipped:` block in the plan, before `size:`.
use std::io::Write;
use std::path::Path;

use crate::config::Review;
use crate::leaks::{self, Input};
use crate::review::{Progress, Sink};
use crate::{Result, ci, paths, review};

/// One file under this many changed lines needs the local review only.
pub const LOCAL_ONLY_LINES: u64 = 50;

/// The `second opinion:` state when no pass counted.
const FALLBACK_OWED: &str = "fallback owed";
/// The leaks `verdict:` when the scan errored.
const DID_NOT_RUN: &str = "did not run";

fn head_line(head: &str) -> String {
    format!("<!-- head: {head} -->")
}

/// Whether a collected `text` may stand in for a fresh collect at `head`. A file from another HEAD reviewed other
/// code, and one owing a fallback or a leaks scan would carry a failure a rerun may clear.
pub fn reusable(text: &str, head: &str) -> bool {
    text.lines().next() == Some(head_line(head).as_str())
        && !text.contains(&format!("\nsecond opinion: {FALLBACK_OWED}"))
        && !text.contains(&format!("\nverdict: {DID_NOT_RUN}"))
}

/// A collected `text`'s pass count and review log dir, from its second opinion's pass headings and `log:` line.
pub fn passes_and_log(text: &str) -> (usize, Option<&str>) {
    let Some((_, opinion)) = text.split_once("\n## Second opinion\n") else {
        return (0, None);
    };
    let passes = opinion
        .lines()
        .filter(|l| {
            l.starts_with("### Pass ")
                && (l.ends_with(" — counted") || l.ends_with(" — unreviewed"))
        })
        .count();
    let log = opinion.lines().find_map(|l| l.strip_prefix("log: "));
    (passes, log)
}

/// Whether the size row adds the second opinion to the local review.
pub fn needs_second_opinion(files: &[(String, u64)]) -> bool {
    let lines: u64 = files.iter().map(|(_, n)| n).sum();
    files.len() > 1 || lines >= LOCAL_ONLY_LINES
}

/// The collected file for `<base>...HEAD` in `root`, the second opinion's progress lines going to `progress`.
/// `sink` hears `Started` once the passes are known, then each pass as it ends.
/// Only a git error reading HEAD or the changed files, or the sink failing on a skipped second opinion, is the
/// error: every later part writes its own failure into the file.
pub fn collect(
    root: &Path,
    base: &str,
    cfg: &Review,
    progress: &mut dyn Write,
    mut sink: Sink,
) -> Result<String> {
    let head = ci::head(root)?;
    let skip = review::skip(root, review::numstat(root, base, &[])?);
    let files = &skip.kept;
    let lines: u64 = files.iter().map(|(_, n)| n).sum();
    let leaks = leaks_part(root, base);
    let second = needs_second_opinion(files);
    let (state, opinion) = if second {
        second_opinion(root, base, cfg, files, progress, sink)
    } else {
        if let Some(sink) = sink.as_mut() {
            sink(Progress::Started {
                passes: 0,
                dir: None,
            })?;
        }
        let why = if files.is_empty() && !skip.skipped.is_empty() {
            format!("no file left after skipping {}", skip.names())
        } else {
            format!(
                "{} under {LOCAL_ONLY_LINES} changed lines",
                count(files.len(), "file")
            )
        };
        (
            "skipped by size".to_string(),
            format!("skipped by size: {why}\n"),
        )
    };

    let size = if second {
        "local review and second opinion"
    } else {
        "local review only"
    };
    let mut plan = format!(
        "changed: {}, {} against {base}...HEAD\n",
        count(files.len(), "file"),
        count(lines as usize, "line")
    );
    for (file, n) in files {
        plan.push_str(&format!(
            "- {} ({})\n",
            review::one_line(file),
            count(*n as usize, "line")
        ));
    }
    plan.push_str(&skip.lines());
    Ok(format!(
        "{}\n\n## Plan\n\n{plan}size: {size}\nsecond opinion: {state}\n\n## Leaks\n\n{leaks}\n## Second opinion\n\n{opinion}",
        head_line(&head)
    ))
}

fn count(n: usize, what: &str) -> String {
    if n == 1 {
        format!("1 {what}")
    } else {
        format!("{n} {what}s")
    }
}

fn leaks_part(root: &Path, base: &str) -> String {
    match leaks::scan(root, Input::Base(base)) {
        Ok(report) => {
            let verdict = match report.fix_hits() {
                0 => "clean".to_string(),
                n => count(n, "fix hit"),
            };
            format!("verdict: {verdict}\n\n{}", report.render())
        }
        Err(e) => format!("verdict: {DID_NOT_RUN} — {e}\n"),
    }
}

/// The plan's state line and the section's text.
fn second_opinion(
    root: &Path,
    base: &str,
    cfg: &Review,
    files: &[(String, u64)],
    progress: &mut dyn Write,
    mut sink: Sink,
) -> (String, String) {
    let unreviewed = |reason: &str, log: Option<&Path>, passes: &str| {
        let log = log.map_or(String::new(), |d| format!("log: {}\n", d.display()));
        (
            format!("{FALLBACK_OWED} — unreviewed: {reason}"),
            format!("exit: 2\nunreviewed: {reason}\n{log}{passes}"),
        )
    };
    let passes = review::group(files, cfg.pass_lines);
    let mut started = |dir: Option<&Path>| match sink.as_mut() {
        Some(sink) => sink(Progress::Started {
            passes: passes.len(),
            dir: dir.map(Path::to_path_buf),
        }),
        None => Ok(()),
    };
    let dir = match review::new_log_dir(&paths::data_dir()) {
        Ok(d) => d,
        Err(e) => {
            let _ = started(None);
            return unreviewed(&e.to_string(), None, "");
        }
    };
    if let Err(e) = started(Some(&dir)) {
        return unreviewed(&e.to_string(), Some(&dir), "");
    }
    if let Err(reason) = review::probe(cfg, &dir) {
        return unreviewed(&reason, Some(&dir), "");
    }
    let ran = match review::run_passes(
        cfg,
        root,
        &dir,
        &passes,
        |n, pass| review::diff_pass_prompt(root, base, &dir, n, pass),
        progress,
        sink,
    ) {
        Ok(r) => r,
        Err(e) => return unreviewed(&e.to_string(), Some(&dir), ""),
    };
    let mut counted = 0;
    let mut sections = String::new();
    for (i, (pass, ran)) in passes.iter().zip(&ran).enumerate() {
        let (text, counts) = review::pass_text(ran, cfg.timeout_secs);
        counted += usize::from(counts);
        let mark = if counts { "counted" } else { "unreviewed" };
        let header = review::pass_header(i + 1, pass, ran, cfg);
        sections.push_str(&format!("\n### {header} — {mark}\n\n{text}"));
    }
    if counted == 0 {
        let reason = format!("no pass of {} counted", passes.len());
        return unreviewed(&reason, Some(&dir), &sections);
    }
    (
        format!("reviewed, {counted} of {} passes counted", passes.len()),
        format!("exit: 0\nlog: {}\n{sections}", dir.display()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn files(ns: &[u64]) -> Vec<(String, u64)> {
        ns.iter().map(|n| (format!("f{n}"), *n)).collect()
    }

    #[test]
    fn one_file_under_fifty_lines_is_the_local_review_only() {
        assert!(!needs_second_opinion(&files(&[])));
        assert!(!needs_second_opinion(&files(&[49])));
        assert!(needs_second_opinion(&files(&[50])));
        assert!(needs_second_opinion(&files(&[1, 1])));
    }

    #[test]
    fn a_file_is_reusable_at_its_head_unless_it_owes_a_rerun() {
        let file = |head: &str, state: &str, verdict: &str| {
            format!(
                "{}\n\n## Plan\n\nsecond opinion: {state}\n\n## Leaks\n\nverdict: {verdict}\n",
                head_line(head)
            )
        };
        let counted = "reviewed, 1 of 1 passes counted";
        assert!(reusable(&file("abc", counted, "clean"), "abc"));
        assert!(!reusable(&file("abc", counted, "clean"), "def"));
        let owed = format!("{FALLBACK_OWED} — unreviewed: logged out");
        assert!(!reusable(&file("abc", &owed, "clean"), "abc"));
        let errored = format!("{DID_NOT_RUN} — x");
        assert!(!reusable(&file("abc", counted, &errored), "abc"));
    }
}
