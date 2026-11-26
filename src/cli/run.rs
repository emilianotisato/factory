use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use clap::Args;

use crate::cli::lanes;
use crate::docs::{Branch, Docs, Preflight, preflight};
use crate::error::{io, json};
use crate::events::EventKind;
use crate::provider::{self, Provider};
use crate::run::{Phase, Run};
use crate::{Error, Result, commit, config, daemon, paths, scheduler};

#[derive(Args, Debug, Clone)]
pub struct RunArgs {
    /// The feature: its number (`001`) or slug
    pub feature: String,
    /// Weighted-token budget for this run; overrides `config.toml` and
    /// `docs/project.toml`
    #[arg(long, value_name = "N")]
    pub budget: Option<u64>,
    /// Keep the driver attached to this terminal instead of detaching it
    #[arg(long)]
    pub foreground: bool,
    /// The one line this run tests, stored on its row
    #[arg(long, value_name = "TEXT")]
    pub hypothesis: Option<String>,
    /// Where this run's CI runs; overrides `[ci] mode` in `config.toml`
    /// and `FACTORY_CI_MODE`
    #[arg(long, value_enum)]
    pub ci: Option<config::CiMode>,
    /// The agent CLI a role's workers run on, as `<provider>[:<model>[:<effort>]]` for every role or
    /// `<role>=<provider>[:<model>[:<effort>]]` for one of build, fix, checkpoint and polish; repeatable.
    /// Overrides `[workers]` in `config.toml` and `FACTORY_WORKER_PROVIDER`
    #[arg(long = "worker", value_name = "SPEC")]
    pub workers: Vec<String>,
}

/// `factory daemon <run>`: the detached driver `factory run` starts. Hidden.
#[derive(Args, Debug, Clone)]
pub struct DaemonArgs {
    pub run: String,
}

