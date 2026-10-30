//! Slices and lanes come from the board, the truth; everything else from the run's own files.
use std::io::Write;
use std::path::Path;

use clap::Args;
use serde::Serialize;

use crate::cockpit::{self, tokens};
use crate::docs::{Docs, Status};
use crate::error::{io, json};
use crate::events::{self, Event, EventKind};
use crate::run::{Phase, Run, RunState};
use crate::scheduler::pad_list;
use crate::stream::RateLimit;
use crate::{Result, clock, paths, quota, worker};

#[derive(Args, Debug, Clone)]
pub struct StatusArgs {
    /// The run id (`001-20261009-1a2b`); none lists the live runs
    pub run: Option<String>,
    /// One JSON object: `{"schema":1,"factory":…,"runs":[…]}`, each run with its line, what the owner owes and its board
    #[arg(long, conflicts_with = "line")]
    pub json: bool,
    /// One short line per run, and nothing when there is none
    #[arg(long)]
    pub line: bool,
    /// Only the live runs of the git repository this directory is in
    #[arg(long, conflicts_with = "run")]
    pub here: bool,
}

/// The `schema` of `--json`; bumped when a field changes meaning or goes away.
const SCHEMA: u32 = 1;

#[derive(Serialize)]
struct Envelope {
    schema: u32,
    factory: &'static str,
    runs: Vec<RunJson>,
}

#[derive(Serialize)]
struct RunJson {
    id: String,
    feature: String,
    phase: &'static str,
    line: String,
    owner: Vec<String>,
    board: Vec<String>,
}

pub fn status(args: &StatusArgs, out: &mut dyn Write, err: &mut dyn Write) -> Result<i32> {
    let data_dir = paths::data_dir();
    let plain = !args.json && !args.line;
    let runs = match &args.run {
        Some(id) if plain => {
            let Some(run) = cockpit::resolve(&data_dir, id, out)? else {
                return Ok(1);
            };
            vec![run]
        }
        Some(id) => {
            if !Run::runs_dir(&data_dir)
                .join(id)
                .join(crate::run::STATE_FILE)
                .is_file()
            {
                writeln!(err, "no run {id}").map_err(io("stderr"))?;
                return Ok(1);
            }
            vec![Run::open(&data_dir, id)?]
        }
        None => {
            let mut live = cockpit::live_runs(&data_dir)?;
            if args.here {
                let here = paths::repo_root(Path::new(".")).ok();
                live.retain(|s| here.as_deref().is_some_and(|h| same_repo(&s.repo, h)));
            }
            if plain && args.here && live.is_empty() {
                writeln!(out, "no live runs in this repository").map_err(io("stdout"))?;
                return Ok(0);
            }
            if plain {
                cockpit::print_live(&data_dir, &live, out)?;
                return Ok(0);
            }
            live.iter()
                .map(|s| Run::open(&data_dir, s.id.as_str()))
                .collect::<Result<_>>()?
        }
    };
    if args.json {
        let runs = runs.iter().map(run_json).collect::<Result<_>>()?;
        let env = Envelope {
            schema: SCHEMA,
            factory: env!("CARGO_PKG_VERSION"),
            runs,
        };
        let text = serde_json::to_string(&env).map_err(json("serialize status"))?;
        writeln!(out, "{text}").map_err(io("stdout"))?;
    } else if args.line {
        for run in &runs {
            writeln!(out, "{}", line_of(run.state(), &run.events()?)).map_err(io("stdout"))?;
        }
    } else {
        for l in board(&runs[0])?.iter().take(10) {
            writeln!(out, "{l}").map_err(io("stdout"))?;
        }
    }
    if args.run.is_some() {
        let run = &runs[0];
        if let Some(note) = events::skip_note(run.dir(), &events::scan(run.dir())?.1) {
            writeln!(err, "{note}").map_err(io("stderr"))?;
        }
    }
    Ok(0)
}

