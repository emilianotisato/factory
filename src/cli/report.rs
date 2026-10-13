//! `factory report <run>` (PRD §10 FR-8): the twelve retro keys the old
//! framework's `retro.md` expects, then the eight run keys FR-11 compares,
//! one `key: value` line each, or one JSON object with `--json`. Every
//! number is a `SELECT` over what the driver wrote as it ran; nothing is
//! re-parsed from a transcript. `--cockpit <session.jsonl>...` adds the
//! cockpit's own cost from its transcript (`measure`). `score` is the
//! owner's and prints empty (§13).
//!
//! Slice 13 (§14 FR-12 decision 4) appends `followup_reads` and
//! `pack_doc_read_chars`, turns `ctx_share` into the share of lookups that
//! were `ctx` calls (the byte share slice 08 printed stays as
//! `lookup_share_chars`), prints the three lines the reversal rule reads
//! after the keys, and `--per-worker` adds `ctx_share` and `repeat_rate` for
//! each worker.
//!
//! `followup_reads_by_role` splits `followup_reads` by worker role: the
//! icg-cloud A/B mixed a polish worker's follow-ups with the builders', and
//! the reversal rule is read on the builders' share (retro FR-11, 2026-10-12).
//!
//! PRD-003 FR-24 appends the quality counts, no composite score: fix
//! commits, CI red rounds and reruns from the events, the polish `FINDINGS`
//! and the agent log's counts from the `runs` row, and prints them again on
//! one `quality:` line beside `gate_red` and `worker_gate_red`.
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::PathBuf;

use clap::Args;
use serde::Serialize;
use serde_json::Value;

use crate::db::Db;
use crate::db::{ToolCallRow, WorkerRow};
use crate::docs::Docs;
use crate::error::{io, json};
use crate::measure::{self, Measure, Subagents};
use crate::metrics::{self, CTX_KINDS, LOOKUP_KINDS};
use crate::pack::Part;
use crate::run::{GateCmds, Run};
use crate::stream::Findings;
use crate::{Error, Result, paths, quota, stream, worker};

#[derive(Args, Debug, Clone)]
pub struct ReportArgs {
    /// The run id (`001-20261009-1a2b`)
    pub run: String,
    /// One JSON object instead of `key: value` lines
    #[arg(long)]
    pub json: bool,
    /// The cockpit session transcript(s), for `cockpit_tokens`
    #[arg(long, value_name = "SESSION.jsonl", num_args = 1..)]
    pub cockpit: Vec<PathBuf>,
    /// `ctx_share` and `repeat_rate` for each worker, after the run's keys
    #[arg(long)]
    pub per_worker: bool,
    /// Rewrite the run's token, context and `tool_calls` rows from the
    /// stored streams and the transcript dirs first (a finished run only)
    #[arg(long)]
    pub recount: bool,
}

/// The retro keys (`retro.md:2-14`), in order, then the run keys (§10).
pub const KEYS: [&str; 23] = [
    "score",
    "gate_runs",
    "gate_red",
    "gate_minutes",
    "ci_cycles",
    "slices",
    "checkpoints",
    "build_minutes",
    "polish_minutes",
    "out_tokens",
    "cache_read_tokens",
    "start_context",
    "peak_context",
    "input_tokens",
    "cache_write_tokens",
    "weighted_tokens",
    "cockpit_tokens",
    "workers",
    "lookup_chars",
    "ctx_share",
    "repeat_rate",
    "followup_reads",
    "pack_doc_read_chars",
];

/// The keys after [`KEYS`]: what the slices' task texts name beyond §10's
/// list (AD-802; `lookup_share_chars`, slice 13). The three `cockpit_*`
/// figures after `cockpit_peak_context` let the FR-8 cross-check compare all
/// four of the QA guide's figures with `measure.py`, not just the peak.
pub const EXTRA_KEYS: [&str; 19] = [
    "worker_gate_runs",
    "worker_gate_red",
    "cockpit_peak_context",
    "cockpit_out_tokens",
    "cockpit_cache_read_tokens",
    "cockpit_start_context",
    "lookup_share_chars",
    "followup_reads_by_role",
    "weighted_by_role",
    "fix_commits",
    "ci_red_rounds",
    "ci_reruns",
    "findings",
    "decisions",
    "decide",
    "blocks",
    "knobs",
    "hypothesis",
    "pack_by_role",
];

// The reversal rule's constants, PRD §14 FR-12 decision 4: "`followup_reads`
// above 30% of `ctx` calls after slice 3 means the answers are too thin";
// "If, after slice 3, `ctx_share` stays under 10% of worker lookups or
// `repeat_rate` does not fall from 26.7%, `ctx` shrinks to `outline` only".
// The report prints the comparison; the reversal is the owner's call.
/// `followup_reads` share above which the `ctx` answers are too thin.
pub const FOLLOWUP_THRESHOLD: f64 = 0.30;
/// `ctx_share` under which `ctx` is not taken up.
pub const CTX_SHARE_THRESHOLD: f64 = 0.10;
/// The `repeat_rate` baseline from the owner's transcripts (§20).
pub const REPEAT_RATE_BASELINE: f64 = 0.267;

/// `followup_reads`: the `ctx` calls a read of a file they named followed
/// within two calls (`label_followup`), of all `ctx` calls.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Followups {
    pub count: u64,
    pub ctx_calls: u64,
    pub share: f64,
}

impl Followups {
    /// The `ctx` calls among `calls` and how many a read followed.
    fn of<'a>(calls: impl Iterator<Item = &'a ToolCallRow>) -> Self {
        let (mut count, mut ctx_calls) = (0, 0);
        for c in calls.filter(|c| CTX_KINDS.contains(&c.kind.as_str())) {
            ctx_calls += 1;
            count += u64::from(c.label_followup == Some(true));
        }
        Followups {
            count,
            ctx_calls,
            share: ratio(count, ctx_calls),
        }
    }

    /// `1 of 2 ctx calls (50%)`.
    fn text(&self) -> String {
        format!(
            "{} of {} ctx calls ({}%)",
            self.count,
            self.ctx_calls,
            (self.share * 100.0).round()
        )
    }
}

