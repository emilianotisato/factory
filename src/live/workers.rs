//! `workers[]`: every worker the run spawned, from `state.json`, the spawn events and the tail of its stream.
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Component, Path};

use serde_json::{Map, Value, json};

use super::{Ctx, mtime};
use crate::events::EventKind;
use crate::run::WorkerRef;
use crate::stream::StatusHint;
use crate::{lanes, worker};

const BLOCK: u64 = 64 * 1024;
/// The most a reader takes from the end of one stream.
pub const BUDGET: u64 = 1024 * 1024;
/// Longer lines are skipped unparsed.
pub const LINE_MAX: usize = 64 * 1024;
const NARRATION_MAX: usize = 160;

pub fn workers(ctx: &Ctx) -> Value {
    let jobs = jobs(ctx);
    ctx.state
        .workers
        .iter()
        .map(|w| {
            let alive = ctx.state.worker_alive(w);
            let stream = worker::stream_path(&ctx.run_dir, &w.session_id);
            let tail = read_tail(&stream, alive, &Paths::of(ctx, w));
            json!({
                "sessionId": w.session_id,
                "role": w.role,
                "slice": w.slice,
                "checkpoint": w.checkpoint,
                "lane": w.lane,
                "spawnedAt": w.spawned_at,
                "alive": alive,
                "lastWriteAt": mtime(&stream),
                "job": jobs.get(w.session_id.as_str()).copied(),
                "doing": tail.doing.map(|d| json!({
                    "tool": d.tool,
                    "label": d.label,
                    "at": tail.at,
                    "inSubagent": d.in_subagent,
                })),
                "narration": tail.narration,
                "running": tail.running.map(|(tool, secs)| json!({
                    "tool": tool,
                    "elapsedSeconds": secs,
                })),
                "outcome": tail.outcome.map(|(status, block)| json!({
                    "status": status,
                    "block": block,
                })),
            })
        })
        .collect()
}

/// A fix worker's job, told by the spawn event that names its session.
fn jobs(ctx: &Ctx) -> HashMap<&str, &'static str> {
    ctx.events
        .iter()
        .filter_map(|e| match &e.kind {
            EventKind::FixSpawned { session_id, .. } => Some((session_id.as_str(), "gateFix")),
            EventKind::CiFixSpawned { session_id, .. } => Some((session_id.as_str(), "ciFix")),
            _ => None,
        })
        .collect()
}

/// The roots a tool's path label may be relative to, nearest first.
pub struct Paths {
    pub roots: Vec<std::path::PathBuf>,
}

impl Paths {
    fn of(ctx: &Ctx, w: &WorkerRef) -> Paths {
        let mut roots = Vec::new();
        if let (Some(lane), Some(data_dir)) = (w.lane, ctx.run_dir.parent().and_then(Path::parent))
        {
            roots.push(lanes::dir(data_dir, &ctx.state.repo, lane));
        }
        roots.push(ctx.state.repo.clone());
        Paths { roots }
    }

    /// Never absolute: a path outside every root gives `None`.
    fn relative(&self, path: &str) -> Option<String> {
        let p = Path::new(path);
        let rel = if p.is_absolute() {
            self.roots.iter().find_map(|r| p.strip_prefix(r).ok())?
        } else {
            p
        };
        let plain = rel.components().all(|c| matches!(c, Component::Normal(_)));
        let s = rel.to_string_lossy();
        (plain && !s.is_empty()).then(|| s.into_owned())
    }
}

#[derive(Debug, Default)]
pub struct Doing {
    pub tool: String,
    pub label: String,
    pub in_subagent: bool,
}