fn same_repo(repo: &Path, top: &Path) -> bool {
    repo.canonicalize().is_ok_and(|r| r == top)
}

fn run_json(run: &Run) -> Result<RunJson> {
    let s = run.state();
    let events = run.events()?;
    Ok(RunJson {
        id: s.id.to_string(),
        feature: s.feature.clone(),
        phase: s.phase.as_str(),
        line: line_of(s, &events),
        owner: cockpit::owner(s, &events),
        board: board(run)?.into_iter().take(10).collect(),
    })
}

/// What the glance line is drawn from; the pause is already local `HH:MM`.
#[derive(Debug, Clone)]
pub struct Glance<'a> {
    pub feature: &'a str,
    pub phase: Phase,
    pub open: &'a [String],
    pub parked: Option<&'a str>,
    pub paused_until: Option<&'a str>,
    pub driver_dead: bool,
    pub slices: Option<(usize, usize)>,
    pub elapsed: u64,
}

/// The first state that applies; a run that is over shows only its progress.
pub fn glance(g: &Glance) -> String {
    let f = g.feature;
    if !g.phase.is_over() {
        if !g.open.is_empty() {
            return format!("{f} · {} open", g.open.join(", "));
        }
        if let Some(reason) = g.parked {
            return format!("{f} · parked: {reason}");
        }
        if let Some(at) = g.paused_until {
            return format!("{f} · paused until {at}");
        }
        if g.driver_dead {
            return format!("{f} · driver dead");
        }
    }
    let done = g
        .slices
        .map_or(String::new(), |(n, m)| format!(" · {n}/{m} done"));
    format!("{f} · {}{done} · {}", g.phase, short_elapsed(g.elapsed))
}

fn line_of(s: &RunState, events: &[Event]) -> String {
    let open = cockpit::open_questions(events);
    let parked = cockpit::park(s, events).and_then(|e| match &e.kind {
        EventKind::RunParked { reason, .. } => Some(reason.as_str()),
        _ => None,
    });
    let now = clock::unix_now();
    let paused = s.paused_until.filter(|u| *u > now).map(clock::local_hhmm);
    let slices = Docs::open(&s.repo)
        .and_then(|d| d.status(&s.feature))
        .ok()
        .map(|b| {
            let done = b
                .slices
                .iter()
                .filter(|x| x.status.as_deref() == Some("done"))
                .count();
            (done, b.slices.len())
        });
    glance(&Glance {
        feature: &s.feature,
        phase: s.phase,
        open: &open,
        parked,
        paused_until: paused.as_deref(),
        driver_dead: cockpit::driver_dead(s, events),
        slices,
        elapsed: span(s),
    })
}

/// Start to now, or to the last update for a run that is over.
fn span(s: &RunState) -> u64 {
    let started = clock::from_rfc3339(&s.started_at).unwrap_or_else(clock::unix_now);
    let end = if s.phase.is_over() {
        clock::from_rfc3339(&s.updated_at).unwrap_or_else(clock::unix_now)
    } else {
        clock::unix_now()
    };
    end.saturating_sub(started)
}

fn short_elapsed(secs: u64) -> String {
    let (h, m) = (secs / 3600, secs % 3600 / 60);
    if h > 0 {
        format!("{h}h {m:02}m")
    } else {
        format!("{m}m")
    }
}