/// The three lines the reversal rule reads (§14 FR-12 decision 4). A share
/// with nothing under it reads `n/a`, not a pass: no ctx call for the
/// follow-up share, no lookup for `ctx_share` and `repeat_rate`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Thresholds {
    /// `ok`, `over` or `n/a`.
    pub followup: &'static str,
    /// `ok`, `under` or `n/a`.
    pub ctx_share: &'static str,
    /// `lower`, `not lower` or `n/a`.
    pub repeat_rate: &'static str,
}

impl Default for Thresholds {
    fn default() -> Self {
        Thresholds::of(None, None, None)
    }
}

/// What a threshold line says with no data under it.
const NO_DATA: &str = "n/a";

impl Thresholds {
    pub fn of(
        followup_share: Option<f64>,
        ctx_share: Option<f64>,
        repeat_rate: Option<f64>,
    ) -> Self {
        let read = |v: Option<f64>, past: fn(f64) -> bool, yes, no| {
            v.map_or(NO_DATA, |v| if past(v) { yes } else { no })
        };
        Thresholds {
            followup: read(followup_share, |v| v > FOLLOWUP_THRESHOLD, "over", "ok"),
            ctx_share: read(ctx_share, |v| v < CTX_SHARE_THRESHOLD, "under", "ok"),
            repeat_rate: read(
                repeat_rate,
                |v| v < REPEAT_RATE_BASELINE,
                "lower",
                "not lower",
            ),
        }
    }

    pub fn lines(&self) -> Vec<String> {
        vec![
            format!("followup threshold 30%: {}", self.followup),
            format!("ctx_share threshold 10%: {}", self.ctx_share),
            format!("repeat_rate vs 26.7%: {}", self.repeat_rate),
        ]
    }
}

/// One worker's lookups, for `--per-worker`.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct PerWorker {
    pub id: i64,
    pub role: String,
    pub slice: Option<u32>,
    pub session_id: Option<String>,
    pub lookups: u64,
    pub ctx_calls: u64,
    pub ctx_share: f64,
    pub repeat_rate: f64,
    /// The worker's own usage, weighted (PRD-003 FR-21).
    pub weighted_own: u64,
    /// Its subagents', weighted; `None` when they were not read.
    pub weighted_sub: Option<u64>,
    pub subagents: Option<i64>,
    /// The model that answered (PRD-003 FR-23); `None` on an older row.
    pub model_id: Option<String>,
}

/// `—` for a figure that was not read.
fn or_dash<T: std::fmt::Display>(v: Option<T>) -> String {
    v.map_or_else(|| "—".to_string(), |n| n.to_string())
}

impl PerWorker {
    /// `worker 3 build slice 02 (1a2b3c4d): lookups 6, ctx_share 0.333,
    /// repeat_rate 0.167, own 812000 sub 0 subagents 0, model_id claude-opus-5-5`.
    fn text(&self) -> String {
        let slice = self
            .slice
            .map_or(String::new(), |n| format!(" slice {n:02}"));
        let session = self.session_id.as_deref().map_or(String::new(), |s| {
            format!(" ({})", s.chars().take(8).collect::<String>())
        });
        format!(
            "worker {} {}{slice}{session}: lookups {}, ctx_share {:.3}, repeat_rate {:.3}, own {} sub {} subagents {}, model_id {}",
            self.id,
            self.role,
            self.lookups,
            self.ctx_share,
            self.repeat_rate,
            self.weighted_own,
            or_dash(self.weighted_sub),
            or_dash(self.subagents),
            or_dash(self.model_id.as_deref())
        )
    }
}

/// The row's subagent columns were never read (an unreadable dir, a lost
/// transcript, a row from before PRD-003).
fn sub_unread(w: &WorkerRow) -> bool {
    [
        w.sub_input,
        w.sub_cache_write,
        w.sub_cache_read,
        w.sub_output,
    ]
    .iter()
    .all(Option::is_none)
}

/// `(lookups, ctx calls, repeats)` among `calls`.
fn lookup_counts<'a>(calls: impl Iterator<Item = &'a ToolCallRow>) -> (u64, u64, u64) {
    let mut out = (0, 0, 0);
    for c in calls.filter(|c| LOOKUP_KINDS.contains(&c.kind.as_str())) {
        out.0 += 1;
        out.1 += u64::from(CTX_KINDS.contains(&c.kind.as_str()));
        out.2 += u64::from(c.repeat_in_worker);
    }
    out
}

/// `ctx_share` and `repeat_rate` of each worker, in spawn order.
pub fn per_worker(workers: &[WorkerRow], calls: &[ToolCallRow]) -> Vec<PerWorker> {
    workers
        .iter()
        .map(|w| {
            let (lookups, ctx, repeats) =
                lookup_counts(calls.iter().filter(|c| c.worker_id == w.id));
            PerWorker {
                id: w.id,
                role: w.role.clone(),
                slice: w.slice,
                session_id: w.session_id.clone(),
                lookups,
                ctx_calls: ctx,
                ctx_share: ratio(ctx, lookups),
                repeat_rate: ratio(repeats, lookups),
                weighted_own: quota::weigh_own(w).round() as u64,
                weighted_sub: (!sub_unread(w)).then(|| quota::weigh_sub(w).round() as u64),
                subagents: w.subagents,
                model_id: w.model_id.clone(),
            }
        })
        .collect()
}

