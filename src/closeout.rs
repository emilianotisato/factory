//! Every step is idempotent on re-entry; nothing here merges or marks the PR ready.
use std::collections::{BTreeSet, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::answers;
use crate::checkpoint::{Landed, Outcome};
use crate::ci::{self, CheckRow, Checks, CiRun, Verdict};
use crate::cli::report::{self, Report};
use crate::cli::run::load_preflight;
use crate::commit;
use crate::config::{self, Config};
use crate::db::RetroRow;
use crate::docs::{Docs, Ship, Status};
use crate::error::io;
use crate::events::EventKind;
use crate::land::{self, Landing};
use crate::pack::{self, Pack};
use crate::quota::{self, Hold, Limit};
use crate::run::{Phase, Run, WorkerOutcome, WorkerRef};
use crate::scheduler::{self, DEATH_CAP, say, short};
use crate::stream::{Findings, Summary};
use crate::worker::{self, Exit, Job, Worker};
use crate::{Result, clock, collect, lanes, metrics, paths, retro};

/// The phases a closeout block names in its heading.
pub const PHASES: [&str; 3] = ["polish", "pr", "ci"];

pub const PR_BODY_FILE: &str = "pr-body.md";
pub const COLLECT_FILE: &str = "collect.md";

pub const STAND_IN_RETRO: &str = "retro";
pub const STAND_IN_BODY: &str = "pr_body";
/// How the PR body's stand-in line starts, so a later pass replaces it.
const STAND_IN_MARK: &str = "**Template stand-in";

// The closeout blocks whose `proceed` answer the driver reads, by title.
const POLISH_DIED: &str = "Two polish workers died";
const POLISH_CONFLICT: &str = "the polish worker's commit conflicts with the feature tip";
const PR_EDIT: &str = "gh pr edit refused";
const PR_CREATE: &str = "gh pr create refused";
const CHECKS_UNREADABLE: &str = "gh pr checks unreadable";
const VIEW_UNREADABLE: &str = "gh pr view unreadable";
const FLAKY: &str = "flaky check ";
const INFRA: &str = "CI did not run to a verdict";
const TIMEOUT: &str = "CI still pending after";
/// A `proceed` to one of these ends the watch: the checks are the owner's.
const ENDS_WATCH: [&str; 5] = [CHECKS_UNREADABLE, VIEW_UNREADABLE, FLAKY, INFRA, TIMEOUT];

pub fn run(
    run: &mut Run,
    docs: &Docs,
    cfg: &Config,
    board: &Status,
    out: &mut dyn Write,
) -> Result<Outcome> {
    let repo = run.state().repo.clone();
    let feature_dir = repo.join(&board.dir);

    if let Some(open) = open_blocks(&feature_dir)?.into_iter().next() {
        return Ok(Outcome::Parked {
            what: park_line(&open.id, &open.title, &open.phase),
            id: open.id,
            fresh: false,
        });
    }

    // A board `shipped` implies polish; the ship counts only once HEAD holds it (`docs ship` reruns as a no-op).
    let board_shipped = board.status.as_deref() == Some("shipped");
    let ship_committed = board_shipped && shipped_at_head(&repo, &board.dir)?;
    let events = run.events()?;
    let polish_by_hand = answered(run, &feature_dir)?
        .iter()
        .any(|a| a.proceed && (a.title == POLISH_DIED || a.title == POLISH_CONFLICT));
    let polished = board_shipped
        || polish_by_hand
        || events
            .iter()
            .any(|e| matches!(e.kind, EventKind::PolishDone { .. }));
    let ship_event = events
        .iter()
        .any(|e| matches!(e.kind, EventKind::Shipped { .. }));
    let shipped = ship_committed || ship_event;
    // The driver's tick leaves a HEAD off the run's branch to the closeout: it blocks before the next step.
    let (phase, what) = match (polished, shipped, run.state().pr_url.is_some()) {
        (false, _, _) => ("polish", "polish"),
        (true, false, _) => ("polish", "ship"),
        (true, true, false) => ("pr", "push"),
        (true, true, true) => ("ci", "CI watch"),
    };
    if let Some(o) = branch_guard(run, docs, &feature_dir, phase, what, out)? {
        return Ok(o);
    }
    if run.state().phase != Phase::Closeout {
        run.set_phase(Phase::Closeout)?;
    }
    say(
        out,
        &format!(
            "closeout — {}",
            match (polished, shipped, run.state().pr_url.is_some()) {
                (false, _, _) => "polish",
                (true, false, _) if board_shipped => {
                    "PRD shipped but not committed, resumed at ship"
                }
                (true, false, _) if polish_by_hand => "polish answered `proceed`, resumed at ship",
                (true, false, _) => "resumed at ship",
                (true, true, false) if !ship_event => "PRD already shipped, resumed at the PR",
                (true, true, false) => "resumed at the PR",
                (true, true, true) => "resumed at CI",
            }
        ),
    )?;

    if !polished && let Some(o) = polish(run, docs, cfg, board, out)? {
        return Ok(o);
    }
    if !shipped && let Some(o) = ship(run, docs, board, out)? {
        return Ok(o);
    }
    if let Some(o) = open_pr(run, docs, board, out)? {
        return Ok(o);
    }
    if let Some(o) = watch_ci(run, docs, cfg, board, out)? {
        return Ok(o);
    }
    Ok(Outcome::Done)
}

// ── polish ──────────────────────────────────────────────────────────────────

fn polish(
    run: &mut Run,
    docs: &Docs,
    cfg: &Config,
    board: &Status,
    out: &mut dyn Write,
) -> Result<Option<Outcome>> {
    let repo = run.state().repo.clone();
    let feature_dir = repo.join(&board.dir);
    let default_branch = default_branch(run, docs);
    let feature_branch = feature_branch(run, &repo)?;
    let template = retro_template();
    loop {
        if let Some(o) = await_quota(run, out)? {
            return Ok(Some(o));
        }
        let since = last_answer(run)?;
        let mine = |w: &WorkerRef| {
            w.role == "polish" && since.as_ref().is_none_or(|at| w.spawned_at > *at)
        };
        let dead: Vec<&WorkerRef> = run
            .state()
            .workers
            .iter()
            .filter(|w| mine(w) && w.died())
            .collect();
        if dead.len() >= DEATH_CAP {
            let sessions: Vec<String> = dead
                .iter()
                .map(|w| {
                    format!(
                        "`{}` ({})",
                        w.session_id,
                        w.outcome.as_deref().unwrap_or("?")
                    )
                })
                .collect();
            let body = format!(
                "**Happened:** {} polish workers exited without a result or a block: {}. \
                 Every slice is done and the last checkpoint is committed; the driver's once-only respawn is spent. \
                 Streams and stderr are under `{}/`.\n\
                 **Options:**\n\
                 1. read the last worker's `.stderr` and `.jsonl`, fix the environment (login, PATH, quota) and answer this block — a fresh polish worker runs\n\
                 2. run `/polish` by hand, write `retro.md` and `runs/<id>/pr-body.md`, run `factory docs set NNN reviewed=<date>`, commit, and answer this block with `proceed` — the driver ships from there, no polish worker\n\
                 **Recommended:** 1 — a worker that dies before its first tool call is the environment's fault, not the review's.\n",
                dead.len(),
                sessions.join(", "),
                worker::dir(run.dir()).display(),
            );
            return Ok(Some(block(
                run,
                &feature_dir,
                "polish",
                "Two polish workers died",
                &body,
                out,
            )?));
        }
        let attempt = run.state().workers.iter().filter(|w| mine(w)).count() as u32 + 1;
        let spawn = run.state().workers.len() as u32 + 1;
        let lane = closeout_lane(board);
        let (lane_dir, lane_def) = scheduler::prepare_lane(run, lane, true, out)?;
        let collected = collected(
            &lane_dir,
            &default_branch,
            &cfg.review,
            &collect_path(run.dir()),
            out,
        )?;
        // The collect can outlast a stop by a whole pass; no worker spawns after one.
        run.reload()?;
        if run.state().stop_requested {
            return Ok(Some(Outcome::Stopped));
        }
        let packs = scheduler::packs_dir(run.dir());
        let pack = Pack::for_polish(
            docs,
            board,
            run.dir(),
            &default_branch,
            &feature_branch,
            &collected,
            &template,
            &packs,
            spawn,
        )?;
        let open_before: HashSet<String> = open_blocks(&feature_dir)?
            .into_iter()
            .map(|b| b.id)
            .collect();
        drop_ref(run, board, &Closer::Polish)?;
        let w = Worker::spawn(
            run,
            Job::Polish { attempt },
            &pack,
            &cfg.workers.polish,
            &lane_def.env(),
            &lane_dir,
            Some(lane),
        )?;
        let session_id = w.session_id.clone();
        let row_id = w.row_id;
        say(
            out,
            &format!(
                "polish: worker {session_id} spawned in lane {lane} (pid {}, attempt {attempt}, pack {})",
                w.pid,
                pack.path.display()
            ),
        )?;
        let exit = w.wait()?;

        if let Some(b) = open_blocks(&feature_dir)?
            .into_iter()
            .find(|b| !open_before.contains(&b.id))
        {
            settle(
                run,
                row_id,
                &session_id,
                &exit.summary,
                WorkerOutcome::Blocked,
            )?;
            run.emit(EventKind::CloseoutBlocked {
                phase: b.phase.clone(),
                block_id: b.id.clone(),
                question: b.title.clone(),
            })?;
            say(
                out,
                &format!(
                    "polish: worker {session_id} blocked — {} in log.jsonl: {}",
                    b.id, b.title
                ),
            )?;
            return Ok(Some(Outcome::Parked {
                what: park_line(&b.id, &b.title, &b.phase),
                id: b.id,
                fresh: true,
            }));
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
                    "polish: worker {session_id} hit a usage limit ({}) — paused until {}, a fresh worker follows",
                    limit.literal,
                    clock::rfc3339(until)
                ),
            )?;
            continue;
        }
        if exit.summary.is_dead() {
            settle_dead(run, &feature_dir, "polish", &session_id, row_id, &exit, out)?;
            continue;
        }
        // Only a return block has a `FINDINGS` line to miss.
        if exit.summary.status_hint.is_some() {
            let findings = exit
                .summary
                .result_text
                .as_deref()
                .and_then(Findings::from_result);
            run.db().set_run_findings(run.id().as_str(), findings)?;
            if findings.is_none() {
                run.emit(EventKind::PolishFindingsMissing {
                    session_id: session_id.clone(),
                })?;
            }
        }
        // Land before settling: a driver killed between the two leaves the worker to the resume, not to a second polish.
        match land_here(run, board, &Closer::Polish, lane, row_id, out)? {
            Ok(_) => {}
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
                return Ok(Some(o));
            }
        }
        settle(run, row_id, &session_id, &exit.summary, WorkerOutcome::Done)?;
        return Ok(None);
    }
}

