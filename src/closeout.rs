//! The closeout (PRD §8 FR-6): when the last checkpoint is done — every
//! slice `done`, nothing unverified — the run goes polish → ship → push and
//! draft PR → CI → finished, or parks with a `BLOCK-n` that says why.
//!
//! Polish is one worker (`/polish` Unattended) that writes the retro prose
//! and the PR body in its last turn; the driver commits what it wrote as
//! `polish: review fixes`. Ship is `docs ship NNN`, the twelve numeric retro
//! keys from the store (`factory report`'s rows, FR-8), `render --write` and
//! `docs: ship PRD-NNN`. The PR is `gh pr create --draft`; a resume after
//! `pr_opened` edits it instead. CI is `gh pr checks --watch`, three fix
//! rounds and one flaky rerun as `autonomy.md:77-79` prescribe. Nothing here
//! merges or marks the PR ready — the owner does.
//!
//! Every step is idempotent on re-entry: the events say what landed
//! (`polish_done`, `shipped`), `state.json` keeps the PR URL, and an open
//! closeout block (`· polish ·`, `· pr ·`, `· ci ·`) keeps the park until the
//! owner answers.
use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::answers;
use crate::checkpoint::{Outcome, lowest_free_block};
use crate::ci::{self, Checks};
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
use crate::stream::Summary;
use crate::worker::{self, Exit, Job, Worker};
use crate::{Result, clock, metrics};

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
    if !agent_log.exists() {
        fs::write(&agent_log, format!("# Agent log — PRD-{feature}\n"))
            .map_err(io(format!("write {}", agent_log.display())))?;
    }

    // A PRD already `shipped` (a new run on a delivered feature: the preflight
    // lets it through to resume at the PR) has had its polish and its ship.
    let already_shipped = board.status.as_deref() == Some("shipped");
    let events = run.events()?;
    let polished = already_shipped
        || events
            .iter()
            .any(|e| matches!(e.kind, EventKind::PolishDone { .. }));
    let shipped = already_shipped
        || events
            .iter()
            .any(|e| matches!(e.kind, EventKind::Shipped { .. }));
    if run.state().phase != Phase::Closeout {
        run.set_phase(Phase::Closeout)?;
    }
    say(
        out,
        &format!(
            "closeout — {}",
            match (polished, shipped, run.state().pr_url.is_some()) {
                (false, _, _) => "polish",
                (true, false, _) => "resumed at ship",
                (true, true, false) if already_shipped => "PRD already shipped, resumed at the PR",
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
                 2. run `/polish` by hand, write `retro.md` and `runs/<id>/pr-body.md`, set `reviewed:` in `prd.md`, commit, and answer this block — the driver ships from there\n\
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
        let committed = commit_polish(run, board, row_id, &exit.summary, &template, out)?;
        run.emit(EventKind::PolishDone {
            sha: committed.as_ref().map(|c| c.sha.clone()),
            files: committed.map(|c| c.files).unwrap_or_default(),
        })?;
        return Ok(None);
    }
}

/// `polish: review fixes`: the paths the stream named plus `prd.md`
/// (`reviewed:`) and `retro.md` — the polish worker is the one worker allowed
/// to write those two; `docs/index.md` stays the ship's. A missing `retro.md`
/// is created from the template first, a missing PR body from a fallback, so
/// the ship and the PR never wait on a worker that skipped its last turn.
fn commit_polish(
    run: &mut Run,
    board: &Status,
    row_id: i64,
    summary: &Summary,
    template: &str,
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
    }
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
    let committed = commit::commit(&repo, &paths, "polish: review fixes")?;
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

/// `docs ship`, the retro keys, `render --write`, `docs: ship PRD-NNN`. A
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
    let report = report::compute(run.db(), run.id().as_str(), None)?;
    let retro = feature_dir.join("retro.md");
    if !retro.exists() {
        fs::write(
            &retro,
            retro_template(docs, board).replace("{NNN}", &feature),
        )
        .map_err(io(format!("write {}", retro.display())))?;
    }
    fill_retro(&retro, &report)?;
    docs.render_write()?;
    let want = commit::expand(&repo, &[board.dir.clone(), "docs/index.md".to_string()])?;
    let committed = commit::commit(&repo, &want, &format!("docs: ship PRD-{feature}"))?;
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

/// `git push -u origin <branch>`, then `gh pr create --draft` — or, with a
/// PR already in `state.json`, `gh pr edit` with the body. A refused push or
/// `gh` blocks with phase `pr`. `None` when the PR stands.
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
    if let Some(o) = push(run, &feature_dir, &branch, out)? {
        return Ok(Some(o));
    }
    let body = run.dir().join(PR_BODY_FILE);
    if !body.exists() {
        fs::write(&body, fallback_body(board, &feature))
            .map_err(io(format!("write {}", body.display())))?;
    }
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
                    &format!("gh pr edit refused: {}", first_line(&text)),
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
    match ci::pr_create(&repo, &base, &title, &body)? {
        Ok(url) => {
            run.set_pr_url(url.clone())?;
            say(out, &format!("pr: draft opened against {base} — {url}"))?;
            run.emit(EventKind::PrOpened { url })?;
            Ok(None)
        }
        Err(text) => {
            let body = format!(
                "**Happened:** the branch `{branch}` is pushed and `gh pr create --draft --base {base} --title \"{title}\" --body-file …` refused:\n  {text}\n\
                 **Options:**\n\
                 1. fix `gh` (login, the remote, an existing PR for this branch) and answer this block — the create runs again\n\
                 2. open the PR by hand and answer this block with `proceed` — the driver then opens none and the run finishes without a CI watch\n\
                 **Recommended:** 1 — the body is written at `{}` either way.\n",
                run.dir().join(PR_BODY_FILE).display()
            );
            Ok(Some(block(
                run,
                &feature_dir,
                "pr",
                &format!("gh pr create refused: {}", first_line(&text)),
                &body,
                out,
            )?))
        }
    }
}

