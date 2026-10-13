//! The owner's half of a parked run (PRD §5 FR-3, §3 FR-1): `block` entries
//! of the feature's `log.jsonl`, their resolution as an `answer` entry
//! (PRD-002 FR-14), and `runs/<id>/answers/<id>.json`,
//! the file the driver's slow tick watches for both `BLOCK-n` and `ASK-n`.
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::docs::log;
use crate::error::{io, json};
use crate::events::EventKind;
use crate::run::Run;
use crate::{Error, Result};

pub const DIR: &str = "answers";

/// `runs/<id>/answers/<id>.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Answer {
    /// `BLOCK-n` or `ASK-n`.
    pub id: String,
    /// The slice the block named; `None` for an ask or a phase block.
    #[serde(default)]
    pub slice: Option<u32>,
    pub text: String,
    pub at: String,
}

/// `runs/<id>/answers`.
pub fn dir(run_dir: &Path) -> PathBuf {
    run_dir.join(DIR)
}

/// `runs/<id>/answers/<id>.json`.
pub fn path(run_dir: &Path, id: &str) -> PathBuf {
    dir(run_dir).join(format!("{id}.json"))
}

/// Through a temporary file and a rename: the driver's slow tick reads this
/// directory while `factory answer` writes it, and must never see half a file.
pub fn write(run_dir: &Path, answer: &Answer) -> Result<()> {
    let d = dir(run_dir);
    fs::create_dir_all(&d).map_err(io(format!("mkdir {}", d.display())))?;
    let p = path(run_dir, &answer.id);
    let tmp = d.join(format!(".{}.json.tmp", answer.id));
    let text = serde_json::to_string_pretty(answer).map_err(json("serialize answer"))?;
    fs::write(&tmp, text + "\n").map_err(io(format!("write {}", tmp.display())))?;
    fs::rename(&tmp, &p).map_err(io(format!("rename {} to {}", tmp.display(), p.display())))
}

/// The answer to `id`. A file that does not parse is no answer: an ask stays
/// open, and `factory answer ASK-n` writes it again.
pub fn read(run_dir: &Path, id: &str) -> Result<Option<Answer>> {
    let p = path(run_dir, id);
    match fs::read_to_string(&p) {
        Ok(text) => Ok(serde_json::from_str(&text).ok()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(io(format!("read {}", p.display()))(e)),
    }
}

/// Every answer on disk, by id.
pub fn all(run_dir: &Path) -> Result<Vec<Answer>> {
    Ok(scan(run_dir)?.0)
}

/// An answer file that does not parse: `(file name, error)`.
pub type Unreadable = (String, String);

/// Every answer on disk, by id, and each `.json` file that does not parse:
/// skipped, for the driver to say so, never a reason to fail the run.
pub fn scan(run_dir: &Path) -> Result<(Vec<Answer>, Vec<Unreadable>)> {
    let d = dir(run_dir);
    let entries = match fs::read_dir(&d) {
        Ok(rd) => rd,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok((Vec::new(), Vec::new())),
        Err(e) => return Err(io(format!("read {}", d.display()))(e)),
    };
    let mut out: Vec<Answer> = Vec::new();
    let mut bad: Vec<Unreadable> = Vec::new();
    for entry in entries {
        let p = entry.map_err(io(format!("read {}", d.display())))?.path();
        if p.extension().is_some_and(|e| e == "json") {
            let text = fs::read_to_string(&p).map_err(io(format!("read {}", p.display())))?;
            match serde_json::from_str(&text) {
                Ok(a) => out.push(a),
                Err(e) => bad.push((
                    p.file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                    e.to_string(),
                )),
            }
        }
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    bad.sort();
    Ok((out, bad))
}

/// The latest block answer naming `slice`, if any: what tells the driver a
/// `building` slice with nothing alive on it was answered, not abandoned.
pub fn block_answer_for_slice(run_dir: &Path, slice: u32) -> Result<Option<Answer>> {
    let mut hits: Vec<Answer> = all(run_dir)?
        .into_iter()
        .filter(|a| a.id.starts_with("BLOCK-") && a.slice == Some(slice))
        .collect();
    hits.sort_by(|a, b| a.at.cmp(&b.at));
    Ok(hits.pop())
}

/// The `ASK-n` items of a run (its `ask_opened` events) that have no answer
/// file yet, as `(id, text)`.
pub fn asks_pending(run: &Run) -> Result<Vec<(String, String)>> {
    let mut out = Vec::new();
    for ev in run.events()? {
        if let EventKind::AskOpened { id, text } = ev.kind
            && read(run.dir(), &id)?.is_none()
        {
            out.push((id, text));
        }
    }
    Ok(out)
}

// ── BLOCK entries in the feature's log ──────────────────────────────────────

/// A `block` entry of the feature's `log.jsonl` (PRD-002 FR-14) and the
/// `answer` entries that point at it: resolved once one does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockEntry {
    pub id: String,
    pub n: u32,
    pub title: String,
    pub slice: Option<u32>,
    pub phase: Option<String>,
    pub body: String,
    /// `(date, text)` of each answer, in log order.
    pub answers: Vec<(String, String)>,
    pub resolved: bool,
    /// The feature's `log.jsonl`.
    pub path: PathBuf,
    /// The block's line in it, 1-based.
    pub line: usize,
}

/// The longest title [`block_title`] keeps, in chars.
pub const TITLE_MAX: usize = 160;

/// `raw` as a block title: one line, no `·` (the separator `log show`
/// renders the heading with), at most [`TITLE_MAX`] chars. Every title the
/// driver builds from tool output — a red gate's last line, a refused ship —
/// goes through here.
pub fn block_title(raw: &str) -> String {
    let one = raw
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace('·', "-");
    if one.chars().count() <= TITLE_MAX {
        return one;
    }
    let mut cut: String = one.chars().take(TITLE_MAX - 1).collect();
    cut.truncate(cut.trim_end().len());
    cut.push('…');
    cut
}

/// Every block in the feature's log, in file order, with its answers. A line
/// that does not parse is skipped: `check` reports it.
pub fn entries(feature_dir: &Path) -> Result<Vec<BlockEntry>> {
    let path = log::path(feature_dir);
    let text = match fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(io(format!("read {}", path.display()))(e)),
    };
    let lines: Vec<(usize, log::Entry)> = text
        .lines()
        .enumerate()
        .filter_map(|(i, l)| Some((i + 1, serde_json::from_str(l).ok()?)))
        .collect();
    let mut out: Vec<BlockEntry> = lines
        .iter()
        .filter(|(_, e)| e.kind == log::Kind::Block)
        .filter_map(|(line, e)| {
            Some(BlockEntry {
                id: e.id.clone()?,
                n: e.n()?,
                title: e.title.clone(),
                slice: e.slice,
                phase: e.phase.clone(),
                body: e.body.clone(),
                answers: Vec::new(),
                resolved: false,
                path: path.clone(),
                line: *line,
            })
        })
        .collect();
    for (_, a) in lines.iter().filter(|(_, e)| e.kind == log::Kind::Answer) {
        if let Some(b) = out
            .iter_mut()
            .find(|b| a.reference.as_deref() == Some(b.id.as_str()))
        {
            let date = a.ts.get(..10).unwrap_or(&a.ts).to_string();
            b.answers.push((date, a.body.clone()));
            b.resolved = true;
        }
    }
    Ok(out)
}