/// The `# Collected` body, from the file while `collect::reusable` holds at the lane's HEAD; a failed collect is
/// the body, never the error.
fn collected(
    lane_dir: &Path,
    base: &str,
    review: &config::Review,
    file: &Path,
    out: &mut dyn Write,
) -> Result<String> {
    if let Ok(head) = ci::head(lane_dir)
        && let Ok(text) = fs::read_to_string(file)
        && collect::reusable(&text, &head)
    {
        say(
            out,
            &format!("polish: reused {} (head {})", file.display(), short(&head)),
        )?;
        return Ok(pack::collected(&text, file));
    }
    let started = Instant::now();
    let written =
        collect::collect(lane_dir, base, review, &mut Stamped::new(out)).and_then(|text| {
            // A torn file would carry a head line and be reused as finished.
            scheduler::write_atomic(file, &text)?;
            Ok(text)
        });
    let secs = started.elapsed().as_secs_f64();
    match written {
        Ok(text) => {
            say(
                out,
                &format!(
                    "polish: collected {} against {base} in {secs:.1} s",
                    file.display()
                ),
            )?;
            Ok(pack::collected(&text, file))
        }
        Err(e) => {
            let reason = e.to_string().trim().replace('\n', " ");
            say(
                out,
                &format!("polish: collect failed in {secs:.1} s — {reason}"),
            )?;
            Ok(format!("collect failed: {reason}"))
        }
    }
}

/// Prefixes each whole line with the time, as `say` does, so progress written by the library reads like the rest of the driver log.
struct Stamped<'a> {
    out: &'a mut dyn Write,
    line: Vec<u8>,
}

impl<'a> Stamped<'a> {
    fn new(out: &'a mut dyn Write) -> Self {
        Stamped {
            out,
            line: Vec::new(),
        }
    }
}

impl Write for Stamped<'_> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.line.extend_from_slice(buf);
        while let Some(end) = self.line.iter().position(|&b| b == b'\n') {
            let line: Vec<u8> = self.line.drain(..=end).collect();
            write!(self.out, "{} ", clock::now_rfc3339())?;
            self.out.write_all(&line)?;
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.out.flush()
    }
}

// ── landing ─────────────────────────────────────────────────────────────────

enum Closer<'a> {
    Polish,
    CiFix { round: u32, check: &'a str },
}

impl Closer<'_> {
    fn ref_name(&self, board: &Status) -> String {
        let key = match self {
            Closer::Polish => "polish".to_string(),
            Closer::CiFix { round, .. } => format!("ci-fix-{round}"),
        };
        format!("refs/factory/{:03}/{key}", board.number)
    }

    fn subject(&self, board: &Status) -> String {
        match self {
            Closer::Polish => commit::subject(board, "fix", "polish review fixes"),
            Closer::CiFix { check, .. } => commit::subject(board, "fix", &format!("CI {check}")),
        }
    }

    /// Polish may write `prd.md` (its Status prose); `docs/index.md` stays the ship's.
    fn protected(&self, board: &Status) -> Vec<String> {
        match self {
            Closer::Polish => vec!["docs/index.md".to_string()],
            Closer::CiFix { .. } => commit::protected_files(board).to_vec(),
        }
    }

    fn phase(&self) -> &'static str {
        match self {
            Closer::Polish => "polish",
            Closer::CiFix { .. } => "ci",
        }
    }

    fn worker(&self) -> String {
        match self {
            Closer::Polish => "polish worker".to_string(),
            Closer::CiFix { round, .. } => format!("CI fix worker of round {round}"),
        }
    }
}

fn closeout_lane(board: &Status) -> u32 {
    board.free_lanes.first().copied().unwrap_or(1)
}

/// A ref an earlier worker left unlanded would land in place of the next worker's lane.
fn drop_ref(run: &Run, board: &Status, who: &Closer) -> Result<()> {
    let repo = run.state().repo.clone();
    let name = who.ref_name(board);
    if scheduler::resolve(&repo, &name)?.is_some() {
        scheduler::git_ok(&repo, &["update-ref", "-d", &name])?;
    }
    Ok(())
}

/// The retro stand-in goes in the lane so it lands with the commit; the body in the run dir.
fn stand_ins(run: &mut Run, board: &Status, lane_dir: &Path, out: &mut dyn Write) -> Result<()> {
    let feature = run.state().feature.clone();
    let retro_rel = format!("{}/retro.md", board.dir);
    let retro = lane_dir.join(&retro_rel);
    if !retro.exists() {
        fs::write(&retro, retro_template().replace("{NNN}", &feature))
            .map_err(io(format!("write {}", retro.display())))?;
        say(
            out,
            &format!("polish: worker left no {retro_rel} — created from the template"),
        )?;
        stood_in(run, STAND_IN_RETRO, &retro_rel)?;
    }
    let body = run.dir().join(PR_BODY_FILE);
    if !body.exists() {
        fs::write(&body, fallback_body(board, &feature))
            .map_err(io(format!("write {}", body.display())))?;
        say(
            out,
            &format!(
                "polish: worker left no {} — a minimal body stands in",
                body.display()
            ),
        )?;
        stood_in(run, STAND_IN_BODY, &body.display().to_string())?;
    }
    flag_body(run, &body)
}

/// Safe to call again for the same worker: a resumed driver finishes the landing this way.
fn land_worker(
    run: &mut Run,
    board: &Status,
    who: &Closer,
    lane: u32,
    row_id: i64,
    out: &mut dyn Write,
) -> Result<Landed> {
    let landed = run
        .db()
        .workers(run.id().as_str())?
        .into_iter()
        .find(|w| w.id == row_id)
        .and_then(|w| w.commit_sha);
    if let Some(sha) = landed {
        emit_landed(run, who, &sha)?;
        return Ok(Landed::Done(Some(sha)));
    }
    let repo = run.state().repo.clone();
    let board_files = commit::tool_files(board).to_vec();
    let name = who.ref_name(board);
    let subject = who.subject(board);
    let base = run.state().lane_bases.get(&lane).cloned();
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
            record(run, who, row_id, &sha, out)?;
            return Ok(Landed::Done(Some(sha)));
        }
    }
    let sha = match scheduler::resolve(&repo, &name)? {
        Some(sha) => sha,
        None => {
            let main = lanes::main_tree(&repo)?;
            let lane_dir = lanes::dir(&paths::data_dir(), &main, lane);
            if matches!(who, Closer::Polish) {
                stand_ins(run, board, &lane_dir, out)?;
            }
            let committed = match &base {
                Some(base) => land::commit_lane(
                    &lane_dir,
                    base,
                    &board_files,
                    &who.protected(board),
                    &subject,
                    &name,
                )?,
                None => None,
            };
            let Some(c) = committed else {
                return Ok(Landed::Done(nothing(run, board, who, row_id, out)?));
            };
            for path in &c.dropped {
                say(
                    out,
                    &format!(
                        "{}: {} wrote {path}, which no {} may — left out of the commit",
                        who.phase(),
                        who.worker(),
                        match who {
                            Closer::Polish => "polish worker",
                            Closer::CiFix { .. } => "fix",
                        }
                    ),
                )?;
            }
            c.sha
        }
    };
    scheduler::undo_half_pick(&repo, &sha)?;
    match land::land(&repo, &sha, &board_files)? {
        Landing::Landed { sha } => {
            record(run, who, row_id, &sha, out)?;
            Ok(Landed::Done(Some(sha)))
        }
        Landing::Dirty { paths } => Ok(Landed::Waiting(format!(
            "the {}'s commit cannot land: {} changed in the main tree — commit or discard them",
            who.worker(),
            paths.join(", ")
        ))),
        Landing::Conflict { paths } => Ok(Landed::Conflict { paths, name }),
    }
}

