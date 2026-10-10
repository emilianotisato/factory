//! `factory status [<run>]` (PRD §9 FR-7): the board in ten lines — the
//! run, the slices, the lanes, the checkpoints, the open questions, the
//! quota, the PR, the last three events, and two lines that appear only when
//! a stop is pending or the driver is dead. Slices and lanes come from the
//! board (`docs status --json`, the truth); everything else from the run's
//! own files. No run id, or an unknown one, lists the live runs and exits 1.
use std::io::Write;

use clap::Args;

use crate::cockpit::{self, tokens};
use crate::docs::{Docs, Status};
use crate::error::io;
use crate::events::{Event, EventKind};
use crate::run::Run;
use crate::scheduler::pad_list;
use crate::stream::RateLimit;
use crate::{Result, clock, paths, quota, worker};

#[derive(Args, Debug, Clone)]
pub struct StatusArgs {
    /// The run id (`001-20261009-1a2b`); none lists the live runs
    pub run: Option<String>,
}

pub fn status(args: &StatusArgs, out: &mut dyn Write) -> Result<i32> {
    let data_dir = paths::data_dir();
    let Some(id) = &args.run else {
        cockpit::list_live(&data_dir, out)?;
        return Ok(1);
    };
    let Some(run) = cockpit::resolve(&data_dir, id, out)? else {
        return Ok(1);
    };
    for l in board(&run)?.iter().take(10) {
        writeln!(out, "{l}").map_err(io("stdout"))?;
    }
    Ok(0)
}

/// The ten lines, in order; the last two only when they apply.
pub fn board(run: &Run) -> Result<Vec<String>> {
    let s = run.state();
    let events = run.events()?;
    let mut lines = Vec::with_capacity(10);

    let started = clock::from_rfc3339(&s.started_at).unwrap_or_else(clock::unix_now);
    let end = if s.phase.is_over() {
        clock::from_rfc3339(&s.updated_at).unwrap_or_else(clock::unix_now)
    } else {
        clock::unix_now()
    };
    lines.push(format!(
        "run {} · {}{} · {} elapsed",
        s.id,
        s.phase,
        if s.stopped { " (stopped)" } else { "" },
        elapsed(end.saturating_sub(started))
    ));

    match Docs::locate(&s.repo).and_then(|d| d.status(&s.feature)) {
        Ok(b) => {
            lines.push(slices_line(&b));
            lines.push(lanes_line(&b));
        }
        Err(e) => {
            lines.push(format!("slices: board unavailable ({e})"));
            lines.push("lanes: board unavailable".to_string());
        }
    }

    lines.push(checkpoints_line(&events));

    let open = cockpit::open_questions(&events);
    lines.push(if open.is_empty() {
        "open: none".to_string()
    } else {
        format!(
            "open: {}",
            open.iter()
                .map(|id| describe(id, &events))
                .collect::<Vec<_>>()
                .join(" · ")
        )
    });

    let now = clock::unix_now();
    let pause = match s.paused_until {
        Some(u) if u > now => format!("paused until {}", clock::rfc3339(u)),
        _ => "not paused".to_string(),
    };
    let limit = last_rate_limit(run).map_or("no rate_limit_event yet".to_string(), |rl| {
        format!(
            "limit {}{}{}",
            rl.status.as_deref().unwrap_or("?"),
            rl.utilization_pct
                .map_or(String::new(), |u| format!(" {u}%")),
            rl.resets_at.map_or(String::new(), |r| format!(
                " (resets {})",
                clock::rfc3339(r)
            ))
        )
    });
    let used = quota::weighted(run.db(), run.id().as_str())?;
    lines.push(format!(
        "quota: {pause} · {limit} · budget: {}/{}{}",
        tokens(used),
        tokens(s.budget_weighted_tokens),
        if cockpit::parked_on(s, &events).is_some() {
            " (parked)"
        } else {
            ""
        }
    ));

    lines.push(format!(
        "PR: {}",
        s.pr_url.as_deref().unwrap_or("no PR yet")
    ));

    lines.push(format!("last: {}", last_three(&events)));

    if s.stop_requested && !s.phase.is_over() {
        lines.push(format!(
            "stop requested: {} in flight; the run finishes when they exit",
            cockpit::in_flight(s)
        ));
    }
    if cockpit::driver_dead(s, &events) {
        lines.push(format!(
            "driver dead (pid {}); run: factory run {}",
            s.pid, s.feature
        ));
    }
    Ok(lines)
}

fn elapsed(secs: u64) -> String {
    let (h, m, s) = (secs / 3600, secs % 3600 / 60, secs % 60);
    if h > 0 {
        format!("{h}h {m:02}m")
    } else {
        format!("{m}m {s:02}s")
    }
}

fn or_dash(s: String) -> String {
    if s.is_empty() { "—".to_string() } else { s }
}

fn slices_line(b: &Status) -> String {
    let by = |st: &str| {
        or_dash(pad_list(
            &b.slices
                .iter()
                .filter(|s| s.status.as_deref() == Some(st))
                .map(|s| s.number)
                .collect::<Vec<_>>(),
        ))
    };
    format!(
        "slices {}: done: {} · building: {} · blocked: {} · pending: {}",
        b.slices.len(),
        by("done"),
        by("building"),
        by("blocked"),
        by("pending")
    )
}

