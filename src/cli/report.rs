//! `factory report <run>` (PRD §10 FR-8): the twelve retro keys the old
//! framework's `retro.md` expects, then the eight run keys FR-11 compares,
//! one `key: value` line each, or one JSON object with `--json`. Every
//! number is a `SELECT` over what the driver wrote as it ran; nothing is
//! re-parsed from a transcript. `--cockpit <session.jsonl>...` adds the
//! cockpit's own cost from its transcript (`measure`). `score` is the
//! owner's and prints empty (§13).
use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;

use clap::Args;
use serde::Serialize;

use crate::db::Db;
use crate::error::{io, json};
use crate::measure::{self, Measure};
use crate::metrics::LOOKUP_KINDS;
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
}

/// The retro keys (`retro.md:2-14`), in order, then the run keys (§10).
pub const KEYS: [&str; 21] = [
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
];

/// The keys after the twenty-one: what the slice's task text names beyond
/// §10's list (AD-802).
pub const EXTRA_KEYS: [&str; 3] = [
    "worker_gate_runs",
    "worker_gate_red",
    "cockpit_peak_context",
];

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
    pub ctx_share: f64,
    pub repeat_rate: f64,
    pub worker_gate_runs: u64,
    pub worker_gate_red: u64,
    pub cockpit_peak_context: Option<u64>,
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
    let repeats = lookup.iter().filter(|c| c.repeat_in_worker).count() as u64;

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
        ctx_share: ratio(lookup_chars, all_chars),
        repeat_rate: ratio(repeats, lookup.len() as u64),
        worker_gate_runs: worker_gates.len() as u64,
        worker_gate_red: worker_gates.iter().filter(|g| g.red).count() as u64,
        cockpit_peak_context: cockpit.map(|m| m.peak_context),
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
            format!("worker_gate_runs: {}", self.worker_gate_runs),
            format!("worker_gate_red: {}", self.worker_gate_red),
            format!("cockpit_peak_context: {}", opt(self.cockpit_peak_context)),
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
    let r = compute(run.db(), run.id().as_str(), cockpit.as_ref())?;
    if args.json {
        let text = serde_json::to_string_pretty(&r).map_err(json("report"))?;
        writeln!(out, "{text}").map_err(io("stdout"))?;
    } else {
        for line in r.lines() {
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
