//! `factory answer <BLOCK-n|ASK-n> "<text>"` (PRD §5 FR-3, §3 FR-1): the
//! owner's half of a parked run.
//!
//! `BLOCK-n`: the entry is found in the feature's log fragments or
//! `agent-log.md`; the answer lands in `runs/<id>/answers/BLOCK-n.json`,
//! `**Resolution ({date}):** <text>` is appended under the entry and its
//! heading flips `open` → `resolved` (autonomy.md § Resuming), and a slice
//! the block left `blocked` goes back to `building`, lane unchanged. The
//! driver's slow tick sees a `building` slice with nothing alive on it and an
//! answer naming it, and respawns it with the block and the answer in the
//! pack. A retry of an answer that failed after the heading flipped finishes
//! it. `ASK-n`: the answer file, a `DEC` line
//! in `agent-log.md`, `ask_answered`; the run starts when the last ask is
//! answered. Either way, a run with no live driver gets one here, so the
//! owner's one command is the whole resume.
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use clap::Args;

use crate::answers::{self, Answer};
use crate::error::io;
use crate::events::EventKind;
use crate::quota::{self, Hold};
use crate::run::{self, Phase, Run};
use crate::{Error, Result, clock, daemon, paths, scheduler};

#[derive(Args, Debug, Clone)]
pub struct AnswerArgs {
    /// `BLOCK-n` or `ASK-n`
    pub id: String,
    /// The answer, as one argument or several words
    #[arg(required = true, trailing_var_arg = true)]
    pub text: Vec<String>,
}

pub fn answer(args: &AnswerArgs, out: &mut dyn Write) -> Result<i32> {
    let cwd = std::env::current_dir().map_err(io("current directory"))?;
    let repo = paths::repo_root(&cwd)?;
    let data_dir = paths::data_dir();
    let id = args.id.trim().to_ascii_uppercase();
    let text = args.text.join(" ").trim().to_string();
    if text.is_empty() {
        return Err(Error::Msg("the answer is empty".into()));
    }
    if id.starts_with("BLOCK-") {
        answer_block(&repo, &data_dir, &id, &text, out)
    } else if id.starts_with("ASK-") {
        answer_ask(&repo, &data_dir, &id, &text, out)
    } else {
        Err(Error::Msg(format!(
            "`{}` is neither BLOCK-n nor ASK-n",
            args.id
        )))
    }
}

fn answer_block(
    repo: &Path,
    data_dir: &Path,
    id: &str,
    text: &str,
    out: &mut dyn Write,
) -> Result<i32> {
    let mut hits: Vec<(PathBuf, answers::BlockEntry)> = Vec::new();
    for dir in feature_dirs(repo)? {
        for e in answers::find_all(&dir, id)? {
            hits.push((dir.clone(), e));
        }
    }
    if hits.is_empty() {
        writeln!(
            out,
            "no {id} under {}: a block is a `### {id} — …` heading in a log-*.md fragment or agent-log.md",
            rel(repo, &repo.join("docs/features"))
        )
        .map_err(io("stdout"))?;
        return Ok(1);
    }
    let at = |e: &answers::BlockEntry| format!("{} line {}", rel(repo, &e.path), e.start + 1);
    // Only an open block takes an answer; a resolved one of the same number
    // (an earlier feature's, or a phase block of this one) is not a target.
    let mut open: Vec<(PathBuf, answers::BlockEntry)> =
        hits.iter().filter(|(_, e)| !e.resolved).cloned().collect();
    if open.is_empty() {
        return answer_resolved(repo, data_dir, id, &hits, out);
    }
    if open.len() > 1 {
        // The parked run's feature first: that is the block a driver waits on.
        let mut parked = Vec::new();
        for (dir, e) in &open {
            let feature = feature_number(dir)?;
            if Run::find_resumable(data_dir, repo, &feature)?
                .is_some_and(|r| r.state().phase == Phase::WaitingOwner)
            {
                parked.push((dir.clone(), e.clone()));
            }
        }
        if !parked.is_empty() {
            open = parked;
        }
    }
    if open.len() > 1 {
        writeln!(
            out,
            "{id} is open in {} places — nothing resolved; resolve the one you mean by hand:",
            open.len()
        )
        .map_err(io("stdout"))?;
        for (_, e) in &open {
            writeln!(out, "  {} — {}", at(e), e.title).map_err(io("stdout"))?;
        }
        return Ok(1);
    }
    let (feature_dir, entry) = open.remove(0);
    let where_ = at(&entry);
    // What can refuse goes first: a block naming a slice with no file
    // writes nothing.
    let slice = match entry.slice {
        Some(n) => Some((n, slice_file(&feature_dir, n)?)),
        None => None,
    };
    let now = clock::now_rfc3339();
    let date = now[..10].to_string();
    let feature = feature_number(&feature_dir)?;
    let mut run = Run::find_resumable(data_dir, repo, &feature)?;
    // The answer file and `block_answered` before the slice flips: a driver
    // that reads `building` must already find the answer, or it would take
    // the slice for a dead worker's (slice 04 task 3's trap), and the log
    // must read `block_answered` before the `slice_spawned` it causes. They
    // go before the heading too: a retry finds the block open until every
    // step that cannot be redone is done, and redoes them.
    if let Some(run) = &mut run {
        answers::write(
            run.dir(),
            &Answer {
                id: id.to_string(),
                slice: entry.slice,
                text: text.to_string(),
                at: now.clone(),
            },
        )?;
        run.emit(EventKind::BlockAnswered {
            id: id.to_string(),
            slice: entry.slice,
            text: text.to_string(),
        })?;
    }
    // The heading and its resolution before the flip: the respawned
    // worker's pack quotes the entry as the log holds it.
    answers::resolve(&entry, text, &date)?;
    writeln!(out, "{id} resolved ({where_}): {text}").map_err(io("stdout"))?;
    deliver(repo, &feature, slice, run, out)?;
    Ok(0)
}