/// `Ok(None)`: nothing to commit; `Err`: the run stopped or parked.
fn land_here(
    run: &mut Run,
    board: &Status,
    who: &Closer,
    lane: u32,
    row_id: i64,
    out: &mut dyn Write,
) -> Result<std::result::Result<Option<String>, Outcome>> {
    let (mut parked, mut announced) = (false, false);
    loop {
        match land_worker(run, board, who, lane, row_id, out)? {
            Landed::Done(sha) => {
                if parked {
                    say(
                        out,
                        &format!(
                            "{}: the landing went through — the closeout goes on",
                            who.phase()
                        ),
                    )?;
                    run.set_phase(Phase::Closeout)?;
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
                return Ok(Err(block_conflict(run, board, who, &paths, &name, out)?));
            }
        }
    }
}

/// Polish still commits the main tree's board files: its `reviewed` lands there.
fn nothing(
    run: &mut Run,
    board: &Status,
    who: &Closer,
    row_id: i64,
    out: &mut dyn Write,
) -> Result<Option<String>> {
    if let Closer::CiFix { round, .. } = who {
        say(
            out,
            &format!("ci: fix worker of round {round} changed nothing to commit"),
        )?;
        return Ok(None);
    }
    let repo = run.state().repo.clone();
    let files = commit::tool_files(board).to_vec();
    if let Some(c) = commit::commit(&repo, &files, &who.subject(board))? {
        record(run, who, row_id, &c.sha, out)?;
        return Ok(Some(c.sha));
    }
    say(out, "polish: worker changed nothing to commit")?;
    let logged = run
        .events()?
        .iter()
        .any(|e| matches!(e.kind, EventKind::PolishDone { .. }));
    if !logged {
        run.emit(EventKind::PolishDone {
            sha: None,
            files: vec![],
        })?;
    }
    Ok(None)
}

fn record(run: &mut Run, who: &Closer, row_id: i64, sha: &str, out: &mut dyn Write) -> Result<()> {
    run.db().set_worker_commit(row_id, sha)?;
    let files = landed_files(run, sha)?;
    let head = match who {
        Closer::Polish => "polish:".to_string(),
        Closer::CiFix { round, .. } => format!("ci: round {round}"),
    };
    say(
        out,
        &format!(
            "{head} committed {} — {} file(s): {}",
            short(sha),
            files.len(),
            files.join(", ")
        ),
    )?;
    emit_landed(run, who, sha)
}

/// A driver killed between the row's commit and this event would run a second polish over a landed one.
fn emit_landed(run: &mut Run, who: &Closer, sha: &str) -> Result<()> {
    let logged = run.events()?.iter().any(|e| match (&e.kind, who) {
        (EventKind::PolishDone { sha: Some(s), .. }, Closer::Polish) => s == sha,
        (EventKind::CiFixCommitted { sha: s, .. }, Closer::CiFix { .. }) => s == sha,
        _ => false,
    });
    if logged {
        return Ok(());
    }
    let files = landed_files(run, sha)?;
    run.emit(match who {
        Closer::Polish => EventKind::PolishDone {
            sha: Some(sha.to_string()),
            files,
        },
        Closer::CiFix { round, .. } => EventKind::CiFixCommitted {
            round: *round,
            sha: sha.to_string(),
            files,
        },
    })?;
    Ok(())
}

fn landed_files(run: &Run, sha: &str) -> Result<Vec<String>> {
    Ok(scheduler::nul_split(&scheduler::git_ok(
        &run.state().repo,
        &[
            "diff-tree",
            "--no-commit-id",
            "--name-only",
            "-r",
            "-z",
            sha,
        ],
    )?))
}

/// Only the owner moves the tip while a closeout worker runs; the block names the paths and the ref keeping the commit.
fn block_conflict(
    run: &mut Run,
    board: &Status,
    who: &Closer,
    paths: &[String],
    name: &str,
    out: &mut dyn Write,
) -> Result<Outcome> {
    let feature_dir = run.state().repo.join(&board.dir);
    let (title, options) = match who {
        Closer::Polish => (
            POLISH_CONFLICT.to_string(),
            format!(
                "1. `git cherry-pick {name}`, resolve, commit, and answer this block with `proceed` — the driver ships from there, no second polish worker\n\
                 2. answer this block with anything else — a fresh polish worker reviews the tip as it is, and its spawn deletes the ref\n\
                 **Recommended:** 1 — the commit holds everything the polish worker left in its lane.\n"
            ),
        ),
        Closer::CiFix { round, check } => (
            format!("the CI fix of round {round} conflicts with the feature tip"),
            format!(
                "1. `git cherry-pick {name}`, resolve, commit, and answer this block — the driver pushes and watches `{check}` again\n\
                 2. answer this block without it — the driver pushes the tip as it is and watches again\n\
                 **Recommended:** 1 — the commit holds everything the fix worker left in its lane.\n"
            ),
        ),
    };
    let body = format!(
        "**Happened:** the {}'s lane, committed as `{name}`, does not apply on the feature tip, \
         which moved while it ran; it conflicts in {}. The main tree is untouched.\n\
         **Options:**\n{options}",
        who.worker(),
        paths.join(", ")
    );
    block(run, &feature_dir, who.phase(), &title, &body, out)
}

fn stood_in(run: &mut Run, what: &str, path: &str) -> Result<()> {
    let seen = run
        .events()?
        .iter()
        .any(|e| matches!(&e.kind, EventKind::TemplateStandIn { what: w, .. } if w == what));
    if !seen {
        run.emit(EventKind::TemplateStandIn {
            what: what.to_string(),
            path: path.to_string(),
        })?;
    }
    Ok(())
}

pub fn stand_in_line(events: &[crate::events::Event]) -> Option<String> {
    let mut retro: Option<&str> = None;
    let mut body = false;
    for e in events {
        if let EventKind::TemplateStandIn { what, path } = &e.kind {
            match what.as_str() {
                STAND_IN_RETRO => retro = Some(path),
                STAND_IN_BODY => body = true,
                _ => {}
            }
        }
    }
    let the_retro = |path: &str| {
        if path.is_empty() {
            "the retro".to_string()
        } else {
            format!("the retro (`{path}`)")
        }
    };
    match (retro, body) {
        (None, false) => None,
        (Some(r), false) => Some(format!(
            "{STAND_IN_MARK}:** {} is factory's template — the polish worker wrote none.",
            the_retro(r)
        )),
        (None, true) => Some(format!(
            "{STAND_IN_MARK}:** this PR body is factory's template — the polish worker wrote none."
        )),
        (Some(r), true) => Some(format!(
            "{STAND_IN_MARK}s:** this PR body and {} are factory's templates — the polish worker wrote neither.",
            the_retro(r)
        )),
    }
}

pub fn flagged(body: &str, line: &str) -> String {
    let rest = if body.starts_with(STAND_IN_MARK) {
        body.split_once('\n')
            .map_or("", |(_, r)| r)
            .trim_start_matches(['\n', '\r'])
    } else {
        body
    };
    format!("{line}\n\n{rest}")
}

fn flag_body(run: &Run, body: &Path) -> Result<()> {
    let Some(line) = stand_in_line(&run.events()?) else {
        return Ok(());
    };
    let text = fs::read_to_string(body).map_err(io(format!("read {}", body.display())))?;
    let want = flagged(&text, &line);
    if want != text {
        scheduler::write_atomic(body, &want)?;
    }
    Ok(())
}

fn fallback_body(board: &Status, feature: &str) -> String {
    format!(
        "## PRD-{feature} — {}\n\n\
         PRD: `{}/prd.md`\n\n\
         > Opened by `factory`, unattended. **Not merged — that is yours.** \
         The polish worker left no PR body; the agent log and the retro in `{}/` carry the decisions and the numbers.\n\n\
         ### Needs your review\n\n- `factory docs log show {feature} --decide`: the `decide` entries\n\n\
         ### Human QA\n\n- see `{}/prd.md` § QA plan\n",
        board.slug, board.dir, board.dir, board.dir
    )
}

// ── ship ────────────────────────────────────────────────────────────────────

fn ship(
    run: &mut Run,
    docs: &Docs,
    board: &Status,
    out: &mut dyn Write,
) -> Result<Option<Outcome>> {
    let feature = run.state().feature.clone();
    let repo = run.state().repo.clone();
    let feature_dir = repo.join(&board.dir);
    if let Some(o) = branch_guard(run, docs, &feature_dir, "polish", "ship", out)? {
        return Ok(Some(o));
    }
    let shipped = match docs.ship(&feature)? {
        Ship::Done(stdout) => stdout,
        Ship::Refused(text) => {
            let text = text.trim().to_string();
            let first = text
                .lines()
                .map(str::trim)
                .find(|l| !l.is_empty())
                .unwrap_or("docs ship failed")
                .to_string();
            let quoted: String = text.lines().map(|l| format!("  {l}\n")).collect();
            let body = format!(
                "**Happened:** the polish commit is in and `docs ship {feature}` refused:\n{quoted}\
             **Options:**\n\
             1. fix what it names (a slice not `done`, a missing frontmatter), commit, and answer this block — the ship runs again\n\
             2. revert the polish commit and set the feature back to `building` if the board is wrong about being done\n\
             **Recommended:** 1 — the tool refuses only when the board and the tree disagree; the board is the truth to fix.\n"
            );
            return Ok(Some(block(
                run,
                &feature_dir,
                "polish",
                &format!("docs ship PRD-{feature} refused: {first}"),
                &body,
                out,
            )?));
        }
    };
    for line in shipped.lines().filter(|l| l.starts_with('⚠')) {
        say(out, &format!("ship: {}", line.trim()))?;
    }
    // Polish may have written `AD` entries since `board` was read.
    let log = docs
        .status(&feature)
        .map(|s| s.log)
        .unwrap_or_else(|_| board.log.clone());
    run.db().set_run_log_counts(
        run.id().as_str(),
        log.decisions,
        log.decide.len() as u32,
        log.blocks,
    )?;
    let base = default_branch(run, docs);
    let lines = match retro::changed_lines(&repo, &base) {
        Ok(l) => Some(l),
        Err(e) => {
            say(out, &format!("ship: no changed lines stored — {e}"))?;
            None
        }
    };
    run.db().set_run_lines(run.id().as_str(), lines)?;
    let leaks = match crate::leaks::scan(&repo, crate::leaks::Input::Base(&base)) {
        Ok(r) => Some((r.comment_lines, r.added_lines, r.id_hits() as u64)),
        Err(e) => {
            say(out, &format!("ship: no leaks figures stored — {e}"))?;
            None
        }
    };
    run.db().set_run_leaks(run.id().as_str(), leaks)?;
    let report = report::compute(run.db(), run.id().as_str(), None)?;
    let retro = feature_dir.join("retro.md");
    if !retro.exists() {
        fs::write(&retro, retro_template().replace("{NNN}", &feature))
            .map_err(io(format!("write {}", retro.display())))?;
        let retro_rel = format!("{}/retro.md", board.dir);
        say(
            out,
            &format!("ship: no {retro_rel} — created from the template"),
        )?;
        stood_in(run, STAND_IN_RETRO, &retro_rel)?;
    }
    retro_row(run, board, &report)?;
    if let Err(e) = retro::write(&retro, &retro::numbers(run.db(), run.id().as_str(), &repo)?) {
        say(out, &format!("ship: no ## Numbers written — {e}"))?;
    }
    let want = commit::expand(&repo, std::slice::from_ref(&board.dir))?;
    let committed = commit::commit(
        &repo,
        &want,
        &commit::subject(board, "docs", &format!("ship PRD-{feature}")),
    )?;
    match &committed {
        Some(c) => say(
            out,
            &format!(
                "ship: PRD-{feature} shipped, {} — {} file(s)",
                short(&c.sha),
                c.files.len()
            ),
        )?,
        None => say(out, &format!("ship: PRD-{feature} already shipped"))?,
    }
    run.emit(EventKind::Shipped {
        sha: committed.map(|c| c.sha),
    })?;
    Ok(None)
}

/// `score` stays null: the upsert keeps one the polish worker or the owner already set.
fn retro_row(run: &Run, board: &Status, r: &Report) -> Result<()> {
    let n = |v: u64| Some(v as i64);
    run.db().upsert_retro(&RetroRow {
        repo: run.state().repo.to_string_lossy().into_owned(),
        feature: board.number,
        slug: board.slug.clone(),
        shipped_at: Some(crate::docs::scaffold::today()),
        run_id: Some(run.id().as_str().to_string()),
        score: None,
        gate_runs: n(r.gate_runs),
        gate_red: n(r.gate_red),
        gate_minutes: Some(r.gate_minutes),
        ci_cycles: n(r.ci_cycles),
        slices: n(r.slices),
        checkpoints: n(r.checkpoints),
        build_minutes: Some(r.build_minutes),
        polish_minutes: Some(r.polish_minutes),
        out_tokens: n(r.out_tokens),
        cache_read_tokens: n(r.cache_read_tokens),
        start_context: n(r.start_context),
        peak_context: n(r.peak_context),
        written_at: String::new(),
    })
}

// ── push and PR ─────────────────────────────────────────────────────────────

fn open_pr(
    run: &mut Run,
    docs: &Docs,
    board: &Status,
    out: &mut dyn Write,
) -> Result<Option<Outcome>> {
    let feature = run.state().feature.clone();
    let repo = run.state().repo.clone();
    let feature_dir = repo.join(&board.dir);
    let branch = feature_branch(run, &repo)?;
    let done = answered(run, &feature_dir)?;
    if run.state().pr_url.is_none()
        && let Some(a) = done
            .iter()
            .find(|a| a.proceed && a.title.starts_with(PR_CREATE))
    {
        say(
            out,
            &format!(
                "pr: {} answered `proceed` — the PR is the owner's; the run finishes without a CI watch",
                a.id
            ),
        )?;
        return Ok(Some(Outcome::Done));
    }
    if let Some(o) = push(run, docs, &feature_dir, &branch, out)? {
        return Ok(Some(o));
    }
    let body = run.dir().join(PR_BODY_FILE);
    if !body.exists() {
        fs::write(&body, fallback_body(board, &feature))
            .map_err(io(format!("write {}", body.display())))?;
        say(
            out,
            &format!("pr: no {} — a minimal body stands in", body.display()),
        )?;
        stood_in(run, STAND_IN_BODY, &body.display().to_string())?;
    }
    flag_body(run, &body)?;
    let known = match run.state().pr_url.clone() {
        Some(url) => Some(url),
        None => {
            let found = ci::pr_view(&repo, &branch)?;
            if let Some(url) = &found {
                run.set_pr_url(url.clone())?;
                say(out, &format!("pr: {url} is already open for {branch}"))?;
            }
            found
        }
    };
    if let Some(url) = known {
        if let Some(a) = done
            .iter()
            .find(|a| a.proceed && a.title.starts_with(PR_EDIT))
        {
            say(
                out,
                &format!("pr: {url} body left as it is — {} answered `proceed`", a.id),
            )?;
            return Ok(None);
        }
        let pr = ci::pr_number(&url);
        match ci::pr_edit(&repo, &pr, &body)? {
            Ok(()) => say(out, &format!("pr: {url} body refreshed (resume)"))?,
            Err(text) => {
                let body = format!(
                    "**Happened:** the draft PR {url} exists and `gh pr edit {pr} --body-file …` refused:\n  {text}\n\
                     **Options:**\n\
                     1. fix `gh` (login, network) and answer this block — the edit runs again\n\
                     2. answer `proceed` to leave the body as it is — CI is watched either way\n\
                     **Recommended:** 1.\n"
                );
                return Ok(Some(block(
                    run,
                    &feature_dir,
                    "pr",
                    &format!("{PR_EDIT}: {}", first_line(&text)),
                    &body,
                    out,
                )?));
            }
        }
        return Ok(None);
    }
    let base = default_branch(run, docs);
    let title = format!(
        "PRD-{feature}: {}",
        pr_title(&repo.join(&board.dir).join("prd.md"), &feature, &board.slug)
    );
    match ci::pr_create(&repo, &base, &branch, &title, &body)? {
        Ok(url) => {
            run.set_pr_url(url.clone())?;
            say(out, &format!("pr: draft opened against {base} — {url}"))?;
            run.emit(EventKind::PrOpened { url })?;
            Ok(None)
        }
        Err(text) => {
            let body = format!(
                "**Happened:** the branch `{branch}` is pushed and `gh pr create --draft --base {base} --title \"{title}\" --body-file … --head {branch}` refused:\n  {text}\n\
                 **Options:**\n\
                 1. fix `gh` (login, the remote, an existing PR for this branch) and answer this block — the push and the create run again\n\
                 2. open the PR by hand and answer this block with `proceed` — the driver then opens none and the run finishes without a CI watch\n\
                 **Recommended:** 1 — the body is written at `{}` either way.\n",
                run.dir().join(PR_BODY_FILE).display()
            );
            Ok(Some(block(
                run,
                &feature_dir,
                "pr",
                &format!("{PR_CREATE}: {}", first_line(&text)),
                &body,
                out,
            )?))
        }
    }
}

fn push(
    run: &mut Run,
    docs: &Docs,
    feature_dir: &Path,
    branch: &str,
    out: &mut dyn Write,
) -> Result<Option<Outcome>> {
    if let Some(o) = branch_guard(run, docs, feature_dir, "pr", "push", out)? {
        return Ok(Some(o));
    }
    let repo = run.state().repo.clone();
    match ci::push(&repo, branch)? {
        Ok(()) => {
            say(out, &format!("pr: pushed {branch} to origin"))?;
            Ok(None)
        }
        Err(text) => {
            let body = format!(
                "**Happened:** `git push -u origin {branch}` refused:\n  {text}\n\
                 **Options:**\n\
                 1. fix the remote (login, a diverged branch — never force from here) and answer this block — the push runs again\n\
                 2. push by hand and answer this block — the driver goes on to the PR\n\
                 **Recommended:** 1 — the driver never forces a push.\n"
            );
            Ok(Some(block(
                run,
                feature_dir,
                "pr",
                &format!("git push refused: {}", first_line(&text)),
                &body,
                out,
            )?))
        }
    }
}

fn branch_guard(
    run: &mut Run,
    docs: &Docs,
    feature_dir: &Path,
    phase: &str,
    what: &str,
    out: &mut dyn Write,
) -> Result<Option<Outcome>> {
    let repo = run.state().repo.clone();
    let branch = feature_branch(run, &repo)?;
    let default = default_branch(run, docs);
    let on = ci::head_branch(&repo)?;
    let Some(problem) = ci::branch_problem(&branch, &default, on.as_deref()) else {
        return Ok(None);
    };
    let body = format!(
        "**Happened:** the {what} did not start: {problem}. \
         The driver polishes, ships, pushes and fixes CI only from the run's feature branch, and never pushes `{default}`.\n\
         **Options:**\n\
         1. put HEAD back on the feature branch (`git switch {branch}`, carrying over any commit made elsewhere) and answer this block — the {what} runs again\n\
         2. if the run's branch itself is wrong, `factory stop` the run and start it again from the right branch\n\
         **Recommended:** 1 — the driver never pushes the default branch or a branch the run does not own.\n"
    );
    Ok(Some(block(
        run,
        feature_dir,
        phase,
        &format!("{what} stopped: {problem}"),
        &body,
        out,
    )?))
}

pub fn pr_title(prd: &Path, feature: &str, slug: &str) -> String {
    let text = fs::read_to_string(prd).unwrap_or_default();
    // Frontmatter can hold `# ` comment lines; the H1 is below it.
    let body = text
        .strip_prefix("---\n")
        .and_then(|t| t.find("\n---").map(|end| &t[end + 4..]))
        .unwrap_or(&text);
    let h1 = body
        .lines()
        .find_map(|l| l.strip_prefix("# "))
        .map(str::trim)
        .unwrap_or("");
    let prefix = format!("PRD-{feature}");
    let rest = h1.strip_prefix(&prefix).unwrap_or(h1);
    let title = rest.trim_start_matches([' ', '—', '-', ':', '–']).trim();
    if title.is_empty() {
        slug.to_string()
    } else {
        title.to_string()
    }
}

// ── CI ──────────────────────────────────────────────────────────────────────

enum Next {
    Wait(String),
    Again,
    Fix {
        round: u32,
        check: String,
        log: String,
    },
    Out(Outcome),
}

struct Watch {
    repo: PathBuf,
    feature_dir: PathBuf,
    /// The seq of the last answered `ci` block: the caps count rounds and reruns after it.
    window: u64,
    /// Job links just reran: their rows are the old attempt's until the new one registers.
    stale: HashSet<String>,
}

fn watch_ci(
    run: &mut Run,
    docs: &Docs,
    cfg: &Config,
    board: &Status,
    out: &mut dyn Write,
) -> Result<Option<Outcome>> {
    let repo = run.state().repo.clone();
    let feature_dir = repo.join(&board.dir);
    let Some(url) = run.state().pr_url.clone() else {
        return Ok(None);
    };
    let pr = ci::pr_number(&url);
    let branch = feature_branch(run, &repo)?;
    let done = answered(run, &feature_dir)?;
    if let Some(a) = done
        .iter()
        .find(|a| a.proceed && a.phase == "ci" && ENDS_WATCH.iter().any(|p| a.title.starts_with(p)))
    {
        say(
            out,
            &format!(
                "ci: {} answered `proceed` — the checks are the owner's; the run finishes without another watch",
                a.id
            ),
        )?;
        return Ok(None);
    }
    let mut w = Watch {
        repo: repo.clone(),
        feature_dir: feature_dir.clone(),
        window: done
            .iter()
            .filter(|a| a.phase == "ci")
            .map(|a| a.seq)
            .max()
            .unwrap_or(0),
        stale: HashSet::new(),
    };
    let interval = ci::interval();
    let grace = Duration::from_secs(ci::grace());
    let timeout = Duration::from_secs(ci::timeout());
    let mut since = Instant::now();
    let mut expected: BTreeSet<String> = BTreeSet::new();
    let mut waiting: Option<String> = None;
    say(
        out,
        &format!("ci: watching the checks of {url} (a poll every {interval}s)"),
    )?;
    loop {
        run.reload()?;
        if run.state().stop_requested {
            return Ok(Some(Outcome::Stopped));
        }
        let head = ci::head(&repo)?;
        let in_grace = since.elapsed() < grace;
        let mut rows = Vec::new();
        let verdict = match ci::pr_head(&repo, &pr)? {
            Err(text) => {
                return Ok(Some(unreadable(
                    run,
                    &feature_dir,
                    &format!("gh pr view {pr} --json headRefOid"),
                    VIEW_UNREADABLE,
                    &text,
                    out,
                )?));
            }
            Ok(at) if at != head => {
                if !in_grace {
                    let body = format!(
                        "**Happened:** the PR {url} is at `{}` while this branch's pushed head is `{}`, {} after the push: \
                         its checks are not this branch's.\n\
                         **Options:**\n\
                         1. check what reached origin (`git ls-remote origin {branch}`) and which commit the PR shows, and answer this block — the watch runs again\n\
                         2. push by hand (never forced) and answer this block\n\
                         **Recommended:** 1 — a fix worker on a head the PR does not have changes the wrong tree.\n",
                        short(&at),
                        short(&head),
                        span(since.elapsed().as_secs())
                    );
                    return Ok(Some(block(
                        run,
                        &feature_dir,
                        "ci",
                        &format!(
                            "the PR's head {} is not this branch's head {}",
                            short(&at),
                            short(&head)
                        ),
                        &body,
                        out,
                    )?));
                }
                Verdict::Pending(format!(
                    "the PR is still at {}, the push of {} has not reached it",
                    short(&at),
                    short(&head)
                ))
            }
            Ok(_) => match ci::checks(&repo, &pr)? {
                Checks::Unreadable { text } => {
                    return Ok(Some(unreadable(
                        run,
                        &feature_dir,
                        &format!("gh pr checks {pr} --json {}", ci::CHECK_FIELDS),
                        CHECKS_UNREADABLE,
                        &text,
                        out,
                    )?));
                }
                Checks::None => ci::judge(&[], &w.stale, &expected, in_grace),
                Checks::Rows(r) => {
                    rows = r;
                    ci::judge(&rows, &w.stale, &expected, in_grace)
                }
            },
        };
        let why = match verdict {
            Verdict::Pending(why) => why,
            Verdict::Green => {
                say(out, &format!("ci: green at {}", short(&head)))?;
                run.emit(EventKind::CiGreen { head })?;
                return Ok(None);
            }
            Verdict::NoChecks => {
                say(
                    out,
                    &format!(
                        "ci: no checks reported on the PR {} after the push — nothing to watch",
                        span(since.elapsed().as_secs())
                    ),
                )?;
                return Ok(None);
            }
            Verdict::Infra(bad) => match rerunning(&repo, &bad)? {
                Some(why) => why,
                None => return Ok(Some(infra(run, &feature_dir, &head, &bad, out)?)),
            },
            Verdict::Red(failed) => {
                expected = rows.iter().map(|r| r.name.clone()).collect();
                match on_red(run, &mut w, &head, &rows, &failed, out)? {
                    Next::Wait(why) => why,
                    Next::Out(o) => return Ok(Some(o)),
                    Next::Again => {
                        since = Instant::now();
                        waiting = None;
                        continue;
                    }
                    Next::Fix { round, check, log } => {
                        if let Some(o) = fix_round(run, docs, cfg, board, round, &check, &log, out)?
                        {
                            return Ok(Some(o));
                        }
                        run.reload()?;
                        if run.state().stop_requested {
                            return Ok(Some(Outcome::Stopped));
                        }
                        if let Some(o) = push(run, docs, &feature_dir, &branch, out)? {
                            return Ok(Some(o));
                        }
                        since = Instant::now();
                        w.stale.clear();
                        waiting = None;
                        continue;
                    }
                }
            }
        };
        if since.elapsed() >= timeout {
            let body = format!(
                "**Happened:** the checks of {url} at `{}` are still not settled {} after the push or the rerun: {why}. The last table:\n{}\
                 **Options:**\n\
                 1. look at the runners (a job queued with no runner, a check that never reports) and answer this block — the watch runs again with a fresh timeout\n\
                 2. answer `proceed` to take the checks over — the run finishes without another watch\n\
                 **Recommended:** 1 — the driver waits {} (`{}`) before it calls a check stuck.\n",
                short(&head),
                span(since.elapsed().as_secs()),
                indent(&ci::table(&rows)),
                span(timeout.as_secs()),
                ci::TIMEOUT_ENV
            );
            return Ok(Some(block(
                run,
                &feature_dir,
                "ci",
                &format!("{TIMEOUT} {}", span(timeout.as_secs())),
                &body,
                out,
            )?));
        }
        if waiting.as_deref() != Some(why.as_str()) {
            say(out, &format!("ci: waiting — {why}"))?;
            waiting = Some(why);
        }
        if let Some(o) = nap(run, interval)? {
            return Ok(Some(o));
        }
    }
}

fn on_red(
    run: &mut Run,
    w: &mut Watch,
    head: &str,
    rows: &[CheckRow],
    failed: &[CheckRow],
    out: &mut dyn Write,
) -> Result<Next> {
    let mut runs: Vec<CiRun> = Vec::new();
    for row in failed {
        let Some(id) = ci::run_id(&row.link) else {
            continue;
        };
        if runs.iter().any(|r| r.database_id == id) {
            continue;
        }
        match ci::run_view(&w.repo, id)? {
            Ok(mut r) => {
                r.database_id = id;
                runs.push(r);
            }
            Err(text) => {
                return Ok(Next::Out(gh_refused(
                    run,
                    &w.feature_dir,
                    &format!("gh run view {id} --json {}", ci::RUN_FIELDS),
                    &format!("the run behind the red `{}`", row.name),
                    &text,
                    out,
                )?));
            }
        }
    }
    if let Some(r) = runs.iter().find(|r| r.in_flight()) {
        return Ok(Next::Wait(format!(
            "run {} of `{}` is {} (attempt {})",
            r.database_id,
            r.workflow_name,
            r.status.to_ascii_lowercase(),
            r.attempt
        )));
    }
    let check = ci::check_names(failed);
    let events = run.events()?;
    let mut ever = BTreeSet::new();
    let mut since_answer = BTreeSet::new();
    for e in &events {
        if let EventKind::CiFixSpawned { round, .. } = e.kind {
            ever.insert(round);
            if e.seq > w.window {
                since_answer.insert(round);
            }
        }
    }
    let rounds_done = since_answer.len() as u32;
    let round = ever.last().copied().unwrap_or(0) + 1;
    run.emit(EventKind::CiRed {
        check: check.clone(),
        round: rounds_done,
        head: head.to_string(),
    })?;
    say(
        out,
        &format!(
            "ci: `{check}` red at {} ({rounds_done} fix round(s) since the last answer)",
            short(head)
        ),
    )?;
    if let Some(f) = runs
        .iter()
        .find(|r| !r.head_sha.is_empty() && r.head_sha != head)
    {
        let body = format!(
            "**Happened:** `gh pr checks` reports `{check}` red, but the failed run ({}) is at `{}` while this branch's head is `{}`: \
             the failure is not this branch's to fix.\n\
             **Options:**\n\
             1. look at the run on GitHub; once the branch's own head is checked, answer this block — the watch runs again\n\
             2. rerun the check by hand and answer this block\n\
             **Recommended:** 1 — a fix worker on a head it does not have changes the wrong tree.\n",
            f.database_id,
            short(&f.head_sha),
            short(head)
        );
        return Ok(Next::Out(block(
            run,
            &w.feature_dir,
            "ci",
            &format!("CI red at a head this branch does not have: {check}"),
            &body,
            out,
        )?));
    }
    let mut flaky = !runs.is_empty() && failed.iter().all(|r| ci::run_id(&r.link).is_some());
    if flaky {
        for r in &runs {
            match ci::runs_at(&w.repo, &r.head_sha, &r.workflow_name)? {
                Ok(list) if ci::passed_once(&list, r) => {}
                Ok(_) => {
                    flaky = false;
                    break;
                }
                Err(text) => {
                    return Ok(Next::Out(gh_refused(
                        run,
                        &w.feature_dir,
                        &format!(
                            "gh run list --commit {} --workflow {} …",
                            short(&r.head_sha),
                            r.workflow_name
                        ),
                        &format!("whether `{}` passed once at this head", r.workflow_name),
                        &text,
                        out,
                    )?));
                }
            }
        }
    }
    if flaky {
        let rerun_before = |name: &str| {
            events
                .iter()
                .filter(|e| {
                    e.seq > w.window
                        && matches!(&e.kind, EventKind::CiRerun { check: c, head: h, .. } if c == name && h == head)
                })
                .count() as u32
        };
        let again: Vec<&CheckRow> = failed
            .iter()
            .filter(|r| rerun_before(&r.name) >= ci::CI_FLAKY_RERUNS)
            .collect();
        if !again.is_empty() {
            let ids: Vec<String> = runs.iter().map(|r| r.database_id.to_string()).collect();
            let body = format!(
                "**Happened:** `{check}` is red at `{}` after its one rerun (run {}); another run of the workflow passed at this head, so this is flake the rerun did not clear, not a defect a fix worker can see.\n\
                 **Options:**\n\
                 1. rerun on GitHub until green and answer this block — the watch runs again\n\
                 2. fix the flaky check on a separate branch and answer this block with `proceed` — the run finishes without another watch\n\
                 **Recommended:** 1 — the driver gets one rerun.\n",
                short(head),
                ids.join(", ")
            );
            return Ok(Next::Out(block(
                run,
                &w.feature_dir,
                "ci",
                &format!("{FLAKY}{check} red again after its rerun"),
                &body,
                out,
            )?));
        }
        for r in &runs {
            let mine: Vec<&CheckRow> = failed
                .iter()
                .filter(|row| ci::run_id(&row.link) == Some(r.database_id))
                .collect();
            let names = ci::check_names(&mine.iter().map(|r| (*r).clone()).collect::<Vec<_>>());
            match ci::rerun(&w.repo, r.database_id)? {
                Ok(()) => {
                    say(
                        out,
                        &format!(
                            "ci: `{names}` passed once at this head in another run of `{}` — rerun {}'s failed jobs (flaky)",
                            r.workflow_name, r.database_id
                        ),
                    )?;
                    let mut seen = HashSet::new();
                    for row in &mine {
                        w.stale.insert(row.link.clone());
                        if seen.insert(row.name.clone()) {
                            run.emit(EventKind::CiRerun {
                                check: row.name.clone(),
                                run_id: r.database_id,
                                head: head.to_string(),
                            })?;
                        }
                    }
                }
                Err(text) => {
                    let body = format!(
                        "**Happened:** `{names}` is red at `{}` and another run of `{}` passed at this head (flaky), but `gh run rerun {} --failed` refused:\n  {text}\n\
                         **Options:**\n\
                         1. fix `gh` (login, permissions) and answer this block — the watch runs again and so does the one rerun\n\
                         2. rerun the failed jobs on GitHub and answer this block — the watch waits for them\n\
                         **Recommended:** 1 — a refused rerun is not a defect a fix worker can see.\n",
                        short(head),
                        r.workflow_name,
                        r.database_id
                    );
                    return Ok(Next::Out(block(
                        run,
                        &w.feature_dir,
                        "ci",
                        &format!(
                            "gh run rerun {} refused: {}",
                            r.database_id,
                            first_line(&text)
                        ),
                        &body,
                        out,
                    )?));
                }
            }
        }
        return Ok(Next::Again);
    }
    if rounds_done >= ci::CI_FIX_ROUNDS {
        let body = format!(
            "**Happened:** `{check}` is still red at `{}` after {rounds_done} fix rounds; the driver's cap is {}. \
             The fix commits are on the branch; the last check table:\n{}\
             **Options:**\n\
             1. fix it by hand, push, and answer this block — the watch runs again, with {} fresh fix rounds if it is still red\n\
             2. revert the fix commits and answer this block once the cause is understood — the same\n\
             **Recommended:** 1 — three workers saw the same red; the cause is outside what the log names.\n",
            short(head),
            ci::CI_FIX_ROUNDS,
            indent(&ci::table(rows)),
            ci::CI_FIX_ROUNDS
        );
        return Ok(Next::Out(block(
            run,
            &w.feature_dir,
            "ci",
            &format!("CI red after {rounds_done} fix rounds: {check}"),
            &body,
            out,
        )?));
    }
    let mut log = String::new();
    for r in &runs {
        let mine: Vec<CheckRow> = failed
            .iter()
            .filter(|row| ci::run_id(&row.link) == Some(r.database_id))
            .cloned()
            .collect();
        log.push_str(&format!(
            "--- run {} (`{}`), failed: {}\n",
            r.database_id,
            r.workflow_name,
            ci::check_names(&mine)
        ));
        let text = ci::log_failed(&w.repo, r.database_id)?;
        log.push_str(&text);
        if !text.ends_with('\n') {
            log.push('\n');
        }
    }
    let unlinked: Vec<CheckRow> = failed
        .iter()
        .filter(|r| ci::run_id(&r.link).is_none())
        .cloned()
        .collect();
    if !unlinked.is_empty() {
        log.push_str("--- checks with no Actions run, so no log to fetch:\n");
        log.push_str(&ci::table(&unlinked));
    }
    Ok(Next::Fix { round, check, log })
}

/// A run `gh` will not show is no reason to wait: the block names the rows.
fn rerunning(repo: &Path, rows: &[CheckRow]) -> Result<Option<String>> {
    let mut seen = HashSet::new();
    for row in rows {
        let Some(id) = ci::run_id(&row.link) else {
            continue;
        };
        if !seen.insert(id) {
            continue;
        }
        if let Ok(r) = ci::run_view(repo, id)?
            && r.in_flight()
        {
            return Ok(Some(format!(
                "run {id} of `{}` is {} (attempt {})",
                r.workflow_name,
                r.status.to_ascii_lowercase(),
                r.attempt
            )));
        }
    }
    Ok(None)
}

fn infra(
    run: &mut Run,
    feature_dir: &Path,
    head: &str,
    rows: &[CheckRow],
    out: &mut dyn Write,
) -> Result<Outcome> {
    let names = ci::check_names(rows);
    let body = format!(
        "**Happened:** at `{}` these checks never reached a verdict on the code:\n{}\
         A timeout, a runner that never started or a cancelled job is infrastructure, not a defect a fix worker can see.\n\
         **Options:**\n\
         1. fix the runner, the secret or the workflow, rerun the job on GitHub and answer this block — the watch runs again and waits for the rerun\n\
         2. answer `proceed` to take the checks over — the run finishes without another watch\n\
         **Recommended:** 1.\n",
        short(head),
        indent(&ci::table(rows))
    );
    block(
        run,
        feature_dir,
        "ci",
        &format!("{INFRA}: {names}"),
        &body,
        out,
    )
}

fn unreadable(
    run: &mut Run,
    feature_dir: &Path,
    command: &str,
    title: &str,
    text: &str,
    out: &mut dyn Write,
) -> Result<Outcome> {
    let body = format!(
        "**Happened:** `{command}` printed neither the PR nor its checks:\n  {text}\n\
         **Options:**\n\
         1. fix `gh` (login, network) and answer this block — the watch runs again\n\
         2. answer `proceed` once the checks are green on GitHub — the run finishes without another watch\n\
         **Recommended:** 1.\n"
    );
    block(
        run,
        feature_dir,
        "ci",
        &format!("{title}: {}", first_line(text)),
        &body,
        out,
    )
}

fn gh_refused(
    run: &mut Run,
    feature_dir: &Path,
    command: &str,
    needed: &str,
    text: &str,
    out: &mut dyn Write,
) -> Result<Outcome> {
    let body = format!(
        "**Happened:** the driver needs {needed} and `{command}` refused:\n  {text}\n\
         **Options:**\n\
         1. fix `gh` (login, permissions, network) and answer this block — the watch runs again\n\
         2. fix the check by hand, push, and answer this block\n\
         **Recommended:** 1 — without it a red is neither a rerun nor a fix round the driver can justify.\n"
    );
    let verb = command.split(" --").next().unwrap_or(command);
    block(
        run,
        feature_dir,
        "ci",
        &format!("{verb} refused: {}", first_line(text)),
        &body,
        out,
    )
}

fn nap(run: &mut Run, secs: u64) -> Result<Option<Outcome>> {
    for _ in 0..secs.max(1) {
        std::thread::sleep(Duration::from_secs(1));
        run.reload()?;
        if run.state().stop_requested {
            return Ok(Some(Outcome::Stopped));
        }
    }
    Ok(None)
}

fn span(secs: u64) -> String {
    if secs < 120 {
        format!("{secs}s")
    } else {
        format!("{} min", secs / 60)
    }
}

fn indent(text: &str) -> String {
    text.lines().map(|l| format!("  {l}\n")).collect()
}

#[allow(clippy::too_many_arguments)]
fn fix_round(
    run: &mut Run,
    docs: &Docs,
    cfg: &Config,
    board: &Status,
    round: u32,
    check: &str,
    log: &str,
    out: &mut dyn Write,
) -> Result<Option<Outcome>> {
    let repo = run.state().repo.clone();
    let feature_dir = repo.join(&board.dir);
    // HEAD may have moved during the watch: guard before a worker spends the round elsewhere.
    if let Some(o) = branch_guard(run, docs, &feature_dir, "ci", "CI fix", out)? {
        return Ok(Some(o));
    }
    loop {
        if let Some(o) = await_quota(run, out)? {
            return Ok(Some(o));
        }
        let sessions: HashSet<String> = run
            .events()?
            .iter()
            .filter_map(|e| match &e.kind {
                EventKind::CiFixSpawned {
                    round: r,
                    session_id,
                    ..
                } if *r == round => Some(session_id.clone()),
                _ => None,
            })
            .collect();
        let mine = |w: &WorkerRef| sessions.contains(&w.session_id);
        let deaths = run
            .state()
            .workers
            .iter()
            .filter(|w| mine(w) && w.died())
            .count();
        if deaths >= DEATH_CAP {
            let body = format!(
                "**Happened:** {deaths} CI fix workers of round {round} exited without a result (check `{check}`); the driver's once-only respawn is spent. \
                 Streams and stderr are under `{}/`.\n\
                 **Options:**\n\
                 1. read the last worker's `.stderr` and `.jsonl`, fix the environment and answer this block — the watch runs again and a fresh round follows\n\
                 2. fix the check by hand, push, and answer this block\n\
                 **Recommended:** 1.\n",
                worker::dir(run.dir()).display()
            );
            return Ok(Some(block(
                run,
                &feature_dir,
                "ci",
                &format!("Two CI fix workers died in round {round}"),
                &body,
                out,
            )?));
        }
        let attempt = run.state().workers.iter().filter(|w| mine(w)).count() as u32 + 1;
        let spawn = run.state().workers.len() as u32 + 1;
        let packs = scheduler::packs_dir(run.dir());
        let pack = Pack::for_ci_fix(docs, board, round, attempt, check, log, &packs, spawn)?;
        let open_before: HashSet<String> = open_blocks(&feature_dir)?
            .into_iter()
            .map(|b| b.id)
            .collect();
        let lane = closeout_lane(board);
        let (lane_dir, lane_def) = scheduler::prepare_lane(run, lane, true, out)?;
        let who = Closer::CiFix { round, check };
        drop_ref(run, board, &who)?;
        let w = Worker::spawn(
            run,
            Job::CiFix { round, attempt },
            &pack,
            &cfg.workers.fix,
            &lane_def.env(),
            &lane_dir,
            Some(lane),
        )?;
        let session_id = w.session_id.clone();
        let row_id = w.row_id;
        say(
            out,
            &format!(
                "ci: fix worker {session_id} spawned for `{check}` in lane {lane} (round {round}, attempt {attempt}, pid {}, pack {})",
                w.pid,
                pack.path.display()
            ),
        )?;
        let exit = w.wait()?;
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
                    "ci: fix worker {session_id} hit a usage limit ({}) — paused until {}, the round runs again",
                    limit.literal,
                    clock::rfc3339(until)
                ),
            )?;
            continue;
        }
        if exit.summary.is_dead() {
            settle_dead(run, &feature_dir, "CI fix", &session_id, row_id, &exit, out)?;
            continue;
        }
        if let Some(b) = open_blocks(&feature_dir)?
            .into_iter()
            .find(|b| !open_before.contains(&b.id))
        {
            settle(
                run,
                row_id,
                &session_id,
                &exit.summary,
                WorkerOutcome::Blocked,
            )?;
            run.emit(EventKind::CloseoutBlocked {
                phase: b.phase.clone(),
                block_id: b.id.clone(),
                question: b.title.clone(),
            })?;
            say(
                out,
                &format!(
                    "ci: fix worker {session_id} blocked — {}: {}",
                    b.id, b.title
                ),
            )?;
            return Ok(Some(Outcome::Parked {
                what: park_line(&b.id, &b.title, &b.phase),
                id: b.id,
                fresh: true,
            }));
        }
        // Land before settling: a driver killed between the two leaves the worker to the resume, which lands it.
        if let Err(o) = land_here(run, board, &who, lane, row_id, out)? {
            if matches!(o, Outcome::Parked { .. }) {
                settle(
                    run,
                    row_id,
                    &session_id,
                    &exit.summary,
                    WorkerOutcome::Blocked,
                )?;
            }
            return Ok(Some(o));
        }
        settle(run, row_id, &session_id, &exit.summary, WorkerOutcome::Done)?;
        return Ok(None);
    }
}

