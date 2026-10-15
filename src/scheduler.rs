//! The board is the truth: every tick routes on it, and a worker's `STATUS:` line is a hint the row keeps.
//! One hint writes the board: a build worker's `done` on a `building` slice, once its lane passes `docs check`.
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use crate::answers::{self, Answer};
use crate::checkpoint::{self, Outcome};
use crate::cli::report;
use crate::closeout;
use crate::commit;
use crate::config::Config;
use crate::docs::{Docs, Status};
use crate::error::io;
use crate::events::{self, EventKind};
use crate::land::{self, Landing};
use crate::pack::{BlockSection, Pack};
use crate::quota::{self, Hold, Limit};
use crate::run::{GateCmds, Phase, Run, WorkerOutcome, WorkerRef};
use crate::stream::{StatusHint, Summary};
use crate::worker::{self, Exit, Job, Worker};
use crate::{Error, Result, clock, lanes, metrics, paths};

pub const DEATH_CAP: usize = 2;

pub const CONFLICT_CAP: usize = 3;

pub const TICK: Duration = Duration::from_secs(1);
pub const SLOW_TICK: Duration = Duration::from_secs(5);

struct Running {
    worker: Worker,
    lane: u32,
    exit: Option<Exit>,
}

type Table = HashMap<u32, Running>;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Orphans {
    None,
    Live,
    Settled,
    Landing(String),
}

pub fn drive(run: &mut Run, docs: &Docs, cfg: &Config, out: &mut dyn Write) -> Result<Phase> {
    let mut table: Table = HashMap::new();
    match drive_inner(run, docs, cfg, &mut table, out) {
        Ok(phase) => Ok(phase),
        Err(e) => {
            // Another process adopted the run: it is theirs, failed or not.
            if let Some(pid) = run.lost_to() {
                let _ = writeln!(
                    out,
                    "run {} was taken over by pid {pid} — this driver stops and leaves it to them",
                    run.id()
                );
                return Err(e);
            }
            abandon(run, docs, &mut table, out);
            let _ = run.set_phase(Phase::Failed);
            let _ = run.emit(EventKind::RunFailed {
                error: e.to_string(),
            });
            let _ = writeln!(out, "run {} failed: {e}", run.id());
            Err(e)
        }
    }
}

fn abandon(run: &mut Run, docs: &Docs, table: &mut Table, out: &mut dyn Write) {
    let board = docs.status(&run.state().feature.clone()).ok();
    for (slice, mut r) in table.drain() {
        let (session_id, row_id, pid) =
            (r.worker.session_id.clone(), r.worker.row_id, r.worker.pid);
        let exit = match r.exit {
            Some(_) if board.as_ref().is_some_and(|b| landing_owed(run, b, slice)) => {
                let _ = say(
                    out,
                    &format!(
                        "slice {slice:02}: worker {session_id} is done and its landing waits for the next driver"
                    ),
                );
                continue;
            }
            Some(exit) => exit,
            None => {
                r.worker.kill();
                match r.worker.wait() {
                    Ok(exit) => exit,
                    Err(_) => continue,
                }
            }
        };
        settle_failed(run, row_id, &session_id, &exit.summary);
        let _ = say(
            out,
            &format!("slice {slice:02}: worker {session_id} (pid {pid}) stopped — the run failed"),
        );
    }
}

fn settle_failed(run: &mut Run, row_id: i64, session_id: &str, summary: &Summary) {
    let open = run
        .state()
        .workers
        .iter()
        .any(|w| w.session_id == session_id && w.outcome.is_none());
    if open {
        let _ = settle(run, row_id, session_id, summary, WorkerOutcome::Failed);
    }
}

