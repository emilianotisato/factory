//! The loop (PRD §3 FR-1, §4 FR-2, §5 FR-3). Each tick: reap the workers that
//! exited (close their rows, re-read the board, route on the slice's
//! frontmatter — `done`, `blocked`, or neither, which is a dead worker), then
//! read `status --json` and spawn as many workers as the board allows, one
//! lane each, writing `lane: N` and `status: building` into the slice before
//! the spawn so the next board read sees the lane held. The board is the
//! truth; the worker's `STATUS:` line is a hint the row keeps.
//!
//! A `blocked` slice parks its lane: the lane stays on the slice and the
//! driver keeps it out of the free set even though the tool frees it (AD-401),
//! while independent slices keep building. When nothing runs and nothing is
//! spawnable with a block or a preflight ask open, the run parks in
//! `waiting-owner` and the driver stays alive at a slow tick watching
//! `runs/<id>/answers/` (`factory answer`). An answered slice is `building`
//! with nothing alive on it and an `answers/BLOCK-n.json` naming it: it gets a
//! fresh worker whose pack carries the block and the answer. The last `ASK-n`
//! answered sends the run through branch and board as the `pass` path does.
//!
//! Dead workers (FR-1, autonomy.md:76): the first death on a slice is a note
//! in the feature's `log.jsonl` and a fresh worker with a new session; the
//! second since the owner last answered a block on it blocks the slice with
//! a `block` entry, its id allocated by the tool. The count lives in
//! `state.json` `workers[]` and `events.jsonl`, so a resume after a kill
//! counts the killed worker, and the spawn loop blocks a capped slice
//! instead of giving it a third worker.
//!
//! PRD-005: a build worker runs in its lane worktree. A slice taking a free
//! lane resets it to the feature tip first; a respawn keeps the lane as its
//! worker left it. A worker whose slice reads `done` has its lane committed
//! whole and landed on the feature tip by cherry-pick ([`land_slice`]), one
//! commit per slice with the board files. The worker's outcome is saved
//! after the landing, so a driver killed in between leaves it unsettled and
//! the resume lands. A path the pick touches dirty in the main tree parks
//! the run with nothing spawned until it lands; a dependent slice waits for
//! its dependency's landing. Every commit lands on the run's branch: HEAD
//! anywhere else parks the run until the owner switches back, with nothing
//! reaped or spawned meanwhile (AD-20005).
//!
//! FR-4: when the board says `checkpoint_due` nothing new spawns; once the
//! table is empty the loop hands the run to [`checkpoint::run`] — the gate,
//! the fix workers, the checkpoint worker and the checkpoint
//! commit — and reads the board again when it is done. The end of the run
//! is the last checkpoint: `all_done` with nothing unverified finishes.
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use crate::answers::{self, Answer};
use crate::checkpoint::{self, Outcome};
use crate::cli::report;
use crate::closeout;
use crate::commit::{self, Pathspec};
use crate::config::Config;
use crate::docs::{Docs, Status};
use crate::error::{io, json};
use crate::events::{self, EventKind};
use crate::land::{self, Landing};
use crate::pack::{BlockSection, Pack};
use crate::quota::{self, Hold, Limit};
use crate::run::{GateCmds, Phase, Run, WorkerOutcome, WorkerRef};
use crate::stream::Summary;
use crate::worker::{self, Exit, Job, Worker};
use crate::{Error, Result, clock, lanes, metrics, paths};

/// Deaths on one slice before it is blocked: respawn once, then block.
pub const DEATH_CAP: usize = 2;

/// The longest a tick waits on its children before reading the board again.
pub const TICK: Duration = Duration::from_secs(1);
/// The board cadence of a parked run; the answers dir is watched faster.
pub const SLOW_TICK: Duration = Duration::from_secs(5);

/// A worker this driver spawned and has not reaped.
struct Running {
    worker: Worker,
    lane: u32,
    /// Set by `poll` when the process exited; routed on the next reap.
    exit: Option<Exit>,
}

type Table = HashMap<u32, Running>;

/// What settling the unsettled workers of a resumed run found.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Orphans {
    None,
    /// A worker of a dead driver is still running: wait, then read again.
    Live,
    Settled,
    /// A done slice cannot land on a dirty main tree: the run parks on this
    /// line, nothing spawns, and the landing is tried again each tick.
    Landing(String),
}

