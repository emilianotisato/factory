//! The closeout (PRD §8 FR-6): when the last checkpoint is done — every
//! slice `done`, nothing unverified — the run goes polish → ship → push and
//! draft PR → CI → finished, or parks with a `BLOCK-n` that says why.
//!
//! Polish is one worker (`/polish` Unattended) that writes the retro prose
//! and the PR body in its last turn; the driver commits what it wrote as
//! `fix(NNN): polish review fixes`. Ship is `docs ship NNN`, the twelve numeric retro
//! keys from the store (`factory report`'s rows, FR-8), the `## Numbers`
//! section (PRD-003 FR-25, [`crate::retro`]), `render --write` and
//! `docs(NNN): ship PRD-NNN`. The PR is `gh pr create --draft --head <branch>`; a
//! resume after `pr_opened` edits it instead. Ship and push happen only from
//! the run's feature branch, never the default one. CI is bounded polls of
//! the PR's checks at the pushed head, three fix rounds and one flaky rerun
//! as `autonomy.md:77-79` prescribe. Nothing here merges or marks the PR
//! ready — the owner does.
//!
//! Every step is idempotent on re-entry: the events say what landed
//! (`polish_done`, `shipped`), `state.json` keeps the PR URL, and an open
//! closeout block (`· polish ·`, `· pr ·`, `· ci ·`) keeps the park until the
//! owner answers.
use std::collections::{BTreeSet, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::answers;
use crate::checkpoint::{Outcome, lowest_free_block};
use crate::ci::{self, CheckRow, Checks, CiRun, Verdict};
use crate::cli::report::{self, Report};
use crate::cli::run::load_preflight;
use crate::commit::{self, Pathspec};
use crate::config::Config;
use crate::docs::{Docs, Status};
use crate::error::io;
use crate::events::EventKind;
use crate::pack::Pack;
use crate::quota::{self, Hold, Limit};
use crate::run::{Phase, Run, WorkerOutcome, WorkerRef};
use crate::scheduler::{self, DEATH_CAP, append_under, say, set_frontmatter, short};
use crate::stream::{Findings, Summary};
use crate::worker::{self, Exit, Job, Worker};
use crate::{Result, clock, metrics, retro};

/// The phases a closeout block names in its heading (autonomy § Blocker
/// protocol step 2).
pub const PHASES: [&str; 3] = ["polish", "pr", "ci"];

/// The retro keys the driver fills (`retro.md:3-14`); `score` is the owner's.
pub const RETRO_KEYS: [&str; 12] = [
    "gate_runs",
    "gate_red",
    "gate_minutes",
    "ci_cycles",
    "slices",
    "checkpoints",
    "build_minutes",
    "polish_minutes",
    "out_tokens",
    "cache_read_tokens",
    "start_context",
    "peak_context",
];

/// The PR body the polish worker writes, in the run dir.
pub const PR_BODY_FILE: &str = "pr-body.md";

/// The `what` of a `template_stand_in` (AD-904): the retro or the PR body.
pub const STAND_IN_RETRO: &str = "retro";
pub const STAND_IN_BODY: &str = "pr_body";
/// How the PR body's stand-in line starts, so a later pass replaces it.
const STAND_IN_MARK: &str = "**Template stand-in";

// The closeout blocks whose `proceed` answer the driver reads, by title.
const POLISH_DIED: &str = "Two polish workers died";
const PR_EDIT: &str = "gh pr edit refused";
const PR_CREATE: &str = "gh pr create refused";
const CHECKS_UNREADABLE: &str = "gh pr checks unreadable";
const VIEW_UNREADABLE: &str = "gh pr view unreadable";
const FLAKY: &str = "flaky check ";
const INFRA: &str = "CI did not run to a verdict";
const TIMEOUT: &str = "CI still pending after";
/// A `proceed` to one of these ends the watch: the checks are the owner's.
const ENDS_WATCH: [&str; 5] = [CHECKS_UNREADABLE, VIEW_UNREADABLE, FLAKY, INFRA, TIMEOUT];

/// The closeout from wherever it stands. `Done` is CI green (or no CI): the
/// scheduler finishes the run. `Parked` is a block for the owner.
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

    // A closeout block the owner has not answered: the park stands.
    if let Some(open) = open_blocks(&feature_dir)?.into_iter().next() {
        return Ok(Outcome::Parked {
            what: park_line(&open.id, &open.title, &open.phase),
            id: open.id,
            fresh: false,
        });
    }
    // A second check: `docs new` creates agent-log.md now (AD-503), but a
    // feature made by a vendored docs.mjs older than that fix has none.
    if !agent_log.exists() {
        fs::write(&agent_log, format!("# Agent log — PRD-{feature}\n"))
            .map_err(io(format!("write {}", agent_log.display())))?;
    }

    // A PRD `shipped` on the board has had its polish: `docs ship` runs only
    // after it. Its ship is done only once HEAD holds it — a driver killed
    // between `docs ship` and `docs(NNN): ship PRD-NNN` left it staged, and the
    // ship runs again (`docs ship` on a shipped PRD is a no-op). A new run on
    // a delivered feature finds both and resumes at the PR.
    let board_shipped = board.status.as_deref() == Some("shipped");
    let ship_committed = board_shipped && shipped_at_head(&repo, &board.dir)?;
    let events = run.events()?;
    let polish_by_hand = answered(run, &feature_dir)?
        .iter()
        .any(|a| a.proceed && a.title == POLISH_DIED);
    let polished = board_shipped
        || polish_by_hand
        || events
            .iter()
            .any(|e| matches!(e.kind, EventKind::PolishDone { .. }));
    let ship_event = events
        .iter()
        .any(|e| matches!(e.kind, EventKind::Shipped { .. }));
    let shipped = ship_committed || ship_event;
    // The closeout owns a HEAD off the run's branch (the driver's tick
    // leaves it here, AD-50001): it blocks before the next step runs.
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

/// The polish worker: once more after a death, then the block (phase
/// `polish`). `None` when its commit landed (or it changed nothing).
fn polish(
    run: &mut Run,
    docs: &Docs,
    cfg: &Config,
    board: &Status,
    out: &mut dyn Write,
) -> Result<Option<Outcome>> {
    let repo = run.state().repo.clone();
    let feature_dir = repo.join(&board.dir);
    let agent_log = feature_dir.join("agent-log.md");
    let default_branch = default_branch(run, docs);
    let feature_branch = feature_branch(run, &repo)?;
    let template = retro_template(docs, board);
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
                 2. run `/polish` by hand, write `retro.md` and `runs/<id>/pr-body.md`, set `reviewed:` in `prd.md`, commit, and answer this block with `proceed` — the driver ships from there, no polish worker\n\
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
        let packs = scheduler::packs_dir(run.dir());
        let pack = Pack::for_polish(
            docs,
            board,
            run.dir(),
            &default_branch,
            &feature_branch,
            &template,
            &packs,
            spawn,
        )?;
        let open_before: HashSet<String> = open_blocks(&feature_dir)?
            .into_iter()
            .map(|b| b.id)
            .collect();
        let before = scheduler::snapshot(run, POLISH_KEY)?;
        let w = Worker::spawn(
            run,
            Job::Polish { attempt },
            &pack,
            &cfg.workers.polish,
            &[],
        )?;
        let session_id = w.session_id.clone();
        let row_id = w.row_id;
        say(
            out,
            &format!(
                "polish: worker {session_id} spawned (pid {}, attempt {attempt}, pack {})",
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
                    "polish: worker {session_id} blocked — {} in agent-log.md: {}",
                    b.id, b.title
                ),
            )?;
            return Ok(Some(Outcome::Parked {
                what: park_line(&b.id, &b.title, &b.phase),
                id: b.id,
                fresh: true,
            }));
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
                    "polish: worker {session_id} hit a usage limit ({}) — paused until {}, a fresh worker follows",
                    limit.literal,
                    clock::rfc3339(until)
                ),
            )?;
            continue;
        }
        if exit.summary.is_dead() {
            settle_dead(run, &agent_log, "polish", &session_id, row_id, &exit, out)?;
            continue;
        }
        settle(run, row_id, &session_id, &exit.summary, WorkerOutcome::Done)?;
        // FR-24: only a return block has a `FINDINGS` line to miss.
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
        let committed = commit_polish(run, board, row_id, &exit.summary, &template, &before, out)?;
        run.emit(EventKind::PolishDone {
            sha: committed.as_ref().map(|c| c.sha.clone()),
            files: committed.map(|c| c.files).unwrap_or_default(),
        })?;
        return Ok(None);
    }
}

