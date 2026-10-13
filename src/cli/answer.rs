//! `factory answer <BLOCK-n|ASK-n> "<text>"` (PRD §5 FR-3, §3 FR-1): the
//! owner's half of a parked run.
//!
//! `BLOCK-n`: the `block` entry is found in a feature's `log.jsonl`; the
//! answer lands in `runs/<id>/answers/BLOCK-n.json`, an `answer` entry with
//! `ref = BLOCK-n` is appended to the log (the block's line stays as it was),
//! and a slice the block left `blocked` goes back to `building` in
//! `state.toml`, lane unchanged. The driver's slow tick sees a `building`
//! slice with nothing alive on it and an answer naming it, and respawns it
//! with the block and the answer in the pack. A retry of an answer that
//! failed after the `answer` entry landed finishes it. Block ids restart at
//! `BLOCK-1` in every feature, so the same id open in two features goes to
//! the one whose run is parked.
//!
//! `ASK-n`: an `ask` entry (phase `preflight`) in the feature's log,
//! `ask_answered`, then the answer file; the run starts when the last ask is answered. ASK numbers are per
//! run: two open runs waiting on the same one are refused by name, and
//! `--run <id>` picks one.
//!
//! Either way, a run with no live driver gets one here, so the owner's one
//! command is the whole resume — unless it is parked on its budget, which
//! `factory run NNN --budget` resumes.
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use clap::Args;

use crate::answers::{self, Answer};
use crate::docs::{log, state};
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
    /// The run an `ASK-n` belongs to, when more than one open run asked it
    #[arg(long, value_name = "RUN")]
    pub run: Option<String>,
    /// The feature a `BLOCK-n` belongs to, when it is open in more than one
    #[arg(long, value_name = "NNN")]
    pub feature: Option<String>,
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
        answer_block(&repo, &data_dir, &id, &text, args.feature.as_deref(), out)
    } else if id.starts_with("ASK-") {
        answer_ask(&repo, &data_dir, &id, &text, args.run.as_deref(), out)
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
    feature: Option<&str>,
    out: &mut dyn Write,
) -> Result<i32> {
    let wanted = feature
        .map(|f| f.trim().parse::<u32>())
        .transpose()
        .map_err(|_| {
            Error::Msg(format!(
                "--feature takes a feature number, not `{}`",
                feature.unwrap_or("")
            ))
        })?;
    let mut hits: Vec<(PathBuf, answers::BlockEntry)> = Vec::new();
    for dir in feature_dirs(repo)? {
        if wanted.is_some_and(|n| feature_number(&dir).ok().and_then(|d| d.parse().ok()) != Some(n))
        {
            continue;
        }
        if let Some(e) = answers::find(&dir, id)? {
            hits.push((dir, e));
        }
    }
    if hits.is_empty() {
        writeln!(
            out,
            "no {id} under {}: a block is a `kind: block` entry in a feature's log.jsonl",
            rel(repo, &repo.join("docs/features"))
        )
        .map_err(io("stdout"))?;
        return Ok(1);
    }
    let at = |e: &answers::BlockEntry| format!("{} line {}", rel(repo, &e.path), e.line);
    // Only an open block takes an answer; a resolved one of the same id (an
    // earlier feature's) is not a target.
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
            "{id} is open in {} places — nothing resolved; name one with `--feature NNN`:",
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
    if let Some(n) = entry.slice {
        slice_file(&feature_dir, n)?;
    }
    let slice = entry.slice;
    let now = clock::now_rfc3339();
    let feature = feature_number(&feature_dir)?;
    let mut run = Run::find_resumable(data_dir, repo, &feature)?;
    // The answer file and `block_answered` before the slice flips: a driver
    // that reads `building` must already find the answer, or it would take
    // the slice for a dead worker's (slice 04 task 3's trap), and the log
    // must read `block_answered` before the `slice_spawned` it causes. They
    // go before the `answer` entry too: a retry finds the block open until
    // every step that cannot be redone is done, and redoes them.
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
    // The `answer` entry before the flip: the respawned worker's pack quotes
    // the block with its answer as the log holds them.
    answers::resolve(&feature_dir, &entry, text)?;
    writeln!(out, "{id} resolved ({where_}): {text}").map_err(io("stdout"))?;
    deliver(repo, &feature_dir, &feature, slice, run, out)?;
    Ok(0)
}

