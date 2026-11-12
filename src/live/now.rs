//! `now`, what the run waits on, and `attention`, what the owner is asked.
use std::fs;
use std::path::Path;
use std::time::UNIX_EPOCH;

use serde_json::{Value, json};

use super::Ctx;
use super::gate::{current_ci_log, excerpt_json, patterns, read_log, run_file};
use crate::clock;
use crate::cockpit;
use crate::docs::log;
use crate::events::{Event, EventKind};
use crate::excerpt;
use crate::run::{Phase, WorkerRef};

pub fn now(ctx: &Ctx) -> Value {
    if ctx.events.is_empty() {
        return Value::Null;
    }
    let s = &ctx.state;
    let w = waiting(ctx);
    let stopping = s.stop_requested.then(|| stopping(ctx)).flatten();
    json!({
        "waitingOn": w.on,
        "label": w.label,
        "since": w.since,
        "progress": w.progress,
        "resumesAt": w.resumes_at,
        "idleSeconds": idle_seconds(ctx),
        "driverAlive": s.driver_alive(),
        "stopping": stopping,
    })
}

struct Waiting {
    on: &'static str,
    label: String,
    since: Option<String>,
    progress: Value,
    resumes_at: Option<String>,
}

impl Waiting {
    fn new(on: &'static str, label: String, since: Option<&Event>) -> Waiting {
        Waiting {
            on,
            label,
            since: since.map(|e| e.ts.clone()),
            progress: Value::Null,
            resumes_at: None,
        }
    }
}

fn waiting(ctx: &Ctx) -> Waiting {
    let s = &ctx.state;
    let events = &ctx.events;
    if s.phase.is_over() {
        let end = events.iter().rev().find(|e| cockpit::is_terminal(&e.kind));
        return Waiting::new("done", format!("run {}", s.phase), end);
    }
    let owed = owner_entries(ctx);
    if let Some(first) = owed.iter().min_by_key(|o| o.seq) {
        let ids: Vec<&str> = owed.iter().map(|o| o.title_or_id()).collect();
        return Waiting::new(
            "owner",
            format!("the owner: {}", ids.join("; ")),
            first.event,
        );
    }
    if let Some(e) = cockpit::park(s, events) {
        let reason = match &e.kind {
            EventKind::RunParked { reason, .. } => reason.as_str(),
            _ => "",
        };
        return Waiting::new("budget", format!("budget: {reason}"), Some(e));
    }
    if s.phase == Phase::Paused {
        let pause = events.iter().rev().find_map(|e| match &e.kind {
            EventKind::RunPaused { literal, resets_at } => Some((e, literal, *resets_at)),
            _ => None,
        });
        let resets = s.paused_until.or(pause.map(|(_, _, at)| at));
        let label = match pause {
            Some((_, literal, _)) if !literal.is_empty() => format!("quota pause: {literal}"),
            _ => "quota pause".to_string(),
        };
        let mut w = Waiting::new("quota", label, pause.map(|(e, _, _)| e));
        w.resumes_at = resets.map(clock::rfc3339);
        return w;
    }
    if !s.driver_alive() {
        let last = events.last();
        let label = match last.map(|e| &e.kind) {
            Some(EventKind::BlockAnswered { id, .. } | EventKind::AskAnswered { id, .. }) => {
                format!("the driver to pick up the answer to {id}")
            }
            _ => "the driver, which is not running".to_string(),
        };
        return Waiting::new("driver", label, last);
    }
    if let Some((e, n, attempt)) = running_gate(events) {
        return Waiting::new("gate", gate_label(n, attempt), Some(e));
    }
    if let Some(at) = open_collect(events) {
        let e = &events[at];
        let of = match e.kind {
            EventKind::CollectStarted { passes, .. } => passes,
            _ => 0,
        };
        let done = events[at..]
            .iter()
            .filter(|e| matches!(e.kind, EventKind::ReviewPassDone { .. }))
            .count();
        let mut w = Waiting::new(
            "review",
            format!("review, {done} of {of} passes done"),
            Some(e),
        );
        w.progress = json!({"done": done, "of": of});
        return w;
    }
    if let Some((e, label)) = ci_wait(ctx) {
        return Waiting::new("ci", label, Some(e));
    }
    let live: Vec<&WorkerRef> = live_workers(ctx);
    if !live.is_empty() {
        let label = live
            .iter()
            .map(|w| worker_label(w))
            .collect::<Vec<_>>()
            .join(", ");
        let mut w = Waiting::new("worker", label, None);
        w.since = live.iter().map(|w| w.spawned_at.clone()).min();
        return w;
    }
    Waiting::new("nothing", "nothing in flight".to_string(), None)
}

fn live_workers(ctx: &Ctx) -> Vec<&WorkerRef> {
    let s = &ctx.state;
    s.workers
        .iter()
        .filter(|w| w.outcome.is_none() && s.worker_alive(w))
        .collect()
}

