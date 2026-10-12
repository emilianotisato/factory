//! The checkpoint (PRD §6 FR-4), run by the driver once the board says
//! `checkpoint_due` and the lanes are drained. The gate is the driver's: the
//! declared command through `sh -c` in the repo root, stdout and stderr to
//! `runs/<id>/gates/<n>-<attempt>.log`, a `gates` row per run (source
//! `driver`) or per cache hit (`cached = 1`, `minutes = 0`). Red spawns a
//! fix worker with the failing output, commits its files by pathspec as
//! `fix(NNN): gate red at checkpoint n` and reruns; three reds in a row block the
//! run on a `BLOCK-n` in `agent-log.md` (phase `checkpoint`). Green folds
//! the slice logs into `agent-log.md` (FR-5), then a checkpoint worker reads
//! the transcript, drives the covered Prove rows and writes `CP-n`; the
//! driver renders and commits `docs(NNN): checkpoint n`. A worker that dies gets
//! one more spawn, then the block; a worker that blocks parks the run.
//!
//! Every step is re-entrant from the board and the run dir alone: a resume
//! (FR-1 "an unverified checkpoint runs first") lands here before any spawn,
//! emits `checkpoint_resumed` and reuses the gate cache.
use std::collections::HashSet;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Instant;

use crate::answers;
use crate::commit::{self, Fold, Pathspec};
use crate::config::Config;
use crate::db::GateInsert;
use crate::docs::{Docs, Status};
use crate::error::io;
use crate::events::{Event, EventKind};
use crate::gatecache::{self, Entry, GateCache};
use crate::pack::Pack;
use crate::quota::{self, Hold, Limit};
use crate::run::{Phase, Run, WorkerOutcome};
use crate::scheduler::{self, DEATH_CAP, append_under, pad_list, say, short};
use crate::stream::Summary;
use crate::worker::{Exit, Job, Worker};
use crate::{Result, clock, metrics};

/// Consecutive red gates at one checkpoint before the run blocks.
pub const RED_CAP: u32 = 3;
/// `runs/<id>/gates`.
pub const GATES_DIR: &str = "gates";
/// The title prefix of a merge-refusal block in `agent-log.md`.
pub const MERGE_REFUSED: &str = "log merge refused";

/// How the checkpoint left the run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// `CP-n` is on the board and committed: the loop reads the board again.
    Done,
    /// The run waits for the owner on `id` in `agent-log.md`. `fresh` is a
    /// block written just now (park anew); otherwise the block was already
    /// open when the checkpoint was entered and the park stands.
    Parked {
        id: String,
        what: String,
        fresh: bool,
    },
    /// `factory stop` arrived between steps.
    Stopped,
    /// FR-9: the weighted tokens passed the budget between steps; the
    /// scheduler parks the run and the checkpoint runs again on resume.
    Budget { weighted: u64, budget: u64 },
}

