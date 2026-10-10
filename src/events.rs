//! `events.jsonl` (PRD §3 FR-1, §9 FR-7): append-only, one object per line,
//! `{seq, ts, kind, ...payload}`. Every event is also a row in the `events`
//! table, written by the same [`emit`]; nothing writes one without the other.
//! The cockpit replays the file; the metrics read the table.
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::db::Db;
use crate::error::{io, json};
use crate::{Result, clock};

pub const FILE: &str = "events.jsonl";

/// The event kinds, serialized `snake_case` under `kind` with the payload
/// flattened beside it. Later slices add theirs (`checkpoint_*`, `gate_*`, …).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EventKind {
    RunStarted,
    RunResumed,
    /// A preflight `ask[]` item the run waits on (`ASK-n`); slice 04 answers it.
    AskOpened {
        id: String,
        text: String,
    },
    /// A preflight `judge[]` line: logged for the cockpit, never waited on
    /// (FR-1 as corrected by BLOCK-201 — the driver is the judge).
    JudgeNoted {
        text: String,
    },
    StopRequested,
    /// A worker started on a slice (FR-2); `pid` is the `claude -p` process.
    SliceSpawned {
        slice: u32,
        session_id: String,
        pid: u32,
    },
    /// The board read `done` after the worker exited.
    SliceDone {
        slice: u32,
    },
    /// The board read `blocked` after the worker exited, or the driver blocked
    /// the slice itself after a second dead worker (FR-1). `block_id` and
    /// `question` come from the open `BLOCK-n` entry in the slice's fragment
    /// (FR-3); absent when the fragment has none.
    SliceBlocked {
        slice: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        block_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        question: Option<String>,
    },
    /// Nothing is spawnable, nothing is running and the owner has a question:
    /// the run parks in `waiting-owner` and keeps watching for an answer (FR-3).
    RunWaiting {
        blocked: Vec<u32>,
        asks: Vec<String>,
    },
    /// `factory answer BLOCK-n`: the resolution is in the log, the slice is
    /// `building` again, the next worker on it gets both in its pack.
    BlockAnswered {
        id: String,
        slice: Option<u32>,
        text: String,
    },
    /// `factory answer ASK-n`: a preflight ask cleared; the run starts when
    /// the last one is.
    AskAnswered {
        id: String,
        text: String,
    },
    /// Process gone with no `result` line, or with one and the slice neither
    /// `done` nor `blocked` (FR-1: "a dead worker is treated as today").
    WorkerDead {
        slice: u32,
        session_id: String,
        exit_code: Option<i32>,
    },
    /// The slice cites a `§n` that `prd.md` lacks (FR-2: a warning, not a crash).
    PackSectionMissing {
        slice: u32,
        section: u32,
    },
    /// The board says a checkpoint is due; slice 06 runs it, this slice stops.
    CheckpointDue {
        unverified: Vec<u32>,
    },
    RunFinished,
    RunFailed {
        error: String,
    },
}

impl EventKind {
    /// The `kind` string as it appears on the line and in the table.
    pub fn name(&self) -> String {
        match serde_json::to_value(self) {
            Ok(serde_json::Value::Object(o)) => o
                .get("kind")
                .and_then(|k| k.as_str())
                .unwrap_or("unknown")
                .to_string(),
            _ => "unknown".to_string(),
        }
    }
}

/// One line of `events.jsonl`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Event {
    pub seq: u64,
    pub ts: String,
    #[serde(flatten)]
    pub kind: EventKind,
}

/// Append `kind` to `<run_dir>/events.jsonl` with the next `seq`, and insert
/// the same event into the `events` table.
pub fn emit(run_dir: &Path, db: &Db, run_id: &str, kind: EventKind) -> Result<Event> {
    let path = run_dir.join(FILE);
    let seq = last_seq(&path)? + 1;
    let event = Event {
        seq,
        ts: clock::now_rfc3339(),
        kind,
    };
    let mut value = serde_json::to_value(&event).map_err(json("serialize event"))?;
    let line = serde_json::to_string(&value).map_err(json("serialize event"))?;
    let mut f = OpenOptions::new()
        .append(true)
        .create(true)
        .open(&path)
        .map_err(io(format!("open {}", path.display())))?;
    writeln!(f, "{line}").map_err(io(format!("append {}", path.display())))?;

    let name = event.kind.name();
    let payload = match value.as_object_mut() {
        Some(o) => {
            o.remove("seq");
            o.remove("ts");
            o.remove("kind");
            serde_json::to_string(o).map_err(json("serialize payload"))?
        }
        None => "{}".to_string(),
    };
    db.insert_event(run_id, seq, &event.ts, &name, &payload)?;
    Ok(event)
}

/// Every line of `<run_dir>/events.jsonl`, in order; empty when none.
pub fn read_all(run_dir: &Path) -> Result<Vec<Event>> {
    let path = run_dir.join(FILE);
    let text = match fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(io(format!("read {}", path.display()))(e)),
    };
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).map_err(json(format!("parse {}: {l}", path.display()))))
        .collect()
}

fn last_seq(path: &Path) -> Result<u64> {
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(e) => return Err(io(format!("read {}", path.display()))(e)),
    };
    let Some(last) = text.lines().rev().find(|l| !l.trim().is_empty()) else {
        return Ok(0);
    };
    let v: serde_json::Value =
        serde_json::from_str(last).map_err(json(format!("parse {}: {last}", path.display())))?;
    Ok(v.get("seq").and_then(|s| s.as_u64()).unwrap_or(0))
}