fn drive_inner(
    run: &mut Run,
    docs: &Docs,
    cfg: &Config,
    table: &mut Table,
    out: &mut dyn Write,
) -> Result<Phase> {
    let feature = run.state().feature.clone();
    // Read once: `metrics` settles a worker's `Bash` call that runs these as a `gates` row.
    if run.gate_cmds().is_none()
        && let Ok(gate) = docs.gate()
    {
        let inner = docs.gate_inner().ok().filter(|s| !s.is_empty());
        run.set_gate_cmds(GateCmds { gate, inner });
    }
    // A resumed parked run stays parked without a second `run_waiting`.
    let mut parked = run.state().phase == Phase::WaitingOwner;
    let mut announced = false;
    let repo = run.state().repo.clone();
    let default = crate::cli::run::load_preflight(run.dir())?.and_then(|pf| pf.branch.default);
    if let Some(b) = run.state().branch.clone() {
        commit::hold_branch(&repo, &b, default.as_deref());
    }
    let mut off_branch = false;
    let mut landing_wait = false;
    let mut waiting: HashSet<String> = HashSet::new();
    let mut unreadable: HashSet<String> = HashSet::new();
    let mut torn: HashSet<usize> = HashSet::new();
    if !parked && run.state().branch.is_some() && run.state().paused_until.is_none() {
        run.set_phase(Phase::Building)?;
    }
    loop {
        run.reload()?;
        if let Some(pid) = run.taken_over() {
            return Err(crate::Error::Msg(format!(
                "run {} was taken over by pid {pid}",
                run.id()
            )));
        }
        for (n, err) in events::scan(run.dir())?.1 {
            if torn.insert(n) {
                say(
                    out,
                    &format!(
                        "{} line {n} is not an event ({err}) — skipped",
                        events::FILE
                    ),
                )?;
            }
        }
        // Off the branch nothing is reaped (a reap commits); once the closeout owns the run, its guard blocks.
        if let Some(b) = run.state().branch.clone() {
            let refusal = commit::branch_refusal(&repo, &b, default.as_deref())?;
            if let Some(why) = &refusal
                && !closeout_owns(run, docs, &feature, table)?
            {
                if run.state().stop_requested && table.values().all(|r| r.exit.is_some()) {
                    return stopped(run, out);
                }
                if !off_branch {
                    parked = false;
                    announced = false;
                    off_branch = true;
                }
                park_with(run, &mut parked, &mut announced, &[], &[], why, out)?;
                if table.is_empty() {
                    slow_tick(run)?;
                } else {
                    poll(table, TICK)?;
                }
                continue;
            }
            if off_branch && refusal.is_none() {
                say(out, &format!("HEAD is back on {b} — the run goes on"))?;
                off_branch = false;
                announced = false;
            }
        }
        reap(run, docs, &feature, table, out)?;

        // A run has no branch until its last preflight ask is answered.
        if run.state().branch.is_none() {
            let pending = answers::asks_pending(run)?;
            if !pending.is_empty() {
                if run.state().stop_requested {
                    return stopped(run, out);
                }
                let ids: Vec<String> = pending.iter().map(|(id, _)| id.clone()).collect();
                enter_park(run, &mut parked, &mut announced, &[], &ids, out)?;
                slow_tick(run)?;
                continue;
            }
            start_after_asks(run, docs, out)?;
            parked = false;
            announced = false;
            run.set_phase(Phase::Building)?;
        }

        if run.state().stop_requested {
            if table.is_empty() {
                let board = docs.status(&feature)?;
                let none = HashSet::new();
                if settle_orphans(run, &board, &none, &mut waiting, out)? == Orphans::Live {
                    std::thread::sleep(TICK);
                    continue;
                }
                return stopped(run, out);
            }
            poll(table, TICK)?;
            continue;
        }

        match quota::hold(run)? {
            Some(Hold::Paused { until }) => {
                if run.state().phase != Phase::Paused {
                    run.set_phase(Phase::Paused)?;
                }
                if table.is_empty() {
                    quota::pause_tick(run, until)?;
                } else {
                    poll(table, TICK)?;
                }
                continue;
            }
            Some(Hold::Budget { weighted, budget }) => {
                if !table.is_empty() {
                    poll(table, TICK)?;
                    continue;
                }
                return quota::park_budget(run, weighted, budget, out);
            }
            None => {
                if run.state().paused_until.is_some() {
                    quota::resume(run, Phase::Building, out)?;
                }
            }
        }

        let mut board = docs.status(&feature)?;
        let live: HashSet<String> = table
            .values()
            .map(|r| r.worker.session_id.clone())
            .collect();
        let orphans = settle_orphans(run, &board, &live, &mut waiting, out)?;
        if landing_wait && !matches!(orphans, Orphans::Landing(_)) {
            say(out, "the landing went through — the run goes on")?;
            landing_wait = false;
            parked = false;
            announced = false;
            run.set_phase(Phase::Building)?;
        }
        match orphans {
            Orphans::Landing(what) => {
                if !landing_wait {
                    parked = false;
                    announced = false;
                    landing_wait = true;
                }
                park_with(run, &mut parked, &mut announced, &[], &[], &what, out)?;
                if table.is_empty() {
                    slow_tick(run)?;
                } else {
                    poll(table, TICK)?;
                }
                continue;
            }
            Orphans::None => {}
            Orphans::Live => {
                if table.is_empty() {
                    std::thread::sleep(TICK);
                } else {
                    poll(table, TICK)?;
                }
                continue;
            }
            Orphans::Settled => {
                // A settled orphan may have paused the run or spent the budget.
                if quota::hold(run)?.is_some() {
                    continue;
                }
                board = docs.status(&feature)?;
            }
        }

        for (name, err) in answers::scan(run.dir())?.1 {
            if unreadable.insert(name.clone()) {
                say(
                    out,
                    &format!("{}/{name} is not an answer ({err}) — skipped", answers::DIR),
                )?;
            }
        }
        let mut free = free_lanes(&board, table);
        // A capped slice here is a kill between its last death or conflict and the flip to `blocked`.
        let mut capped = false;
        let checkpoint_waits = board.checkpoint_due && !free.is_empty();
        for b in board.building.clone() {
            if table.contains_key(&b.number) || has_unsettled(run, b.number) {
                continue;
            }
            // A held ref with a lane is a conflicted pick: the next worker merges.
            let merging = b.lane.is_some()
                && resolve(&run.state().repo, &slice_ref(&board, b.number))?.is_some();
            let events = run.events()?;
            if merging && conflicts_since_answer(&events, b.number) >= CONFLICT_CAP {
                let paths = last_conflict(&events, b.number).map_or_else(Vec::new, |(_, p)| p);
                block_conflicted(run, &board, b.number, &paths, out)?;
                capped = true;
                continue;
            }
            let dead = deaths_since_answer(run, b.number, if merging { "merge" } else { "build" })?;
            if dead.len() >= DEATH_CAP {
                let block = block_dead_slice(run, &board, b.number, &dead, "")?;
                say(
                    out,
                    &format!(
                        "slice {:02}: {} deaths since the last answer — slice blocked ({block})",
                        b.number,
                        dead.len()
                    ),
                )?;
                capped = true;
                continue;
            }
            // A respawn is a spawn: it yields to a due checkpoint that has a lane to run in.
            if checkpoint_waits {
                continue;
            }
            let (lane, fresh) = match b.lane {
                Some(l) => (l, false),
                None => {
                    if free.is_empty() {
                        continue;
                    }
                    (free.remove(0), true)
                }
            };
            let mut answer = answers::block_answer_for_slice(run.dir(), b.number)?;
            // An answer older than the conflict is about the build, already in the picked commit.
            if merging
                && last_conflict(&events, b.number)
                    .is_some_and(|(at, _)| answer_from(&events, b.number) <= at)
            {
                answer = None;
            }
            spawn_on(
                run,
                docs,
                cfg,
                &board,
                b.number,
                lane,
                fresh,
                merging,
                answer.as_ref(),
                table,
                out,
            )?;
        }
        if capped {
            continue;
        }
        // A checkpoint block open past its checkpoint holds the run as the checkpoint would.
        let checkpoint_block = if board.checkpoint_due {
            None
        } else {
            checkpoint::open_checkpoint_blocks(&run.state().repo.join(&board.dir))?
                .into_iter()
                .next()
        };
        if !board.checkpoint_due && checkpoint_block.is_none() {
            for r in board.ready.clone() {
                if free.is_empty() {
                    break;
                }
                if table.contains_key(&r.number) {
                    continue;
                }
                // A dependency counts as done once landed: a lane reset earlier would miss its code.
                let unlanded = board
                    .slices
                    .iter()
                    .find(|s| s.number == r.number)
                    .is_some_and(|s| {
                        s.depends_on
                            .iter()
                            .any(|d| table.contains_key(d) || has_unsettled(run, *d))
                    });
                if unlanded {
                    continue;
                }
                let lane = free.remove(0);
                spawn_on(
                    run, docs, cfg, &board, r.number, lane, true, false, None, table, out,
                )?;
            }
        }

        if !table.is_empty() {
            if parked || run.state().phase != Phase::Building {
                run.set_phase(Phase::Building)?;
            }
            parked = false;
            announced = false;
            poll(table, TICK)?;
            continue;
        }
        if board.checkpoint_due
            && let Some(&lane) = free_lanes(&board, table).first()
        {
            match checkpoint::run(run, docs, cfg, &board, lane, out)? {
                Outcome::Done => {
                    parked = false;
                    announced = false;
                    continue;
                }
                Outcome::Parked { what, fresh, .. } => {
                    if fresh {
                        parked = false;
                        announced = false;
                    }
                    park_with(run, &mut parked, &mut announced, &[], &[], &what, out)?;
                    slow_tick(run)?;
                    continue;
                }
                Outcome::Stopped => return stopped(run, out),
                Outcome::Budget { weighted, budget } => {
                    return quota::park_budget(run, weighted, budget, out);
                }
            }
        }
        if let Some((id, title)) = checkpoint_block {
            let what = checkpoint::park_line(&id, &title);
            park_with(run, &mut parked, &mut announced, &[], &[], &what, out)?;
            slow_tick(run)?;
            continue;
        }
        if !board.blocked.is_empty() {
            enter_park(run, &mut parked, &mut announced, &board.blocked, &[], out)?;
            slow_tick(run)?;
            continue;
        }
        if board.all_done && board.unverified.is_empty() {
            match closeout::run(run, docs, cfg, &board, out)? {
                Outcome::Done => return finish(run, out, "every slice done"),
                Outcome::Parked { what, fresh, .. } => {
                    if fresh {
                        parked = false;
                        announced = false;
                    }
                    park_with(run, &mut parked, &mut announced, &[], &[], &what, out)?;
                    slow_tick(run)?;
                    continue;
                }
                Outcome::Stopped => return stopped(run, out),
                Outcome::Budget { weighted, budget } => {
                    return quota::park_budget(run, weighted, budget, out);
                }
            }
        }
        return finish(run, out, "nothing ready");
    }
}

/// Nothing between here and `closeout::run` commits; the closeout checks the branch itself.
fn closeout_owns(run: &Run, docs: &Docs, feature: &str, table: &Table) -> Result<bool> {
    if !table.is_empty() || run.state().workers.iter().any(|w| w.outcome.is_none()) {
        return Ok(false);
    }
    let board = docs.status(feature)?;
    Ok(board.all_done && board.unverified.is_empty())
}

fn reap(
    run: &mut Run,
    docs: &Docs,
    feature: &str,
    table: &mut Table,
    out: &mut dyn Write,
) -> Result<()> {
    let mut slices: Vec<u32> = table.keys().copied().collect();
    slices.sort_unstable();
    for slice in slices {
        // Stays in the table until routed, so a failing driver's `abandon` finds it.
        let r = table.get_mut(&slice).expect("key from the table");
        if r.exit.is_none() {
            r.exit = r.worker.try_wait()?;
        }
        if r.exit.is_none() {
            continue;
        }
        let mut board = docs.status(feature)?;
        let mut r = table.remove(&slice).expect("key from the table");
        let exit = r.exit.take().expect("checked above");
        let routed = done_by_driver(
            run,
            &board,
            slice,
            &r.worker.role,
            Some(r.lane),
            &exit.summary,
            out,
        )
        .and_then(|status| {
            if status != slice_status(&board, slice) {
                board = docs.status(feature)?;
            }
            route_exit(run, &board, &r, exit, out)
        });
        if let Err(e) = routed {
            if !landing_owed(run, &board, slice) {
                let summary = worker::summary_from_disk(run.dir(), &r.worker.session_id);
                settle_failed(run, r.worker.row_id, &r.worker.session_id, &summary);
            }
            return Err(e);
        }
    }
    Ok(())
}

/// Settled `failed`, such a worker's slice would never land; left unsettled, the resume lands it.
fn landing_owed(run: &Run, board: &Status, slice: u32) -> bool {
    slice_status(board, slice) == "done"
        || resolve(&run.state().repo, &slice_ref(board, slice)).is_ok_and(|r| r.is_some())
}