pub fn run(args: &RunArgs, out: &mut dyn Write, err: &mut dyn Write) -> Result<i32> {
    let cwd = std::env::current_dir().map_err(io("current directory"))?;
    let repo = paths::repo_root(&cwd)?;
    let data_dir = paths::data_dir();
    let mut loaded = config::load_with_project(&paths::config_dir(), &repo)?;
    for w in &loaded.warnings {
        writeln!(err, "factory: {w}").map_err(io("stderr"))?;
    }
    if let Some(b) = args.budget {
        loaded.config.budget_weighted_tokens = b;
        loaded.sources.budget = config::Source::Flag;
    }
    let budget = loaded.config.budget_weighted_tokens;
    // `main` maps an `Err` to exit 1; the not-on-v3 hint exits 2, so it is printed here.
    let docs = match Docs::open(&repo) {
        Ok(d) => d,
        Err(Error::NotV3(hint)) => {
            writeln!(err, "factory: {hint}").map_err(io("stderr"))?;
            return Ok(2);
        }
        Err(e) => return Err(e),
    };
    let feature = feature_number(&docs, &args.feature)?;

    if let Some(live) = Run::find_live(&data_dir, &repo, &feature)? {
        flags_ignored(args, live.id().as_str(), err)?;
        writeln!(
            out,
            "run {id} is live (pid {pid}) — stop it first: factory stop {id}",
            id = live.id(),
            pid = live.state().pid
        )
        .map_err(io("stdout"))?;
        return Ok(1);
    }
    if let Some(mut run) = Run::find_resumable(&data_dir, &repo, &feature)? {
        // The driver commits only on the run's branch: a tree switched away refuses before the run is touched.
        if let Some(branch) = run.state().branch.clone() {
            let default = load_preflight(run.dir())?.and_then(|pf| pf.branch.default);
            if let Some(why) = commit::branch_refusal(&repo, &branch, default.as_deref())? {
                writeln!(out, "run {} not resumed: {why}", run.id()).map_err(io("stdout"))?;
                return Ok(1);
            }
        }
        flags_ignored(args, run.id().as_str(), err)?;
        run.adopt()?;
        if let Some(b) = args.budget {
            run.set_budget(b)?;
            budget_knob(&run, b)?;
        }
        run.emit(EventKind::RunResumed)?;
        writeln!(out, "run {} resumed ({})", run.id(), run.state().phase).map_err(io("stdout"))?;
        print_board(&docs, &feature, out)?;
        return launch(&mut run, &docs, &loaded.config, args.foreground, out);
    }

    let mut specs = Vec::new();
    for raw in &args.workers {
        match config::parse_worker_spec(raw) {
            Ok(s) => specs.extend([s]),
            Err(why) => {
                writeln!(err, "factory: --worker {raw}: {why}").map_err(io("stderr"))?;
                return Ok(1);
            }
        }
    }
    let (worker_config, _) = config::resolve_workers(&loaded, &specs);

    let (ci_mode, ci_source, ci_warning) = config::resolve_ci_mode(&loaded, args.ci);
    if let Some(w) = ci_warning {
        writeln!(err, "factory: {w}").map_err(io("stderr"))?;
    }

    let (mut pf, _code) = docs.preflight(&feature)?;
    preflight::workers(&loaded, &specs, &mut pf);
    if !pf.refuse.is_empty() {
        for line in &pf.refuse {
            writeln!(out, "  ✗ {line}").map_err(io("stdout"))?;
        }
        writeln!(out, "\nrefused: {} blocker(s)", pf.refuse.len()).map_err(io("stdout"))?;
        return Ok(1);
    }
    preflight::second_opinion(&loaded, &mut pf);
    preflight::ci_mode(&loaded, args.ci, &mut pf);
    loaded.config.ci.mode = ci_mode;
    loaded.sources.ci = ci_source;
    // Lanes are made before the run exists, so a lane failure leaves no run behind.
    if !lanes::ensure_all(&data_dir, &crate::lanes::main_tree(&repo)?, out)? {
        writeln!(out, "\nrefused: a lane could not be made").map_err(io("stdout"))?;
        return Ok(1);
    }
    if !pf.ask.is_empty() {
        let mut run = Run::create(&data_dir, &repo, &feature, None, budget)?;
        run.set_authorized_tests(read_authorized_tests(&docs, &feature)?)?;
        run.set_ci_mode(ci_mode)?;
        run.set_worker_config(worker_config.clone())?;
        record_start(
            &run,
            &docs,
            &loaded,
            &specs,
            args.hypothesis.as_deref(),
            err,
        )?;
        save_preflight(run.dir(), &pf)?;
        for (i, text) in pf.ask.iter().enumerate() {
            let id = format!("ASK-{}", i + 1);
            writeln!(out, "{id}: {text}").map_err(io("stdout"))?;
            run.emit(EventKind::AskOpened {
                id,
                text: text.clone(),
            })?;
        }
        run.set_phase(Phase::WaitingOwner)?;
        writeln!(
            out,
            "\nrun {} waits for the owner: {} question(s) before it starts — \
             `factory answer ASK-n \"<text>\"`; \"proceed\" is always an option",
            run.id(),
            pf.ask.len()
        )
        .map_err(io("stdout"))?;
        return Ok(0);
    }

    let branch = ensure_branch(&repo, &pf.branch)?;
    let mut run = Run::create(&data_dir, &repo, &feature, Some(branch), budget)?;
    run.set_authorized_tests(read_authorized_tests(&docs, &feature)?)?;
    run.set_ci_mode(ci_mode)?;
    run.set_worker_config(worker_config)?;
    record_start(
        &run,
        &docs,
        &loaded,
        &specs,
        args.hypothesis.as_deref(),
        err,
    )?;
    // The closeout reads `branch.default` from here.
    save_preflight(run.dir(), &pf)?;
    // Judge lines are noted for the cockpit, never waited on.
    for text in &pf.judge {
        writeln!(out, "  · {text}").map_err(io("stdout"))?;
        run.emit(EventKind::JudgeNoted { text: text.clone() })?;
    }
    begin(&mut run, &docs, out)?;
    launch(&mut run, &docs, &loaded.config, args.foreground, out)
}

pub fn launch(
    run: &mut Run,
    docs: &Docs,
    cfg: &config::Config,
    foreground: bool,
    out: &mut dyn Write,
) -> Result<i32> {
    if foreground {
        scheduler::drive(run, docs, cfg, out)?;
        return Ok(0);
    }
    let pid = daemon::spawn(run)?;
    writeln!(
        out,
        "driver pid {pid} — log {}",
        daemon::log_path(run.dir()).display()
    )
    .map_err(io("stdout"))?;
    Ok(0)
}