/// Every `id` found is resolved. A retry of an answer that stopped after
/// the heading flipped — the slice still `blocked`, or the parked run never
/// woken — finishes it; anything else is refused.
fn answer_resolved(
    repo: &Path,
    data_dir: &Path,
    id: &str,
    hits: &[(PathBuf, answers::BlockEntry)],
    out: &mut dyn Write,
) -> Result<i32> {
    let at = |e: &answers::BlockEntry| format!("{} line {}", rel(repo, &e.path), e.start + 1);
    for (dir, e) in hits {
        let feature = feature_number(dir)?;
        let Some(run) = Run::find_resumable(data_dir, repo, &feature)? else {
            continue;
        };
        if answers::read(run.dir(), id)?.is_none_or(|a| a.slice != e.slice) {
            continue;
        }
        let slice = match e.slice {
            Some(n) => Some((n, slice_file(dir, n)?)),
            None => None,
        };
        let blocked = match &slice {
            Some((_, f)) => scheduler::read_slice_status(f)?.as_deref() == Some("blocked"),
            None => false,
        };
        let unwoken = run.state().phase == Phase::WaitingOwner
            && !run.state().driver_alive()
            && !matches!(quota::hold(&run)?, Some(Hold::Budget { .. }));
        if blocked || unwoken {
            writeln!(
                out,
                "{id} is resolved ({}) and run {} holds its answer — finishing it",
                at(e),
                run.id()
            )
            .map_err(io("stdout"))?;
            deliver(repo, &feature, slice, Some(run), out)?;
            return Ok(0);
        }
    }
    writeln!(out, "{id} is already resolved ({})", at(&hits[0].1)).map_err(io("stdout"))?;
    Ok(1)
}

/// After the log: the slice back to `building` if the block parked it —
/// a slice that is `done`, `pending` or already `building` is left as is —
/// then the run woken.
fn deliver(
    repo: &Path,
    feature: &str,
    slice: Option<(u32, PathBuf)>,
    run: Option<Run>,
    out: &mut dyn Write,
) -> Result<()> {
    if let Some((n, file)) = slice {
        match scheduler::read_slice_status(&file)? {
            Some(s) if s == "blocked" => {
                scheduler::set_slice_status(&file, "building")?;
                writeln!(out, "slice {n:02} → building ({})", rel(repo, &file))
                    .map_err(io("stdout"))?;
            }
            status => {
                writeln!(
                    out,
                    "slice {n:02} is {}: left as is ({})",
                    status.as_deref().unwrap_or("without a status"),
                    rel(repo, &file)
                )
                .map_err(io("stdout"))?;
            }
        }
    }
    match run {
        None => {
            writeln!(
                out,
                "no open run for PRD-{feature} — `factory run {feature}` starts one"
            )
            .map_err(io("stdout"))?;
        }
        Some(run) => wake(&run, out)?,
    }
    Ok(())
}