/// `fix(NNN): polish review fixes`: the paths the stream named plus `prd.md`
/// (`reviewed:`) and `retro.md` — the polish worker is the one worker allowed
/// to write those two; `docs/index.md` stays the ship's. A missing `retro.md`
/// is created from the template first, a missing PR body from a fallback, so
/// the ship and the PR never wait on a worker that skipped its last turn —
/// but each stand-in is flagged (AD-904): a `template_stand_in` event the
/// cockpit relays, and a first line in the PR body naming it.
/// What it changed unnamed since `before` is swept or listed (AD-50002).
fn commit_polish(
    run: &mut Run,
    board: &Status,
    row_id: i64,
    summary: &Summary,
    template: &str,
    before: &HashSet<String>,
    out: &mut dyn Write,
) -> Result<Option<commit::Committed>> {
    let repo = run.state().repo.clone();
    let feature = run.state().feature.clone();
    let retro_rel = format!("{}/retro.md", board.dir);
    let retro = repo.join(&retro_rel);
    if !retro.exists() {
        fs::write(&retro, template.replace("{NNN}", &feature))
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
    flag_body(run, &body)?;
    let spec = Pathspec::from_stream(summary, board, &repo);
    let mut paths = spec.paths.clone();
    for p in &spec.protected {
        if p != "docs/index.md" && !paths.contains(p) {
            paths.push(p.clone());
        }
    }
    for p in [format!("{}/prd.md", board.dir), retro_rel] {
        if !paths.contains(&p) {
            paths.push(p);
        }
    }
    let skip: HashSet<String> = paths.iter().chain(&spec.protected).cloned().collect();
    let found = scheduler::unnamed(run, board, before, &skip, &spec.dirs, true)?;
    note_leftovers(run, "polish", "polish: worker", &found, out)?;
    paths.extend(found.sweep);
    let committed = commit::commit(
        &repo,
        &paths,
        &commit::subject(board, "fix", "polish review fixes"),
    )?;
    match &committed {
        Some(c) => {
            run.db().set_worker_commit(row_id, &c.sha)?;
            say(
                out,
                &format!(
                    "polish: committed {} — {} file(s): {}",
                    short(&c.sha),
                    c.files.len(),
                    c.files.join(", ")
                ),
            )?;
        }
        None => say(out, "polish: worker changed nothing to commit")?,
    }
    Ok(committed)
}

/// Emit `template_stand_in` for `what` (AD-904), once per run: a resume that
/// finds the file it filled already there does not repeat it.
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

/// The PR body's first line when a closeout file is a template stand-in
/// (AD-904), from the run's `template_stand_in` events: the retro, the PR
/// body, or both. `None` when the workers wrote both.
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

/// `body` with `line` first: a stand-in line an earlier pass put there gives
/// way, so a resume or a second stand-in never stacks them (AD-904).
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

/// Put the stand-in line first in the PR body file, if the run has one; a
/// body no stand-in touched is left as the worker wrote it.
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

/// A PR body when the polish worker wrote none: the PRD and the pointers.
fn fallback_body(board: &Status, feature: &str) -> String {
    format!(
        "## PRD-{feature} — {}\n\n\
         PRD: `{}/prd.md`\n\n\
         > Opened by `factory`, unattended. **Not merged — that is yours.** \
         The polish worker left no PR body; the agent log and the retro in `{}/` carry the decisions and the numbers.\n\n\
         ### Needs your review\n\n- see `{}/agent-log.md` § Decisions, the `Review: decide` entries\n\n\
         ### Human QA\n\n- see `{}/prd.md` § QA plan\n",
        board.slug, board.dir, board.dir, board.dir, board.dir
    )
}

// ── ship ────────────────────────────────────────────────────────────────────

/// `docs ship`, the retro keys, `render --write`, `docs(NNN): ship PRD-NNN`. A
/// refusal (a slice not `done`) blocks with phase `polish` and the tool's
/// text. `None` when the ship commit landed.
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
    let shipped = docs.ship(&feature)?;
    if shipped.code != 0 {
        let text = {
            let err = shipped.stderr.trim();
            if err.is_empty() {
                shipped.stdout.trim().to_string()
            } else {
                err.to_string()
            }
        };
        let first = text
            .lines()
            .map(str::trim)
            .find(|l| !l.is_empty())
            .unwrap_or("docs ship failed")
            .to_string();
        let quoted: String = text.lines().map(|l| format!("  {l}\n")).collect();
        let body = format!(
            "**Happened:** the polish commit is in and `docs ship {feature}` refused (exit {}):\n{quoted}\
             **Options:**\n\
             1. fix what it names (a slice not `done`, a missing frontmatter), commit, and answer this block — the ship runs again\n\
             2. revert the polish commit and set the feature back to `building` if the board is wrong about being done\n\
             **Recommended:** 1 — the tool refuses only when the board and the tree disagree; the board is the truth to fix.\n",
            shipped.code
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
    for line in shipped.stdout.lines().filter(|l| l.starts_with('⚠')) {
        say(out, &format!("ship: {}", line.trim()))?;
    }
    // FR-24: polish wrote `AD` entries after `board` was read; a fresh
    // status counts them, the held one stands in if the tool cannot answer.
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
    // FR-25: the changed lines against the base; a failed diff stores nulls
    // and never stops the ship.
    let base = default_branch(run, docs);
    let lines = match retro::changed_lines(&repo, &base) {
        Ok(l) => Some(l),
        Err(e) => {
            say(out, &format!("ship: no changed lines stored — {e}"))?;
            None
        }
    };
    run.db().set_run_lines(run.id().as_str(), lines)?;
    let report = report::compute(run.db(), run.id().as_str(), None)?;
    let retro = feature_dir.join("retro.md");
    if !retro.exists() {
        fs::write(
            &retro,
            retro_template(docs, board).replace("{NNN}", &feature),
        )
        .map_err(io(format!("write {}", retro.display())))?;
        let retro_rel = format!("{}/retro.md", board.dir);
        say(
            out,
            &format!("ship: no {retro_rel} — created from the template"),
        )?;
        stood_in(run, STAND_IN_RETRO, &retro_rel)?;
    }
    fill_retro(&retro, &report)?;
    // A section the markers refuse is a line in the log, never a stopped ship.
    if let Err(e) = retro::write(&retro, &retro::numbers(run.db(), run.id().as_str())?) {
        say(out, &format!("ship: no ## Numbers written — {e}"))?;
    }
    docs.render_write()?;
    let want = commit::expand(&repo, &[board.dir.clone(), "docs/index.md".to_string()])?;
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

/// The twelve numeric keys of `retro.md` from the report, by name; `score`
/// untouched. Minutes keep two decimals, counts are integers.
pub fn fill_retro(retro: &Path, r: &Report) -> Result<()> {
    let values = retro_values(r);
    let pairs: Vec<(&str, &str)> = values.iter().map(|(k, v)| (*k, v.as_str())).collect();
    set_frontmatter(retro, &pairs)
}

fn retro_values(r: &Report) -> Vec<(&'static str, String)> {
    vec![
        ("gate_runs", r.gate_runs.to_string()),
        ("gate_red", r.gate_red.to_string()),
        ("gate_minutes", format!("{:.2}", r.gate_minutes)),
        ("ci_cycles", r.ci_cycles.to_string()),
        ("slices", r.slices.to_string()),
        ("checkpoints", r.checkpoints.to_string()),
        ("build_minutes", format!("{:.2}", r.build_minutes)),
        ("polish_minutes", format!("{:.2}", r.polish_minutes)),
        ("out_tokens", r.out_tokens.to_string()),
        ("cache_read_tokens", r.cache_read_tokens.to_string()),
        ("start_context", r.start_context.to_string()),
        ("peak_context", r.peak_context.to_string()),
    ]
}

// ── push and PR ─────────────────────────────────────────────────────────────

/// `git push -u origin <branch>`, then `gh pr create --draft --head <branch>`
/// — or, with a PR already in `state.json`, `gh pr edit` with the body. A
/// refused push or `gh` blocks with phase `pr`; a `proceed` to the create's
/// block finishes the run with the owner's PR, to the edit's leaves the body.
/// `None` when the PR stands.
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
    // The PR this run opened, or the one a previous run on this branch did
    // (`gh pr view`): either way the body is refreshed, no second PR.
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

/// `git push -u origin <branch>` — from the feature branch only; a refusal
/// blocks with phase `pr`.
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

/// Polish, ship, push and fix CI only from the run's feature branch: never
/// from the default branch, a detached HEAD or another branch
/// (`ci::branch_problem`). A problem blocks with `phase`; `None` when HEAD is
/// where it should be.
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

/// The PRD's h1 without its `PRD-NNN — ` prefix; the slug when it has none.
pub fn pr_title(prd: &Path, feature: &str, slug: &str) -> String {
    let text = fs::read_to_string(prd).unwrap_or_default();
    let h1 = text
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

/// What the watch does after one red poll.
enum Next {
    /// Not settled (a rerun in flight): wait one interval and poll again.
    Wait(String),
    /// A rerun is queued: poll again now, the grace starts over.
    Again,
    /// A fix round on these checks, with their failed logs.
    Fix {
        round: u32,
        check: String,
        log: String,
    },
    /// Leave the watch (a block).
    Out(Outcome),
}

/// The watch's view of one PR: where it runs and what it has seen.
struct Watch {
    repo: PathBuf,
    feature_dir: PathBuf,
    /// The `closeout_blocked` seq of the last answered `ci` block: the caps
    /// count the rounds and reruns after it.
    window: u64,
    /// The job links of the rows the driver just reran: until the new
    /// attempt registers, they are the old attempt's.
    stale: HashSet<String>,
}

/// Bounded polls of the PR's checks until green: each poll reads the PR's
/// head (`gh pr view --json headRefOid`) and, when it is this branch's head,
/// the checks (`gh pr checks --json`). Inside the grace window after the
/// watch starts, a push or a rerun, an empty table, a missing check or an old
/// PR head is "not registered yet". A red is one flaky rerun when another run
/// of the workflow passed at the same head, else a fix round (three since the
/// last answer, then block), each fix pushed and polled again. A timeout or a
/// runner that never started blocks at once, and so does a pending table past
/// the timeout. `None` is green, no checks to watch, or the owner's `proceed`.
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

/// A red poll: the run behind each failed check (from its job link), then a
/// foreign head (block), a flaky red (one rerun, then block), the cap
/// (block) or the next fix round with the failed logs.
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
    // A rerun in flight (the owner's, or ours after a restart): its failed
    // rows are the previous attempt's.
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
    // autonomy.md:79 — a red that is not this branch's head is the owner's.
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
    // autonomy.md:78 — flaky: another run of the same workflow passed at
    // this head; each failed run is rerun once.
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
                 **Recommended:** 1 — autonomy.md:78 gives the driver one rerun.\n",
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
                            "ci: `{names}` passed once at this head in another run of `{}` — rerun {}'s failed jobs (flaky, autonomy.md:78)",
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
    // autonomy.md:77 — three fix rounds since the owner last answered, then block.
    if rounds_done >= ci::CI_FIX_ROUNDS {
        let body = format!(
            "**Happened:** `{check}` is still red at `{}` after {rounds_done} fix rounds; the driver's cap is {} (autonomy.md:77). \
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

/// The run behind an infrastructure row is queued or running again (a
/// rerun the owner started): the reason to wait, else `None`. A run `gh`
/// will not show is no reason to wait — the block names the rows.
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

/// autonomy.md:79 — a check that never reached a verdict (timed out, never
/// started, cancelled) is infrastructure, not a defect: block at once.
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
         A timeout, a runner that never started or a cancelled job is infrastructure (autonomy.md:79), not a defect a fix worker can see.\n\
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

/// A `gh pr view`/`gh pr checks` the watch cannot read: block with `title`.
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

/// A `gh run` lookup the red verdict needs, refused: block rather than guess
/// (a swallowed failure is a wrong fix round or a missed rerun).
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

/// One interval in one-second steps, `Stopped` as soon as a stop is asked.
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

/// `45s`, `6 min`: a wait as a block says it.
fn span(secs: u64) -> String {
    if secs < 120 {
        format!("{secs}s")
    } else {
        format!("{} min", secs / 60)
    }
}

/// Each line indented two spaces, for a table quoted in a block.
fn indent(text: &str) -> String {
    text.lines().map(|l| format!("  {l}\n")).collect()
}

/// One CI fix round: a fix worker (once more after a death, then block),
/// its files committed as `fix(NNN): CI <check>` with `ci_cycles` bumped in the
/// same commit. `None` when the round is committed (or changed nothing).
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
    let agent_log = feature_dir.join("agent-log.md");
    // The fix commits: a HEAD moved during the watch blocks before a worker
    // spends the round on another branch.
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
        let before = scheduler::snapshot(run, &ci_fix_key(round))?;
        let w = Worker::spawn(
            run,
            Job::CiFix { round, attempt },
            &pack,
            &cfg.workers.fix,
            &[],
        )?;
        let session_id = w.session_id.clone();
        let row_id = w.row_id;
        say(
            out,
            &format!(
                "ci: fix worker {session_id} spawned for `{check}` (round {round}, attempt {attempt}, pid {}, pack {})",
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
            settle_dead(run, &agent_log, "CI fix", &session_id, row_id, &exit, out)?;
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
        settle(run, row_id, &session_id, &exit.summary, WorkerOutcome::Done)?;
        commit_ci_fix(
            run,
            board,
            round,
            check,
            row_id,
            &exit.summary,
            &before,
            out,
        )?;
        return Ok(None);
    }
}

/// `fix(NNN): CI <check>`: the stream's paths plus `retro.md` with `ci_cycles`
/// bumped to the rounds committed, this one included (`retro.md:11`). What
/// the worker changed unnamed since `before` is swept or listed (AD-50002).
#[allow(clippy::too_many_arguments)]
fn commit_ci_fix(
    run: &mut Run,
    board: &Status,
    round: u32,
    check: &str,
    row_id: i64,
    summary: &Summary,
    before: &HashSet<String>,
    out: &mut dyn Write,
) -> Result<()> {
    let repo = run.state().repo.clone();
    let spec = Pathspec::from_stream(summary, board, &repo);
    for path in &spec.protected {
        say(
            out,
            &format!("ci: fix worker wrote {path}, which no fix may — left out of the commit"),
        )?;
    }
    let mut paths = spec.paths.clone();
    let retro_rel = format!("{}/retro.md", board.dir);
    let retro = repo.join(&retro_rel);
    if retro.exists() {
        let committed = run
            .events()?
            .iter()
            .filter(|e| matches!(e.kind, EventKind::CiFixCommitted { .. }))
            .count() as u64;
        set_frontmatter(&retro, &[("ci_cycles", &(committed + 1).to_string())])?;
        if !paths.contains(&retro_rel) {
            paths.push(retro_rel);
        }
    }
    let skip: HashSet<String> = paths.iter().chain(&spec.protected).cloned().collect();
    let found = scheduler::unnamed(run, board, before, &skip, &spec.dirs, true)?;
    note_leftovers(run, "ci-fix", "ci: fix worker", &found, out)?;
    paths.extend(found.sweep);
    match commit::commit(
        &repo,
        &paths,
        &commit::subject(board, "fix", &format!("CI {check}")),
    )? {
        Some(c) => {
            run.db().set_worker_commit(row_id, &c.sha)?;
            say(
                out,
                &format!(
                    "ci: round {round} committed {} — {} file(s): {}",
                    short(&c.sha),
                    c.files.len(),
                    c.files.join(", ")
                ),
            )?;
            run.emit(EventKind::CiFixCommitted {
                round,
                sha: c.sha,
                files: c.files,
            })?;
        }
        None => say(
            out,
            &format!("ci: fix worker of round {round} changed nothing to commit"),
        )?,
    }
    Ok(())
}

// ── orphans ─────────────────────────────────────────────────────────────────

/// A polish or CI fix worker of a dead driver (FR-1 resume): a returned one
/// has its files committed as the live path would; a limited one pauses the
/// run; anything else is dead, and the closeout runs again from the events.
pub fn settle_orphan(
    run: &mut Run,
    board: &Status,
    w: &WorkerRef,
    summary: Summary,
    out: &mut dyn Write,
) -> Result<()> {
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
        return Ok(());
    }
    if summary.is_dead() {
        settle(run, w.row_id, &w.session_id, &summary, WorkerOutcome::Dead)?;
        run.emit(EventKind::WorkerDead {
            slice: None,
            session_id: w.session_id.clone(),
            exit_code: None,
        })?;
        say(
            out,
            &format!(
                "closeout: {what} worker {} of an earlier driver is gone (dead) — the closeout runs again",
                w.session_id
            ),
        )?;
        return Ok(());
    }
    settle(run, w.row_id, &w.session_id, &summary, WorkerOutcome::Done)?;
    say(
        out,
        &format!(
            "closeout: {what} worker {} of an earlier driver returned — committing what it wrote",
            w.session_id
        ),
    )?;
    if w.role == "polish" {
        let docs = Docs::locate(&run.state().repo)?;
        let template = retro_template(&docs, board);
        let before = scheduler::load_baseline(run.dir(), POLISH_KEY)?;
        let committed = commit_polish(run, board, w.row_id, &summary, &template, &before, out)?;
        run.emit(EventKind::PolishDone {
            sha: committed.as_ref().map(|c| c.sha.clone()),
            files: committed.map(|c| c.files).unwrap_or_default(),
        })?;
        return Ok(());
    }
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
    let before = scheduler::load_baseline(run.dir(), &ci_fix_key(round))?;
    commit_ci_fix(run, board, round, &check, w.row_id, &summary, &before, out)
}

/// The spawn baseline key of the polish worker: one runs at a time, and a
/// respawn waits for the last one to be settled.
const POLISH_KEY: &str = "closeout-polish";

/// The spawn baseline key of a CI fix round's worker.
fn ci_fix_key(round: u32) -> String {
    format!("closeout-ci-fix-{round}")
}

/// A closeout worker's unnamed changes, said and logged: the swept ones go
/// in its commit, the listed ones stay in the tree (`closeout_leftovers`).
fn note_leftovers(
    run: &mut Run,
    role: &str,
    who: &str,
    found: &scheduler::Unnamed,
    out: &mut dyn Write,
) -> Result<()> {
    if !found.sweep.is_empty() {
        say(
            out,
            &format!(
                "{who} changed {} tracked file(s) no tool call named — in its commit: {}",
                found.sweep.len(),
                found.sweep.join(", ")
            ),
        )?;
    }
    if !found.listed.is_empty() {
        run.emit(EventKind::CloseoutLeftovers {
            role: role.to_string(),
            files: found.listed.clone(),
            blobs: found.blobs.clone(),
        })?;
        say(
            out,
            &format!(
                "{who} left {} file(s) no tool call named — not committed: {}",
                found.listed.len(),
                found.listed.join(", ")
            ),
        )?;
    }
    Ok(())
}

// ── shared ──────────────────────────────────────────────────────────────────

/// An unresolved closeout block as the log files hold it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenBlock {
    pub id: String,
    pub title: String,
    pub phase: String,
}

/// Every unresolved `BLOCK-n` with a closeout phase in the feature's log
/// files (fragments first, then `agent-log.md`), in file order.
pub fn open_blocks(feature_dir: &Path) -> Result<Vec<OpenBlock>> {
    let mut out = Vec::new();
    for file in answers::log_files(feature_dir)? {
        out.extend(
            answers::entries(&file)?
                .into_iter()
                .filter(|e| !e.resolved && e.phase.as_deref().is_some_and(|p| PHASES.contains(&p)))
                .map(|e| OpenBlock {
                    id: e.id,
                    title: e.title,
                    phase: e.phase.unwrap_or_default(),
                }),
        );
    }
    Ok(out)
}

/// Write `BLOCK-n` (lowest free number) under `## Blockers` in `agent-log.md`
/// with the phase in its heading, emit `closeout_blocked`, hand back the park.
fn block(
    run: &mut Run,
    feature_dir: &Path,
    phase: &str,
    title: &str,
    body: &str,
    out: &mut dyn Write,
) -> Result<Outcome> {
    let agent_log = feature_dir.join("agent-log.md");
    let id = format!("BLOCK-{}", lowest_free_block(feature_dir)?);
    // Titles carry tool output (a refusal's first line, a check's name): a
    // `·` or a newline there would break the heading's fields.
    let title = answers::block_title(title);
    let entry = format!("### {id} — {title} · {phase} · open\n\n{body}");
    append_under(&agent_log, "## Blockers", &entry)?;
    run.emit(EventKind::CloseoutBlocked {
        phase: phase.to_string(),
        block_id: id.clone(),
        question: title.clone(),
    })?;
    say(
        out,
        &format!("closeout ({phase}) blocked — {id} written to agent-log.md: {title}"),
    )?;
    Ok(Outcome::Parked {
        what: park_line(&id, &title, phase),
        id,
        fresh: true,
    })
}

fn park_line(id: &str, title: &str, phase: &str) -> String {
    format!(
        "closeout blocked ({phase}) — {title}: {id} in agent-log.md waits for the owner: `factory answer {id} \"<text>\"`"
    )
}

/// A closeout block of this run the owner has answered: the seq of its
/// `closeout_blocked` event and whether the answer is `proceed`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Answered {
    seq: u64,
    id: String,
    phase: String,
    title: String,
    proceed: bool,
}

/// The run's closeout blocks (its `closeout_blocked` events, in order) whose
/// entry is resolved, with the answer read from the entry's
/// `**Resolution (…):**` line — written with the `resolved` heading in one
/// write, so a resolved entry always has it.
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
        let text = resolution(&answers::entry_text(&entry)?).unwrap_or_default();
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

/// The text of the entry's last `**Resolution (date):** text` line.
fn resolution(entry: &str) -> Option<String> {
    entry.lines().rev().find_map(|l| {
        let rest = l.trim().strip_prefix("**Resolution (")?;
        let (_, text) = rest.split_once("):**")?;
        Some(text.trim().to_string())
    })
}

/// The answer takes a block's `proceed` option: its first word, after an
/// option number (`2.`, `2)`) and any backticks or punctuation, is `proceed`.
fn is_proceed(text: &str) -> bool {
    let t = text
        .trim()
        .trim_start_matches(|c: char| c.is_ascii_digit())
        .trim_start_matches(|c: char| !c.is_alphanumeric());
    let word: String = t.chars().take_while(|c| c.is_alphanumeric()).collect();
    word.eq_ignore_ascii_case("proceed")
}

/// HEAD's `prd.md` says `status: shipped`: the ship commit is in.
fn shipped_at_head(repo: &Path, dir: &str) -> Result<bool> {
    let out = ci::git(repo, &["show", &format!("HEAD:{dir}/prd.md")])?;
    Ok(out.ok() && frontmatter_value(&out.stdout, "status").as_deref() == Some("shipped"))
}

/// `key:` of a markdown file's frontmatter, unquoted.
fn frontmatter_value(text: &str, key: &str) -> Option<String> {
    let body = text.strip_prefix("---\n")?;
    let end = body.find("\n---")?;
    body[..end].lines().find_map(|l| {
        let (k, v) = l.split_once(':')?;
        (k.trim() == key).then(|| v.trim().trim_matches(['"', '\'']).to_string())
    })
}

/// `at` of the latest answer to a block naming no slice: a checkpoint's or
/// the closeout's. Deaths before it do not count (the owner reset the cap).
fn last_answer(run: &Run) -> Result<Option<String>> {
    Ok(answers::all(run.dir())?
        .into_iter()
        .filter(|a| a.slice.is_none() && a.id.starts_with("BLOCK-"))
        .map(|a| a.at)
        .max())
}

/// FR-9 between the closeout's steps: wait out a pause (phase `paused`, then
/// back to `closeout`), hand a spent budget up, or go on (`None`).
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

/// A worker gone without a result: a Traps line in `agent-log.md`; the caller
/// respawns or blocks on the count.
fn settle_dead(
    run: &mut Run,
    agent_log: &Path,
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
    let date = clock::today();
    append_under(
        agent_log,
        "## Traps",
        &format!("- **Dead {what} worker ({date})**: session {session_id} {code} with no result."),
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
    let _ = metrics::record_subagents(run, row_id, session_id);
    metrics::record_exit(run, row_id, summary)?;
    run.set_worker_outcome(session_id, outcome.as_str())
}

/// The default branch: the preflight the run was created from, else a fresh
/// `docs preflight`, else `main`.
fn default_branch(run: &Run, docs: &Docs) -> String {
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

/// The run's branch (`state.json`), else the one the repo is on.
fn feature_branch(run: &Run, repo: &Path) -> Result<String> {
    if let Some(b) = run.state().branch.clone() {
        return Ok(b);
    }
    let out = ci::git(repo, &["rev-parse", "--abbrev-ref", "HEAD"])?;
    Ok(out.stdout.trim().to_string())
}

/// `templates/retro.md` beside the docs tool, else a bare frontmatter with
/// the keys and the heading.
fn retro_template(docs: &Docs, board: &Status) -> String {
    let path = docs
        .tool()
        .parent()
        .and_then(Path::parent)
        .map(|d| d.join("templates").join("retro.md"));
    if let Some(text) = path.and_then(|p| fs::read_to_string(p).ok()) {
        return text;
    }
    let mut t = String::from("---\nscore:\n");
    for k in RETRO_KEYS {
        t.push_str(&format!(
            "{k}:{}\n",
            if k == "ci_cycles" { " 0" } else { "" }
        ));
    }
    t.push_str(&format!(
        "---\n\n# Retro — PRD-{{NNN}}\n\nWritten by `factory` in the closeout commit of `{}`.\n",
        board.slug
    ));
    t
}

fn first_line(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("no output")
        .to_string()
}

/// The run dir's PR body path.
pub fn pr_body_path(run_dir: &Path) -> PathBuf {
    run_dir.join(PR_BODY_FILE)
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
            "---\nstatus: ready\n---\n\n# PRD-001 — Fixture\n\ntext\n",
        )
        .unwrap();
        assert_eq!(pr_title(&prd, "001", "fixture"), "Fixture");
        fs::write(&prd, "# PRD-001: The driver\n").unwrap();
        assert_eq!(pr_title(&prd, "001", "driver"), "The driver");
        fs::write(&prd, "no heading\n").unwrap();
        assert_eq!(pr_title(&prd, "001", "driver"), "driver");
    }

    #[test]
    fn the_retro_keys_are_filled_by_name_and_score_stays() {
        let dir = tempfile::tempdir().unwrap();
        let retro = dir.path().join("retro.md");
        fs::write(
            &retro,
            "---\nscore:\ngate_runs:\ngate_red:\ngate_minutes:\nci_cycles: 0\nslices:\ncheckpoints:\nbuild_minutes:\npolish_minutes:\nout_tokens:\ncache_read_tokens:\nstart_context:\npeak_context:\n---\n\n# Retro\n",
        )
        .unwrap();
        let r = Report {
            gate_runs: 2,
            gate_red: 1,
            gate_minutes: 1.5,
            slices: 3,
            checkpoints: 2,
            build_minutes: 0.25,
            out_tokens: 7,
            peak_context: 9,
            ..Report::default()
        };
        fill_retro(&retro, &r).unwrap();
        let text = fs::read_to_string(&retro).unwrap();
        assert!(text.starts_with("---\nscore:\ngate_runs: 2\ngate_red: 1\ngate_minutes: 1.50\nci_cycles: 0\nslices: 3\ncheckpoints: 2\nbuild_minutes: 0.25\npolish_minutes: 0.00\nout_tokens: 7\ncache_read_tokens: 0\nstart_context: 0\npeak_context: 9\n---\n"), "{text}");
        assert!(text.ends_with("# Retro\n"));
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
    fn the_frontmatter_status_is_read_unquoted() {
        let prd =
            "---\nstatus: \"shipped\"\nreviewed: 2026-10-10\n---\n\n# PRD-001\n\nstatus: draft\n";
        assert_eq!(frontmatter_value(prd, "status").as_deref(), Some("shipped"));
        assert_eq!(frontmatter_value(prd, "score"), None);
        assert_eq!(frontmatter_value("# no frontmatter\n", "status"), None);
    }

    #[test]
    fn the_fallback_template_has_every_key() {
        let t = {
            let mut t = String::from("---\nscore:\n");
            for k in RETRO_KEYS {
                t.push_str(&format!("{k}:\n"));
            }
            t
        };
        for k in RETRO_KEYS {
            assert!(t.contains(&format!("\n{k}:")));
        }
        assert_eq!(RETRO_KEYS.len(), 12);
    }
}