// ── orphans ─────────────────────────────────────────────────────────────────

/// `Some`: the landing waits on the owner's dirty main tree; the worker stays unsettled until the next tick.
pub fn settle_orphan(
    run: &mut Run,
    board: &Status,
    w: &WorkerRef,
    summary: Summary,
    out: &mut dyn Write,
) -> Result<Option<String>> {
    let what = if w.role == "polish" {
        "polish"
    } else {
        "CI fix"
    };
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
                "closeout: {what} worker {} of an earlier driver hit a usage limit ({}) — paused until {}",
                w.session_id,
                limit.literal,
                clock::rfc3339(until)
            ),
        )?;
        return Ok(None);
    }
    let Some(lane) = w.lane.filter(|_| !summary.is_dead()) else {
        settle(run, w.row_id, &w.session_id, &summary, WorkerOutcome::Dead)?;
        run.emit(EventKind::WorkerDead {
            slice: None,
            session_id: w.session_id.clone(),
            exit_code: None,
        })?;
        say(
            out,
            &format!(
                "closeout: {what} worker {} of an earlier driver is gone ({}) — the closeout runs again",
                w.session_id,
                if summary.is_dead() {
                    "dead"
                } else {
                    "spawned before lanes"
                }
            ),
        )?;
        return Ok(None);
    };
    let events = run.events()?;
    let round = events
        .iter()
        .find_map(|e| match &e.kind {
            EventKind::CiFixSpawned {
                round, session_id, ..
            } if *session_id == w.session_id => Some(*round),
            _ => None,
        })
        .unwrap_or(1);
    let check = events
        .iter()
        .rev()
        .find_map(|e| match &e.kind {
            EventKind::CiRed { check, .. } => Some(check.clone()),
            _ => None,
        })
        .unwrap_or_else(|| "ci".to_string());
    let who = if w.role == "polish" {
        Closer::Polish
    } else {
        Closer::CiFix {
            round,
            check: &check,
        }
    };
    // Said once: a landing a dirty main tree holds is retried every tick.
    if scheduler::resolve(&run.state().repo, &who.ref_name(board))?.is_none() {
        say(
            out,
            &format!(
                "closeout: {what} worker {} of an earlier driver returned — landing its lane",
                w.session_id
            ),
        )?;
    }
    match land_worker(run, board, &who, lane, w.row_id, out)? {
        Landed::Done(_) => {}
        Landed::Waiting(what) => return Ok(Some(what)),
        Landed::Conflict { paths, name } => {
            block_conflict(run, board, &who, &paths, &name, out)?;
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
    settle(run, w.row_id, &w.session_id, &summary, WorkerOutcome::Done)?;
    Ok(None)
}

// ── shared ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenBlock {
    pub id: String,
    pub title: String,
    pub phase: String,
}