/// Run the checkpoint the board says is due.
pub fn run(
    run: &mut Run,
    docs: &Docs,
    cfg: &Config,
    board: &Status,
    out: &mut dyn Write,
) -> Result<Outcome> {
    let feature = run.state().feature.clone();
    let repo = run.state().repo.clone();
    let feature_dir = repo.join(&board.dir);
    let agent_log = feature_dir.join("agent-log.md");

    // A checkpoint block the owner has not answered: the park stands.
    if let Some(open) = open_checkpoint_blocks(&agent_log)?.into_iter().next() {
        return Ok(Outcome::Parked {
            what: park_line(&open.0, &open.1),
            id: open.0,
            fresh: false,
        });
    }

    let n = board.checkpoints.iter().map(|c| c.n).max().unwrap_or(0) + 1;
    let covers = board.unverified.clone();
    // A second check: `docs new` creates agent-log.md now (AD-503), but a
    // feature made by a vendored docs.mjs older than that fix has none.
    if !agent_log.exists() {
        fs::write(&agent_log, format!("# Agent log — PRD-{feature}\n"))
            .map_err(io(format!("write {}", agent_log.display())))?;
    }

    let (started, done) = run
        .events()?
        .iter()
        .fold((false, false), |(s, d), e| match &e.kind {
            EventKind::CheckpointStarted { n: m, .. }
            | EventKind::CheckpointResumed { n: m, .. } => (s || *m == n, d),
            EventKind::CheckpointDone { n: m, .. } => (s, d || *m == n),
            _ => (s, d),
        });
    let resumed = started && !done;
    run.emit(if resumed {
        EventKind::CheckpointResumed {
            n,
            covers: covers.clone(),
        }
    } else {
        EventKind::CheckpointStarted {
            n,
            covers: covers.clone(),
        }
    })?;
    run.set_phase(Phase::Checkpoint)?;
    say(
        out,
        &format!(
            "checkpoint {n} (covers {}) — {}",
            pad_list(&covers),
            if resumed { "resumed" } else { "started" }
        ),
    )?;

    // A merge that will refuse blocks now, not after the gate is paid for.
    if let Some(detail) = commit::check_logs(docs, &repo, board, &feature)? {
        let (id, title) = block_on_refusal(run, &feature_dir, &feature, detail, out)?;
        return Ok(Outcome::Parked {
            what: park_line(&id, &title),
            id,
            fresh: true,
        });
    }

    // The gate, until green or three reds. A cached red is reused only
    // between fix workers of this entry: an owner's answer or a resume runs
    // the gate again whatever the tree says (the fix may be outside it).
    // The reds count from the events, so a resumed driver goes on from the
    // reds its predecessor saw.
    let gate = docs.gate()?.trim().to_string();
    let mut ran_red = false;
    let green_log: PathBuf;
    let mut green_cached;
    loop {
        if run.state().stop_requested {
            return Ok(Outcome::Stopped);
        }
        let result = gate_once(run, &repo, &gate, n, ran_red, out)?;
        if !result.red {
            green_log = result.log;
            green_cached = result.cached;
            break;
        }
        ran_red = true;
        let (reds, fixes) = reds_in_a_row(run, n)?;
        if reds >= RED_CAP {
            let tail = fs::read_to_string(&result.log).unwrap_or_default();
            let line = tail
                .lines()
                .rev()
                .map(str::trim)
                .find(|l| !l.is_empty())
                .unwrap_or("no output")
                .to_string();
            let id = format!("BLOCK-{}", lowest_free_block(&feature_dir)?);
            let title =
                answers::block_title(&format!("gate red three times at checkpoint {n}: {line}"));
            let quoted: String = crate::pack::tail(&tail, 40)
                .lines()
                .map(|l| format!("  {l}\n"))
                .collect();
            let entry = format!(
                "### {id} — {title} · checkpoint · open\n\n\
                 **Happened:** the gate (`{gate}`) was red {reds} times in a row at checkpoint {n}; \
                 {fixes} fix worker(s) ran between the runs. The last transcript ({log}) ends:\n{quoted}\
                 **Options:**\n\
                 1. fix the failure by hand, leave the tree as the commit should be, answer this block — the gate runs again\n\
                 2. revert the covered slices' commits and set their slices back to `pending`\n\
                 **Recommended:** 1 — the gate stayed red through {fixes} fix worker(s); the cause is outside what the failing output names.\n",
                log = result.log.display(),
            );
            append_under(&agent_log, "## Blockers", &entry)?;
            run.emit(EventKind::CheckpointBlocked {
                n,
                block_id: id.clone(),
                question: title.clone(),
            })?;
            say(
                out,
                &format!(
                    "checkpoint {n}: gate red {reds} times — {id} written to agent-log.md: {line}"
                ),
            )?;
            return Ok(Outcome::Parked {
                what: park_line(&id, &title),
                id,
                fresh: true,
            });
        }
        // quota: rate_limited is not a red (FR-9). A limited fix worker
        // pauses the run; the same gate attempt gets a fresh fix worker once
        // the pause lifts, and `reds` does not move.
        loop {
            if let Some(o) = await_quota(run, out)? {
                return Ok(o);
            }
            if fix_once(run, docs, cfg, board, n, result.attempt, &result.log, out)? {
                break;
            }
        }
    }

    // Green: fold the slice logs before the worker reads agent-log.md.
    match commit::fold_logs(docs, &repo, board, &feature)? {
        Fold::Nothing => {}
        Fold::Folded { fragments } => {
            say(
                out,
                &format!(
                    "slice logs merged into agent-log.md ({})",
                    fragments.join(", ")
                ),
            )?;
            run.emit(EventKind::LogMerged { fragments })?;
        }
        Fold::Refused { detail } => {
            let (id, title) = block_on_refusal(run, &feature_dir, &feature, detail, out)?;
            return Ok(Outcome::Parked {
                what: park_line(&id, &title),
                id,
                fresh: true,
            });
        }
    }

    // The checkpoint worker: once more after a death, then the block.
    loop {
        if let Some(o) = await_quota(run, out)? {
            return Ok(o);
        }
        let before = spawned_before_answer(&run.events()?, n);
        let mine = |w: &crate::run::WorkerRef| {
            w.role == "checkpoint"
                && w.checkpoint == Some(n)
                && !before.contains(w.session_id.as_str())
        };
        let deaths = run
            .state()
            .workers
            .iter()
            .filter(|w| mine(w) && w.died())
            .count();
        if deaths >= DEATH_CAP {
            let id = format!("BLOCK-{}", lowest_free_block(&feature_dir)?);
            let title = format!("Two checkpoint workers died at checkpoint {n}");
            let sessions: Vec<String> = run
                .state()
                .workers
                .iter()
                .filter(|w| mine(w) && w.died())
                .map(|w| {
                    format!(
                        "`{}` ({})",
                        w.session_id,
                        w.outcome.as_deref().unwrap_or("?")
                    )
                })
                .collect();
            let entry = format!(
                "### {id} — {title} · checkpoint · open\n\n\
                 **Happened:** {deaths} checkpoint workers exited without writing `CP-{n}` or a block: {sessions}. \
                 The gate is green and the slice logs are merged; the driver's once-only respawn is spent. \
                 Streams and stderr are under `{dir}/`.\n\
                 **Options:** 1. read the last worker's `.stderr` and `.jsonl`, fix the environment (login, PATH, quota) and answer this block — a fresh worker runs; \
                 2. drive the Prove rows by hand, write `CP-{n}` under `## Checkpoints` and answer this block — the driver commits the checkpoint.\n\
                 **Recommended:** 1 — a worker that dies before its first tool call is the environment's fault, not the checkpoint's.\n",
                sessions = sessions.join(", "),
                dir = crate::worker::dir(run.dir()).display(),
            );
            append_under(&agent_log, "## Blockers", &entry)?;
            run.emit(EventKind::CheckpointBlocked {
                n,
                block_id: id.clone(),
                question: title.clone(),
            })?;
            say(
                out,
                &format!(
                    "checkpoint {n}: second checkpoint worker died — {id} written to agent-log.md"
                ),
            )?;
            return Ok(Outcome::Parked {
                what: park_line(&id, &title),
                id,
                fresh: true,
            });
        }
        let attempt = run.state().workers.iter().filter(|w| mine(w)).count() as u32 + 1;
        let spawn = run.state().workers.len() as u32 + 1;
        let packs = scheduler::packs_dir(run.dir());
        let pack = Pack::for_checkpoint(
            docs,
            board,
            n,
            &covers,
            &green_log,
            green_cached,
            &packs,
            spawn,
        )?;
        let open_before: HashSet<String> = open_checkpoint_blocks(&agent_log)?
            .into_iter()
            .map(|(id, _)| id)
            .collect();
        let changed_before = scheduler::snapshot(run, &scheduler::checkpoint_key(n, "checkpoint"))?;
        let worker = Worker::spawn(
            run,
            Job::Checkpoint { n, attempt },
            &pack,
            &cfg.workers.checkpoint,
            &[],
        )?;
        let session_id = worker.session_id.clone();
        let row_id = worker.row_id;
        say(
            out,
            &format!(
                "checkpoint {n}: worker {session_id} spawned (pid {}, attempt {attempt}, pack {})",
                worker.pid,
                pack.path.display()
            ),
        )?;
        let exit = worker.wait()?;
        green_cached = true; // a respawn reads the same transcript
        let after = docs.status(&feature)?;
        if after.checkpoints.iter().any(|c| c.n == n) {
            let spec = Pathspec::from_stream(&exit.summary, board, &repo);
            let sha = commit_checkpoint(run, docs, board, n, row_id, &spec, &changed_before, out)?;
            settle(run, row_id, &session_id, &exit.summary, WorkerOutcome::Done)?;
            run.emit(EventKind::CheckpointDone { n, sha })?;
            return Ok(Outcome::Done);
        }
        let open_now = open_checkpoint_blocks(&agent_log)?;
        if let Some((id, title)) = open_now
            .into_iter()
            .find(|(id, _)| !open_before.contains(id))
        {
            settle(
                run,
                row_id,
                &session_id,
                &exit.summary,
                WorkerOutcome::Blocked,
            )?;
            run.emit(EventKind::CheckpointBlocked {
                n,
                block_id: id.clone(),
                question: title.clone(),
            })?;
            say(
                out,
                &format!(
                    "checkpoint {n}: worker {session_id} blocked — {id} in agent-log.md: {title}"
                ),
            )?;
            return Ok(Outcome::Parked {
                what: park_line(&id, &title),
                id,
                fresh: true,
            });
        }
        // FR-9: a limit is not a death — pause, then a fresh worker.
        if let Some(limit) = Limit::from(&exit.summary) {
            settle(
                run,
                row_id,
                &session_id,
                &exit.summary,
                WorkerOutcome::RateLimited,
            )?;
            let until = quota::pause(run, &limit, out)?;
            say(
                out,
                &format!(
                    "checkpoint {n}: worker {session_id} hit a usage limit ({}) — paused until {}, a fresh worker follows",
                    limit.literal,
                    clock::rfc3339(until)
                ),
            )?;
            continue;
        }
        settle_dead(run, &agent_log, n, &session_id, row_id, &exit, out)?;
    }
}

