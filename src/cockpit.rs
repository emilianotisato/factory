//! FR-7's shared half (PRD §9): which events the cockpit must see, the one
//! line each becomes, the cursor `factory wait` advances, the status line
//! `wait` and `status` print, and the live-runs listing a missing run id
//! gets. `cli/wait.rs` and `cli/status.rs` are thin over this.
use std::fs;
use std::io::Write;
use std::path::Path;

use crate::error::io;
use crate::events::{Event, EventKind};
use crate::run::{self, Run, RunState, pid_alive};
use crate::{Result, clock};

/// `runs/<id>/cockpit.cursor`: the last `seq` the cockpit was shown.
pub const CURSOR_FILE: &str = "cockpit.cursor";

/// §9: a block, an ask, a quota pause, a cap reached, finished, failed — and
/// `run_stopped`, the end of a stopped run (AD-1002). Everything else is
/// progress and only shows in `factory status`.
pub fn is_cockpit(kind: &EventKind) -> bool {
    matches!(
        kind,
        EventKind::SliceBlocked { .. }
            | EventKind::CheckpointBlocked { .. }
            | EventKind::MergeRefused { .. }
            | EventKind::CloseoutBlocked { .. }
            | EventKind::AskOpened { .. }
            | EventKind::RunPaused { .. }
            | EventKind::RunParked { .. }
            | EventKind::RunFinished { .. }
            | EventKind::RunStopped { .. }
            | EventKind::RunFailed { .. }
    )
}

fn first_line(s: &str) -> &str {
    s.lines().next().unwrap_or("").trim()
}

fn block_line(id: &str, where_: &str, question: &str) -> String {
    format!(
        "{id} {where_}: {} (answer: factory answer {id} \"...\")",
        first_line(question)
    )
}

/// The line `factory wait` prints for a cockpit event; `None` for progress.
pub fn line(ev: &Event, feature: &str) -> Option<String> {
    Some(match &ev.kind {
        EventKind::SliceBlocked {
            slice,
            block_id,
            question,
        } => match block_id {
            Some(id) => block_line(
                id,
                &format!("slice {slice:02}"),
                question.as_deref().unwrap_or(""),
            ),
            None => format!("slice {slice:02} blocked with no BLOCK-n in its fragment"),
        },
        EventKind::CheckpointBlocked {
            n,
            block_id,
            question,
        } => block_line(block_id, &format!("checkpoint {n}"), question),
        EventKind::MergeRefused { block_id, detail } => {
            block_line(block_id, "log merge refused", detail)
        }
        EventKind::CloseoutBlocked {
            phase,
            block_id,
            question,
        } => block_line(block_id, &format!("closeout {phase}"), question),
        EventKind::AskOpened { id, text } => format!(
            "{id}: {} (answer: factory answer {id} \"...\")",
            first_line(text)
        ),
        EventKind::RunPaused { literal, resets_at } => {
            format!("paused until {} ({literal})", clock::rfc3339(*resets_at))
        }
        EventKind::RunParked {
            reason,
            weighted,
            budget,
        } => format!("parked: {reason} {}/{}", tokens(*weighted), tokens(*budget)),
        EventKind::RunFinished { pr_url, .. } => {
            format!("finished: {}", pr_url.as_deref().unwrap_or("no PR"))
        }
        EventKind::RunStopped { slices, .. } => {
            format!("stopped: {slices} slice(s) done — factory run {feature} resumes")
        }
        EventKind::RunFailed { error } => format!("failed: {}", first_line(error)),
        _ => return None,
    })
}

/// `31.2M`, `30M`, `11.6k`, `42`.
pub fn tokens(n: u64) -> String {
    const M: u64 = 1_000_000;
    const K: u64 = 1_000;
    if n >= M {
        if n.is_multiple_of(M) {
            format!("{}M", n / M)
        } else {
            format!("{:.1}M", n as f64 / M as f64)
        }
    } else if n >= K {
        // Rounded to a tenth of a k first: 999,950 is `1.0M`, not `1000.0k`.
        let k = (n as f64 / 100.0).round() / 10.0;
        if k >= K as f64 {
            format!("{:.1}M", k / K as f64)
        } else {
            format!("{k:.1}k")
        }
    } else {
        n.to_string()
    }
}

/// The `BLOCK-n` and `ASK-n` opened and not yet answered, in opening order.
/// One source for `wait` and `status`: the run's own events, which is also
/// what `factory answer` writes (AD-1003).
pub fn open_questions(events: &[Event]) -> Vec<String> {
    let mut open: Vec<String> = Vec::new();
    for e in events {
        if let Some(id) = opens(&e.kind) {
            if !open.iter().any(|o| o == id) {
                open.push(id.to_string());
            }
        } else if let EventKind::BlockAnswered { id, .. } | EventKind::AskAnswered { id, .. } =
            &e.kind
        {
            open.retain(|o| o != id);
        }
    }
    open
}

