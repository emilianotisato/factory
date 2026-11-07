//! `timeline[]`: the run's events, each with a tone, a short label and a whitelisted `data`.
use serde_json::{Map, Value, json};

use super::Ctx;
use crate::events::{Event, EventKind};

/// How many events the timeline keeps when no `since` is given.
pub const DEFAULT: usize = 100;

pub fn timeline(ctx: &Ctx, since: Option<u64>) -> Value {
    match since {
        Some(since) => ctx
            .events
            .iter()
            .filter(|e| e.seq > since)
            .map(item)
            .collect(),
        None => ctx.events[ctx.events.len().saturating_sub(DEFAULT)..]
            .iter()
            .map(item)
            .collect(),
    }
}

fn item(e: &Event) -> Value {
    json!({
        "seq": e.seq,
        "ts": e.ts,
        "kind": e.kind.name(),
        "tone": tone(&e.kind),
        "label": e.kind.short(),
        "data": data(&e.kind),
    })
}

/// No `_` arm: a new kind does not compile until it has a tone.
pub fn tone(kind: &EventKind) -> &'static str {
    use EventKind as K;
    match kind {
        K::AskOpened { .. }
        | K::RunWaiting { .. }
        | K::RunParked { .. }
        | K::SliceBlocked { .. }
        | K::CheckpointBlocked { .. }
        | K::CloseoutBlocked { .. }
        | K::BlockAnswered { .. }
        | K::AskAnswered { .. } => "owner",
        K::GateRun { red: true, .. }
        | K::CiRed { .. }
        | K::WorkerDead { .. }
        | K::MergeRefused { .. }
        | K::LandingConflicted { .. }
        | K::FixEmpty { .. }
        | K::RunFailed { .. } => "red",
        K::GateRun { red: false, .. }
        | K::CiGreen { .. }
        | K::SliceDone { .. }
        | K::CheckpointDone { .. }
        | K::PolishDone { .. }
        | K::PrOpened { .. }
        | K::Shipped { .. }
        | K::RunFinished { .. } => "green",
        K::CiWaiting { .. } | K::ReviewPassDone { .. } => "noise",
        K::RunStarted
        | K::RunResumed
        | K::JudgeNoted { .. }
        | K::StopRequested
        | K::SliceSpawned { .. }
        | K::MergeSpawned { .. }
        | K::RunPaused { .. }
        | K::RunResumedFromPause
        | K::PackSectionMissing { .. }
        | K::SubagentsUnread { .. }
        | K::SliceCommitted { .. }
        | K::WorkerWroteProtectedFile { .. }
        | K::LogMerged { .. }
        | K::CheckpointStarted { .. }
        | K::CheckpointResumed { .. }
        | K::GateStarted { .. }
        | K::GateCached { .. }
        | K::FixSpawned { .. }
        | K::FixCommitted { .. }
        | K::CheckpointSpawned { .. }
        | K::PolishSpawned { .. }
        | K::PolishFindingsMissing { .. }
        | K::TemplateStandIn { .. }
        | K::CiRerun { .. }
        | K::CiFixSpawned { .. }
        | K::CiNoChecks { .. }
        | K::CiFixCommitted { .. }
        | K::CollectStarted { .. }
        | K::CollectDone { .. }
        | K::RunStopped { .. }
        | K::Unknown => "info",
    }
}

/// The event's own fields: scalars as they are, a list as its length, an object as its key count.
fn data(kind: &EventKind) -> Value {
    let Ok(Value::Object(o)) = serde_json::to_value(kind) else {
        return json!({});
    };
    let out: Map<String, Value> = o
        .into_iter()
        .filter(|(k, _)| k != "kind")
        .map(|(k, v)| {
            let v = match v {
                Value::Array(a) => json!(a.len()),
                Value::Object(o) => json!(o.len()),
                v => v,
            };
            (k, v)
        })
        .collect();
    Value::Object(out)
}
