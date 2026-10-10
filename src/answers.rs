//! The owner's half of a parked run (PRD §5 FR-3, §3 FR-1): `BLOCK-n`
//! entries as the feature's log files hold them, their resolution in the
//! shape `autonomy.md` § Resuming prescribes, and `runs/<id>/answers/<id>.json`,
//! the file the driver's slow tick watches for both `BLOCK-n` and `ASK-n`.
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

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

pub fn write(run_dir: &Path, answer: &Answer) -> Result<()> {
    let d = dir(run_dir);
    fs::create_dir_all(&d).map_err(io(format!("mkdir {}", d.display())))?;
    let p = path(run_dir, &answer.id);
    let text = serde_json::to_string_pretty(answer).map_err(json("serialize answer"))?;
    fs::write(&p, text + "\n").map_err(io(format!("write {}", p.display())))
}

pub fn read(run_dir: &Path, id: &str) -> Result<Option<Answer>> {
    let p = path(run_dir, id);
    match fs::read_to_string(&p) {
        Ok(text) => Ok(Some(
            serde_json::from_str(&text).map_err(json(format!("parse {}", p.display())))?,
        )),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(io(format!("read {}", p.display()))(e)),
    }
}

/// Every answer on disk, by id.
pub fn all(run_dir: &Path) -> Result<Vec<Answer>> {
    let d = dir(run_dir);
    let entries = match fs::read_dir(&d) {
        Ok(rd) => rd,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(io(format!("read {}", d.display()))(e)),
    };
    let mut out: Vec<Answer> = Vec::new();
    for entry in entries {
        let p = entry.map_err(io(format!("read {}", d.display())))?.path();
        if p.extension().is_some_and(|e| e == "json") {
            let text = fs::read_to_string(&p).map_err(io(format!("read {}", p.display())))?;
            out.push(serde_json::from_str(&text).map_err(json(format!("parse {}", p.display())))?);
        }
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(out)
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

// ── BLOCK entries in the log files ───────────────────────────────────────────

/// A `### BLOCK-n — {title} · slice-NN · open|resolved` entry as a log file
/// holds it: the heading at line `start`, the body through line `end`
/// (exclusive; the next `### `/`## ` heading or the end of the file, trailing
/// blank lines left out).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockEntry {
    pub id: String,
    pub n: u32,
    pub title: String,
    pub slice: Option<u32>,
    pub phase: Option<String>,
    pub resolved: bool,
    pub path: PathBuf,
    pub start: usize,
    pub end: usize,
}

/// `### BLOCK-n — rest` → `(n, rest)`; the same shape `parseAgentLog` reads.
fn block_heading(line: &str) -> Option<(u32, &str)> {
    let rest = line.strip_prefix("###")?;
    let rest = rest.trim_start();
    let rest = rest.strip_prefix("BLOCK-")?;
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() {
        return None;
    }
    let rest = rest[digits.len()..].trim_start();
    let rest = rest
        .strip_prefix('—')
        .or_else(|| rest.strip_prefix('–'))
        .or_else(|| rest.strip_prefix('-'))?;
    Some((digits.parse().ok()?, rest.trim()))
}

/// Every BLOCK entry in one log file, in order. Fenced blocks are skipped as
/// the tool skips them (the template's examples live in one).
pub fn entries(path: &Path) -> Result<Vec<BlockEntry>> {
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(io(format!("read {}", path.display()))(e)),
    };
    let lines: Vec<&str> = text.lines().collect();
    let mut out: Vec<BlockEntry> = Vec::new();
    let mut fenced = false;
    for (i, line) in lines.iter().enumerate() {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
        }
        if fenced {
            continue;
        }
        let is_heading = line.starts_with("## ") || line.starts_with("### ");
        if is_heading
            && let Some(last) = out.last_mut()
            && last.end == 0
        {
            last.end = i;
        }
        let Some((n, rest)) = block_heading(line) else {
            continue;
        };
        let parts: Vec<&str> = rest.split(" · ").map(str::trim).collect();
        let last = parts.last().copied().unwrap_or("");
        let where_ = parts[1..]
            .iter()
            .find(|p| {
                p.starts_with("slice-") || p.chars().all(|c| c.is_ascii_lowercase() || c == '-')
            })
            .copied();
        let slice = where_
            .and_then(|w| w.strip_prefix("slice-"))
            .and_then(|s| s.parse().ok());
        let phase = where_
            .filter(|w| !w.starts_with("slice-") && *w != "open" && *w != "resolved")
            .map(str::to_string);
        out.push(BlockEntry {
            id: format!("BLOCK-{n}"),
            n,
            title: parts.first().copied().unwrap_or("").to_string(),
            slice,
            phase,
            resolved: last == "resolved",
            path: path.to_path_buf(),
            start: i,
            end: 0,
        });
    }
    for e in &mut out {
        if e.end == 0 {
            e.end = lines.len();
        }
        while e.end > e.start + 1 && lines[e.end - 1].trim().is_empty() {
            e.end -= 1;
        }
    }
    Ok(out)
}