/// FR-9 between the checkpoint's steps: wait out a pause (phase `paused`,
/// then back to `checkpoint`), hand a spent budget up to the scheduler, or
/// go on (`None`).
fn await_quota(run: &mut Run, out: &mut dyn Write) -> Result<Option<Outcome>> {
    loop {
        run.reload()?;
        if run.state().stop_requested {
            return Ok(Some(Outcome::Stopped));
        }
        match quota::hold(run)? {
            Some(Hold::Budget { weighted, budget }) => {
                return Ok(Some(Outcome::Budget { weighted, budget }));
            }
            Some(Hold::Paused { until }) => {
                if run.state().phase != Phase::Paused {
                    run.set_phase(Phase::Paused)?;
                }
                quota::pause_tick(run, until)?;
            }
            None => {
                if run.state().paused_until.is_some() {
                    quota::resume(run, Phase::Checkpoint, out)?;
                }
                return Ok(None);
            }
        }
    }
}

/// One gate decision.
struct GateResult {
    red: bool,
    cached: bool,
    attempt: u32,
    /// The transcript, absolute.
    log: PathBuf,
}

/// The cache, or `sh -c <gate>`; either way a `gates` row and an event. A
/// cached red counts only with `trust_red` (a fix worker just ran on it).
/// The gate's reds in a row at checkpoint `n`, and the fix workers that
/// ran between them (a rate-limited one did not), read from the events:
/// since the gate was last green there, or since the owner answered one of
/// this checkpoint's blocks.
fn reds_in_a_row(run: &Run, n: u32) -> Result<(u32, u32)> {
    let limited: HashSet<String> = run
        .state()
        .workers
        .iter()
        .filter(|w| w.outcome.as_deref() == Some(WorkerOutcome::RateLimited.as_str()))
        .map(|w| w.session_id.clone())
        .collect();
    let mut blocks: HashSet<String> = HashSet::new();
    let (mut reds, mut fixes) = (0u32, 0u32);
    for e in run.events()? {
        match e.kind {
            EventKind::GateRun { n: m, red, .. } | EventKind::GateCached { n: m, red, .. }
                if m == n =>
            {
                if red {
                    reds += 1;
                } else {
                    (reds, fixes) = (0, 0);
                }
            }
            EventKind::FixSpawned {
                n: m, session_id, ..
            } if m == n && !limited.contains(&session_id) => fixes += 1,
            EventKind::CheckpointBlocked { n: m, block_id, .. } if m == n => {
                blocks.insert(block_id);
            }
            EventKind::BlockAnswered { id, .. } if blocks.contains(&id) => {
                (reds, fixes) = (0, 0);
            }
            _ => {}
        }
    }
    Ok((reds, fixes))
}

