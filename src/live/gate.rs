//! `gate`, the running or last gate, and `ci`, the pull request's checks.
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use regex::Regex;
use serde_json::{Value, json};

use super::Ctx;
use crate::docs::project;
use crate::events::{Event, EventKind};
use crate::excerpt::{self, Line};

/// The most of a log one read takes, from its end.
pub const READ_MAX: u64 = 1024 * 1024;
pub const TAIL_LINES: usize = 40;

/// The last [`READ_MAX`] bytes of a log, starting at a whole line, and its size.
pub struct LogRead {
    pub bytes: u64,
    pub text: Vec<u8>,
}

pub fn read_log(path: &Path) -> Option<LogRead> {
    let mut f = File::open(path).ok()?;
    let bytes = f.metadata().ok()?.len();
    let from = bytes.saturating_sub(READ_MAX);
    f.seek(SeekFrom::Start(from)).ok()?;
    let mut text = Vec::new();
    f.take(READ_MAX).read_to_end(&mut text).ok()?;
    if from > 0
        && let Some(nl) = text.iter().position(|&b| b == b'\n')
    {
        text.drain(..=nl);
    }
    Some(LogRead { bytes, text })
}

/// The project's `gate_failure` patterns that compile; none when the file is missing or unreadable.
pub fn patterns(ctx: &Ctx) -> Vec<Regex> {
    project::load(&ctx.state.repo)
        .ok()
        .flatten()
        .map(|p| p.failure_patterns())
        .unwrap_or_default()
}

/// Each excerpt line with whether it matched or stands beside a match.
pub fn excerpt_json(lines: &[Line]) -> Value {
    lines
        .iter()
        .map(|l| json!({ "text": l.text, "hit": l.hit }))
        .collect()
}

fn tail(text: &[u8]) -> Vec<String> {
    let clean = excerpt::clean(&String::from_utf8_lossy(text));
    let mut lines: Vec<&str> = clean.split('\n').collect();
    if lines.last() == Some(&"") {
        lines.pop();
    }
    let from = lines.len().saturating_sub(TAIL_LINES);
    lines[from..]
        .iter()
        .map(|l| excerpt::cut(l, excerpt::WIDTH))
        .collect()
}

fn gate_key(kind: &EventKind) -> Option<(u32, u32)> {
    match kind {
        EventKind::GateStarted { n, attempt, .. }
        | EventKind::GateRun { n, attempt, .. }
        | EventKind::GateCached { n, attempt, .. } => Some((*n, *attempt)),
        _ => None,
    }
}

fn is_result(kind: &EventKind) -> bool {
    matches!(
        kind,
        EventKind::GateRun { .. } | EventKind::GateCached { .. }
    )
}

pub fn gate(ctx: &Ctx) -> Value {
    let events = &ctx.events;
    let Some(at) = events.iter().rposition(|e| gate_key(&e.kind).is_some()) else {
        return Value::Null;
    };
    let newest = &events[at];
    let Some((n, attempt)) = gate_key(&newest.kind) else {
        return Value::Null;
    };
    let started: Option<&Event> = events[..=at].iter().rev().find(|e| {
        matches!(&e.kind, EventKind::GateStarted { n: sn, attempt: sa, .. } if (*sn, *sa) == (n, attempt))
    });
    let (running, red, minutes, cached, log) = match &newest.kind {
        EventKind::GateStarted { log, .. } => (true, None, None, false, log),
        EventKind::GateRun {
            red, minutes, log, ..
        } => (false, Some(*red), Some(*minutes), false, log),
        EventKind::GateCached { red, log, .. } => (false, Some(*red), None, true, log),
        _ => return Value::Null,
    };
    let cmd = started.and_then(|e| match &e.kind {
        EventKind::GateStarted { cmd, .. } => Some(cmd.clone()),
        _ => None,
    });
    let prev_result = events[..at].iter().rposition(|e| is_result(&e.kind));
    let fix_empty = events[prev_result.map_or(0, |i| i + 1)..]
        .iter()
        .any(|e| matches!(e.kind, EventKind::FixEmpty { n: Some(fn_), .. } if fn_ == n));

    let read = read_log(&ctx.run_dir.join(log));
    let excerpt = match (&read, red) {
        (Some(r), Some(true)) => excerpt_json(&excerpt::excerpt(&r.text, &patterns(ctx))),
        _ => Value::Null,
    };
    json!({
        "n": n,
        "attempt": attempt,
        "cmd": cmd,
        "running": running,
        "startedAt": started.map(|e| e.ts.clone()),
        "red": red,
        "minutes": minutes,
        "cached": cached,
        "log": log,
        "bytes": read.as_ref().map(|r| r.bytes),
        "tail": read.as_ref().map(|r| tail(&r.text)),
        "excerpt": excerpt,
        "fixEmpty": fix_empty,
    })
}

/// The highest `<round>` of `ci/<round>.log`.
fn newest_round(run_dir: &Path) -> Option<u32> {
    fs::read_dir(run_dir.join("ci"))
        .ok()?
        .filter_map(|e| {
            let name = e.ok()?.file_name();
            name.to_str()?.strip_suffix(".log")?.parse::<u32>().ok()
        })
        .max()
}

pub fn ci(ctx: &Ctx) -> Value {
    let round = newest_round(&ctx.run_dir);
    if ctx.events.is_empty() && round.is_none() {
        return Value::Null;
    }
    let state = ctx
        .events
        .iter()
        .rev()
        .find_map(|e| match e.kind {
            EventKind::CiGreen { .. } => Some("green"),
            EventKind::CiRed { .. } => Some("red"),
            EventKind::CiWaiting { .. } => Some("waiting"),
            EventKind::CiNoChecks { .. } => Some("noChecks"),
            _ => None,
        })
        .unwrap_or("none");
    let head = ctx.events.iter().rev().find_map(|e| match &e.kind {
        EventKind::CiGreen { head } | EventKind::CiRed { head, .. } => Some(head.clone()),
        _ => None,
    });
    let checks = ctx
        .events
        .iter()
        .rev()
        .find_map(|e| match &e.kind {
            EventKind::CiWaiting { checks } => Some(checks.clone()),
            _ => None,
        })
        .unwrap_or_default();
    let excerpt = match round {
        Some(r) if state == "red" => read_log(&ctx.run_dir.join(format!("ci/{r}.log")))
            .map(|read| excerpt_json(&excerpt::excerpt(&read.text, &patterns(ctx))))
            .unwrap_or(Value::Null),
        _ => Value::Null,
    };
    json!({
        "state": state,
        "round": round,
        "head": head,
        "checks": checks,
        "excerpt": excerpt,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_log_over_the_read_cap_starts_at_a_whole_line() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("big.log");
        let line = format!("{}\n", "x".repeat(99));
        let body = line.repeat(11_000);
        fs::write(&p, &body).unwrap();
        let r = read_log(&p).unwrap();
        assert_eq!(r.bytes, body.len() as u64);
        assert!(r.text.len() as u64 <= READ_MAX);
        assert!(r.text.starts_with(line.as_bytes()));
        assert!(read_log(&d.path().join("gone.log")).is_none());
    }
}
