//! `factory docs migrate [--dry]` (PRD-002 FR-16): a v2 repo's docs moved to
//! the v3 layout in one uncommitted diff. [`plan`] only reads, the tree and
//! `git` read commands; [`run`] writes what the plan holds. The undo is git,
//! so a re-run after `git checkout -- docs && git clean -fd docs` must write
//! the same bytes: the plan never reads the clock, and every date in it comes
//! from the files or from `git blame`. Only a row's `written_at` is the time
//! [`run`] wrote it.
use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};

use crate::clock;
use crate::db::{Db, RetroRow};
use crate::docs::board::{self, Feature, Kind as Place};
use crate::docs::frontmatter::{self, Frontmatter, Value};
use crate::docs::log::{self, Entry, Flag, Kind, Verdict};
use crate::docs::migrate_ideas;
use crate::docs::project;
use crate::docs::state::{self, SliceState, State};
use crate::error::{io, json};
use crate::scheduler::write_atomic;
use crate::{Error, Result};

/// One line of the plan: `guess`, `create`, `rewrite`, `delete`, `keep`,
/// `row`, `unlink`, `relink`, `idea`, `drop`, `fix` or `warn`, then the path
/// or the key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanRow {
    pub verb: &'static str,
    pub what: String,
}

impl PlanRow {
    pub(crate) fn new(verb: &'static str, what: impl Into<String>) -> PlanRow {
        PlanRow {
            verb,
            what: what.into(),
        }
    }
}

impl fmt::Display for PlanRow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.verb, self.what)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    pub rows: Vec<PlanRow>,
    /// Repo-relative, in write order; `None` deletes. `docs/project.toml` is
    /// last, so routing flips to v3 only once everything else is on disk.
    pub files: Vec<(PathBuf, Option<String>)>,
    pub retros: Vec<RetroRow>,
}

fn refuse<T>(msg: impl Into<String>) -> Result<T> {
    Err(Error::Msg(msg.into()))
}

const PROJECT: &str = "docs/project.toml";
const INDEX: &str = "docs/index.md";
const AGENT_LOG: &str = "agent-log.md";
const PRD_TYPES: [&str; 4] = ["feature", "bugfix", "enhancement", "refactor"];
const HEADER_LABELS: [&str; 5] = [
    "**Type**:",
    "**Status**:",
    "**Created**:",
    "**Author**:",
    "**Owns FR range**:",
];
const RETRO_INTS: [&str; 9] = [
    "gate_runs",
    "gate_red",
    "ci_cycles",
    "slices",
    "checkpoints",
    "out_tokens",
    "cache_read_tokens",
    "start_context",
    "peak_context",
];

// ───────────────────────────────────────────────────────────── the v2 log

/// `^###\s+(AD|BLOCK|CP)-(\d+)\s+[—–-]\s+(.+)$` (`docs.mjs:270`).
struct Heading<'a> {
    prefix: &'static str,
    n: u32,
    rest: &'a str,
}

/// `\s+`, then what follows; `None` unless whitespace leads.
fn ws1(s: &str) -> Option<&str> {
    let t = s.trim_start();
    (t.len() < s.len()).then_some(t)
}

fn heading(line: &str) -> Option<Heading<'_>> {
    let l = ws1(line.trim_end().strip_prefix("###")?)?;
    let (prefix, l) = ["AD", "BLOCK", "CP"]
        .into_iter()
        .find_map(|p| Some((p, l.strip_prefix(p)?.strip_prefix('-')?)))?;
    let digits = l.find(|c: char| !c.is_ascii_digit()).unwrap_or(l.len());
    let n = l[..digits].parse().ok()?;
    let l = ws1(&l[digits..])?.strip_prefix(['—', '–', '-'])?;
    let rest = ws1(l)?;
    (!rest.is_empty()).then_some(Heading { prefix, n, rest })
}

/// `rest.split(/\s+·\s+/)`.
fn parts(rest: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    for (i, c) in rest.char_indices() {
        if c != '·' {
            continue;
        }
        let before = rest[..i].chars().next_back();
        let after = rest[i + c.len_utf8()..].chars().next();
        if before.is_some_and(char::is_whitespace) && after.is_some_and(char::is_whitespace) {
            out.push(rest[start..i].trim());
            start = i + c.len_utf8();
        }
    }
    out.push(rest[start..].trim());
    out
}

fn slice_field(p: &str) -> Option<u32> {
    let d = p.strip_prefix("slice-")?;
    (!d.is_empty() && d.bytes().all(|b| b.is_ascii_digit()))
        .then(|| d.parse().ok())
        .flatten()
}

/// `^[a-z][a-z-]*$`: a phase, or `open`/`resolved`.
fn is_word(p: &str) -> bool {
    p.starts_with(|c: char| c.is_ascii_lowercase())
        && p.chars().all(|c| c.is_ascii_lowercase() || c == '-')
}

/// A heading date: `YYYY-MM-DD`, or `YYYYMMDD` rewritten (`true`).
fn day(p: &str) -> Option<(String, bool)> {
    if state::is_date(p) {
        return Some((p.to_string(), false));
    }
    let d = (p.len() == 8 && p.bytes().all(|b| b.is_ascii_digit()))
        .then(|| format!("{}-{}-{}", &p[..4], &p[4..6], &p[6..]))?;
    Some((d, true))
}

fn is_fence(line: &str) -> bool {
    line.trim_start().starts_with("```")
}

/// `^#{1,2}\s`, or a `---` rule: either ends the entry above it.
fn ends_entry(line: &str) -> bool {
    let h = line.len() - line.trim_start_matches('#').len();
    let heading = (h == 1 || h == 2) && line[h..].starts_with(char::is_whitespace);
    heading || line.trim_end() == "---"
}

/// `^\*\*Review:?\*\*:?\s*(decide|fyi|yes|no)\b`, case-insensitive
/// (`docs.mjs:289-290`): `Some((None, _))` is a `no`; the `bool` says text
/// follows the flag word (`decide — because …`).
fn review_flag(line: &str) -> Option<(Option<Flag>, bool)> {
    let lower = line.trim().to_ascii_lowercase();
    let r = lower.strip_prefix("**review")?;
    let r = r.strip_prefix(':').unwrap_or(r).strip_prefix("**")?;
    let r = r.strip_prefix(':').unwrap_or(r).trim_start();
    [
        ("decide", Some(Flag::Decide)),
        ("fyi", Some(Flag::Fyi)),
        ("yes", Some(Flag::Fyi)),
        ("no", None),
    ]
    .into_iter()
    .find_map(|(w, f)| {
        let a = r.strip_prefix(w)?;
        (!a.starts_with(|c: char| c.is_alphanumeric() || c == '_'))
            .then(|| (f, !a.trim().is_empty()))
    })
}

/// `^\*\*{label} \((\d{4}-\d{2}-\d{2})\):\*\*\s*(.+)`: the date and the rest.
fn dated<'a>(line: &'a str, label: &str) -> Option<(&'a str, &'a str)> {
    let l = line
        .trim_start()
        .strip_prefix("**")?
        .strip_prefix(label)?
        .strip_prefix(" (")?;
    let date = l.get(..10).filter(|d| state::is_date(d))?;
    let rest = l[10..].strip_prefix("):**")?.trim();
    (!rest.is_empty()).then_some((date, rest))
}

fn is_marker(line: &str) -> bool {
    dated(line, "Owner").is_some()
        || dated(line, "Resolution").is_some()
        || review_flag(line).is_some()
}

/// An Owner line's verdict: the first word without trailing punctuation,
/// and the rest after `, `, ` — ` or `: `.
fn verdict(text: &str) -> Option<(Verdict, String)> {
    let first = &text[..text.find(char::is_whitespace).unwrap_or(text.len())];
    let word = first.trim_end_matches(|c: char| c.is_ascii_punctuation() || c == '—' || c == '–');
    let v = match word.to_ascii_lowercase().as_str() {
        "kept" => Verdict::Kept,
        "changed" => Verdict::Changed,
        _ => return None,
    };
    let rest = text[word.len()..].trim_start();
    let rest = rest
        .strip_prefix([',', ':', ';', '.', '—', '–', '-'])
        .unwrap_or(rest)
        .trim_start();
    Some((v, rest.to_string()))
}

fn ts(date: &str) -> String {
    format!("{date}T00:00:00Z")
}

/// Lines `a..b` with the blank ones at either end trimmed, joined.
fn trimmed(lines: &[&str]) -> String {
    let blank = |l: &&str| l.trim().is_empty();
    let a = lines.iter().position(|l| !blank(l)).unwrap_or(lines.len());
    let b = lines.iter().rposition(|l| !blank(l)).map_or(a, |i| i + 1);
    lines[a..b.max(a)].join("\n")
}

/// One v2 log read: the entries in v3 order and what the plan says about it.
struct Parsed {
    entries: Vec<Entry>,
    rows: Vec<PlanRow>,
}

/// One `### AD|BLOCK|CP-n` entry in the file: its heading line, and the
/// line after its last body line.
struct Span<'a> {
    start: usize,
    end: usize,
    heading: Heading<'a>,
}

/// `parseAgentLog` (`docs.mjs:263-293`) extended to keep what migration
/// must not lose: bodies, Owner and Resolution lines, and the text between
/// entries as notes. `dates[i]` is line `i`'s `git blame` date.
pub fn parse_agent_log(text: &str, dates: &[String]) -> Result<Vec<Entry>> {
    read_log(text, dates).map(|p| p.entries)
}

