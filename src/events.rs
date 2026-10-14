//! Every event lands in both `events.jsonl` and the `events` table: the cockpit replays the file, metrics read the table.
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::db::Db;
use crate::error::{io, json};
use crate::{Result, clock};

pub const FILE: &str = "events.jsonl";

/// No `Eq`: `gate_run` carries a float.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EventKind {
    RunStarted,
    RunResumed,
    AskOpened {
        id: String,
        text: String,
    },
    /// A preflight `judge[]` line, never waited on: the driver is the judge.
    JudgeNoted {
        text: String,
    },
    StopRequested,
    SliceSpawned {
        slice: u32,
        session_id: String,
        pid: u32,
    },
    /// `attempt` counts the slice's merge workers.
    MergeSpawned {
        slice: u32,
        attempt: u32,
        session_id: String,
        pid: u32,
    },
    LandingConflicted {
        slice: u32,
        paths: Vec<String>,
    },
    SliceDone {
        slice: u32,
    },
    /// Absent ids: the log held no open block for the slice.
    SliceBlocked {
        slice: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        block_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        question: Option<String>,
    },
    /// Nothing spawnable, nothing running, and the owner owes an answer.
    RunWaiting {
        blocked: Vec<u32>,
        asks: Vec<String>,
    },
    BlockAnswered {
        id: String,
        slice: Option<u32>,
        text: String,
    },
    AskAnswered {
        id: String,
        text: String,
    },
    /// Exited with no `result` line, or with one and the slice neither `done` nor `blocked`.
    WorkerDead {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        slice: Option<u32>,
        session_id: String,
        exit_code: Option<i32>,
    },
    /// `resets_at` is epoch seconds (now + 15 min when the stream gave none); `literal` is what the stream said.
    RunPaused {
        literal: String,
        resets_at: u64,
    },
    RunResumedFromPause,
    RunParked {
        reason: String,
        weighted: u64,
        budget: u64,
    },
    PackSectionMissing {
        slice: u32,
        section: u32,
    },
    /// The transcript or its `subagents/` dir could not be read; the `sub_*` columns stay null.
    SubagentsUnread {
        worker_id: i64,
        session_id: String,
        path: String,
        error: String,
    },
    /// `files` is what was staged, by pathspec.
    SliceCommitted {
        slice: u32,
        sha: String,
        files: Vec<String>,
    },
    /// Left out of the commit; see [`crate::commit::protected_files`].
    WorkerWroteProtectedFile {
        slice: u32,
        path: String,
    },
    /// Nothing emits this; kept so old event logs read.
    LogMerged {
        fragments: Vec<String>,
    },
    /// Nothing emits this; kept so old event logs read.
    MergeRefused {
        block_id: String,
        detail: String,
    },
    /// `covers` is the board's `unverified[]`.
    CheckpointStarted {
        n: u32,
        covers: Vec<u32>,
    },
    CheckpointResumed {
        n: u32,
        covers: Vec<u32>,
    },
    /// `log` is `runs/<id>/gates/<n>-<attempt>.log`.
    GateRun {
        n: u32,
        attempt: u32,
        red: bool,
        minutes: f64,
        log: String,
    },
    /// No process ran; `log` is the earlier run's transcript.
    GateCached {
        n: u32,
        attempt: u32,
        red: bool,
        log: String,
    },
    FixSpawned {
        n: u32,
        attempt: u32,
        session_id: String,
        pid: u32,
    },
    FixCommitted {
        n: u32,
        attempt: u32,
        sha: String,
        files: Vec<String>,
    },
    CheckpointSpawned {
        n: u32,
        attempt: u32,
        session_id: String,
        pid: u32,
    },
    CheckpointBlocked {
        n: u32,
        block_id: String,
        question: String,
    },
    /// `sha` is `None` when the render changed nothing to commit.
    CheckpointDone {
        n: u32,
        sha: Option<String>,
    },
    PolishSpawned {
        attempt: u32,
        session_id: String,
        pid: u32,
    },
    /// `sha` is `None` when the worker changed nothing.
    PolishDone {
        sha: Option<String>,
        files: Vec<String>,
    },
    PolishFindingsMissing {
        session_id: String,
    },
    /// `what` is `retro` or `pr_body`; both fields default so a bare line still parses.
    TemplateStandIn {
        #[serde(default)]
        what: String,
        #[serde(default)]
        path: String,
    },
    /// `sha` is `None` when nothing was left to commit.
    Shipped {
        sha: Option<String>,
    },
    PrOpened {
        url: String,
    },
    CiGreen {
        head: String,
    },
    /// `round` counts the fix rounds done before it.
    CiRed {
        check: String,
        round: u32,
        head: String,
    },
    /// The one rerun allowed: `gh run rerun <id> --failed`.
    CiRerun {
        check: String,
        run_id: u64,
        head: String,
    },
    /// `round` is 1-based; `attempt` counts that round's workers.
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
    CloseoutBlocked {
        phase: String,
        block_id: String,
        question: String,
    },
    /// Every field defaults so a bare `run_finished` line still parses.
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
    /// Ends a stopped run in place of `run_finished`, once its in-flight workers exited.
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
    /// A kind a newer `factory` wrote: it parses and every consumer passes over it.
    #[serde(other, skip_serializing)]
    Unknown,
}