fn gate_once(
    run: &mut Run,
    repo: &Path,
    gate: &str,
    n: u32,
    trust_red: bool,
    out: &mut dyn Write,
) -> Result<GateResult> {
    let attempt = run
        .db()
        .gates(run.id().as_str())?
        .iter()
        .filter(|g| g.checkpoint == Some(n))
        .count() as u32
        + 1;
    let index = run.dir().join("gate-index");
    let key = gatecache::tree_key(repo, &index, gate)?;
    let mut cache = GateCache::load(run.dir())?;
    let at = clock::now_rfc3339();
    let fits = |h: &Entry| (!h.red || trust_red) && h.checkpoint.is_none_or(|c| c == n);
    if let Some(hit) = cache.get(&key).filter(|h| fits(h)).cloned() {
        run.db().insert_gate(&GateInsert {
            run_id: run.id().as_str(),
            source: "driver",
            kind: "checkpoint",
            cmd: gate,
            red: hit.red,
            minutes: 0.0,
            at: &at,
            worker_id: None,
            cached: true,
            checkpoint: Some(n),
            attempt: Some(attempt),
        })?;
        run.emit(EventKind::GateCached {
            n,
            attempt,
            red: hit.red,
            log: hit.log.clone(),
        })?;
        say(
            out,
            &format!(
                "checkpoint {n}: gate attempt {attempt} cached — {} (tree unchanged since {})",
                if hit.red { "red" } else { "green" },
                hit.log
            ),
        )?;
        return Ok(GateResult {
            red: hit.red,
            cached: true,
            attempt,
            log: run.dir().join(&hit.log),
        });
    }

    let rel = format!("{GATES_DIR}/{n}-{attempt}.log");
    let log = run.dir().join(&rel);
    if let Some(parent) = log.parent() {
        fs::create_dir_all(parent).map_err(io(format!("mkdir {}", parent.display())))?;
    }
    let file = File::create(&log).map_err(io(format!("create {}", log.display())))?;
    let err = file
        .try_clone()
        .map_err(io(format!("clone {}", log.display())))?;
    say(
        out,
        &format!("checkpoint {n}: gate attempt {attempt} — `{gate}`"),
    )?;
    let started = Instant::now();
    let status = Command::new("sh")
        .arg("-c")
        .arg(gate)
        .current_dir(repo)
        .env_remove("FACTORY_LANE")
        .stdin(Stdio::null())
        .stdout(Stdio::from(file))
        .stderr(Stdio::from(err))
        .status()
        .map_err(|e| crate::Error::Tool {
            cmd: format!("sh -c {gate}"),
            detail: format!("could not start the gate: {e}"),
        })?;
    let minutes = started.elapsed().as_secs_f64() / 60.0;
    let red = !status.success();
    run.db().insert_gate(&GateInsert {
        run_id: run.id().as_str(),
        source: "driver",
        kind: "checkpoint",
        cmd: gate,
        red,
        minutes,
        at: &at,
        worker_id: None,
        cached: false,
        checkpoint: Some(n),
        attempt: Some(attempt),
    })?;
    // A green run on a tree with changes outside `docs/` tested what HEAD
    // does not hold: it holds for this checkpoint only (Xb7), where a rerun
    // would test the same tree. Run B of the FR-11 A/B (2026-10-11) reran a
    // 7-minute green after the owner's answer for want of it.
    let dirty = !red && commit::dirty_outside_docs(repo)?;
    cache.put(
        &key,
        Entry {
            red,
            minutes,
            at: at.clone(),
            log: rel.clone(),
            checkpoint: dirty.then_some(n),
        },
    )?;
    run.emit(EventKind::GateRun {
        n,
        attempt,
        red,
        minutes,
        log: rel.clone(),
    })?;
    say(
        out,
        &format!(
            "checkpoint {n}: gate attempt {attempt} {} in {:.2} min ({rel})",
            if red { "red" } else { "green" },
            minutes
        ),
    )?;
    Ok(GateResult {
        red,
        cached: false,
        attempt,
        log,
    })
}

