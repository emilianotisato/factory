//! The serial loop (PRD §3 FR-1, §4 FR-2), lanes = 1: read the board, spawn
//! one worker for `ready[0]`, wait, close its `workers` row, re-read the board
//! and route on the slice's frontmatter — `done`, `blocked`, or neither, which
//! is a dead worker. The board is the truth; the worker's `STATUS:` line is a
//! hint the row keeps.
//!
//! Dead workers (FR-1, autonomy.md:76): the first death on a slice is a note
//! in that slice's `log-slice-NN.md` and a fresh worker with a new session;
//! the second blocks the slice with `BLOCK-NN02`. The count lives in
//! `state.json` `workers[]`, so a resume after a kill counts the killed worker.
//!
//! Slice 04 parks a lane on a block instead of the run; slice 06 runs the
//! checkpoint where this loop stops at `checkpoint_due`.
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::config::Config;
use crate::docs::{Docs, Status};
use crate::error::io;
use crate::events::EventKind;
use crate::pack::Pack;
use crate::run::{Phase, Run, WorkerRef, pid_alive};
use crate::stream::Summary;
use crate::worker::{self, Worker};
use crate::{Result, clock};

/// Deaths on one slice before it is blocked: respawn once, then block.
pub const DEATH_CAP: usize = 2;

/// How one build attempt ended for the loop's purposes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Outcome {
    Done,
    Respawn,
    Blocked,
}

/// What settling the unsettled workers of a resumed run found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Orphans {
    None,
    /// A worker of a dead driver is still running: wait, then read again.
    Live,
    Settled,
    Blocked,
}

/// Drive `run` until it finishes, parks or stops. On an error the run is
/// marked `failed` with a `run_failed` event before the error returns.
pub fn drive(run: &mut Run, docs: &Docs, cfg: &Config, out: &mut dyn Write) -> Result<Phase> {
    match drive_inner(run, docs, cfg, out) {
        Ok(phase) => Ok(phase),
        Err(e) => {
            let _ = run.set_phase(Phase::Failed);
            let _ = run.emit(EventKind::RunFailed {
                error: e.to_string(),
            });
            let _ = writeln!(out, "run {} failed: {e}", run.id());
            Err(e)
        }
    }
}