fn worker_label(w: &WorkerRef) -> String {
    match (w.slice, w.checkpoint) {
        (Some(s), _) => format!("slice {s:02} {} worker", w.role),
        (None, Some(n)) => format!("checkpoint {n} {} worker", w.role),
        (None, None) => format!("closeout {} worker", w.role),
    }
}

fn gate_label(n: u32, attempt: u32) -> String {
    format!("checkpoint {n} gate, attempt {attempt}")
}

/// A `gate_started` no result followed.
fn running_gate(events: &[Event]) -> Option<(&Event, u32, u32)> {
    let e = events.iter().rev().find(|e| {
        matches!(
            e.kind,
            EventKind::GateStarted { .. }
                | EventKind::GateRun { .. }
                | EventKind::GateCached { .. }
        )
    })?;
    match e.kind {
        EventKind::GateStarted { n, attempt, .. } => Some((e, n, attempt)),
        _ => None,
    }
}

/// The index of a `collect_started` no `collect_done` followed.
fn open_collect(events: &[Event]) -> Option<usize> {
    let at = events.iter().rposition(|e| {
        matches!(
            e.kind,
            EventKind::CollectStarted { .. } | EventKind::CollectDone { .. }
        )
    })?;
    matches!(events[at].kind, EventKind::CollectStarted { .. }).then_some(at)
}

/// The newest CI event when it is `ci_waiting` or `ci_no_checks`; else, in closeout, the newest head when no verdict
/// followed it, which covers the grace window before checks register, a head lag and a rerun.
fn ci_wait(ctx: &Ctx) -> Option<(&Event, String)> {
    let events = &ctx.events;
    if let Some(e) = events.iter().rev().find(|e| is_ci(&e.kind)) {
        match &e.kind {
            EventKind::CiWaiting { checks } => {
                return Some((e, format!("CI checks: {}", checks.join(", "))));
            }
            EventKind::CiNoChecks { minutes } => {
                return Some((e, format!("CI checks, none reported after {minutes} min")));
            }
            _ => {}
        }
    }
    if ctx.state.phase != Phase::Closeout {
        return None;
    }
    let head = events.iter().rev().find(|e| {
        matches!(
            e.kind,
            EventKind::PrOpened { .. }
                | EventKind::CiFixCommitted { .. }
                | EventKind::CiRerun { .. }
                | EventKind::CiGreen { .. }
                | EventKind::CiRed { .. }
        )
    })?;
    let label = match &head.kind {
        EventKind::PrOpened { .. } => "CI checks to register on the PR".to_string(),
        EventKind::CiFixCommitted { round, sha, .. } => {
            let short = sha.get(..7).unwrap_or(sha);
            format!("CI checks to register on fix round {round} ({short})")
        }
        EventKind::CiRerun { check, .. } => format!("CI rerun of {check}"),
        _ => return None,
    };
    Some((head, label))
}

fn is_ci(kind: &EventKind) -> bool {
    matches!(
        kind,
        EventKind::CiWaiting { .. }
            | EventKind::CiNoChecks { .. }
            | EventKind::CiRed { .. }
            | EventKind::CiGreen { .. }
    )
}

fn stopping(ctx: &Ctx) -> Option<Value> {
    if let Some((_, n, attempt)) = running_gate(&ctx.events) {
        return Some(json!({"on": "gate", "what": gate_label(n, attempt)}));
    }
    let live = live_workers(ctx);
    (!live.is_empty()).then(|| {
        let what = live
            .iter()
            .map(|w| worker_label(w))
            .collect::<Vec<_>>()
            .join(", ");
        json!({"on": "worker", "what": what})
    })
}

fn idle_seconds(ctx: &Ctx) -> u64 {
    let event = ctx.events.last().and_then(|e| clock::from_rfc3339(&e.ts));
    let newest = [
        event,
        newest_mtime(&ctx.run_dir.join("workers")),
        newest_mtime(&ctx.run_dir.join("gates")),
    ]
    .into_iter()
    .flatten()
    .max();
    newest.map_or(0, |t| ctx.now.saturating_sub(t))
}

fn newest_mtime(dir: &Path) -> Option<u64> {
    fs::read_dir(dir)
        .ok()?
        .filter_map(|e| {
            let t = e.ok()?.metadata().ok()?.modified().ok()?;
            Some(t.duration_since(UNIX_EPOCH).ok()?.as_secs())
        })
        .max()
}

/// One thing the owner is asked or told.
struct Entry<'a> {
    id: String,
    kind: &'static str,
    seq: u64,
    event: Option<&'a Event>,
    title: String,
    body: Option<String>,
    reason: Value,
    answer: Option<Vec<String>>,
}

impl Entry<'_> {
    fn title_or_id(&self) -> &str {
        if self.kind == "park" {
            &self.title
        } else {
            &self.id
        }
    }

    fn json(&self) -> Value {
        json!({
            "id": self.id,
            "kind": self.kind,
            "openedAt": self.event.map(|e| e.ts.clone()),
            "title": self.title,
            "body": self.body,
            "reason": self.reason,
            "answer": self.answer,
        })
    }
}