fn read_log(text: &str, dates: &[String]) -> Result<Parsed> {
    let mut lines: Vec<&str> = text.split('\n').collect();
    if text.ends_with('\n') {
        lines.pop();
    }
    let date_at = |i: usize| {
        dates
            .get(i)
            .or(dates.last())
            .cloned()
            .unwrap_or_else(|| "1970-01-01".into())
    };
    const STRUCTURAL: [&str; 3] = ["## Blockers", "## Decisions", "## Checkpoints"];

    let mut spans: Vec<Span> = Vec::new();
    let mut notes: Vec<Entry> = Vec::new();
    let mut run: Vec<usize> = Vec::new();
    let flush = |run: &mut Vec<usize>, notes: &mut Vec<Entry>| {
        if let (Some(&a), Some(&b)) = (run.first(), run.last()) {
            let body = trimmed(&lines[a..=b]);
            if !body.is_empty() {
                let first = (a..=b).find(|&i| !lines[i].trim().is_empty()).unwrap_or(a);
                let mut e = Entry::new(Kind::Note, "", &body);
                e.ts = ts(&date_at(first));
                notes.push(e);
            }
        }
        run.clear();
    };
    let mut open: Option<(usize, Heading)> = None;
    let mut fenced = false;
    let mut under_structural = false;
    for (i, &line) in lines.iter().enumerate() {
        if !fenced {
            if let Some(h) = heading(line) {
                if let Some((start, heading)) = open.take() {
                    spans.push(Span {
                        start,
                        end: i,
                        heading,
                    });
                }
                flush(&mut run, &mut notes);
                open = Some((i, h));
                continue;
            }
            if ends_entry(line)
                && let Some((start, heading)) = open.take()
            {
                spans.push(Span {
                    start,
                    end: i,
                    heading,
                });
            }
        }
        let was_fenced = fenced;
        if is_fence(line) {
            fenced = !fenced;
        }
        if open.is_some() {
            continue;
        }
        if !was_fenced {
            let t = line.trim();
            // `log show` prints the title and these three headings itself.
            if t.starts_with("# Agent log") || STRUCTURAL.contains(&t) {
                flush(&mut run, &mut notes);
                under_structural = STRUCTURAL.contains(&t);
                continue;
            }
            if under_structural && !t.is_empty() {
                under_structural = false;
                if t == "_(none)_" {
                    flush(&mut run, &mut notes);
                    continue;
                }
            }
        }
        run.push(i);
    }
    if let Some((start, heading)) = open.take() {
        spans.push(Span {
            start,
            end: lines.len(),
            heading,
        });
    }
    flush(&mut run, &mut notes);

    let mut rows = Vec::new();
    let mut seen: HashMap<(&str, u32), usize> = HashMap::new();
    let mut blocks: Vec<(u32, Vec<Entry>)> = Vec::new();
    let mut ads: Vec<(u32, Vec<Entry>)> = Vec::new();
    let mut cps: Vec<(u32, Vec<Entry>)> = Vec::new();
    for s in &spans {
        let h = &s.heading;
        let id = format!("{}-{}", h.prefix, h.n);
        if let Some(first) = seen.insert((h.prefix, h.n), s.start) {
            return refuse(format!(
                "{id} appears twice, at lines {} and {}: ids are never reused — renumber one in the v2 log first",
                first + 1,
                s.start + 1
            ));
        }
        let ps = parts(h.rest);
        let tail = &ps[1..];
        let at = tail
            .iter()
            .position(|p| slice_field(p).is_some() || is_word(p));
        let place = at.map(|i| &tail[i]);
        let slice = place.and_then(|p| slice_field(p));
        let phase = place
            .filter(|p| slice_field(p).is_none() && !matches!(**p, "open" | "resolved"))
            .map(|p| p.to_string());
        // A middle part that is no field (a ` · ` inside the title) stays in
        // the title: the place, the date `heading_date` reads and a BLOCK's
        // state are the fields.
        let date_at_part = tail.iter().rposition(|p| day(p).is_some());
        let state_at = (h.prefix == "BLOCK")
            .then(|| tail.len().checked_sub(1))
            .flatten()
            .filter(|&i| matches!(tail[i], "open" | "resolved"));
        let title = std::iter::once(ps[0])
            .chain(
                tail.iter()
                    .enumerate()
                    .filter(|(i, _)| {
                        Some(*i) != at && Some(*i) != date_at_part && Some(*i) != state_at
                    })
                    .map(|(_, p)| *p),
            )
            .collect::<Vec<_>>()
            .join(" · ");
        if let Some(p) = &phase
            && !log::PHASES.contains(&p.as_str())
        {
            rows.push(PlanRow::new(
                "warn",
                format!(
                    "{id}: phase \"{p}\" is not one of {}",
                    log::PHASES.join(", ")
                ),
            ));
        }
        let heading_date = || match tail.iter().rev().find_map(|p| day(p)) {
            Some((d, fixed)) => {
                if fixed {
                    rows_fix(&id, &d)
                } else {
                    (d, None)
                }
            }
            None => (
                date_at(s.start),
                Some(PlanRow::new(
                    "warn",
                    format!("{id}: no date in the heading, git blame's is used"),
                )),
            ),
        };

        // The body, minus the lines that become fields or entries of their own.
        let body = &lines[s.start + 1..s.end];
        let mut kept: Vec<&str> = Vec::new();
        let mut flag: Option<Flag> = None;
        let mut subs: Vec<Entry> = Vec::new();
        let mut in_fence = false;
        let mut j = 0;
        while j < body.len() {
            let line = body[j];
            if in_fence || is_fence(line) {
                if is_fence(line) {
                    in_fence = !in_fence;
                }
                kept.push(line);
                j += 1;
                continue;
            }
            if h.prefix == "AD"
                && let Some((f, tail)) = review_flag(line)
            {
                // The last Review line wins, as in v2. A `no`, and a line
                // with text after its flag word, stay in the body verbatim:
                // `flag` holds the word, the body the reason.
                flag = f;
                if f.is_none() || tail {
                    kept.push(line);
                }
                j += 1;
                continue;
            }
            let label = match h.prefix {
                "AD" => "Owner",
                "BLOCK" => "Resolution",
                _ => "",
            };
            if let Some((date, first)) = dated(line, label).filter(|_| !label.is_empty()) {
                let mut k = j + 1;
                while k < body.len()
                    && !body[k].trim().is_empty()
                    && !is_marker(body[k])
                    && !is_fence(body[k])
                {
                    k += 1;
                }
                let mut text = first.to_string();
                for c in &body[j + 1..k] {
                    text.push('\n');
                    text.push_str(c);
                }
                let text = text.trim_end().to_string();
                if label == "Owner" {
                    match verdict(&text) {
                        Some((v, rest)) => {
                            let mut e = Entry::new(Kind::Review, "", &rest);
                            e.ts = ts(date);
                            e.reference = Some(id.clone());
                            e.verdict = Some(v);
                            subs.push(e);
                        }
                        None => {
                            rows.push(PlanRow::new(
                                "warn",
                                format!(
                                    "{id}: Owner line \"{}\" is neither kept nor changed, it stays in the body",
                                    text.lines().next().unwrap_or_default()
                                ),
                            ));
                            kept.extend(&body[j..k]);
                        }
                    }
                } else {
                    let mut e = Entry::new(Kind::Answer, "", &text);
                    e.ts = ts(date);
                    e.reference = Some(id.clone());
                    subs.push(e);
                }
                j = k;
                continue;
            }
            kept.push(line);
            j += 1;
        }
        let body = trimmed(&kept);

        let mut e = Entry::new(Kind::Decision, &title, &body);
        e.id = Some(id.clone());
        e.slice = slice;
        e.phase = phase;
        match h.prefix {
            "AD" => {
                let (d, row) = heading_date();
                rows.extend(row);
                e.ts = ts(&d);
                if e.phase.as_deref() == Some("preflight") && flag.is_none() {
                    e.kind = Kind::Ask;
                } else {
                    e.flag = flag;
                }
                ads.push((h.n, std::iter::once(e).chain(subs).collect()));
            }
            "BLOCK" => {
                e.kind = Kind::Block;
                e.ts = ts(&date_at(s.start));
                let resolved = tail.last() == Some(&"resolved");
                if resolved && subs.is_empty() {
                    let mut a = Entry::new(
                        Kind::Answer,
                        "",
                        "_(resolved in the v2 log with no Resolution line)_",
                    );
                    a.ts = e.ts.clone();
                    a.reference = Some(id.clone());
                    subs.push(a);
                    rows.push(PlanRow::new(
                        "warn",
                        format!("{id}: resolved with no Resolution line, an answer says so"),
                    ));
                } else if !resolved && !subs.is_empty() {
                    rows.push(PlanRow::new(
                        "warn",
                        format!("{id}: the heading says open but a Resolution line answers it, so v3 reads it resolved"),
                    ));
                }
                blocks.push((h.n, std::iter::once(e).chain(subs).collect()));
            }
            _ => {
                e.kind = Kind::Checkpoint;
                e.phase = None;
                e.slice = slice.or_else(|| {
                    let at = title.find("slice-")? + 6;
                    let d = &title[at..];
                    d[..d.find(|c: char| !c.is_ascii_digit()).unwrap_or(d.len())]
                        .parse()
                        .ok()
                });
                if e.slice.is_none() {
                    rows.push(PlanRow::new(
                        "warn",
                        format!("{id}: no slice-NN in the heading, the checkpoint covers none"),
                    ));
                }
                let (d, row) = heading_date();
                rows.extend(row);
                e.ts = ts(&d);
                cps.push((h.n, vec![e]));
            }
        }
    }
    // Each sequence ascending (E-LOG-ORDER); v2 file order is not.
    let mut entries = notes;
    for mut group in [blocks, ads, cps] {
        group.sort_by_key(|(n, _)| *n);
        entries.extend(group.into_iter().flat_map(|(_, es)| es));
    }
    Ok(Parsed { entries, rows })
}

fn rows_fix(id: &str, d: &str) -> (String, Option<PlanRow>) {
    (
        d.to_string(),
        Some(PlanRow::new(
            "fix",
            format!("{id}: heading date {} → {d}", d.replace('-', "")),
        )),
    )
}

// ───────────────────────────────────────────────────────────────── git

fn git_out(root: &Path, args: &[&str]) -> std::result::Result<String, String> {
    let out = Command::new("git")
        .args(args)
        .current_dir(root)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

/// Each line's `git blame` author date, `YYYY-MM-DD` in the author's zone.
pub fn blame_dates(root: &Path, path: &Path) -> Result<Vec<String>> {
    Ok(blame_lines(root, path)?
        .into_iter()
        .map(|(d, _)| d)
        .collect())
}

/// [`blame_dates`], and whether each line is committed: git dates a line
/// that is not with the current time.
pub(crate) fn blame_lines(root: &Path, path: &Path) -> Result<Vec<(String, bool)>> {
    let rel = path.strip_prefix(root).unwrap_or(path).to_string_lossy();
    match git_out(root, &["blame", "--porcelain", "--", &rel]) {
        Ok(out) => Ok(parse_blame_lines(&out)),
        Err(e) => refuse(format!(
            "git blame {rel}: {e} — migrate dates from git, commit the file first"
        )),
    }
}

#[cfg(test)]
fn parse_blame(text: &str) -> Vec<String> {
    parse_blame_lines(text)
        .into_iter()
        .map(|(d, _)| d)
        .collect()
}

fn parse_blame_lines(text: &str) -> Vec<(String, bool)> {
    let mut when: HashMap<&str, (i64, i64)> = HashMap::new();
    let mut sha = "";
    let mut out = Vec::new();
    for line in text.split('\n') {
        if line.starts_with('\t') {
            let (t, tz) = when.get(sha).copied().unwrap_or_default();
            out.push((
                clock::rfc3339((t + tz).max(0) as u64)[..10].to_string(),
                sha.bytes().any(|b| b != b'0'),
            ));
            continue;
        }
        let first = line.split(' ').next().unwrap_or_default();
        if first.len() == 40 && first.bytes().all(|b| b.is_ascii_hexdigit()) {
            sha = first;
        } else if let Some(t) = line.strip_prefix("author-time ") {
            when.entry(sha).or_default().0 = t.trim().parse().unwrap_or(0);
        } else if let Some(z) = line.strip_prefix("author-tz ") {
            let z = z.trim();
            let sign = if z.starts_with('-') { -1 } else { 1 };
            let d = z.trim_start_matches(['+', '-']);
            let hh: i64 = d.get(..2).and_then(|h| h.parse().ok()).unwrap_or(0);
            let mm: i64 = d.get(2..4).and_then(|m| m.parse().ok()).unwrap_or(0);
            when.entry(sha).or_default().1 = sign * (hh * 3600 + mm * 60);
        }
    }
    out
}

// ─────────────────────────────────────────────── project.toml and index.md

fn has_docs_table(root: &Path) -> Result<bool> {
    let p = project::path(root);
    let text = match fs::read_to_string(&p) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(io(format!("read {}", p.display()))(e)),
    };
    let table: toml::Table = text.parse().map_err(|source| Error::Toml {
        what: format!("read {}", p.display()),
        source,
    })?;
    Ok(table.contains_key("docs"))
}

