//! One branch's mechanical review as a markdown file: the plan, the leaks scan and the second opinion.
//!
//! The file starts with `<!-- head: <sha> -->`. Each part sits under a fixed `## Plan`, `## Leaks` or
//! `## Second opinion` heading, in that order; the second opinion is last and runs to the end, because
//! the findings hold headings of their own. The plan's `size:` and `second opinion:` rows, the leaks
//! `verdict:` and the second opinion's `exit:` are lines of their own, and each pass is a
//! `### Pass n — … — counted` (or `— unreviewed`) heading above its findings.
use std::io::Write;
use std::path::Path;

use crate::config::Review;
use crate::leaks::{self, Input};
use crate::{Result, paths, review};

/// One file under this many changed lines needs the local review only.
pub const LOCAL_ONLY_LINES: u64 = 50;

/// Whether the size row adds the second opinion to the local review.
pub fn needs_second_opinion(files: &[(String, u64)]) -> bool {
    let lines: u64 = files.iter().map(|(_, n)| n).sum();
    files.len() > 1 || lines >= LOCAL_ONLY_LINES
}

/// The collected file for `<base>...HEAD` in `root`, the second opinion's progress lines going to `progress`.
/// Only a git error reading HEAD or the changed files is the error: every later part writes its own failure into the file.
pub fn collect(root: &Path, base: &str, cfg: &Review, progress: &mut dyn Write) -> Result<String> {
    let head = review::head(root)?;
    let files = review::numstat(root, base, &[])?;
    let lines: u64 = files.iter().map(|(_, n)| n).sum();
    let leaks = leaks_part(root, base);
    let second = needs_second_opinion(&files);
    let (state, opinion) = if second {
        second_opinion(root, base, cfg, &files, progress)
    } else {
        (
            "skipped by size".to_string(),
            format!(
                "skipped by size: {} under {LOCAL_ONLY_LINES} changed lines\n",
                count(files.len(), "file")
            ),
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
    for (file, n) in &files {
        plan.push_str(&format!("- {file} ({})\n", count(*n as usize, "line")));
    }
    Ok(format!(
        "<!-- head: {head} -->\n\n## Plan\n\n{plan}size: {size}\nsecond opinion: {state}\n\n## Leaks\n\n{leaks}\n## Second opinion\n\n{opinion}"
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
        Err(e) => format!("verdict: did not run — {e}\n"),
    }
}

/// The plan's state line and the section's text.
fn second_opinion(
    root: &Path,
    base: &str,
    cfg: &Review,
    files: &[(String, u64)],
    progress: &mut dyn Write,
) -> (String, String) {
    let unreviewed = |reason: &str, log: Option<&Path>, passes: &str| {
        let log = log.map_or(String::new(), |d| format!("log: {}\n", d.display()));
        (
            format!("fallback owed — unreviewed: {reason}"),
            format!("exit: 2\nunreviewed: {reason}\n{log}{passes}"),
        )
    };
    let dir = match review::new_log_dir(&paths::data_dir()) {
        Ok(d) => d,
        Err(e) => return unreviewed(&e.to_string(), None, ""),
    };
    if let Err(reason) = review::probe(cfg, &dir) {
        return unreviewed(&reason, Some(&dir), "");
    }
    let passes = review::group(files, cfg.pass_lines);
    let ran = match review::run_passes(
        cfg,
        root,
        &dir,
        &passes,
        |n, pass| review::diff_pass_prompt(root, base, &dir, n, pass),
        progress,
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
}