/// The block `id` (`BLOCK-n`) in the feature's log.
pub fn find(feature_dir: &Path, id: &str) -> Result<Option<BlockEntry>> {
    Ok(entries(feature_dir)?.into_iter().find(|e| e.id == id))
}

/// The open block of `slice`: the last unanswered block naming it. What
/// `slice_blocked` reports.
pub fn open_block_of_slice(feature_dir: &Path, slice: u32) -> Result<Option<BlockEntry>> {
    Ok(entries(feature_dir)?
        .into_iter()
        .rfind(|e| !e.resolved && e.slice == Some(slice)))
}

/// The entry as `log show` renders a block: the heading, the body and each
/// answer as a `**Resolution (date):**` line.
pub fn entry_text(entry: &BlockEntry) -> String {
    let mut place = String::new();
    if let Some(s) = entry.slice {
        place.push_str(&format!(" · slice-{s:02}"));
    }
    if let Some(p) = &entry.phase {
        place.push_str(&format!(" · {p}"));
    }
    let state = if entry.resolved { "resolved" } else { "open" };
    let mut text = format!("### {} — {}{place} · {state}", entry.id, entry.title);
    if !entry.body.trim().is_empty() {
        text.push_str("\n\n");
        text.push_str(entry.body.trim_end());
    }
    for (date, answer) in &entry.answers {
        text.push_str(&format!("\n**Resolution ({date}):** {answer}"));
    }
    text
}

/// A `block` entry with its phase and slice, written by the tool; its id.
pub fn add_block(
    feature_dir: &Path,
    slice: Option<u32>,
    phase: Option<&str>,
    title: &str,
    body: &str,
) -> Result<String> {
    let mut e = log::Entry::new(log::Kind::Block, &block_title(title), body);
    e.slice = slice;
    e.phase = phase.map(str::to_string);
    log::add(feature_dir, e)?.ok_or_else(|| Error::Msg("log add returned no block id".into()))
}

/// A `note` entry (a trap, a leftover list) with its slice.
pub fn add_note(feature_dir: &Path, slice: Option<u32>, title: &str, body: &str) -> Result<()> {
    let mut e = log::Entry::new(log::Kind::Note, title, body);
    e.slice = slice;
    log::add(feature_dir, e).map(|_| ())
}