/// What the end of one stream says. Every field is `None` when the stream does not say it.
#[derive(Debug, Default)]
pub struct Tail {
    pub at: Option<String>,
    pub doing: Option<Doing>,
    pub narration: Option<String>,
    pub running: Option<(String, u64)>,
    pub outcome: Option<(Option<&'static str>, Option<String>)>,
}

/// Newest line first. A finished worker stops at its `result` line; a live one reads until every
/// field is decided or [`BUDGET`] is spent.
pub fn read_tail(path: &Path, alive: bool, paths: &Paths) -> Tail {
    let mut t = Tail::default();
    let mut at_done = false;
    let mut returned: HashSet<String> = HashSet::new();
    let _ = each_line_back(path, BUDGET, |line| {
        if !line.starts_with(b"{") || !wanted(&t, at_done, line) {
            return true;
        }
        let Ok(Value::Object(v)) = serde_json::from_slice::<Value>(line) else {
            return true;
        };
        if !at_done && let Some(ts) = v.get("timestamp").and_then(Value::as_str) {
            t.at = Some(ts.to_string());
            at_done = true;
        }
        match v.get("type").and_then(Value::as_str) {
            Some("result") if t.outcome.is_none() => {
                t.outcome = Some(outcome(v.get("result").and_then(Value::as_str)));
                if !alive {
                    return false;
                }
            }
            Some("assistant") => assistant(&mut t, &v, paths),
            Some("user") => {
                for id in blocks(&v, "tool_result")
                    .filter_map(|b| b.get("tool_use_id").and_then(Value::as_str))
                {
                    returned.insert(id.to_string());
                }
            }
            Some("tool_progress") if t.running.is_none() => {
                let tool = v.get("parent_tool_use_id").and_then(Value::as_str);
                if let Some(id) = tool
                    && !returned.contains(id)
                {
                    t.running = Some((
                        v.get("tool_name")
                            .and_then(Value::as_str)
                            .unwrap_or("?")
                            .to_string(),
                        v.get("elapsed_time_seconds")
                            .and_then(Value::as_f64)
                            .map_or(0, |s| s.max(0.0) as u64),
                    ));
                }
            }
            _ => {}
        }
        !(alive && at_done && t.doing.is_some() && t.narration.is_some() && t.running.is_some())
    });
    if !alive {
        t.running = None;
    }
    t
}

/// Only lines that can still decide something are parsed.
fn wanted(t: &Tail, at_done: bool, line: &[u8]) -> bool {
    let has = |n: &[u8]| line.windows(n.len()).any(|w| w == n);
    (!at_done && has(b"\"timestamp\""))
        || (t.doing.is_none() && has(b"\"tool_use\""))
        || (t.narration.is_none() && has(b"\"text\""))
        || (t.running.is_none() && (has(b"\"tool_progress\"") || has(b"\"tool_result\"")))
        || (t.outcome.is_none() && has(b"\"result\""))
}

fn assistant(t: &mut Tail, v: &Map<String, Value>, paths: &Paths) {
    if t.doing.is_none()
        && let Some(b) = blocks(v, "tool_use").last()
    {
        let tool = b
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("?")
            .to_string();
        t.doing = Some(Doing {
            label: label(&tool, b.get("input"), paths),
            tool,
            in_subagent: v.get("parent_tool_use_id").is_some_and(Value::is_string),
        });
    }
    if t.narration.is_none()
        && let Some(text) = blocks(v, "text")
            .filter_map(|b| b.get("text").and_then(Value::as_str))
            .map(one_line)
            .filter(|s| !s.is_empty())
            .last()
    {
        t.narration = Some(cut(&text, NARRATION_MAX));
    }
}

/// Never the command and never tool output: a description the model wrote, a skill name, or a path.
fn label(tool: &str, input: Option<&Value>, paths: &Paths) -> String {
    let field = |k: &str| {
        input
            .and_then(|i| i.get(k))
            .and_then(Value::as_str)
            .map(one_line)
            .filter(|s| !s.is_empty())
    };
    let named = match tool {
        "Bash" => field("description"),
        "Skill" => field("skill").map(|s| format!("Skill {s}")),
        "Agent" | "Task" => field("description").map(|s| format!("Agent {s}")),
        _ => ["file_path", "notebook_path", "path"]
            .into_iter()
            .find_map(field)
            .and_then(|p| paths.relative(&p))
            .map(|p| format!("{tool} {p}")),
    };
    cut(&named.unwrap_or_else(|| tool.to_string()), NARRATION_MAX)
}

fn outcome(text: Option<&str>) -> (Option<&'static str>, Option<String>) {
    let text = text.unwrap_or("");
    let status = match StatusHint::from_result(text) {
        Some(StatusHint::Done) => Some("done"),
        Some(StatusHint::Blocked) => Some("blocked"),
        _ => None,
    };
    let block = text
        .lines()
        .map(str::trim)
        .find_map(|l| l.strip_prefix("BLOCK:"))
        .map(str::trim)
        .filter(|b| !b.is_empty() && !b.eq_ignore_ascii_case("none"))
        .map(|b| cut(b, NARRATION_MAX));
    (status, block)
}

fn blocks<'a>(v: &'a Map<String, Value>, kind: &'a str) -> impl Iterator<Item = &'a Value> {
    v.get("message")
        .and_then(|m| m.get("content"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(move |b| b.get("type").and_then(Value::as_str) == Some(kind))
}

fn one_line(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn cut(s: &str, max: usize) -> String {
    match s.char_indices().nth(max) {
        Some(_) => {
            let mut out: String = s.chars().take(max - 1).collect();
            out.push('…');
            out
        }
        None => s.to_string(),
    }
}

/// Calls `f` on each whole line from the newest, until it returns false or `budget` bytes are read.
/// The last line without its newline, lines longer than [`LINE_MAX`] and the line cut by the budget
/// never reach `f`.
pub fn each_line_back(
    path: &Path,
    budget: u64,
    mut f: impl FnMut(&[u8]) -> bool,
) -> std::io::Result<()> {
    let mut file = File::open(path)?;
    let len = file.metadata()?.len();
    let floor = len.saturating_sub(budget);
    let mut pos = len;
    let mut carry: Vec<u8> = Vec::new();
    let mut oversized = false;
    let mut partial = true;
    let mut buf = Vec::new();
    while pos > floor {
        let start = pos.saturating_sub(BLOCK).max(floor);
        buf.resize((pos - start) as usize, 0);
        file.seek(SeekFrom::Start(start))?;
        file.read_exact(&mut buf)?;
        pos = start;
        let mut end = buf.len();
        while let Some(i) = buf[..end].iter().rposition(|&b| b == b'\n') {
            let seg = &buf[i + 1..end];
            if partial {
                partial = false;
            } else if !oversized && seg.len() + carry.len() <= LINE_MAX {
                let line = [seg, &carry[..]].concat();
                if !f(&line) {
                    return Ok(());
                }
            }
            carry.clear();
            oversized = false;
            end = i;
        }
        if !oversized {
            if end + carry.len() > LINE_MAX {
                oversized = true;
                carry.clear();
            } else {
                carry.splice(0..0, buf[..end].iter().copied());
            }
        }
    }
    if pos == 0 && !partial && !oversized && !carry.is_empty() {
        f(&carry);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines_back(text: &[u8], budget: u64) -> Vec<String> {
        let dir = std::env::temp_dir().join(format!("live-tail-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = dir.join(format!("{n}.jsonl"));
        std::fs::write(&path, text).unwrap();
        let mut out = Vec::new();
        each_line_back(&path, budget, |l| {
            out.push(String::from_utf8_lossy(l).into_owned());
            true
        })
        .unwrap();
        out
    }

    #[test]
    fn lines_come_newest_first_without_the_partial_one() {
        assert_eq!(lines_back(b"a\nb\nc\n", BUDGET), ["c", "b", "a"]);
        assert_eq!(lines_back(b"a\nb\n{\"c", BUDGET), ["b", "a"]);
        assert!(lines_back(b"no newline", BUDGET).is_empty());
    }

    #[test]
    fn a_line_across_blocks_is_whole_and_an_oversized_one_is_skipped() {
        let long = "x".repeat(BLOCK as usize + 10);
        let mut text = format!("first\n{long}\nlast\n").into_bytes();
        let got = lines_back(&text, BUDGET);
        assert_eq!(got, ["last", "first"]);

        let mid = "y".repeat(40 * 1024);
        text = format!("first\n{mid}\nlast\n").into_bytes();
        let got = lines_back(&text, BUDGET);
        assert_eq!(got.len(), 3);
        assert_eq!(got[1], mid);
    }

    #[test]
    fn the_budget_drops_the_line_it_cuts() {
        assert_eq!(lines_back(b"aaaa\nbb\ncc\n", 7), ["cc", "bb"]);
        assert_eq!(lines_back(b"aaaa\nbb\ncc\n", 5), ["cc"]);
    }

    #[test]
    fn a_label_is_never_an_absolute_path() {
        let paths = Paths {
            roots: vec!["/lanes/lane-2".into(), "/repo".into()],
        };
        let l = |tool: &str, input: Value| label(tool, Some(&input), &paths);
        assert_eq!(
            l("Edit", json!({"file_path": "/lanes/lane-2/src/a.rs"})),
            "Edit src/a.rs"
        );
        assert_eq!(
            l("Read", json!({"file_path": "/repo/docs/x.md"})),
            "Read docs/x.md"
        );
        assert_eq!(l("Read", json!({"file_path": "/etc/passwd"})), "Read");
        assert_eq!(l("Read", json!({"file_path": "../up"})), "Read");
        assert_eq!(l("Bash", json!({"command": "echo hi"})), "Bash");
        assert_eq!(l("Agent", json!({"description": "look"})), "Agent look");
    }

    #[test]
    fn the_block_line_is_found_anywhere_and_none_is_null() {
        assert_eq!(
            outcome(Some(
                "x\ny\nSTATUS: blocked\nBLOCK: the index forbids soft-delete"
            )),
            (
                Some("blocked"),
                Some("the index forbids soft-delete".into())
            )
        );
        assert_eq!(
            outcome(Some("STATUS: done\nBLOCK: none")),
            (Some("done"), None)
        );
        assert_eq!(outcome(None), (None, None));
    }
}