fn answer_ask(
    repo: &Path,
    data_dir: &Path,
    id: &str,
    text: &str,
    out: &mut dyn Write,
) -> Result<i32> {
    // The newest open run of this repo that still waits on `id`.
    let mut target: Option<(Run, String)> = None;
    for s in run::list_open(data_dir, repo)?.into_iter().rev() {
        let run = Run::open(data_dir, s.id.as_str())?;
        if let Some((_, item)) = answers::asks_pending(&run)?
            .into_iter()
            .find(|(i, _)| i == id)
        {
            target = Some((run, item));
            break;
        }
    }
    let Some((mut run, item)) = target else {
        writeln!(out, "no open run of this repo waits on {id}").map_err(io("stdout"))?;
        return Ok(1);
    };
    let now = clock::now_rfc3339();
    let date = now[..10].to_string();
    answers::write(
        run.dir(),
        &Answer {
            id: id.to_string(),
            slice: None,
            text: text.to_string(),
            at: now,
        },
    )?;
    let feature = run.state().feature.clone();
    let log = agent_log(repo, &feature)?;
    if !log.is_file() {
        fs::write(&log, format!("# Agent log — PRD-{feature}\n"))
            .map_err(io(format!("write {}", log.display())))?;
    }
    scheduler::append_under(
        &log,
        "## Decisions",
        &format!("- **DEC ({date})**: preflight ask \"{item}\" answered: {text}"),
    )?;
    run.emit(EventKind::AskAnswered {
        id: id.to_string(),
        text: text.to_string(),
    })?;
    writeln!(out, "{id} answered: {text} — noted in {}", rel(repo, &log)).map_err(io("stdout"))?;
    let remaining = answers::asks_pending(&run)?;
    if remaining.is_empty() {
        writeln!(out, "every ask answered — the run proceeds").map_err(io("stdout"))?;
        wake(&run, out)?;
    } else {
        writeln!(
            out,
            "{} ask(s) still open: {}",
            remaining.len(),
            remaining
                .iter()
                .map(|(i, _)| i.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )
        .map_err(io("stdout"))?;
    }
    Ok(0)
}

/// A live driver picks the answer up on its slow tick; a parked run with no
/// driver gets one — unless it is parked on its budget, which a driver would
/// only park on again: that takes `factory run NNN --budget`; anything else
/// is `factory run`'s to resume.
fn wake(run: &Run, out: &mut dyn Write) -> Result<()> {
    let pid = run.state().pid;
    if run.state().driver_alive() {
        writeln!(
            out,
            "run {}: the driver (pid {pid}) picks it up within {} s",
            run.id(),
            scheduler::SLOW_TICK.as_secs()
        )
        .map_err(io("stdout"))?;
    } else if let Some(Hold::Budget { weighted, budget }) = quota::hold(run)? {
        writeln!(
            out,
            "run {} is parked on its budget ({weighted} weighted tokens, budget {budget}) — `factory run {} --budget <higher>` resumes it",
            run.id(),
            run.state().feature
        )
        .map_err(io("stdout"))?;
    } else if run.state().phase == Phase::WaitingOwner {
        let pid = daemon::spawn(run)?;
        writeln!(
            out,
            "run {}: driver pid {pid} — log {}",
            run.id(),
            daemon::log_path(run.dir()).display()
        )
        .map_err(io("stdout"))?;
    } else {
        writeln!(
            out,
            "run {} is {} with no live driver — `factory run {}` resumes it",
            run.id(),
            run.state().phase,
            run.state().feature
        )
        .map_err(io("stdout"))?;
    }
    Ok(())
}

/// `docs/features/NNN-*/`, in order.
fn feature_dirs(repo: &Path) -> Result<Vec<PathBuf>> {
    let features = repo.join("docs/features");
    let rd = match fs::read_dir(&features) {
        Ok(rd) => rd,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(io(format!("read {}", features.display()))(e)),
    };
    let mut dirs: Vec<PathBuf> = rd
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    Ok(dirs)
}

/// `001` from `docs/features/001-fixture`.
fn feature_number(feature_dir: &Path) -> Result<String> {
    let name = feature_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    let digits: String = name.chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() {
        return Err(Error::Msg(format!(
            "{} is not a NNN-slug feature folder",
            feature_dir.display()
        )));
    }
    Ok(digits)
}

/// The feature folder of `NNN`, and its `agent-log.md`.
fn agent_log(repo: &Path, feature: &str) -> Result<PathBuf> {
    let dir = feature_dirs(repo)?
        .into_iter()
        .find(|d| feature_number(d).is_ok_and(|n| n == feature))
        .ok_or_else(|| Error::Msg(format!("no docs/features/{feature}-* folder")))?;
    Ok(dir.join("agent-log.md"))
}

/// `slice-NN-*.md` in the feature folder.
fn slice_file(feature_dir: &Path, n: u32) -> Result<PathBuf> {
    let prefix = format!("slice-{n:02}-");
    fs::read_dir(feature_dir)
        .map_err(io(format!("read {}", feature_dir.display())))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .find(|p| {
            p.file_name()
                .and_then(|f| f.to_str())
                .is_some_and(|f| f.starts_with(&prefix) && f.ends_with(".md"))
        })
        .ok_or_else(|| Error::Msg(format!("no slice-{n:02}-*.md in {}", feature_dir.display())))
}

fn rel(repo: &Path, path: &Path) -> String {
    path.strip_prefix(repo)
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned()
}