/// A fix worker on the red transcript; what it changed is committed however
/// it ended — done, dead or on a usage limit — and before its outcome is
/// saved, so a driver killed in between leaves it to the resume. A dead
/// worker is settled and counted by the gate that follows (a tree it did not
/// change is a cache hit, red again). `false` when the worker hit a usage
/// limit (FR-9): the run is paused and this attempt gets another fix worker,
/// no gate run between.
#[allow(clippy::too_many_arguments)]
fn fix_once(
    run: &mut Run,
    docs: &Docs,
    cfg: &Config,
    board: &Status,
    n: u32,
    attempt: u32,
    gate_log: &Path,
    out: &mut dyn Write,
) -> Result<bool> {
    let spawn = run.state().workers.len() as u32 + 1;
    let packs = scheduler::packs_dir(run.dir());
    let pack = Pack::for_fix(docs, board, n, attempt, gate_log, &packs, spawn)?;
    let before = scheduler::snapshot(run, &scheduler::checkpoint_key(n, "fix"))?;
    let worker = Worker::spawn(
        run,
        Job::Fix {
            checkpoint: n,
            attempt,
        },
        &pack,
        &cfg.workers.fix,
        &[],
    )?;
    let session_id = worker.session_id.clone();
    let row_id = worker.row_id;
    say(
        out,
        &format!(
            "checkpoint {n}: fix worker {session_id} spawned on gate attempt {attempt} (pid {}, pack {})",
            worker.pid,
            pack.path.display()
        ),
    )?;
    let exit = worker.wait()?;
    let limit = Limit::from(&exit.summary);
    let dead = limit.is_none() && exit.summary.is_dead();
    commit_fix(
        run,
        board,
        n,
        attempt,
        row_id,
        &session_id,
        &exit.summary,
        &before,
        !dead && limit.is_none(),
        out,
    )?;
    if let Some(limit) = limit {
        settle(
            run,
            row_id,
            &session_id,
            &exit.summary,
            WorkerOutcome::RateLimited,
        )?;
        let until = quota::pause(run, &limit, out)?;
        say(
            out,
            &format!(
                "checkpoint {n}: fix worker {session_id} hit a usage limit ({}) — paused until {}, the fix runs again",
                limit.literal,
                clock::rfc3339(until)
            ),
        )?;
        return Ok(false);
    }
    if dead {
        settle(run, row_id, &session_id, &exit.summary, WorkerOutcome::Dead)?;
        run.emit(EventKind::WorkerDead {
            slice: None,
            session_id: session_id.clone(),
            exit_code: exit.code,
        })?;
        say(
            out,
            &format!(
                "checkpoint {n}: fix worker {session_id} died ({}) — the gate runs again",
                exit.code
                    .map_or("code unknown".to_string(), |c| format!("exit {c}"))
            ),
        )?;
        return Ok(true);
    }
    settle(run, row_id, &session_id, &exit.summary, WorkerOutcome::Done)?;
    Ok(true)
}