/// By the time `drive` returns an error the run is already `failed`.
pub fn daemon(args: &DaemonArgs, out: &mut dyn Write, err: &mut dyn Write) -> Result<i32> {
    let mut run = Run::open(&paths::data_dir(), &args.run)?;
    run.adopt()?;
    let loaded = config::load_with_project(&paths::config_dir(), &run.state().repo)?;
    for w in &loaded.warnings {
        writeln!(err, "factory: {w}").map_err(io("stderr"))?;
    }
    let repo = run.state().repo.clone();
    let docs = Docs::open(&repo)?;
    writeln!(
        out,
        "{} driver pid {} took run {}",
        crate::clock::now_rfc3339(),
        std::process::id(),
        run.id()
    )
    .map_err(io("driver log"))?;
    match scheduler::drive(&mut run, &docs, &loaded.config, out) {
        Ok(_) => Ok(0),
        Err(e) => {
            writeln!(err, "factory: {e}").map_err(io("stderr"))?;
            Ok(1)
        }
    }
}

fn flags_ignored(args: &RunArgs, id: &str, err: &mut dyn Write) -> Result<()> {
    if args.hypothesis.is_some() {
        writeln!(err, "hypothesis ignored: run {id} already exists").map_err(io("stderr"))?;
    }
    if args.ci.is_some() {
        writeln!(err, "--ci ignored: run {id} already exists").map_err(io("stderr"))?;
    }
    if !args.workers.is_empty() {
        writeln!(err, "--worker ignored: run {id} already exists").map_err(io("stderr"))?;
    }
    Ok(())
}

fn record_start(
    run: &Run,
    docs: &Docs,
    loaded: &config::Loaded,
    specs: &[config::WorkerSpec],
    hypothesis: Option<&str>,
    err: &mut dyn Write,
) -> Result<()> {
    let lanes = docs.status(&run.state().feature).ok().map(|s| s.lanes);
    let workers = run.state().worker_config.clone();
    let providers = workers
        .as_ref()
        .map_or_else(Vec::new, config::ResolvedWorkers::distinct_providers);
    let versions = probe_provider_versions(&providers, err)?;
    let (review, review_source, _) = config::resolve_review(loaded);
    let knobs = config::knobs(
        loaded,
        workers.as_ref(),
        specs,
        (review.as_ref(), review_source),
        lanes,
        &versions,
    );
    let id = run.id().as_str();
    run.db().set_run_knobs(id, &knobs.to_string())?;
    if let Some(h) = hypothesis {
        run.db().set_run_hypothesis(id, h)?;
    }
    Ok(())
}

/// For a run that started before its workers were chosen: once they are, its knobs say what it runs on.
pub(crate) fn set_worker_knobs(
    run: &Run,
    loaded: Option<&config::Loaded>,
    err: &mut dyn Write,
) -> Result<()> {
    let Some(workers) = run.state().worker_config.clone() else {
        return Ok(());
    };
    let id = run.id().as_str();
    let Some(text) = run.db().run_row(id)?.and_then(|r| r.knobs) else {
        return Ok(());
    };
    let Ok(mut knobs) = serde_json::from_str::<serde_json::Value>(&text) else {
        return Ok(());
    };
    let versions = probe_provider_versions(&workers.distinct_providers(), err)?;
    knobs["workers"] = config::worker_knobs(loaded, &workers, &[]);
    knobs["provider_versions"] = serde_json::json!(versions);
    knobs["claude_version"] = serde_json::json!(versions.get(&Provider::Claude));
    run.db().set_run_knobs(id, &knobs.to_string())
}

fn budget_knob(run: &Run, budget: u64) -> Result<()> {
    set_knob(
        run,
        "budget",
        serde_json::json!(budget),
        config::Source::Flag,
    )
}

/// So the row says what the run went on with, not only what it started with.
pub(crate) fn set_knob(
    run: &Run,
    key: &str,
    value: serde_json::Value,
    source: config::Source,
) -> Result<()> {
    let id = run.id().as_str();
    let Some(text) = run.db().run_row(id)?.and_then(|r| r.knobs) else {
        return Ok(());
    };
    let Ok(mut knobs) = serde_json::from_str::<serde_json::Value>(&text) else {
        return Ok(());
    };
    knobs[key] = serde_json::json!({"value": value, "source": source});
    run.db().set_run_knobs(id, &knobs.to_string())
}

/// A lazy shim may download its tool or hang when first run, so each probe is bounded.
const VERSION_TIMEOUT: Duration = Duration::from_secs(5);

