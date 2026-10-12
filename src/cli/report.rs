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
use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;

use clap::Args;
use serde::Serialize;

use crate::db::Db;
use crate::db::{ToolCallRow, WorkerRow};
use crate::error::{io, json};
use crate::measure::{self, Measure};
use crate::metrics::{CTX_KINDS, LOOKUP_KINDS};
use crate::run::Run;
use crate::{Result, paths, quota};

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
/// list (AD-802; `lookup_share_chars`, slice 13).
pub const EXTRA_KEYS: [&str; 5] = [
    "worker_gate_runs",
    "worker_gate_red",
    "cockpit_peak_context",
    "lookup_share_chars",
    "followup_reads_by_role",
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
}

impl PerWorker {
    /// `worker 3 build slice 02 (1a2b3c4d): lookups 6, ctx_share 0.333, repeat_rate 0.167`.
    fn text(&self) -> String {
        let slice = self
            .slice
            .map_or(String::new(), |n| format!(" slice {n:02}"));
        let session = self.session_id.as_deref().map_or(String::new(), |s| {
            format!(" ({})", s.chars().take(8).collect::<String>())
        });
        format!(
            "worker {} {}{slice}{session}: lookups {}, ctx_share {:.3}, repeat_rate {:.3}",
            self.id, self.role, self.lookups, self.ctx_share, self.repeat_rate
        )
    }
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
    /// Lookup bytes among all result bytes: slice 08's `ctx_share`.
    pub lookup_share_chars: f64,
    /// Roles with no `ctx` call are left out.
    pub followup_reads_by_role: BTreeMap<String, Followups>,
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
    let calls = db.tool_calls(run_id)?;
    let events = db.events(run_id)?;
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
        out_tokens: workers.iter().map(|w| n(w.output)).sum(),
        cache_read_tokens: workers.iter().map(|w| n(w.cache_read)).sum(),
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
        input_tokens: workers.iter().map(|w| n(w.input)).sum(),
        cache_write_tokens: workers.iter().map(|w| n(w.cache_write)).sum(),
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
        lookup_share_chars: ratio(lookup_chars, all_chars),
        followup_reads_by_role: followups_by_role(&workers, &calls),
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
            format!("lookup_share_chars: {:.3}", self.lookup_share_chars),
            format!(
                "followup_reads_by_role: {}",
                by_role_text(&self.followup_reads_by_role)
            ),
        ]
    }
}

pub fn report(args: &ReportArgs, out: &mut dyn Write) -> Result<i32> {
    let run = Run::open(&paths::data_dir(), &args.run)?;
    let cockpit = if args.cockpit.is_empty() {
        None
    } else {
        Some(measure::combine(&args.cockpit)?)
    };
    let mut r = compute(run.db(), run.id().as_str(), cockpit.as_ref())?;
    if args.per_worker {
        let db = run.db();
        let id = run.id().as_str();
        r.per_worker = Some(per_worker(&db.workers(id)?, &db.tool_calls(id)?));
    }
    if args.json {
        let text = serde_json::to_string_pretty(&r).map_err(json("report"))?;
        writeln!(out, "{text}").map_err(io("stdout"))?;
    } else {
        let workers = r.per_worker.iter().flatten().map(PerWorker::text);
        for line in r
            .lines()
            .into_iter()
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