/// A fix worker's commit: the paths its stream named (less the protected
/// files) and the tracked files it changed unnamed — nothing else is live at
/// a checkpoint (AD-20007); its new unnamed files are listed, not committed.
/// `fix(NNN): gate red at checkpoint n`. A worker that changed nothing says so
/// only when `say_nothing`. The scheduler settles a dead driver's fix worker
/// through here too.
#[allow(clippy::too_many_arguments)]
pub fn commit_fix(
    run: &mut Run,
    board: &Status,
    n: u32,
    attempt: u32,
    row_id: i64,
    session_id: &str,
    summary: &Summary,
    before: &HashSet<String>,
    say_nothing: bool,
    out: &mut dyn Write,
) -> Result<()> {
    let repo = run.state().repo.clone();
    let spec = Pathspec::from_stream(summary, board, &repo);
    for path in &spec.protected {
        say(
            out,
            &format!(
                "checkpoint {n}: fix worker wrote {path}, which no fix may — left out of the commit"
            ),
        )?;
    }
    let skip: HashSet<String> = spec.paths.iter().chain(&spec.protected).cloned().collect();
    let found = scheduler::unnamed(run, board, before, &skip, &spec.dirs, true)?;
    note_leftovers(run, n, "fix", &found, out)?;
    let mut paths = spec.paths.clone();
    paths.extend(found.sweep);
    match commit::commit(
        &repo,
        &paths,
        &commit::subject(board, "fix", &format!("gate red at checkpoint {n}")),
    )? {
        Some(c) => {
            run.db().set_worker_commit(row_id, &c.sha)?;
            say(
                out,
                &format!(
                    "checkpoint {n}: fix committed {} — {} file(s): {}",
                    short(&c.sha),
                    c.files.len(),
                    c.files.join(", ")
                ),
            )?;
            run.emit(EventKind::FixCommitted {
                n,
                attempt,
                sha: c.sha,
                files: c.files,
            })?;
        }
        None if say_nothing => say(
            out,
            &format!("checkpoint {n}: fix worker {session_id} changed nothing to commit"),
        )?,
        None => {}
    }
    Ok(())
}

/// The unnamed changes of a checkpoint's worker, said and logged: the swept
/// ones go in its commit, the listed ones stay in the tree.
fn note_leftovers(
    run: &mut Run,
    n: u32,
    role: &str,
    found: &scheduler::Unnamed,
    out: &mut dyn Write,
) -> Result<()> {
    if !found.sweep.is_empty() {
        say(
            out,
            &format!(
                "checkpoint {n}: {role} worker changed {} tracked file(s) no tool call named — in its commit: {}",
                found.sweep.len(),
                found.sweep.join(", ")
            ),
        )?;
    }
    if !found.listed.is_empty() {
        run.emit(EventKind::CheckpointLeftovers {
            n,
            role: role.to_string(),
            files: found.listed.clone(),
            blobs: found.blobs.clone(),
        })?;
        say(
            out,
            &format!(
                "checkpoint {n}: {role} worker left {} file(s) no tool call named — not committed: {}",
                found.listed.len(),
                found.listed.join(", ")
            ),
        )?;
    }
    Ok(())
}