/// `Math.max(1, Number(v ?? d) || d)` (`docs.mjs:309-315`), as an integer.
fn floor1(v: Option<String>, d: u32) -> u32 {
    match v.and_then(|s| s.trim().parse::<f64>().ok()) {
        Some(n) if n.is_finite() && n != 0.0 => n.max(1.0) as u32,
        _ => d,
    }
}

fn number(n: f64) -> String {
    if n.fract() == 0.0 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

/// The index frontmatter as `[docs]`, appended to an existing
/// `docs/project.toml` so its `[run]` and `[workers]` tables keep their bytes.
fn project_toml(root: &Path, fm: &Frontmatter) -> Result<(String, Vec<PlanRow>)> {
    if has_docs_table(root)? {
        return refuse(format!(
            "{PROJECT} already has [docs]: migrate writes that table, so it refuses a repo that has one"
        ));
    }
    let p = project::path(root);
    let old = match fs::read_to_string(&p) {
        Ok(t) => Some(t),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(io(format!("read {}", p.display()))(e)),
    };
    let get = |k: &str| fm.get(k).and_then(Value::text);
    let q = |s: &str| toml::Value::String(s.to_string()).to_string();
    let mut rows = Vec::new();
    let mut lines = vec!["framework = 3".to_string()];
    for k in ["gate", "inner"] {
        if let Some(v) = get(k) {
            lines.push(format!("{k} = {}", q(&v)));
        }
    }
    if let Some(v) = get("gate_parallel") {
        lines.push(format!("gate_parallel = {}", v.trim() == "true"));
    }
    lines.push(format!("lanes = {}", floor1(get("lanes"), 1)));
    lines.push(format!(
        "checkpoint_every = {}",
        floor1(get("checkpoint_every"), 2)
    ));
    for (k, d) in [("prd_max_kb", 120.0), ("slice_max_kb", 30.0)] {
        let n = match get(k) {
            None => d,
            Some(v) => match v.trim().parse::<f64>() {
                Ok(n) if n.is_finite() => n,
                _ => {
                    rows.push(PlanRow::new(
                        "warn",
                        format!("{INDEX} {k}: \"{v}\" is not a number, {PROJECT} gets {d}"),
                    ));
                    d
                }
            },
        };
        lines.push(format!("{k} = {}", number(n)));
    }
    for (k, _) in fm.data.iter().flatten() {
        if !project::KEYS.contains(&k.as_str()) {
            rows.push(PlanRow::new("drop", format!("{INDEX} key: {k}")));
        }
    }
    let table = format!("[docs]\n{}\n", lines.join("\n"));
    let text = match &old {
        None => {
            rows.push(PlanRow::new("create", PROJECT));
            table
        }
        Some(t) => {
            rows.push(PlanRow::new(
                "rewrite",
                format!("{PROJECT} ([docs] appended, the other tables kept byte for byte)"),
            ));
            let mut out = t.clone();
            if !out.is_empty() && !out.ends_with('\n') {
                out.push('\n');
            }
            if !out.trim().is_empty() {
                out.push('\n');
            }
            out + &table
        }
    };
    Ok((text, rows))
}

/// The frontmatter block and the newline after it, gone; `None` when the
/// text has none.
fn strip_frontmatter(text: &str) -> Option<&str> {
    let rest = text.strip_prefix("---\n")?;
    if let Some(r) = rest.strip_prefix("---\n") {
        return Some(r);
    }
    let end = rest.find("\n---\n")?;
    Some(&rest[end + 5..])
}

/// Lines with `None` for a removed one, joined: a blank line a removal
/// leaves next to another blank one goes too.
fn collapse(lines: Vec<Option<String>>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut removed = false;
    for l in lines {
        let Some(l) = l else {
            removed = true;
            continue;
        };
        let blank = l.trim().is_empty();
        if blank && removed && out.last().is_none_or(|p| p.trim().is_empty()) {
            continue;
        }
        if !blank {
            removed = false;
        }
        out.push(l);
    }
    out
}

fn split_lines(text: &str) -> (Vec<&str>, bool) {
    let mut lines: Vec<&str> = text.split('\n').collect();
    let nl = text.ends_with('\n');
    if nl {
        lines.pop();
    }
    (lines, nl)
}

fn join_lines(lines: &[String], nl: bool) -> String {
    let mut out = lines.join("\n");
    if nl {
        out.push('\n');
    }
    out
}

/// `^##\s` and not deeper.
fn is_h2(line: &str) -> bool {
    line.starts_with("##") && !line.starts_with("###") && line[2..].starts_with(char::is_whitespace)
}

/// The line range under the first `## ` heading whose text holds `name`
/// (`parseTable`, `docs.mjs:337-356`): heading, first line after it, end.
fn section(lines: &[&str], name: &str) -> Option<(usize, usize)> {
    let h = lines
        .iter()
        .position(|l| is_h2(l) && l.to_ascii_lowercase().contains(name))?;
    let end = lines[h + 1..]
        .iter()
        .position(|l| is_h2(l) || (l.starts_with("# ")))
        .map_or(lines.len(), |i| h + 1 + i);
    Some((h, end))
}

/// The line with `{label}\s*{prefix}\d+` cut out, with the `**` around it
/// and a trailing `.`; `None` when the line does not hold it.
fn cut_counter(line: &str, label: &str, prefix: &str) -> Option<String> {
    let lower = line.to_ascii_lowercase();
    let pos = lower.find(label)?;
    let after = &line[pos + label.len()..];
    let after_ws = after.trim_start();
    let num = after_ws
        .get(..prefix.len())
        .filter(|p| p.eq_ignore_ascii_case(prefix))
        .map(|_| &after_ws[prefix.len()..])?;
    let digits = num.find(|c: char| !c.is_ascii_digit()).unwrap_or(num.len());
    if digits == 0 {
        return None;
    }
    let mut start = pos;
    let mut end = line.len() - num.len() + digits;
    let tail = &line[end..];
    match tail.find("**") {
        Some(k)
            if line[..start].ends_with("**")
                && !tail[..k].contains(char::is_alphanumeric)
                && !tail[..k].contains(' ') =>
        {
            start -= 2;
            end += k + 2;
        }
        _ => {
            if tail.starts_with('.') {
                end += 1;
            }
        }
    }
    let left = line[..start].trim_end();
    let right = line[end..].trim_start();
    Some(match (left.is_empty(), right.is_empty()) {
        (true, _) => right.to_string(),
        (_, true) => left.to_string(),
        _ => format!("{left} {right}"),
    })
}

/// `PRD-001 … PRD-003`, or one number alone.
fn fr_paragraph(numbers: &[u32]) -> String {
    let range = match (numbers.iter().min(), numbers.iter().max()) {
        (Some(a), Some(b)) if a == b => format!(" (PRD-{a:03})"),
        (Some(a), Some(b)) => format!(" (PRD-{a:03} … PRD-{b:03})"),
        _ => String::new(),
    };
    format!(
        "**FR numbers:** PRDs written before v3{range} keep their global FR numbers; from v3 on, FR ids are feature-scoped: `FR-1` within a PRD, `PRD-NNN FR-n` across."
    )
}

/// `docs/index.md` without its frontmatter, the Live-features and Archived
/// table rows and the two counters, with the FR-numbers paragraph after
/// Build order. Every heading stays: in v2 the counters, Build order and the
/// retired line are paragraphs inside `## Live features`.
fn index_md(text: &str, numbers: &[u32]) -> (String, Vec<PlanRow>) {
    let body = strip_frontmatter(text).unwrap_or(text);
    let (lines, nl) = split_lines(body);
    let tables: Vec<(usize, usize)> = ["live features", "archived"]
        .iter()
        .filter_map(|n| section(&lines, n))
        .collect();
    let mut table_rows = 0;
    let mut counters = 0;
    let mut kept: Vec<Option<String>> = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if tables.iter().any(|(h, e)| i > *h && i < *e) && line.trim_start().starts_with('|') {
            table_rows += 1;
            kept.push(None);
            continue;
        }
        let mut l = line.to_string();
        let mut cut = false;
        for (label, prefix) in [("next free prd number:", ""), ("next free fr:", "FR-")] {
            if let Some(x) = cut_counter(&l, label, prefix) {
                l = x;
                cut = true;
                counters += 1;
            }
        }
        kept.push((!(cut && l.trim().is_empty())).then_some(l));
    }
    let mut out = collapse(kept);
    while out.first().is_some_and(|l| l.trim().is_empty()) {
        out.remove(0);
    }
    let para = fr_paragraph(numbers);
    let blank = |l: &String| l.trim().is_empty();
    let where_;
    if let Some(b) = out
        .iter()
        .position(|l| l.trim_start().starts_with("**Build order:**"))
    {
        let e = out[b..].iter().position(blank).map_or(out.len(), |i| b + i);
        out.splice(e..e, [String::new(), para]);
        where_ = "after **Build order:**";
    } else if let Some(h) = out
        .iter()
        .position(|l| is_h2(l) && l.to_ascii_lowercase().contains("live features"))
    {
        let end = out[h + 1..]
            .iter()
            .position(|l| is_h2(l))
            .map_or(out.len(), |i| h + 1 + i);
        let last = (h..end).rev().find(|&i| !blank(&out[i])).unwrap_or(h);
        out.splice(last + 1..last + 1, [String::new(), para]);
        where_ = "at the end of ## Live features (no **Build order:** paragraph)";
    } else {
        out.extend([String::new(), para]);
        where_ = "at the end of the file (no ## Live features)";
    }
    let rows = vec![PlanRow::new(
        "rewrite",
        format!(
            "{INDEX} (frontmatter, {table_rows} table rows and {counters} counters dropped; FR-numbers paragraph {where_})"
        ),
    )];
    (join_lines(&out, nl || body.is_empty()), rows)
}

/// `\|`-aware cells of a `| a | b |` row. A `|` inside a code span (a run
/// of backticks up to the next run of the same length) is literal, so a
/// Notes cell holding `` `log add | show` `` stays whole.
fn cells(row: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let chars: Vec<char> = row.trim().chars().collect();
    let run_at = |i: usize| chars[i..].iter().take_while(|&&c| c == '`').count();
    // The length of the open code span's backtick run, if one is open.
    let mut code: Option<usize> = None;
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '\\' if chars.get(i + 1) == Some(&'|') => {
                cur.push('|');
                i += 2;
            }
            '`' => {
                let n = run_at(i);
                cur.extend(&chars[i..i + n]);
                code = match code {
                    Some(open) if open == n => None,
                    Some(open) => Some(open),
                    // Opens only when a run of the same length closes it.
                    None => {
                        let mut j = i + n;
                        let mut closes = false;
                        while j < chars.len() {
                            let m = run_at(j);
                            if m == n {
                                closes = true;
                                break;
                            }
                            j += m.max(1);
                        }
                        closes.then_some(n)
                    }
                };
                i += n;
            }
            '|' if code.is_none() => {
                out.push(std::mem::take(&mut cur));
                i += 1;
            }
            c => {
                cur.push(c);
                i += 1;
            }
        }
    }
    out.push(cur);
    // The pieces before the first and after the last `|`.
    if out.first().is_some_and(|c| c.trim().is_empty()) {
        out.remove(0);
    }
    if out.last().is_some_and(|c| c.trim().is_empty()) {
        out.pop();
    }
    out.into_iter().map(|c| c.trim().to_string()).collect()
}