/// `autonomy.md` § Resuming step 1, on the log: an `answer` entry with
/// `ref` = the block's id. The block line is left as written.
pub fn resolve(feature_dir: &Path, entry: &BlockEntry, text: &str) -> Result<()> {
    let mut e = log::Entry::new(log::Kind::Answer, "", text);
    e.slice = entry.slice;
    e.reference = Some(entry.id.clone());
    log::add(feature_dir, e).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_title_from_tool_output_is_one_line_and_capped() {
        let raw = "error[E0425]: cannot find value · in scope\n  --> src/a.rs:3 · resolved";
        assert_eq!(
            block_title(raw),
            "error[E0425]: cannot find value - in scope --> src/a.rs:3 - resolved"
        );
        let long = block_title(&"x".repeat(500));
        assert_eq!(long.chars().count(), TITLE_MAX);
        assert!(long.ends_with('…'));
    }

    #[test]
    fn a_block_is_resolved_by_an_answer_and_its_line_stays() {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        let id = add_block(d, None, Some("checkpoint"), "gate red", "**Happened:** x").unwrap();
        assert_eq!(id, "BLOCK-1");
        let id2 = add_block(d, Some(2), None, "Which option?", "b").unwrap();
        assert_eq!(id2, "BLOCK-2");
        let before = fs::read_to_string(log::path(d)).unwrap();
        let e = find(d, "BLOCK-1").unwrap().unwrap();
        assert_eq!(e.phase.as_deref(), Some("checkpoint"));
        assert_eq!(e.line, 1);
        assert!(!e.resolved);
        assert_eq!(open_block_of_slice(d, 2).unwrap().unwrap().id, "BLOCK-2");
        resolve(d, &e, "fixed").unwrap();
        let after = fs::read_to_string(log::path(d)).unwrap();
        assert!(after.starts_with(&before), "the block lines are untouched");
        let e = find(d, "BLOCK-1").unwrap().unwrap();
        assert!(e.resolved);
        let text = entry_text(&e);
        assert!(
            text.starts_with(
                "### BLOCK-1 — gate red · checkpoint · resolved\n\n**Happened:** x\n**Resolution ("
            ),
            "{text}"
        );
        assert!(text.ends_with("):** fixed"), "{text}");
        assert!(find(d, "BLOCK-9").unwrap().is_none());
    }

    #[test]
    fn answers_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        assert!(all(dir.path()).unwrap().is_empty());
        assert!(read(dir.path(), "BLOCK-101").unwrap().is_none());
        let a = Answer {
            id: "BLOCK-101".into(),
            slice: Some(1),
            text: "Pick 1".into(),
            at: "2026-10-10T00:00:00Z".into(),
        };
        write(dir.path(), &a).unwrap();
        write(
            dir.path(),
            &Answer {
                id: "ASK-1".into(),
                slice: None,
                text: "proceed".into(),
                at: "2026-10-10T00:00:01Z".into(),
            },
        )
        .unwrap();
        assert_eq!(read(dir.path(), "BLOCK-101").unwrap(), Some(a.clone()));
        assert_eq!(all(dir.path()).unwrap().len(), 2);
        assert_eq!(block_answer_for_slice(dir.path(), 1).unwrap(), Some(a));
        assert!(block_answer_for_slice(dir.path(), 2).unwrap().is_none());
    }

    #[test]
    fn a_torn_answer_file_is_skipped_not_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let a = Answer {
            id: "BLOCK-101".into(),
            slice: Some(1),
            text: "Pick 1".into(),
            at: "2026-10-10T00:00:00Z".into(),
        };
        write(dir.path(), &a).unwrap();
        // What a reader saw mid-write before writes were atomic: a
        // truncated file, and an empty one.
        fs::write(path(dir.path(), "ASK-1"), "{\"id\": \"ASK-1\", \"te").unwrap();
        fs::write(path(dir.path(), "BLOCK-102"), "").unwrap();
        let (good, bad) = scan(dir.path()).unwrap();
        assert_eq!(good, vec![a.clone()]);
        assert_eq!(
            bad.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(),
            ["ASK-1.json", "BLOCK-102.json"]
        );
        assert_eq!(all(dir.path()).unwrap(), vec![a.clone()]);
        assert_eq!(block_answer_for_slice(dir.path(), 1).unwrap(), Some(a));
        assert_eq!(read(dir.path(), "ASK-1").unwrap(), None);
        // `write` leaves no temporary file behind.
        let names: Vec<String> = fs::read_dir(super::dir(dir.path()))
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert!(!names.iter().any(|n| n.ends_with(".tmp")), "{names:?}");
    }
}