/// `render --write`, then one commit of the feature folder, `docs/index.md`
/// and whatever else the checkpoint worker's stream named.
#[allow(clippy::too_many_arguments)]
pub fn commit_checkpoint(
    run: &mut Run,
    docs: &Docs,
    board: &Status,
    n: u32,
    row_id: i64,
    spec: &Pathspec,
    before: &HashSet<String>,
    out: &mut dyn Write,
) -> Result<Option<String>> {
    let repo = run.state().repo.clone();
    docs.render_write()?;
    let mut want = vec![board.dir.clone(), "docs/index.md".to_string()];
    want.extend(spec.paths.iter().cloned());
    let mut files = commit::expand(&repo, &want)?;
    let skip: HashSet<String> = files.iter().chain(&spec.protected).cloned().collect();
    let found = scheduler::unnamed(run, board, before, &skip, &spec.dirs, true)?;
    note_leftovers(run, n, "checkpoint", &found, out)?;
    files.extend(found.sweep);
    match commit::commit(
        &repo,
        &files,
        &commit::subject(board, "docs", &format!("checkpoint {n}")),
    )? {
        Some(c) => {
            run.db().set_worker_commit(row_id, &c.sha)?;
            say(
                out,
                &format!(
                    "checkpoint {n} done — committed {} ({} file(s))",
                    short(&c.sha),
                    c.files.len()
                ),
            )?;
            Ok(Some(c.sha))
        }
        None => {
            say(out, &format!("checkpoint {n} done — nothing to commit"))?;
            Ok(None)
        }
    }
}

/// A refused `docs log merge` (an AD clash the driver never fixes by
/// renumbering): a `BLOCK-n` in `agent-log.md`, phase `checkpoint`.
fn block_on_refusal(
    run: &mut Run,
    feature_dir: &Path,
    feature: &str,
    detail: String,
    out: &mut dyn Write,
) -> Result<(String, String)> {
    let agent_log = feature_dir.join("agent-log.md");
    let id = format!("BLOCK-{}", lowest_free_block(feature_dir)?);
    let clashes: Vec<&str> = detail
        .lines()
        .map(str::trim)
        .filter_map(|l| l.strip_prefix("✗ "))
        .collect();
    let question = if clashes.is_empty() {
        detail
            .lines()
            .map(str::trim)
            .find(|l| !l.is_empty())
            .unwrap_or("docs log merge failed")
            .to_string()
    } else {
        clashes.join("; ")
    };
    let quoted: String = detail.lines().map(|l| format!("  {l}\n")).collect();
    let title = answers::block_title(&format!("{MERGE_REFUSED}: {question}"));
    let entry = format!(
        "### {id} — {title} · checkpoint · open\n\n\
         **Happened:** the lanes are drained; `docs log merge {feature}` \
         merged nothing:\n{quoted}\
         **Options:**\n\
         1. renumber the clashing entry in the later fragment into its slice's range (`NN01`, `NN02`, …)\n\
         2. drop the later entry if it only repeats the earlier one\n\
         **Recommended:** 1 — the driver never renumbers (FR-5); each builder's seed is its slice's range. \
         Answer this block once the fragment is fixed and the merge runs again.\n"
    );
    append_under(&agent_log, "## Blockers", &entry)?;
    run.emit(EventKind::MergeRefused {
        block_id: id.clone(),
        detail,
    })?;
    say(
        out,
        &format!("log merge refused — {id} written to agent-log.md: {question}"),
    )?;
    Ok((id, title))
}

/// A checkpoint worker gone without `CP-n` or a block: a Traps line in
/// `agent-log.md`; the caller respawns or blocks on the count.
fn settle_dead(
    run: &mut Run,
    agent_log: &Path,
    n: u32,
    session_id: &str,
    row_id: i64,
    exit: &Exit,
    out: &mut dyn Write,
) -> Result<()> {
    let outcome = if exit.summary.is_dead() {
        WorkerOutcome::Dead
    } else {
        WorkerOutcome::Failed
    };
    settle(run, row_id, session_id, &exit.summary, outcome)?;
    run.emit(EventKind::WorkerDead {
        slice: None,
        session_id: session_id.to_string(),
        exit_code: exit.code,
    })?;
    let code = exit.code.map_or("exited (code unknown)".to_string(), |c| {
        format!("exited {c}")
    });
    let what = if exit.summary.is_dead() {
        "with no result"
    } else {
        "with a result but neither CP entry nor block"
    };
    let date = clock::now_rfc3339()[..10].to_string();
    append_under(
        agent_log,
        "## Traps",
        &format!(
            "- **Dead checkpoint worker ({date})**: checkpoint {n}, session {session_id} {code} {what}."
        ),
    )?;
    say(
        out,
        &format!("checkpoint {n}: worker {session_id} died ({code}, {what})"),
    )?;
    Ok(())
}

fn settle(
    run: &mut Run,
    row_id: i64,
    session_id: &str,
    summary: &Summary,
    outcome: WorkerOutcome,
) -> Result<()> {
    run.db()
        .finish_worker(row_id, summary, outcome.as_str(), &clock::now_rfc3339())?;
    metrics::record_exit(run, row_id, summary)?;
    run.set_worker_outcome(session_id, outcome.as_str())
}