fn drive_inner(
    run: &mut Run,
    docs: &Docs,
    cfg: &Config,
    out: &mut dyn Write,
) -> Result<Phase> {
    let feature = run.state().feature.clone();
    run.set_phase(Phase::Building)?;
    loop {
        run.reload()?;
        if run.state().stop_requested {
            return finish(run, out, "stop requested");
        }
        let mut board = docs.status(&feature)?;
        match settle_orphans(run, &board, out)? {
            Orphans::None => {}
            Orphans::Live => {
                std::thread::sleep(Duration::from_millis(200));
                continue;
            }
            Orphans::Settled => board = docs.status(&feature)?,
            Orphans::Blocked => return park(run, out, Phase::WaitingOwner),
        }
        if !board.blocked.is_empty() {
            say(
                out,
                &format!(
                    "slice {} blocked — open blocks wait for the owner",
                    board
                        .blocked
                        .iter()
                        .map(|n| format!("{n:02}"))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            )?;
            return park(run, out, Phase::WaitingOwner);
        }
        // A `building` slice with every recorded worker settled is one whose
        // worker died below the cap: it gets its fresh worker here.
        let target = if let Some(b) = board.building.first() {
            b.number
        } else if board.checkpoint_due {
            say(
                out,
                &format!(
                    "checkpoint due (unverified: {}) — slice 06 runs it; stopping here",
                    board
                        .unverified
                        .iter()
                        .map(|n| format!("{n:02}"))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            )?;
            run.emit(EventKind::CheckpointDue {
                unverified: board.unverified.clone(),
            })?;
            run.set_phase(Phase::Checkpoint)?;
            return Ok(Phase::Checkpoint);
        } else if let Some(r) = board.ready.first() {
            r.number
        } else {
            return finish(run, out, "nothing ready");
        };
        match build_once(run, docs, cfg, &board, target, out)? {
            Outcome::Done | Outcome::Respawn => {}
            Outcome::Blocked => return park(run, out, Phase::WaitingOwner),
        }
    }
}

/// Pack, spawn, wait, route.
fn build_once(
    run: &mut Run,
    docs: &Docs,
    cfg: &Config,
    board: &Status,
    slice: u32,
    out: &mut dyn Write,
) -> Result<Outcome> {
    let spawn_n = run.state().workers.len() as u32 + 1;
    let pack = Pack::for_slice(docs, board, slice, &run.dir().join("packs"), spawn_n)?;
    for section in &pack.missing_sections {
        run.emit(EventKind::PackSectionMissing {
            slice,
            section: *section,
        })?;
        say(
            out,
            &format!("slice {slice:02} cites §{section}, which prd.md does not have"),
        )?;
    }
    let w = Worker::spawn(run, "build", slice, &pack, &cfg.workers.build)?;
    let (session_id, row_id) = (w.session_id.clone(), w.row_id);
    say(
        out,
        &format!(
            "slice {slice:02} → worker {session_id} (pid {}), pack {}",
            w.pid,
            pack.path.display()
        ),
    )?;
    let exit = w.wait()?;
    let after = docs.status(&run.state().feature)?;
    match slice_status(&after, slice) {
        "done" => {
            settle(run, row_id, &session_id, &exit.summary, "done")?;
            run.emit(EventKind::SliceDone { slice })?;
            say(
                out,
                &format!(
                    "slice {slice:02} done (worker {session_id}, {} turns, hint {})",
                    exit.summary.num_turns.unwrap_or(0),
                    exit.summary
                        .status_hint
                        .map(|h| h.as_str())
                        .unwrap_or("none")
                ),
            )?;
            Ok(Outcome::Done)
        }
        "blocked" => {
            settle(run, row_id, &session_id, &exit.summary, "blocked")?;
            run.emit(EventKind::SliceBlocked { slice })?;
            say(out, &format!("slice {slice:02} blocked by its worker"))?;
            Ok(Outcome::Blocked)
        }
        _ => {
            let w = find_ref(run, &session_id);
            if settle_dead(run, &after, &w, exit.code, exit.summary, out)? {
                Ok(Outcome::Blocked)
            } else {
                Ok(Outcome::Respawn)
            }
        }
    }
}

/// Workers `state.json` says are unsettled, for a run this process did not
/// start: a dead driver's workers (FR-1 resume).
fn settle_orphans(run: &mut Run, board: &Status, out: &mut dyn Write) -> Result<Orphans> {
    let unsettled: Vec<WorkerRef> = run
        .state()
        .workers
        .iter()
        .filter(|w| w.outcome.is_none())
        .cloned()
        .collect();
    if unsettled.is_empty() {
        return Ok(Orphans::None);
    }
    let mut result = Orphans::Settled;
    for w in unsettled {
        if pid_alive(w.pid) {
            say(
                out,
                &format!(
                    "slice {:02}: worker {} (pid {}) of an earlier driver is still running — waiting",
                    w.slice, w.session_id, w.pid
                ),
            )?;
            return Ok(Orphans::Live);
        }
        let summary = worker::summary_from_disk(run.dir(), &w.session_id);
        match slice_status(board, w.slice) {
            "done" => {
                settle(run, w.row_id, &w.session_id, &summary, "done")?;
                run.emit(EventKind::SliceDone { slice: w.slice })?;
                say(
                    out,
                    &format!(
                        "slice {:02} done by worker {} of an earlier driver",
                        w.slice, w.session_id
                    ),
                )?;
            }
            "blocked" => {
                settle(run, w.row_id, &w.session_id, &summary, "blocked")?;
                run.emit(EventKind::SliceBlocked { slice: w.slice })?;
                result = Orphans::Blocked;
            }
            _ => {
                if settle_dead(run, board, &w, None, summary, out)? {
                    result = Orphans::Blocked;
                }
            }
        }
    }
    Ok(result)
}

/// Close the row and the state entry.
fn settle(
    run: &mut Run,
    row_id: i64,
    session_id: &str,
    summary: &Summary,
    outcome: &str,
) -> Result<()> {
    run.db()
        .finish_worker(row_id, summary, outcome, &clock::now_rfc3339())?;
    run.set_worker_outcome(session_id, outcome)
}

/// A worker gone with the slice neither `done` nor `blocked`. Returns `true`
/// when this death blocked the slice.
fn settle_dead(
    run: &mut Run,
    board: &Status,
    w: &WorkerRef,
    exit_code: Option<i32>,
    summary: Summary,
    out: &mut dyn Write,
) -> Result<bool> {
    let outcome = if summary.is_dead() { "dead" } else { "failed" };
    settle(run, w.row_id, &w.session_id, &summary, outcome)?;
    run.emit(EventKind::WorkerDead {
        slice: w.slice,
        session_id: w.session_id.clone(),
        exit_code,
    })?;
    let deaths = run
        .state()
        .workers
        .iter()
        .filter(|x| x.slice == w.slice && matches!(x.outcome.as_deref(), Some("dead" | "failed")))
        .count();
    let repo = run.state().repo.clone();
    let fragment = repo
        .join(&board.dir)
        .join(format!("log-slice-{:02}.md", w.slice));
    let code = exit_code.map_or("exited (code unknown)".to_string(), |c| format!("exited {c}"));
    let what = if summary.is_dead() {
        "with no result"
    } else {
        "with a result but the slice neither done nor blocked"
    };
    let date = clock::now_rfc3339()[..10].to_string();
    if deaths >= DEATH_CAP {
        let block = format!("BLOCK-{}02", w.slice);
        append_under(
            &fragment,
            "## Traps",
            &format!(
                "- **Dead worker ({date})**: session {} {code} {what}; second death on this slice, blocked ({block})."
                , w.session_id
            ),
        )?;
        let sessions: Vec<String> = run
            .state()
            .workers
            .iter()
            .filter(|x| x.slice == w.slice && matches!(x.outcome.as_deref(), Some("dead" | "failed")))
            .map(|x| format!("`{}` ({})", x.session_id, x.outcome.as_deref().unwrap_or("?")))
            .collect();
        let entry = format!(
            "### {block} — Two workers died on slice {nn} · slice-{nn} · open\n\n\
             **Happened:** {n} workers on slice {nn} exited without leaving it `done` or `blocked`: {sessions}. \
             The last one {code} {what}. The slice sits at `building` on the board with nothing alive on it; \
             the driver's once-only respawn (autonomy.md § Caps, hand-back once) is spent. \
             Streams and stderr are under `{dir}/`.\n\
             **Options:** 1. read the last worker's `.stderr` and `.jsonl`, fix the environment (login, PATH, quota) and set the slice back to `pending`; \
             2. build the slice by hand and set it `done`; \
             3. drop the slice from the plan if it no longer applies.\n\
             **Recommended:** 1 — a worker that dies before its first tool call is the environment's fault, not the slice's.\n",
            nn = format!("{:02}", w.slice),
            n = deaths,
            sessions = sessions.join(", "),
            dir = worker::dir(run.dir()).display(),
        );
        append_under(&fragment, "## Blockers", &entry)?;
        let slice_file = board
            .slices
            .iter()
            .find(|s| s.number == w.slice)
            .map(|s| repo.join("docs/features").join(&s.file))
            .ok_or_else(|| crate::Error::Msg(format!("slice {:02} is not on the board", w.slice)))?;
        set_slice_status(&slice_file, "blocked")?;
        run.emit(EventKind::SliceBlocked { slice: w.slice })?;
        say(
            out,
            &format!(
                "slice {:02}: worker {} died ({code}, {what}) — second death, slice blocked ({block})",
                w.slice, w.session_id
            ),
        )?;
        Ok(true)
    } else {
        append_under(
            &fragment,
            "## Traps",
            &format!(
                "- **Dead worker ({date})**: session {} {code} {what}; respawned.",
                w.session_id
            ),
        )?;
        say(
            out,
            &format!(
                "slice {:02}: worker {} died ({code}, {what}) — respawning once",
                w.slice, w.session_id
            ),
        )?;
        Ok(false)
    }
}

fn find_ref(run: &Run, session_id: &str) -> WorkerRef {
    run.state()
        .workers
        .iter()
        .rev()
        .find(|w| w.session_id == session_id)
        .cloned()
        .expect("a spawned worker is in state.workers")
}

fn finish(run: &mut Run, out: &mut dyn Write, why: &str) -> Result<Phase> {
    run.set_phase(Phase::Finished)?;
    run.emit(EventKind::RunFinished)?;
    say(out, &format!("run {} finished ({why})", run.id()))?;
    Ok(Phase::Finished)
}

fn park(run: &mut Run, out: &mut dyn Write, phase: Phase) -> Result<Phase> {
    run.set_phase(phase)?;
    say(out, &format!("run {} parked ({phase})", run.id()))?;
    Ok(phase)
}

fn say(out: &mut dyn Write, line: &str) -> Result<()> {
    writeln!(out, "{} {line}", clock::now_rfc3339()).map_err(io("driver output"))
}

/// The slice's frontmatter status as the board reports it; `""` when unknown.
pub fn slice_status(board: &Status, slice: u32) -> &str {
    board
        .slices
        .iter()
        .find(|s| s.number == slice)
        .and_then(|s| s.status.as_deref())
        .unwrap_or("")
}

/// Append `body` at the end of the `heading` section of a log fragment,
/// creating the file or the section when missing. Every line of a fragment
/// must sit under a `## ` heading (`docs log merge`).
pub fn append_under(path: &Path, heading: &str, body: &str) -> Result<()> {
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(io(format!("read {}", path.display()))(e)),
    };
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    let body = body.trim_end();
    match lines.iter().position(|l| l.trim_end() == heading) {
        Some(h) => {
            let next = lines[h + 1..]
                .iter()
                .position(|l| l.starts_with("## "))
                .map_or(lines.len(), |i| h + 1 + i);
            let mut at = next;
            while at > h + 1 && lines[at - 1].trim().is_empty() {
                at -= 1;
            }
            let mut insert: Vec<String> = Vec::new();
            if at == h + 1 || body.starts_with("### ") {
                insert.push(String::new());
            }
            insert.extend(body.lines().map(str::to_string));
            if next < lines.len() {
                insert.push(String::new());
            }
            lines.splice(at..next, insert);
        }
        None => {
            if !lines.is_empty() {
                while lines.last().is_some_and(|l| l.trim().is_empty()) {
                    lines.pop();
                }
                lines.push(String::new());
            }
            lines.push(heading.to_string());
            lines.push(String::new());
            lines.extend(body.lines().map(str::to_string));
        }
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(io(format!("mkdir {}", parent.display())))?;
    }
    fs::write(path, lines.join("\n") + "\n").map_err(io(format!("write {}", path.display())))
}

/// `status: <status>` in a slice file's frontmatter, nothing else touched.
pub fn set_slice_status(path: &Path, status: &str) -> Result<()> {
    let text = fs::read_to_string(path).map_err(io(format!("read {}", path.display())))?;
    let rest = text
        .strip_prefix("---\n")
        .ok_or_else(|| crate::Error::Msg(format!("{} has no frontmatter", path.display())))?;
    let end = rest
        .find("\n---")
        .ok_or_else(|| crate::Error::Msg(format!("{} has no frontmatter end", path.display())))?;
    let fm = &rest[..end];
    let mut replaced = false;
    let new_fm: Vec<String> = fm
        .lines()
        .map(|l| {
            if l.starts_with("status:") {
                replaced = true;
                format!("status: {status}")
            } else {
                l.to_string()
            }
        })
        .collect();
    let mut new_fm = new_fm.join("\n");
    if !replaced {
        new_fm.push_str(&format!("\nstatus: {status}"));
    }
    let out = format!("---\n{new_fm}{}", &rest[end..]);
    fs::write(path, out).map_err(io(format!("write {}", path.display())))
}

/// `runs/<id>/packs`.
pub fn packs_dir(run_dir: &Path) -> PathBuf {
    run_dir.join("packs")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_under_creates_extends_and_keeps_sections_apart() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("log-slice-01.md");
        append_under(&p, "## Traps", "- one").unwrap();
        assert_eq!(fs::read_to_string(&p).unwrap(), "## Traps\n\n- one\n");
        append_under(&p, "## Decisions", "### AD-101 — q · slice-01 · 2026-10-10\n\n**Chose:** x").unwrap();
        append_under(&p, "## Traps", "- two").unwrap();
        let text = fs::read_to_string(&p).unwrap();
        assert_eq!(
            text,
            "## Traps\n\n- one\n- two\n\n## Decisions\n\n### AD-101 — q · slice-01 · 2026-10-10\n\n**Chose:** x\n"
        );
        append_under(&p, "## Blockers", "### BLOCK-102 — w · slice-01 · open\n\n**Happened:** y").unwrap();
        let text = fs::read_to_string(&p).unwrap();
        assert!(text.ends_with("## Blockers\n\n### BLOCK-102 — w · slice-01 · open\n\n**Happened:** y\n"), "{text}");
        assert!(!text.contains("\n\n\n"), "{text}");
    }

    #[test]
    fn set_slice_status_touches_only_the_status_line() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("slice-01-a.md");
        fs::write(&p, "---\nslice: 01\nstatus: building\ndepends_on: []\n---\n\n# A\n\nstatus: not this\n").unwrap();
        set_slice_status(&p, "blocked").unwrap();
        assert_eq!(
            fs::read_to_string(&p).unwrap(),
            "---\nslice: 01\nstatus: blocked\ndepends_on: []\n---\n\n# A\n\nstatus: not this\n"
        );
    }
}