/// `followup_reads` of each role that made a `ctx` call.
pub fn followups_by_role(
    workers: &[WorkerRow],
    calls: &[ToolCallRow],
) -> BTreeMap<String, Followups> {
    let role: BTreeMap<i64, &str> = workers.iter().map(|w| (w.id, w.role.as_str())).collect();
    let mut roles: Vec<&str> = role.values().copied().collect();
    roles.sort_unstable();
    roles.dedup();
    roles
        .into_iter()
        .map(|r| {
            let mine = calls
                .iter()
                .filter(|c| role.get(&c.worker_id).copied() == Some(r));
            (r.to_string(), Followups::of(mine))
        })
        .filter(|(_, f)| f.ctx_calls > 0)
        .collect()
}

/// `build 1 of 2 (50%), polish 5 of 14 (36%)`, roles in [`ROLE_ORDER`].
fn by_role_text(by_role: &BTreeMap<String, Followups>) -> String {
    let known = ROLE_ORDER.iter().filter_map(|r| by_role.get_key_value(*r));
    let other = by_role
        .iter()
        .filter(|(r, _)| !ROLE_ORDER.contains(&r.as_str()));
    let parts: Vec<String> = known
        .chain(other)
        .map(|(r, f)| {
            format!(
                "{r} {} of {} ({}%)",
                f.count,
                f.ctx_calls,
                (f.share * 100.0).round()
            )
        })
        .collect();
    if parts.is_empty() {
        "none".to_string()
    } else {
        parts.join(", ")
    }
}

/// A role's weighted tokens: the workers' own, and their subagents'
/// (PRD-003 FR-21).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct OwnSub {
    pub own: u64,
    pub sub: u64,
}

/// Weighted tokens per role, own and subagents apart.
pub fn weighted_by_role(workers: &[WorkerRow]) -> BTreeMap<String, OwnSub> {
    let mut sums: BTreeMap<String, (f64, f64)> = BTreeMap::new();
    for w in workers {
        let e = sums.entry(w.role.clone()).or_default();
        e.0 += quota::weigh_own(w);
        e.1 += quota::weigh_sub(w);
    }
    sums.into_iter()
        .map(|(r, (own, sub))| {
            (
                r,
                OwnSub {
                    own: own.round() as u64,
                    sub: sub.round() as u64,
                },
            )
        })
        .collect()
}

/// `build 9950000+0 · polish 1582003+1859987`, roles in [`ROLE_ORDER`].
fn weighted_by_role_text(by_role: &BTreeMap<String, OwnSub>) -> String {
    let known = ROLE_ORDER.iter().filter_map(|r| by_role.get_key_value(*r));
    let other = by_role
        .iter()
        .filter(|(r, _)| !ROLE_ORDER.contains(&r.as_str()));
    let parts: Vec<String> = known
        .chain(other)
        .map(|(r, w)| format!("{r} {}+{}", w.own, w.sub))
        .collect();
    if parts.is_empty() {
        "none".to_string()
    } else {
        parts.join(" · ")
    }
}

/// One role's packs (PRD-003 FR-22), means over its rows that recorded
/// `pack_parts`: each section's chars in the order the packs wrote them (a
/// row without the section counts 0, so the sections add up to the total),
/// the pack total, and `start_context` beside it, of which the pack is a share.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct PackShare {
    #[serde(serialize_with = "in_order")]
    pub sections: Vec<(String, u64)>,
    pub total: u64,
    pub start_context: Option<u64>,
}

/// The sections as a JSON object, keys in pack order.
fn in_order<S: serde::Serializer>(
    sections: &[(String, u64)],
    s: S,
) -> std::result::Result<S::Ok, S::Error> {
    s.collect_map(sections.iter().map(|(k, n)| (k, n)))
}

/// One worker's pack sections and its `start_context`.
type PackRow = (Vec<Part>, Option<i64>);

/// [`PackShare`] per role; `None` for a role with no `pack_parts` row (a run
/// from before FR-22).
pub fn pack_by_role(workers: &[WorkerRow]) -> BTreeMap<String, Option<PackShare>> {
    let mut rows: BTreeMap<String, Vec<PackRow>> = BTreeMap::new();
    for w in workers {
        let role = rows.entry(w.role.clone()).or_default();
        if let Some(parts) = w
            .pack_parts
            .as_deref()
            .and_then(|t| serde_json::from_str::<Vec<Part>>(t).ok())
        {
            role.push((parts, w.start_context));
        }
    }
    let mean = |sum: u64, n: usize| (sum as f64 / n as f64).round() as u64;
    rows.into_iter()
        .map(|(role, rows)| {
            if rows.is_empty() {
                return (role, None);
            }
            let mut sums: Vec<(String, u64)> = Vec::new();
            for p in rows.iter().flat_map(|(parts, _)| parts) {
                match sums.iter_mut().find(|(s, _)| *s == p.section) {
                    Some(s) => s.1 += p.chars as u64,
                    None => sums.push((p.section.clone(), p.chars as u64)),
                }
            }
            let starts: Vec<u64> = rows.iter().filter_map(|r| r.1).map(|n| n as u64).collect();
            let share = PackShare {
                total: mean(sums.iter().map(|s| s.1).sum(), rows.len()),
                sections: sums
                    .into_iter()
                    .map(|(s, sum)| (s, mean(sum, rows.len())))
                    .collect(),
                start_context: (!starts.is_empty())
                    .then(|| mean(starts.iter().sum(), starts.len())),
            };
            (role, Some(share))
        })
        .collect()
}