/// `(id, title)` of every unresolved `BLOCK-n` with phase `checkpoint` in
/// `agent-log.md`, in file order.
pub fn open_checkpoint_blocks(agent_log: &Path) -> Result<Vec<(String, String)>> {
    Ok(answers::entries(agent_log)?
        .into_iter()
        .filter(|e| !e.resolved && e.phase.as_deref() == Some("checkpoint"))
        .map(|e| (e.id, e.title))
        .collect())
}

/// The index into `events` just past the latest `block_answered` for one of
/// checkpoint `n`'s own blocks (its `checkpoint_blocked` ids); 0 when the
/// owner answered none. The death cap and the red count start there: an
/// answer to another block naming no slice (a refused merge, a closeout
/// block, another checkpoint's) resets neither.
fn since_answer(events: &[Event], n: u32) -> usize {
    let mut mine: HashSet<&str> = HashSet::new();
    let mut from = 0;
    for (i, e) in events.iter().enumerate() {
        match &e.kind {
            EventKind::CheckpointBlocked { n: m, block_id, .. } if *m == n => {
                mine.insert(block_id.as_str());
            }
            EventKind::BlockAnswered {
                id, slice: None, ..
            } if mine.contains(id.as_str()) => from = i + 1,
            _ => {}
        }
    }
    from
}

/// The sessions of checkpoint `n`'s workers (fix and checkpoint) spawned
/// before the owner's latest answer to one of its blocks: spent, they count
/// toward no cap. A worker with no spawn event (a kill inside the spawn) is
/// not among them, so it counts.
fn spawned_before_answer(events: &[Event], n: u32) -> HashSet<String> {
    events[..since_answer(events, n)]
        .iter()
        .filter_map(|e| match &e.kind {
            EventKind::CheckpointSpawned {
                n: m, session_id, ..
            }
            | EventKind::FixSpawned {
                n: m, session_id, ..
            } if *m == n => Some(session_id.clone()),
            _ => None,
        })
        .collect()
}

fn park_line(id: &str, title: &str) -> String {
    format!(
        "checkpoint blocked — {title}: {id} in agent-log.md waits for the owner: `factory answer {id} \"<text>\"`"
    )
}

/// The lowest `n ≥ 1` no `BLOCK-n` in the feature's logs uses. A builder's
/// blocks are `NN01`, `NN02`, …; the driver's own stay below 100 (AD-501).
pub fn lowest_free_block(feature_dir: &Path) -> Result<u32> {
    let mut used: HashSet<u32> = HashSet::new();
    for path in answers::log_files(feature_dir)? {
        used.extend(answers::entries(&path)?.into_iter().map(|e| e.n));
    }
    Ok((1..)
        .find(|n| !used.contains(n))
        .expect("an unused block number"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(kind: EventKind) -> Event {
        Event {
            seq: 0,
            ts: String::new(),
            kind,
        }
    }

    fn spawned(n: u32, session: &str) -> Event {
        ev(EventKind::CheckpointSpawned {
            n,
            attempt: 1,
            session_id: session.to_string(),
            pid: 1,
        })
    }

    fn blocked(n: u32, id: &str) -> Event {
        ev(EventKind::CheckpointBlocked {
            n,
            block_id: id.to_string(),
            question: "q".to_string(),
        })
    }

    fn answered(id: &str) -> Event {
        ev(EventKind::BlockAnswered {
            id: id.to_string(),
            slice: None,
            text: "a".to_string(),
        })
    }

    #[test]
    fn only_an_answer_to_this_checkpoints_block_spends_its_workers() {
        let mut events = vec![
            spawned(1, "a"),
            ev(EventKind::MergeRefused {
                block_id: "BLOCK-1".to_string(),
                detail: String::new(),
            }),
            answered("BLOCK-1"),
        ];
        // A refused merge's answer names no slice, but it is not this
        // checkpoint's block: worker `a` still counts.
        assert_eq!(since_answer(&events, 1), 0);
        assert!(spawned_before_answer(&events, 1).is_empty());

        events.extend([blocked(1, "BLOCK-2"), answered("BLOCK-2")]);
        assert_eq!(since_answer(&events, 1), 5);
        assert_eq!(
            spawned_before_answer(&events, 1),
            HashSet::from(["a".to_string()])
        );

        // Checkpoint 2's block, answered, spends none of checkpoint 1's.
        events.extend([spawned(1, "b"), blocked(2, "BLOCK-3"), answered("BLOCK-3")]);
        assert_eq!(since_answer(&events, 1), 5);
        assert_eq!(
            spawned_before_answer(&events, 1),
            HashSet::from(["a".to_string()])
        );
        assert_eq!(since_answer(&events, 2), 8);
    }
}