/// `git push -u origin <branch>`; a refusal blocks with phase `pr`.
fn push(
    run: &mut Run,
    feature_dir: &Path,
    branch: &str,
    out: &mut dyn Write,
) -> Result<Option<Outcome>> {
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

/// `gh pr checks --watch` until green: a red is one flaky rerun when the same
/// head passed once, else a fix round (three, then block), each fix pushed and
/// watched again. `None` is green (or no checks to watch).
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
    loop {
        run.reload()?;
        if run.state().stop_requested {
            return Ok(Some(Outcome::Stopped));
        }
        say(out, &format!("ci: watching the checks of {url}"))?;
        let (check, table) = match ci::watch(&repo, &pr)? {
            Checks::Green => {
                let head = ci::head(&repo)?;
                say(out, &format!("ci: green at {}", short(&head)))?;
                run.emit(EventKind::CiGreen { head })?;
                return Ok(None);
            }
            Checks::None => {
                say(
                    out,
                    "ci: no checks reported on the branch — nothing to watch",
                )?;
                return Ok(None);
            }
            Checks::Unreadable { text } => {
                let body = format!(
                    "**Happened:** `gh pr checks {pr} --watch` exited non-zero with no failing check to name:\n  {text}\n\
                     **Options:**\n\
                     1. fix `gh` (login, network) and answer this block — the watch runs again\n\
                     2. answer `proceed` once the checks are green on GitHub — the watch runs again and finishes\n\
                     **Recommended:** 1.\n"
                );
                return Ok(Some(block(
                    run,
                    &feature_dir,
                    "ci",
                    &format!("gh pr checks unreadable: {}", first_line(&text)),
                    &body,
                    out,
                )?));
            }
            Checks::Red { check, table } => (check, table),
        };
        let head = ci::head(&repo)?;
        let events = run.events()?;
        let rounds: HashSet<u32> = events
            .iter()
            .filter_map(|e| match &e.kind {
                EventKind::CiFixSpawned { round, .. } => Some(*round),
                _ => None,
            })
            .collect();
        let rounds_done = rounds.len() as u32;
        run.emit(EventKind::CiRed {
            check: check.clone(),
            round: rounds_done,
            head: head.clone(),
        })?;
        say(
            out,
            &format!(
                "ci: `{check}` red at {} ({rounds_done} fix round(s) so far)",
                short(&head)
            ),
        )?;
        let runs = ci::runs(&repo, &branch).unwrap_or_default();
        let failed = ci::failed_run(&runs, &check);
        // autonomy.md:79 — a red that is not this branch's head is the owner's.
        if let Some(f) = failed
            && !f.head_sha.is_empty()
            && f.head_sha != head
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
                short(&head)
            );
            return Ok(Some(block(
                run,
                &feature_dir,
                "ci",
                &format!("CI red at a head this branch does not have: {check}"),
                &body,
                out,
            )?));
        }
        // autonomy.md:78 — flaky: the same head passed once; rerun once.
        if let Some(f) = failed
            && ci::green_at(&runs, &f.head_sha)
        {
            let reruns = events
                .iter()
                .filter(|e| matches!(&e.kind, EventKind::CiRerun { head: h, .. } if *h == head))
                .count() as u32;
            if reruns < ci::CI_FLAKY_RERUNS {
                match ci::rerun(&repo, f.database_id)? {
                    Ok(()) => {
                        say(
                            out,
                            &format!(
                                "ci: `{check}` passed once at this head — rerun {} of its failed jobs (flaky, autonomy.md:78)",
                                f.database_id
                            ),
                        )?;
                        run.emit(EventKind::CiRerun {
                            check: check.clone(),
                            run_id: f.database_id,
                            head: head.clone(),
                        })?;
                        continue;
                    }
                    Err(text) => say(
                        out,
                        &format!(
                            "ci: `gh run rerun {}` refused ({text}) — treated as a fix round",
                            f.database_id
                        ),
                    )?,
                }
            } else {
                let body = format!(
                    "**Happened:** `{check}` is red at `{}` after its one rerun ({}); the same head passed before, so this is flake the rerun did not clear, not a defect a fix worker can see.\n\
                     **Options:**\n\
                     1. rerun on GitHub until green and answer this block — the watch runs again\n\
                     2. fix the flaky check on a separate branch and answer this block with `proceed`\n\
                     **Recommended:** 1 — autonomy.md:78 gives the driver one rerun.\n",
                    short(&head),
                    f.database_id
                );
                return Ok(Some(block(
                    run,
                    &feature_dir,
                    "ci",
                    &format!("flaky check {check} red again after its rerun"),
                    &body,
                    out,
                )?));
            }
        }
        // autonomy.md:77 — three fix rounds, then block.
        if rounds_done >= ci::CI_FIX_ROUNDS {
            let body = format!(
                "**Happened:** `{check}` is still red at `{}` after {rounds_done} fix rounds; the driver's cap is {} (autonomy.md:77). \
                 The fix commits are on the branch; the last check table:\n{}\
                 **Options:**\n\
                 1. fix it by hand, push, and answer this block — the watch runs again\n\
                 2. revert the fix commits and answer this block once the cause is understood\n\
                 **Recommended:** 1 — three workers saw the same red; the cause is outside what the log names.\n",
                short(&head),
                ci::CI_FIX_ROUNDS,
                table
                    .lines()
                    .map(|l| format!("  {l}\n"))
                    .collect::<String>()
            );
            return Ok(Some(block(
                run,
                &feature_dir,
                "ci",
                &format!("CI red after {rounds_done} fix rounds: {check}"),
                &body,
                out,
            )?));
        }
        let round = rounds_done + 1;
        let log = match failed {
            Some(f) => ci::log_failed(&repo, f.database_id)?,
            None => table.clone(),
        };
        if let Some(o) = fix_round(run, docs, cfg, board, round, &check, &log, out)? {
            return Ok(Some(o));
        }
        if let Some(o) = push(run, &feature_dir, &branch, out)? {
            return Ok(Some(o));
        }
    }
}

