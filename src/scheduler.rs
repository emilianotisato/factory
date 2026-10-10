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
//! in that slice's `log-slice-NN.md` and a fresh worker with a new session;
//! the second since the owner last answered a block on it blocks the slice
//! with the lowest free `BLOCK-NNkk` from `NN02`. The count lives in
//! `state.json` `workers[]` and `events.jsonl`, so a resume after a kill
//! counts the killed worker, and the spawn loop blocks a capped slice
//! instead of giving it a third worker.
//!
//! FR-5: a worker whose slice reads `done` has its slice committed by
//! pathspec before the next board read — the paths its stream named, the
//! slice file and its fragment, nothing with `-A` since another lane may be
//! mid-write. The worker's outcome is saved after the commit, so a driver
//! killed in between leaves it unsettled and the resume commits. A tracked
//! file the worker changed without naming it joins the commit when no
//! sibling was live, and is listed otherwise; a new file no tool call named
//! is always listed (AD-20007). Every commit lands on the run's branch: HEAD
//! anywhere else parks the run until the owner switches back, with nothing
//! reaped or spawned meanwhile (AD-20005).
//!
//! FR-4: when the board says `checkpoint_due` nothing new spawns; once the
//! table is empty the loop hands the run to [`checkpoint::run`] — the gate,
//! the fix workers, the log merge, the checkpoint worker and the checkpoint
//! commit — and reads the board again when it is done. The end of the run
//! is the last checkpoint: `all_done` with nothing unverified finishes.
use std::collections::{HashMap, HashSet};
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
use crate::events::EventKind;
use crate::pack::{BlockSection, Pack};
use crate::quota::{self, Hold, Limit};
use crate::run::{GateCmds, Phase, Run, WorkerOutcome, WorkerRef};
use crate::stream::Summary;
use crate::worker::{self, Exit, Job, Worker};
use crate::{Result, clock, lanes, metrics};

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
    /// The tree's changed paths at spawn: what the worker changes beyond
    /// these and its stream's paths is unnamed (FR-5, [`unnamed`]).
    changed_before: HashSet<String>,
}

type Table = HashMap<u32, Running>;