pub fn open_blocks(feature_dir: &Path) -> Result<Vec<OpenBlock>> {
    Ok(answers::entries(feature_dir)?
        .into_iter()
        .filter(|e| !e.resolved && e.phase.as_deref().is_some_and(|p| PHASES.contains(&p)))
        .map(|e| OpenBlock {
            id: e.id,
            title: e.title,
            phase: e.phase.unwrap_or_default(),
        })
        .collect())
}

fn block(
    run: &mut Run,
    feature_dir: &Path,
    phase: &str,
    title: &str,
    body: &str,
    out: &mut dyn Write,
) -> Result<Outcome> {
    // Titles carry tool output; `log show` renders them in a heading.
    let title = answers::block_title(title);
    let id = answers::add_block(feature_dir, None, Some(phase), &title, body)?;
    run.emit(EventKind::CloseoutBlocked {
        phase: phase.to_string(),
        block_id: id.clone(),
        question: title.clone(),
    })?;
    say(
        out,
        &format!("closeout ({phase}) blocked — {id} written to log.jsonl: {title}"),
    )?;
    Ok(Outcome::Parked {
        what: park_line(&id, &title, phase),
        id,
        fresh: true,
    })
}

fn park_line(id: &str, title: &str, phase: &str) -> String {
    format!(
        "closeout blocked ({phase}) — {title}: {id} in log.jsonl waits for the owner: `factory answer {id} \"<text>\"`"
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Answered {
    seq: u64,
    id: String,
    phase: String,
    title: String,
    proceed: bool,
}

fn answered(run: &Run, feature_dir: &Path) -> Result<Vec<Answered>> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for e in run.events()? {
        let EventKind::CloseoutBlocked {
            phase,
            block_id,
            question,
        } = e.kind
        else {
            continue;
        };
        if !seen.insert(block_id.clone()) {
            continue;
        }
        let Some(entry) = answers::find(feature_dir, &block_id)? else {
            continue;
        };
        if !entry.resolved {
            continue;
        }
        let text = resolution(&answers::entry_text(&entry)).unwrap_or_default();
        out.push(Answered {
            seq: e.seq,
            id: block_id,
            phase,
            title: question,
            proceed: is_proceed(&text),
        });
    }
    Ok(out)
}

fn resolution(entry: &str) -> Option<String> {
    entry.lines().rev().find_map(|l| {
        let rest = l.trim().strip_prefix("**Resolution (")?;
        let (_, text) = rest.split_once("):**")?;
        Some(text.trim().to_string())
    })
}

fn is_proceed(text: &str) -> bool {
    let t = text
        .trim()
        .trim_start_matches(|c: char| c.is_ascii_digit())
        .trim_start_matches(|c: char| !c.is_alphanumeric());
    let word: String = t.chars().take_while(|c| c.is_alphanumeric()).collect();
    word.eq_ignore_ascii_case("proceed")
}

fn shipped_at_head(repo: &Path, dir: &str) -> Result<bool> {
    let out = ci::git(repo, &["show", &format!("HEAD:{dir}/state.toml")])?;
    Ok(out.ok() && shipped_in(&out.stdout))
}

fn shipped_in(state_toml: &str) -> bool {
    crate::docs::state::parse(state_toml).is_ok_and(|s| s.status == "shipped")
}

/// Deaths before the latest answer do not count: the owner reset the cap.
fn last_answer(run: &Run) -> Result<Option<String>> {
    Ok(answers::all(run.dir())?
        .into_iter()
        .filter(|a| a.slice.is_none() && a.id.starts_with("BLOCK-"))
        .map(|a| a.at)
        .max())
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
                    quota::resume(run, Phase::Closeout, out)?;
                }
                return Ok(None);
            }
        }
    }
}