pub fn attention(ctx: &Ctx) -> Value {
    let mut all = owner_entries(ctx);
    all.extend(warnings(&ctx.events));
    all.iter().map(Entry::json).collect()
}

/// The open questions in `cockpit::questions` order, else the park that has none.
fn owner_entries(ctx: &Ctx) -> Vec<Entry<'_>> {
    let s = &ctx.state;
    let events = &ctx.events;
    let logged = ctx
        .feature_dir
        .as_deref()
        .and_then(|dir| log::entries(dir).ok())
        .map(|(entries, _)| entries)
        .unwrap_or_default();
    let body = |id: &str| {
        logged
            .iter()
            .rev()
            .find(|e| e.id.as_deref() == Some(id))
            .map(|e| e.body.clone())
    };
    let mut out: Vec<Entry> = cockpit::questions(s, events)
        .into_iter()
        .filter_map(|e| {
            let q = cockpit::question(e, &s.feature, s.id.as_str())?;
            Some(Entry {
                kind: if q.id.starts_with("ASK-") {
                    "ask"
                } else {
                    "block"
                },
                seq: e.seq,
                event: Some(e),
                body: body(&q.id),
                reason: question_reason(ctx, e),
                title: q.text,
                answer: Some(q.answer),
                id: q.id,
            })
        })
        .collect();
    if out.is_empty()
        && let Some(e) = bare_park(s.phase, events)
    {
        let reason = match &e.kind {
            EventKind::RunWaiting { reason, .. } => reason.clone(),
            _ => String::new(),
        };
        out.push(Entry {
            id: format!("park-{}", e.seq),
            kind: "park",
            seq: e.seq,
            event: Some(e),
            title: reason,
            body: None,
            reason: source("land", None, Value::Null),
            answer: None,
        });
    }
    out
}

/// The newest park of a waiting run, when it names no block and no ask and nothing restarted or re-parked the run since.
fn bare_park(phase: Phase, events: &[Event]) -> Option<&Event> {
    if phase != Phase::WaitingOwner {
        return None;
    }
    let e = events.iter().rev().find(|e| {
        matches!(
            e.kind,
            EventKind::RunWaiting { .. }
                | EventKind::RunParked { .. }
                | EventKind::RunStarted
                | EventKind::RunResumed
        )
    })?;
    match &e.kind {
        EventKind::RunWaiting { blocked, asks, .. } if blocked.is_empty() && asks.is_empty() => {
            Some(e)
        }
        _ => None,
    }
}

fn source(source: &str, log: Option<String>, excerpt: Value) -> Value {
    json!({"source": source, "log": log, "excerpt": excerpt})
}

fn question_reason(ctx: &Ctx, e: &Event) -> Value {
    let logged = |rel: String| {
        let excerpt = run_file(ctx, &rel)
            .and_then(|p| read_log(&p))
            .map_or(Value::Null, |r| {
                excerpt_json(&excerpt::excerpt(&r.text, &patterns(ctx)))
            });
        (Some(rel), excerpt)
    };
    match &e.kind {
        EventKind::CheckpointBlocked { .. } => {
            let gate = ctx
                .events
                .iter()
                .rev()
                .filter(|g| g.seq < e.seq)
                .find_map(|g| match &g.kind {
                    EventKind::GateRun { red, log, .. }
                    | EventKind::GateCached { red, log, .. } => Some((*red, log.clone())),
                    _ => None,
                });
            match gate {
                Some((true, log)) => {
                    let (log, excerpt) = logged(log);
                    source("gate", log, excerpt)
                }
                _ => source("driver", None, Value::Null),
            }
        }
        EventKind::CloseoutBlocked { phase, .. } if phase == "ci" => {
            match current_ci_log(ctx, e.seq) {
                Some(r) => {
                    let (log, excerpt) = logged(format!("ci/{r}.log"));
                    source("ci", log, excerpt)
                }
                None => source("driver", None, Value::Null),
            }
        }
        _ => source("driver", None, Value::Null),
    }
}

fn warnings(events: &[Event]) -> Vec<Entry<'_>> {
    events
        .iter()
        .filter_map(|e| {
            let title = match &e.kind {
                EventKind::PolishFindingsMissing { session_id } => {
                    format!("polish findings missing from worker {session_id}")
                }
                EventKind::PackSectionMissing { slice, section } => {
                    format!("slice {slice:02} pack is missing section {section}")
                }
                _ => return None,
            };
            Some(Entry {
                id: format!("warn-{}", e.seq),
                kind: "warn",
                seq: e.seq,
                event: Some(e),
                title,
                body: None,
                reason: source("driver", None, Value::Null),
                answer: None,
            })
        })
        .collect()
}