/// `lanes: 2 (1 parked) — 1: slice 01 blocked · 2: free`. A `blocked` slice
/// keeps its lane parked (AD-401); a `building` one holds it with a worker.
fn lanes_line(b: &Status) -> String {
    let holders: Vec<(u32, u32, &str)> = b
        .slices
        .iter()
        .filter_map(|s| {
            let st = s.status.as_deref()?;
            (st == "building" || st == "blocked").then_some((s.lane?, s.number, st))
        })
        .collect();
    let parked = holders.iter().filter(|h| h.2 == "blocked").count();
    let per: Vec<String> = (1..=b.lanes.max(1))
        .map(|lane| match holders.iter().find(|h| h.0 == lane) {
            Some((_, n, st)) => format!("{lane}: slice {n:02} {st}"),
            None => format!("{lane}: free"),
        })
        .collect();
    let head = if parked > 0 {
        format!("lanes: {} ({parked} parked)", b.lanes)
    } else {
        format!("lanes: {}", b.lanes)
    };
    format!("{head} — {}", per.join(" · "))
}

fn checkpoints_line(events: &[Event]) -> String {
    let done = events
        .iter()
        .filter(|e| matches!(e.kind, EventKind::CheckpointDone { .. }))
        .count();
    let gate = events.iter().rev().find_map(|e| match &e.kind {
        EventKind::GateRun {
            n,
            attempt,
            red,
            minutes,
            ..
        } => Some(format!(
            "{} (checkpoint {n} attempt {attempt}, {minutes:.2} min)",
            if *red { "red" } else { "green" }
        )),
        EventKind::GateCached {
            n, attempt, red, ..
        } => Some(format!(
            "{} (checkpoint {n} attempt {attempt}, cached)",
            if *red { "red" } else { "green" }
        )),
        _ => None,
    });
    format!(
        "checkpoints: {done} done · last gate: {}",
        gate.unwrap_or_else(|| "not run yet".to_string())
    )
}

/// `BLOCK-101 (slice 01)`, `BLOCK-5 (checkpoint 2)`, `BLOCK-1 (closeout ci)`, `ASK-1`.
fn describe(id: &str, events: &[Event]) -> String {
    let where_ = events.iter().find_map(|e| match &e.kind {
        EventKind::SliceBlocked {
            slice,
            block_id: Some(b),
            ..
        } if b == id => Some(format!("slice {slice:02}")),
        EventKind::CheckpointBlocked { n, block_id, .. } if block_id == id => {
            Some(format!("checkpoint {n}"))
        }
        EventKind::MergeRefused { block_id, .. } if block_id == id => Some("log merge".to_string()),
        EventKind::CloseoutBlocked {
            phase, block_id, ..
        } if block_id == id => Some(format!("closeout {phase}")),
        _ => None,
    });
    match where_ {
        Some(w) => format!("{id} ({w})"),
        None => id.to_string(),
    }
}

/// The last `rate_limit_event` on disk, newest worker first; the streams of
/// at most the last five workers are read.
fn last_rate_limit(run: &Run) -> Option<RateLimit> {
    run.state()
        .workers
        .iter()
        .rev()
        .take(5)
        .find_map(|w| worker::summary_from_disk(run.dir(), &w.session_id).rate_limit)
}

/// `slice_done(03) → run_waiting → stop_requested`: the kind and the one
/// field that names its subject, when it has one.
fn last_three(events: &[Event]) -> String {
    if events.is_empty() {
        return "none yet".to_string();
    }
    events
        .iter()
        .rev()
        .take(3)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .map(short)
        .collect::<Vec<_>>()
        .join(" → ")
}

fn short(e: &Event) -> String {
    let name = e.kind.name();
    let Ok(serde_json::Value::Object(o)) = serde_json::to_value(&e.kind) else {
        return name;
    };
    let subject = o
        .get("slice")
        .and_then(|v| v.as_u64())
        .map(|n| format!("{n:02}"))
        .or_else(|| o.get("n").and_then(|v| v.as_u64()).map(|n| n.to_string()))
        .or_else(|| {
            ["block_id", "id", "check", "literal", "reason"]
                .iter()
                .find_map(|k| o.get(*k).and_then(|v| v.as_str()).map(str::to_string))
        });
    match subject {
        Some(s) => format!("{name}({s})"),
        None => name,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn elapsed_reads_as_minutes_then_hours() {
        assert_eq!(elapsed(0), "0m 00s");
        assert_eq!(elapsed(754), "12m 34s");
        assert_eq!(elapsed(3 * 3600 + 7 * 60), "3h 07m");
    }

    #[test]
    fn the_last_three_events_name_their_subject() {
        let ev = |seq, kind| Event {
            seq,
            ts: String::new(),
            kind,
        };
        let events = vec![
            ev(1, EventKind::RunStarted),
            ev(2, EventKind::SliceDone { slice: 3 }),
            ev(
                3,
                EventKind::RunWaiting {
                    blocked: vec![1],
                    asks: vec![],
                },
            ),
            ev(
                4,
                EventKind::BlockAnswered {
                    id: "BLOCK-101".into(),
                    slice: Some(1),
                    text: "x".into(),
                },
            ),
        ];
        assert_eq!(
            last_three(&events),
            "slice_done(03) → run_waiting → block_answered(01)"
        );
        assert_eq!(last_three(&[]), "none yet");
    }
}