/// The `BLOCK-n` or `ASK-n` an event opens.
fn opens(kind: &EventKind) -> Option<&str> {
    match kind {
        EventKind::SliceBlocked {
            block_id: Some(id), ..
        }
        | EventKind::CheckpointBlocked { block_id: id, .. }
        | EventKind::MergeRefused { block_id: id, .. }
        | EventKind::CloseoutBlocked { block_id: id, .. }
        | EventKind::AskOpened { id, .. } => Some(id),
        _ => None,
    }
}

/// What a cockpit was shown and the owner still owes (AD-40005): the event
/// that opened each question still open and the park the run still waits
/// on, at or before `cursor`, in `seq` order. A cockpit that died after
/// `wait` printed them never relayed them, so `wait` prints them again
/// until they close. None once the run is over.
pub fn owed<'a>(state: &RunState, events: &'a [Event], cursor: u64) -> Vec<&'a Event> {
    if state.phase.is_over() {
        return Vec::new();
    }
    owed_from(events, cursor, parked_on(state, events).is_some())
}

fn owed_from(events: &[Event], cursor: u64, parked: bool) -> Vec<&Event> {
    let open = open_questions(events);
    let mut owed: Vec<&Event> = open
        .iter()
        .filter_map(|id| events.iter().rev().find(|e| opens(&e.kind) == Some(id)))
        .collect();
    if parked {
        owed.extend(
            events
                .iter()
                .rev()
                .find(|e| matches!(e.kind, EventKind::RunParked { .. })),
        );
    }
    owed.retain(|e| e.seq <= cursor);
    owed.sort_by_key(|e| e.seq);
    owed.dedup_by_key(|e| e.seq);
    owed
}

/// `waiting-owner, BLOCK-101 open` · `paused until …, no open question` ·
/// `waiting-owner, parked: budget 31.2M/30M, no open question`.
pub fn status_line(state: &RunState, events: &[Event]) -> String {
    let open = open_questions(events);
    let mut s = state.phase.to_string();
    if let Some(until) = state.paused_until.filter(|u| *u > clock::unix_now()) {
        s.push_str(&format!(" until {}", clock::rfc3339(until)));
    }
    if let Some(parked) = parked_on(state, events) {
        s.push_str(&format!(", {parked}"));
    }
    if open.is_empty() {
        s.push_str(", no open question");
    } else {
        s.push_str(&format!(", {} open", open.join(", ")));
    }
    s
}

/// Workers spawned, unsettled and still alive.
pub fn in_flight(state: &RunState) -> usize {
    state
        .workers
        .iter()
        .filter(|w| w.outcome.is_none() && pid_alive(w.pid))
        .count()
}

/// `parked: budget 31.2M/30M` while the run waits on a cap: the last
/// `run_parked` is newer than the last start or resume and the phase is
/// still `waiting-owner`. Such a run has no driver by design (AD-703).
pub fn parked_on(state: &RunState, events: &[Event]) -> Option<String> {
    if state.phase != run::Phase::WaitingOwner {
        return None;
    }
    events.iter().rev().find_map(|e| match &e.kind {
        EventKind::RunParked { .. } => Some(line(e, &state.feature)),
        EventKind::RunStarted | EventKind::RunResumed => Some(None),
        _ => None,
    })?
}

/// Pid gone and the run not over — except a run parked on preflight asks,
/// which has no driver until the last `factory answer ASK-n` (slice 04), and
/// a run parked on its budget, which `factory run NNN --budget <n>` resumes
/// (AD-703, AD-1004).
pub fn driver_dead(state: &RunState, events: &[Event]) -> bool {
    if state.phase.is_over() || pid_alive(state.pid) {
        return false;
    }
    let asks_open = open_questions(events)
        .iter()
        .any(|id| id.starts_with("ASK-"));
    !(state.branch.is_none() && asks_open) && parked_on(state, events).is_none()
}

/// The event that ends a run: `run_finished`, `run_stopped`, `run_failed`.
pub fn is_terminal(kind: &EventKind) -> bool {
    matches!(
        kind,
        EventKind::RunFinished { .. } | EventKind::RunStopped { .. } | EventKind::RunFailed { .. }
    )
}

/// The cursor, 0 when absent or unreadable (a fresh cockpit replays all).
pub fn read_cursor(run_dir: &Path) -> u64 {
    fs::read_to_string(run_dir.join(CURSOR_FILE))
        .ok()
        .and_then(|t| t.trim().parse().ok())
        .unwrap_or(0)
}