/// The index Notes cell per feature number (`cellNumber`, `docs.mjs:358`).
fn notes_cells(index: &str) -> HashMap<u32, String> {
    let (lines, _) = split_lines(index);
    let mut out = HashMap::new();
    let Some((h, end)) = section(&lines, "live features") else {
        return out;
    };
    let rows: Vec<Vec<String>> = lines[h + 1..end]
        .iter()
        .filter(|l| l.trim_start().starts_with('|'))
        .map(|l| cells(l))
        .collect();
    let Some(head) = rows.first() else {
        return out;
    };
    let Some(col) = head.iter().position(|c| c.eq_ignore_ascii_case("notes")) else {
        return out;
    };
    for r in &rows[1..] {
        let first = r
            .first()
            .map(|c| c.trim_start_matches('*').trim_start())
            .unwrap_or("");
        let d = first
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(first.len());
        if d == 0 || d > 4 {
            continue;
        }
        if let (Ok(n), Some(note)) = (first[..d].parse(), r.get(col)) {
            out.insert(n, note.clone());
        }
    }
    out
}

// ─────────────────────────────────────────────────────── per live feature

/// A frontmatter scalar that both `frontmatter::parse` (which strips one
/// pair of quotes and has no escapes) and a standard YAML reader read back
/// as written: plain when nothing in it needs quoting, else double quotes
/// when it holds no `"` or `\` (YAML's escapes), else single quotes when it
/// holds no `'` (as the ideas writer does), else a `|-` block scalar, which
/// both read verbatim.
fn yaml(v: &str) -> String {
    let plain = !v.starts_with(['[', '"', '\'', '{', '|', '>', '&', '*', '!', '%', '@', '`'])
        && !v.contains(": ")
        && !v.contains(" #")
        && !v.ends_with(':');
    if plain {
        v.to_string()
    } else if !v.contains(['"', '\\']) {
        format!("\"{v}\"")
    } else if !v.contains('\'') {
        format!("'{v}'")
    } else {
        format!("|-\n  {v}")
    }
}

/// `prd.md` with the FR-13 frontmatter and no prose header block; returns
/// the text, the plan rows, and the `status` and `reviewed` it held.
fn prd_md(
    rel: &str,
    f: &Feature,
    text: &str,
    notes: &str,
) -> Result<(String, Vec<PlanRow>, State)> {
    let fm = frontmatter::parse(text);
    if fm.data.is_none() {
        return refuse(format!(
            "{rel} has no frontmatter{} — fix the v2 tree first (`node docs.mjs check`)",
            fm.error
                .as_deref()
                .map(|e| format!(" ({e})"))
                .unwrap_or_default()
        ));
    }
    let mut rows = Vec::new();
    let (lines, nl) = split_lines(&fm.body);
    let head_end = lines
        .iter()
        .position(|l| l.starts_with("## "))
        .unwrap_or(lines.len());
    let mut values: HashMap<&str, String> = HashMap::new();
    let mut kept: Vec<Option<String>> = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let label = HEADER_LABELS
            .iter()
            .find(|l| i < head_end && line.trim_start().starts_with(**l));
        match label {
            Some(l) => {
                values
                    .entry(l)
                    .or_insert_with(|| line.trim_start()[l.len()..].trim().to_string());
                kept.push(None);
            }
            None => kept.push(Some(line.to_string())),
        }
    }
    let body = join_lines(&collapse(kept), nl);
    let title = lines[..head_end]
        .iter()
        .find_map(|l| l.strip_prefix("# "))
        .map(|h| {
            let h = h.trim();
            let strip = || {
                let r = h.strip_prefix("PRD-")?;
                let d = r.find(|c: char| !c.is_ascii_digit())?;
                ws1(&r[d..])?.strip_prefix(['—', '–', '-']).and_then(ws1)
            };
            strip().unwrap_or(h).to_string()
        })
        .unwrap_or_else(|| {
            rows.push(PlanRow::new(
                "warn",
                format!("{rel}: no `# ` heading, title is the slug"),
            ));
            f.slug.clone()
        });
    let ty = match values.get("**Type**:") {
        None => {
            rows.push(PlanRow::new(
                "warn",
                format!("{rel}: no **Type**: line, type is empty"),
            ));
            String::new()
        }
        Some(v) => {
            let first = v
                .split(char::is_whitespace)
                .next()
                .unwrap_or("")
                .trim_end_matches(|c: char| !c.is_alphanumeric())
                .to_ascii_lowercase();
            if PRD_TYPES.contains(&first.as_str()) {
                if first != *v {
                    rows.push(PlanRow::new("fix", format!("{rel} type \"{v}\" → {first}")));
                }
                first
            } else {
                rows.push(PlanRow::new(
                    "warn",
                    format!(
                        "{rel} type \"{v}\" is not one of {}: kept as written, check reports it",
                        PRD_TYPES.join(" | ")
                    ),
                ));
                v.clone()
            }
        }
    };
    let created = match values.get("**Created**:") {
        None => {
            rows.push(PlanRow::new(
                "warn",
                format!("{rel}: no **Created**: line, created is empty"),
            ));
            String::new()
        }
        Some(v) if state::is_date(v) => v.clone(),
        Some(v) => match v.get(..10).filter(|d| state::is_date(d)) {
            Some(d) => {
                rows.push(PlanRow::new("fix", format!("{rel} created \"{v}\" → {d}")));
                d.to_string()
            }
            None => {
                rows.push(PlanRow::new(
                    "warn",
                    format!(
                        "{rel} created \"{v}\" is not a date: kept as written, check reports it"
                    ),
                ));
                v.clone()
            }
        },
    };
    let author = values.get("**Author**:").cloned().unwrap_or_else(|| {
        rows.push(PlanRow::new(
            "warn",
            format!("{rel}: no **Author**: line, author is empty"),
        ));
        String::new()
    });
    let mut out = format!(
        "---\nnumber: {:03}\nslug: {}\ntitle: {}\ntype: {}\ncreated: {}\nauthor: {}\n",
        f.number,
        f.slug,
        yaml(&title),
        yaml(&ty),
        yaml(&created),
        yaml(&author)
    );
    if notes.trim().is_empty() {
        rows.push(PlanRow::new(
            "warn",
            format!("{rel}: no index Notes cell, summary is empty"),
        ));
        out.push_str("summary: \"\"\n");
    } else {
        out.push_str(&format!("summary: |\n  {}\n", notes.trim()));
    }
    out.push_str("---\n");
    out.push_str(&body);
    for (k, _) in fm.data.iter().flatten() {
        if !matches!(k.as_str(), "number" | "slug" | "status" | "reviewed") {
            rows.push(PlanRow::new("drop", format!("{rel} key: {k}")));
        }
    }

    let status = match fm.get("status").and_then(Value::text) {
        Some(s) => s,
        None => {
            rows.push(PlanRow::new(
                "warn",
                format!("{rel}: no status, state.toml says draft"),
            ));
            "draft".into()
        }
    };
    let reviewed = match fm.get("reviewed").and_then(Value::text) {
        None => None,
        Some(v) if state::is_date(&v) => Some(v),
        Some(v) => match day(&v) {
            Some((d, true)) => {
                rows.push(PlanRow::new("fix", format!("{rel} reviewed {v} → {d}")));
                Some(d)
            }
            _ => {
                return refuse(format!(
                    "{rel}: reviewed \"{v}\" is not a date (YYYY-MM-DD) — fix it in the v2 frontmatter first"
                ));
            }
        },
    };
    let st = State {
        status,
        reviewed,
        shipped: None,
        slices: BTreeMap::new(),
    };
    Ok((out, rows, st))
}

/// One `[slices.N]` from a v2 slice file's frontmatter.
fn slice_state(rel: &str, n: u32, fm: &Frontmatter, rows: &mut Vec<PlanRow>) -> SliceState {
    let num = |v: &Value| v.as_i64().and_then(|n| u32::try_from(n).ok());
    if let Some(v) = fm.get("slice")
        && num(v) != Some(n)
    {
        rows.push(PlanRow::new(
            "warn",
            format!(
                "{rel}: frontmatter slice {} ≠ file number {n}, the file number wins",
                v.text().unwrap_or_default()
            ),
        ));
    }
    let status = fm.get("status").and_then(Value::text).unwrap_or_else(|| {
        rows.push(PlanRow::new("warn", format!("{rel}: no status, pending")));
        "pending".into()
    });
    let depends_on = match fm.get("depends_on") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::List(l)) => l.iter().filter_map(num).collect(),
        Some(v) => num(v).into_iter().collect(),
    };
    SliceState {
        status,
        lane: fm.get("lane").and_then(num),
        depends_on,
    }
}

// ─────────────────────────────────────────────────────────────── links

/// A link at the `[` at `i`: its text, its href and the byte after its `)`
/// (`check.rs`'s `link_at`, the reader E-LINK-BROKEN uses).
fn link_at(text: &str, i: usize) -> Option<(&str, &str, usize)> {
    let close = i + text[i..].find(']')?;
    let label = &text[i + 1..close];
    if label.contains('[') || label.contains('\n') {
        return None;
    }
    let rest = text[close + 1..].strip_prefix('(')?;
    let start = close + 2;
    let len = rest
        .find(|c: char| c == ')' || c.is_whitespace())
        .unwrap_or(rest.len());
    if len == 0 {
        return None;
    }
    let href = &rest[..len];
    let after = &rest[len..];
    if after.starts_with(')') {
        return Some((label, href, start + len + 1));
    }
    let title = after.trim_start().strip_prefix('"')?;
    let q = title.find('"')?;
    let tail = title[q + 1..].strip_prefix(')')?;
    Some((label, href, text.len() - tail.len()))
}

fn normalize(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            c => out.push(c),
        }
    }
    out
}

/// Every link in `text` (the file at `dir`) whose target is one of `gone`
/// replaced by its text alone; the `(line, link)` of each.
fn unlink(text: &str, dir: &Path, gone: &[PathBuf]) -> (String, Vec<(usize, String)>) {
    let (out, hits) = edit_links(text, dir, |label, _, p| {
        gone.iter().any(|g| g == p).then(|| label.to_string())
    });
    (out, hits.into_iter().map(|(l, old, _)| (l, old)).collect())
}

/// Every link in `text` to `ideas` (`docs/ideas.md`) pointed at
/// `ideas/README.md` instead, anchor dropped: the README holds the rules
/// those links meant (FR-20). The `(line, old link, new link)` of each.
fn relink(text: &str, dir: &Path, ideas: &Path) -> (String, Vec<(usize, String, String)>) {
    edit_links(text, dir, |label, target, p| {
        let base = target.strip_suffix("ideas.md")?;
        (p == ideas).then(|| format!("[{label}]({base}ideas/README.md)"))
    })
}