/// The log files of a feature folder that may hold a block: the fragments
/// first (`log-slice-NN.md`, `log-fix-N.md`), then `agent-log.md`.
pub fn log_files(feature_dir: &Path) -> Result<Vec<PathBuf>> {
    let mut frags: Vec<PathBuf> = Vec::new();
    let rd = match fs::read_dir(feature_dir) {
        Ok(rd) => rd,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(io(format!("read {}", feature_dir.display()))(e)),
    };
    for entry in rd {
        let p = entry
            .map_err(io(format!("read {}", feature_dir.display())))?
            .path();
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name.starts_with("log-") && name.ends_with(".md") {
            frags.push(p);
        }
    }
    frags.sort();
    let log = feature_dir.join("agent-log.md");
    if log.is_file() {
        frags.push(log);
    }
    Ok(frags)
}

/// The entry `id` (`BLOCK-n`) anywhere in the feature's log files.
pub fn find(feature_dir: &Path, id: &str) -> Result<Option<BlockEntry>> {
    Ok(find_all(feature_dir, id)?.into_iter().next())
}

/// Every entry `id` (`BLOCK-n`) in the feature's log files, open or not, in
/// [`log_files`] order: a builder's `BLOCK-101` and a resolved one of an
/// earlier run can share a number, so `factory answer` must see them all.
pub fn find_all(feature_dir: &Path, id: &str) -> Result<Vec<BlockEntry>> {
    let mut out = Vec::new();
    for file in log_files(feature_dir)? {
        out.extend(entries(&file)?.into_iter().filter(|e| e.id == id));
    }
    Ok(out)
}

/// The open block of `slice`: the last `open` entry naming `slice-NN`, the
/// slice's own fragment searched first. What `slice_blocked` reports.
pub fn open_block_of_slice(feature_dir: &Path, slice: u32) -> Result<Option<BlockEntry>> {
    let own = feature_dir.join(format!("log-slice-{slice:02}.md"));
    let mut files = vec![own];
    files.extend(log_files(feature_dir)?);
    let mut hit = None;
    for file in files {
        if let Some(e) = entries(&file)?
            .into_iter()
            .rfind(|e| !e.resolved && e.slice == Some(slice))
        {
            hit = Some(e);
            break;
        }
    }
    Ok(hit)
}

/// The entry's lines, heading through body, as the file holds them.
pub fn entry_text(entry: &BlockEntry) -> Result<String> {
    let text =
        fs::read_to_string(&entry.path).map_err(io(format!("read {}", entry.path.display())))?;
    let lines: Vec<&str> = text.lines().collect();
    let end = entry.end.min(lines.len());
    Ok(lines[entry.start..end].join("\n"))
}