/// Drive `run` until it finishes, stops or reaches a checkpoint. A parked run
/// does not return: the driver waits for the owner's answer. On an error the
/// run is marked `failed` with a `run_failed` event before the error returns.
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
            abandon(run, &mut table, out);
            let _ = run.set_phase(Phase::Failed);
            let _ = run.emit(EventKind::RunFailed {
                error: e.to_string(),
            });
            let _ = writeln!(out, "run {} failed: {e}", run.id());
            Err(e)
        }
    }
}

/// A failed driver's workers: the live ones killed and reaped, every one
/// settled `failed`, so nothing builds on unwatched and no row stays open.
/// Best effort: the run is failing already.
fn abandon(run: &mut Run, table: &mut Table, out: &mut dyn Write) {
    for (slice, mut r) in table.drain() {
        let (session_id, row_id, pid) =
            (r.worker.session_id.clone(), r.worker.row_id, r.worker.pid);
        let exit = match r.exit {
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

/// Settle a worker `failed` unless something settled it already: the
/// failing driver's last word on it. Best effort.
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
    // FR-8: the gate pair, read once, so a worker's `Bash` call that runs
    // it settles as a `gates` row with source `worker` (`metrics`).
    if run.gate_cmds().is_none()
        && let Ok(gate) = docs.gate()
    {
        let inner = docs.gate_inner().ok().filter(|s| !s.is_empty());
        run.set_gate_cmds(GateCmds { gate, inner });
    }
    // A resumed parked run stays parked without a second `run_waiting`.
    let mut parked = run.state().phase == Phase::WaitingOwner;
    let mut announced = false;
    // Every commit this process makes is held to the run's branch.
    let repo = run.state().repo.clone();
    let default = crate::cli::run::load_preflight(run.dir())?.and_then(|pf| pf.branch.default);
    if let Some(b) = run.state().branch.clone() {
        commit::hold_branch(&repo, &b, default.as_deref());
    }
    let mut off_branch = false;
    // A landing parked on the owner's dirty main tree (PRD-005 FR-3).
    let mut landing_wait = false;
    // Earlier drivers' live workers this one said it waits on.
    let mut waiting: HashSet<String> = HashSet::new();
    // Answer files that do not parse, said once each.
    let mut unreadable: HashSet<String> = HashSet::new();
    // `events.jsonl` lines that do not parse, said once each (DEC-3).
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
        // HEAD off the run's branch: nothing is reaped (a reap commits),
        // nothing spawns; the run parks until the owner switches back. Once
        // the closeout owns the run, its own guard blocks instead, with a
        // `BLOCK-n` the owner answers (AD-50001).
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

        // A run parked on preflight asks has no branch yet (FR-1): it starts
        // when the last one is answered, and not before.
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
                // An earlier driver's workers are in flight too: a live one
                // finishes, a dead one is settled, before the run ends.
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

        // FR-9: a pause or a spent budget holds every spawn. Workers in
        // flight finish; a paused run with nothing alive sleeps until the
        // reset, a run over budget parks and the driver exits (AD-703).
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
                // Nothing spawns while a landing waits; live workers run on.
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
                // The dead ones are settled; the live ones get a tick, as
                // this driver's own workers do.
                if table.is_empty() {
                    std::thread::sleep(TICK);
                } else {
                    poll(table, TICK)?;
                }
                continue;
            }
            Orphans::Settled => {
                // A settled orphan can pause the run (its limit) or spend
                // the budget (its tokens): the hold is checked again first.
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
        // A `building` slice with nothing alive on it is either answered
        // (an `answers/BLOCK-n.json` names it) or a worker that died below
        // the cap: either way it gets a fresh worker, on the lane it holds,
        // once no checkpoint is due.
        // One whose deaths since the last answer reached the cap is a kill
        // between the second death and the flip: it blocks, it never gets a
        // third worker.
        let mut capped = false;
        for b in board.building.clone() {
            if table.contains_key(&b.number) || has_unsettled(run, b.number) {
                continue;
            }
            let dead = deaths_since_answer(run, b.number)?;
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
            // A due checkpoint drains first (FR-4): a respawn is a spawn,
            // so the slice waits for the checkpoint like a ready one.
            if board.checkpoint_due {
                continue;
            }
            // A slice holding its lane keeps it as its worker left it.
            let (lane, fresh) = match b.lane {
                Some(l) => (l, false),
                None => {
                    if free.is_empty() {
                        continue;
                    }
                    let l = free.remove(0);
                    crate::docs::state::set(
                        &run.state().repo.join(&board.dir),
                        &[(&format!("slices.{}.lane", b.number), &l.to_string())],
                    )?;
                    (l, true)
                }
            };
            let answer = answers::block_answer_for_slice(run.dir(), b.number)?;
            spawn_on(
                run,
                docs,
                cfg,
                &board,
                b.number,
                lane,
                fresh,
                answer.as_ref(),
                table,
                out,
            )?;
        }
        if capped {
            continue;
        }
        // Then the ready slices, first `spawnable` of them (FR-3). A due
        // checkpoint drains first: the tool says `spawnable: 0` and so do we.
        if !board.checkpoint_due {
            for r in board.ready.clone() {
                if free.is_empty() {
                    break;
                }
                if table.contains_key(&r.number) {
                    continue;
                }
                // A dependency counts as done once it has landed (FR-3): its
                // worker wrote `done` before it exited, and a lane reset
                // before the landing would miss its code.
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
                crate::docs::state::set(
                    &run.state().repo.join(&board.dir),
                    &[
                        (&format!("slices.{}.status", r.number), "building"),
                        (&format!("slices.{}.lane", r.number), &lane.to_string()),
                    ],
                )?;
                spawn_on(
                    run, docs, cfg, &board, r.number, lane, true, None, table, out,
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
        // The lanes are drained with a checkpoint due (FR-4): the driver
        // runs it — gate, fix workers, log merge, checkpoint worker, commit.
        if board.checkpoint_due {
            match checkpoint::run(run, docs, cfg, &board, out)? {
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
        if !board.blocked.is_empty() {
            enter_park(run, &mut parked, &mut announced, &board.blocked, &[], out)?;
            slow_tick(run)?;
            continue;
        }
        // The last checkpoint is done (FR-6): polish, ship, PR, CI — then
        // the run is finished, or parked on a closeout block.
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

/// Whether the closeout owns a HEAD off the run's branch: nothing is in
/// flight, every worker is settled, and every slice is done and checked.
/// Nothing between here and `closeout::run` then commits, and the closeout
/// checks the branch before it polishes, ships, pushes or fixes CI.
fn closeout_owns(run: &Run, docs: &Docs, feature: &str, table: &Table) -> Result<bool> {
    if !table.is_empty() || run.state().workers.iter().any(|w| w.outcome.is_none()) {
        return Ok(false);
    }
    let board = docs.status(feature)?;
    Ok(board.all_done && board.unverified.is_empty())
}

/// Route every worker in the table that has exited.
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
        // The worker stays in the table until it is routed, so a failing
        // driver still finds it (`abandon`).
        let r = table.get_mut(&slice).expect("key from the table");
        if r.exit.is_none() {
            r.exit = r.worker.try_wait()?;
        }
        if r.exit.is_none() {
            continue;
        }
        let board = docs.status(feature)?;
        let mut r = table.remove(&slice).expect("key from the table");
        let exit = r.exit.take().expect("checked above");
        if let Err(e) = route_exit(run, &board, &r, exit, out) {
            let summary = worker::summary_from_disk(run.dir(), &r.worker.session_id);
            settle_failed(run, r.worker.row_id, &r.worker.session_id, &summary);
            return Err(e);
        }
    }
    Ok(())
}

/// Wait on the children: back as soon as one exits, or after `cap`.
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

/// A parked run's tick: the board every `SLOW_TICK`, the answers dir and
/// `stop_requested` every 250 ms so an answer is seen within the second.
fn slow_tick(run: &mut Run) -> Result<()> {
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

/// Park: once per entry, `run_waiting`, phase `waiting-owner`, a line.
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

fn park_with(
    run: &mut Run,
    parked: &mut bool,
    announced: &mut bool,
    blocked: &[u32],
    asks: &[String],
    what: &str,
    out: &mut dyn Write,
) -> Result<()> {
    // The log line goes before the phase: a reader that sees `waiting-owner`
    // in state.json must find the park in driver.log too.
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

/// Every `ASK-n` answered: the `pass` path of `factory run` — the branch from
/// the preflight saved at ask time, `judge_noted` per line, `run_started` and
/// the board.
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

/// The lanes this driver may spawn on now: the board's free lanes minus
/// those a `blocked` slice parks (AD-401) and those this table holds.
/// The `blocked` filter is a second check: docs.mjs leaves those lanes out
/// of `freeLanes` itself now, but a vendored copy older than that fix does not.
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

/// Pack and spawn `slice` in `lane`'s worktree ([`open_lane`]; `fresh`, it
/// is reset first). The lane's setup runs inside it once per run, recorded
/// in `state.json` once it succeeded (a failed setup runs again). The
/// worker is in the table the moment it runs, so an error after the spawn
/// still finds it.
#[allow(clippy::too_many_arguments)]
fn spawn_on(
    run: &mut Run,
    docs: &Docs,
    cfg: &Config,
    board: &Status,
    slice: u32,
    lane: u32,
    fresh: bool,
    answer: Option<&Answer>,
    table: &mut Table,
    out: &mut dyn Write,
) -> Result<()> {
    let repo = run.state().repo.clone();
    let lane_def = lanes::load(&repo, lane)?;
    let lane_dir = open_lane(run, lane, fresh)?;
    // An earlier worker's unlanded commit (a conflict) is the lane's HEAD,
    // or gone with a reset: this worker's landing commits the lane afresh.
    let stale = slice_ref(board, slice);
    if resolve(&repo, &stale)?.is_some() {
        git_ok(&repo, &["update-ref", "-d", &stale])?;
    }
    if !run.state().lanes_set_up.contains(&lane) {
        if let Some(cmd) = &lane_def.setup {
            lane_def.setup(&lane_dir)?;
            say(out, &format!("lane {lane} setup ran: {cmd}"))?;
        }
        run.mark_lane_set_up(lane)?;
    }
    let block = match answer {
        Some(a) => Some(block_section(&repo, board, a)?),
        None => None,
    };
    let spawn_n = run.state().workers.len() as u32 + 1;
    let pack = Pack::for_slice_with(
        docs,
        board,
        slice,
        &packs_dir(run.dir()),
        spawn_n,
        block.as_ref(),
    )?;
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
        Job::Build { slice },
        &pack,
        &cfg.workers.build,
        &lane_def.env(),
        &lane_dir,
    )?;
    let line = format!(
        "slice {slice:02} → worker {} (pid {}), lane {lane}{}, pack {}",
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

/// Lane `lane`'s worktree, made when missing (a resumed run skips the
/// preflight that makes them). A slice taking it free (`fresh`) resets it to
/// the main tree's HEAD and saves that sha as the lane's base; so does a
/// lane just made, or one with no base saved, since its landing needs one.
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

/// The `# Block and answer` section for an answered slice: the entry with its
/// resolution as the log holds it, or the answer alone when the entry is gone.
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

/// A reaped worker: route on the board.
fn route_exit(
    run: &mut Run,
    board: &Status,
    r: &Running,
    exit: Exit,
    out: &mut dyn Write,
) -> Result<()> {
    let slice = r.worker.slice.expect("the table holds build workers only");
    let session_id = r.worker.session_id.clone();
    let row_id = r.worker.row_id;
    match slice_status(board, slice) {
        "done" => {
            // The landing before the outcome: a driver killed in between
            // leaves the worker unsettled, and the resume lands the slice.
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
            // A landing that waits leaves the worker unsettled: the next
            // `settle_orphans` tries again and parks the run.
            if let Some(outcome) = land_slice(run, board, slice, row_id, out)?.outcome() {
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

/// `runs/<id>/untracked-before/<key>.json`: the tree's changed paths at the
/// spawn of a worker, for the driver that settles it. One worker runs per
/// slice (`slice-NN`) and per checkpoint role (`checkpoint-n-fix`,
/// `checkpoint-n-checkpoint`), and a respawn waits for the last one to be
/// settled, so the key names it.
fn baseline_path(run_dir: &Path, key: &str) -> PathBuf {
    run_dir.join("untracked-before").join(format!("{key}.json"))
}

/// The baseline key of a checkpoint's worker of `role`.
pub fn checkpoint_key(n: u32, role: &str) -> String {
    format!("checkpoint-{n}-{role}")
}

/// The tree's changed paths now, saved under `key` and returned.
pub fn snapshot(run: &Run, key: &str) -> Result<HashSet<String>> {
    let repo = run.state().repo.clone();
    let files: HashSet<String> = commit::changed(&repo)?
        .into_iter()
        .map(|c| c.path)
        .collect();
    save_baseline(run.dir(), key, &files)?;
    Ok(files)
}

fn save_baseline(run_dir: &Path, key: &str, files: &HashSet<String>) -> Result<()> {
    let path = baseline_path(run_dir, key);
    let dir = path.parent().expect("a file in a dir");
    fs::create_dir_all(dir).map_err(io(format!("create {}", dir.display())))?;
    let mut sorted: Vec<&String> = files.iter().collect();
    sorted.sort();
    let text = serde_json::to_string(&sorted).map_err(json("serialize the untracked baseline"))?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, text).map_err(io(format!("write {}", tmp.display())))?;
    fs::rename(&tmp, &path).map_err(io(format!("rename {}", tmp.display())))
}

/// The saved baseline; empty when none was saved (a run from before it was).
pub fn load_baseline(run_dir: &Path, key: &str) -> Result<HashSet<String>> {
    let path = baseline_path(run_dir, key);
    match fs::read_to_string(&path) {
        Ok(t) => Ok(serde_json::from_str::<Vec<String>>(&t)
            .map_err(json(format!("parse {}", path.display())))?
            .into_iter()
            .collect()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(HashSet::new()),
        Err(e) => Err(io(format!("read {}", path.display()))(e)),
    }
}

/// Whether `slice_done` is already logged for this worker of `slice`: a
/// driver killed between the event and the commit left it, and the resume
/// that commits must not log it twice.
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

/// `(BLOCK-n, question)` of the slice's open block, from the feature's log.
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

/// What [`land_slice`] made of a done slice.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Landed {
    /// Landed, or nothing to land: the worker settles `done`.
    Done,
    /// The pick conflicts: the slice is blocked and the worker settles so.
    Blocked,
    /// The main tree is dirty where the pick writes: the worker stays
    /// unsettled, its ref holds the work, and the run parks on this line.
    Waiting(String),
}

impl Landed {
    fn outcome(&self) -> Option<WorkerOutcome> {
        match self {
            Landed::Done => Some(WorkerOutcome::Done),
            Landed::Blocked => Some(WorkerOutcome::Blocked),
            Landed::Waiting(_) => None,
        }
    }
}

/// `refs/factory/<NNN>/slice-NN`: a slice's lane commit until it lands.
fn slice_ref(board: &Status, slice: u32) -> String {
    format!("refs/factory/{:03}/slice-{slice:02}", board.number)
}

/// PRD-005 FR-3: a done slice's lane committed whole under its ref, then
/// cherry-picked onto the feature tip with the board files. Safe to call
/// again for the same worker, which is how a resumed driver finishes it: a
/// worker row with a commit has landed; a ref that resolves is committed and
/// waits for its landing; anything else commits the lane first, so the ref
/// holds the work before any lane is reset.
fn land_slice(
    run: &mut Run,
    board: &Status,
    slice: u32,
    row_id: i64,
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
    // A driver killed after the landing commit and before its record: the
    // commit is on the tip, its ref maybe not yet deleted. Record it; a
    // second pick would find nothing to commit.
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
    let sha = match resolve(&repo, &name)? {
        Some(sha) => sha,
        None => {
            let Some(lane) = lane else {
                return Err(Error::Msg(format!("slice {slice:02} is done with no lane")));
            };
            let Some(base) = base else {
                return Err(Error::Msg(format!(
                    "lane {lane} has no base in state.json to commit slice {slice:02} on"
                )));
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
            // Slice 08's merge worker takes this branch over; until then the
            // slice blocks with its work on the ref.
            let feature_dir = repo.join(&board.dir);
            let list = paths.join(", ");
            let body = format!(
                "**Happened:** slice {slice:02}'s lane commit {} conflicts with the feature tip in {list}. The main tree is untouched; the commit is kept on `{name}`.\n\n\
                 **Options:**\n\
                 1. Land it by hand: cherry-pick `{name}` onto the feature branch, resolve {list}, commit, and leave the slice `done`.\n\
                 2. Answer to respawn its worker in its lane, as the last one left it, to rework {list}.\n\n\
                 **Recommended:** 1 for a small conflict; 2 otherwise.\n",
                short(&sha)
            );
            let id = answers::add_block(
                &feature_dir,
                Some(slice),
                None,
                &format!("slice {slice:02} conflicts with the feature tip in {list}"),
                &body,
            )?;
            crate::docs::state::set(
                &feature_dir,
                &[(&format!("slices.{slice}.status"), "blocked")],
            )?;
            let block = open_block(run, board, slice)?;
            emit_blocked(run, slice, block.as_ref())?;
            say(
                out,
                &format!(
                    "slice {slice:02} cannot land: it conflicts with the feature tip in {list} — slice blocked ({id}), its commit kept on {name}"
                ),
            )?;
            Ok(Landed::Blocked)
        }
    }
}

/// A slice's landed commit, on the worker's row, in a line and an event.
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

/// A driver killed in [`land::land`] between its pick and its commit leaves
/// the picked paths staged in the main tree, which the next landing would
/// read as the owner's dirty work. When something is staged there and every
/// picked path in the index and the tree is exactly `sha`'s, the pick is the
/// driver's own: those paths go back to HEAD.
fn undo_half_pick(repo: &Path, sha: &str) -> Result<()> {
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

/// The commit `name` points at, if it resolves.
fn resolve(repo: &Path, name: &str) -> Result<Option<String>> {
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

fn nul_split(text: &str) -> Vec<String> {
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

fn git_ok(repo: &Path, args: &[&str]) -> Result<String> {
    let o = git_out(repo, args)?;
    if !o.status.success() {
        return Err(Error::Tool {
            cmd: format!("git {}", args.join(" ")),
            detail: String::from_utf8_lossy(&o.stderr).trim().to_string(),
        });
    }
    Ok(String::from_utf8_lossy(&o.stdout).into_owned())
}

/// What a worker changed in the tree that no tool call of its stream named
/// (FR-5, AD-20007).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Unnamed {
    /// Tracked files it changed, and new files under a directory it
    /// `git add`ed, with no other worker live: they go in its commit, since
    /// no lane can be mid-write in them.
    pub sweep: Vec<String>,
    /// The rest — new untracked files, tracked changes while another worker
    /// was live, the protected files: listed, left in the tree for the owner.
    pub listed: Vec<String>,
    /// The blob id of each `listed` file, for its leftovers event.
    pub blobs: BTreeMap<String, String>,
}

/// The tree's changes now that were not changed at the worker's spawn
/// (`baseline`), that `skip` does not hold and that no earlier leftovers
/// event listed with the content they have now. A listed file is listed
/// once; changed again since, it is judged afresh, swept when alone. Run B
/// of 2026-10-11 skipped every listed path for good, and polish's edits to
/// four files that slice 02 had left and the owner had committed since went
/// in no commit and on no list. An event from a driver that kept no blob
/// ids skips its file while it is still untracked.
pub fn unnamed(
    run: &Run,
    board: &Status,
    baseline: &HashSet<String>,
    skip: &HashSet<String>,
    dirs: &[String],
    alone: bool,
) -> Result<Unnamed> {
    let repo = run.state().repo.clone();
    let protected = commit::protected_files(board);
    // The last listing of each path wins; `None`, a listing with no blob.
    let mut listed_before: HashMap<String, Option<String>> = HashMap::new();
    for e in run.events()? {
        if let EventKind::UntrackedLeftovers { files, blobs, .. }
        | EventKind::CheckpointLeftovers { files, blobs, .. }
        | EventKind::CloseoutLeftovers { files, blobs, .. } = e.kind
        {
            for f in files {
                let blob = blobs.get(&f).cloned();
                listed_before.insert(f, blob);
            }
        }
    }
    let changed: Vec<commit::Changed> = commit::changed(&repo)?
        .into_iter()
        .filter(|c| !baseline.contains(&c.path) && !skip.contains(&c.path))
        .collect();
    let relisted: Vec<String> = changed
        .iter()
        .filter(|c| listed_before.contains_key(&c.path))
        .map(|c| c.path.clone())
        .collect();
    let now = commit::blobs(&repo, &relisted)?;
    let mut found = Unnamed::default();
    for c in changed {
        let same = match listed_before.get(&c.path) {
            None => false,
            Some(Some(blob)) => now.get(&c.path) == Some(blob),
            Some(None) => !c.tracked,
        };
        if same {
            continue;
        }
        let claimed = dirs.iter().any(|d| {
            c.path
                .strip_prefix(d.as_str())
                .is_some_and(|r| r.starts_with('/'))
        });
        if (c.tracked || claimed) && alone && !protected.contains(&c.path) {
            found.sweep.push(c.path);
        } else {
            found.listed.push(c.path);
        }
    }
    found.blobs = commit::blobs(&repo, &found.listed)?;
    Ok(found)
}

pub fn short(sha: &str) -> &str {
    &sha[..sha.len().min(7)]
}

/// Workers `state.json` says are unsettled and this driver did not spawn: a
/// dead driver's workers (FR-1 resume).
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
    // Every dead one is settled now, whatever runs beside it; a live one is
    // waited on, said once.
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
                closeout::settle_orphan(run, board, &w, summary, out)?;
            } else {
                settle_checkpoint_orphan(run, board, &w, summary, out)?;
            }
            continue;
        };
        match slice_status(board, slice) {
            "done" => {
                if !done_logged(run, slice, &w.session_id)? {
                    run.emit(EventKind::SliceDone { slice })?;
                }
                // A landing already under way (its ref made) is retried
                // every tick a dirty main tree holds it: said once, at first.
                if resolve(&run.state().repo, &slice_ref(board, slice))?.is_none() {
                    say(
                        out,
                        &format!(
                            "slice {:02} done by worker {} of an earlier driver",
                            slice, w.session_id
                        ),
                    )?;
                }
                match land_slice(run, board, slice, w.row_id, out)? {
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

/// `slice NN` or `checkpoint n (role)`, for a line about a worker.
fn label(w: &WorkerRef) -> String {
    match (w.slice, w.checkpoint) {
        (Some(s), _) => format!("slice {s:02}"),
        (None, Some(n)) => format!("checkpoint {n} ({})", w.role),
        (None, None) => format!("closeout ({})", w.role),
    }
}

/// A fix or checkpoint worker of a dead driver (FR-4). A checkpoint worker
/// whose `CP-n` is on the board is done, and this driver finishes what the
/// dead one did not: `docs: checkpoint n`, `checkpoint_done`. A fix
/// worker has what it changed committed however it ended, judged against
/// its spawn baseline (AD-20007). A limit pauses the run; anything else is
/// dead, and the checkpoint runs again from the board.
fn settle_checkpoint_orphan(
    run: &mut Run,
    board: &Status,
    w: &WorkerRef,
    summary: Summary,
    out: &mut dyn Write,
) -> Result<()> {
    let n = w.checkpoint.unwrap_or(0);
    let repo = run.state().repo.clone();
    if w.role == "checkpoint" && board.checkpoints.iter().any(|c| c.n == n) {
        let before = load_baseline(run.dir(), &checkpoint_key(n, "checkpoint"))?;
        let spec = Pathspec::from_stream(&summary, board, &repo);
        let sha = checkpoint::commit_checkpoint(run, board, n, w.row_id, &spec, &before, out)?;
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
        return Ok(());
    }
    if w.role == "fix" {
        let before = load_baseline(run.dir(), &checkpoint_key(n, "fix"))?;
        let attempt = run
            .db()
            .gates(run.id().as_str())?
            .iter()
            .filter(|g| g.checkpoint == Some(n))
            .filter_map(|g| g.attempt)
            .max()
            .unwrap_or(0);
        checkpoint::commit_fix(
            run,
            board,
            n,
            attempt,
            w.row_id,
            &w.session_id,
            &summary,
            &before,
            false,
            out,
        )?;
    }
    // FR-9: a limit is not a death; the checkpoint runs again after the pause.
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
        return Ok(());
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
        return Ok(());
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
    Ok(())
}

/// Close the row and the state entry.
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

/// FR-9: a build worker gone on a usage limit. Not a death: the slice goes
/// back to `pending` with its lane freed, and the run pauses until the
/// reset. The next spawn of the slice is a fresh worker on a fresh lane.
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

/// A worker gone with the slice neither `done` nor `blocked`. Returns `true`
/// when this death blocked the slice.
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
    let dead = deaths_since_answer(run, slice)?;
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

/// The title of the driver's block on a slice whose workers kept dying.
fn dead_title(slice: u32) -> String {
    format!("Two workers died on slice {:02}", slice)
}

/// The slice's workers that died since the owner last answered a block on
/// it: what the once-only respawn (FR-1) counts. A `block_answered` naming
/// the slice starts the count again; a worker whose `slice_spawned` comes
/// before it is behind the answer. A worker with no `slice_spawned` (a kill
/// inside the spawn) counts: the cap errs toward the owner.
pub fn deaths_since_answer(run: &Run, slice: u32) -> Result<Vec<WorkerRef>> {
    let events = run.events()?;
    let from = events
        .iter()
        .rposition(
            |e| matches!(&e.kind, EventKind::BlockAnswered { slice: Some(s), .. } if *s == slice),
        )
        .map_or(0, |i| i + 1);
    let before: HashSet<&str> = events[..from]
        .iter()
        .filter_map(|e| match &e.kind {
            EventKind::SliceSpawned {
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
        .filter(|w| w.slice == Some(slice) && w.died() && !before.contains(w.session_id.as_str()))
        .cloned()
        .collect())
}

/// The open death block on `slice`: one a kill left written after the
/// entry and before the slice flipped.
fn open_dead_block(feature_dir: &Path, slice: u32) -> Result<Option<answers::BlockEntry>> {
    let title = dead_title(slice);
    Ok(answers::entries(feature_dir)?
        .into_iter()
        .rfind(|e| !e.resolved && e.slice == Some(slice) && e.title == title))
}

/// Block `slice` on its dead workers: a `block` entry (unless a kill left
/// one open), the slice `blocked`, `slice_blocked`. `last` is what the
/// latest death adds to the entry, empty when the driver came back to a cap
/// reached before it died. Returns the block's id.
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
             the driver's once-only respawn (autonomy.md § Caps, hand-back once) is spent. \
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

fn find_ref(run: &Run, session_id: &str) -> WorkerRef {
    run.state()
        .workers
        .iter()
        .rev()
        .find(|w| w.session_id == session_id)
        .cloned()
        .expect("a spawned worker is in state.workers")
}

/// Phase `finished` and the `run_finished` line (FR-7): the PR URL and the
/// numbers the cockpit reads first, from the store.
fn finish(run: &mut Run, out: &mut dyn Write, why: &str) -> Result<Phase> {
    // The numbers first, so the event and the phase land back to back; the
    // event before the phase, so a reader that sees `finished` finds
    // `run_finished` (AD-1005), and a driver killed in between leaves a run
    // that resumes and finishes without a second one.
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

/// `factory stop` honoured (FR-7): the in-flight workers were reaped, their
/// done slices committed; now phase `finished` with `stopped: true` and
/// `run_stopped` as the last line — not `run_finished`, which says the run
/// reached its end (AD-1002). The run is over: `factory run NNN` starts a
/// new one from the board.
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

/// The state and the event of a stopped run; `factory stop` on a run with no
/// live driver uses it too. Returns the slices done.
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

/// Some event of the run is of this kind already.
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

/// The slice's status as the board reports it; `""` when unknown.
pub fn slice_status(board: &Status, slice: u32) -> &str {
    board
        .slices
        .iter()
        .find(|s| s.number == slice)
        .and_then(|s| s.status.as_deref())
        .unwrap_or("")
}

/// Replace `path` with `text` atomically (DEC-3): a temp file beside it,
/// so the rename stays on one filesystem, then the rename. A reader sees
/// the old file or the new one, whole; a failed rename leaves no temp.
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

/// `runs/<id>/packs`.
pub fn packs_dir(run_dir: &Path) -> PathBuf {
    run_dir.join("packs")
}
