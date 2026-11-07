//! Slices and lanes come from the board, the truth; everything else from the run's own files.
use std::io::Write;
use std::path::{Path, PathBuf};

use clap::Args;
use serde::Serialize;

use crate::cockpit::{self, tokens};
use crate::docs::{Docs, Status, frontmatter};
use crate::error::{io, json};
use crate::events::{self, Event, EventKind};
use crate::run::{Phase, Run, RunState};
use crate::scheduler::pad_list;
use crate::stream::RateLimit;
use crate::{Result, checkpoint, clock, paths, quota, worker};

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
    /// With `--json`, also the runs that ended in the last 7 days
    #[arg(long, requires = "json", conflicts_with = "run")]
    pub recent: bool,
}

/// The `schema` of `--json`; bumped when a field changes meaning or goes away.
const SCHEMA: u32 = 1;

/// How far back `--recent` reaches for runs that are over.
const RECENT_SECS: u64 = 7 * 86_400;

#[derive(Serialize)]
struct Envelope {
    schema: u32,
    factory: &'static str,
    runs: Vec<RunJson>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RunJson {
    id: String,
    feature: String,
    phase: &'static str,
    line: String,
    owner: Vec<String>,
    questions: Vec<QuestionJson>,
    board: Vec<String>,
    repo: String,
    branch: Option<String>,
    slug: Option<String>,
    title: Option<String>,
    started_at: String,
    updated_at: String,
    elapsed_seconds: u64,
    seq: u64,
    slices: Option<Vec<SliceJson>>,
    budget: BudgetJson,
    checkpoints: CheckpointsJson,
    pr: Option<String>,
    ci: Option<&'static str>,
    paused_until: Option<String>,
    parked: Option<String>,
    driver_dead: bool,
    stopped: bool,
}

#[derive(Serialize, Debug, PartialEq)]
struct QuestionJson {
    id: String,
    at: &'static str,
    n: Option<u32>,
    text: String,
    answer: Vec<String>,
}

#[derive(Serialize)]
struct SliceJson {
    n: u32,
    status: String,
}

#[derive(Serialize)]
struct BudgetJson {
    used: u64,
    limit: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CheckpointsJson {
    done: usize,
    last_gate: Option<&'static str>,
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
            let mut live = if args.recent {
                recent_runs(&data_dir)?
            } else {
                cockpit::live_runs(&data_dir)?
            };
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

/// The live runs and the ones that ended within `RECENT_SECS`, oldest start first.
fn recent_runs(data_dir: &Path) -> Result<Vec<RunState>> {
    let since = clock::unix_now().saturating_sub(RECENT_SECS);
    Ok(crate::run::list_runs(data_dir)?
        .into_iter()
        .filter(|s| {
            !s.phase.is_over() || clock::from_rfc3339(&s.updated_at).is_some_and(|t| t >= since)
        })
        .collect())
}

fn same_repo(repo: &Path, top: &Path) -> bool {
    repo.canonicalize().is_ok_and(|r| r == top)
}

fn run_json(run: &Run) -> Result<RunJson> {
    let s = run.state();
    let events = run.events()?;
    let docs = Docs::open(&s.repo).and_then(|d| d.status(&s.feature)).ok();
    let folder = feature_folder(&s.repo, &s.feature, docs.as_ref());
    let title = folder.as_ref().and_then(|(_, dir)| prd_title(dir));
    let slices = docs.as_ref().map(|b| {
        let mut v: Vec<SliceJson> = b
            .slices
            .iter()
            .map(|x| SliceJson {
                n: x.number,
                status: x.status.clone().unwrap_or_else(|| "pending".to_string()),
            })
            .collect();
        v.sort_by_key(|x| x.n);
        v
    });
    let now = clock::unix_now();
    Ok(RunJson {
        id: s.id.to_string(),
        feature: s.feature.clone(),
        phase: s.phase.as_str(),
        line: line_of(s, &events),
        owner: cockpit::owner(s, &events),
        questions: questions(s, &events),
        board: board(run)?.into_iter().take(10).collect(),
        repo: s.repo.display().to_string(),
        branch: s.branch.clone(),
        slug: folder.map(|(slug, _)| slug),
        title,
        started_at: s.started_at.clone(),
        updated_at: s.updated_at.clone(),
        elapsed_seconds: span(s),
        seq: events.last().map_or(0, |e| e.seq),
        slices,
        budget: BudgetJson {
            used: budget_used(run)?,
            limit: s.budget_weighted_tokens,
        },
        checkpoints: CheckpointsJson {
            done: checkpoints_done(&events),
            last_gate: last_gate(&events).map(|g| if g.red { "red" } else { "green" }),
        },
        pr: s.pr_url.clone(),
        ci: ci(&events, s.pr_url.is_some(), s.phase.is_over()),
        paused_until: s.paused_until.filter(|u| *u > now).map(clock::rfc3339),
        parked: park_reason(s, &events).map(str::to_string),
        driver_dead: cockpit::driver_dead(s, &events),
        stopped: s.stopped,
    })
}

fn questions(s: &RunState, events: &[Event]) -> Vec<QuestionJson> {
    cockpit::questions(s, events)
        .into_iter()
        .filter_map(|e| question(e, &s.feature, s.id.as_str()))
        .collect()
}

/// `answer` is the argv to run from the run's repo, every flag before the id so the text can follow it.
fn question(e: &Event, feature: &str, run: &str) -> Option<QuestionJson> {
    let (id, at, n, text) = match &e.kind {
        EventKind::SliceBlocked {
            slice,
            block_id: Some(id),
            question,
        } => (
            id,
            "slice",
            Some(*slice),
            question.clone().unwrap_or_default(),
        ),
        EventKind::CheckpointBlocked {
            n,
            block_id,
            question,
        } => (block_id, "checkpoint", Some(*n), question.clone()),
        EventKind::CloseoutBlocked {
            block_id, question, ..
        } => (block_id, "closeout", None, question.clone()),
        EventKind::MergeRefused { block_id, detail } => {
            (block_id, "merge", None, checkpoint::refusal_summary(detail))
        }
        EventKind::AskOpened { id, text } => (id, "preflight", None, text.clone()),
        _ => return None,
    };
    let (flag, value, prefix) = if at == "preflight" {
        ("--run", run, "ASK-")
    } else {
        ("--feature", feature, "BLOCK-")
    };
    let number = id.strip_prefix(prefix)?;
    if number.is_empty() || !number.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some(QuestionJson {
        id: id.clone(),
        at,
        n,
        text,
        answer: ["factory", "answer", flag, value, id]
            .map(str::to_string)
            .to_vec(),
    })
}

/// The slug and folder of a feature, live before archived.
pub(crate) fn feature_folder(
    repo: &Path,
    feature: &str,
    docs: Option<&Status>,
) -> Option<(String, PathBuf)> {
    if let Some(b) = docs {
        let dir = repo.join(&b.dir);
        if dir.is_dir() {
            return Some((b.slug.clone(), dir));
        }
    }
    let prefix = format!("{feature}-");
    let found = ["docs/features", "docs/archive"].iter().find_map(|sub| {
        let found: Vec<(String, PathBuf)> = std::fs::read_dir(repo.join(sub))
            .ok()?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().into_owned();
                let slug = name.strip_prefix(&prefix)?.to_string();
                Some((slug, e.path()))
            })
            .collect();
        (!found.is_empty()).then_some(found)
    })?;
    let [one] = <[_; 1]>::try_from(found).ok()?;
    Some(one)
}

fn prd_title(dir: &Path) -> Option<String> {
    let text = std::fs::read_to_string(dir.join("prd.md")).ok()?;
    frontmatter::parse(&text).str("title").map(str::to_string)
}

/// Rounded up, so `used > budget` reads as such (1.4 of 1 is `2/1`, not `1/1`).
fn budget_used(run: &Run) -> Result<u64> {
    Ok(quota::weighted_sum(run.db(), run.id().as_str())?.ceil() as u64)
}

/// A run can end with no verdict on its last head, so a run that is over is never `pending`: that would never clear.
fn ci(events: &[Event], pr: bool, over: bool) -> Option<&'static str> {
    if !pr {
        return None;
    }
    let verdict = events.iter().rev().find_map(|e| match e.kind {
        EventKind::CiGreen { .. } => Some(Some("green")),
        EventKind::CiRed { .. } => Some(Some("red")),
        EventKind::CiRerun { .. } | EventKind::CiFixCommitted { .. } => Some(None),
        _ => None,
    });
    Some(match verdict.flatten() {
        Some(v) => v,
        None if over => "none",
        None => "pending",
    })
}