pub fn write_cursor(run_dir: &Path, seq: u64) -> Result<()> {
    let path = run_dir.join(CURSOR_FILE);
    fs::write(&path, format!("{seq}\n")).map_err(io(format!("write {}", path.display())))
}

/// `Run::open`, or `None` after printing `no run <id>` and the live runs:
/// the caller exits 1.
pub fn resolve(data_dir: &Path, id: &str, out: &mut dyn Write) -> Result<Option<Run>> {
    if Run::runs_dir(data_dir)
        .join(id)
        .join(run::STATE_FILE)
        .is_file()
    {
        return Ok(Some(Run::open(data_dir, id)?));
    }
    writeln!(out, "no run {id}").map_err(io("stdout"))?;
    list_live(data_dir, out)?;
    Ok(None)
}

/// Every run of every repo that is neither finished nor failed, one line each.
pub fn list_live(data_dir: &Path, out: &mut dyn Write) -> Result<()> {
    let live: Vec<RunState> = run::list_runs(data_dir)?
        .into_iter()
        .filter(|s| !s.phase.is_over())
        .collect();
    if live.is_empty() {
        writeln!(
            out,
            "no live runs under {}",
            Run::runs_dir(data_dir).display()
        )
        .map_err(io("stdout"))?;
        return Ok(());
    }
    writeln!(out, "live runs:").map_err(io("stdout"))?;
    for s in live {
        writeln!(
            out,
            "  {}  {}  PRD-{}  {}{}",
            s.id,
            s.phase,
            s.feature,
            s.repo.display(),
            if pid_alive(s.pid) {
                ""
            } else {
                "  (no driver)"
            }
        )
        .map_err(io("stdout"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(seq: u64, kind: EventKind) -> Event {
        Event {
            seq,
            ts: "2026-10-10T12:00:00Z".to_string(),
            kind,
        }
    }

    #[test]
    fn tokens_read_like_the_prd() {
        assert_eq!(tokens(31_200_000), "31.2M");
        assert_eq!(tokens(30_000_000), "30M");
        assert_eq!(tokens(11_609), "11.6k");
        assert_eq!(tokens(42), "42");
        assert_eq!(tokens(0), "0");
        // The k range rounds into the M range, not to `1000.0k`.
        assert_eq!(tokens(999_950), "1.0M");
        assert_eq!(tokens(999_949), "999.9k");
        assert_eq!(tokens(1_000), "1.0k");
        assert_eq!(tokens(999), "999");
    }

    #[test]
    fn every_cockpit_kind_has_a_line_and_progress_has_none() {
        let f = "001";
        let l = |k| line(&ev(1, k), f);
        assert_eq!(
            l(EventKind::SliceBlocked {
                slice: 1,
                block_id: Some("BLOCK-101".into()),
                question: Some("Which storage?\nmore".into()),
            }),
            Some(
                "BLOCK-101 slice 01: Which storage? (answer: factory answer BLOCK-101 \"...\")"
                    .into()
            )
        );
        assert_eq!(
            l(EventKind::CheckpointBlocked {
                n: 2,
                block_id: "BLOCK-5".into(),
                question: "Three reds".into(),
            }),
            Some(
                "BLOCK-5 checkpoint 2: Three reds (answer: factory answer BLOCK-5 \"...\")".into()
            )
        );
        assert_eq!(
            l(EventKind::CloseoutBlocked {
                phase: "ci".into(),
                block_id: "BLOCK-1".into(),
                question: "CI red at a foreign head".into(),
            }),
            Some(
                "BLOCK-1 closeout ci: CI red at a foreign head (answer: factory answer BLOCK-1 \"...\")"
                    .into()
            )
        );
        assert_eq!(
            l(EventKind::MergeRefused {
                block_id: "BLOCK-7".into(),
                detail: "AD-301 twice".into(),
            }),
            Some(
                "BLOCK-7 log merge refused: AD-301 twice (answer: factory answer BLOCK-7 \"...\")"
                    .into()
            )
        );
        assert_eq!(
            l(EventKind::AskOpened {
                id: "ASK-1".into(),
                text: "lanes 2 with no lanes doc".into(),
            }),
            Some("ASK-1: lanes 2 with no lanes doc (answer: factory answer ASK-1 \"...\")".into())
        );
        assert_eq!(
            l(EventKind::RunPaused {
                literal: "rejected".into(),
                resets_at: 1_791_592_200,
            }),
            Some("paused until 2026-10-10T00:30:00Z (rejected)".into())
        );
        assert_eq!(
            l(EventKind::RunParked {
                reason: "budget".into(),
                weighted: 31_200_000,
                budget: 30_000_000,
            }),
            Some("parked: budget 31.2M/30M".into())
        );
        assert_eq!(
            l(EventKind::RunFinished {
                pr_url: Some("https://example.invalid/pr/1".into()),
                slices: 3,
                checkpoints: 2,
                ci_cycles: 0,
                weighted_tokens: 0,
                duration_minutes: 1.0,
            }),
            Some("finished: https://example.invalid/pr/1".into())
        );
        assert_eq!(
            l(EventKind::RunStopped {
                slices: 1,
                weighted_tokens: 0,
                duration_minutes: 0.1,
            }),
            Some("stopped: 1 slice(s) done — factory run 001 resumes".into())
        );
        assert_eq!(
            l(EventKind::RunFailed {
                error: "boom\nstack".into(),
            }),
            Some("failed: boom".into())
        );
        for progress in [
            EventKind::RunStarted,
            EventKind::SliceDone { slice: 1 },
            EventKind::StopRequested,
            EventKind::JudgeNoted {
                text: "fine".into(),
            },
            EventKind::RunWaiting {
                blocked: vec![1],
                asks: vec![],
            },
        ] {
            assert!(!is_cockpit(&progress), "{progress:?}");
            assert_eq!(l(progress), None);
        }
    }

    #[test]
    fn open_questions_close_on_their_answer() {
        let events = vec![
            ev(
                1,
                EventKind::AskOpened {
                    id: "ASK-1".into(),
                    text: "x".into(),
                },
            ),
            ev(
                2,
                EventKind::SliceBlocked {
                    slice: 1,
                    block_id: Some("BLOCK-101".into()),
                    question: None,
                },
            ),
            ev(
                3,
                EventKind::AskAnswered {
                    id: "ASK-1".into(),
                    text: "y".into(),
                },
            ),
            ev(
                4,
                EventKind::CloseoutBlocked {
                    phase: "pr".into(),
                    block_id: "BLOCK-1".into(),
                    question: "q".into(),
                },
            ),
        ];
        assert_eq!(open_questions(&events), ["BLOCK-101", "BLOCK-1"]);
        let mut more = events.clone();
        more.push(ev(
            5,
            EventKind::BlockAnswered {
                id: "BLOCK-101".into(),
                slice: Some(1),
                text: "a".into(),
            },
        ));
        assert_eq!(open_questions(&more), ["BLOCK-1"]);
    }

    #[test]
    fn what_is_owed_is_open_and_already_shown() {
        let blocked = |seq, id: &str| {
            ev(
                seq,
                EventKind::SliceBlocked {
                    slice: 1,
                    block_id: Some(id.into()),
                    question: None,
                },
            )
        };
        let events = vec![
            ev(
                1,
                EventKind::AskOpened {
                    id: "ASK-1".into(),
                    text: "x".into(),
                },
            ),
            blocked(2, "BLOCK-101"),
            ev(
                3,
                EventKind::AskAnswered {
                    id: "ASK-1".into(),
                    text: "y".into(),
                },
            ),
            blocked(4, "BLOCK-102"),
            ev(
                5,
                EventKind::BlockAnswered {
                    id: "BLOCK-102".into(),
                    slice: Some(1),
                    text: "a".into(),
                },
            ),
            blocked(6, "BLOCK-102"),
            blocked(7, "BLOCK-103"),
        ];
        let seqs = |cursor, parked| {
            owed_from(&events, cursor, parked)
                .iter()
                .map(|e| e.seq)
                .collect::<Vec<_>>()
        };
        // The answered ASK-1 is not owed; BLOCK-102 by its re-opening; what
        // the cursor has not reached is fresh, not owed.
        assert_eq!(seqs(7, false), [2, 6, 7]);
        assert_eq!(seqs(6, false), [2, 6]);
        assert_eq!(seqs(0, false), Vec::<u64>::new());
        let mut parked = events.clone();
        parked.push(ev(
            8,
            EventKind::RunParked {
                reason: "budget".into(),
                weighted: 31_200_000,
                budget: 30_000_000,
            },
        ));
        assert_eq!(
            owed_from(&parked, 8, true)
                .iter()
                .map(|e| e.seq)
                .collect::<Vec<_>>(),
            [2, 6, 7, 8]
        );
        assert_eq!(owed_from(&parked, 8, false).len(), 3);
    }

    #[test]
    fn the_cursor_is_zero_until_written() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(read_cursor(dir.path()), 0);
        write_cursor(dir.path(), 7).unwrap();
        assert_eq!(read_cursor(dir.path()), 7);
        fs::write(dir.path().join(CURSOR_FILE), "junk").unwrap();
        assert_eq!(read_cursor(dir.path()), 0);
    }
}