/// Every link in `text` (the file at `dir`, so a relative target resolves)
/// that `edit` gives a replacement for, given its text, its target without
/// the anchor and the resolved path. Images and external links are left.
fn edit_links(
    text: &str,
    dir: &Path,
    edit: impl Fn(&str, &str, &Path) -> Option<String>,
) -> (String, Vec<(usize, String, String)>) {
    let mut out = String::new();
    let mut hits = Vec::new();
    let mut at = 0;
    let mut copied = 0;
    while let Some(i) = text[at..].find('[').map(|i| at + i) {
        let Some((label, href, end)) = link_at(text, i) else {
            at = i + 1;
            continue;
        };
        let target = href.split('#').next().unwrap_or_default();
        let external = href.contains(':') || target.is_empty();
        if !external
            && !text[..i].ends_with('!')
            && let Some(new) = edit(label, target, &normalize(&dir.join(target)))
        {
            out.push_str(&text[copied..i]);
            out.push_str(&new);
            hits.push((
                text[..i].matches('\n').count() + 1,
                text[i..end].to_string(),
                new,
            ));
            copied = end;
        }
        at = end;
    }
    out.push_str(&text[copied..]);
    (out, hits)
}

fn walk_md(dir: &Path, skip: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    let mut paths: Vec<PathBuf> = rd.filter_map(|e| e.ok()).map(|e| e.path()).collect();
    paths.sort();
    for p in paths {
        if p == skip {
            continue;
        }
        if p.is_dir() {
            walk_md(&p, skip, out);
        } else if p.extension().is_some_and(|e| e == "md") {
            out.push(p);
        }
    }
}

// ─────────────────────────────────────────────────────────────── retros

/// One `retros` row per `retro.md` with frontmatter, live and archived.
fn retro_rows(root: &Path, repo: &str) -> Result<(Vec<RetroRow>, Vec<PlanRow>)> {
    let mut rows = Vec::new();
    let mut plan = Vec::new();
    for base in ["docs/features", "docs/archive"] {
        let Ok(rd) = fs::read_dir(root.join(base)) else {
            continue;
        };
        let mut names: Vec<String> = rd
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        for name in names {
            let Some((feature, slug)) = board::folder_name(&name) else {
                continue;
            };
            let rel = format!("{base}/{name}/retro.md");
            let Ok(text) = fs::read_to_string(root.join(&rel)) else {
                continue;
            };
            let fm = frontmatter::parse(&text);
            if fm.data.is_none() {
                continue;
            }
            let mut bad: Vec<(&str, String)> = Vec::new();
            let mut float = |k: &'static str| -> Option<f64> {
                let t = fm.get(k)?.text()?;
                match t.trim().parse::<f64>() {
                    Ok(n) if n.is_finite() => Some(n),
                    _ => {
                        bad.push((k, t));
                        None
                    }
                }
            };
            let mut ints: HashMap<&str, Option<i64>> = HashMap::new();
            let mut fractions = Vec::new();
            for k in std::iter::once("score").chain(RETRO_INTS) {
                let n = match float(k) {
                    Some(n) if n.fract() != 0.0 => {
                        fractions.push((k, number(n)));
                        None
                    }
                    n => n.map(|n| n as i64),
                };
                ints.insert(k, n);
            }
            let gate_minutes = float("gate_minutes");
            let build_minutes = float("build_minutes");
            let polish_minutes = float("polish_minutes");
            for (k, v) in bad.into_iter().chain(fractions) {
                plan.push(PlanRow::new(
                    "warn",
                    format!("{rel} {k}: \"{v}\" is not a number, stored NULL"),
                ));
            }
            let shipped_at = git_out(
                root,
                &[
                    "log",
                    "--diff-filter=A",
                    "--follow",
                    "--format=%cs",
                    "--",
                    &rel,
                ],
            )
            .ok()
            .and_then(|o| {
                o.lines()
                    .rev()
                    .find(|l| !l.trim().is_empty())
                    .map(str::to_string)
            });
            let row = RetroRow {
                repo: repo.to_string(),
                feature,
                slug: slug.to_string(),
                shipped_at,
                run_id: None,
                score: ints["score"],
                gate_runs: ints["gate_runs"],
                gate_red: ints["gate_red"],
                gate_minutes,
                ci_cycles: ints["ci_cycles"],
                slices: ints["slices"],
                checkpoints: ints["checkpoints"],
                build_minutes,
                polish_minutes,
                out_tokens: ints["out_tokens"],
                cache_read_tokens: ints["cache_read_tokens"],
                start_context: ints["start_context"],
                peak_context: ints["peak_context"],
                written_at: String::new(),
            };
            plan.push(PlanRow::new("row", retro_line(&row)));
            plan.push(PlanRow::new("keep", rel));
            rows.push(row);
        }
    }
    Ok((rows, plan))
}

fn retro_line(r: &RetroRow) -> String {
    let i = |v: Option<i64>| v.map_or("NULL".into(), |n| n.to_string());
    let f = |v: Option<f64>| v.map_or("NULL".into(), number);
    format!(
        "retros ({:03} {}) score={} gate_runs={} gate_red={} gate_minutes={} ci_cycles={} slices={} checkpoints={} build_minutes={} polish_minutes={} out_tokens={} cache_read_tokens={} start_context={} peak_context={} shipped_at={}",
        r.feature,
        r.slug,
        i(r.score),
        i(r.gate_runs),
        i(r.gate_red),
        f(r.gate_minutes),
        i(r.ci_cycles),
        i(r.slices),
        i(r.checkpoints),
        f(r.build_minutes),
        f(r.polish_minutes),
        i(r.out_tokens),
        i(r.cache_read_tokens),
        i(r.start_context),
        i(r.peak_context),
        r.shipped_at.as_deref().unwrap_or("NULL")
    )
}

// ───────────────────────────────────────────────────────── plan and run

fn counts(entries: &[Entry]) -> String {
    let n = |k: &[Kind]| entries.iter().filter(|e| k.contains(&e.kind)).count();
    format!(
        "AD {}, BLOCK {}, CP {}, answer {}, review {}, note {}",
        n(&[Kind::Decision, Kind::Ask]),
        n(&[Kind::Block]),
        n(&[Kind::Checkpoint]),
        n(&[Kind::Answer]),
        n(&[Kind::Review]),
        n(&[Kind::Note])
    )
}

fn rel_of(root: &Path, p: &Path) -> PathBuf {
    p.strip_prefix(root).unwrap_or(p).to_path_buf()
}