fn park_reason<'a>(s: &RunState, events: &'a [Event]) -> Option<&'a str> {
    cockpit::park(s, events).and_then(|e| match &e.kind {
        EventKind::RunParked { reason, .. } => Some(reason.as_str()),
        _ => None,
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
    let parked = park_reason(s, events);
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
    let used = budget_used(run)?;
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

struct LastGate {
    n: u32,
    attempt: u32,
    red: bool,
    minutes: Option<f64>,
}

fn checkpoints_done(events: &[Event]) -> usize {
    events
        .iter()
        .filter(|e| matches!(e.kind, EventKind::CheckpointDone { .. }))
        .count()
}

/// `minutes` is `None` for a cached gate.
fn last_gate(events: &[Event]) -> Option<LastGate> {
    events.iter().rev().find_map(|e| match &e.kind {
        EventKind::GateRun {
            n,
            attempt,
            red,
            minutes,
            ..
        } => Some(LastGate {
            n: *n,
            attempt: *attempt,
            red: *red,
            minutes: Some(*minutes),
        }),
        EventKind::GateCached {
            n, attempt, red, ..
        } => Some(LastGate {
            n: *n,
            attempt: *attempt,
            red: *red,
            minutes: None,
        }),
        _ => None,
    })
}

fn checkpoints_line(events: &[Event]) -> String {
    let gate = last_gate(events).map(|g| {
        format!(
            "{} (checkpoint {} attempt {}, {})",
            if g.red { "red" } else { "green" },
            g.n,
            g.attempt,
            g.minutes
                .map_or("cached".to_string(), |m| format!("{m:.2} min"))
        )
    });
    format!(
        "checkpoints: {} done · last gate: {}",
        checkpoints_done(events),
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
        .map(|e| e.kind.short())
        .collect::<Vec<_>>()
        .join(" → ")
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
    fn a_feature_folder_is_one_match_in_the_first_tree_that_has_any() {
        let repo = tempfile::tempdir().unwrap();
        let mkdir = |p: &str| std::fs::create_dir_all(repo.path().join(p)).unwrap();
        mkdir("docs/archive/007-old");
        let found = |f| feature_folder(repo.path(), f, None).map(|(slug, _)| slug);
        assert_eq!(found("007").as_deref(), Some("old"));
        mkdir("docs/features/007-new");
        assert_eq!(found("007").as_deref(), Some("new"));
        mkdir("docs/features/007-other");
        assert_eq!(
            found("007"),
            None,
            "two live folders never fall back to the archive"
        );
        assert_eq!(found("008"), None);
    }

    #[test]
    fn ci_reads_the_latest_verdict_and_a_missing_one_by_whether_the_run_is_over() {
        let ev = |seq, kind| Event {
            seq,
            ts: String::new(),
            kind,
        };
        let red = ev(
            1,
            EventKind::CiRed {
                check: "test".into(),
                round: 0,
                head: "a".into(),
            },
        );
        let fixed = ev(
            2,
            EventKind::CiFixCommitted {
                round: 1,
                sha: "b".into(),
                files: vec![],
            },
        );
        let green = ev(3, EventKind::CiGreen { head: "b".into() });
        assert_eq!(ci(&[red.clone(), green.clone()], false, false), None);
        assert_eq!(ci(std::slice::from_ref(&red), true, false), Some("red"));
        assert_eq!(
            ci(&[red.clone(), fixed.clone()], true, false),
            Some("pending")
        );
        assert_eq!(
            ci(&[red.clone(), fixed.clone()], true, true),
            Some("none"),
            "a run that ended on a fix with no verdict is not left pending"
        );
        assert_eq!(ci(&[red, fixed, green], true, true), Some("green"));
        assert_eq!(
            ci(&[ev(1, EventKind::RunStarted)], true, false),
            Some("pending")
        );
        assert_eq!(ci(&[], true, true), Some("none"));
    }

    #[test]
    fn questions_come_apart_from_their_latest_opening_with_the_argv_that_answers_them() {
        let ev = |seq, kind| Event {
            seq,
            ts: String::new(),
            kind,
        };
        let state = |phase: &str| -> RunState {
            serde_json::from_value(serde_json::json!({
                "id": "007-20261106-abcd",
                "repo": "/r",
                "feature": "007",
                "branch": null,
                "phase": phase,
                "pid": 0,
                "started_at": "2026-11-06T00:00:00Z",
                "updated_at": "2026-11-06T00:00:00Z",
                "budget_weighted_tokens": 1,
                "stop_requested": false,
            }))
            .unwrap()
        };
        let events = vec![
            ev(
                1,
                EventKind::SliceBlocked {
                    slice: 2,
                    block_id: Some("BLOCK-1".into()),
                    question: Some("Which store?\nA or B".into()),
                },
            ),
            ev(
                2,
                EventKind::CheckpointBlocked {
                    n: 1,
                    block_id: "BLOCK-2".into(),
                    question: "Gate red".into(),
                },
            ),
            ev(
                3,
                EventKind::MergeRefused {
                    block_id: "BLOCK-3".into(),
                    detail: "docs log merge\n✗ AD-1 twice\n✗ AD-2 twice".into(),
                },
            ),
            ev(
                4,
                EventKind::CloseoutBlocked {
                    phase: "pr".into(),
                    block_id: "BLOCK-4".into(),
                    question: "No PR body".into(),
                },
            ),
            ev(
                5,
                EventKind::AskOpened {
                    id: "ASK-1".into(),
                    text: "no inner loop declared".into(),
                },
            ),
            ev(
                6,
                EventKind::SliceBlocked {
                    slice: 2,
                    block_id: Some("BLOCK-1".into()),
                    question: None,
                },
            ),
            ev(
                7,
                EventKind::BlockAnswered {
                    id: "BLOCK-2".into(),
                    slice: None,
                    text: "fix it".into(),
                },
            ),
        ];
        let q = |id: &str, at, n, text: &str, flag, value| QuestionJson {
            id: id.into(),
            at,
            n,
            text: text.into(),
            answer: ["factory", "answer", flag, value, id]
                .map(str::to_string)
                .to_vec(),
        };
        let feature = |id, at, n, text| q(id, at, n, text, "--feature", "007");
        assert_eq!(
            questions(&state("waiting-owner"), &events),
            [
                feature("BLOCK-3", "merge", None, "AD-1 twice; AD-2 twice"),
                feature("BLOCK-4", "closeout", None, "No PR body"),
                q(
                    "ASK-1",
                    "preflight",
                    None,
                    "no inner loop declared",
                    "--run",
                    "007-20261106-abcd"
                ),
                feature("BLOCK-1", "slice", Some(2), ""),
            ],
            "the re-opened block moves to its latest opening and the answered one is gone"
        );
        assert_eq!(
            questions(&state("building"), &events[..2]),
            [
                feature("BLOCK-1", "slice", Some(2), "Which store?\nA or B"),
                feature("BLOCK-2", "checkpoint", Some(1), "Gate red"),
            ],
            "the text is whole"
        );
        assert_eq!(questions(&state("finished"), &events), []);
        let crafted = ["--run=X-1", "BLOCK-", "BLOCK-1x", "ASK-1"].map(|id| {
            ev(
                1,
                EventKind::SliceBlocked {
                    slice: 1,
                    block_id: Some(id.into()),
                    question: None,
                },
            )
        });
        for e in &crafted {
            assert_eq!(
                questions(&state("building"), std::slice::from_ref(e)),
                [],
                "an id the answer command cannot take as one never reaches its argv: {e:?}"
            );
        }
        let json = serde_json::to_value(questions(&state("building"), &events[..1])).unwrap();
        assert_eq!(
            json,
            serde_json::json!([{
                "id": "BLOCK-1",
                "at": "slice",
                "n": 2,
                "text": "Which store?\nA or B",
                "answer": ["factory", "answer", "--feature", "007", "BLOCK-1"],
            }])
        );
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
                    reason: "slice 01 blocked".into(),
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
            "slice_done(03) → run_waiting(slice 01 blocked) → block_answered(01)",
            "a kind this binary does not know is passed over"
        );
        assert_eq!(last_three(&[]), "none yet");
        assert_eq!(last_three(&[ev(1, EventKind::Unknown)]), "none yet");
    }
}
