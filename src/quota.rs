//! FR-9: quota. A worker that hits a usage limit pauses the run until the
//! stream's `resetsAt` (15 minutes when it gave none); the weighted token
//! sum over the `workers` table parks the run when it passes the budget.
//! Neither counts toward a death cap: `WorkerOutcome::counts_as_death`.
//!
//! The scheduler and the checkpoint ask [`hold`] before every spawn; a
//! pause is a phase (`paused`) the owner sees on the board, a budget park is
//! `waiting-owner` and the driver exits so `factory run NNN --budget <n>`
//! can resume it (AD-703).
use std::io::Write;
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::db::Db;
use crate::events::EventKind;
use crate::run::{Phase, Run};
use crate::scheduler::{SLOW_TICK, say};
use crate::stream::Summary;
use crate::{Result, clock};

/// How each `workers` column counts toward the budget.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Weights {
    pub input: f64,
    pub cache_write: f64,
    pub cache_read: f64,
    pub output: f64,
}

/// `weighted = input + cache_write + 0.1 × cache_read + 5 × output`
/// (PRD §11). Thinking tokens are inside `output` already. The ratios are
/// the ones `~/.claude/skills/feature-docs/MAINTAINING.md:298` states — "a
/// cache read costs 0.1 of an input token, an output token 5" — confirmed
/// against that file on 2026-10-10. Change them here only.
pub const WEIGHTS: Weights = Weights {
    input: 1.0,
    cache_write: 1.0,
    cache_read: 0.1,
    output: 5.0,
};

/// A limit with no `resetsAt`: retry this often, re-pausing on each miss.
pub const RETRY_SECS: u64 = 900;

/// A usage limit read off a worker's stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Limit {
    /// The stream's `resetsAt` in epoch seconds; `None` when it gave none.
    pub until: Option<u64>,
    /// The `status` literal, or what matched in the `result` text.
    pub literal: String,
}

impl Limit {
    /// PRD §11: a `rate_limit_event` whose `status` is not an `allowed`
    /// (`allowed_warning` is still allowed — AD-702), or a `result` with
    /// `api_error_status` 429, or an error `result` naming a usage or rate
    /// limit. The `result` paths carry no reset: the 15-minute retry applies.
    pub fn from(summary: &Summary) -> Option<Limit> {
        if let Some(status) = summary
            .rate_limit
            .as_ref()
            .and_then(|r| r.status.as_deref())
            && !status.starts_with("allowed")
        {
            return Some(Limit {
                until: summary.rate_limit.as_ref().and_then(|r| r.resets_at),
                literal: status.to_string(),
            });
        }
        if !summary.has_result || !summary.is_error {
            return None;
        }
        if is_429(summary.api_error_status.as_ref()) {
            return Some(Limit {
                until: None,
                literal: "api_error_status 429".to_string(),
            });
        }
        let text = summary.result_text.as_deref()?.to_ascii_lowercase();
        ["usage limit", "rate limit"]
            .into_iter()
            .find(|p| text.contains(p))
            .map(|p| Limit {
                until: None,
                literal: p.to_string(),
            })
    }
}

fn is_429(v: Option<&Value>) -> bool {
    match v {
        Some(Value::Number(n)) => n.as_u64() == Some(429),
        Some(Value::String(s)) => s == "429",
        _ => false,
    }
}

/// One worker's weighted tokens.
pub fn weigh(input: u64, cache_write: u64, cache_read: u64, output: u64) -> f64 {
    input as f64 * WEIGHTS.input
        + cache_write as f64 * WEIGHTS.cache_write
        + cache_read as f64 * WEIGHTS.cache_read
        + output as f64 * WEIGHTS.output
}

/// The run's weighted tokens: the sum over its `workers` rows, rounded.
pub fn weighted(db: &Db, run_id: &str) -> Result<u64> {
    let n = |v: Option<i64>| v.unwrap_or(0).max(0) as u64;
    let sum: f64 = db
        .workers(run_id)?
        .iter()
        .map(|w| weigh(n(w.input), n(w.cache_write), n(w.cache_read), n(w.output)))
        .sum();
    Ok(sum.round() as u64)
}

/// Why nothing may spawn right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hold {
    /// Paused until this epoch second.
    Paused { until: u64 },
    /// Over budget: in-flight workers finish, the driver parks.
    Budget { weighted: u64, budget: u64 },
}

/// The budget first — it does not lift by waiting — then the pause.
pub fn hold(run: &Run) -> Result<Option<Hold>> {
    let weighted = weighted(run.db(), run.id().as_str())?;
    let budget = run.state().budget_weighted_tokens;
    if weighted > budget {
        return Ok(Some(Hold::Budget { weighted, budget }));
    }
    if let Some(until) = run.state().paused_until
        && clock::unix_now() < until
    {
        return Ok(Some(Hold::Paused { until }));
    }
    Ok(None)
}

/// `false` until `now >= paused_until` and the budget has room.
pub fn can_spawn(run: &Run) -> Result<bool> {
    Ok(hold(run)?.is_none())
}