/// What settling the unsettled workers of a resumed run found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Orphans {
    None,
    /// A worker of a dead driver is still running: wait, then read again.
    Live,
    Settled,
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
    for (slice, r) in table.drain() {
        let (session_id, row_id, pid) =
            (r.worker.session_id.clone(), r.worker.row_id, r.worker.pid);
        let exit = match r.exit {
            Some(exit) => exit,
            None => {
                // SAFETY: kill(2) on this process's own child, not yet reaped,
                // so the pid is still that child's.
                unsafe { libc::kill(pid as i32, libc::SIGKILL) };
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
    // Earlier drivers' live workers this one said it waits on.
    let mut waiting: HashSet<String> = HashSet::new();
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
        // HEAD off the run's branch: nothing is reaped (a reap commits),
        // nothing spawns; the run parks until the owner switches back.
        if let Some(b) = run.state().branch.clone() {
            if let Some(why) = commit::branch_refusal(&repo, &b, default.as_deref())? {
                if run.state().stop_requested && table.values().all(|r| r.exit.is_some()) {
                    return stopped(run, out);
                }
                if !off_branch {
                    parked = false;
                    announced = false;
                    off_branch = true;
                }
                park_with(run, &mut parked, &mut announced, &[], &[], &why, out)?;
                if table.is_empty() {
                    slow_tick(run)?;
                } else {
                    poll(table, TICK)?;
                }
                continue;
            }
            if off_branch {
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
                if settle_orphans(run, docs, &board, &none, &mut waiting, out)? == Orphans::Live {
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
        match settle_orphans(run, docs, &board, &live, &mut waiting, out)? {
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
                let block = dead_block_id(run, &board, b.number)?;
                block_dead_slice(run, &board, b.number, &block, &dead, "")?;
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
            let lane = match b.lane {
                Some(l) => l,
                None => {
                    if free.is_empty() {
                        continue;
                    }
                    let l = free.remove(0);
                    set_frontmatter(
                        &slice_file(run, &board, b.number)?,
                        &[("lane", &l.to_string())],
                    )?;
                    l
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
                let lane = free.remove(0);
                set_frontmatter(
                    &slice_file(run, &board, r.number)?,
                    &[("status", "building"), ("lane", &lane.to_string())],
                )?;
                spawn_on(run, docs, cfg, &board, r.number, lane, None, table, out)?;
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
        let live: HashSet<String> = table
            .values()
            .map(|o| o.worker.session_id.clone())
            .collect();
        if let Err(e) = route_exit(run, &board, &r, exit, &live, out) {
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
    if !*parked {
        run.emit(EventKind::RunWaiting {
            blocked: blocked.to_vec(),
            asks: asks.to_vec(),
        })?;
        run.set_phase(Phase::WaitingOwner)?;
        *parked = true;
    }
    if !*announced {
        say(
            out,
            &format!("run {} parked (waiting-owner): {what}", run.id()),
        )?;
        *announced = true;
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

/// Pack and spawn `slice` on `lane`; the lane's setup runs once per run,
/// recorded in `state.json` once it succeeded (a failed setup runs again).
/// The worker is in the table the moment it runs, so an error after the
/// spawn still finds it.
#[allow(clippy::too_many_arguments)]
fn spawn_on(
    run: &mut Run,
    docs: &Docs,
    cfg: &Config,
    board: &Status,
    slice: u32,
    lane: u32,
    answer: Option<&Answer>,
    table: &mut Table,
    out: &mut dyn Write,
) -> Result<()> {
    let repo = run.state().repo.clone();
    let lane_def = lanes::load(&repo, lane)?;
    if !run.state().lanes_set_up.contains(&lane) {
        if let Some(cmd) = &lane_def.setup {
            lane_def.setup(&repo)?;
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
    // The tree's changed paths before the worker touches it (FR-5), kept
    // on disk too for a resumed driver that settles this worker.
    let changed_before = snapshot(run, &slice_key(slice))?;
    let w = Worker::spawn(
        run,
        Job::Build { slice },
        &pack,
        &cfg.workers.build,
        &lane_def.env(),
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
            changed_before,
        },
    );
    say(out, &line)
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
            text: answers::entry_text(&entry)?,
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
    live: &HashSet<String>,
    out: &mut dyn Write,
) -> Result<()> {
    let slice = r.worker.slice.expect("the table holds build workers only");
    let session_id = r.worker.session_id.clone();
    let row_id = r.worker.row_id;
    match slice_status(board, slice) {
        "done" => {
            // The commit before the outcome: a driver killed in between
            // leaves the worker unsettled, and the resume commits the slice.
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
            commit_slice(
                run,
                board,
                slice,
                row_id,
                &exit.summary,
                &r.changed_before,
                live,
                out,
            )?;
            settle(run, row_id, &session_id, &exit.summary, WorkerOutcome::Done)?;
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
                        .unwrap_or_else(|| "no BLOCK entry in its fragment".into()),
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

/// The baseline key of a slice's worker.
fn slice_key(slice: u32) -> String {
    format!("slice-{slice:02}")
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

/// `(BLOCK-n, question)` of the slice's open block, from its fragment.
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

/// FR-5: the slice's commit, right after the board read `done`. The paths
/// its stream named go in (less the three protected files), then the slice
/// file and the fragment. Files that appeared in the tree unnamed are listed
/// in the fragment under `## Untracked leftovers` first, so the list is in
/// the commit and the files are not.
#[allow(clippy::too_many_arguments)]
fn commit_slice(
    run: &mut Run,
    board: &Status,
    slice: u32,
    row_id: i64,
    summary: &Summary,
    baseline: &HashSet<String>,
    live: &HashSet<String>,
    out: &mut dyn Write,
) -> Result<()> {
    let repo = run.state().repo.clone();
    let spec = Pathspec::from(summary, board, slice, &repo);
    for path in &spec.protected {
        run.emit(EventKind::WorkerWroteProtectedFile {
            slice,
            path: path.clone(),
        })?;
        say(
            out,
            &format!("slice {slice:02} wrote {path}, which no worker may — left out of the commit"),
        )?;
    }
    for path in &spec.outside {
        say(
            out,
            &format!("slice {slice:02} wrote {path}, outside the repo — left out of the commit"),
        )?;
    }
    let Unnamed { sweep, listed } = leftovers(run, board, slice, &spec, baseline, live)?;
    if !listed.is_empty() {
        let fragment = repo.join(commit::fragment_path(board, slice));
        let body: String = listed.iter().map(|p| format!("- {p}\n")).collect();
        append_under(&fragment, "## Untracked leftovers", &body)?;
        run.emit(EventKind::UntrackedLeftovers {
            slice,
            files: listed.clone(),
        })?;
        say(
            out,
            &format!(
                "slice {slice:02} left {} file(s) no tool call named — listed in its fragment, not committed: {}",
                listed.len(),
                listed.join(", ")
            ),
        )?;
    }
    if !sweep.is_empty() {
        say(
            out,
            &format!(
                "slice {slice:02} changed {} tracked file(s) no tool call named, with no other worker live — in its commit: {}",
                sweep.len(),
                sweep.join(", ")
            ),
        )?;
    }
    let mut paths = spec.paths.clone();
    paths.extend(sweep);
    let title = board
        .slices
        .iter()
        .find(|s| s.number == slice)
        .and_then(|s| s.title.clone())
        .unwrap_or_else(|| format!("slice {slice:02}"));
    match commit::commit(&repo, &paths, &format!("feat: {title}"))? {
        Some(c) => {
            run.db().set_worker_commit(row_id, &c.sha)?;
            say(
                out,
                &format!(
                    "slice {slice:02} committed {} — {} file(s): {}",
                    short(&c.sha),
                    c.files.len(),
                    c.files.join(", ")
                ),
            )?;
            run.emit(EventKind::SliceCommitted {
                slice,
                sha: c.sha,
                files: c.files,
            })?;
        }
        None => say(out, &format!("slice {slice:02}: nothing to commit"))?,
    }
    Ok(())
}

/// What a worker changed in the tree that no tool call of its stream named
/// (FR-5, AD-20007).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Unnamed {
    /// Tracked files it changed with no other worker live: they go in its
    /// commit, since no lane can be mid-write in them.
    pub sweep: Vec<String>,
    /// The rest — new untracked files, tracked changes while another worker
    /// was live, the protected files: listed, left in the tree for the owner.
    pub listed: Vec<String>,
}

/// The tree's changes now that were not changed at the worker's spawn
/// (`baseline`), that `skip` does not hold and that no earlier
/// `untracked_leftovers` already listed.
pub fn unnamed(
    run: &Run,
    board: &Status,
    baseline: &HashSet<String>,
    skip: &HashSet<String>,
    alone: bool,
) -> Result<Unnamed> {
    let repo = run.state().repo.clone();
    let protected = commit::protected_files(board);
    let mut listed_before: HashSet<String> = HashSet::new();
    for e in run.events()? {
        if let EventKind::UntrackedLeftovers { files, .. }
        | EventKind::CheckpointLeftovers { files, .. } = e.kind
        {
            listed_before.extend(files);
        }
    }
    let mut found = Unnamed::default();
    for c in commit::changed(&repo)? {
        if baseline.contains(&c.path) || skip.contains(&c.path) || listed_before.contains(&c.path) {
            continue;
        }
        if c.tracked && alone && !protected.contains(&c.path) {
            found.sweep.push(c.path);
        } else {
            found.listed.push(c.path);
        }
    }
    Ok(found)
}

/// The slice's [`unnamed`] changes: not another slice's file or fragment,
/// and nothing a live worker's stream names so far — the other lane's
/// half-written work is its own, not this slice's leftover.
fn leftovers(
    run: &Run,
    board: &Status,
    slice: u32,
    spec: &Pathspec,
    baseline: &HashSet<String>,
    live: &HashSet<String>,
) -> Result<Unnamed> {
    let repo = run.state().repo.clone();
    let canon = repo.canonicalize().unwrap_or_else(|_| repo.clone());
    let mut skip: HashSet<String> = HashSet::new();
    skip.extend(spec.paths.iter().cloned());
    skip.extend(spec.protected.iter().cloned());
    for s in &board.slices {
        if s.number != slice {
            skip.insert(format!("docs/features/{}", s.file));
            skip.insert(commit::fragment_path(board, s.number));
        }
    }
    for sid in live {
        let other = worker::summary_from_disk(run.dir(), sid);
        skip.extend(
            commit::written_paths(&other).filter_map(|raw| commit::relativize(&repo, &canon, raw)),
        );
    }
    unnamed(run, board, baseline, &skip, live.is_empty())
}

pub fn short(sha: &str) -> &str {
    &sha[..sha.len().min(7)]
}

/// Workers `state.json` says are unsettled and this driver did not spawn: a
/// dead driver's workers (FR-1 resume).
fn settle_orphans(
    run: &mut Run,
    docs: &Docs,
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
    for w in dead {
        let summary = worker::summary_from_disk(run.dir(), &w.session_id);
        let Some(slice) = w.slice else {
            if w.role == "polish" || (w.role == "fix" && w.checkpoint.is_none()) {
                closeout::settle_orphan(run, board, &w, summary, out)?;
            } else {
                settle_checkpoint_orphan(run, docs, board, &w, summary, out)?;
            }
            continue;
        };
        match slice_status(board, slice) {
            "done" => {
                if !done_logged(run, slice, &w.session_id)? {
                    run.emit(EventKind::SliceDone { slice })?;
                }
                say(
                    out,
                    &format!(
                        "slice {:02} done by worker {} of an earlier driver",
                        slice, w.session_id
                    ),
                )?;
                // The snapshot the dead driver saved at this worker's spawn;
                // the other orphans were live beside it.
                let baseline = load_baseline(run.dir(), &slice_key(slice))?;
                let mut beside = live.clone();
                beside.extend(
                    run.state()
                        .workers
                        .iter()
                        .filter(|o| o.outcome.is_none() && o.session_id != w.session_id)
                        .map(|o| o.session_id.clone()),
                );
                commit_slice(
                    run, board, slice, w.row_id, &summary, &baseline, &beside, out,
                )?;
                settle(run, w.row_id, &w.session_id, &summary, WorkerOutcome::Done)?;
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
    Ok(outcome)
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
/// dead one did not: render, `docs: checkpoint n`, `checkpoint_done`. A fix
/// worker has what it changed committed however it ended, judged against
/// its spawn baseline (AD-20007). A limit pauses the run; anything else is
/// dead, and the checkpoint runs again from the board.
fn settle_checkpoint_orphan(
    run: &mut Run,
    docs: &Docs,
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
        let sha =
            checkpoint::commit_checkpoint(run, docs, board, n, w.row_id, &spec, &before, out)?;
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
        let date = clock::now_rfc3339()[..10].to_string();
        append_under(
            &repo.join(&board.dir).join("agent-log.md"),
            "## Traps",
            &format!(
                "- **Dead checkpoint worker ({date})**: checkpoint {n}, session {} exited (code unknown) with {}.",
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
    let file = slice_file(run, board, slice)?;
    set_frontmatter(&file, &[("status", "pending")])?;
    remove_frontmatter_keys(&file, &["lane"])?;
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
    let repo = run.state().repo.clone();
    let fragment = repo
        .join(&board.dir)
        .join(format!("log-slice-{:02}.md", slice));
    let code = exit_code.map_or("exited (code unknown)".to_string(), |c| {
        format!("exited {c}")
    });
    let what = if summary.is_dead() {
        "with no result"
    } else {
        "with a result but the slice neither done nor blocked"
    };
    let date = clock::now_rfc3339()[..10].to_string();
    if dead.len() >= DEATH_CAP {
        let block = dead_block_id(run, board, slice)?;
        append_under(
            &fragment,
            "## Traps",
            &format!(
                "- **Dead worker ({date})**: session {} {code} {what}; second death on this slice, blocked ({block}).",
                w.session_id
            ),
        )?;
        block_dead_slice(
            run,
            board,
            slice,
            &block,
            &dead,
            &format!("The last one {code} {what}. "),
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

/// The id of the death block on `slice`: one already open under the death
/// title (a kill after it was written, before the slice flipped), else the
/// lowest free `BLOCK-NNkk` from `kk = 02` — the slice's own range, so the
/// merge keeps it, and never a number a builder of the slice already used.
fn dead_block_id(run: &Run, board: &Status, slice: u32) -> Result<String> {
    let feature_dir = run.state().repo.join(&board.dir);
    if let Some(e) = open_dead_block(&feature_dir, slice)? {
        return Ok(e.id);
    }
    let mut used: HashSet<u32> = HashSet::new();
    for path in answers::log_files(&feature_dir)? {
        used.extend(answers::entries(&path)?.into_iter().map(|e| e.n));
    }
    let k = (2..100)
        .find(|k| !used.contains(&(slice * 100 + k)))
        .ok_or_else(|| {
            crate::Error::Msg(format!("slice {slice:02} has used every BLOCK-{slice}NN"))
        })?;
    Ok(format!("BLOCK-{}", slice * 100 + k))
}

fn open_dead_block(feature_dir: &Path, slice: u32) -> Result<Option<answers::BlockEntry>> {
    let title = dead_title(slice);
    let mut hit = None;
    for path in answers::log_files(feature_dir)? {
        if let Some(e) = answers::entries(&path)?
            .into_iter()
            .find(|e| !e.resolved && e.slice == Some(slice) && e.title == title)
        {
            hit = Some(e);
        }
    }
    Ok(hit)
}

/// Block `slice` on its dead workers: the entry under `## Blockers` (unless
/// a kill left it written), the slice `blocked`, `slice_blocked`. `last` is
/// what the latest death adds to the entry, empty when the driver came back
/// to a cap reached before it died.
fn block_dead_slice(
    run: &mut Run,
    board: &Status,
    slice: u32,
    block: &str,
    dead: &[WorkerRef],
    last: &str,
) -> Result<()> {
    let repo = run.state().repo.clone();
    let feature_dir = repo.join(&board.dir);
    let title = dead_title(slice);
    if open_dead_block(&feature_dir, slice)?.is_none_or(|e| e.id != block) {
        let fragment = feature_dir.join(format!("log-slice-{:02}.md", slice));
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
        let entry = format!(
            "### {block} — {title} · slice-{nn} · open\n\n\
             **Happened:** {n} workers on slice {nn} exited without leaving it `done` or `blocked`: {sessions}. \
             {last}The slice sits at `building` on the board with nothing alive on it; \
             the driver's once-only respawn (autonomy.md § Caps, hand-back once) is spent. \
             Streams and stderr are under `{dir}/`.\n\
             **Options:** 1. read the last worker's `.stderr` and `.jsonl`, fix the environment (login, PATH, quota) and set the slice back to `pending`; \
             2. build the slice by hand and set it `done`; \
             3. drop the slice from the plan if it no longer applies.\n\
             **Recommended:** 1 — a worker that dies before its first tool call is the environment's fault, not the slice's.\n",
            nn = format!("{:02}", slice),
            n = dead.len(),
            sessions = sessions.join(", "),
            dir = worker::dir(run.dir()).display(),
        );
        append_under(&fragment, "## Blockers", &entry)?;
    }
    set_slice_status(&slice_file(run, board, slice)?, "blocked")?;
    emit_blocked(run, slice, Some(&(block.to_string(), title)))
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

/// The slice's file in the repo, from the board.
fn slice_file(run: &Run, board: &Status, slice: u32) -> Result<PathBuf> {
    board
        .slices
        .iter()
        .find(|s| s.number == slice)
        .map(|s| run.state().repo.join("docs/features").join(&s.file))
        .ok_or_else(|| crate::Error::Msg(format!("slice {slice:02} is not on the board")))
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

/// The slice's frontmatter status as the board reports it; `""` when unknown.
pub fn slice_status(board: &Status, slice: u32) -> &str {
    board
        .slices
        .iter()
        .find(|s| s.number == slice)
        .and_then(|s| s.status.as_deref())
        .unwrap_or("")
}

/// Append `body` at the end of the `heading` section of a log file, creating
/// the file or the section when missing; a `_(none)_` placeholder gives way.
/// Every line of a fragment must sit under a `## ` heading (`docs log merge`).
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
            if at > h + 1 && lines[at - 1].trim() == "_(none)_" {
                at -= 1;
                while at > h + 1 && lines[at - 1].trim().is_empty() {
                    at -= 1;
                }
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
    set_frontmatter(path, &[("status", status)])
}

/// The `status:` of a slice file's frontmatter; `None` when it has none.
pub fn read_slice_status(path: &Path) -> Result<Option<String>> {
    let mut status = None;
    let text = fs::read_to_string(path).map_err(io(format!("read {}", path.display())))?;
    let fm = text
        .strip_prefix("---\n")
        .or_else(|| text.strip_prefix("---\r\n"))
        .and_then(|rest| rest.find("\n---").map(|end| &rest[..end]))
        .ok_or_else(|| crate::Error::Msg(format!("{} has no frontmatter", path.display())))?;
    for line in fm.lines() {
        if let Some(v) = line.strip_prefix("status:") {
            status = Some(v.trim().trim_matches(|c| c == '"' || c == '\'').to_string());
            break;
        }
    }
    Ok(status)
}

/// Set `key: value` lines in a slice file's frontmatter: an existing key is
/// rewritten in place, a missing one appended before the closing `---`.
/// Nothing below the frontmatter is touched.
pub fn set_frontmatter(path: &Path, pairs: &[(&str, &str)]) -> Result<()> {
    edit_frontmatter(path, |lines| {
        for (key, value) in pairs {
            let prefix = format!("{key}:");
            match lines.iter_mut().find(|l| l.starts_with(&prefix)) {
                Some(l) => *l = format!("{key}: {value}"),
                None => lines.push(format!("{key}: {value}")),
            }
        }
    })
}

/// Drop `key:` lines from a slice file's frontmatter (FR-9 clears `lane`
/// when a limited worker's slice goes back to `pending`).
pub fn remove_frontmatter_keys(path: &Path, keys: &[&str]) -> Result<()> {
    edit_frontmatter(path, |lines| {
        lines.retain(|l| !keys.iter().any(|k| l.starts_with(&format!("{k}:"))));
    })
}

/// The file keeps its line endings: a CRLF file's edited lines are CRLF.
fn edit_frontmatter(path: &Path, edit: impl FnOnce(&mut Vec<String>)) -> Result<()> {
    let text = fs::read_to_string(path).map_err(io(format!("read {}", path.display())))?;
    let nl = if text.starts_with("---\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let rest = text
        .strip_prefix(&format!("---{nl}"))
        .ok_or_else(|| crate::Error::Msg(format!("{} has no frontmatter", path.display())))?;
    let end = rest
        .find(&format!("{nl}---"))
        .ok_or_else(|| crate::Error::Msg(format!("{} has no frontmatter end", path.display())))?;
    let fm = &rest[..end];
    let mut lines: Vec<String> = fm
        .lines()
        .map(|l| l.trim_end_matches('\r').to_string())
        .collect();
    edit(&mut lines);
    let out = format!("---{nl}{}{}", lines.join(nl), &rest[end..]);
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
        append_under(
            &p,
            "## Decisions",
            "### AD-101 — q · slice-01 · 2026-10-10\n\n**Chose:** x",
        )
        .unwrap();
        append_under(&p, "## Traps", "- two").unwrap();
        let text = fs::read_to_string(&p).unwrap();
        assert_eq!(
            text,
            "## Traps\n\n- one\n- two\n\n## Decisions\n\n### AD-101 — q · slice-01 · 2026-10-10\n\n**Chose:** x\n"
        );
        append_under(
            &p,
            "## Blockers",
            "### BLOCK-102 — w · slice-01 · open\n\n**Happened:** y",
        )
        .unwrap();
        let text = fs::read_to_string(&p).unwrap();
        assert!(
            text.ends_with(
                "## Blockers\n\n### BLOCK-102 — w · slice-01 · open\n\n**Happened:** y\n"
            ),
            "{text}"
        );
        assert!(!text.contains("\n\n\n"), "{text}");
    }

    #[test]
    fn append_under_replaces_the_none_placeholder() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("agent-log.md");
        fs::write(&p, "# Agent log\n\n## Traps\n\n_(none)_\n\n## Decisions\n\n_(none)_\n\n## Checkpoints\n\n_(none)_\n").unwrap();
        append_under(
            &p,
            "## Decisions",
            "- **DEC (2026-10-10)**: preflight ask \"x\" answered: proceed",
        )
        .unwrap();
        assert_eq!(
            fs::read_to_string(&p).unwrap(),
            "# Agent log\n\n## Traps\n\n_(none)_\n\n## Decisions\n\n- **DEC (2026-10-10)**: preflight ask \"x\" answered: proceed\n\n## Checkpoints\n\n_(none)_\n"
        );
    }

    #[test]
    fn set_slice_status_touches_only_the_status_line() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("slice-01-a.md");
        fs::write(
            &p,
            "---\nslice: 01\nstatus: building\ndepends_on: []\n---\n\n# A\n\nstatus: not this\n",
        )
        .unwrap();
        set_slice_status(&p, "blocked").unwrap();
        assert_eq!(
            fs::read_to_string(&p).unwrap(),
            "---\nslice: 01\nstatus: blocked\ndepends_on: []\n---\n\n# A\n\nstatus: not this\n"
        );
    }

    #[test]
    fn set_frontmatter_rewrites_or_appends_the_lane() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("slice-02-b.md");
        fs::write(
            &p,
            "---\nslice: 02\nstatus: pending\ndepends_on: []\n---\n\n# B\n",
        )
        .unwrap();
        set_frontmatter(&p, &[("status", "building"), ("lane", "2")]).unwrap();
        assert_eq!(
            fs::read_to_string(&p).unwrap(),
            "---\nslice: 02\nstatus: building\ndepends_on: []\nlane: 2\n---\n\n# B\n"
        );
        set_frontmatter(&p, &[("lane", "1")]).unwrap();
        assert_eq!(
            fs::read_to_string(&p).unwrap(),
            "---\nslice: 02\nstatus: building\ndepends_on: []\nlane: 1\n---\n\n# B\n"
        );
        assert!(set_frontmatter(&dir.path().join("none.md"), &[("lane", "1")]).is_err());
    }

    #[test]
    fn a_crlf_slice_file_is_edited_and_stays_crlf() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("slice-02-b.md");
        fs::write(
            &p,
            "---\r\nslice: 02\r\nstatus: pending\r\nlane: 2\r\n---\r\n\r\n# B\r\n",
        )
        .unwrap();
        set_frontmatter(&p, &[("status", "building"), ("lane", "1")]).unwrap();
        assert_eq!(
            fs::read_to_string(&p).unwrap(),
            "---\r\nslice: 02\r\nstatus: building\r\nlane: 1\r\n---\r\n\r\n# B\r\n"
        );
        remove_frontmatter_keys(&p, &["lane"]).unwrap();
        set_frontmatter(&p, &[("owner", "x")]).unwrap();
        assert_eq!(
            fs::read_to_string(&p).unwrap(),
            "---\r\nslice: 02\r\nstatus: building\r\nowner: x\r\n---\r\n\r\n# B\r\n"
        );
    }
}
