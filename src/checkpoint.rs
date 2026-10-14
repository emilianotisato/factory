//! Every step is re-entrant from the board and the run dir alone: a resume lands here before any spawn.
use std::collections::HashSet;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Instant;

use crate::answers;
use crate::commit;
use crate::config::Config;
use crate::db::GateInsert;
use crate::docs::{Docs, Status};
use crate::error::io;
use crate::events::{Event, EventKind};
use crate::gatecache::{self, Entry, GateCache};
use crate::land::{self, Landing};
use crate::pack::Pack;
use crate::quota::{self, Hold, Limit};
use crate::run::{Phase, Run, WorkerOutcome};
use crate::scheduler::{self, DEATH_CAP, pad_list, say, short};
use crate::stream::Summary;
use crate::worker::{Exit, Job, Worker};
use crate::{Result, clock, lanes, metrics, paths};

pub const RED_CAP: u32 = 3;
pub const GATES_DIR: &str = "gates";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Done,
    /// `fresh`: the block was written just now; else it was open on entry and the park stands.
    Parked {
        id: String,
        what: String,
        fresh: bool,
    },
    Stopped,
    Budget {
        weighted: u64,
        budget: u64,
    },
}

pub fn run(
    run: &mut Run,
    docs: &Docs,
    cfg: &Config,
    board: &Status,
    lane: u32,
    out: &mut dyn Write,
) -> Result<Outcome> {
    let feature = run.state().feature.clone();
    let repo = run.state().repo.clone();
    let feature_dir = repo.join(&board.dir);

    if let Some(open) = open_checkpoint_blocks(&feature_dir)?.into_iter().next() {
        return Ok(Outcome::Parked {
            what: park_line(&open.0, &open.1),
            id: open.0,
            fresh: false,
        });
    }

    let n = board.checkpoints.iter().map(|c| c.n).max().unwrap_or(0) + 1;
    let covers = board.unverified.clone();

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

    // A cached red counts only between fix workers: an answer or a resume reruns the gate.
    let gate = docs.gate()?.trim().to_string();
    let mut ran_red = false;
    let green_log: PathBuf;
    let mut green_cached;
    let leaks: String;
    loop {
        if run.state().stop_requested {
            return Ok(Outcome::Stopped);
        }
        // Reset before every run: the cache key counts untracked files, so only a clean lane keys on HEAD's tree.
        let (lane_dir, lane_def) = scheduler::prepare_lane(run, lane, true, out)?;
        let result = gate_once(run, &lane_dir, &lane_def.env(), &gate, n, ran_red, out)?;
        if !result.red {
            green_log = result.log;
            green_cached = result.cached;
            let base = crate::closeout::default_branch(run, docs);
            let scanned = crate::leaks::scan(&lane_dir, crate::leaks::Input::Base(&base));
            if let Err(e) = &scanned {
                say(out, &format!("checkpoint {n}: leaks not run — {e}"))?;
            }
            leaks = leaks_section(&base, scanned);
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
            let title =
                answers::block_title(&format!("gate red three times at checkpoint {n}: {line}"));
            let quoted: String = crate::pack::tail(&tail, 40)
                .lines()
                .map(|l| format!("  {l}\n"))
                .collect();
            let body = format!(
                "**Happened:** the gate (`{gate}`) was red {reds} times in a row at checkpoint {n}; \
                 {fixes} fix worker(s) ran between the runs. The last transcript ({log}) ends:\n{quoted}\
                 **Options:**\n\
                 1. fix the failure by hand, leave the tree as the commit should be, answer this block — the gate runs again\n\
                 2. revert the covered slices' commits and set their slices back to `pending`\n\
                 **Recommended:** 1 — the gate stayed red through {fixes} fix worker(s); the cause is outside what the failing output names.\n",
                log = result.log.display(),
            );
            let id = answers::add_block(&feature_dir, None, Some("checkpoint"), &title, &body)?;
            run.emit(EventKind::CheckpointBlocked {
                n,
                block_id: id.clone(),
                question: title.clone(),
            })?;
            say(
                out,
                &format!(
                    "checkpoint {n}: gate red {reds} times — {id} written to log.jsonl: {line}"
                ),
            )?;
            return Ok(Outcome::Parked {
                what: park_line(&id, &title),
                id,
                fresh: true,
            });
        }
        loop {
            if let Some(o) = await_quota(run, out)? {
                return Ok(o);
            }
            match fix_once(
                run,
                docs,
                cfg,
                board,
                lane,
                n,
                result.attempt,
                &result.log,
                out,
            )? {
                Fixed::Gate => break,
                Fixed::Again => {}
                Fixed::End(o) => return Ok(o),
            }
        }
    }

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
            let body = format!(
                "**Happened:** {deaths} checkpoint workers exited without writing `CP-{n}` or a block: {sessions}. \
                 The gate is green; the driver's once-only respawn is spent. \
                 Streams and stderr are under `{dir}/`.\n\
                 **Options:** 1. read the last worker's `.stderr` and `.jsonl`, fix the environment (login, PATH, quota) and answer this block — a fresh worker runs; \
                 2. drive the Prove rows by hand, add `CP-{n}` (`factory docs log add {feature} --kind checkpoint --phase checkpoint --slice <last> --covers {covers_csv}`) and answer this block — the driver commits the checkpoint.\n\
                 **Recommended:** 1 — a worker that dies before its first tool call is the environment's fault, not the checkpoint's.\n",
                sessions = sessions.join(", "),
                dir = crate::worker::dir(run.dir()).display(),
                covers_csv = covers
                    .iter()
                    .map(u32::to_string)
                    .collect::<Vec<_>>()
                    .join(","),
            );
            let id = answers::add_block(&feature_dir, None, Some("checkpoint"), &title, &body)?;
            run.emit(EventKind::CheckpointBlocked {
                n,
                block_id: id.clone(),
                question: title.clone(),
            })?;
            say(
                out,
                &format!(
                    "checkpoint {n}: second checkpoint worker died — {id} written to log.jsonl"
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
            &leaks,
            &packs,
            spawn,
        )?;
        let open_before: HashSet<String> = open_checkpoint_blocks(&feature_dir)?
            .into_iter()
            .map(|(id, _)| id)
            .collect();
        let (lane_dir, lane_def) = scheduler::prepare_lane(run, lane, true, out)?;
        drop_ref(run, board, n, "checkpoint")?;
        let worker = Worker::spawn(
            run,
            Job::Checkpoint { n, attempt },
            &pack,
            &cfg.workers.checkpoint,
            &lane_def.env(),
            &lane_dir,
            Some(lane),
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
            let sha = match land_here(run, board, n, "checkpoint", attempt, lane, row_id, out)? {
                Ok(sha) => sha,
                Err(o) => {
                    if matches!(o, Outcome::Parked { .. }) {
                        settle(
                            run,
                            row_id,
                            &session_id,
                            &exit.summary,
                            WorkerOutcome::Blocked,
                        )?;
                    }
                    return Ok(o);
                }
            };
            if sha.is_none() {
                say(out, &format!("checkpoint {n} done — nothing to commit"))?;
            }
            settle(run, row_id, &session_id, &exit.summary, WorkerOutcome::Done)?;
            run.emit(EventKind::CheckpointDone { n, sha })?;
            return Ok(Outcome::Done);
        }
        let open_now = open_checkpoint_blocks(&feature_dir)?;
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
                    "checkpoint {n}: worker {session_id} blocked — {id} in log.jsonl: {title}"
                ),
            )?;
            return Ok(Outcome::Parked {
                what: park_line(&id, &title),
                id,
                fresh: true,
            });
        }
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
        settle_dead(run, &feature_dir, n, &session_id, row_id, &exit, out)?;
    }
}

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