/// What `migrate` would write. Reads only; refuses (exit 2) on anything the
/// migration cannot carry over whole.
pub fn plan(root: &Path) -> Result<Plan> {
    let index = match fs::read_to_string(root.join(INDEX)) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return refuse(format!(
                "no {INDEX}: not on the framework, nothing to migrate"
            ));
        }
        Err(e) => return Err(io(format!("read {INDEX}"))(e)),
    };
    if project::is_v3(root) {
        return refuse(format!(
            "v3: already migrated ({PROJECT} has [docs] framework = 3)"
        ));
    }
    let index_fm = frontmatter::parse(&index);
    match index_fm
        .get("framework")
        .and_then(Value::as_i64)
        .unwrap_or(1)
    {
        2 => {}
        1 => return refuse("v1: upgrade to v2 with docs.mjs first, then migrate"),
        3 => return refuse(format!("v3: already migrated ({INDEX} says framework: 3)")),
        n => return refuse(format!("framework {n}: migrate reads v2 only")),
    }
    if has_docs_table(root)? {
        return refuse(format!(
            "{PROJECT} already has [docs]: migrate writes that table, so it refuses a repo that has one"
        ));
    }
    let docs = root.join("docs");
    let live = board::scan_folder(&docs.join("features"), Place::Live)?;
    let archived = board::scan_folder(&docs.join("archive"), Place::Archive)?;
    for f in &live {
        let rd = fs::read_dir(&f.dir).map_err(io(format!("read {}", f.dir.display())))?;
        let mut frags: Vec<String> = rd
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.starts_with("log-") && n.ends_with(".md"))
            .collect();
        frags.sort();
        if let Some(n) = frags.first() {
            return refuse(format!(
                "docs/features/{}/{n} is not merged: run `factory docs log merge {:03}` first",
                f.dir_name, f.number
            ));
        }
        if f.prd.is_none() {
            return refuse(format!(
                "docs/features/{} has no prd.md — fix the v2 tree first",
                f.dir_name
            ));
        }
    }

    let mut rows = Vec::new();
    let mut files: Vec<(PathBuf, Option<String>)> = Vec::new();

    // Links first, on the files as they are, so a row names the line a
    // reader finds in the v2 file.
    let gone: Vec<PathBuf> = live
        .iter()
        .map(|f| f.dir.join(AGENT_LOG))
        .filter(|p| p.is_file())
        .collect();
    // `docs/ideas.md` goes too, but its links are pointed at the folder's
    // README rather than cut (slice 08).
    let ideas_md = root.join(migrate_ideas::FILE);
    let ideas_go = ideas_md.is_file();
    let mut texts: BTreeMap<PathBuf, String> = BTreeMap::new();
    let mut md = Vec::new();
    walk_md(&docs, &docs.join("archive"), &mut md);
    let mut unlinked: Vec<(PathBuf, String)> = Vec::new();
    for p in md
        .iter()
        .filter(|p| !gone.contains(p) && !(ideas_go && **p == ideas_md))
    {
        let text = fs::read_to_string(p).map_err(io(format!("read {}", p.display())))?;
        let rel = rel_of(root, p);
        let dir = p.parent().unwrap_or(root);
        let (new, hits) = unlink(&text, dir, &gone);
        let (new, rehits) = match ideas_go {
            true => relink(&new, dir, &normalize(&ideas_md)),
            false => (new, Vec::new()),
        };
        let mut why = Vec::new();
        if !hits.is_empty() {
            why.push("links to a deleted agent-log.md");
        }
        for (line, link) in hits {
            rows.push(PlanRow::new(
                "unlink",
                format!("{}:{line} {link}", rel.display()),
            ));
        }
        let hits = rehits;
        if !hits.is_empty() {
            why.push("links to ideas.md → ideas/README.md");
        }
        for (line, old, link) in hits {
            rows.push(PlanRow::new(
                "relink",
                format!("{}:{line} {old} → {link}", rel.display()),
            ));
        }
        if new != text {
            unlinked.push((rel.clone(), why.join("; ")));
        }
        texts.insert(rel, new);
    }
    let text_of = |rel: &Path| -> Result<String> {
        match texts.get(rel) {
            Some(t) => Ok(t.clone()),
            None => {
                fs::read_to_string(root.join(rel)).map_err(io(format!("read {}", rel.display())))
            }
        }
    };

    let numbers: Vec<u32> = live.iter().chain(&archived).map(|f| f.number).collect();
    let (index_text, index_rows) = index_md(&text_of(Path::new(INDEX))?, &numbers);
    rows.extend(index_rows);
    files.push((INDEX.into(), Some(index_text)));
    let notes = notes_cells(&index);

    for f in &live {
        let dir = PathBuf::from("docs/features").join(&f.dir_name);
        let d = dir.display().to_string();
        let prd_rel = dir.join("prd.md");
        let (prd, prd_rows, mut st) = prd_md(
            &prd_rel.display().to_string(),
            f,
            &text_of(&prd_rel)?,
            notes.get(&f.number).map_or("", String::as_str),
        )?;
        rows.push(PlanRow::new(
            "rewrite",
            format!("{d}/prd.md (FR-13 frontmatter, prose header dropped)"),
        ));
        rows.extend(prd_rows);
        files.push((prd_rel, Some(prd)));

        let mut slice_files = Vec::new();
        let mut slice_rows = Vec::new();
        for s in &f.slices {
            let rel = dir.join(&s.file);
            let r = rel.display().to_string();
            st.slices
                .insert(s.number, slice_state(&r, s.number, &s.fm, &mut slice_rows));
            let text = text_of(&rel)?;
            match strip_frontmatter(&text) {
                Some(rest) => {
                    let rest = rest.strip_prefix('\n').unwrap_or(rest);
                    slice_rows.push(PlanRow::new(
                        "rewrite",
                        format!("{r} (frontmatter → state.toml)"),
                    ));
                    slice_files.push((rel, Some(rest.to_string())));
                }
                None => slice_rows.push(PlanRow::new(
                    "warn",
                    format!("{r}: no frontmatter, [slices.{}] is pending", s.number),
                )),
            }
        }
        rows.push(PlanRow::new(
            "create",
            format!(
                "{d}/state.toml (status {}{}, {} slices)",
                st.status,
                st.reviewed
                    .as_deref()
                    .map(|r| format!(", reviewed {r}"))
                    .unwrap_or_default(),
                st.slices.len()
            ),
        ));
        if st.status == "shipped" {
            rows.push(PlanRow::new(
                "warn",
                format!("{d}/state.toml: shipped, but v2 kept no shipped date, so it has none"),
            ));
        }
        files.push((dir.join(state::FILE), Some(state::render(&st))));
        rows.extend(slice_rows);
        files.extend(slice_files);

        let log_path = f.dir.join(AGENT_LOG);
        if log_path.is_file() {
            let text = fs::read_to_string(&log_path)
                .map_err(io(format!("read {}", log_path.display())))?;
            let dates = blame_dates(root, &log_path)?;
            let log_rel = format!("{d}/{AGENT_LOG}");
            let parsed = read_log(&text, &dates).map_err(|e| match e {
                Error::Msg(m) => Error::Msg(format!("{log_rel}: {m}")),
                e => e,
            })?;
            let mut jsonl = String::new();
            for e in &parsed.entries {
                jsonl.push_str(&serde_json::to_string(e).map_err(json("serialize log entry"))?);
                jsonl.push('\n');
            }
            rows.push(PlanRow::new(
                "create",
                format!("{d}/{} ({})", log::FILE, counts(&parsed.entries)),
            ));
            rows.extend(
                parsed
                    .rows
                    .into_iter()
                    .map(|r| PlanRow::new(r.verb, format!("{log_rel} {}", r.what))),
            );
            rows.push(PlanRow::new("delete", log_rel.clone()));
            files.push((dir.join(log::FILE), Some(jsonl)));
            files.push((PathBuf::from(log_rel), None));
        }
    }
    for (rel, why) in unlinked {
        if !files.iter().any(|(p, _)| *p == rel) {
            rows.push(PlanRow::new(
                "rewrite",
                format!("{} ({why})", rel.display()),
            ));
            let text = texts[&rel].clone();
            files.push((rel, Some(text)));
        }
    }

    // `docs.lock` hidden, as `init` does: `log add` and `set` create it.
    let gi = PathBuf::from("docs/.gitignore");
    match fs::read_to_string(root.join(&gi)) {
        Ok(t) if t.lines().any(|l| l.trim() == state::LOCK) => {}
        Ok(t) => {
            let sep = if t.is_empty() || t.ends_with('\n') {
                ""
            } else {
                "\n"
            };
            rows.push(PlanRow::new(
                "rewrite",
                format!("{} (+ {})", gi.display(), state::LOCK),
            ));
            files.push((gi, Some(format!("{t}{sep}{}\n", state::LOCK))));
        }
        Err(_) => {
            rows.push(PlanRow::new(
                "create",
                format!("{} ({})", gi.display(), state::LOCK),
            ));
            files.push((gi, Some(format!("{}\n", state::LOCK))));
        }
    }

    let repo = root
        .canonicalize()
        .map_err(io(format!("canonicalize {}", root.display())))?
        .to_string_lossy()
        .into_owned();
    let (retros, retro_plan) = retro_rows(root, &repo)?;
    // A retro whose links were rewritten above is not kept as it is.
    rows.extend(retro_plan.into_iter().filter(|r| {
        r.verb != "keep" || !files.iter().any(|(p, _)| p.as_os_str() == r.what.as_str())
    }));

    let mut plan = Plan {
        rows,
        files,
        retros,
    };
    if ideas_go {
        migrate_ideas::plan(root, &mut plan)?;
    }
    let (toml, toml_rows) = project_toml(root, &index_fm)?;
    plan.rows.extend(toml_rows);
    plan.files.push((PROJECT.into(), Some(toml)));
    Ok(plan)
}

/// Refuses unless `git status --porcelain -- docs/` is empty, so the whole
/// migration is one diff the git undo restores.
pub fn clean(root: &Path) -> Result<()> {
    let out = git_out(root, &["status", "--porcelain", "--", "docs/"])
        .or_else(|e| refuse(format!("git status: {e}")))?;
    let n = out.lines().filter(|l| !l.trim().is_empty()).count();
    if n > 0 {
        return refuse(format!(
            "docs/ is not clean ({n} paths in git status --porcelain -- docs/): commit or stash first, so the migration is one diff"
        ));
    }
    Ok(())
}

