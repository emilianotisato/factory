//! A timeline item's `label`: factory's short sentence for a human, where `kind` is the machine name.
use super::redact;
use super::workers::{cut, one_line};
use crate::clock;
use crate::cockpit::tokens;
use crate::events::EventKind;

/// The longest label, in chars, `…` included.
pub const MAX: usize = 80;

/// No `_` arm: a new kind does not compile until it has a label.
pub fn label(kind: &EventKind) -> String {
    use EventKind as K;
    let s = match kind {
        K::RunStarted => "run started".to_string(),
        K::RunResumed => "run resumed".to_string(),
        K::AskOpened { id, text } => format!("{id} asks: {text}"),
        K::JudgeNoted { text } => format!("judge: {text}"),
        K::StopRequested => "stop requested".to_string(),
        K::SliceSpawned { slice, .. } => format!("slice {slice:02} started"),
        K::MergeSpawned { slice, attempt, .. } => {
            format!("slice {slice:02} merge started{}", again(*attempt))
        }
        K::LandingConflicted { slice, paths } => {
            format!(
                "slice {slice:02} conflicted in {}",
                count(paths.len(), "file")
            )
        }
        K::SliceDone { slice } => format!("slice {slice:02} done"),
        K::SliceBlocked {
            slice, question, ..
        } => match question {
            Some(q) => format!("slice {slice:02} blocked: {q}"),
            None => format!("slice {slice:02} blocked"),
        },
        K::RunWaiting { .. } => "waiting on the owner".to_string(),
        K::BlockAnswered { id, text, .. } | K::AskAnswered { id, text } => {
            format!("{id} answered: {text}")
        }
        K::WorkerDead {
            slice, exit_code, ..
        } => {
            let who = match slice {
                Some(s) => format!("slice {s:02} worker"),
                None => "a worker".to_string(),
            };
            match exit_code {
                Some(c) => format!("{who} died, exit {c}"),
                None => format!("{who} died"),
            }
        }
        K::RunPaused { resets_at, .. } => {
            format!(
                "paused on the usage limit until {}",
                clock::rfc3339(*resets_at)
            )
        }
        K::RunResumedFromPause => "resumed after the pause".to_string(),
        K::RunParked {
            reason,
            weighted,
            budget,
        } => format!(
            "parked on {reason}: {} of {} tokens",
            tokens(*weighted),
            tokens(*budget)
        ),
        K::PackSectionMissing { slice, section } => {
            format!("slice {slice:02} pack has no PRD section {section}")
        }
        K::SubagentsUnread { session_id, .. } => {
            format!("subagents unread for worker {}", short(session_id, 8))
        }
        K::SliceCommitted { slice, sha, files } => format!(
            "slice {slice:02} committed {} ({})",
            short(sha, 7),
            count(files.len(), "file")
        ),
        K::WorkerWroteProtectedFile { slice, path } => {
            format!("slice {slice:02} wrote protected {path}, left out")
        }
        K::LogMerged { fragments } => {
            format!("log merged from {}", count(fragments.len(), "fragment"))
        }
        K::MergeRefused { block_id, .. } => format!("log merge refused: {block_id}"),
        K::CheckpointStarted { n, covers } => format!(
            "checkpoint {n} started over {}",
            count(covers.len(), "slice")
        ),
        K::CheckpointResumed { n, .. } => format!("checkpoint {n} resumed"),
        K::GateRun {
            n,
            attempt,
            red: true,
            ..
        } => format!("gate {n} red{}", again(*attempt)),
        K::GateRun {
            n,
            attempt,
            red: false,
            minutes,
            ..
        } => format!("gate {n} green in {minutes:.1} min{}", again(*attempt)),
        K::GateStarted { n, attempt, .. } => format!("gate {n} running{}", again(*attempt)),
        K::GateCached { n, red, .. } => {
            format!("gate {n} {}, cached", if *red { "red" } else { "green" })
        }
        K::FixSpawned { n, attempt, .. } => format!("gate {n} fix started{}", again(*attempt)),
        K::FixCommitted { n, sha, files, .. } => format!(
            "gate {n} fix committed {} ({})",
            short(sha, 7),
            count(files.len(), "file")
        ),
        K::CheckpointSpawned { n, attempt, .. } => {
            format!("checkpoint {n} worker started{}", again(*attempt))
        }
        K::CheckpointBlocked { n, question, .. } => format!("checkpoint {n} blocked: {question}"),
        K::CheckpointDone { n, .. } => format!("checkpoint {n} done"),
        K::PolishSpawned { attempt, .. } => format!("polish started{}", again(*attempt)),
        K::PolishDone { sha: None, .. } => "polish done, nothing changed".to_string(),
        K::PolishDone { files, .. } => {
            format!("polish done, {} changed", count(files.len(), "file"))
        }
        K::PolishFindingsMissing { .. } => "polish wrote no findings".to_string(),
        K::TemplateStandIn { what, path } => {
            let file = match (path.is_empty(), what.as_str()) {
                (false, _) => path.as_str(),
                (true, "retro") => "the retro",
                (true, "pr_body") => "the PR body",
                (true, _) => "a closeout file",
            };
            format!("{file} is still the template")
        }
        K::Shipped { .. } => "shipped".to_string(),
        K::PrOpened { url } => format!("PR opened: {url}"),
        K::CiGreen { head } => format!("CI green on {}", short(head, 7)),
        K::CiRed { check, .. } => format!("CI red: {check}"),
        K::CiRerun { check, .. } => format!("CI rerun: {check}"),
        K::CiFixSpawned { round, attempt, .. } => {
            format!("CI fix round {round} started{}", again(*attempt))
        }
        K::CiWaiting { checks } => format!("CI running {}", count(checks.len(), "check")),
        K::CiNoChecks { minutes } => format!("no CI checks after {minutes} min"),
        K::FixEmpty { n: Some(n), .. } => format!("gate {n} fix changed nothing"),
        K::FixEmpty { round: Some(r), .. } => format!("CI fix round {r} changed nothing"),
        K::FixEmpty { .. } => "a fix changed nothing".to_string(),
        K::CollectStarted { passes: 0, .. } => "review started, no second opinion".to_string(),
        K::CollectStarted { passes, .. } => {
            format!("review started, {}", count(*passes as usize, "pass"))
        }
        K::ReviewPassDone { k, of, seconds } => {
            format!("review pass {k} of {of} done in {}", took(*seconds))
        }
        K::CollectDone { seconds, ok: true } => format!("review done in {}", took(*seconds)),
        K::CollectDone { seconds, ok: false } => {
            format!("review failed after {}", took(*seconds))
        }
        K::CiFixCommitted { round, sha, files } => format!(
            "CI fix round {round} committed {} ({})",
            short(sha, 7),
            count(files.len(), "file")
        ),
        K::CloseoutBlocked {
            phase, question, ..
        } => format!("closeout {phase} blocked: {question}"),
        K::RunFinished {
            slices,
            duration_minutes,
            ..
        } => format!(
            "run finished: {} in {duration_minutes:.0} min",
            count(*slices as usize, "slice")
        ),
        K::RunStopped { slices, .. } => {
            format!("run stopped after {}", count(*slices as usize, "slice"))
        }
        K::RunFailed { error } => format!("run failed: {error}"),
        K::Unknown => "an event from a newer factory".to_string(),
    };
    // Redacted before the cut, so a cut never leaves half a secret the pass no longer matches.
    cut(&one_line(&redact::tokens(&s)), MAX)
}

fn again(attempt: u32) -> String {
    if attempt > 1 {
        format!(", attempt {attempt}")
    } else {
        String::new()
    }
}

fn count(n: usize, one: &str) -> String {
    match (n, one) {
        (1, _) => format!("1 {one}"),
        (_, "pass") => format!("{n} passes"),
        _ => format!("{n} {one}s"),
    }
}

fn short(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

fn took(seconds: u64) -> String {
    if seconds < 120 {
        format!("{seconds} s")
    } else {
        format!("{:.1} min", seconds as f64 / 60.0)
    }
}