/// `build slice 9120 outline 7800 … total 28400 start_context 45000 ·
/// polish —`, roles in [`ROLE_ORDER`].
fn pack_by_role_text(by_role: &BTreeMap<String, Option<PackShare>>) -> String {
    let known = ROLE_ORDER.iter().filter_map(|r| by_role.get_key_value(*r));
    let other = by_role
        .iter()
        .filter(|(r, _)| !ROLE_ORDER.contains(&r.as_str()));
    let parts: Vec<String> = known
        .chain(other)
        .map(|(r, share)| match share {
            None => format!("{r} —"),
            Some(p) => {
                let sections: Vec<String> =
                    p.sections.iter().map(|(s, n)| format!("{s} {n}")).collect();
                format!(
                    "{r} {} total {} start_context {}",
                    sections.join(" "),
                    p.total,
                    or_dash(p.start_context)
                )
            }
        })
        .collect();
    if parts.is_empty() {
        "none".to_string()
    } else {
        parts.join(" · ")
    }
}

/// The worker's own calls: the lookup figures leave its subagents' out
/// (PRD-003 FR-21), though the table keeps them.
fn own_calls(db: &Db, run_id: &str) -> Result<Vec<ToolCallRow>> {
    Ok(db
        .tool_calls(run_id)?
        .into_iter()
        .filter(|c| !c.in_subagent)
        .collect())
}

/// Worker rows by role.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Workers {
    pub total: u64,
    #[serde(flatten)]
    pub by_role: BTreeMap<String, u64>,
}

const ROLE_ORDER: [&str; 4] = ["build", "fix", "checkpoint", "polish"];