/// Writes the plan: every file but `project.toml`, the deletions, the
/// `retros` rows, then `project.toml`. State goes through `write_atomic`
/// directly, never `state::set`: its `docs.lock` would survive the undo.
pub fn run(root: &Path, plan: &Plan, db: &Db) -> Result<()> {
    let project = Path::new(PROJECT);
    for (rel, text) in plan.files.iter().filter(|(p, _)| p != project) {
        let p = root.join(rel);
        match text {
            Some(t) => {
                if let Some(dir) = p.parent() {
                    fs::create_dir_all(dir).map_err(io(format!("mkdir {}", dir.display())))?;
                }
                write_atomic(&p, t)?
            }
            None => fs::remove_file(&p).map_err(io(format!("delete {}", p.display())))?,
        }
    }
    let now = clock::now_rfc3339();
    for r in &plan.retros {
        db.upsert_retro(&RetroRow {
            written_at: now.clone(),
            ..r.clone()
        })?;
    }
    if let Some((rel, Some(t))) = plan.files.iter().find(|(p, _)| p == project) {
        write_atomic(&root.join(rel), t)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(n: usize) -> Vec<String> {
        vec!["2026-09-30".to_string(); n]
    }

    fn by_id<'a>(es: &'a [Entry], id: &str) -> &'a Entry {
        es.iter().find(|e| e.id.as_deref() == Some(id)).expect(id)
    }

    fn refs<'a>(es: &'a [Entry], kind: Kind, id: &str) -> Vec<&'a Entry> {
        es.iter()
            .filter(|e| e.kind == kind && e.reference.as_deref() == Some(id))
            .collect()
    }

    /// A file of this repo's v2 docs as frozen in `tests/fixture/` (its
    /// README says why: the live `docs/` moves with every commit).
    fn frozen(rel: &str) -> String {
        let p = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixture/repo-at-a7e89bf")
            .join(rel);
        fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
    }

    fn head(rel: &str) -> Option<String> {
        Some(frozen(rel))
    }

    fn head_log(rel: &str) -> Option<String> {
        Some(frozen(rel))
    }

    #[test]
    fn a_decision_keeps_its_fence_flag_and_owner_review() {
        let log = "## Decisions\n\n### AD-3 — Why? · slice-02 · 2026-10-10\n\n**Chose:** a\n```md\n### AD-9 — inside · slice-01 · 2026-10-10\n```\n**Review:** yes\n**Owner (2026-10-11):** kept, as refined by AD-7\n\n### AD-4 — Then? · slice-02 · 2026-10-10\n\n**Review:** decide\n\n**Owner (2026-10-12):** changed — the owner's text\n";
        let es = parse_agent_log(log, &d(20)).unwrap();
        assert!(es.iter().all(|e| e.id.as_deref() != Some("AD-9")));
        let ad = by_id(&es, "AD-3");
        assert_eq!(ad.kind, Kind::Decision);
        assert_eq!(ad.flag, Some(Flag::Fyi));
        assert_eq!(ad.slice, Some(2));
        assert_eq!(ad.ts, "2026-10-10T00:00:00Z");
        assert_eq!(
            ad.body,
            "**Chose:** a\n```md\n### AD-9 — inside · slice-01 · 2026-10-10\n```"
        );
        let r = refs(&es, Kind::Review, "AD-3");
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].verdict, Some(Verdict::Kept));
        assert_eq!(r[0].body, "as refined by AD-7");
        assert_eq!(r[0].ts, "2026-10-11T00:00:00Z");
        let r = refs(&es, Kind::Review, "AD-4");
        assert_eq!(r[0].verdict, Some(Verdict::Changed));
        assert_eq!(r[0].body, "the owner's text");
        assert_eq!(by_id(&es, "AD-4").flag, Some(Flag::Decide));
        assert_eq!(by_id(&es, "AD-4").body, "");
    }

    #[test]
    fn a_preflight_ad_with_review_no_is_an_ask() {
        let log =
            "### AD-1 — Preflight q: x · preflight · 2026-10-12\n\n**Chose:** y\n**Review:** no\n";
        let es = parse_agent_log(log, &d(5)).unwrap();
        let a = by_id(&es, "AD-1");
        assert_eq!(a.kind, Kind::Ask);
        assert_eq!(a.phase.as_deref(), Some("preflight"));
        assert_eq!(a.flag, None);
        assert_eq!(a.title, "Preflight q: x");
        assert!(a.body.ends_with("**Review:** no"), "{}", a.body);
    }

    #[test]
    fn ids_come_out_ascending_and_blocks_get_answers() {
        let log = "## Blockers\n\n_(none)_\n\n### BLOCK-2 — b · slice-02 · resolved\n\n**Happened:** x\n\n### BLOCK-1 — a · polish · resolved\n\n**Happened:** y\n**Resolution (2026-10-03):** do z\nand more\n\n## Decisions\n\n### AD-201 — late · slice-02 · 20261013\n\n### AD-101 — early · slice-01 · 2026-10-01\n";
        let p = read_log(log, &d(30)).unwrap();
        let ids: Vec<&str> = p.entries.iter().filter_map(|e| e.id.as_deref()).collect();
        assert_eq!(ids, ["BLOCK-1", "BLOCK-2", "AD-101", "AD-201"]);
        let a = refs(&p.entries, Kind::Answer, "BLOCK-1");
        assert_eq!(a[0].body, "do z\nand more");
        assert_eq!(a[0].ts, "2026-10-03T00:00:00Z");
        let a = refs(&p.entries, Kind::Answer, "BLOCK-2");
        assert_eq!(a.len(), 1);
        assert!(a[0].body.contains("no Resolution line"));
        assert_eq!(
            by_id(&p.entries, "BLOCK-1").phase.as_deref(),
            Some("polish")
        );
        assert_eq!(by_id(&p.entries, "AD-201").ts, "2026-10-13T00:00:00Z");
        assert!(
            p.rows
                .iter()
                .any(|r| r.verb == "fix" && r.what.contains("20261013"))
        );
        assert!(
            p.entries.iter().all(|e| e.kind != Kind::Note),
            "{:?}",
            p.entries
        );
    }

    #[test]
    fn a_duplicate_id_refuses_with_both_lines() {
        let log = "### AD-1 — a · slice-01 · 2026-10-01\n\n### AD-1 — b · slice-01 · 2026-10-01\n";
        let e = parse_agent_log(log, &d(4)).unwrap_err().to_string();
        assert!(e.contains("lines 1 and 3"), "{e}");
    }

    #[test]
    fn a_checkpoint_ends_at_the_rule_and_the_template_is_a_note() {
        let log = "# Agent log — PRD-009\n\n## Checkpoints\n\n### CP-1 — after slice-02 · 2026-10-05\n\n**Covers:** 01, 02\n\n---\n\n```markdown\n### AD-1 — x · slice-02 · 2026-10-01\n```\n";
        let es = parse_agent_log(log, &d(14)).unwrap();
        let cp = by_id(&es, "CP-1");
        assert_eq!(cp.kind, Kind::Checkpoint);
        assert_eq!(cp.slice, Some(2));
        assert_eq!(cp.body, "**Covers:** 01, 02");
        let notes: Vec<&Entry> = es.iter().filter(|e| e.kind == Kind::Note).collect();
        assert_eq!(notes.len(), 1);
        assert!(
            notes[0].body.starts_with("---\n\n```markdown"),
            "{}",
            notes[0].body
        );
        assert_eq!(notes[0].ts, "2026-09-30T00:00:00Z");
    }

    #[test]
    fn the_v2_template_log_gives_notes_only() {
        // The template docs.mjs writes into a new feature; 002's own log held
        // only this at slicing, and real entries since checkpoint 1.
        let t = include_str!("../../tests/fixture/v2/.docs/templates/agent-log.md");
        let es = parse_agent_log(t, &[]).unwrap();
        assert!(!es.is_empty());
        assert!(es.iter().all(|e| e.kind == Kind::Note));
        let all: String = es.iter().map(|e| e.body.as_str()).collect();
        assert!(
            !all.contains("## Decisions\n"),
            "structural headings are dropped"
        );
        assert!(all.contains("```markdown"), "the fenced examples stay");
    }

    #[test]
    fn prd_001_log_at_head_keeps_every_entry() {
        let Some(t) = head_log("docs/features/001-driver/agent-log.md") else {
            return;
        };
        let es = parse_agent_log(&t, &[]).unwrap();
        let n = |k: Kind| es.iter().filter(|e| e.kind == k).count();
        assert_eq!(n(Kind::Decision) + n(Kind::Ask), 145);
        assert_eq!(n(Kind::Checkpoint), 7);
        assert_eq!(n(Kind::Block), 1);
        assert_eq!(refs(&es, Kind::Answer, "BLOCK-201").len(), 1);
        assert_eq!(n(Kind::Answer), 1);
        assert_eq!(n(Kind::Review), 78);
        for e in es.iter().filter(|e| e.kind.sequence() == Some("AD")) {
            if e.n().unwrap() < 101 {
                assert_eq!(e.phase.as_deref(), Some("preflight"), "{:?}", e.id);
            }
        }
        // Each sequence ascending, the polish/checkpoint phase kept.
        let ads: Vec<u32> = es
            .iter()
            .filter(|e| e.kind.sequence() == Some("AD"))
            .filter_map(Entry::n)
            .collect();
        assert!(ads.windows(2).all(|w| w[0] < w[1]));
        assert!(es.iter().any(|e| e.phase.as_deref() == Some("polish")));
        // The rules prose after the last CP is a note, not its body.
        let last = es.iter().rfind(|e| e.kind == Kind::Checkpoint).unwrap();
        assert!(!last.body.contains("```"), "{}", last.body);
    }

    #[test]
    fn blame_porcelain_dates_each_line_in_its_zone() {
        let sha = "a".repeat(40);
        let other = "b".repeat(40);
        let text = format!(
            "{sha} 1 1 2\nauthor X\nauthor-time 1759233600\nauthor-tz +0000\n\tline one\n{sha} 2 2\n\tline two\n{other} 3 3 1\nauthor-time 1759276800\nauthor-tz -0300\n\tthree\n"
        );
        assert_eq!(
            parse_blame(&text),
            ["2025-09-30", "2025-09-30", "2025-09-30"]
        );
    }

    #[test]
    fn index_md_keeps_the_prose_around_the_board() {
        let Some(t) = head("docs/index.md") else {
            return;
        };
        let (out, rows) = index_md(&t, &[1, 2, 3]);
        assert!(!out.starts_with("---"), "{out}");
        assert!(out.lines().all(|l| !l.trim_start().starts_with('|')));
        assert!(!out.contains("Next free PRD number"));
        assert!(!out.contains("Next free FR"));
        assert!(
            out.contains("Next free number wins"),
            "lifecycle prose stays"
        );
        assert!(out.contains("## Live features"));
        assert!(out.contains("## Archived"));
        assert!(out.contains("**Retired numbers — never reused:**"));
        let fl = t.find("## Feature lifecycle").unwrap();
        for l in t[fl..].lines() {
            assert!(out.contains(l), "lost {l}");
        }
        let lines: Vec<&str> = out.lines().collect();
        let b = lines
            .iter()
            .position(|l| l.starts_with("**Build order:**"))
            .unwrap();
        let e = (b..lines.len()).find(|&i| lines[i].is_empty()).unwrap();
        assert!(
            lines[e + 1].starts_with("**FR numbers:** PRDs written before v3 (PRD-001 … PRD-003)")
        );
        assert!(!out.contains("\n\n\n"));
        assert_eq!(rows[0].verb, "rewrite");
    }

    #[test]
    fn counters_go_with_their_bold_markers() {
        let l = "**Next free PRD number: 004.** **Next free FR: FR-26.**";
        let a = cut_counter(l, "next free prd number:", "").unwrap();
        assert_eq!(a, "**Next free FR: FR-26.**");
        assert_eq!(cut_counter(&a, "next free fr:", "FR-").unwrap(), "");
        assert_eq!(
            cut_counter("Plain. Next free FR: FR-3. More.", "next free fr:", "FR-").unwrap(),
            "Plain. More."
        );
    }

    #[test]
    fn project_toml_takes_the_loadproject_values() {
        let dir = tempfile::tempdir().unwrap();
        let fm =
            frontmatter::parse("---\nframework: 2\nlanes: 0\ngate_parallel: true\nfoo: 1\n---\n");
        let (t, rows) = project_toml(dir.path(), &fm).unwrap();
        assert_eq!(
            t,
            "[docs]\nframework = 3\ngate_parallel = true\nlanes = 1\ncheckpoint_every = 2\nprd_max_kb = 120\nslice_max_kb = 30\n"
        );
        assert!(rows.contains(&PlanRow::new("drop", "docs/index.md key: foo")));
        let parsed: toml::Table = t.parse().unwrap();
        assert_eq!(parsed["docs"]["lanes"].as_integer(), Some(1));

        fs::create_dir_all(dir.path().join("docs")).unwrap();
        let old = "[workers.polish]\nmodel = \"sonnet\"\n";
        fs::write(dir.path().join(PROJECT), old).unwrap();
        let (t, _) = project_toml(dir.path(), &fm).unwrap();
        assert!(t.starts_with(&format!("{old}\n[docs]\n")), "{t}");
        fs::write(dir.path().join(PROJECT), t).unwrap();
        assert!(
            project_toml(dir.path(), &fm).is_err(),
            "[docs] already there"
        );
    }

    fn feature_at(dir: &Path, name: &str) -> Feature {
        board::scan_folder(dir, Place::Live)
            .unwrap()
            .into_iter()
            .find(|f| f.dir_name == name)
            .unwrap()
    }

    #[test]
    fn this_repos_prds_get_the_seven_keys() {
        let (Some(index), Some(p1), Some(p2), Some(p3)) = (
            head("docs/index.md"),
            head("docs/features/001-driver/prd.md"),
            head("docs/features/002-docs-port/prd.md"),
            head("docs/features/003-observatory/prd.md"),
        ) else {
            return;
        };
        let notes = notes_cells(&index);
        let tmp = tempfile::tempdir().unwrap();
        let mut outs = Vec::new();
        for (name, text) in [
            ("001-driver", &p1),
            ("002-docs-port", &p2),
            ("003-observatory", &p3),
        ] {
            fs::create_dir_all(tmp.path().join(name)).unwrap();
            fs::write(tmp.path().join(name).join("prd.md"), text).unwrap();
            let f = feature_at(tmp.path(), name);
            let (out, _, st) = prd_md("prd.md", &f, text, &notes[&f.number]).unwrap();
            let fm = frontmatter::parse(&out);
            let keys: Vec<&str> = fm
                .data
                .as_ref()
                .unwrap()
                .iter()
                .map(|(k, _)| k.as_str())
                .collect();
            assert_eq!(
                keys,
                [
                    "number", "slug", "title", "type", "created", "author", "summary"
                ]
            );
            assert!(PRD_TYPES.contains(&fm.str("type").unwrap()));
            assert_eq!(fm.str("summary").unwrap().trim(), notes[&f.number].trim());
            outs.push((out, st));
        }
        assert!(
            outs[0]
                .0
                .contains("title: \"Driver: the unattended deliver loop\"\n")
        );
        assert!(outs[0].0.contains("created: 2026-10-09\n"));
        assert_eq!(
            state::render(&outs[2].1).lines().nth(1),
            Some("reviewed = \"2026-10-13\"")
        );
        assert!(outs[1].0.contains("**Split from / Related**:"));
        assert!(!outs[1].0.contains("**Owns FR range**:"));
        assert!(outs[1].0.lines().all(|l| !l.starts_with("**Status**:")));
    }

    #[test]
    fn a_slice_file_becomes_its_slices_entry() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("001-x");
        fs::create_dir_all(&dir).unwrap();
        let text =
            "---\nslice: 03\nstatus: building\nlane: 1\ndepends_on: [2]\n---\n\n# Slice 03 — c\n";
        fs::write(dir.join("slice-03-c.md"), text).unwrap();
        let f = feature_at(tmp.path(), "001-x");
        let mut rows = Vec::new();
        let s = slice_state("x", 3, &f.slices[0].fm, &mut rows);
        assert_eq!(
            s,
            SliceState {
                status: "building".into(),
                lane: Some(1),
                depends_on: vec![2]
            }
        );
        assert!(rows.is_empty(), "{rows:?}");
        let rest = strip_frontmatter(text).unwrap();
        assert_eq!(rest.strip_prefix('\n').unwrap(), "# Slice 03 — c\n");
        let mut st = State {
            status: "in-progress".into(),
            ..State::default()
        };
        st.slices.insert(3, s);
        assert_eq!(
            state::render(&st),
            "status = \"in-progress\"\n\n[slices.3]\nstatus = \"building\"\nlane = 1\ndepends_on = [2]\n"
        );
    }

    #[test]
    fn unlink_keeps_the_link_text() {
        let dir = Path::new("/r/docs/features/001-x");
        let gone = vec![dir.join("agent-log.md")];
        let (out, hits) = unlink(
            "a\nSee [the log](agent-log.md#ad-3). [prd](prd.md)",
            dir,
            &gone,
        );
        assert_eq!(out, "a\nSee the log. [prd](prd.md)");
        assert_eq!(hits, [(2, "[the log](agent-log.md#ad-3)".to_string())]);
        let (out, _) = unlink(
            "[x](../001-x/agent-log.md)",
            &dir.parent().unwrap().join("002-y"),
            &gone,
        );
        assert_eq!(out, "x");
    }

    fn git(root: &Path, args: &[&str]) {
        let ok = Command::new("git")
            .args(args)
            .current_dir(root)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .output()
            .unwrap()
            .status
            .success();
        assert!(ok, "git {args:?}");
    }

    fn v2_tree() -> tempfile::TempDir {
        v2_tree_with(&[])
    }

    /// The small v2 tree, plus `extra` `(path, text)` files in its commit.
    fn v2_tree_with(extra: &[(&str, &str)]) -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let w = |rel: &str, text: &str| {
            let p = root.join(rel);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, text).unwrap();
        };
        w(
            "docs/index.md",
            "---\nframework: 2\n---\n\n# Index\n\n## Live features\n\n| # | Notes |\n|---|---|\n| 001 | the x |\n",
        );
        w(
            "docs/features/001-x/prd.md",
            "---\nnumber: 001\nslug: x\nstatus: ready\n---\n\n# PRD-001 — X\n\n**Type**: Feature\n**Created**: 2026-10-01\n**Author**: me\n\n## 1. Why\n",
        );
        w(
            "docs/features/001-x/agent-log.md",
            "## Decisions\n\n### AD-1 — q · slice-01 · 2026-10-02\n\n**Review:** fyi\n",
        );
        w(
            "docs/archive/000-old/prd.md",
            "---\nstatus: archived\n---\n",
        );
        w(
            "docs/archive/000-old/retro.md",
            "---\nscore: 3\ngate_minutes: n/a\n---\n# Retro\n",
        );
        for (rel, text) in extra {
            w(rel, text);
        }
        git(root, &["init", "-q"]);
        git(root, &["add", "-A"]);
        git(
            root,
            &[
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "commit",
                "-q",
                "-m",
                "v2",
            ],
        );
        tmp
    }

    #[test]
    fn plan_is_stable_and_reads_only() {
        let tmp = v2_tree();
        let root = tmp.path();
        let a = plan(root).unwrap();
        let b = plan(root).unwrap();
        assert_eq!(a, b);
        assert!(!root.join(PROJECT).exists());
        let r = &a.retros[0];
        assert_eq!(
            (r.feature, r.score, r.gate_minutes, r.run_id.clone()),
            (0, Some(3), None, None)
        );
        assert!(
            a.rows
                .iter()
                .any(|r| r.verb == "warn" && r.what.contains("gate_minutes"))
        );
        assert!(
            a.rows
                .contains(&PlanRow::new("keep", "docs/archive/000-old/retro.md"))
        );
        assert!(a.files.iter().all(|(p, _)| !p.starts_with("docs/archive")));
        assert_eq!(a.files.last().unwrap().0, Path::new(PROJECT));
    }

    #[test]
    fn an_unmerged_fragment_refuses() {
        let tmp = v2_tree();
        fs::write(
            tmp.path().join("docs/features/001-x/log-slice-02.md"),
            "## Decisions\n",
        )
        .unwrap();
        let e = plan(tmp.path()).unwrap_err().to_string();
        assert!(
            e.contains("log-slice-02.md is not merged: run `factory docs log merge 001` first"),
            "{e}"
        );
    }

    #[test]
    fn a_bad_reviewed_refuses() {
        let tmp = v2_tree();
        let p = tmp.path().join("docs/features/001-x/prd.md");
        let t = fs::read_to_string(&p)
            .unwrap()
            .replace("status: ready", "status: ready\nreviewed: soon");
        fs::write(&p, t).unwrap();
        let e = plan(tmp.path()).unwrap_err().to_string();
        assert!(e.contains("reviewed \"soon\" is not a date"), "{e}");
    }

    #[test]
    fn this_repos_retros_at_head() {
        let (Some(r1), Some(r3)) = (
            head("docs/features/001-driver/retro.md"),
            head("docs/features/003-observatory/retro.md"),
        ) else {
            return;
        };
        let tmp = tempfile::tempdir().unwrap();
        for (name, t) in [("001-driver", &r1), ("003-observatory", &r3)] {
            fs::create_dir_all(tmp.path().join("docs/features").join(name)).unwrap();
            fs::write(
                tmp.path().join("docs/features").join(name).join("retro.md"),
                t,
            )
            .unwrap();
        }
        let (rows, _) = retro_rows(tmp.path(), "r").unwrap();
        assert_eq!(rows[0].score, Some(4));
        assert_eq!(rows[0].gate_minutes, Some(16.0));
        assert_eq!(rows[0].out_tokens, Some(3772425));
        assert_eq!(rows[0].peak_context, Some(182520));
        assert_eq!(rows[1].score, None);
        assert_eq!(rows[1].gate_minutes, Some(2.23));
        assert_eq!(rows[1].shipped_at, None, "not a git repo");
    }

    #[test]
    fn a_review_line_keeps_its_reason() {
        let log = "## Decisions\n\n### AD-1 — q · slice-01 · 2026-10-02\n\n**Chose:** a\n\n**Review:** decide — deviates from the slice\n\n**Owner (2026-10-03):** kept\n\n### AD-2 — r · slice-01 · 2026-10-02\n\n**Review:** fyi\n";
        let es = parse_agent_log(log, &d(20)).unwrap();
        let a = by_id(&es, "AD-1");
        assert_eq!(a.flag, Some(Flag::Decide));
        assert_eq!(
            a.body,
            "**Chose:** a\n\n**Review:** decide — deviates from the slice"
        );
        assert_eq!(refs(&es, Kind::Review, "AD-1").len(), 1);
        let b = by_id(&es, "AD-2");
        assert_eq!((b.flag, b.body.as_str()), (Some(Flag::Fyi), ""));
        let shown = log::render(&es, log::Filter::All, 1);
        assert!(
            shown.contains("\n**Review:** decide — deviates from the slice\n"),
            "{shown}"
        );
        assert_eq!(shown.matches("**Review:** decide").count(), 1, "{shown}");
        assert!(shown.contains("\n**Review:** fyi\n"), "{shown}");
    }

    #[test]
    fn a_middle_dot_in_a_title_stays_in_the_title() {
        let log = "### AD-77 — Title with a · middle dot · slice-02 · 2026-10-02\n\n### BLOCK-3 — Gate · red run · polish · open\n";
        let es = parse_agent_log(log, &d(5)).unwrap();
        let a = by_id(&es, "AD-77");
        assert_eq!(a.title, "Title with a · middle dot");
        assert_eq!((a.slice, a.ts.as_str()), (Some(2), "2026-10-02T00:00:00Z"));
        let b = by_id(&es, "BLOCK-3");
        assert_eq!(
            (b.title.as_str(), b.phase.as_deref()),
            ("Gate · red run", Some("polish"))
        );
        let shown = log::render(&es, log::Filter::All, 1);
        for h in log.lines().filter(|l| l.starts_with("### ")) {
            assert!(shown.contains(h), "{h}\n{shown}");
        }
    }

    #[test]
    fn a_pipe_in_a_code_span_stays_in_its_cell() {
        assert_eq!(
            cells("| 002 | `log add | show` and \\| x | ``a ` | b`` |"),
            ["002", "`log add | show` and | x", "``a ` | b``"]
        );
        assert_eq!(
            cells("| a ` b | c |"),
            ["a ` b", "c"],
            "an unclosed backtick"
        );
        let index = "## Live features\n\n| # | Notes |\n|---|---|\n| 002 | ports `log add | show`, then more |\n";
        assert_eq!(notes_cells(index)[&2], "ports `log add | show`, then more");
    }

    #[test]
    fn yaml_values_read_back_as_written() {
        for v in [
            "plain",
            "A: b",
            "say \"hi\": now",
            "it's: x",
            "it's \"x\": y",
            "\"lead",
            "'lead",
            "[x]",
            "a \\ b: c",
            "it's a \\ b: c",
            "x #y",
            "|",
        ] {
            let y = yaml(v);
            let fm = frontmatter::parse(&format!("---\ntitle: {y}\nnext: 1\n---\n"));
            assert_eq!(fm.str("title"), Some(v), "{v} as {y}");
            assert_eq!(fm.get("next"), Some(&Value::Num(1)), "{v} as {y}");
        }
        assert_eq!(yaml("say \"hi\": now"), "'say \"hi\": now'");
        assert_eq!(yaml("it's: x"), "\"it's: x\"");
        assert_eq!(yaml("it's \"x\": y"), "|-\n  it's \"x\": y");
    }

    #[test]
    fn a_retros_links_are_rewritten_like_any_other() {
        let fm = "---\nscore: 3\n---\n";
        let retro = format!(
            "{fm}# Retro\n\nSee [the log](agent-log.md#ad-1) and [ideas](../../ideas.md).\n"
        );
        let tmp = v2_tree_with(&[
            ("docs/features/001-x/retro.md", &retro),
            ("docs/ideas.md", "## Open\n\n- **A.** a\n"),
        ]);
        let p = plan(tmp.path()).unwrap();
        let rel = "docs/features/001-x/retro.md";
        let (_, text) = p
            .files
            .iter()
            .find(|(f, _)| f == Path::new(rel))
            .unwrap_or_else(|| panic!("{:?}", p.files));
        assert_eq!(
            text.as_deref(),
            Some(
                format!("{fm}# Retro\n\nSee the log and [ideas](../../ideas/README.md).\n")
                    .as_str()
            ),
            "the frontmatter stays, the two links move"
        );
        for (verb, what) in [
            ("unlink", format!("{rel}:6 [the log](agent-log.md#ad-1)")),
            (
                "relink",
                format!("{rel}:6 [ideas](../../ideas.md) → [ideas](../../ideas/README.md)"),
            ),
        ] {
            assert!(p.rows.contains(&PlanRow::new(verb, what)), "{:#?}", p.rows);
        }
        assert!(
            p.rows
                .iter()
                .any(|r| r.verb == "rewrite" && r.what.starts_with(rel)),
            "{:#?}",
            p.rows
        );
        assert!(
            p.rows
                .iter()
                .all(|r| r.verb != "warn" || !r.what.starts_with(rel)),
            "{:#?}",
            p.rows
        );
        assert!(
            !p.rows.contains(&PlanRow::new("keep", rel)),
            "{:#?}",
            p.rows
        );
        let row = p.retros.iter().find(|r| r.feature == 1).unwrap();
        assert_eq!(row.score, Some(3), "its numbers still make a row");
    }
}