fn settle_dead(
    run: &mut Run,
    feature_dir: &Path,
    what: &str,
    session_id: &str,
    row_id: i64,
    exit: &Exit,
    out: &mut dyn Write,
) -> Result<()> {
    settle(run, row_id, session_id, &exit.summary, WorkerOutcome::Dead)?;
    run.emit(EventKind::WorkerDead {
        slice: None,
        session_id: session_id.to_string(),
        exit_code: exit.code,
    })?;
    let code = exit.code.map_or("exited (code unknown)".to_string(), |c| {
        format!("exited {c}")
    });
    answers::add_note(
        feature_dir,
        None,
        &format!("Dead {what} worker"),
        &format!("session {session_id} {code} with no result."),
    )?;
    say(
        out,
        &format!("closeout: {what} worker {session_id} died ({code}) — once more, then a block"),
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

pub(crate) fn default_branch(run: &Run, docs: &Docs) -> String {
    load_preflight(run.dir())
        .ok()
        .flatten()
        .and_then(|p| p.branch.default)
        .or_else(|| {
            docs.preflight(&run.state().feature)
                .ok()
                .and_then(|(p, _)| p.branch.default)
        })
        .unwrap_or_else(|| "main".to_string())
}

fn feature_branch(run: &Run, repo: &Path) -> Result<String> {
    if let Some(b) = run.state().branch.clone() {
        return Ok(b);
    }
    let out = ci::git(repo, &["rev-parse", "--abbrev-ref", "HEAD"])?;
    Ok(out.stdout.trim().to_string())
}

pub fn retro_template() -> String {
    crate::pack::RETRO.to_string()
}

fn first_line(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("no output")
        .to_string()
}

pub fn pr_body_path(run_dir: &Path) -> PathBuf {
    run_dir.join(PR_BODY_FILE)
}

pub fn collect_path(run_dir: &Path) -> PathBuf {
    run_dir.join(COLLECT_FILE)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stand_in(what: &str, path: &str) -> crate::events::Event {
        crate::events::Event {
            seq: 1,
            ts: "2026-10-11T00:00:00Z".to_string(),
            kind: EventKind::TemplateStandIn {
                what: what.into(),
                path: path.into(),
            },
        }
    }

    #[test]
    fn the_stand_in_line_names_the_retro_the_body_or_both() {
        let retro = stand_in(STAND_IN_RETRO, "docs/features/001-x/retro.md");
        let body = stand_in(STAND_IN_BODY, "/runs/r/pr-body.md");
        assert_eq!(stand_in_line(&[]), None);
        assert_eq!(
            stand_in_line(std::slice::from_ref(&retro)).as_deref(),
            Some(
                "**Template stand-in:** the retro (`docs/features/001-x/retro.md`) is factory's template — the polish worker wrote none."
            )
        );
        assert_eq!(
            stand_in_line(std::slice::from_ref(&body)).as_deref(),
            Some(
                "**Template stand-in:** this PR body is factory's template — the polish worker wrote none."
            )
        );
        assert_eq!(
            stand_in_line(&[body, retro]).as_deref(),
            Some(
                "**Template stand-ins:** this PR body and the retro (`docs/features/001-x/retro.md`) are factory's templates — the polish worker wrote neither."
            )
        );
    }

    #[test]
    fn a_flagged_body_keeps_one_stand_in_line() {
        let worker = "## PRD-001 — x\n\nBody.\n";
        let once = flagged(worker, "**Template stand-in:** a");
        assert_eq!(
            once,
            "**Template stand-in:** a\n\n## PRD-001 — x\n\nBody.\n"
        );
        assert_eq!(flagged(&once, "**Template stand-in:** a"), once);
        assert_eq!(
            flagged(&once, "**Template stand-ins:** b"),
            "**Template stand-ins:** b\n\n## PRD-001 — x\n\nBody.\n"
        );
    }

    #[test]
    fn a_block_title_from_tool_output_keeps_the_heading_parseable() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("repo");
        let feature_dir = repo.join("docs/features/001-x");
        fs::create_dir_all(&feature_dir).unwrap();
        let mut run = Run::create(&tmp.path().join("data"), &repo, "001", None, 1).unwrap();
        let mut out = Vec::new();
        block(
            &mut run,
            &feature_dir,
            "ci",
            "flaky check test · linux\nsecond line",
            "**Happened:** x\n",
            &mut out,
        )
        .unwrap();
        let open = open_blocks(&feature_dir).unwrap();
        assert_eq!(open.len(), 1, "{open:?}");
        assert_eq!(open[0].phase, "ci");
        assert_eq!(open[0].title, "flaky check test - linux second line");
        let question = run
            .events()
            .unwrap()
            .into_iter()
            .find_map(|e| match e.kind {
                EventKind::CloseoutBlocked { question, .. } => Some(question),
                _ => None,
            });
        assert_eq!(question.as_deref(), Some(open[0].title.as_str()));
    }

    #[test]
    fn the_pr_title_drops_the_prd_prefix() {
        let dir = tempfile::tempdir().unwrap();
        let prd = dir.path().join("prd.md");
        fs::write(
            &prd,
            "---\n# type: feature | bugfix\ntitle: Fixture\n---\n\n# PRD-001 — Fixture\n\ntext\n",
        )
        .unwrap();
        assert_eq!(pr_title(&prd, "001", "fixture"), "Fixture");
        fs::write(&prd, "# PRD-001: The driver\n").unwrap();
        assert_eq!(pr_title(&prd, "001", "driver"), "The driver");
        fs::write(&prd, "no heading\n").unwrap();
        assert_eq!(pr_title(&prd, "001", "driver"), "driver");
    }

    #[test]
    fn a_proceed_answer_is_its_first_word() {
        for yes in [
            "proceed",
            "Proceed.",
            "`proceed`",
            "2. proceed",
            "2) proceed — I opened the PR by hand",
            "  PROCEED",
        ] {
            assert!(is_proceed(yes), "{yes}");
        }
        for no in [
            "",
            "2",
            "rerun it once more",
            "do not proceed",
            "proceeding later",
            "1. fixed gh, proceed",
        ] {
            assert!(!is_proceed(no), "{no}");
        }
    }

    #[test]
    fn the_answer_is_the_last_resolution_line_of_the_entry() {
        let entry = "### BLOCK-2 — gh pr create refused: x · pr · resolved\n\n**Happened:** y\n**Recommended:** 1.\n**Resolution (2026-10-10):** proceed";
        assert_eq!(resolution(entry).as_deref(), Some("proceed"));
        assert_eq!(
            resolution("### BLOCK-2 — t · pr · open\n\n**Happened:** y"),
            None
        );
    }

    #[test]
    fn the_state_status_is_read_from_the_toml() {
        let state = "status = \"shipped\"\nreviewed = \"2026-10-10\"\nshipped = \"2026-10-11\"\n";
        assert!(shipped_in(state));
        assert!(!shipped_in("status = \"in-progress\"\n"));
        assert!(!shipped_in("# no state\nstatus = shipped\n"));
    }

    #[test]
    fn progress_lines_are_stamped_like_the_driver_log_whole_lines_only() {
        let mut log = Vec::new();
        {
            let mut w = Stamped::new(&mut log);
            write!(w, "review pass 1/2: a.rs").unwrap();
            writeln!(w, " (10 lines)").unwrap();
            writeln!(w, "review pass 2/2: b.rs (5 lines)").unwrap();
        }
        let log = String::from_utf8(log).unwrap();
        let lines: Vec<&str> = log.lines().collect();
        assert_eq!(lines.len(), 2, "{log}");
        for (line, want) in lines.iter().zip([
            " review pass 1/2: a.rs (10 lines)",
            " review pass 2/2: b.rs (5 lines)",
        ]) {
            let (stamp, rest) = line.split_at(line.find(' ').unwrap());
            assert!(stamp.starts_with(&clock::now_rfc3339()[..4]), "{line}");
            assert!(stamp.contains('T'), "{line}");
            assert_eq!(rest, want);
        }
    }

    fn repo_on_main(dir: &Path) -> String {
        let git = |args: &[&str]| {
            let o = std::process::Command::new("git")
                .args(args)
                .current_dir(dir)
                .output()
                .unwrap();
            assert!(o.status.success(), "{args:?}: {o:?}");
            String::from_utf8(o.stdout).unwrap()
        };
        git(&["init", "-q", "-b", "main"]);
        fs::write(dir.join("a.txt"), "a\n").unwrap();
        git(&["add", "a.txt"]);
        git(&[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "commit",
            "-qm",
            "a",
        ]);
        git(&["rev-parse", "HEAD"]).trim().to_string()
    }

    #[test]
    fn a_file_at_the_same_head_is_reused_unless_it_owes_a_rerun() {
        let tmp = tempfile::tempdir().unwrap();
        let lane = tmp.path().join("lane");
        fs::create_dir_all(&lane).unwrap();
        let head = repo_on_main(&lane);
        let file = tmp.path().join(COLLECT_FILE);
        let cfg = config::Review::default();
        for (plan, reused) in [
            (
                "second opinion: reviewed, 1 of 1 passes counted\n\n## Leaks\n\nverdict: clean\n",
                true,
            ),
            (
                "second opinion: fallback owed — unreviewed: logged out\n\n## Leaks\n\nverdict: clean\n",
                false,
            ),
            (
                "second opinion: reviewed, 1 of 1 passes counted\n\n## Leaks\n\nverdict: did not run — x\n",
                false,
            ),
        ] {
            fs::write(&file, format!("<!-- head: {head} -->\n\n## Plan\n\n{plan}")).unwrap();
            let mut out = Vec::new();
            collected(&lane, "main", &cfg, &file, &mut out).unwrap();
            let log = String::from_utf8(out).unwrap();
            assert_eq!(log.contains("polish: reused "), reused, "{plan}\n{log}");
            assert_eq!(log.contains("polish: collected "), !reused, "{plan}\n{log}");
        }
        assert!(
            fs::read_to_string(&file)
                .unwrap()
                .contains("skipped by size")
        );
    }

    #[test]
    fn a_collect_that_fails_is_the_section_and_writes_no_file() {
        let tmp = tempfile::tempdir().unwrap();
        let not_a_repo = tmp.path().join("lane");
        fs::create_dir_all(&not_a_repo).unwrap();
        let file = tmp.path().join(COLLECT_FILE);
        let mut out = Vec::new();
        let body = collected(
            &not_a_repo,
            "main",
            &config::Review::default(),
            &file,
            &mut out,
        )
        .unwrap();
        assert!(body.starts_with("collect failed: "), "{body}");
        assert!(!body.contains('\n'), "{body}");
        assert!(!file.exists());
        let log = String::from_utf8(out).unwrap();
        assert!(log.contains("polish: collect failed in "), "{log}");
    }

    #[test]
    fn the_retro_template_has_no_key_list() {
        let t = retro_template();
        assert!(t.starts_with("# Retro — PRD-{NNN}\n"), "{t}");
        let head = &t[..t.find("\n#").unwrap_or(0)];
        assert!(
            !head.lines().any(|l| l.trim_end().ends_with(':')),
            "a key line before the first heading: {head}"
        );
        for gone in ["score:", "measure.py", "**Frontmatter**"] {
            assert!(!t.contains(gone), "{gone}");
        }
        for want in ["## Cost vs catch", "## Where I guessed", "## Score"] {
            assert!(t.contains(want), "{want}");
        }
    }
}