impl Workers {
    /// `5 (build 3, fix 1, checkpoint 1)`.
    fn text(&self) -> String {
        let mut parts: Vec<String> = ROLE_ORDER
            .iter()
            .filter_map(|r| self.by_role.get(*r).map(|n| format!("{r} {n}")))
            .collect();
        for (r, n) in &self.by_role {
            if !ROLE_ORDER.contains(&r.as_str()) {
                parts.push(format!("{r} {n}"));
            }
        }
        if parts.is_empty() {
            format!("{} (none)", self.total)
        } else {
            format!("{} ({})", self.total, parts.join(", "))
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Report {
    /// The owner's; never computed.
    pub score: Option<f64>,
    pub gate_runs: u64,
    pub gate_red: u64,
    pub gate_minutes: f64,
    /// CI fix rounds that landed a `fix: CI <check>` commit (FR-6).
    pub ci_cycles: u64,
    pub slices: u64,
    pub checkpoints: u64,
    pub build_minutes: f64,
    pub polish_minutes: f64,
    pub out_tokens: u64,
    pub cache_read_tokens: u64,
    pub start_context: u64,
    pub peak_context: u64,
    pub input_tokens: u64,
    pub cache_write_tokens: u64,
    pub weighted_tokens: u64,
    /// `None` without `--cockpit`.
    pub cockpit_tokens: Option<u64>,
    pub workers: Workers,
    pub lookup_chars: u64,
    /// `ctx-*` rows among lookup rows (§14 decision 4).
    pub ctx_share: f64,
    pub repeat_rate: f64,
    pub followup_reads: Followups,
    /// `result_chars` of `read`/`bash-cat` rows on a file the pack held (§4).
    pub pack_doc_read_chars: u64,
    pub worker_gate_runs: u64,
    pub worker_gate_red: u64,
    pub cockpit_peak_context: Option<u64>,
    pub cockpit_out_tokens: Option<u64>,
    pub cockpit_cache_read_tokens: Option<u64>,
    pub cockpit_start_context: Option<u64>,
    /// Lookup bytes among all result bytes: slice 08's `ctx_share`.
    pub lookup_share_chars: f64,
    /// Roles with no `ctx` call are left out.
    pub followup_reads_by_role: BTreeMap<String, Followups>,
    /// Weighted tokens per role, own and subagents apart.
    pub weighted_by_role: BTreeMap<String, OwnSub>,
    /// `fix_committed` events: checkpoint fix workers that landed a commit.
    pub fix_commits: u64,
    pub ci_red_rounds: u64,
    pub ci_reruns: u64,
    /// The polish `FINDINGS` line; `None` when it had none.
    pub findings: Option<Findings>,
    /// Stored at ship; `None` before it.
    pub decisions: Option<u32>,
    pub decide: Option<u32>,
    pub blocks: Option<u32>,
    /// What the run resolved at its start (PRD-003 FR-23), the stored
    /// object; `None` on a run from before it.
    pub knobs: Option<Value>,
    pub hypothesis: Option<String>,
    /// Per role, what its packs were made of (PRD-003 FR-22); `None` for a
    /// role none of whose rows recorded `pack_parts`.
    pub pack_by_role: BTreeMap<String, Option<PackShare>>,
    pub thresholds: Thresholds,
    /// With `--per-worker` only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub per_worker: Option<Vec<PerWorker>>,
}

fn round(x: f64, places: i32) -> f64 {
    let p = 10f64.powi(places);
    (x * p).round() / p
}

fn ratio(num: u64, den: u64) -> f64 {
    if den == 0 {
        0.0
    } else {
        round(num as f64 / den as f64, 3)
    }
}

/// The report of `run_id` from the tables, plus the cockpit measure when given.
pub fn compute(db: &Db, run_id: &str, cockpit: Option<&Measure>) -> Result<Report> {
    let workers = db.workers(run_id)?;
    let gates = db.gates(run_id)?;
    let calls = own_calls(db, run_id)?;
    let events = db.events(run_id)?;
    let row = db.run_row(run_id)?;
    let n = |v: Option<i64>| v.unwrap_or(0).max(0) as u64;
    let count = |kind: &str| events.iter().filter(|e| e.kind == kind).count() as u64;
    let minutes = |roles: &[&str]| {
        let ms: u64 = workers
            .iter()
            .filter(|w| roles.contains(&w.role.as_str()))
            .map(|w| n(w.duration_ms))
            .sum();
        round(ms as f64 / 60_000.0, 2)
    };

    let driver: Vec<_> = gates.iter().filter(|g| g.source == "driver").collect();
    let worker_gates: Vec<_> = gates.iter().filter(|g| g.source == "worker").collect();
    let mut by_role = BTreeMap::new();
    for w in &workers {
        *by_role.entry(w.role.clone()).or_insert(0) += 1;
    }
    let lookup: Vec<_> = calls
        .iter()
        .filter(|c| LOOKUP_KINDS.contains(&c.kind.as_str()))
        .collect();
    let lookup_chars: u64 = lookup.iter().map(|c| n(c.result_chars)).sum();
    let all_chars: u64 = calls.iter().map(|c| n(c.result_chars)).sum();
    let (lookups, ctx_calls, repeats) = lookup_counts(calls.iter());
    let followup_reads = Followups::of(calls.iter());
    let ctx_share = ratio(ctx_calls, lookups);
    let repeat_rate = ratio(repeats, lookups);

    Ok(Report {
        score: None,
        gate_runs: driver.len() as u64,
        gate_red: driver.iter().filter(|g| g.red).count() as u64,
        gate_minutes: round(driver.iter().map(|g| g.minutes).sum(), 2),
        ci_cycles: count("ci_fix_committed"),
        slices: count("slice_done"),
        checkpoints: count("checkpoint_done"),
        build_minutes: minutes(&["build", "fix"]),
        polish_minutes: minutes(&["polish"]),
        out_tokens: workers.iter().map(|w| n(w.output) + n(w.sub_output)).sum(),
        cache_read_tokens: workers
            .iter()
            .map(|w| n(w.cache_read) + n(w.sub_cache_read))
            .sum(),
        start_context: workers
            .iter()
            .filter_map(|w| w.start_context)
            .min()
            .map_or(0, |v| v.max(0) as u64),
        peak_context: workers
            .iter()
            .filter_map(|w| w.peak_context)
            .max()
            .map_or(0, |v| v.max(0) as u64),
        input_tokens: workers.iter().map(|w| n(w.input) + n(w.sub_input)).sum(),
        cache_write_tokens: workers
            .iter()
            .map(|w| n(w.cache_write) + n(w.sub_cache_write))
            .sum(),
        weighted_tokens: quota::weighted(db, run_id)?,
        cockpit_tokens: cockpit.map(Measure::weighted),
        workers: Workers {
            total: workers.len() as u64,
            by_role,
        },
        lookup_chars,
        ctx_share,
        repeat_rate,
        thresholds: Thresholds::of(
            (ctx_calls > 0).then_some(followup_reads.share),
            (lookups > 0).then_some(ctx_share),
            (lookups > 0).then_some(repeat_rate),
        ),
        followup_reads,
        pack_doc_read_chars: calls
            .iter()
            .filter(|c| c.pack_doc)
            .map(|c| n(c.result_chars))
            .sum(),
        worker_gate_runs: worker_gates.len() as u64,
        worker_gate_red: worker_gates.iter().filter(|g| g.red).count() as u64,
        cockpit_peak_context: cockpit.map(|m| m.peak_context),
        cockpit_out_tokens: cockpit.map(|m| m.out_tokens),
        cockpit_cache_read_tokens: cockpit.map(|m| m.cache_read_tokens),
        cockpit_start_context: cockpit.map(|m| m.start_context),
        lookup_share_chars: ratio(lookup_chars, all_chars),
        followup_reads_by_role: followups_by_role(&workers, &calls),
        weighted_by_role: weighted_by_role(&workers),
        fix_commits: count("fix_committed"),
        ci_red_rounds: count("ci_red"),
        ci_reruns: count("ci_rerun"),
        findings: row.as_ref().and_then(|r| r.findings),
        decisions: row.as_ref().and_then(|r| r.decisions),
        decide: row.as_ref().and_then(|r| r.decide),
        blocks: row.as_ref().and_then(|r| r.blocks),
        knobs: row
            .as_ref()
            .and_then(|r| r.knobs.as_deref())
            .and_then(|t| serde_json::from_str(t).ok()),
        hypothesis: row.as_ref().and_then(|r| r.hypothesis.clone()),
        pack_by_role: pack_by_role(&workers),
        per_worker: None,
    })
}

impl Report {
    /// The `key: value` lines, [`KEYS`] then [`EXTRA_KEYS`].
    pub fn lines(&self) -> Vec<String> {
        let absent = "0 (no --cockpit)".to_string();
        let opt = |v: Option<u64>| v.map_or_else(|| absent.clone(), |n| n.to_string());
        vec![
            "score: ".to_string(),
            format!("gate_runs: {}", self.gate_runs),
            format!("gate_red: {}", self.gate_red),
            format!("gate_minutes: {:.2}", self.gate_minutes),
            format!("ci_cycles: {}", self.ci_cycles),
            format!("slices: {}", self.slices),
            format!("checkpoints: {}", self.checkpoints),
            format!("build_minutes: {:.2}", self.build_minutes),
            format!("polish_minutes: {:.2}", self.polish_minutes),
            format!("out_tokens: {}", self.out_tokens),
            format!("cache_read_tokens: {}", self.cache_read_tokens),
            format!("start_context: {}", self.start_context),
            format!("peak_context: {}", self.peak_context),
            format!("input_tokens: {}", self.input_tokens),
            format!("cache_write_tokens: {}", self.cache_write_tokens),
            format!("weighted_tokens: {}", self.weighted_tokens),
            format!("cockpit_tokens: {}", opt(self.cockpit_tokens)),
            format!("workers: {}", self.workers.text()),
            format!("lookup_chars: {}", self.lookup_chars),
            format!("ctx_share: {:.3}", self.ctx_share),
            format!("repeat_rate: {:.3}", self.repeat_rate),
            format!("followup_reads: {}", self.followup_reads.text()),
            format!("pack_doc_read_chars: {}", self.pack_doc_read_chars),
            format!("worker_gate_runs: {}", self.worker_gate_runs),
            format!("worker_gate_red: {}", self.worker_gate_red),
            format!("cockpit_peak_context: {}", opt(self.cockpit_peak_context)),
            format!("cockpit_out_tokens: {}", opt(self.cockpit_out_tokens)),
            format!(
                "cockpit_cache_read_tokens: {}",
                opt(self.cockpit_cache_read_tokens)
            ),
            format!("cockpit_start_context: {}", opt(self.cockpit_start_context)),
            format!("lookup_share_chars: {:.3}", self.lookup_share_chars),
            format!(
                "followup_reads_by_role: {}",
                by_role_text(&self.followup_reads_by_role)
            ),
            format!(
                "weighted_by_role: {}",
                weighted_by_role_text(&self.weighted_by_role)
            ),
            format!("fix_commits: {}", self.fix_commits),
            format!("ci_red_rounds: {}", self.ci_red_rounds),
            format!("ci_reruns: {}", self.ci_reruns),
            format!("findings: {}", findings_text(self.findings)),
            format!("decisions: {}", dash(self.decisions)),
            format!("decide: {}", dash(self.decide)),
            format!("blocks: {}", dash(self.blocks)),
            format!("knobs: {}", knobs_text(self.knobs.as_ref())),
            format!("hypothesis: {}", or_dash(self.hypothesis.as_deref())),
            format!("pack_by_role: {}", pack_by_role_text(&self.pack_by_role)),
        ]
    }

    /// FR-24's quality block on one line; a null count prints `—`.
    pub fn quality_line(&self) -> String {
        let findings = match self.findings {
            Some(f) => format!("findings {}", f.text()),
            None => "findings: —".to_string(),
        };
        format!(
            "quality: gate_red {} · worker_gate_red {} · fix_commits {} · ci_red_rounds {} · ci_reruns {} · {findings} · decisions {} decide {} blocks {}",
            self.gate_red,
            self.worker_gate_red,
            self.fix_commits,
            self.ci_red_rounds,
            self.ci_reruns,
            dash(self.decisions),
            dash(self.decide),
            dash(self.blocks),
        )
    }
}

fn dash(v: Option<u32>) -> String {
    v.map_or_else(|| "—".to_string(), |n| n.to_string())
}

fn findings_text(f: Option<Findings>) -> String {
    f.map_or_else(|| "—".to_string(), |f| f.text())
}

/// `build opus/high default · … · polish sonnet/high project · budget
/// 30000000 user · lanes 1 · factory 0.1.0 · claude 2.1.296`. A role whose
/// effort came from another scope reads `project/user`; `—` for no knobs.
fn knobs_text(k: Option<&Value>) -> String {
    let Some(k) = k else {
        return "—".to_string();
    };
    let s = |v: &Value| v.as_str().unwrap_or("—").to_string();
    let mut parts: Vec<String> = ROLE_ORDER
        .iter()
        .map(|role| (role, &k["workers"][role]))
        .filter(|(_, w)| !w.is_null())
        .map(|(role, w)| {
            let mut src = s(&w["source"]);
            if let Some(e) = w["effort_source"].as_str() {
                src = format!("{src}/{e}");
            }
            format!("{role} {}/{} {src}", s(&w["model"]), s(&w["effort"]))
        })
        .collect();
    parts.extend([
        format!(
            "budget {} {}",
            or_dash(k["budget"]["value"].as_u64()),
            s(&k["budget"]["source"])
        ),
        format!("lanes {}", or_dash(k["lanes"].as_u64())),
        format!("factory {}", s(&k["factory_version"])),
        // `2.1.296 (Claude Code)` reads `2.1.296`.
        format!(
            "claude {}",
            or_dash(
                k["claude_version"]
                    .as_str()
                    .and_then(|v| v.split_whitespace().next())
            )
        ),
    ]);
    parts.join(" · ")
}

/// `--recount` (PRD-003 FR-21): rewrite the run's token, context and
/// `tool_calls` rows from the streams under `runs/<id>/workers/` and the
/// transcript dirs, one `recount:` line per worker. `events`, `gates` and
/// the run's docs stay as they are. A run still going is refused: the
/// budget and the quota pause read these rows live.
pub fn recount(run: &Run, out: &mut dyn Write) -> Result<()> {
    let phase = run.state().phase;
    if !phase.is_over() {
        return Err(Error::Msg(format!(
            "report --recount: run {} is {phase}; only a finished, stopped or failed run is recounted",
            run.id()
        )));
    }
    let db = run.db();
    let run_id = run.id().as_str();
    let repo = run.state().repo.clone();
    let mut workers = db.workers(run_id)?;
    // `repeat_in_run` depends on which worker came first.
    workers.sort_by(|a, b| (&a.started_at, a.id).cmp(&(&b.started_at, b.id)));
    // Every stream is read before a row is written: one gone leaves the
    // store as it was.
    let mut ended = Vec::new();
    for w in &workers {
        let short: String = w
            .session_id
            .as_deref()
            .unwrap_or("")
            .chars()
            .take(8)
            .collect();
        if w.ended_at.is_none() {
            writeln!(out, "recount: {} {short} no result yet, skipped", w.role)
                .map_err(io("stdout"))?;
            continue;
        }
        let session = w.session_id.as_deref().unwrap_or("");
        let path = worker::stream_path(run.dir(), session);
        let text = fs::read_to_string(&path).map_err(|e| {
            Error::Msg(format!(
                "report --recount: worker {} stream {}: {e}; nothing rewritten",
                w.id,
                path.display()
            ))
        })?;
        ended.push((w, session, short, stream::parse(&text).1));
    }
    // The gate pair as the driver reads it, so an inner-loop run keeps its kind.
    let gate = Docs::locate(&repo).ok().and_then(|d| {
        let gate = d.gate().ok()?;
        let inner = d.gate_inner().ok().filter(|s| !s.is_empty());
        Some(GateCmds { gate, inner })
    });
    let config = paths::claude_config_dir();
    db.delete_tool_calls(run_id)?;
    let mut seen_run = HashSet::new();
    for (w, session, short, summary) in &ended {
        db.update_worker_usage(w.id, summary)?;
        metrics::insert_calls(
            db,
            run_id,
            w.id,
            &summary.tool_uses,
            gate.as_ref(),
            &repo,
            &mut seen_run,
        )?;
        let transcript = measure::transcript_path(&config, &repo, session);
        let (sub, note) = if fs::symlink_metadata(&transcript).is_err() {
            (None, ", transcript missing, own tokens only".to_string())
        } else {
            match measure::subagents(&transcript) {
                Subagents::None => (Some((0, 0, 0, 0, 0)), String::new()),
                Subagents::Read { usage, count } => {
                    (Some(metrics::sub_columns(&usage, count)), String::new())
                }
                Subagents::Unreadable(e) => (None, format!(", subagents unreadable: {e}")),
            }
        };
        db.set_worker_subagents(w.id, sub)?;
        let u = summary.usage.as_ref();
        let own = quota::weigh(
            u.and_then(|u| u.input_tokens).unwrap_or(0),
            u.and_then(|u| u.cache_creation_input_tokens).unwrap_or(0),
            u.and_then(|u| u.cache_read_input_tokens).unwrap_or(0),
            u.and_then(|u| u.output_tokens).unwrap_or(0),
        );
        let n = |v: i64| v.max(0) as u64;
        let sub_weighted = sub.map(|s| quota::weigh(n(s.0), n(s.1), n(s.2), n(s.3)).round() as u64);
        writeln!(
            out,
            "recount: {} {short} own {} sub {} subagents {}{note}",
            w.role,
            own.round() as u64,
            or_dash(sub_weighted),
            or_dash(sub.map(|s| s.4))
        )
        .map_err(io("stdout"))?;
    }
    Ok(())
}

pub fn report(args: &ReportArgs, out: &mut dyn Write) -> Result<i32> {
    let run = Run::open(&paths::data_dir(), &args.run)?;
    let cockpit = if args.cockpit.is_empty() {
        None
    } else {
        Some(measure::combine(&args.cockpit)?)
    };
    if args.recount {
        // The JSON on stdout stays one object.
        let mut err = std::io::stderr();
        let lines: &mut dyn Write = if args.json { &mut err } else { out };
        recount(&run, lines)?;
    }
    let mut r = compute(run.db(), run.id().as_str(), cockpit.as_ref())?;
    if args.per_worker {
        let db = run.db();
        let id = run.id().as_str();
        r.per_worker = Some(per_worker(&db.workers(id)?, &own_calls(db, id)?));
    }
    if args.json {
        let text = serde_json::to_string_pretty(&r).map_err(json("report"))?;
        writeln!(out, "{text}").map_err(io("stdout"))?;
    } else {
        let workers = r.per_worker.iter().flatten().map(PerWorker::text);
        for line in r
            .lines()
            .into_iter()
            .chain([r.quality_line()])
            .chain(r.thresholds.lines())
            .chain(workers)
        {
            writeln!(out, "{line}").map_err(io("stdout"))?;
        }
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lines_carry_the_keys_in_order() {
        let r = Report::default();
        let keys: Vec<String> = r
            .lines()
            .iter()
            .map(|l| l.split_once(':').unwrap().0.to_string())
            .collect();
        let want: Vec<&str> = KEYS.iter().chain(EXTRA_KEYS.iter()).copied().collect();
        assert_eq!(keys, want);
        assert_eq!(r.lines()[0], "score: ");
        assert_eq!(r.lines()[16], "cockpit_tokens: 0 (no --cockpit)");
        assert_eq!(r.lines()[17], "workers: 0 (none)");
        assert_eq!(r.lines()[21], "followup_reads: 0 of 0 ctx calls (0%)");
        assert_eq!(EXTRA_KEYS[8], "weighted_by_role");
        assert_eq!(r.lines()[KEYS.len() + 8], "weighted_by_role: none");
        assert_eq!(
            &EXTRA_KEYS[9..16],
            [
                "fix_commits",
                "ci_red_rounds",
                "ci_reruns",
                "findings",
                "decisions",
                "decide",
                "blocks"
            ]
        );
        assert_eq!(r.lines()[KEYS.len() + 15], "blocks: —");
        assert_eq!(&EXTRA_KEYS[16..18], ["knobs", "hypothesis"]);
        assert_eq!(r.lines()[KEYS.len() + 17], "hypothesis: —");
        assert_eq!(r.lines()[KEYS.len() + 16], "knobs: —");
        assert_eq!(EXTRA_KEYS.last(), Some(&"pack_by_role"));
        assert_eq!(r.lines().last().unwrap(), "pack_by_role: none");
    }

    #[test]
    fn pack_by_role_means_each_section_in_pack_order() {
        let row = |role: &str, parts: Option<&str>, start: Option<i64>| WorkerRow {
            role: role.to_string(),
            start_context: start,
            pack_parts: parts.map(str::to_string),
            ..WorkerRow::default()
        };
        let workers = [
            row("polish", Some(r#"[{"section":"branch","chars":10}]"#), None),
            row(
                "build",
                Some(r#"[{"section":"slice","chars":100},{"section":"outline","chars":51}]"#),
                Some(1000),
            ),
            row(
                "build",
                Some(
                    r#"[{"section":"slice","chars":200},{"section":"block_and_answer","chars":10},{"section":"outline","chars":50}]"#,
                ),
                Some(2001),
            ),
            row("build", None, Some(9999)),
            row("fix", None, None),
        ];
        let m = pack_by_role(&workers);
        let build = m["build"].as_ref().unwrap();
        assert_eq!(
            build.sections,
            [
                ("slice".to_string(), 150),
                ("outline".to_string(), 51),
                ("block_and_answer".to_string(), 5)
            ]
        );
        assert_eq!((build.total, build.start_context), (206, Some(1501)));
        assert_eq!(m["fix"], None);
        assert_eq!(
            pack_by_role_text(&m),
            "build slice 150 outline 51 block_and_answer 5 total 206 start_context 1501 · fix — · polish branch 10 total 10 start_context —"
        );
        let json = serde_json::to_value(&m).unwrap();
        assert_eq!(json["build"]["sections"]["outline"], 51);
        assert_eq!(json["fix"], Value::Null);
    }

    #[test]
    fn the_quality_line_dashes_what_is_null() {
        let mut r = Report {
            fix_commits: 2,
            ci_red_rounds: 1,
            ..Report::default()
        };
        assert_eq!(
            r.quality_line(),
            "quality: gate_red 0 · worker_gate_red 0 · fix_commits 2 · ci_red_rounds 1 · ci_reruns 0 · findings: — · decisions — decide — blocks —"
        );
        r.findings = Some(Findings {
            high: 1,
            medium: 2,
            low: 3,
            fixed: 4,
            parked: 2,
        });
        (r.decisions, r.decide, r.blocks) = (Some(3), Some(0), Some(0));
        assert!(
            r.quality_line().ends_with(
                " · findings high=1 medium=2 low=3 fixed=4 parked=2 · decisions 3 decide 0 blocks 0"
            ),
            "{}",
            r.quality_line()
        );
        assert!(
            r.lines()
                .contains(&"findings: high=1 medium=2 low=3 fixed=4 parked=2".to_string())
        );
    }

    #[test]
    fn knobs_text_names_each_roles_scope() {
        let k = serde_json::json!({
            "workers": {
                "build": {"model": "opus", "effort": "high", "source": "default"},
                "fix": {"model": "opus", "effort": "high", "source": "default"},
                "checkpoint": {"model": "opus", "effort": "high", "source": "default"},
                "polish": {"model": "sonnet", "effort": "low", "source": "project", "effort_source": "user"},
            },
            "budget": {"value": 30000000, "source": "user"},
            "lanes": 1,
            "factory_version": "0.1.0",
            "claude_version": "2.1.296 (Claude Code)",
        });
        assert_eq!(
            knobs_text(Some(&k)),
            "build opus/high default · fix opus/high default · checkpoint opus/high default · \
             polish sonnet/low project/user · budget 30000000 user · lanes 1 · factory 0.1.0 · claude 2.1.296"
        );
        let old = serde_json::json!({"budget": {"value": 5, "source": "flag"}, "lanes": null, "claude_version": null});
        assert_eq!(
            knobs_text(Some(&old)),
            "budget 5 flag · lanes — · factory — · claude —"
        );
    }

    #[test]
    fn weighted_by_role_text_puts_own_before_subagents() {
        let mut m = BTreeMap::new();
        m.insert("polish".to_string(), OwnSub { own: 3, sub: 4 });
        m.insert("audit".to_string(), OwnSub { own: 9, sub: 0 });
        m.insert("build".to_string(), OwnSub { own: 1, sub: 0 });
        assert_eq!(
            weighted_by_role_text(&m),
            "build 1+0 · polish 3+4 · audit 9+0"
        );
    }

    #[test]
    fn the_threshold_lines_compare_with_the_section_14_constants() {
        assert_eq!(
            Thresholds::of(Some(0.5), Some(0.05), Some(0.267)).lines(),
            vec![
                "followup threshold 30%: over",
                "ctx_share threshold 10%: under",
                "repeat_rate vs 26.7%: not lower",
            ]
        );
        assert_eq!(
            Thresholds::of(Some(0.3), Some(0.1), Some(0.2)).lines(),
            vec![
                "followup threshold 30%: ok",
                "ctx_share threshold 10%: ok",
                "repeat_rate vs 26.7%: lower",
            ]
        );
        // No ctx call, no lookup: nothing to compare, not a pass.
        assert_eq!(
            Thresholds::default().lines(),
            vec![
                "followup threshold 30%: n/a",
                "ctx_share threshold 10%: n/a",
                "repeat_rate vs 26.7%: n/a",
            ]
        );
        assert_eq!(Thresholds::of(None, Some(0.0), Some(0.0)).followup, "n/a");
        let f = Followups {
            count: 1,
            ctx_calls: 3,
            share: ratio(1, 3),
        };
        assert_eq!(f.text(), "1 of 3 ctx calls (33%)");
    }

    #[test]
    fn followups_by_role_text_lists_roles_in_the_table_order() {
        let f = |count, ctx_calls| Followups {
            count,
            ctx_calls,
            share: ratio(count, ctx_calls),
        };
        let mut m = BTreeMap::new();
        assert_eq!(by_role_text(&m), "none");
        m.insert("polish".to_string(), f(5, 14));
        m.insert("build".to_string(), f(2, 7));
        assert_eq!(by_role_text(&m), "build 2 of 7 (29%), polish 5 of 14 (36%)");
    }

    #[test]
    fn workers_text_lists_roles_in_the_table_order() {
        let mut w = Workers {
            total: 5,
            by_role: BTreeMap::new(),
        };
        w.by_role.insert("checkpoint".to_string(), 1);
        w.by_role.insert("build".to_string(), 3);
        w.by_role.insert("fix".to_string(), 1);
        assert_eq!(w.text(), "5 (build 3, fix 1, checkpoint 1)");
    }

    #[test]
    fn json_round_trips_with_nulls_for_the_absent() {
        let r = Report {
            weighted_tokens: 7,
            ..Report::default()
        };
        let v: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&r).unwrap()).unwrap();
        assert_eq!(v["weighted_tokens"], 7);
        assert!(v["score"].is_null());
        assert!(v["cockpit_tokens"].is_null());
        assert_eq!(v["workers"]["total"], 0);
    }
}