/// Every `id` found is resolved. A retry of an answer that stopped after
/// the `answer` entry landed — the slice still `blocked`, or the parked run never
/// woken — finishes it; anything else is refused.
fn answer_resolved(
    repo: &Path,
    data_dir: &Path,
    id: &str,
    hits: &[(PathBuf, answers::BlockEntry)],
    out: &mut dyn Write,
) -> Result<i32> {
    let at = |e: &answers::BlockEntry| format!("{} line {}", rel(repo, &e.path), e.line);
    for (dir, e) in hits {
        let feature = feature_number(dir)?;
        let Some(run) = Run::find_resumable(data_dir, repo, &feature)? else {
            continue;
        };
        if answers::read(run.dir(), id)?.is_none_or(|a| a.slice != e.slice) {
            continue;
        }
        let blocked = match e.slice {
            Some(n) => slice_status(dir, n)?.as_deref() == Some("blocked"),
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
            deliver(repo, dir, &feature, e.slice, Some(run), out)?;
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
    feature_dir: &Path,
    feature: &str,
    slice: Option<u32>,
    run: Option<Run>,
    out: &mut dyn Write,
) -> Result<()> {
    if let Some(n) = slice {
        let file = state::path(feature_dir);
        match slice_status(feature_dir, n)? {
            Some(s) if s == "blocked" => {
                state::set(feature_dir, &[(&format!("slices.{n}.status"), "building")])?;
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

/// The slice's status in `state.toml`; `None` when the file or the slice
/// has none.
fn slice_status(feature_dir: &Path, n: u32) -> Result<Option<String>> {
    Ok(state::read(feature_dir)?.and_then(|s| s.slices.get(&n).map(|s| s.status.clone())))
}

/// Whether a try that failed later already logged this ask's answer: an
/// `ask` entry titled `title` with its `**Chose:**` line.
fn preflight_logged(entries: &[log::Entry], title: &str, text: &str) -> bool {
    let chose = format!("**Chose:** {text}");
    entries
        .iter()
        .any(|e| e.kind == log::Kind::Ask && e.title == title && e.body.lines().any(|l| l == chose))
}

fn answer_ask(
    repo: &Path,
    data_dir: &Path,
    id: &str,
    text: &str,
    only: Option<&str>,
    out: &mut dyn Write,
) -> Result<i32> {
    // `ASK-n` is numbered per run: every open run of this repo that asked
    // it, newest first, with whether its answer is on file.
    let mut asked: Vec<(Run, String, bool)> = Vec::new();
    for s in run::list_open(data_dir, repo)?.into_iter().rev() {
        if only.is_some_and(|o| o != s.id.as_str()) {
            continue;
        }
        let run = Run::open(data_dir, s.id.as_str())?;
        let item = run.events()?.into_iter().find_map(|e| match e.kind {
            EventKind::AskOpened { id: i, text } if i == id => Some(text),
            _ => None,
        });
        if let Some(item) = item {
            let answered = answers::read(run.dir(), id)?.is_some();
            asked.push((run, item, answered));
        }
    }
    let waiting = asked.iter().filter(|(_, _, answered)| !answered).count();
    if waiting > 1 {
        writeln!(
            out,
            "{waiting} open runs wait on {id} — nothing answered; name one with `factory answer {id} --run <id> \"<text>\"`:"
        )
        .map_err(io("stdout"))?;
        for (run, item, _) in asked.iter().filter(|(_, _, answered)| !answered) {
            writeln!(out, "  {} (PRD-{}) — {item}", run.id(), run.state().feature)
                .map_err(io("stdout"))?;
        }
        return Ok(1);
    }
    let Some(i) = asked.iter().position(|(_, _, answered)| !answered) else {
        // A retry of an answer whose wake failed: every ask is on file and
        // the parked run has no driver.
        if let Some((run, _, _)) = asked.iter().find(|(run, _, _)| {
            run.state().phase == Phase::WaitingOwner && !run.state().driver_alive()
        }) && answers::asks_pending(run)?.is_empty()
        {
            writeln!(out, "{id} is answered and run {} has no driver", run.id())
                .map_err(io("stdout"))?;
            wake(run, out)?;
            return Ok(0);
        }
        writeln!(out, "no open run of this repo waits on {id}").map_err(io("stdout"))?;
        return Ok(1);
    };
    let (mut run, item, _) = asked.swap_remove(i);
    let now = clock::now_rfc3339();
    let feature = run.state().feature.clone();
    let dir = feature_dir(repo, &feature)?;
    // The `ask` entry and `ask_answered` before the answer file: the file is
    // what the driver starts on and what takes the ask off the list, so a
    // retry after a failure here still finds the ask open. Each step is
    // skipped when a failed try already did it.
    let title = format!("Preflight {id}: {item}");
    let (logged, _) = log::entries(&dir)?;
    if !preflight_logged(&logged, &title, text) {
        let mut entry = log::Entry::new(
            log::Kind::Ask,
            &title,
            &format!(
                "**Chose:** {text}\n**Because:** the owner's answer to a preflight ask, before any slice started.\n**Review:** no\n"
            ),
        );
        entry.phase = Some("preflight".to_string());
        log::add(&dir, entry)?;
    }
    let emitted = run.events()?.into_iter().any(
        |e| matches!(&e.kind, EventKind::AskAnswered { id: i, text: t } if i == id && t == text),
    );
    if !emitted {
        run.emit(EventKind::AskAnswered {
            id: id.to_string(),
            text: text.to_string(),
        })?;
    }
    answers::write(
        run.dir(),
        &Answer {
            id: id.to_string(),
            slice: None,
            text: text.to_string(),
            at: now,
        },
    )?;
    writeln!(
        out,
        "{id} answered: {text} — noted in {}",
        rel(repo, &log::path(&dir))
    )
    .map_err(io("stdout"))?;
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

/// The feature folder of `NNN`.
fn feature_dir(repo: &Path, feature: &str) -> Result<PathBuf> {
    feature_dirs(repo)?
        .into_iter()
        .find(|d| feature_number(d).is_ok_and(|n| n == feature))
        .ok_or_else(|| Error::Msg(format!("no docs/features/{feature}-* folder")))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_logged_answer_is_found_by_its_title_and_choice() {
        let title = "Preflight ASK-1: lanes";
        let ask = log::Entry::new(
            log::Kind::Ask,
            title,
            "**Chose:** proceed\n**Review:** no\n",
        );
        let decision = log::Entry::new(
            log::Kind::Decision,
            "Preflight ASK-2: x",
            "**Chose:** proceed\n",
        );
        let log = vec![ask, decision];
        assert!(preflight_logged(&log, title, "proceed"));
        assert!(!preflight_logged(&log, title, "fix it"));
        assert!(!preflight_logged(&log, "Preflight ASK-2: x", "proceed"));
    }

    #[test]
    fn a_state_toml_that_does_not_parse_is_an_error_not_no_status() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(state::path(dir.path()), "status = \n").unwrap();
        assert!(slice_status(dir.path(), 1).is_err());
        assert_eq!(
            slice_status(tempfile::tempdir().unwrap().path(), 1).unwrap(),
            None
        );
    }
}
