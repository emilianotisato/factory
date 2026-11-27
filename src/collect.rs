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

use crate::config::ResolvedReview;
use crate::leaks::{self, Input};
use crate::provider::ReviewProvider;
use crate::review::{Cancel, Progress, Range, Sink};
use crate::{Result, ci, paths, review, settled};

/// One file under this many changed lines needs the local review only.
pub const LOCAL_ONLY_LINES: u64 = 50;

/// The `second opinion:` state when no pass counted.
const FALLBACK_OWED: &str = "fallback owed";
/// The `second opinion:` state when the owner turned it off: nothing is owed.
const DISABLED: &str = "skipped (disabled in config)";
/// The `second opinion:` state when the owner answered `proceed` for this run only.
const PROCEED: &str = "skipped (proceed for this run)";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Claim {
    pub pass: usize,
    pub location: String,
    pub severity: Severity,
    pub summary: String,
}

fn sanitize_cell(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn escape_table_cell(s: &str) -> String {
    sanitize_cell(s).replace('|', "\\|")
}

fn parse_severity(s: &str) -> Option<Severity> {
    match s.trim().to_lowercase().as_str() {
        "high" => Some(Severity::High),
        "medium" | "med" => Some(Severity::Medium),
        "low" => Some(Severity::Low),
        _ => None,
    }
}

fn table_cells(line: &str) -> Option<Vec<String>> {
    let t = line.trim();
    if !t.starts_with('|') {
        return None;
    }
    let cells: Vec<String> = t
        .trim_start_matches('|')
        .trim_end_matches('|')
        .split('|')
        .map(sanitize_cell)
        .filter(|c| !c.is_empty())
        .collect();
    if cells.is_empty() {
        return None;
    }
    Some(cells)
}

fn is_separator_row(cells: &[String]) -> bool {
    cells.iter().all(|c| {
        let t = c.trim();
        t.chars()
            .all(|ch| ch == '-' || ch == ':' || ch.is_whitespace())
            && t.contains('-')
    })
}

fn is_placeholder_row(cells: &[String]) -> bool {
    cells.iter().any(|c| {
        let t = c.trim();
        t.eq_ignore_ascii_case("none") || t == "—" || t == "-"
    })
}

#[derive(Clone, Copy)]
enum HeaderCol {
    Location,
    Severity,
    Claim,
}

fn header_col(cell: &str) -> Option<HeaderCol> {
    match cell.trim().to_lowercase().as_str() {
        "location" | "file" | "file:line" => Some(HeaderCol::Location),
        "severity" | "sev" => Some(HeaderCol::Severity),
        "claim" | "defect" | "summary" | "issue" | "finding" => Some(HeaderCol::Claim),
        _ => None,
    }
}

fn header_map(cells: &[String]) -> Option<(usize, usize, usize)> {
    if cells.len() != 3 {
        return None;
    }
    let mut loc = None;
    let mut sev = None;
    let mut claim = None;
    for (i, c) in cells.iter().enumerate() {
        match header_col(c) {
            Some(HeaderCol::Location) => loc = Some(i),
            Some(HeaderCol::Severity) => sev = Some(i),
            Some(HeaderCol::Claim) => claim = Some(i),
            None => return None,
        }
    }
    Some((loc?, sev?, claim?))
}

fn extract_table_at(lines: &[&str], start: usize, pass_n: usize) -> Vec<Claim> {
    let Some(header) = table_cells(lines[start]) else {
        return Vec::new();
    };
    let Some((loc_i, sev_i, claim_i)) = header_map(&header) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for line in &lines[start + 1..] {
        let cells = match table_cells(line) {
            Some(c) => c,
            None => break,
        };
        if is_separator_row(&cells) {
            continue;
        }
        if cells.len() != 3 || is_placeholder_row(&cells) {
            continue;
        }
        let Some(severity) = parse_severity(&cells[sev_i]) else {
            continue;
        };
        out.push(Claim {
            pass: pass_n,
            location: sanitize_cell(&cells[loc_i]),
            severity,
            summary: sanitize_cell(&cells[claim_i]),
        });
    }
    out
}

fn find_summary_tables(text: &str) -> Vec<usize> {
    let lines: Vec<&str> = text.lines().collect();
    let mut starts = Vec::new();
    let mut in_summary = false;
    for (i, line) in lines.iter().enumerate() {
        let t = line.trim();
        if t == "### Summary" || t == "## Summary" {
            in_summary = true;
            continue;
        }
        if in_summary && (t.starts_with("## ") || t.starts_with("### ")) && t != "### Summary" {
            in_summary = false;
        }
        if !in_summary {
            continue;
        }
        if let Some(cells) = table_cells(line)
            && header_map(&cells).is_some()
        {
            starts.push(i);
        }
    }
    starts
}

fn find_any_tables(text: &str) -> Vec<usize> {
    text.lines()
        .enumerate()
        .filter_map(|(i, line)| {
            let cells = table_cells(line)?;
            header_map(&cells).is_some().then_some(i)
        })
        .collect()
}

/// Pull structured defect rows from a pass body's markdown summary table.
pub fn extract_claims(pass_n: usize, text: &str) -> Vec<Claim> {
    let lines: Vec<&str> = text.lines().collect();
    let start = find_summary_tables(text)
        .into_iter()
        .next()
        .or_else(|| find_any_tables(text).into_iter().next());
    start.map_or(Vec::new(), |start| extract_table_at(&lines, start, pass_n))
}

fn render_claims_index(claims: &[Claim]) -> String {
    let mut out = String::from(
        "\n### Claims index\n\n| severity | file:line | pass | claim |\n|---|---|---|---|\n",
    );
    for c in claims {
        let sev = match c.severity {
            Severity::High => "high",
            Severity::Medium => "medium",
            Severity::Low => "low",
        };
        out.push_str(&format!(
            "| {sev} | {} | {} | {} |\n",
            escape_table_cell(&c.location),
            c.pass,
            escape_table_cell(&c.summary),
        ));
    }
    out
}
/// The leaks `verdict:` when the scan errored.
const DID_NOT_RUN: &str = "did not run";

fn head_line(head: &str) -> String {
    format!("<!-- head: {head} -->")
}

fn reviewer_line(cfg: &ResolvedReview) -> String {
    format!("<!-- reviewer: {} -->", reviewer_tag(cfg))
}

/// The tag stored on line 2 of a collect file for `reusable_for`.
pub fn reviewer_tag(cfg: &ResolvedReview) -> String {
    match cfg.provider {
        ReviewProvider::None => "none".into(),
        ReviewProvider::Provider(p) => match &cfg.effort {
            Some(e) => format!("{}:{}:{}", p, cfg.model, e),
            None => format!("{}:{}", p, cfg.model),
        },
    }
}

/// Whether a collected `text` may stand in for a fresh collect at `head`. A file from another HEAD reviewed other
/// code, and one owing a fallback, a leaks scan or a stopped pass would carry a failure a rerun may clear.
pub fn reusable(text: &str, head: &str) -> bool {
    text.lines().next() == Some(head_line(head).as_str())
        && !text.contains(&format!("\nsecond opinion: {FALLBACK_OWED}"))
        && !text.contains(&format!("\nverdict: {DID_NOT_RUN}"))
        && !text.contains(&format!("\n{}", review::STOPPED))
}

/// Whether `text` may stand in for a fresh collect at `head` for this reviewer.
pub fn reusable_for(text: &str, head: &str, review: &ResolvedReview) -> bool {
    reusable(text, head) && text.lines().nth(1) == Some(reviewer_line(review).as_str())
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

/// What a collect reviews, what its reviewers are told, and when they are stopped.
#[derive(Clone, Copy)]
pub struct Options<'a> {
    /// The branch the leaks scan, and without `since` the second opinion, measure from.
    pub base: &'a str,
    /// The second opinion reads only the commits after this one.
    pub since: Option<&'a str>,
    /// The feature whose settled block every pass prompt carries.
    pub feature: Option<&'a str>,
    /// Kills the second opinion's running reviewers when it fires.
    pub cancel: Cancel<'a>,
    /// The owner answered `proceed` for this run while config still names a reviewer.
    pub skipped_by_proceed: bool,
}

impl<'a> Options<'a> {
    pub fn new(base: &'a str) -> Options<'a> {
        Options {
            base,
            since: None,
            feature: None,
            cancel: None,
            skipped_by_proceed: false,
        }
    }

    fn range(&self) -> Range<'a> {
        match self.since {
            Some(c) => Range::Since(c),
            None => Range::Base(self.base),
        }
    }
}

/// The collected file for `root`'s branch, the second opinion's progress lines going to `progress`.
/// `sink` hears `Started` once the passes are known, then each pass as it ends.
/// Only a git error reading HEAD or the changed files, or the sink failing on a skipped second opinion, is the
/// error: every later part writes its own failure into the file.
pub fn collect(
    root: &Path,
    opts: &Options,
    cfg: impl Into<ResolvedReview>,
    progress: &mut dyn Write,
    mut sink: Sink,
) -> Result<String> {
    let cfg: ResolvedReview = cfg.into();
    let cfg = &cfg;
    let head = ci::head(root)?;
    let range = opts.range();
    let skip = review::skip(root, review::numstat(root, range, &[])?);
    let files = &skip.kept;
    let lines: u64 = files.iter().map(|(_, n)| n).sum();
    let leaks = leaks_part(root, opts.base);
    let second = needs_second_opinion(files);
    let (state, opinion) = if second && cfg.provider == ReviewProvider::None {
        if let Some(sink) = sink.as_mut() {
            sink(Progress::Started {
                passes: 0,
                dir: None,
            })?;
        }
        if opts.skipped_by_proceed {
            (
                PROCEED.to_string(),
                "skipped: second opinion skipped for this run (proceed)\n".into(),
            )
        } else {
            (
                DISABLED.to_string(),
                format!("skipped: {}\n", review::DISABLED),
            )
        }
    } else if second {
        let settled = opts.feature.and_then(|f| settled::for_feature(root, f));
        let reviewing = Reviewing {
            root,
            range,
            settled: settled.as_deref(),
            cancel: opts.cancel,
        };
        second_opinion(&reviewing, cfg, files, progress, sink)
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
    let since = match range {
        Range::Since(_) => " (since the last review)",
        Range::Base(_) => "",
    };
    let mut plan = format!(
        "changed: {}, {} against {}{since}\n",
        count(files.len(), "file"),
        count(lines as usize, "line"),
        range.spec()
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
        "{}\n{}\n\n## Plan\n\n{plan}size: {size}\nsecond opinion: {state}\n\n## Leaks\n\n{leaks}\n## Second opinion\n\n{opinion}",
        head_line(&head),
        reviewer_line(cfg),
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

/// The checkout, the commits its passes read, and the settled block their prompts carry.
struct Reviewing<'a> {
    root: &'a Path,
    range: Range<'a>,
    settled: Option<&'a str>,
    cancel: Cancel<'a>,
}

/// The plan's state line and the section's text.
fn second_opinion(
    on: &Reviewing,
    cfg: &ResolvedReview,
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
    let root = on.root;
    let passes = review::plan(root, on.range, files, cfg.pass_lines);
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
    if let Err(reason) = review::probe_resolved(cfg, &dir, on.cancel) {
        return unreviewed(&reason, Some(&dir), "");
    }
    if on.cancel.is_some_and(|stop| stop()) {
        return unreviewed("stopped", Some(&dir), "");
    }
    let ran = match review::run_passes(
        cfg,
        root,
        &dir,
        &passes,
        |n, pass| {
            let prompt = review::diff_pass_prompt(root, on.range, &dir, n, pass)?;
            Ok(match on.settled {
                Some(s) => review::with_settled(&prompt, s),
                None => prompt,
            })
        },
        progress,
        sink,
        on.cancel,
    ) {
        Ok(r) => r,
        Err(e) => return unreviewed(&e.to_string(), Some(&dir), ""),
    };
    let mut counted = 0;
    let mut claims = Vec::new();
    let mut sections = String::new();
    for (i, (pass, ran)) in passes.iter().zip(&ran).enumerate() {
        let (text, counts) = review::pass_text(ran, cfg.timeout_secs);
        counted += usize::from(counts);
        if counts {
            claims.extend(extract_claims(i + 1, &text));
        }
        let mark = if counts { "counted" } else { "unreviewed" };
        let header = review::pass_header(i + 1, pass, ran, cfg);
        sections.push_str(&format!("\n### {header} — {mark}\n\n{text}"));
    }
    if counted == 0 {
        let reason = format!("no pass of {} counted", passes.len());
        return unreviewed(&reason, Some(&dir), &sections);
    }
    claims.sort_by(|a, b| {
        (a.severity, a.location.as_str(), a.pass).cmp(&(b.severity, b.location.as_str(), b.pass))
    });
    let mut opinion = format!("exit: 0\nlog: {}\n", dir.display());
    if !claims.is_empty() {
        opinion.push_str(&render_claims_index(&claims));
    }
    opinion.push_str(&sections);
    (
        format!("reviewed, {counted} of {} passes counted", passes.len()),
        opinion,
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
    fn extract_claims_prefers_the_first_summary_table() {
        let body = "\
### Summary

| file:line | severity | claim |
|---|---|---|
| z.rs:9 | low | zed |
| a.rs:1 | high | alpha |

| file:line | severity | claim |
|---|---|---|
| ignored.rs:1 | high | other table |
";
        let claims = extract_claims(2, body);
        assert_eq!(claims.len(), 2);
        assert!(
            claims
                .iter()
                .any(|c| c.location == "a.rs:1" && c.severity == Severity::High)
        );
        assert!(
            claims
                .iter()
                .any(|c| c.location == "z.rs:9" && c.severity == Severity::Low)
        );
        assert!(!claims.iter().any(|c| c.location == "ignored.rs:1"));
    }

    #[test]
    fn extract_claims_reads_stub_shaped_pass_body() {
        let t = "Pass one prose.\n\n### Summary\n\n| file:line | severity | claim |\n|---|---|---|\n| src/a.rs:1 | low | low claim pass1 |\n| src/b.rs:2 | medium | medium claim pass1 |\n";
        assert_eq!(extract_claims(1, t).len(), 2);
    }

    #[test]
    fn extract_claims_omits_placeholders_and_unknown_severities() {
        let body = "\
| file:line | severity | claim |
|---|---|---|
| none | none | none |
| a.rs:1 | bogus | x |
| b.rs:2 | med | medium one |
";
        let claims = extract_claims(1, body);
        assert_eq!(claims.len(), 1);
        assert_eq!(claims[0].severity, Severity::Medium);
        assert_eq!(claims[0].summary, "medium one");
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
        let stopped = format!(
            "{}\n### Pass 2 — b.rs (9 lines, 1 s, m) — unreviewed\n\n{}",
            file("abc", counted, "clean"),
            review::STOPPED
        );
        assert!(!reusable(&stopped, "abc"));
    }
}