/// One CI fix round: a fix worker (once more after a death, then block),
/// its files committed as `fix: CI <check>` with `ci_cycles` bumped in the
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
        commit_ci_fix(run, board, round, check, row_id, &exit.summary, out)?;
        return Ok(None);
    }
}

/// `fix: CI <check>`: the stream's paths plus `retro.md` with `ci_cycles`
/// bumped to the rounds committed, this one included (`retro.md:11`).
fn commit_ci_fix(
    run: &mut Run,
    board: &Status,
    round: u32,
    check: &str,
    row_id: i64,
    summary: &Summary,
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
    match commit::commit(&repo, &paths, &format!("fix: CI {check}"))? {
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
        let committed = commit_polish(run, board, w.row_id, &summary, &template, out)?;
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
    commit_ci_fix(run, board, round, &check, w.row_id, &summary, out)
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
    let entry = format!("### {id} — {title} · {phase} · open\n\n{body}");
    append_under(&agent_log, "## Blockers", &entry)?;
    run.emit(EventKind::CloseoutBlocked {
        phase: phase.to_string(),
        block_id: id.clone(),
        question: title.to_string(),
    })?;
    say(
        out,
        &format!("closeout ({phase}) blocked — {id} written to agent-log.md: {title}"),
    )?;
    Ok(Outcome::Parked {
        what: park_line(&id, title, phase),
        id,
        fresh: true,
    })
}

fn park_line(id: &str, title: &str, phase: &str) -> String {
    format!(
        "closeout blocked ({phase}) — {title}: {id} in agent-log.md waits for the owner: `factory answer {id} \"<text>\"`"
    )
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