/// `<bin> --version` for each provider the run uses and no other; one that does not answer is warned of and left out.
pub fn probe_provider_versions(
    providers: &[Provider],
    err: &mut dyn Write,
) -> Result<BTreeMap<Provider, String>> {
    let mut out = BTreeMap::new();
    for &p in providers {
        if out.contains_key(&p) {
            continue;
        }
        let why = match provider::resolve_binary(p) {
            None => "not on PATH".to_string(),
            Some(bin) => match provider::run_bounded(&bin, &["--version"], VERSION_TIMEOUT) {
                Some(text) => match text.lines().map(str::trim).find(|l| !l.is_empty()) {
                    Some(v) => {
                        out.insert(p, v.to_string());
                        continue;
                    }
                    None => "printed nothing".to_string(),
                },
                None => format!(
                    "failed or gave no answer within {}s",
                    VERSION_TIMEOUT.as_secs()
                ),
            },
        };
        writeln!(
            err,
            "factory: {p} --version: {why}; the run records no {p} version"
        )
        .map_err(io("stderr"))?;
    }
    Ok(out)
}

/// Called again when the last `ASK-n` is answered.
pub fn begin(run: &mut Run, docs: &Docs, out: &mut dyn Write) -> Result<i32> {
    run.emit(EventKind::RunStarted)?;
    writeln!(
        out,
        "run {} started on {}",
        run.id(),
        run.state().branch.as_deref().unwrap_or("?")
    )
    .map_err(io("stdout"))?;
    print_board(docs, &run.state().feature.clone(), out)
}

/// The ask→branch step follows this `branch` rather than re-running preflight on a tree the owner may have touched.
pub const PREFLIGHT_FILE: &str = "preflight.json";

pub fn save_preflight(run_dir: &Path, pf: &Preflight) -> Result<()> {
    let path = run_dir.join(PREFLIGHT_FILE);
    let text = serde_json::to_string_pretty(pf).map_err(json("serialize preflight"))?;
    fs::write(&path, text + "\n").map_err(io(format!("write {}", path.display())))
}

pub fn load_preflight(run_dir: &Path) -> Result<Option<Preflight>> {
    let path = run_dir.join(PREFLIGHT_FILE);
    match fs::read_to_string(&path) {
        Ok(t) => Ok(Some(
            serde_json::from_str(&t).map_err(json(format!("parse {}", path.display())))?,
        )),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(io(format!("read {}", path.display()))(e)),
    }
}

pub fn ensure_branch(repo: &Path, b: &Branch) -> Result<String> {
    let feature = b
        .feature
        .as_deref()
        .ok_or_else(|| Error::Msg("preflight named no feature branch".into()))?;
    let current = b.current.as_deref().unwrap_or("");
    let default = b.default.as_deref().unwrap_or("the default branch");
    if b.default.as_deref() == Some(feature) {
        return Err(Error::Msg(format!(
            "the feature branch {feature} is the default branch — the driver commits only on a \
             feature branch, and the owner decides which one"
        )));
    }
    if current == feature {
        return Ok(feature.to_string());
    }
    if Some(current) == b.default.as_deref() {
        let args: &[&str] = if b.feature_exists {
            &["switch", feature]
        } else {
            &["switch", "-c", feature]
        };
        git(repo, args)?;
        return Ok(feature.to_string());
    }
    Err(Error::Msg(format!(
        "on {current}, which is neither {default} nor {feature} — the owner decides which branch this feature lives on"
    )))
}

fn git(repo: &Path, args: &[&str]) -> Result<()> {
    let out = Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .map_err(|e| Error::Tool {
            cmd: format!("git {}", args.join(" ")),
            detail: format!("could not start git: {e}"),
        })?;
    if !out.status.success() {
        return Err(Error::Tool {
            cmd: format!("git {}", args.join(" ")),
            detail: String::from_utf8_lossy(&out.stderr).trim().to_string(),
        });
    }
    Ok(())
}

fn print_board(docs: &Docs, feature: &str, out: &mut dyn Write) -> Result<i32> {
    let board = docs.status(feature)?;
    if board.ready.is_empty() {
        writeln!(out, "nothing ready").map_err(io("stdout"))?;
    }
    for s in &board.ready {
        writeln!(out, "{:02} {}", s.number, s.file).map_err(io("stdout"))?;
    }
    Ok(0)
}

fn feature_number(docs: &Docs, arg: &str) -> Result<String> {
    if let Ok(n) = arg.parse::<u32>() {
        return Ok(format!("{n:03}"));
    }
    Ok(format!("{:03}", docs.status(arg)?.number))
}

fn read_authorized_tests(docs: &Docs, feature: &str) -> Result<Vec<String>> {
    let dir = docs.dir(feature)?;
    crate::testguard::prd_authorized_tests(&dir.join("prd.md"))
}