impl EventKind {
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Event {
    pub seq: u64,
    pub ts: String,
    #[serde(flatten)]
    pub kind: EventKind,
}

/// Locked from reading the last `seq` to the table row, one `write` per line: emitters never interleave.
pub fn emit(run_dir: &Path, db: &Db, run_id: &str, kind: EventKind) -> Result<Event> {
    let path = run_dir.join(FILE);
    let mut f = OpenOptions::new()
        .append(true)
        .create(true)
        .open(&path)
        .map_err(io(format!("open {}", path.display())))?;
    f.lock().map_err(io(format!("lock {}", path.display())))?;
    let (last, torn) = tail(&path)?;
    let seq = last + 1;
    let event = Event {
        seq,
        ts: clock::now_rfc3339(),
        kind,
    };
    let mut value = serde_json::to_value(&event).map_err(json("serialize event"))?;
    let line = serde_json::to_string(&value).map_err(json("serialize event"))? + "\n";
    let line = if torn { format!("\n{line}") } else { line };
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

/// `(line number, error)`, the number 1-based.
pub type Skipped = (usize, String);

/// Read as bytes so a write cut inside a multi-byte character tears only its own line.
pub fn scan(run_dir: &Path) -> Result<(Vec<Event>, Vec<Skipped>)> {
    let path = run_dir.join(FILE);
    let bytes = match fs::read(&path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok((Vec::new(), Vec::new())),
        Err(e) => return Err(io(format!("read {}", path.display()))(e)),
    };
    let mut out = Vec::new();
    let mut bad = Vec::new();
    for (i, l) in String::from_utf8_lossy(&bytes).lines().enumerate() {
        if l.trim().is_empty() {
            continue;
        }
        match serde_json::from_str(l) {
            Ok(e) => out.push(e),
            Err(e) => bad.push((i + 1, e.to_string())),
        }
    }
    Ok((out, bad))
}

pub fn read_all(run_dir: &Path) -> Result<Vec<Event>> {
    Ok(scan(run_dir)?.0)
}

pub fn skip_note(run_dir: &Path, skipped: &[Skipped]) -> Option<String> {
    if skipped.is_empty() {
        return None;
    }
    let lines: Vec<String> = skipped.iter().map(|(n, _)| n.to_string()).collect();
    Some(format!(
        "factory: {}: skipped unreadable line{} {}",
        run_dir.join(FILE).display(),
        if lines.len() == 1 { "" } else { "s" },
        lines.join(", ")
    ))
}

/// The last `seq` that parses, and whether the file ends mid-line.
fn tail(path: &Path) -> Result<(u64, bool)> {
    let bytes = match fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok((0, false)),
        Err(e) => return Err(io(format!("read {}", path.display()))(e)),
    };
    let seq = String::from_utf8_lossy(&bytes)
        .lines()
        .rev()
        .find_map(|l| {
            serde_json::from_str::<serde_json::Value>(l)
                .ok()?
                .get("seq")?
                .as_u64()
        })
        .unwrap_or(0);
    Ok((seq, bytes.last().is_some_and(|b| *b != b'\n')))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::run::Run;

    /// Line 2: a newer kind; 3: torn inside a multi-byte char; 4: payload missing; 6: torn at EOF.
    fn mixed() -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(br#"{"seq":1,"ts":"t","kind":"run_started"}"#);
        b.push(b'\n');
        b.extend_from_slice(br#"{"seq":2,"ts":"t","kind":"from_a_newer_factory","x":1}"#);
        b.push(b'\n');
        b.extend_from_slice(
            b"{\"seq\":3,\"ts\":\"t\",\"kind\":\"run_failed\",\"error\":\"a \xe2\x80",
        );
        b.push(b'\n');
        b.extend_from_slice(br#"{"seq":4,"ts":"t","kind":"slice_done"}"#);
        b.push(b'\n');
        b.extend_from_slice(br#"{"seq":5,"ts":"t","kind":"slice_done","slice":2}"#);
        b.push(b'\n');
        b.extend_from_slice(br#"{"seq":6,"ts":"t","kind":"run_fin"#);
        b
    }

    fn run() -> (tempfile::TempDir, Run) {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("repo");
        fs::create_dir_all(&repo).unwrap();
        let run = Run::create(&tmp.path().join("data"), &repo, "001", None, 1).unwrap();
        (tmp, run)
    }

    #[test]
    fn an_unknown_kind_reads_as_unknown_and_an_unreadable_line_is_skipped() {
        let (_tmp, run) = run();
        fs::write(run.dir().join(FILE), mixed()).unwrap();
        let (events, skipped) = scan(run.dir()).unwrap();
        let got: Vec<(u64, EventKind)> = events.into_iter().map(|e| (e.seq, e.kind)).collect();
        assert_eq!(
            got,
            vec![
                (1, EventKind::RunStarted),
                (2, EventKind::Unknown),
                (5, EventKind::SliceDone { slice: 2 }),
            ]
        );
        assert_eq!(
            skipped.iter().map(|(n, _)| *n).collect::<Vec<_>>(),
            [3, 4, 6]
        );
        assert_eq!(read_all(run.dir()).unwrap().len(), 3);
        assert_eq!(
            skip_note(run.dir(), &skipped),
            Some(format!(
                "factory: {}: skipped unreadable lines 3, 4, 6",
                run.dir().join(FILE).display()
            ))
        );
        assert_eq!(
            skip_note(run.dir(), &skipped[..1])
                .unwrap()
                .split(": ")
                .last(),
            Some("skipped unreadable line 3")
        );
        assert_eq!(skip_note(run.dir(), &[]), None);
    }

    /// The names are spliced so no grep finds a use of a deleted kind.
    #[test]
    fn a_deleted_leftover_kind_reads_as_unknown() {
        let (_tmp, run) = run();
        let kind = |what: &str| format!("{what}_{}", "leftovers");
        let lines = [
            format!(
                r#"{{"seq":1,"ts":"t","kind":"{}","role":"polish","files":["a"],"blobs":{{"a":"b"}}}}"#,
                kind("closeout")
            ),
            format!(
                r#"{{"seq":2,"ts":"t","kind":"{}","slice":1,"files":["a"]}}"#,
                kind("untracked")
            ),
            format!(
                r#"{{"seq":3,"ts":"t","kind":"{}","n":1,"role":"fix","files":[]}}"#,
                kind("checkpoint")
            ),
        ];
        fs::write(run.dir().join(FILE), lines.join("\n") + "\n").unwrap();
        let (events, skipped) = scan(run.dir()).unwrap();
        let got: Vec<(u64, EventKind)> = events.into_iter().map(|e| (e.seq, e.kind)).collect();
        assert_eq!(
            got,
            vec![
                (1, EventKind::Unknown),
                (2, EventKind::Unknown),
                (3, EventKind::Unknown),
            ]
        );
        assert!(skipped.is_empty(), "{skipped:?}");
    }

    #[test]
    fn an_emit_after_a_torn_last_line_takes_the_next_seq_on_a_line_of_its_own() {
        let (_tmp, mut run) = run();
        fs::write(run.dir().join(FILE), mixed()).unwrap();
        // The torn line 6 falls back to line 5's seq.
        let e = run.emit(EventKind::SliceDone { slice: 3 }).unwrap();
        assert_eq!(e.seq, 6);
        let (events, skipped) = scan(run.dir()).unwrap();
        assert_eq!(
            events.last().map(|e| &e.kind),
            Some(&EventKind::SliceDone { slice: 3 })
        );
        assert_eq!(
            skipped.iter().map(|(n, _)| *n).collect::<Vec<_>>(),
            [3, 4, 6]
        );
        let e = run.emit(EventKind::RunStarted).unwrap();
        assert_eq!(e.seq, 7);
    }

    #[test]
    fn an_unknown_kind_is_never_written() {
        let (_tmp, mut run) = run();
        assert!(run.emit(EventKind::Unknown).is_err());
        assert_eq!(fs::read_to_string(run.dir().join(FILE)).unwrap(), "");
        assert!(run.db().events(run.id().as_str()).unwrap().is_empty());
        assert_eq!(EventKind::Unknown.name(), "unknown");
    }

    #[test]
    fn the_legacy_log_merge_events_still_read() {
        let merged = r#"{"fragments":["docs/features/003-observatory/log-slice-01.md","docs/features/003-observatory/log-slice-02.md"],"kind":"log_merged","seq":26,"ts":"2026-10-13T05:42:10Z"}"#;
        let refused = r#"{"block_id":"BLOCK-1","detail":"✗ log-slice-03.md has content above its first `## ` heading\n2 problem(s) — nothing merged.","kind":"merge_refused","seq":26,"ts":"2026-10-11T22:55:57Z"}"#;
        let e: Event = serde_json::from_str(merged).unwrap();
        assert!(matches!(e.kind, EventKind::LogMerged { ref fragments } if fragments.len() == 2));
        let e: Event = serde_json::from_str(refused).unwrap();
        assert!(
            matches!(e.kind, EventKind::MergeRefused { ref block_id, .. } if block_id == "BLOCK-1")
        );
    }
}