/// `autonomy.md` § Resuming step 1: append `**Resolution ({date}):** {text}`
/// under the entry and turn the heading's last field `open` into `resolved`.
pub fn resolve(entry: &BlockEntry, text: &str, date: &str) -> Result<()> {
    let content =
        fs::read_to_string(&entry.path).map_err(io(format!("read {}", entry.path.display())))?;
    let mut lines: Vec<String> = content.lines().map(str::to_string).collect();
    let heading = &lines[entry.start];
    let flipped = match heading.rsplit_once(" · ") {
        Some((head, last)) if last.trim() == "open" => format!("{head} · resolved"),
        _ => {
            return Err(Error::Msg(format!(
                "{} line {}: heading does not end in `· open`: {heading}",
                entry.path.display(),
                entry.start + 1
            )));
        }
    };
    lines[entry.start] = flipped;
    let line = format!("**Resolution ({date}):** {text}");
    let end = entry.end.min(lines.len());
    lines.insert(end, line);
    fs::write(&entry.path, lines.join("\n") + "\n")
        .map_err(io(format!("write {}", entry.path.display())))
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRAG: &str = "## Blockers\n\n### BLOCK-101 — Which option? · slice-01 · open\n\n**Happened:** x\n**Options:**\n1. a\n2. b\n**Recommended:** 1, because\n\n## Decisions\n\n### AD-101 — q · slice-01 · 2026-10-10\n\n**Chose:** y\n";

    #[test]
    fn entries_find_and_resolve_a_block() {
        let dir = tempfile::tempdir().unwrap();
        let frag = dir.path().join("log-slice-01.md");
        fs::write(&frag, FRAG).unwrap();
        let es = entries(&frag).unwrap();
        assert_eq!(es.len(), 1);
        let e = &es[0];
        assert_eq!(
            (e.id.as_str(), e.n, e.slice, e.resolved),
            ("BLOCK-101", 101, Some(1), false)
        );
        assert_eq!(e.title, "Which option?");
        assert_eq!(e.phase, None);
        assert_eq!((e.start, e.end), (2, 9));
        assert_eq!(
            entry_text(e).unwrap(),
            "### BLOCK-101 — Which option? · slice-01 · open\n\n**Happened:** x\n**Options:**\n1. a\n2. b\n**Recommended:** 1, because"
        );
        assert_eq!(find(dir.path(), "BLOCK-101").unwrap().unwrap().path, frag);
        assert!(find(dir.path(), "BLOCK-102").unwrap().is_none());
        assert_eq!(
            open_block_of_slice(dir.path(), 1).unwrap().unwrap().id,
            "BLOCK-101"
        );
        assert!(open_block_of_slice(dir.path(), 2).unwrap().is_none());

        resolve(e, "Pick 1", "2026-10-10").unwrap();
        let text = fs::read_to_string(&frag).unwrap();
        assert_eq!(
            text,
            "## Blockers\n\n### BLOCK-101 — Which option? · slice-01 · resolved\n\n**Happened:** x\n**Options:**\n1. a\n2. b\n**Recommended:** 1, because\n**Resolution (2026-10-10):** Pick 1\n\n## Decisions\n\n### AD-101 — q · slice-01 · 2026-10-10\n\n**Chose:** y\n"
        );
        let e2 = find(dir.path(), "BLOCK-101").unwrap().unwrap();
        assert!(e2.resolved);
        assert!(open_block_of_slice(dir.path(), 1).unwrap().is_none());
        assert!(resolve(&e2, "again", "2026-10-10").is_err());
    }

    #[test]
    fn phase_blocks_fences_and_agent_log_order() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("agent-log.md");
        fs::write(
            &log,
            "# Agent log\n\n## Blockers\n\n### BLOCK-3 — gate missing · preflight · open\n\n**Happened:** z\n\n```markdown\n### BLOCK-1 — example · slice-02 · open\n```\n",
        )
        .unwrap();
        let es = entries(&log).unwrap();
        assert_eq!(es.len(), 1);
        assert_eq!(
            (es[0].slice, es[0].phase.as_deref()),
            (None, Some("preflight"))
        );
        // A fenced example inside the entry is the entry's: the resolution
        // lands after it, not inside it.
        assert_eq!(es[0].end, 11);
        assert!(entry_text(&es[0]).unwrap().ends_with("```"));
        assert_eq!(log_files(dir.path()).unwrap(), vec![log.clone()]);
        fs::write(dir.path().join("log-slice-02.md"), "## Traps\n\n- t\n").unwrap();
        let files = log_files(dir.path()).unwrap();
        assert_eq!(files.len(), 2);
        assert_eq!(files[1], log);
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
}