pub fn board(run: &Run) -> Result<Vec<String>> {
    let s = run.state();
    let events = run.events()?;
    let mut lines = Vec::with_capacity(10);

    lines.push(format!(
        "run {} · {}{} · {} elapsed",
        s.id,
        s.phase,
        if s.stopped { " (stopped)" } else { "" },
        elapsed(span(s))
    ));

    match Docs::open(&s.repo).and_then(|d| d.status(&s.feature)) {
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
    // Rounded up, so `used > budget` reads as such (1.4 of 1 is `2/1`, not `1/1`).
    let used = quota::weighted_sum(run.db(), run.id().as_str())?.ceil() as u64;
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

fn last_rate_limit(run: &Run) -> Option<RateLimit> {
    run.state()
        .workers
        .iter()
        .rev()
        .take(5)
        .find_map(|w| worker::summary_from_disk(run.dir(), &w.session_id).rate_limit)
}

fn last_three(events: &[Event]) -> String {
    let known: Vec<&Event> = events
        .iter()
        .filter(|e| e.kind != EventKind::Unknown)
        .collect();
    if known.is_empty() {
        return "none yet".to_string();
    }
    known
        .into_iter()
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
    fn the_glance_line_names_the_first_state_that_applies() {
        let base = Glance {
            feature: "013",
            phase: Phase::Building,
            open: &[],
            parked: None,
            paused_until: None,
            driver_dead: false,
            slices: Some((1, 3)),
            elapsed: 12 * 60 + 34,
        };
        assert_eq!(glance(&base), "013 · building · 1/3 done · 12m");
        let one = ["BLOCK-1".to_string()];
        let two = ["BLOCK-1".to_string(), "ASK-2".to_string()];
        assert_eq!(
            glance(&Glance {
                open: &one,
                ..base.clone()
            }),
            "013 · BLOCK-1 open",
            "an open block wins over the building phase"
        );
        assert_eq!(
            glance(&Glance {
                open: &two,
                driver_dead: true,
                ..base.clone()
            }),
            "013 · BLOCK-1, ASK-2 open"
        );
        assert_eq!(
            glance(&Glance {
                phase: Phase::WaitingOwner,
                parked: Some("budget"),
                ..base.clone()
            }),
            "013 · parked: budget"
        );
        assert_eq!(
            glance(&Glance {
                phase: Phase::Paused,
                paused_until: Some("14:30"),
                driver_dead: true,
                ..base.clone()
            }),
            "013 · paused until 14:30"
        );
        assert_eq!(
            glance(&Glance {
                driver_dead: true,
                ..base.clone()
            }),
            "013 · driver dead",
            "a dead driver wins over the building phase"
        );
        assert_eq!(
            glance(&Glance {
                phase: Phase::Finished,
                open: &one,
                driver_dead: true,
                slices: Some((3, 3)),
                elapsed: 3600 + 2 * 60 + 59,
                ..base.clone()
            }),
            "013 · finished · 3/3 done · 1h 02m",
            "a run that is over shows only its progress"
        );
        assert_eq!(
            glance(&Glance {
                phase: Phase::Checkpoint,
                slices: None,
                elapsed: 0,
                ..base.clone()
            }),
            "013 · checkpoint · 0m",
            "no board, no slice count"
        );

        let three = [
            "BLOCK-101".to_string(),
            "BLOCK-102".to_string(),
            "ASK-103".to_string(),
        ];
        let widest = [
            Glance {
                feature: "999",
                open: &three,
                ..base.clone()
            },
            Glance {
                feature: "999",
                phase: Phase::WaitingOwner,
                parked: Some("budget"),
                ..base.clone()
            },
            Glance {
                feature: "999",
                paused_until: Some("23:59"),
                ..base.clone()
            },
            Glance {
                feature: "999",
                driver_dead: true,
                ..base.clone()
            },
            Glance {
                feature: "999",
                phase: Phase::WaitingOwner,
                slices: Some((99, 99)),
                elapsed: 999 * 3600 + 59 * 60,
                ..base.clone()
            },
        ];
        for g in &widest {
            let l = glance(g);
            assert!(l.chars().count() <= 60, "{} chars: {l}", l.chars().count());
        }
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
            ev(5, EventKind::Unknown),
        ];
        assert_eq!(
            last_three(&events),
            "slice_done(03) → run_waiting → block_answered(01)",
            "a kind this binary does not know is passed over"
        );
        assert_eq!(last_three(&[]), "none yet");
        assert_eq!(last_three(&[ev(1, EventKind::Unknown)]), "none yet");
    }
}