fn poll(table: &mut Table, cap: Duration) -> Result<()> {
    let deadline = Instant::now() + cap;
    loop {
        for r in table.values_mut() {
            if r.exit.is_none()
                && let Some(exit) = r.worker.try_wait()?
            {
                r.exit = Some(exit);
                return Ok(());
            }
        }
        if Instant::now() >= deadline {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

pub(crate) fn slow_tick(run: &mut Run) -> Result<()> {
    let dir = answers::dir(run.dir());
    let before = dir_stamp(&dir);
    let deadline = Instant::now() + SLOW_TICK;
    while Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(250));
        run.reload()?;
        if run.state().stop_requested || dir_stamp(&dir) != before {
            return Ok(());
        }
    }
    Ok(())
}

fn dir_stamp(dir: &Path) -> Option<(SystemTime, usize)> {
    let meta = fs::metadata(dir).ok()?;
    let count = fs::read_dir(dir).map(|rd| rd.count()).unwrap_or(0);
    Some((meta.modified().ok()?, count))
}

fn enter_park(
    run: &mut Run,
    parked: &mut bool,
    announced: &mut bool,
    blocked: &[u32],
    asks: &[String],
    out: &mut dyn Write,
) -> Result<()> {
    let what = if asks.is_empty() {
        format!(
            "slice {} blocked — open blocks wait for the owner: `factory answer BLOCK-n \"<text>\"`",
            pad_list(blocked)
        )
    } else {
        format!(
            "{} waiting — the run starts when every ask is answered: `factory answer ASK-n \"<text>\"`",
            asks.join(", ")
        )
    };
    park_with(run, parked, announced, blocked, asks, &what, out)
}

pub(crate) fn park_with(
    run: &mut Run,
    parked: &mut bool,
    announced: &mut bool,
    blocked: &[u32],
    asks: &[String],
    what: &str,
    out: &mut dyn Write,
) -> Result<()> {
    // Line before phase: a reader of `waiting-owner` in state.json must find the park in driver.log.
    if !*announced {
        say(
            out,
            &format!("run {} parked (waiting-owner): {what}", run.id()),
        )?;
        *announced = true;
    }
    if !*parked {
        run.emit(EventKind::RunWaiting {
            blocked: blocked.to_vec(),
            asks: asks.to_vec(),
        })?;
        run.set_phase(Phase::WaitingOwner)?;
        *parked = true;
    }
    Ok(())
}

fn start_after_asks(run: &mut Run, docs: &Docs, out: &mut dyn Write) -> Result<()> {
    let feature = run.state().feature.clone();
    let repo = run.state().repo.clone();
    let pf = match crate::cli::run::load_preflight(run.dir())? {
        Some(pf) => pf,
        None => docs.preflight(&feature)?.0,
    };
    say(out, "every ask answered — the run proceeds")?;
    let branch = crate::cli::run::ensure_branch(&repo, &pf.branch)?;
    commit::hold_branch(&repo, &branch, pf.branch.default.as_deref());
    run.set_branch(branch)?;
    for text in &pf.judge {
        run.emit(EventKind::JudgeNoted { text: text.clone() })?;
    }
    crate::cli::run::begin(run, docs, out)?;
    Ok(())
}

/// An older vendored docs.mjs lists a blocked slice's lane in `free_lanes`.
fn free_lanes(board: &Status, table: &Table) -> Vec<u32> {
    let parked: HashSet<u32> = board
        .slices
        .iter()
        .filter(|s| s.status.as_deref() == Some("blocked"))
        .filter_map(|s| s.lane)
        .collect();
    let held: HashSet<u32> = table.values().map(|r| r.lane).collect();
    board
        .free_lanes
        .iter()
        .copied()
        .filter(|l| !parked.contains(l) && !held.contains(l))
        .collect()
}

fn has_unsettled(run: &Run, slice: u32) -> bool {
    run.state()
        .workers
        .iter()
        .any(|w| w.slice == Some(slice) && w.outcome.is_none())
}

#[allow(clippy::too_many_arguments)]
fn spawn_on(
    run: &mut Run,
    docs: &Docs,
    cfg: &Config,
    board: &Status,
    slice: u32,
    lane: u32,
    fresh: bool,
    merge: bool,
    answer: Option<&Answer>,
    table: &mut Table,
    out: &mut dyn Write,
) -> Result<()> {
    let repo = run.state().repo.clone();
    let (lane_dir, lane_def) = prepare_lane(run, lane, fresh, out)?;
    // The lane is the slice's only once reset: a kill before leaves it free, not held stale.
    if fresh {
        crate::docs::state::set(
            &repo.join(&board.dir),
            &[
                (&format!("slices.{slice}.status"), "building"),
                (&format!("slices.{slice}.lane"), &lane.to_string()),
            ],
        )?;
    }
    // A build landing commits the lane afresh: a ref left by an earlier landing would hide its work.
    let stale = slice_ref(board, slice);
    if !merge && resolve(&repo, &stale)?.is_some() {
        git_ok(&repo, &["update-ref", "-d", &stale])?;
    }
    let block = match answer {
        Some(a) => Some(block_section(&repo, board, a)?),
        None => None,
    };
    let spawn_n = run.state().workers.len() as u32 + 1;
    let (pack, job) = if merge {
        let events = run.events()?;
        let paths = last_conflict(&events, slice).map_or_else(Vec::new, |(_, p)| p);
        let sha = resolve(&repo, &stale)?
            .ok_or_else(|| Error::Msg(format!("slice {slice:02} has no {stale} to merge")))?;
        let attempt = events
            .iter()
            .filter(|e| matches!(e.kind, EventKind::MergeSpawned { slice: s, .. } if s == slice))
            .count() as u32
            + 1;
        let pack = Pack::for_merge(
            docs,
            board,
            slice,
            &paths,
            &sha,
            &packs_dir(run.dir()),
            spawn_n,
            block.as_ref(),
        )?;
        (pack, Job::Merge { slice, attempt })
    } else {
        let pack = Pack::for_slice_with(
            docs,
            board,
            slice,
            &packs_dir(run.dir()),
            spawn_n,
            block.as_ref(),
        )?;
        (pack, Job::Build { slice })
    };
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
    let w = Worker::spawn(
        run,
        job,
        &pack,
        &cfg.workers.build,
        &lane_def.env(),
        &lane_dir,
        Some(lane),
    )?;
    let line = format!(
        "slice {slice:02} → {}worker {} (pid {}), lane {lane}{}, pack {}",
        if merge { "merge " } else { "" },
        w.session_id,
        w.pid,
        if block.is_some() { ", answered" } else { "" },
        pack.path.display()
    );
    table.insert(
        slice,
        Running {
            worker: w,
            lane,
            exit: None,
        },
    );
    say(out, &line)
}

pub(crate) fn prepare_lane(
    run: &mut Run,
    lane: u32,
    fresh: bool,
    out: &mut dyn Write,
) -> Result<(PathBuf, lanes::Lane)> {
    let lane_def = lanes::load(&run.state().repo.clone(), lane)?;
    let lane_dir = open_lane(run, lane, fresh)?;
    if !run.state().lanes_set_up.contains(&lane) {
        if let Some(cmd) = &lane_def.setup {
            lane_def.setup(&lane_dir)?;
            say(out, &format!("lane {lane} setup ran: {cmd}"))?;
        }
        run.mark_lane_set_up(lane)?;
    }
    Ok((lane_dir, lane_def))
}

/// A resumed run skipped the preflight that makes the worktrees; a landing needs the base saved.
fn open_lane(run: &mut Run, lane: u32, fresh: bool) -> Result<PathBuf> {
    let repo = run.state().repo.clone();
    let main = lanes::main_tree(&repo)?;
    let seed = crate::docs::project::seed(&main)?.unwrap_or_default();
    let made = lanes::ensure(&paths::data_dir(), &main, lane, &seed)?;
    if fresh || made.created || !run.state().lane_bases.contains_key(&lane) {
        let head = git_ok(&repo, &["rev-parse", "HEAD"])?.trim().to_string();
        lanes::reset(&made.dir, &head)?;
        run.set_lane_base(lane, &head)?;
    }
    Ok(made.dir)
}

fn block_section(repo: &Path, board: &Status, answer: &Answer) -> Result<BlockSection> {
    let feature_dir = repo.join(&board.dir);
    if let Some(entry) = answers::find(&feature_dir, &answer.id)? {
        let source = entry
            .path
            .strip_prefix(repo)
            .unwrap_or(&entry.path)
            .to_string_lossy()
            .into_owned();
        return Ok(BlockSection {
            id: answer.id.clone(),
            source,
            text: answers::entry_text(&entry),
        });
    }
    Ok(BlockSection {
        id: answer.id.clone(),
        source: format!("{}/{}.json", answers::DIR, answer.id),
        text: format!(
            "### {} · resolved\n\n**Resolution ({}):** {}",
            answer.id,
            &answer.at[..10.min(answer.at.len())],
            answer.text
        ),
    })
}

fn route_exit(
    run: &mut Run,
    board: &Status,
    r: &Running,
    exit: Exit,
    out: &mut dyn Write,
) -> Result<()> {
    let slice = r.worker.slice.expect("the table holds slice workers only");
    let session_id = r.worker.session_id.clone();
    let row_id = r.worker.row_id;
    // A merge worker never sets the slice's status: its role routes it.
    if r.worker.role == "merge" {
        let w = find_ref(run, &session_id);
        route_merge(run, board, &w, slice, exit.code, exit.summary, out)?;
        return Ok(());
    }
    match slice_status(board, slice) {
        "done" => {
            if !done_logged(run, slice, &session_id)? {
                run.emit(EventKind::SliceDone { slice })?;
            }
            say(
                out,
                &format!(
                    "slice {slice:02} done (worker {session_id}, {} turns, hint {}, lane {} free)",
                    exit.summary.num_turns.unwrap_or(0),
                    exit.summary
                        .status_hint
                        .map(|h| h.as_str())
                        .unwrap_or("none"),
                    r.lane
                ),
            )?;
            // The landing before the settle: a driver killed between leaves the resume to land it.
            if let Some(outcome) = land_slice(run, board, slice, row_id, false, out)?.outcome() {
                settle(run, row_id, &session_id, &exit.summary, outcome)?;
            }
        }
        "blocked" => {
            settle(
                run,
                row_id,
                &session_id,
                &exit.summary,
                WorkerOutcome::Blocked,
            )?;
            let block = open_block(run, board, slice)?;
            emit_blocked(run, slice, block.as_ref())?;
            say(
                out,
                &format!(
                    "slice {slice:02} blocked by its worker ({}) — lane {} parked",
                    block
                        .as_ref()
                        .map(|(id, q)| format!("{id}: {q}"))
                        .unwrap_or_else(|| "no block entry in log.jsonl".into()),
                    r.lane
                ),
            )?;
        }
        _ => {
            let w = find_ref(run, &session_id);
            match Limit::from(&exit.summary) {
                Some(limit) => settle_limited(run, board, &w, slice, &exit.summary, &limit, out)?,
                None => {
                    settle_dead(run, board, &w, slice, exit.code, exit.summary, out)?;
                }
            }
        }
    }
    Ok(())
}

/// A kill between the event and the commit leaves it logged: the resume must not log it twice.
fn done_logged(run: &Run, slice: u32, session_id: &str) -> Result<bool> {
    let events = run.events()?;
    let Some(spawned) = events.iter().rposition(|e| {
        matches!(&e.kind, EventKind::SliceSpawned { slice: s, session_id: sid, .. }
            if *s == slice && sid == session_id)
    }) else {
        return Ok(false);
    };
    Ok(events[spawned + 1..]
        .iter()
        .any(|e| matches!(e.kind, EventKind::SliceDone { slice: s } if s == slice)))
}

fn open_block(run: &Run, board: &Status, slice: u32) -> Result<Option<(String, String)>> {
    let feature_dir = run.state().repo.join(&board.dir);
    Ok(answers::open_block_of_slice(&feature_dir, slice)?.map(|e| (e.id, e.title)))
}

fn emit_blocked(run: &mut Run, slice: u32, block: Option<&(String, String)>) -> Result<()> {
    run.emit(EventKind::SliceBlocked {
        slice,
        block_id: block.map(|(id, _)| id.clone()),
        question: block.map(|(_, q)| q.clone()),
    })?;
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Landed {
    Done,
    Merging,
    Blocked,
    Waiting(String),
}

impl Landed {
    fn outcome(&self) -> Option<WorkerOutcome> {
        match self {
            Landed::Done | Landed::Merging => Some(WorkerOutcome::Done),
            Landed::Blocked => Some(WorkerOutcome::Blocked),
            Landed::Waiting(_) => None,
        }
    }
}

fn slice_ref(board: &Status, slice: u32) -> String {
    format!("refs/factory/{:03}/slice-{slice:02}", board.number)
}

/// Idempotent: a resumed driver calls it again for the same worker and goes on from where it was.
fn land_slice(
    run: &mut Run,
    board: &Status,
    slice: u32,
    row_id: i64,
    merged: bool,
    out: &mut dyn Write,
) -> Result<Landed> {
    let landed = run
        .db()
        .workers(run.id().as_str())?
        .into_iter()
        .any(|w| w.id == row_id && w.commit_sha.is_some());
    if landed {
        return Ok(Landed::Done);
    }
    let repo = run.state().repo.clone();
    let board_files = commit::tool_files(board).to_vec();
    let name = slice_ref(board, slice);
    let row = board.slices.iter().find(|s| s.number == slice);
    let lane = row.and_then(|s| s.lane);
    let base = lane.and_then(|l| run.state().lane_bases.get(&l).cloned());
    let title = row
        .and_then(|s| s.title.clone())
        .unwrap_or_else(|| format!("slice {slice:02}"));
    let subject = commit::subject(board, "feat", &format!("slice-{slice:02} {title}"));
    // Killed after the landing commit, before its record: a second pick would find nothing.
    if let Some(base) = &base {
        let range = format!("{base}..HEAD");
        let tip = git_ok(&repo, &["log", "--format=%H %s", &range])?;
        if let Some(landed) = tip
            .lines()
            .find_map(|l| l.split_once(' ').filter(|(_, s)| *s == subject))
            .map(|(sha, _)| sha.to_string())
        {
            if resolve(&repo, &name)?.is_some() {
                git_ok(&repo, &["update-ref", "-d", &name])?;
            }
            record_landing(run, slice, row_id, landed, out)?;
            return Ok(Landed::Done);
        }
    }
    let held = resolve(&repo, &name)?;
    let on_base = match &held {
        Some(sha) => !merged || resolve(&repo, &format!("{sha}^"))? == base,
        None => false,
    };
    let sha = match held {
        Some(sha) if on_base => sha,
        held => {
            let Some(lane) = lane else {
                return Err(Error::Msg(format!("slice {slice:02} is done with no lane")));
            };
            let Some(base) = base else {
                return Err(Error::Msg(format!(
                    "lane {lane} has no base in state.json to commit slice {slice:02} on"
                )));
            };
            let message = match &held {
                Some(sha) => git_ok(&repo, &["log", "-1", "--format=%B", sha])?
                    .trim()
                    .to_string(),
                None => subject.clone(),
            };
            let main = lanes::main_tree(&repo)?;
            let Some(c) = land::commit_lane(
                &lanes::dir(&paths::data_dir(), &main, lane),
                &base,
                &board_files,
                &commit::protected_files(board),
                &message,
                &name,
            )?
            else {
                // An empty merge is as likely a reset lane as a resolution: the owner says.
                if merged && let Some(held) = &held {
                    block_empty_merge(run, board, slice, held, lane, out)?;
                    return Ok(Landed::Blocked);
                }
                if held.is_some() {
                    git_ok(&repo, &["update-ref", "-d", &name])?;
                }
                say(out, &format!("slice {slice:02}: nothing to commit"))?;
                return Ok(Landed::Done);
            };
            for path in &c.dropped {
                run.emit(EventKind::WorkerWroteProtectedFile {
                    slice,
                    path: path.clone(),
                })?;
                say(
                    out,
                    &format!(
                        "slice {slice:02} wrote {path}, which no worker may — left out of the commit"
                    ),
                )?;
            }
            c.sha
        }
    };
    undo_half_pick(&repo, &sha)?;
    match land::land(&repo, &sha, &board_files)? {
        Landing::Landed { sha: landed } => {
            record_landing(run, slice, row_id, landed, out)?;
            Ok(Landed::Done)
        }
        Landing::Dirty { paths } => Ok(Landed::Waiting(format!(
            "slice {slice:02} cannot land: {} changed in the main tree — commit or discard them",
            paths.join(", ")
        ))),
        Landing::Conflict { paths } => {
            let session_id = run
                .state()
                .workers
                .iter()
                .find(|w| w.row_id == row_id)
                .map(|w| w.session_id.clone())
                .unwrap_or_default();
            start_merge(run, board, slice, &session_id, &sha, &paths, out)
        }
    }
}

/// The event follows the pick: a resumed driver that finds it knows the lane is ready.
fn start_merge(
    run: &mut Run,
    board: &Status,
    slice: u32,
    session_id: &str,
    sha: &str,
    paths: &[String],
    out: &mut dyn Write,
) -> Result<Landed> {
    let repo = run.state().repo.clone();
    let lane = board
        .slices
        .iter()
        .find(|s| s.number == slice)
        .and_then(|s| s.lane)
        .ok_or_else(|| Error::Msg(format!("slice {slice:02} conflicts with no lane")))?;
    let main = lanes::main_tree(&repo)?;
    let lane_dir = lanes::dir(&paths::data_dir(), &main, lane);
    let head = git_ok(&repo, &["rev-parse", "HEAD"])?.trim().to_string();
    lanes::reset(&lane_dir, &head)?;
    run.set_lane_base(lane, &head)?;
    // Its nonzero exit is the conflict this is for, once it left one.
    let pick = git_out(&lane_dir, &["cherry-pick", "--no-commit", sha])?;
    if !pick.status.success() && unmerged_paths(&lane_dir)?.is_empty() {
        return Err(Error::Msg(format!(
            "slice {slice:02}: the pick of {} in lane {lane} failed with no conflict: {}",
            short(sha),
            String::from_utf8_lossy(&pick.stderr).trim()
        )));
    }
    if !conflicted_since_spawn(&run.events()?, slice, session_id) {
        run.emit(EventKind::LandingConflicted {
            slice,
            paths: paths.to_vec(),
        })?;
    }
    let list = paths.join(", ");
    if conflicts_since_answer(&run.events()?, slice) >= CONFLICT_CAP {
        block_conflicted(run, board, slice, paths, out)?;
        return Ok(Landed::Blocked);
    }
    crate::docs::state::set(
        &repo.join(&board.dir),
        &[(&format!("slices.{slice}.status"), "building")],
    )?;
    say(
        out,
        &format!(
            "slice {slice:02} conflicts with the feature tip in {list} — the pick redone in lane {lane} for a merge worker, the main tree untouched"
        ),
    )?;
    Ok(Landed::Merging)
}

fn block_conflicted(
    run: &mut Run,
    board: &Status,
    slice: u32,
    paths: &[String],
    out: &mut dyn Write,
) -> Result<String> {
    let feature_dir = run.state().repo.join(&board.dir);
    let name = slice_ref(board, slice);
    let list = paths.join(", ");
    let body = format!(
        "**Happened:** slice {slice:02}'s landing conflicted with the feature tip {CONFLICT_CAP} times since its last answer, the last time in {list}: the tip kept moving under its merge workers. The main tree is untouched; the slice's commit is kept on `{name}` and its lane holds the pick, conflict included.\n\n\
         **Options:**\n\
         1. Answer to spawn one more merge worker on the lane as it is.\n\
         2. Land it by hand: cherry-pick `{name}` onto the feature branch, resolve {list}, commit, and set the slice `done`.\n\n\
         **Recommended:** 1 once the slices landing beside it are done.\n"
    );
    let title = format!("slice {slice:02} conflicts with the feature tip in {list}");
    let id = answers::add_block(&feature_dir, Some(slice), None, &title, &body)?;
    crate::docs::state::set(
        &feature_dir,
        &[(&format!("slices.{slice}.status"), "blocked")],
    )?;
    emit_blocked(run, slice, Some(&(id.clone(), title)))?;
    say(
        out,
        &format!(
            "slice {slice:02} conflicts with the feature tip in {list} for the {CONFLICT_CAP}rd time — slice blocked ({id}), its commit kept on {name}"
        ),
    )?;
    Ok(id)
}

fn block_empty_merge(
    run: &mut Run,
    board: &Status,
    slice: u32,
    held: &str,
    lane: u32,
    out: &mut dyn Write,
) -> Result<String> {
    let feature_dir = run.state().repo.join(&board.dir);
    let name = slice_ref(board, slice);
    let body = format!(
        "**Happened:** slice {slice:02}'s merge worker returned and its lane {lane} holds nothing the feature tip lacks: either the resolution took the tip's side everywhere, or the lane was reset under the respawn and the pick is gone. Nothing landed; the slice's commit `{}` is kept on `{name}`.\n\n\
         **Options:**\n\
         1. Cherry-pick `{name}` onto the feature branch, resolve, commit, and set the slice `done`.\n\
         2. Answer to spawn one more merge worker; if the lane lost the pick, it finds nothing to resolve again.\n\
         3. The slice's work is already on the tip: delete `{name}` and set the slice `done`.\n\n\
         **Recommended:** 1.\n",
        short(held)
    );
    let title = format!("slice {slice:02}'s merge left nothing to land");
    let id = answers::add_block(&feature_dir, Some(slice), None, &title, &body)?;
    crate::docs::state::set(
        &feature_dir,
        &[(&format!("slices.{slice}.status"), "blocked")],
    )?;
    emit_blocked(run, slice, Some(&(id.clone(), title)))?;
    say(
        out,
        &format!(
            "slice {slice:02}: its merge left nothing to land — slice blocked ({id}), its commit kept on {name}"
        ),
    )?;
    Ok(id)
}

fn route_merge(
    run: &mut Run,
    board: &Status,
    w: &WorkerRef,
    slice: u32,
    exit_code: Option<i32>,
    summary: Summary,
    out: &mut dyn Write,
) -> Result<Option<String>> {
    let repo = run.state().repo.clone();
    let feature_dir = repo.join(&board.dir);
    if let Some(block) = open_block(run, board, slice)? {
        merge_blocked(
            run,
            board,
            w,
            slice,
            &summary,
            block,
            "it logged a block",
            out,
        )?;
        return Ok(None);
    }
    if let Some(limit) = Limit::from(&summary) {
        settle(
            run,
            w.row_id,
            &w.session_id,
            &summary,
            WorkerOutcome::RateLimited,
        )?;
        let until = quota::pause(run, &limit, out)?;
        say(
            out,
            &format!(
                "slice {slice:02}: merge worker {} hit a usage limit ({}) — the lane kept for the next one, paused until {}",
                w.session_id,
                limit.literal,
                clock::rfc3339(until)
            ),
        )?;
        return Ok(None);
    }
    if summary.is_dead() {
        settle_dead(run, board, w, slice, exit_code, summary, out)?;
        return Ok(None);
    }
    let lane = board
        .slices
        .iter()
        .find(|s| s.number == slice)
        .and_then(|s| s.lane)
        .ok_or_else(|| Error::Msg(format!("slice {slice:02} is merging with no lane")))?;
    let lane_dir = lanes::dir(&paths::data_dir(), &lanes::main_tree(&repo)?, lane);
    // A respawn reads the lane as it is, not as the first pick left it.
    let mut conflicted = last_conflict(&run.events()?, slice).map_or_else(Vec::new, |(_, p)| p);
    let unmerged = unmerged_paths(&lane_dir)?;
    for p in unmerged.keys() {
        if !conflicted.contains(p) {
            conflicted.push(p.clone());
        }
    }
    let marked: Vec<String> = conflicted
        .into_iter()
        .filter(|p| has_marker(&lane_dir.join(p)))
        .collect();
    // A conflict no marker shows (a side deleted, a binary file) is open until staged.
    let unmarked: Vec<String> = unmerged
        .into_iter()
        .filter(|(p, stages)| {
            !marked.contains(p) && (stages.len() < 3 || is_binary_conflict(&lane_dir, p))
        })
        .map(|(p, _)| p)
        .collect();
    if !marked.is_empty() || !unmarked.is_empty() {
        let name = slice_ref(board, slice);
        let mut what = Vec::new();
        if !marked.is_empty() {
            what.push(format!(
                "a conflict marker still at the start of a line in {}",
                marked.join(", ")
            ));
        }
        if !unmarked.is_empty() {
            what.push(format!(
                "{} still unmerged, a conflict no marker shows (a side deleted, a binary file)",
                unmarked.join(", ")
            ));
        }
        let list = marked
            .iter()
            .chain(&unmarked)
            .cloned()
            .collect::<Vec<_>>()
            .join(", ");
        let body = format!(
            "**Happened:** the merge worker `{}` on slice {slice:02} returned with {} (lane {lane}). Nothing landed; the main tree is untouched and the slice's commit is kept on `{name}`.\n\n\
             **Options:**\n\
             1. Answer with how to resolve {list}: one more merge worker gets the lane as it is, and the answer.\n\
             2. Resolve {list} in the lane by hand and stage it, then answer: the next merge worker finds nothing open and lands it.\n\n\
             **Recommended:** 1.\n",
            w.session_id,
            what.join("; and ")
        );
        let title = if unmarked.is_empty() {
            format!("slice {slice:02}'s merge left conflict markers in {list}")
        } else {
            format!("slice {slice:02}'s merge left conflicts open in {list}")
        };
        let id = answers::add_block(&feature_dir, Some(slice), None, &title, &body)?;
        merge_blocked(
            run,
            board,
            w,
            slice,
            &summary,
            (id, title),
            "markers left",
            out,
        )?;
        return Ok(None);
    }
    match land_slice(run, board, slice, w.row_id, true, out)? {
        Landed::Waiting(what) => return Ok(Some(what)),
        Landed::Done => {
            crate::docs::state::set(&feature_dir, &[(&format!("slices.{slice}.status"), "done")])?;
            settle(run, w.row_id, &w.session_id, &summary, WorkerOutcome::Done)?;
            say(
                out,
                &format!(
                    "slice {slice:02} merged (merge worker {}), lane {lane} free",
                    w.session_id
                ),
            )?;
        }
        l => {
            let outcome = l.outcome().expect("a settled landing");
            settle(run, w.row_id, &w.session_id, &summary, outcome)?;
        }
    }
    Ok(None)
}

#[allow(clippy::too_many_arguments)]
fn merge_blocked(
    run: &mut Run,
    board: &Status,
    w: &WorkerRef,
    slice: u32,
    summary: &Summary,
    block: (String, String),
    why: &str,
    out: &mut dyn Write,
) -> Result<()> {
    crate::docs::state::set(
        &run.state().repo.join(&board.dir),
        &[(&format!("slices.{slice}.status"), "blocked")],
    )?;
    settle(
        run,
        w.row_id,
        &w.session_id,
        summary,
        WorkerOutcome::Blocked,
    )?;
    emit_blocked(run, slice, Some(&block))?;
    say(
        out,
        &format!(
            "slice {slice:02}: merge worker {} returned — {why}, slice blocked ({}: {})",
            w.session_id, block.0, block.1
        ),
    )
}

/// `=======` alone is as often a Markdown heading's underline: not read.
fn has_marker(path: &Path) -> bool {
    fs::read(path).is_ok_and(|bytes| {
        String::from_utf8_lossy(&bytes)
            .lines()
            .any(|l| l.starts_with("<<<<<<<") || l.starts_with(">>>>>>>"))
    })
}

/// Stages as git numbers them: 1 the base, 2 the tip, 3 the slice's commit.
fn unmerged_paths(lane: &Path) -> Result<std::collections::BTreeMap<String, Vec<u8>>> {
    let mut paths = std::collections::BTreeMap::<String, Vec<u8>>::new();
    for entry in nul_split(&git_ok(lane, &["ls-files", "-u", "-z"])?) {
        // `<mode> <sha> <stage>\t<path>`
        let Some((meta, path)) = entry.split_once('\t') else {
            continue;
        };
        let stage = meta
            .rsplit(' ')
            .next()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        paths.entry(path.to_string()).or_default().push(stage);
    }
    Ok(paths)
}

/// Binary as git judges it (a NUL in the first 8000 bytes): git leaves no marker in those.
fn is_binary_conflict(lane: &Path, path: &str) -> bool {
    git_out(lane, &["cat-file", "blob", &format!(":2:{path}")])
        .is_ok_and(|o| o.stdout.iter().take(8000).any(|b| *b == 0))
}

fn answer_from(events: &[events::Event], slice: u32) -> usize {
    events
        .iter()
        .rposition(
            |e| matches!(&e.kind, EventKind::BlockAnswered { slice: Some(s), .. } if *s == slice),
        )
        .map_or(0, |i| i + 1)
}

pub fn conflicts_since_answer(events: &[events::Event], slice: u32) -> usize {
    events[answer_from(events, slice)..]
        .iter()
        .filter(|e| matches!(&e.kind, EventKind::LandingConflicted { slice: s, .. } if *s == slice))
        .count()
}

fn last_conflict(events: &[events::Event], slice: u32) -> Option<(usize, Vec<String>)> {
    events
        .iter()
        .enumerate()
        .rev()
        .find_map(|(i, e)| match &e.kind {
            EventKind::LandingConflicted { slice: s, paths } if *s == slice => {
                Some((i, paths.clone()))
            }
            _ => None,
        })
}

fn conflicted_since_spawn(events: &[events::Event], slice: u32, session_id: &str) -> bool {
    events
        .iter()
        .rposition(|e| match &e.kind {
            EventKind::SliceSpawned { session_id: s, .. }
            | EventKind::MergeSpawned { session_id: s, .. } => s == session_id,
            _ => false,
        })
        .is_some_and(|at| {
            events[at + 1..].iter().any(
                |e| matches!(&e.kind, EventKind::LandingConflicted { slice: s, .. } if *s == slice),
            )
        })
}

fn record_landing(
    run: &mut Run,
    slice: u32,
    row_id: i64,
    landed: String,
    out: &mut dyn Write,
) -> Result<()> {
    let repo = run.state().repo.clone();
    let files = nul_split(&git_ok(
        &repo,
        &[
            "diff-tree",
            "--no-commit-id",
            "--name-only",
            "-r",
            "-z",
            &landed,
        ],
    )?);
    run.db().set_worker_commit(row_id, &landed)?;
    say(
        out,
        &format!(
            "slice {slice:02} landed {} — {} file(s): {}",
            short(&landed),
            files.len(),
            files.join(", ")
        ),
    )?;
    run.emit(EventKind::SliceCommitted {
        slice,
        sha: landed,
        files,
    })?;
    Ok(())
}

/// Paths staged exactly as `sha` has them are a killed driver's half pick, not the owner's work.
pub(crate) fn undo_half_pick(repo: &Path, sha: &str) -> Result<()> {
    let parent = format!("{sha}^");
    let picked = nul_split(&git_ok(
        repo,
        &["diff", "--name-only", "--no-renames", "-z", &parent, sha],
    )?);
    if picked.is_empty() {
        return Ok(());
    }
    let on = |args: &[&str]| -> Vec<String> {
        args.iter()
            .map(|a| a.to_string())
            .chain(["--".to_string()])
            .chain(picked.iter().cloned())
            .collect()
    };
    let quiet = |args: &[&str]| -> Result<bool> {
        let all = on(args);
        let refs: Vec<&str> = all.iter().map(String::as_str).collect();
        Ok(git_out(repo, &refs)?.status.success())
    };
    if !quiet(&["diff", "--cached", "--quiet", "HEAD"])?
        && quiet(&["diff", "--cached", "--quiet", sha])?
        && quiet(&["diff", "--quiet", sha])?
    {
        let all = on(&["restore", "--source=HEAD", "--staged", "--worktree"]);
        let refs: Vec<&str> = all.iter().map(String::as_str).collect();
        git_ok(repo, &refs)?;
    }
    Ok(())
}

pub(crate) fn resolve(repo: &Path, name: &str) -> Result<Option<String>> {
    let o = git_out(
        repo,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("{name}^{{commit}}"),
        ],
    )?;
    Ok(o.status
        .success()
        .then(|| String::from_utf8_lossy(&o.stdout).trim().to_string()))
}

pub(crate) fn nul_split(text: &str) -> Vec<String> {
    text.split('\0')
        .filter(|p| !p.is_empty())
        .map(str::to_string)
        .collect()
}

fn git_out(repo: &Path, args: &[&str]) -> Result<std::process::Output> {
    std::process::Command::new("git")
        .current_dir(repo)
        .args(args)
        .output()
        .map_err(io(format!("run git {}", args.join(" "))))
}

pub(crate) fn git_ok(repo: &Path, args: &[&str]) -> Result<String> {
    let o = git_out(repo, args)?;
    if !o.status.success() {
        return Err(Error::Tool {
            cmd: format!("git {}", args.join(" ")),
            detail: String::from_utf8_lossy(&o.stderr).trim().to_string(),
        });
    }
    Ok(String::from_utf8_lossy(&o.stdout).into_owned())
}

pub fn short(sha: &str) -> &str {
    &sha[..sha.len().min(7)]
}

fn settle_orphans(
    run: &mut Run,
    board: &Status,
    live: &HashSet<String>,
    waiting: &mut HashSet<String>,
    out: &mut dyn Write,
) -> Result<Orphans> {
    let unsettled: Vec<WorkerRef> = run
        .state()
        .workers
        .iter()
        .filter(|w| w.outcome.is_none() && !live.contains(&w.session_id))
        .cloned()
        .collect();
    if unsettled.is_empty() {
        return Ok(Orphans::None);
    }
    let (running, dead): (Vec<WorkerRef>, Vec<WorkerRef>) = unsettled
        .into_iter()
        .partition(|w| run.state().worker_alive(w));
    for w in &running {
        if waiting.insert(w.session_id.clone()) {
            say(
                out,
                &format!(
                    "{}: worker {} (pid {}) of an earlier driver is still running — waiting",
                    label(w),
                    w.session_id,
                    w.pid
                ),
            )?;
        }
    }
    let outcome = if running.is_empty() {
        Orphans::Settled
    } else {
        Orphans::Live
    };
    let mut landing = None;
    for w in dead {
        let summary = worker::summary_from_disk(run.dir(), &w.session_id);
        let Some(slice) = w.slice else {
            if w.role == "polish" || (w.role == "fix" && w.checkpoint.is_none()) {
                if let Some(what) = closeout::settle_orphan(run, board, &w, summary, out)? {
                    landing = Some(what);
                }
            } else if let Some(what) = settle_checkpoint_orphan(run, board, &w, summary, out)? {
                landing = Some(what);
            }
            continue;
        };
        // A conflict since the spawn: the lane holds the pick; `done`: it has not flipped yet.
        let status = slice_status(board, slice);
        if status != "done" && conflicted_since_spawn(&run.events()?, slice, &w.session_id) {
            let outcome = if status == "blocked" {
                WorkerOutcome::Blocked
            } else {
                WorkerOutcome::Done
            };
            settle(run, w.row_id, &w.session_id, &summary, outcome)?;
            continue;
        }
        if w.role == "merge" {
            if let Some(what) = route_merge(run, board, &w, slice, None, summary, out)? {
                landing = Some(what);
            }
            continue;
        }
        // The board was read before this loop: route on what the driver just wrote.
        let status = done_by_driver(run, board, slice, &w.role, w.lane, &summary, out)?;
        match status.as_str() {
            "done" => {
                if !done_logged(run, slice, &w.session_id)? {
                    run.emit(EventKind::SliceDone { slice })?;
                }
                // A landing under way (its ref made) is retried every tick: said once, at first.
                if resolve(&run.state().repo, &slice_ref(board, slice))?.is_none() {
                    say(
                        out,
                        &format!(
                            "slice {:02} done by worker {} of an earlier driver",
                            slice, w.session_id
                        ),
                    )?;
                }
                match land_slice(run, board, slice, w.row_id, false, out)? {
                    Landed::Waiting(what) => landing = Some(what),
                    l => {
                        let outcome = l.outcome().expect("a settled landing");
                        settle(run, w.row_id, &w.session_id, &summary, outcome)?;
                    }
                }
            }
            "blocked" => {
                settle(
                    run,
                    w.row_id,
                    &w.session_id,
                    &summary,
                    WorkerOutcome::Blocked,
                )?;
                let block = open_block(run, board, slice)?;
                emit_blocked(run, slice, block.as_ref())?;
            }
            _ => match Limit::from(&summary) {
                Some(limit) => settle_limited(run, board, &w, slice, &summary, &limit, out)?,
                None => {
                    settle_dead(run, board, &w, slice, None, summary, out)?;
                }
            },
        }
    }
    Ok(landing.map(Orphans::Landing).unwrap_or(outcome))
}

fn label(w: &WorkerRef) -> String {
    match (w.slice, w.checkpoint) {
        (Some(s), _) => format!("slice {s:02}"),
        (None, Some(n)) => format!("checkpoint {n} ({})", w.role),
        (None, None) => format!("closeout ({})", w.role),
    }
}

/// A fix worker's lane lands however it ended; a checkpoint worker's once `CP-n` is on the board.
fn settle_checkpoint_orphan(
    run: &mut Run,
    board: &Status,
    w: &WorkerRef,
    summary: Summary,
    out: &mut dyn Write,
) -> Result<Option<String>> {
    let n = w.checkpoint.unwrap_or(0);
    let repo = run.state().repo.clone();
    let lands =
        w.role == "fix" || (w.role == "checkpoint" && board.checkpoints.iter().any(|c| c.n == n));
    let mut sha = None;
    if lands {
        let attempt = if w.role == "fix" {
            run.db()
                .gates(run.id().as_str())?
                .iter()
                .filter(|g| g.checkpoint == Some(n))
                .filter_map(|g| g.attempt)
                .max()
                .unwrap_or(0)
        } else {
            0
        };
        match checkpoint::land_worker(run, board, n, &w.role, attempt, w.lane, w.row_id, out)? {
            checkpoint::Landed::Done(landed) => sha = landed,
            checkpoint::Landed::Waiting(what) => return Ok(Some(what)),
            checkpoint::Landed::Conflict { paths, name } => {
                checkpoint::block_conflict(
                    run, board, n, &w.role, attempt, w.row_id, &paths, &name, out,
                )?;
                settle(
                    run,
                    w.row_id,
                    &w.session_id,
                    &summary,
                    WorkerOutcome::Blocked,
                )?;
                return Ok(None);
            }
        }
    }
    if w.role == "checkpoint" && lands {
        settle(run, w.row_id, &w.session_id, &summary, WorkerOutcome::Done)?;
        let logged = run
            .events()?
            .iter()
            .any(|e| matches!(e.kind, EventKind::CheckpointDone { n: m, .. } if m == n));
        if !logged {
            run.emit(EventKind::CheckpointDone { n, sha })?;
        }
        say(
            out,
            &format!(
                "checkpoint {n}: worker {} of an earlier driver wrote CP-{n} — checkpoint {n} done",
                w.session_id
            ),
        )?;
        return Ok(None);
    }
    if let Some(limit) = Limit::from(&summary) {
        settle(
            run,
            w.row_id,
            &w.session_id,
            &summary,
            WorkerOutcome::RateLimited,
        )?;
        let until = quota::pause(run, &limit, out)?;
        say(
            out,
            &format!(
                "checkpoint {n}: {} worker {} of an earlier driver hit a usage limit ({}) — paused until {}, the checkpoint runs again",
                w.role,
                w.session_id,
                limit.literal,
                clock::rfc3339(until)
            ),
        )?;
        return Ok(None);
    }
    if w.role == "fix" && !summary.is_dead() {
        settle(run, w.row_id, &w.session_id, &summary, WorkerOutcome::Done)?;
        say(
            out,
            &format!(
                "checkpoint {n}: fix worker {} of an earlier driver returned — the gate runs again",
                w.session_id
            ),
        )?;
        return Ok(None);
    }
    let outcome = if summary.is_dead() {
        WorkerOutcome::Dead
    } else {
        WorkerOutcome::Failed
    };
    settle(run, w.row_id, &w.session_id, &summary, outcome)?;
    run.emit(EventKind::WorkerDead {
        slice: None,
        session_id: w.session_id.clone(),
        exit_code: None,
    })?;
    if w.role == "checkpoint" {
        answers::add_note(
            &repo.join(&board.dir),
            None,
            "Dead checkpoint worker",
            &format!(
                "checkpoint {n}, session {} exited (code unknown) with {}.",
                w.session_id,
                if summary.is_dead() {
                    "no result"
                } else {
                    "a result but neither CP entry nor block"
                }
            ),
        )?;
    }
    say(
        out,
        &format!(
            "checkpoint {n}: {} worker {} of an earlier driver is gone ({}) — the checkpoint runs again",
            w.role,
            w.session_id,
            outcome.as_str()
        ),
    )?;
    Ok(None)
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

#[allow(clippy::too_many_arguments)]
fn settle_limited(
    run: &mut Run,
    board: &Status,
    w: &WorkerRef,
    slice: u32,
    summary: &Summary,
    limit: &Limit,
    out: &mut dyn Write,
) -> Result<()> {
    settle(
        run,
        w.row_id,
        &w.session_id,
        summary,
        WorkerOutcome::RateLimited,
    )?;
    // An empty value removes the key: the lane is freed, not set to "".
    crate::docs::state::set(
        &run.state().repo.join(&board.dir),
        &[
            (&format!("slices.{slice}.status"), "pending"),
            (&format!("slices.{slice}.lane"), ""),
        ],
    )?;
    let until = quota::pause(run, limit, out)?;
    say(
        out,
        &format!(
            "slice {slice:02}: worker {} hit a usage limit ({}) — back to pending, paused until {}",
            w.session_id,
            limit.literal,
            clock::rfc3339(until)
        ),
    )
}

#[allow(clippy::too_many_arguments)]
fn settle_dead(
    run: &mut Run,
    board: &Status,
    w: &WorkerRef,
    slice: u32,
    exit_code: Option<i32>,
    summary: Summary,
    out: &mut dyn Write,
) -> Result<bool> {
    let outcome = if summary.is_dead() {
        WorkerOutcome::Dead
    } else {
        WorkerOutcome::Failed
    };
    settle(run, w.row_id, &w.session_id, &summary, outcome)?;
    run.emit(EventKind::WorkerDead {
        slice: Some(slice),
        session_id: w.session_id.clone(),
        exit_code,
    })?;
    let dead = deaths_since_answer(run, slice, &w.role)?;
    let feature_dir = run.state().repo.join(&board.dir);
    let code = exit_code.map_or("exited (code unknown)".to_string(), |c| {
        format!("exited {c}")
    });
    let what = if summary.is_dead() {
        "with no result"
    } else {
        "with a result but the slice neither done nor blocked"
    };
    if dead.len() >= DEATH_CAP {
        let block = block_dead_slice(
            run,
            board,
            slice,
            &dead,
            &format!("The last one {code} {what}. "),
        )?;
        answers::add_note(
            &feature_dir,
            Some(slice),
            "Dead worker",
            &format!(
                "session {} {code} {what}; second death on this slice, blocked ({block}).",
                w.session_id
            ),
        )?;
        say(
            out,
            &format!(
                "slice {:02}: worker {} died ({code}, {what}) — second death, slice blocked ({block})",
                slice, w.session_id
            ),
        )?;
        Ok(true)
    } else {
        answers::add_note(
            &feature_dir,
            Some(slice),
            "Dead worker",
            &format!("session {} {code} {what}; respawned.", w.session_id),
        )?;
        say(
            out,
            &format!(
                "slice {:02}: worker {} died ({code}, {what}) — respawning once",
                slice, w.session_id
            ),
        )?;
        Ok(false)
    }
}

fn dead_title(slice: u32) -> String {
    format!("Two workers died on slice {:02}", slice)
}

/// A worker with no spawn event (a kill inside the spawn) counts: the cap errs toward the owner.
pub fn deaths_since_answer(run: &Run, slice: u32, role: &str) -> Result<Vec<WorkerRef>> {
    let events = run.events()?;
    let from = answer_from(&events, slice);
    let before: HashSet<&str> = events[..from]
        .iter()
        .filter_map(|e| match &e.kind {
            EventKind::SliceSpawned {
                slice: s,
                session_id,
                ..
            }
            | EventKind::MergeSpawned {
                slice: s,
                session_id,
                ..
            } if *s == slice => Some(session_id.as_str()),
            _ => None,
        })
        .collect();
    Ok(run
        .state()
        .workers
        .iter()
        .filter(|w| {
            w.slice == Some(slice)
                && w.role == role
                && w.died()
                && !before.contains(w.session_id.as_str())
        })
        .cloned()
        .collect())
}

/// One a kill left: written after the entry, before the slice flipped.
fn open_dead_block(feature_dir: &Path, slice: u32) -> Result<Option<answers::BlockEntry>> {
    let title = dead_title(slice);
    Ok(answers::entries(feature_dir)?
        .into_iter()
        .rfind(|e| !e.resolved && e.slice == Some(slice) && e.title == title))
}

fn block_dead_slice(
    run: &mut Run,
    board: &Status,
    slice: u32,
    dead: &[WorkerRef],
    last: &str,
) -> Result<String> {
    let repo = run.state().repo.clone();
    let feature_dir = repo.join(&board.dir);
    let title = dead_title(slice);
    let block = if let Some(e) = open_dead_block(&feature_dir, slice)? {
        e.id
    } else {
        let sessions: Vec<String> = dead
            .iter()
            .map(|x| {
                format!(
                    "`{}` ({})",
                    x.session_id,
                    x.outcome.as_deref().unwrap_or("?")
                )
            })
            .collect();
        let body = format!(
            "**Happened:** {n} workers on slice {slice:02} exited without leaving it `done` or `blocked`: {sessions}. \
             {last}The slice sits at `building` on the board with nothing alive on it; \
             the driver's once-only respawn is spent. \
             Streams and stderr are under `{dir}/`.\n\
             **Options:** 1. read the last worker's `.stderr` and `.jsonl`, fix the environment (login, PATH, quota) and set the slice back to `pending`; \
             2. build the slice by hand and set it `done`; \
             3. drop the slice from the plan if it no longer applies.\n\
             **Recommended:** 1 — a worker that dies before its first tool call is the environment's fault, not the slice's.\n",
            n = dead.len(),
            sessions = sessions.join(", "),
            dir = worker::dir(run.dir()).display(),
        );
        answers::add_block(&feature_dir, Some(slice), None, &title, &body)?
    };
    crate::docs::state::set(
        &feature_dir,
        &[(&format!("slices.{slice}.status"), "blocked")],
    )?;
    emit_blocked(run, slice, Some(&(block.clone(), title)))?;
    Ok(block)
}

fn check_title(slice: u32) -> String {
    format!("docs check failed in slice {:02}'s lane", slice)
}

/// A build worker's `done` hint on a `building` slice: a clean check of its lane writes `done`, errors block it.
/// Returns the status to route on; every other exit leaves the board as it was.
fn done_by_driver(
    run: &mut Run,
    board: &Status,
    slice: u32,
    role: &str,
    lane: Option<u32>,
    summary: &Summary,
    out: &mut dyn Write,
) -> Result<String> {
    let status = slice_status(board, slice);
    if role != "build" || summary.status_hint != Some(StatusHint::Done) || status != "building" {
        return Ok(status.to_string());
    }
    let row = board.slices.iter().find(|s| s.number == slice);
    let Some(lane) = lane.or_else(|| row.and_then(|s| s.lane)) else {
        return Ok(status.to_string());
    };
    let repo = run.state().repo.clone();
    let feature_dir = repo.join(&board.dir);
    let lane_dir = lanes::dir(&paths::data_dir(), &lanes::main_tree(&repo)?, lane);
    let report = crate::docs::check::run(&lane_dir);
    if report.errors.is_empty() {
        crate::docs::state::set(&feature_dir, &[(&format!("slices.{slice}.status"), "done")])?;
        return Ok("done".into());
    }
    let title = check_title(slice);
    // A kill between the entry and the flip left it open: a restart writes no second one.
    let open = answers::entries(&feature_dir)?
        .into_iter()
        .rfind(|e| !e.resolved && e.slice == Some(slice) && e.title == title);
    if open.is_none() {
        let errors: Vec<String> = report
            .errors
            .iter()
            .map(|(code, msg)| format!("- `{code}` {msg}"))
            .collect();
        let body = format!(
            "**Happened:** the build worker on slice {slice:02} returned `STATUS: done`, \
             and `factory docs check` in its lane ({lane_dir}) found {n} error{s}:\n{errors}\n\
             **Options:** 1. fix the docs in the lane by hand, then set the slice `done`; \
             2. set the slice back to `pending` for a fresh worker.\n\
             **Recommended:** 1 — the work is built and only its docs fail the check.\n",
            lane_dir = lane_dir.display(),
            n = report.errors.len(),
            s = if report.errors.len() == 1 { "" } else { "s" },
            errors = errors.join("\n"),
        );
        answers::add_block(&feature_dir, Some(slice), None, &title, &body)?;
    }
    crate::docs::state::set(
        &feature_dir,
        &[(&format!("slices.{slice}.status"), "blocked")],
    )?;
    say(
        out,
        &format!(
            "slice {slice:02}: docs check in lane {lane} found {} — blocking",
            report.counts()
        ),
    )?;
    Ok("blocked".into())
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
    // The event before the phase: whoever reads `finished` finds `run_finished`.
    let r = report::compute(run.db(), run.id().as_str(), None)?;
    let started = clock::from_rfc3339(&run.state().started_at).unwrap_or_else(clock::unix_now);
    let duration_minutes =
        (clock::unix_now().saturating_sub(started) as f64 / 60.0 * 100.0).round() / 100.0;
    if !logged(run, |k| matches!(k, EventKind::RunFinished { .. }))? {
        run.emit(EventKind::RunFinished {
            pr_url: run.state().pr_url.clone(),
            slices: r.slices,
            checkpoints: r.checkpoints,
            ci_cycles: r.ci_cycles,
            weighted_tokens: r.weighted_tokens,
            duration_minutes,
        })?;
    }
    run.set_phase(Phase::Finished)?;
    say(
        out,
        &format!(
            "run {} finished ({why}){}",
            run.id(),
            run.state()
                .pr_url
                .as_deref()
                .map_or(String::new(), |u| format!(" — PR {u}"))
        ),
    )?;
    Ok(Phase::Finished)
}

/// `run_stopped`, not `run_finished`, is the last line: the run did not reach its end.
fn stopped(run: &mut Run, out: &mut dyn Write) -> Result<Phase> {
    let slices = mark_stopped(run)?;
    say(
        out,
        &format!(
            "run {} stopped ({slices} slice(s) done) — factory run {} resumes the feature in a new run",
            run.id(),
            run.state().feature
        ),
    )?;
    Ok(Phase::Finished)
}

pub fn mark_stopped(run: &mut Run) -> Result<u64> {
    let r = report::compute(run.db(), run.id().as_str(), None)?;
    let started = clock::from_rfc3339(&run.state().started_at).unwrap_or_else(clock::unix_now);
    let duration_minutes =
        (clock::unix_now().saturating_sub(started) as f64 / 60.0 * 100.0).round() / 100.0;
    // The event first, as in `finish`.
    if !logged(run, |k| matches!(k, EventKind::RunStopped { .. }))? {
        run.emit(EventKind::RunStopped {
            slices: r.slices,
            weighted_tokens: r.weighted_tokens,
            duration_minutes,
        })?;
    }
    run.set_stopped()?;
    Ok(r.slices)
}

fn logged(run: &Run, kind: impl Fn(&EventKind) -> bool) -> Result<bool> {
    Ok(run.events()?.iter().any(|e| kind(&e.kind)))
}

pub fn say(out: &mut dyn Write, line: &str) -> Result<()> {
    writeln!(out, "{} {line}", clock::now_rfc3339()).map_err(io("driver output"))
}

pub fn pad_list(ns: &[u32]) -> String {
    ns.iter()
        .map(|n| format!("{n:02}"))
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn slice_status(board: &Status, slice: u32) -> &str {
    board
        .slices
        .iter()
        .find(|s| s.number == slice)
        .and_then(|s| s.status.as_deref())
        .unwrap_or("")
}

/// The temp file sits beside `path` so the rename stays on one filesystem.
pub fn write_atomic(path: &Path, text: &str) -> Result<()> {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let tmp = path.with_file_name(format!(".{name}.{}.tmp", std::process::id()));
    fs::write(&tmp, text).map_err(io(format!("write {}", tmp.display())))?;
    fs::rename(&tmp, path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        io(format!("rename {} to {}", tmp.display(), path.display()))(e)
    })
}

pub fn packs_dir(run_dir: &Path) -> PathBuf {
    run_dir.join("packs")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::Event;

    fn ev(seq: u64, kind: EventKind) -> Event {
        Event {
            seq,
            ts: String::new(),
            kind,
        }
    }

    fn conflict(seq: u64, slice: u32) -> Event {
        ev(
            seq,
            EventKind::LandingConflicted {
                slice,
                paths: vec!["shared.txt".into()],
            },
        )
    }

    fn answer(seq: u64, slice: u32) -> Event {
        ev(
            seq,
            EventKind::BlockAnswered {
                id: "BLOCK-1".into(),
                slice: Some(slice),
                text: "go".into(),
            },
        )
    }

    #[test]
    fn the_third_conflict_since_an_answer_blocks_and_an_answer_starts_over() {
        let mut events = vec![answer(1, 2), conflict(2, 2), conflict(3, 2), conflict(4, 1)];
        assert_eq!(conflicts_since_answer(&events, 2), 2);
        events.push(conflict(5, 2));
        assert!(conflicts_since_answer(&events, 2) >= CONFLICT_CAP);

        let mut events = vec![conflict(1, 2), conflict(2, 2), answer(3, 2)];
        assert_eq!(conflicts_since_answer(&events, 2), 0);
        events.push(conflict(4, 2));
        assert!(conflicts_since_answer(&events, 2) < CONFLICT_CAP);
        assert_eq!(last_conflict(&events, 2).map(|(i, _)| i), Some(3));
        assert_eq!(answer_from(&events, 2), 3);
    }

    #[test]
    fn a_marker_is_a_line_starting_with_seven_angles() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("f.md");
        fs::write(&path, "Title\n=======\n  <<<<<<< not at the start\n").unwrap();
        assert!(!has_marker(&path));
        fs::write(&path, "a\n<<<<<<< HEAD\none\n=======\ntwo\n>>>>>>> abc\n").unwrap();
        assert!(has_marker(&path));
        fs::write(&path, "a\n>>>>>>> abc\n").unwrap();
        assert!(has_marker(&path));
        assert!(!has_marker(&tmp.path().join("gone.txt")));
    }
}