struct GateResult {
    red: bool,
    cached: bool,
    attempt: u32,
    log: PathBuf,
}

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
    lane: &Path,
    env: &[(String, String)],
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
    let key = gatecache::tree_key(lane, &index, gate)?;
    let mut cache = GateCache::load(run.dir())?;
    let at = clock::now_rfc3339();
    let fits = |h: &Entry| !h.red || trust_red;
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
        .current_dir(lane)
        .envs(env.iter().map(|(k, v)| (k, v)))
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
    cache.put(
        &key,
        Entry {
            red,
            minutes,
            at: at.clone(),
            log: rel.clone(),
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

enum Fixed {
    Gate,
    /// A usage limit: this attempt gets another fix worker, no gate run between.
    Again,
    End(Outcome),
}

/// The lane lands before the outcome is saved: a driver killed between leaves the landing to the resume.
#[allow(clippy::too_many_arguments)]
fn fix_once(
    run: &mut Run,
    docs: &Docs,
    cfg: &Config,
    board: &Status,
    lane: u32,
    n: u32,
    attempt: u32,
    gate_log: &Path,
    out: &mut dyn Write,
) -> Result<Fixed> {
    let spawn = run.state().workers.len() as u32 + 1;
    let packs = scheduler::packs_dir(run.dir());
    let pack = Pack::for_fix(docs, board, n, attempt, gate_log, &packs, spawn)?;
    let (lane_dir, lane_def) = scheduler::prepare_lane(run, lane, true, out)?;
    drop_ref(run, board, n, "fix")?;
    let worker = Worker::spawn(
        run,
        Job::Fix {
            checkpoint: n,
            attempt,
        },
        &pack,
        &cfg.workers.fix,
        &lane_def.env(),
        &lane_dir,
        Some(lane),
    )?;
    let session_id = worker.session_id.clone();
    let row_id = worker.row_id;
    say(
        out,
        &format!(
            "checkpoint {n}: fix worker {session_id} spawned on gate attempt {attempt} in lane {lane} (pid {}, pack {})",
            worker.pid,
            pack.path.display()
        ),
    )?;
    let exit = worker.wait()?;
    let limit = Limit::from(&exit.summary);
    let dead = limit.is_none() && exit.summary.is_dead();
    match land_here(run, board, n, "fix", attempt, lane, row_id, out)? {
        Ok(Some(_)) => {}
        Ok(None) if !dead && limit.is_none() => say(
            out,
            &format!("checkpoint {n}: fix worker {session_id} changed nothing to commit"),
        )?,
        Ok(None) => {}
        Err(o) => {
            if matches!(o, Outcome::Parked { .. }) {
                settle(
                    run,
                    row_id,
                    &session_id,
                    &exit.summary,
                    WorkerOutcome::Blocked,
                )?;
            }
            return Ok(Fixed::End(o));
        }
    }
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
        return Ok(Fixed::Again);
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
        return Ok(Fixed::Gate);
    }
    settle(run, row_id, &session_id, &exit.summary, WorkerOutcome::Done)?;
    Ok(Fixed::Gate)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Landed {
    Done(Option<String>),
    Waiting(String),
    Conflict { paths: Vec<String>, name: String },
}

pub fn worker_ref(board: &Status, n: u32, role: &str) -> String {
    format!("refs/factory/{:03}/checkpoint-{n}-{role}", board.number)
}

fn worker_subject(board: &Status, n: u32, role: &str) -> String {
    if role == "fix" {
        commit::subject(board, "fix", &format!("gate red at checkpoint {n}"))
    } else {
        commit::subject(board, "docs", &format!("checkpoint {n}"))
    }
}

/// An unlanded ref from an earlier worker would land in place of the next worker's lane.
fn drop_ref(run: &Run, board: &Status, n: u32, role: &str) -> Result<()> {
    let repo = run.state().repo.clone();
    let name = worker_ref(board, n, role);
    if scheduler::resolve(&repo, &name)?.is_some() {
        scheduler::git_ok(&repo, &["update-ref", "-d", &name])?;
    }
    Ok(())
}

/// Idempotent: a resumed driver calls it again for the same worker and goes on from the step it reached.
#[allow(clippy::too_many_arguments)]
pub fn land_worker(
    run: &mut Run,
    board: &Status,
    n: u32,
    role: &str,
    attempt: u32,
    lane: Option<u32>,
    row_id: i64,
    out: &mut dyn Write,
) -> Result<Landed> {
    let landed = run
        .db()
        .workers(run.id().as_str())?
        .into_iter()
        .find(|w| w.id == row_id)
        .and_then(|w| w.commit_sha);
    if landed.is_some() {
        return Ok(Landed::Done(landed));
    }
    let repo = run.state().repo.clone();
    let board_files = commit::tool_files(board).to_vec();
    let name = worker_ref(board, n, role);
    let subject = worker_subject(board, n, role);
    let base = lane.and_then(|l| run.state().lane_bases.get(&l).cloned());
    // A driver killed after the landing commit and before its record.
    if let Some(base) = &base {
        let tip = scheduler::git_ok(&repo, &["log", "--format=%H %s", &format!("{base}..HEAD")])?;
        if let Some(sha) = tip
            .lines()
            .find_map(|l| l.split_once(' ').filter(|(_, s)| *s == subject))
            .map(|(sha, _)| sha.to_string())
        {
            if scheduler::resolve(&repo, &name)?.is_some() {
                scheduler::git_ok(&repo, &["update-ref", "-d", &name])?;
            }
            record(run, n, role, attempt, row_id, &sha, out)?;
            return Ok(Landed::Done(Some(sha)));
        }
    }
    let sha = match scheduler::resolve(&repo, &name)? {
        Some(sha) => sha,
        None => {
            let (Some(lane), Some(base)) = (lane, base) else {
                return Ok(Landed::Done(commit_board(
                    run, board, n, role, attempt, row_id, out,
                )?));
            };
            let main = lanes::main_tree(&repo)?;
            let Some(c) = land::commit_lane(
                &lanes::dir(&paths::data_dir(), &main, lane),
                &base,
                &board_files,
                &commit::protected_files(board),
                &subject,
                &name,
            )?
            else {
                return Ok(Landed::Done(commit_board(
                    run, board, n, role, attempt, row_id, out,
                )?));
            };
            let who = if role == "fix" {
                "fix"
            } else {
                "checkpoint worker"
            };
            for path in &c.dropped {
                say(
                    out,
                    &format!(
                        "checkpoint {n}: {role} worker wrote {path}, which no {who} may — left out of the commit"
                    ),
                )?;
            }
            c.sha
        }
    };
    scheduler::undo_half_pick(&repo, &sha)?;
    match land::land(&repo, &sha, &board_files)? {
        Landing::Landed { sha } => {
            record(run, n, role, attempt, row_id, &sha, out)?;
            Ok(Landed::Done(Some(sha)))
        }
        Landing::Dirty { paths } => Ok(Landed::Waiting(format!(
            "checkpoint {n} cannot land: {} changed in the main tree — commit or discard them",
            paths.join(", ")
        ))),
        Landing::Conflict { paths } => Ok(Landed::Conflict { paths, name }),
    }
}

#[allow(clippy::too_many_arguments)]
fn land_here(
    run: &mut Run,
    board: &Status,
    n: u32,
    role: &str,
    attempt: u32,
    lane: u32,
    row_id: i64,
    out: &mut dyn Write,
) -> Result<std::result::Result<Option<String>, Outcome>> {
    let (mut parked, mut announced) = (false, false);
    loop {
        match land_worker(run, board, n, role, attempt, Some(lane), row_id, out)? {
            Landed::Done(sha) => {
                if parked {
                    say(
                        out,
                        &format!(
                            "checkpoint {n}: the landing went through — the checkpoint goes on"
                        ),
                    )?;
                    run.set_phase(Phase::Checkpoint)?;
                }
                return Ok(Ok(sha));
            }
            Landed::Waiting(what) => {
                scheduler::park_with(run, &mut parked, &mut announced, &[], &[], &what, out)?;
                scheduler::slow_tick(run)?;
                if run.state().stop_requested {
                    return Ok(Err(Outcome::Stopped));
                }
            }
            Landed::Conflict { paths, name } => {
                return Ok(Err(block_conflict(
                    run, board, n, role, attempt, row_id, &paths, &name, out,
                )?));
            }
        }
    }
}

/// A fix worker's board writes wait for the next commit; a checkpoint worker's `CP-n` lives in the main tree.
fn commit_board(
    run: &mut Run,
    board: &Status,
    n: u32,
    role: &str,
    attempt: u32,
    row_id: i64,
    out: &mut dyn Write,
) -> Result<Option<String>> {
    if role == "fix" {
        return Ok(None);
    }
    let repo = run.state().repo.clone();
    let files = commit::tool_files(board).to_vec();
    match commit::commit(&repo, &files, &worker_subject(board, n, role))? {
        Some(c) => {
            record(run, n, role, attempt, row_id, &c.sha, out)?;
            Ok(Some(c.sha))
        }
        None => Ok(None),
    }
}

fn record(
    run: &mut Run,
    n: u32,
    role: &str,
    attempt: u32,
    row_id: i64,
    sha: &str,
    out: &mut dyn Write,
) -> Result<()> {
    let repo = run.state().repo.clone();
    let files = scheduler::nul_split(&scheduler::git_ok(
        &repo,
        &[
            "diff-tree",
            "--no-commit-id",
            "--name-only",
            "-r",
            "-z",
            sha,
        ],
    )?);
    run.db().set_worker_commit(row_id, sha)?;
    if role != "fix" {
        return say(
            out,
            &format!(
                "checkpoint {n} done — committed {} ({} file(s))",
                short(sha),
                files.len()
            ),
        );
    }
    say(
        out,
        &format!(
            "checkpoint {n}: fix committed {} — {} file(s): {}",
            short(sha),
            files.len(),
            files.join(", ")
        ),
    )?;
    run.emit(EventKind::FixCommitted {
        n,
        attempt,
        sha: sha.to_string(),
        files,
    })?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn block_conflict(
    run: &mut Run,
    board: &Status,
    n: u32,
    role: &str,
    attempt: u32,
    row_id: i64,
    paths: &[String],
    name: &str,
    out: &mut dyn Write,
) -> Result<Outcome> {
    commit_board(run, board, n, role, attempt, row_id, out)?;
    let feature_dir = run.state().repo.join(&board.dir);
    let title = answers::block_title(&format!(
        "checkpoint {n}: the {role} worker's commit conflicts with the feature tip in {}",
        paths.join(", ")
    ));
    // A checkpoint worker's `CP-n` is on the board by now: the answer goes past the checkpoint, not back into it.
    let next = if role == "checkpoint" {
        "the run goes on past the recorded checkpoint"
    } else {
        "the checkpoint goes on from the gate"
    };
    let body = format!(
        "**Happened:** the {role} worker's lane, committed as `{name}`, does not apply on the feature tip, \
         which moved during checkpoint {n}; it conflicts in {paths}. The main tree is untouched, and the run holds until this is answered.\n\
         **Options:**\n\
         1. `git cherry-pick {name}`, resolve, commit, answer this block — {next}\n\
         2. answer this block without it — {next} on the tip as it is, and the next {role} worker's spawn deletes the ref\n\
         **Recommended:** 1 — the commit holds everything the worker left in its lane.\n",
        paths = paths.join(", "),
    );
    let id = answers::add_block(&feature_dir, None, Some("checkpoint"), &title, &body)?;
    run.emit(EventKind::CheckpointBlocked {
        n,
        block_id: id.clone(),
        question: title.clone(),
    })?;
    say(
        out,
        &format!("checkpoint {n}: {role} worker's commit conflicts — {id} written to log.jsonl"),
    )?;
    Ok(Outcome::Parked {
        what: park_line(&id, &title),
        id,
        fresh: true,
    })
}

/// `MergeRefused` events of old runs carry v2 `docs log merge` output; only the cockpit shows it.
pub fn refusal_summary(detail: &str) -> String {
    let refusals: Vec<&str> = detail
        .lines()
        .map(str::trim)
        .filter_map(|l| l.strip_prefix("✗ "))
        .collect();
    if refusals.is_empty() {
        detail
            .lines()
            .map(str::trim)
            .find(|l| !l.is_empty())
            .unwrap_or("docs log merge failed")
            .to_string()
    } else {
        refusals.join("; ")
    }
}

fn settle_dead(
    run: &mut Run,
    feature_dir: &Path,
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
    answers::add_note(
        feature_dir,
        None,
        "Dead checkpoint worker",
        &format!("checkpoint {n}, session {session_id} {code} {what}."),
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
    // Never fails the exit: an unreadable dir is an event and null columns.
    let _ = metrics::record_subagents(run, row_id, session_id, summary.cwd.as_deref());
    metrics::record_exit(run, row_id, summary)?;
    run.set_worker_outcome(session_id, outcome.as_str())
}

pub fn open_checkpoint_blocks(feature_dir: &Path) -> Result<Vec<(String, String)>> {
    Ok(answers::entries(feature_dir)?
        .into_iter()
        .filter(|e| !e.resolved && e.phase.as_deref() == Some("checkpoint"))
        .map(|e| (e.id, e.title))
        .collect())
}

/// The death cap and the red count restart at the owner's latest answer to one of this checkpoint's blocks.
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

/// A worker with no spawn event (killed inside the spawn) is not among them, so it counts.
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

/// The body of the pack's `# Leaks`; a scan that fails reads `not run`, never a red.
fn leaks_section(base: &str, scanned: Result<crate::leaks::Report>) -> String {
    match scanned {
        Ok(report) => {
            let body = report.render();
            // Longer than any backtick run in the hits, so a hit's own fence cannot close it.
            let run = body.split(|c| c != '`').map(str::len).max().unwrap_or(0);
            let fence = "`".repeat(run.max(2) + 1);
            format!("<!-- factory leaks --base {base} -->\n\n{fence}\n{body}{fence}")
        }
        Err(e) => format!("not run: {e}"),
    }
}

pub(crate) fn park_line(id: &str, title: &str) -> String {
    format!(
        "checkpoint blocked — {title}: {id} in log.jsonl waits for the owner: `factory answer {id} \"<text>\"`"
    )
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
    fn a_scan_that_fails_reads_not_run() {
        let tmp = tempfile::tempdir().unwrap();
        let scanned = crate::leaks::scan(tmp.path(), crate::leaks::Input::Base("main"));
        let body = leaks_section("main", scanned);
        assert!(body.starts_with("not run: git "), "{body}");
        assert_eq!(body.lines().count(), 1, "{body}");
    }

    #[test]
    fn a_hit_holding_a_fence_cannot_close_the_leaks_block() {
        let line = crate::leaks::Added {
            path: "a.rs".into(),
            line: 1,
            text: "/// ```FR-3".into(),
        };
        let report = crate::leaks::check(&[], &[line], &[]);
        let body = leaks_section("main", Ok(report));
        assert!(
            body.contains("\n````\n") && body.ends_with("````"),
            "{body}"
        );
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
        assert_eq!(since_answer(&events, 1), 0);
        assert!(spawned_before_answer(&events, 1).is_empty());

        events.extend([blocked(1, "BLOCK-2"), answered("BLOCK-2")]);
        assert_eq!(since_answer(&events, 1), 5);
        assert_eq!(
            spawned_before_answer(&events, 1),
            HashSet::from(["a".to_string()])
        );

        events.extend([spawned(1, "b"), blocked(2, "BLOCK-3"), answered("BLOCK-3")]);
        assert_eq!(since_answer(&events, 1), 5);
        assert_eq!(
            spawned_before_answer(&events, 1),
            HashSet::from(["a".to_string()])
        );
        assert_eq!(since_answer(&events, 2), 8);
    }
}
