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
/// flattened beside it. Later slices add theirs (`rate_limit_*`, `pr_*`, …).
/// No `Eq`: `gate_run` carries the gate's minutes as a float.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    /// `done` nor `blocked` (FR-1: "a dead worker is treated as today"). A
    /// fix or checkpoint worker has no slice (FR-4).
    WorkerDead {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        slice: Option<u32>,
        session_id: String,
        exit_code: Option<i32>,
    },
    /// FR-9: a worker hit a usage limit; the driver spawns nothing until
    /// `resets_at` (epoch seconds; now + 15 min when the stream gave none —
    /// AD-701). `literal` is the stream's `status`, or what matched in a
    /// `result` when there was no `rate_limit_event`.
    RunPaused {
        literal: String,
        resets_at: u64,
    },
    /// FR-9: `paused_until` passed; spawning resumes.
    RunResumedFromPause,
    /// FR-9: the weighted token sum passed the budget; in-flight workers
    /// finish, nothing new spawns, `factory run NNN --budget <n>` resumes.
    RunParked {
        reason: String,
        weighted: u64,
        budget: u64,
    },
    /// The slice cites a `§n` that `prd.md` lacks (FR-2: a warning, not a crash).
    PackSectionMissing {
        slice: u32,
        section: u32,
    },
    /// FR-5: the slice's commit — the stream's written paths plus the slice
    /// file and its fragment, by pathspec; `files` is what was staged.
    SliceCommitted {
        slice: u32,
        sha: String,
        files: Vec<String>,
    },
    /// Files that appeared in the tree while the worker ran and that no tool
    /// call named: listed in the fragment under `## Untracked leftovers`,
    /// never committed (FR-5).
    UntrackedLeftovers {
        slice: u32,
        files: Vec<String>,
    },
    /// What a checkpoint's fix or checkpoint worker changed that its stream
    /// never named and its commit does not take: untracked files and the
    /// protected ones, left in the tree for the owner (AD-20007).
    CheckpointLeftovers {
        n: u32,
        role: String,
        files: Vec<String>,
    },
    /// The stream names one of the three files no worker may write
    /// (`prd.md`, `agent-log.md`, `docs/index.md`); left out of the commit.
    WorkerWroteProtectedFile {
        slice: u32,
        path: String,
    },
    /// The gate is green at a checkpoint: `docs log merge` folded the
    /// fragments into `agent-log.md` (FR-5); the checkpoint commit lands them.
    LogMerged {
        fragments: Vec<String>,
    },
    /// `docs log merge` refused (an AD clash): `block_id` is the `BLOCK-n`
    /// written to `agent-log.md` for the owner; the run parks on it.
    MergeRefused {
        block_id: String,
        detail: String,
    },
    /// FR-4: the board says a checkpoint is due and the lanes are drained.
    /// `n` is the `CP-n` this checkpoint writes, `covers` the unverified
    /// slices (`unverified[]` on the board).
    CheckpointStarted {
        n: u32,
        covers: Vec<u32>,
    },
    /// FR-1 resume: the run died inside checkpoint `n` (or parked on it);
    /// the checkpoint runs again before any spawn.
    CheckpointResumed {
        n: u32,
        covers: Vec<u32>,
    },
    /// The driver ran the gate (FR-4): `log` is `runs/<id>/gates/<n>-<attempt>.log`.
    GateRun {
        n: u32,
        attempt: u32,
        red: bool,
        minutes: f64,
        log: String,
    },
    /// The gate cache answered for this code tree: no process ran, the
    /// `gates` row says `cached`; `log` is the earlier run's transcript.
    GateCached {
        n: u32,
        attempt: u32,
        red: bool,
        log: String,
    },
    /// A fix worker on a red gate (FR-4), with the failing output in its pack.
    FixSpawned {
        n: u32,
        attempt: u32,
        session_id: String,
        pid: u32,
    },
    /// The fix worker's files, committed by pathspec: `fix: gate red at checkpoint n`.
    FixCommitted {
        n: u32,
        attempt: u32,
        sha: String,
        files: Vec<String>,
    },
    /// A checkpoint worker on a green gate: it drives the Prove rows and
    /// writes `CP-n` (FR-4).
    CheckpointSpawned {
        n: u32,
        attempt: u32,
        session_id: String,
        pid: u32,
    },
    /// The checkpoint stopped on the owner: three reds in a row, a refused
    /// merge, two dead checkpoint workers or the worker's own block.
    /// `block_id` is the `BLOCK-n` in `agent-log.md`.
    CheckpointBlocked {
        n: u32,
        block_id: String,
        question: String,
    },
    /// `CP-n` is on the board and the checkpoint commit (`docs: checkpoint n`)
    /// landed; `None` when the render changed nothing to commit.
    CheckpointDone {
        n: u32,
        sha: Option<String>,
    },
    /// Closeout (FR-6): the polish worker, once the last checkpoint is done.
    PolishSpawned {
        attempt: u32,
        session_id: String,
        pid: u32,
    },
    /// The polish worker returned; `sha` is its `polish: review fixes`
    /// commit, `None` when it changed nothing.
    PolishDone {
        sha: Option<String>,
        files: Vec<String>,
    },
    /// `docs ship` ran, the retro keys are filled and `docs: ship PRD-NNN`
    /// is committed (`sha`, `None` when nothing was left to commit).
    Shipped {
        sha: Option<String>,
    },
    /// The branch is pushed and the draft PR exists.
    PrOpened {
        url: String,
    },
    /// `gh pr checks` came back green at `head`.
    CiGreen {
        head: String,
    },
    /// A red check at `head`; `round` is the fix rounds done before it.
    CiRed {
        check: String,
        round: u32,
        head: String,
    },
    /// The one flaky rerun (`gh run rerun <id> --failed`) at `head`.
    CiRerun {
        check: String,
        run_id: u64,
        head: String,
    },
    /// A CI fix worker on `check`, `round` 1-based; `attempt` counts the
    /// workers of that round (a death or a usage limit respawns).
    CiFixSpawned {
        round: u32,
        attempt: u32,
        session_id: String,
        pid: u32,
    },
    CiFixCommitted {
        round: u32,
        sha: String,
        files: Vec<String>,
    },
    /// A `BLOCK-n` written by the closeout, `phase` one of `polish`, `pr`,
    /// `ci`; the scheduler parks the run (`run_waiting` follows).
    CloseoutBlocked {
        phase: String,
        block_id: String,
        question: String,
    },
    /// The end of the run (FR-7). Every field has a default so a line from
    /// before slice 09, a bare `run_finished`, still parses.
    RunFinished {
        #[serde(default)]
        pr_url: Option<String>,
        #[serde(default)]
        slices: u64,
        #[serde(default)]
        checkpoints: u64,
        #[serde(default)]
        ci_cycles: u64,
        #[serde(default)]
        weighted_tokens: u64,
        #[serde(default)]
        duration_minutes: f64,
    },
    /// FR-7: the run ended on `factory stop` once its in-flight workers had
    /// exited — phase `finished`, `stopped: true` in `state.json`, and
    /// `factory run NNN` resumes from the board. The last line of a stopped
    /// run, in place of `run_finished` (AD-1002).
    RunStopped {
        #[serde(default)]
        slices: u64,
        #[serde(default)]
        weighted_tokens: u64,
        #[serde(default)]
        duration_minutes: f64,
    },
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Event {
    pub seq: u64,
    pub ts: String,
    #[serde(flatten)]
    pub kind: EventKind,
}

/// Append `kind` to `<run_dir>/events.jsonl` with the next `seq`, and insert
/// the same event into the `events` table. The driver, `factory stop` and
/// `factory answer` all emit, so the file is held under `flock` from reading
/// the last `seq` to the table row (AD-20006), and the line goes out in one
/// `write`: two emitters never share a `seq` or interleave their bytes.
pub fn emit(run_dir: &Path, db: &Db, run_id: &str, kind: EventKind) -> Result<Event> {
    let path = run_dir.join(FILE);
    let mut f = OpenOptions::new()
        .append(true)
        .create(true)
        .open(&path)
        .map_err(io(format!("open {}", path.display())))?;
    f.lock().map_err(io(format!("lock {}", path.display())))?;
    let seq = last_seq(&path)? + 1;
    let event = Event {
        seq,
        ts: clock::now_rfc3339(),
        kind,
    };
    let mut value = serde_json::to_value(&event).map_err(json("serialize event"))?;
    let line = serde_json::to_string(&value).map_err(json("serialize event"))? + "\n";
    f.write_all(line.as_bytes())
        .map_err(io(format!("append {}", path.display())))?;

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
    drop(f);
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