/// Pause on a limit; returns the epoch second spawning may resume. One
/// `run_paused` per pause: a second limited worker that was already in
/// flight only extends the pause, it does not announce again.
pub fn pause(run: &mut Run, limit: &Limit, out: &mut dyn Write) -> Result<u64> {
    let now = clock::unix_now();
    let until = limit.until.filter(|u| *u > now).unwrap_or(now + RETRY_SECS);
    if let Some(current) = run.state().paused_until.filter(|c| *c > now) {
        if until > current {
            run.set_paused_until(Some(until))?;
        }
        return Ok(until.max(current));
    }
    run.set_paused_until(Some(until))?;
    run.set_phase(Phase::Paused)?;
    run.emit(EventKind::RunPaused {
        literal: limit.literal.clone(),
        resets_at: until,
    })?;
    say(
        out,
        &format!(
            "paused: {} — spawning resumes at {}",
            limit.literal,
            clock::rfc3339(until)
        ),
    )?;
    Ok(until)
}

/// The pause passed: back to `after` (building, or checkpoint when the
/// limited worker was a fix or checkpoint worker).
pub fn resume(run: &mut Run, after: Phase, out: &mut dyn Write) -> Result<()> {
    run.set_paused_until(None)?;
    run.set_phase(after)?;
    run.emit(EventKind::RunResumedFromPause)?;
    say(out, "resumed from pause")
}

/// Wait out up to one slow tick of a pause: returns on `factory stop`, when
/// the clock passes `until`, or after `SLOW_TICK`.
pub fn pause_tick(run: &mut Run, until: u64) -> Result<()> {
    let deadline = Instant::now() + SLOW_TICK;
    while Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(250));
        run.reload()?;
        if run.state().stop_requested || clock::unix_now() >= until {
            return Ok(());
        }
    }
    Ok(())
}

/// Over budget with nothing in flight: announce, park, and hand the driver
/// `waiting-owner` so it exits — `factory run NNN --budget <n>` refuses a
/// live run, so the parked driver cannot stay up (AD-703).
pub fn park_budget(
    run: &mut Run,
    weighted: u64,
    budget: u64,
    out: &mut dyn Write,
) -> Result<Phase> {
    run.emit(EventKind::RunParked {
        reason: "budget".to_string(),
        weighted,
        budget,
    })?;
    run.set_phase(Phase::WaitingOwner)?;
    say(
        out,
        &format!(
            "parked: {weighted} weighted tokens over the {budget} budget — resume with `factory run {} --budget <higher>`",
            run.state().feature
        ),
    )?;
    Ok(Phase::WaitingOwner)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stream::RateLimit;

    fn limited(status: &str, resets_at: Option<u64>) -> Summary {
        Summary {
            has_result: true,
            is_error: true,
            rate_limit: Some(RateLimit {
                status: Some(status.to_string()),
                resets_at,
                utilization_pct: None,
            }),
            ..Summary::default()
        }
    }

    #[test]
    fn rejected_status_is_a_limit_with_its_reset() {
        let l = Limit::from(&limited("rejected", Some(1_791_592_200))).unwrap();
        assert_eq!(l.until, Some(1_791_592_200));
        assert_eq!(l.literal, "rejected");
        let l = Limit::from(&limited("rejected", None)).unwrap();
        assert_eq!(l.until, None);
    }

    #[test]
    fn allowed_statuses_are_not_limits() {
        let mut s = limited("allowed", Some(1));
        s.is_error = false;
        assert_eq!(Limit::from(&s), None);
        let mut s = limited("allowed_warning", Some(1));
        s.is_error = false;
        assert_eq!(Limit::from(&s), None);
    }

    #[test]
    fn result_paths_pause_without_a_reset() {
        let s = Summary {
            has_result: true,
            is_error: true,
            api_error_status: Some(serde_json::json!(429)),
            ..Summary::default()
        };
        assert_eq!(Limit::from(&s).unwrap().literal, "api_error_status 429");
        let s = Summary {
            has_result: true,
            is_error: true,
            result_text: Some("You've hit your Usage Limit for the day.".to_string()),
            ..Summary::default()
        };
        let l = Limit::from(&s).unwrap();
        assert_eq!((l.until, l.literal.as_str()), (None, "usage limit"));
        // Same text, no error flag: a worker talking about limits is not limited.
        let s = Summary {
            has_result: true,
            result_text: Some("rate limit handling is done".to_string()),
            ..Summary::default()
        };
        assert_eq!(Limit::from(&s), None);
        // No result at all is a death (FR-1), not a limit.
        assert_eq!(Limit::from(&Summary::dead()), None);
    }

    #[test]
    fn weights_match_the_prd_formula() {
        // The stub's result: 18 + 6514 + 0.1×41674 + 5×182.
        assert!((weigh(18, 6514, 41674, 182) - 11_609.4).abs() < 1e-9);
        assert_eq!(weigh(0, 0, 0, 0), 0.0);
        assert_eq!(weigh(10, 0, 0, 0), 10.0);
        assert_eq!(weigh(0, 0, 10, 0), 1.0);
        assert_eq!(weigh(0, 0, 0, 1), 5.0);
    }
}
