//! `factory setup`: the owner's cockpit, workers, reviewer and CI mode, asked or flagged, saved in one patch.
use std::ffi::OsString;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use clap::{Args, Subcommand, ValueEnum};
use serde_json::{Value, json};

use crate::config::{self, CiMode, Loaded, Patch, Source};
use crate::error::{io, json};
use crate::provider::{self, Detected, Provider, ReviewProvider};
use crate::skills::{self, Verdict};
use crate::{Error, Result, paths};

const ROLES: [&str; 4] = ["build", "fix", "checkpoint", "polish"];
const DISCOVER_TIMEOUT: Duration = Duration::from_secs(5);

pub const USAGE: &str = "\
factory setup asks its questions on a terminal; without one, pass flags:
  factory setup cockpit --provider <claude|codex|cursor|agy|opencode>
  factory setup workers [--role <default|build|fix|checkpoint|polish>] [--provider <claude|codex|cursor|agy|opencode>] [--model <m>] [--effort <e>] [--clear]
  factory setup review --provider <claude|codex|cursor|agy|opencode|none> [--model <m>] [--effort <e>]
  factory setup ci --mode <local|remote>
  factory setup --show [--json]
";

#[derive(Args, Debug, Clone, Default)]
pub struct SetupArgs {
    /// Print the effective configuration, each value's source and the CLIs on PATH; writes nothing
    #[arg(long)]
    pub show: bool,
    /// Print --show as JSON
    #[arg(long)]
    pub json: bool,
    #[command(subcommand)]
    pub section: Option<SetupSection>,
}

#[derive(Subcommand, Debug, Clone, PartialEq, Eq)]
pub enum SetupSection {
    /// The agent CLI you drive factory from
    Cockpit {
        #[arg(long, value_enum)]
        provider: Option<Provider>,
    },
    /// The agent CLI, model and effort workers run with, by default or for one role
    Workers {
        #[arg(long, value_enum)]
        role: Option<WorkerRole>,
        #[arg(long, value_enum)]
        provider: Option<Provider>,
        #[arg(long)]
        model: Option<String>,
        #[arg(long)]
        effort: Option<String>,
        /// Remove the role's provider, model and effort so it takes the defaults again
        #[arg(long)]
        clear: bool,
    },
    /// Who gives the second opinion, with which model and effort
    Review {
        #[arg(long, value_enum)]
        provider: Option<ReviewProvider>,
        #[arg(long)]
        model: Option<String>,
        #[arg(long)]
        effort: Option<String>,
    },
    /// Where a run's CI runs: on this machine or on the remote's checks
    Ci {
        #[arg(long, value_enum)]
        mode: Option<CiMode>,
    },
}

/// `Default` is the `[workers]` table every role inherits from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[value(rename_all = "lowercase")]
pub enum WorkerRole {
    Default,
    Build,
    Fix,
    Checkpoint,
    Polish,
}

impl WorkerRole {
    fn table(self) -> &'static str {
        match self {
            WorkerRole::Default => "workers",
            WorkerRole::Build => "workers.build",
            WorkerRole::Fix => "workers.fix",
            WorkerRole::Checkpoint => "workers.checkpoint",
            WorkerRole::Polish => "workers.polish",
        }
    }
}

/// Everything `run_with` would otherwise read from the process, so a test supplies its own.
pub struct Env {
    pub config_dir: PathBuf,
    pub repo: Option<PathBuf>,
    /// Searched for the provider CLIs as `PATH` is; never executed except to list a reviewer's models.
    pub path: OsString,
    pub is_tty: bool,
}

pub fn setup(
    args: &SetupArgs,
    input: &mut dyn BufRead,
    is_tty: bool,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<i32> {
    let env = Env {
        config_dir: paths::config_dir(),
        repo: paths::repo_root(Path::new(".")).ok(),
        path: std::env::var_os("PATH").unwrap_or_default(),
        is_tty,
    };
    run_with(args, input, &env, out, err)
}

pub fn run_with(
    args: &SetupArgs,
    input: &mut dyn BufRead,
    env: &Env,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<i32> {
    if args.show || args.json {
        if args.section.is_some() {
            writeln!(
                err,
                "factory: --show changes nothing, so it takes no section"
            )
            .map_err(io("stderr"))?;
            return Ok(1);
        }
        return show(env, args.json, out);
    }
    if let Some(section) = &args.section
        && has_flags(section)
    {
        return flagged(section, env, out);
    }
    if !env.is_tty {
        err.write_all(USAGE.as_bytes()).map_err(io("stderr"))?;
        return Ok(1);
    }
    run_wizard(input, out, env, args.section.as_ref())
}

fn say(out: &mut dyn Write, line: &str) -> Result<()> {
    writeln!(out, "{line}").map_err(io("stdout"))
}

fn msg(text: impl Into<String>) -> Error {
    Error::Msg(text.into())
}

fn set(table: &str, key: &str, value: &str) -> Patch {
    Patch {
        table: table.into(),
        key: key.into(),
        value: Some(toml::Value::String(value.into())),
    }
}

fn unset(table: &str, key: &str) -> Patch {
    Patch {
        table: table.into(),
        key: key.into(),
        value: None,
    }
}

fn save(config_dir: &Path, edits: &[Patch], out: &mut dyn Write) -> Result<i32> {
    if edits.is_empty() {
        let path = config_dir.join("config.toml");
        say(
            out,
            &format!("nothing changed; {} left as it was", path.display()),
        )?;
        return Ok(0);
    }
    let path = config::patch(config_dir, edits)?;
    say(out, &format!("saved {}", path.display()))?;
    for e in edits {
        let line = match &e.value {
            Some(v) => format!("  [{}] {} = {v}", e.table, e.key),
            None => format!("  [{}] {} removed", e.table, e.key),
        };
        say(out, &line)?;
    }
    Ok(0)
}

/// What the owner said to one prompt.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Answer {
    Keep,
    Set(String),
    Clear,
}

/// The edit that turns `current` into the answer; none when they already agree.
fn answer_patch(table: &str, key: &str, answer: &Answer, current: Option<&str>) -> Option<Patch> {
    match answer {
        Answer::Keep => None,
        Answer::Set(v) if current == Some(v.as_str()) => None,
        Answer::Set(v) => Some(set(table, key, v)),
        Answer::Clear => current.is_some().then(|| unset(table, key)),
    }
}

/// The reviewer the owner's file names, by `provider` or a legacy `agent`; the environment does not count.
fn current_review(loaded: &Loaded) -> Option<ReviewProvider> {
    match config::resolve_review(loaded) {
        (Some(r), Source::User, _) => Some(r.provider),
        _ => None,
    }
}

/// A model or effort left as it was is dropped when the provider changes: slugs and levels are provider-specific.
fn review_edits(loaded: &Loaded, p: Provider, model: Answer, effort: Answer) -> Vec<Patch> {
    let r = &loaded.config.review;
    let s = &loaded.sources.review;
    let same = current_review(loaded) == Some(ReviewProvider::Provider(p));
    let mut edits = Vec::new();
    if !same {
        edits.push(set("review", "provider", p.as_str()));
        if s.agent != Source::Default && Provider::infer_from_binary(&r.agent) != Some(p) {
            edits.push(unset("review", "agent"));
        }
    }
    let fresh = |a: Answer| {
        if !same && a == Answer::Keep {
            Answer::Clear
        } else {
            a
        }
    };
    let model_now = (s.model != Source::Default).then_some(r.model.as_str());
    edits.extend(answer_patch("review", "model", &fresh(model), model_now));
    edits.extend(answer_patch(
        "review",
        "effort",
        &fresh(effort),
        r.effort.as_deref(),
    ));
    edits
}

fn review_none_edits(loaded: &Loaded) -> Vec<Patch> {
    let s = &loaded.sources.review;
    let mut edits = vec![set("review", "provider", "none")];
    for (key, present) in [("model", s.model), ("effort", s.effort), ("agent", s.agent)] {
        if present != Source::Default {
            edits.push(unset("review", key));
        }
    }
    edits
}

fn level(levels: &[&str], effort: &str, who: &str) -> Result<String> {
    levels
        .iter()
        .find(|l| l.eq_ignore_ascii_case(effort.trim()))
        .map(|l| l.to_string())
        .ok_or_else(|| {
            if levels.is_empty() {
                msg(format!("{who} takes no --effort"))
            } else {
                msg(format!(
                    "--effort `{effort}` is not one of {who}'s: {}",
                    levels.join(", ")
                ))
            }
        })
}

fn model_flag(model: &str) -> Result<String> {
    let m = model.trim();
    if m.is_empty() {
        return Err(msg("--model is empty"));
    }
    if !provider::is_model_name(m) {
        return Err(msg(format!("--model {m:?} is not a model id")));
    }
    Ok(m.to_string())
}

fn has_flags(section: &SetupSection) -> bool {
    match section {
        SetupSection::Cockpit { provider } => provider.is_some(),
        SetupSection::Workers {
            role,
            provider,
            model,
            effort,
            clear,
        } => role.is_some() || provider.is_some() || model.is_some() || effort.is_some() || *clear,
        SetupSection::Review {
            provider,
            model,
            effort,
        } => provider.is_some() || model.is_some() || effort.is_some(),
        SetupSection::Ci { mode } => mode.is_some(),
    }
}

fn flagged(section: &SetupSection, env: &Env, out: &mut dyn Write) -> Result<i32> {
    let loaded = config::load(&env.config_dir)?;
    let edits = match section {
        SetupSection::Cockpit { provider } => {
            let p = provider.ok_or_else(|| msg("give --provider"))?;
            vec![set("cockpit", "provider", p.as_str())]
        }
        SetupSection::Workers {
            role,
            provider,
            model,
            effort,
            clear,
        } => worker_flags(
            &loaded,
            *role,
            *provider,
            model.as_deref(),
            effort.as_deref(),
            *clear,
        )?,
        SetupSection::Review {
            provider,
            model,
            effort,
        } => review_flags(&loaded, *provider, model.as_deref(), effort.as_deref())?,
        SetupSection::Ci { mode } => {
            let m = mode.ok_or_else(|| msg("give --mode"))?;
            vec![set("ci", "mode", m.as_str())]
        }
    };
    save(&env.config_dir, &edits, out)
}

/// What one `[workers]` table holds, and the provider its model and effort belong to.
struct TableNow {
    table: &'static str,
    own_provider: Option<Provider>,
    /// A table that names no provider anywhere holds Claude slugs, as it did before workers had a provider.
    provider: Provider,
    model: Option<String>,
    effort: Option<String>,
    /// A model this table resolves to without holding one: a role's `[workers]` default for the same provider.
    inherited_model: bool,
}

fn table_now(loaded: &Loaded, role: WorkerRole) -> TableNow {
    let w = &loaded.config.workers;
    let name = match role {
        WorkerRole::Default => None,
        WorkerRole::Build => Some("build"),
        WorkerRole::Fix => Some("fix"),
        WorkerRole::Checkpoint => Some("checkpoint"),
        WorkerRole::Polish => Some("polish"),
    };
    let Some(name) = name else {
        return TableNow {
            table: role.table(),
            own_provider: w.provider,
            provider: w.provider.unwrap_or(Provider::Claude),
            model: w.model.clone(),
            effort: w.effort.clone(),
            inherited_model: false,
        };
    };
    let worker = w.role(name).expect("a known role");
    let own = loaded
        .sources
        .workers
        .role(name)
        .cloned()
        .unwrap_or_default();
    let provider = worker.provider.or(w.provider).unwrap_or(Provider::Claude);
    TableNow {
        table: role.table(),
        own_provider: worker.provider,
        provider,
        model: (own.model != Source::Default).then(|| worker.model.clone()),
        effort: (own.effort != Source::Default).then(|| worker.effort.clone()),
        inherited_model: w.model.is_some() && w.provider.unwrap_or(Provider::Claude) == provider,
    }
}

/// The edits that set a table's provider, model and effort; a model or effort left as it was is dropped when the
/// provider changes, as slugs and levels are provider-specific.
fn worker_edits(
    now: &TableNow,
    provider: Answer,
    chosen: Provider,
    model: Answer,
    effort: Answer,
) -> Vec<Patch> {
    let table = now.table;
    let own = now.own_provider.map(Provider::as_str);
    let switched = chosen != now.provider;
    let fresh = |a: Answer| match a {
        Answer::Keep if switched => Answer::Clear,
        a => a,
    };
    let effort = if chosen.effort_levels().is_empty() {
        Answer::Clear
    } else {
        fresh(effort)
    };
    let mut edits: Vec<Patch> = answer_patch(table, "provider", &provider, own)
        .into_iter()
        .collect();
    edits.extend(answer_patch(
        table,
        "model",
        &fresh(model),
        now.model.as_deref(),
    ));
    edits.extend(answer_patch(
        table,
        "effort",
        &effort,
        now.effort.as_deref(),
    ));
    edits
}

fn worker_flags(
    loaded: &Loaded,
    role: Option<WorkerRole>,
    provider: Option<Provider>,
    model: Option<&str>,
    effort: Option<&str>,
    clear: bool,
) -> Result<Vec<Patch>> {
    let now = table_now(loaded, role.unwrap_or(WorkerRole::Default));
    let table = now.table;
    if clear {
        if provider.is_some() || model.is_some() || effort.is_some() {
            return Err(msg("--clear takes no --provider, --model or --effort"));
        }
        return Ok(vec![
            unset(table, "provider"),
            unset(table, "model"),
            unset(table, "effort"),
        ]);
    }
    if provider.is_none() && model.is_none() && effort.is_none() {
        return Err(msg("give --provider, --model, --effort or --clear"));
    }
    let p = provider.unwrap_or(now.provider);
    let model = match model {
        Some(m) => Answer::Set(model_flag(m)?),
        None => Answer::Keep,
    };
    let effort = match effort {
        Some(e) => Answer::Set(level(p.effort_levels(), e, p.as_str())?),
        None => Answer::Keep,
    };
    let keeps_model = p == now.provider && (now.model.is_some() || now.inherited_model);
    if p.default_model().is_empty() && model == Answer::Keep && !keeps_model {
        return Err(msg(format!("{p} has no default model: give --model")));
    }
    let answer = match provider {
        Some(p) => Answer::Set(p.as_str().to_string()),
        None => Answer::Keep,
    };
    Ok(worker_edits(&now, answer, p, model, effort))
}

fn review_flags(
    loaded: &Loaded,
    provider: Option<ReviewProvider>,
    model: Option<&str>,
    effort: Option<&str>,
) -> Result<Vec<Patch>> {
    let chosen = provider
        .or_else(|| current_review(loaded))
        .ok_or_else(|| msg("no reviewer is set yet: give --provider"))?;
    let ReviewProvider::Provider(p) = chosen else {
        if model.is_some() || effort.is_some() {
            return Err(msg("--provider none takes no --model or --effort"));
        }
        return Ok(review_none_edits(loaded));
    };
    let model = match model {
        Some(m) => Answer::Set(model_flag(m)?),
        None => Answer::Keep,
    };
    let effort = match effort {
        Some(e) => Answer::Set(level(p.effort_levels(), e, p.as_str())?),
        None => Answer::Keep,
    };
    let keeps_model =
        current_review(loaded) == Some(chosen) && loaded.sources.review.model != Source::Default;
    if p.default_model().is_empty() && model == Answer::Keep && !keeps_model {
        return Err(msg(format!("{p} has no default model: give --model")));
    }
    let mut edits = review_edits(loaded, p, model, effort);
    if provider.is_some() && !edits.iter().any(|e| e.key == "provider") {
        edits.insert(0, set("review", "provider", p.as_str()));
    }
    Ok(edits)
}

fn source_name(source: Source, resolved: bool) -> &'static str {
    match source {
        Source::Default if resolved => "env",
        Source::Default => "default",
        _ => "user",
    }
}

fn effective(loaded: &Loaded, detected: &[Detected]) -> Value {
    let w = &loaded.config.workers;
    let (_, worker_warning) = config::resolve_workers(loaded, &[]);
    let env_provider = std::env::var(config::WORKER_PROVIDER_ENV)
        .ok()
        .and_then(|v| v.trim().parse::<Provider>().ok());
    let fallback = w.provider.or(env_provider).unwrap_or(Provider::Claude);
    let default_provider_source = match (w.provider, env_provider) {
        (Some(_), _) => "user",
        (None, Some(_)) => "env",
        (None, None) => "default",
    };
    let given = |set: bool| if set { "user" } else { "default" };
    // A provider with no effort levels runs with none, as each role does.
    let effort = w
        .effort
        .clone()
        .filter(|_| !fallback.effort_levels().is_empty());
    let mut workers = serde_json::Map::new();
    workers.insert(
        "default".into(),
        json!({
            "provider": fallback.as_str(),
            "provider_source": default_provider_source,
            "model": w.model.clone().unwrap_or_else(|| fallback.default_model().to_string()),
            "effort": effort.clone().or_else(|| fallback.default_effort().map(str::to_string)),
            "model_source": given(w.model.is_some()),
            "effort_source": given(effort.is_some()),
        }),
    );
    for role in ROLES {
        let r = w.resolved(&loaded.sources.workers, role, fallback);
        let own = loaded
            .sources
            .workers
            .role(role)
            .cloned()
            .unwrap_or_default();
        let inherits = r.provider == fallback;
        let from = |own: Source, default_set: bool| {
            given(own != Source::Default || default_set && inherits)
        };
        workers.insert(
            role.into(),
            json!({
                "provider": r.provider.as_str(),
                "provider_source": if own.provider != Source::Default { "user" } else { default_provider_source },
                "model": r.model,
                "effort": r.effort,
                "model_source": from(own.model, w.model.is_some()),
                "effort_source": if r.effort.is_some() { from(own.effort, w.effort.is_some()) } else { "default" },
            }),
        );
    }
    let (review, review_source, review_warning) = config::resolve_review(loaded);
    let review = match review {
        Some(r) => json!({
            "provider": r.provider.as_str(),
            "model": (!r.model.is_empty()).then_some(r.model),
            "effort": r.effort,
            "binary": r.binary,
            "source": source_name(review_source, true),
        }),
        None => {
            json!({"provider": null, "model": null, "effort": null, "binary": null, "source": "default"})
        }
    };
    let (ci, ci_source, ci_warning) = config::resolve_ci_mode(loaded, None);
    let mut on_path = serde_json::Map::new();
    for p in Provider::ALL {
        let bin = detected.iter().find(|d| d.provider == p).map(|d| &d.binary);
        on_path.insert(p.as_str().into(), json!(bin));
    }
    let warnings: Vec<&String> = loaded
        .warnings
        .iter()
        .chain(worker_warning.iter())
        .chain(review_warning.iter())
        .chain(ci_warning.iter())
        .collect();
    json!({
        "config": loaded.path,
        "cockpit": {
            "provider": loaded.config.cockpit.provider.map(Provider::as_str),
            "source": source_name(loaded.sources.cockpit, false),
        },
        "workers": workers,
        "review": review,
        "ci": {"mode": ci.map(CiMode::as_str), "source": source_name(ci_source, ci.is_some())},
        "detected": on_path,
        "warnings": warnings,
    })
}

fn show(env: &Env, as_json: bool, out: &mut dyn Write) -> Result<i32> {
    let loaded = config::load(&env.config_dir)?;
    let detected = provider::detect_installed_in(&env.path);
    let v = effective(&loaded, &detected);
    if as_json {
        let text = serde_json::to_string_pretty(&v).map_err(json("setup --show"))?;
        say(out, &text)?;
        return Ok(0);
    }
    let s = |v: &Value| v.as_str().unwrap_or("not set").to_string();
    let sources = |o: &Value| {
        let (m, e) = (s(&o["model_source"]), s(&o["effort_source"]));
        if m == e {
            m
        } else {
            format!("model {m}, effort {e}")
        }
    };
    let worker = |o: &Value| {
        let effort = o["effort"]
            .as_str()
            .map_or(String::new(), |e| format!(" · {e}"));
        format!(
            "{} {}{effort} ({})",
            s(&o["provider"]),
            s(&o["model"]),
            sources(o)
        )
    };
    let mut lines = vec![
        format!("config      {}", s(&v["config"])),
        format!(
            "cockpit     {} ({})",
            s(&v["cockpit"]["provider"]),
            s(&v["cockpit"]["source"])
        ),
        format!("workers     {}", worker(&v["workers"]["default"])),
    ];
    for role in ROLES {
        lines.push(format!("  {role:<10}{}", worker(&v["workers"][role])));
    }
    let r = &v["review"];
    let mut review = s(&r["provider"]);
    if let Some(m) = r["model"].as_str() {
        review += &format!(" {m}");
    }
    if let Some(e) = r["effort"].as_str() {
        review += &format!(" · {e}");
    }
    lines.push(format!("review      {review} ({})", s(&r["source"])));
    lines.push(format!(
        "ci          {} ({})",
        s(&v["ci"]["mode"]),
        s(&v["ci"]["source"])
    ));
    lines.push(format!("on PATH     {}", detected_line(&detected)));
    for w in v["warnings"].as_array().into_iter().flatten() {
        lines.push(format!("warning: {}", s(w)));
    }
    for line in lines {
        say(out, &line)?;
    }
    Ok(0)
}

fn detected_line(detected: &[Detected]) -> String {
    if detected.is_empty() {
        return "none of claude, codex, cursor, agy, opencode".into();
    }
    detected
        .iter()
        .map(|d| format!("{} ({})", d.provider, d.binary.display()))
        .collect::<Vec<_>>()
        .join(", ")
}

struct Prompter<'a> {
    input: &'a mut dyn BufRead,
    out: &'a mut dyn Write,
    eof: bool,
    failed: bool,
}

impl Prompter<'_> {
    fn say(&mut self, line: &str) -> Result<()> {
        say(self.out, line)
    }

    /// An empty string for Enter, and for end of input.
    fn line(&mut self, prompt: &str) -> Result<String> {
        write!(self.out, "{prompt} ").map_err(io("stdout"))?;
        self.out.flush().map_err(io("stdout"))?;
        let mut s = String::new();
        if self.input.read_line(&mut s).map_err(io("stdin"))? == 0 {
            self.eof = true;
            self.say("")?;
        }
        Ok(s.trim().to_string())
    }

    fn yes(&mut self, prompt: &str) -> Result<bool> {
        let a = self.line(prompt)?;
        Ok(a.eq_ignore_ascii_case("y") || a.eq_ignore_ascii_case("yes"))
    }

    /// A numbered pick by number or name. With `free_text` any other answer is taken as typed; with `clearable` `-` clears.
    fn pick(
        &mut self,
        options: &[(String, String)],
        shown: &str,
        free_text: bool,
        clearable: bool,
    ) -> Result<Answer> {
        for (i, (_, label)) in options.iter().enumerate() {
            self.say(&format!("  {}) {label}", i + 1))?;
        }
        let ask = match (free_text, clearable) {
            (true, true) => "Number or name, - to clear",
            (true, false) => "Number or name",
            (false, true) => "Number, - to clear",
            (false, false) => "Number",
        };
        loop {
            let a = self.line(&format!("{ask} [{shown}]:"))?;
            let placeholder = a.eq_ignore_ascii_case(shown)
                && !options.iter().any(|(v, _)| v.eq_ignore_ascii_case(&a));
            if a.is_empty() || placeholder {
                return Ok(Answer::Keep);
            }
            if clearable && a == "-" {
                return Ok(Answer::Clear);
            }
            let by_number = a
                .parse::<usize>()
                .ok()
                .and_then(|n| n.checked_sub(1))
                .and_then(|i| options.get(i));
            let by_name = || options.iter().find(|(v, _)| v.eq_ignore_ascii_case(&a));
            if let Some((v, _)) = by_number.or_else(by_name) {
                return Ok(Answer::Set(v.clone()));
            }
            if free_text && provider::is_model_name(&a) {
                return Ok(Answer::Set(a));
            }
            self.say(&format!("`{a}` is not one of the choices"))?;
        }
    }
}

fn plain(values: &[&str]) -> Vec<(String, String)> {
    values
        .iter()
        .map(|v| (v.to_string(), v.to_string()))
        .collect()
}

fn provider_options(detected: &[Detected]) -> Vec<(String, String)> {
    Provider::ALL
        .into_iter()
        .map(|p| {
            let on_path = detected.iter().any(|d| d.provider == p);
            let label = if on_path {
                format!("{p} (on PATH)")
            } else {
                p.to_string()
            };
            (p.to_string(), label)
        })
        .collect()
}

/// Runs the wizard over `input`: every section, or the one named, saved in a single `config::patch`.
pub fn run_wizard(
    input: &mut dyn BufRead,
    out: &mut dyn Write,
    env: &Env,
    section: Option<&SetupSection>,
) -> Result<i32> {
    let loaded = config::load(&env.config_dir)?;
    let detected = provider::detect_installed_in(&env.path);
    let mut q = Prompter {
        input,
        out,
        eof: false,
        failed: false,
    };
    for w in &loaded.warnings {
        q.say(&format!("warning: {w}"))?;
    }
    q.say(&format!("On PATH: {}", detected_line(&detected)))?;
    q.say("Enter keeps the value in [brackets].")?;
    let wants = |name: &str| {
        section.is_none_or(|s| {
            name == match s {
                SetupSection::Cockpit { .. } => "cockpit",
                SetupSection::Workers { .. } => "workers",
                SetupSection::Review { .. } => "review",
                SetupSection::Ci { .. } => "ci",
            }
        })
    };
    let mut edits = Vec::new();
    if wants("cockpit") {
        edits.extend(cockpit_step(&mut q, &loaded, &detected, env)?);
    }
    if wants("workers") {
        edits.extend(workers_step(&mut q, &loaded, &detected, env)?);
    }
    if wants("review") {
        edits.extend(review_step(&mut q, &loaded, &detected, env)?);
    }
    if wants("ci") {
        edits.extend(ci_step(&mut q, &loaded)?);
    }
    if q.eof {
        q.say("input ended before the last prompt; nothing saved")?;
        return Ok(1);
    }
    let code = save(&env.config_dir, &edits, q.out)?;
    Ok(if q.failed { 1 } else { code })
}

fn cockpit_step(
    q: &mut Prompter,
    loaded: &Loaded,
    detected: &[Detected],
    env: &Env,
) -> Result<Vec<Patch>> {
    q.say("\nCockpit — the agent CLI you drive factory from")?;
    let current = loaded.config.cockpit.provider;
    let suggested = current.or_else(|| detected.first().map(|d| d.provider));
    let answer = q.pick(
        &provider_options(detected),
        suggested.map_or("not set", Provider::as_str),
        false,
        false,
    )?;
    let chosen = match answer {
        Answer::Set(v) => Some(v.parse::<Provider>().map_err(Error::Msg)?),
        Answer::Keep | Answer::Clear => suggested,
    };
    let mut edits = Vec::new();
    if let Some(p) = chosen
        && chosen != current
    {
        edits.push(set("cockpit", "provider", p.as_str()));
    }
    if chosen == Some(Provider::Claude) && q.yes("Install/update Claude Code cockpit mod? [y/N]")? {
        q.failed |= crate::cli::cockpit::install(&paths::data_dir().join("mods"), q.out)? != 0;
    }
    if let Some(repo) = env.repo.as_deref() {
        let verdict = skills::verdict(repo);
        if verdict != Verdict::None || repo.join(config::PROJECT_FILE).is_file() {
            q.say(&format!("skills: {}", verdict.lines().join("; ")))?;
        }
    }
    Ok(edits)
}

fn workers_step(
    q: &mut Prompter,
    loaded: &Loaded,
    detected: &[Detected],
    env: &Env,
) -> Result<Vec<Patch>> {
    q.say("\nWorkers — the agent CLI, model and effort unattended workers run with")?;
    let now = table_now(loaded, WorkerRole::Default);
    let suggested = now
        .own_provider
        .or_else(|| detected.first().map(|d| d.provider));
    q.say("Provider:")?;
    let a = q.pick(
        &provider_options(detected),
        suggested.map_or("not set", Provider::as_str),
        false,
        false,
    )?;
    let provider = match a {
        Answer::Keep | Answer::Clear => {
            suggested.map_or(Answer::Keep, |p| Answer::Set(p.to_string()))
        }
        set => set,
    };
    let p = match &provider {
        Answer::Set(v) => v.parse::<Provider>().map_err(Error::Msg)?,
        _ => now.provider,
    };
    let same = p == now.provider;
    q.say("Model (a number, or type any model):")?;
    let shown = now
        .model
        .as_deref()
        .filter(|_| same)
        .or(Some(p.default_model()).filter(|m| !m.is_empty()));
    let model = model_pick(q, p, env, shown, false)?;
    let effort = effort_pick(
        q,
        "Effort:",
        p,
        now.effort
            .as_deref()
            .filter(|_| same)
            .or(p.default_effort()),
        false,
    )?;
    let mut edits = worker_edits(&now, provider, p, model, effort);
    if !q.yes(
        "Override any role (build, fix, checkpoint, polish) with a different provider/model/effort? [y/N]",
    )? {
        return Ok(edits);
    }
    loop {
        let typed = q.line("Role (build, fix, checkpoint, polish; Enter when done):")?;
        if typed.is_empty() {
            break;
        }
        let Some(role) = [
            WorkerRole::Build,
            WorkerRole::Fix,
            WorkerRole::Checkpoint,
            WorkerRole::Polish,
        ]
        .into_iter()
        .find(|r| r.table().strip_prefix("workers.") == Some(typed.to_ascii_lowercase().as_str())) else {
            q.say(&format!("`{typed}` is not a role"))?;
            continue;
        };
        let name = &role.table()["workers.".len()..];
        let now = table_now(loaded, role);
        q.say(&format!("{name} provider:"))?;
        let provider = q.pick(
            &provider_options(detected),
            now.own_provider.map_or("inherit", Provider::as_str),
            false,
            true,
        )?;
        let rp = match &provider {
            Answer::Set(v) => v.parse::<Provider>().map_err(Error::Msg)?,
            Answer::Keep => now.own_provider.unwrap_or(p),
            Answer::Clear => p,
        };
        let same = rp == now.provider;
        q.say(&format!("{name} model:"))?;
        let shown = now.model.as_deref().filter(|_| same).unwrap_or("inherit");
        let model = model_pick(q, rp, env, Some(shown), true)?;
        let shown = now.effort.as_deref().filter(|_| same).unwrap_or("inherit");
        let effort = effort_pick(q, &format!("{name} effort:"), rp, Some(shown), true)?;
        edits.extend(worker_edits(&now, provider, rp, model, effort));
    }
    Ok(edits)
}

/// The CLI's own model list when it is on `PATH` and lists quickly, the curated one otherwise; with no `shown` value
/// the provider has no default, so Enter asks again.
fn model_pick(
    q: &mut Prompter,
    p: Provider,
    env: &Env,
    shown: Option<&str>,
    clearable: bool,
) -> Result<Answer> {
    let models = match provider::resolve_binary_in(p, &env.path) {
        Some(bin) => provider::discover_models(p, &bin, DISCOVER_TIMEOUT),
        None => p.curated_models().iter().map(|m| m.to_string()).collect(),
    };
    let models: Vec<(String, String)> = models.into_iter().map(|m| (m.clone(), m)).collect();
    loop {
        let a = q.pick(&models, shown.unwrap_or("none"), true, clearable)?;
        if a != Answer::Keep || shown.is_some() {
            return Ok(a);
        }
        if q.eof {
            return Err(msg(format!("{p} has no default model and none was chosen")));
        }
        q.say(&format!("{p} has no default model: pick or type one"))?;
    }
}

/// Skipped for a CLI that takes no effort level.
fn effort_pick(
    q: &mut Prompter,
    label: &str,
    p: Provider,
    shown: Option<&str>,
    clearable: bool,
) -> Result<Answer> {
    let levels = p.effort_levels();
    if levels.is_empty() {
        return Ok(Answer::Keep);
    }
    q.say(label)?;
    q.pick(
        &plain(levels),
        shown.unwrap_or("CLI default"),
        false,
        clearable,
    )
}

fn review_step(
    q: &mut Prompter,
    loaded: &Loaded,
    detected: &[Detected],
    env: &Env,
) -> Result<Vec<Patch>> {
    q.say("\nAdversarial review — the CLI that gives a second opinion, or none")?;
    let current = current_review(loaded);
    let suggested = current.or_else(|| {
        detected
            .first()
            .map(|d| ReviewProvider::Provider(d.provider))
    });
    let mut options = provider_options(detected);
    options.push(("none".into(), "none — no second opinion".into()));
    let answer = q.pick(
        &options,
        suggested.map_or("not set", ReviewProvider::as_str),
        false,
        false,
    )?;
    let chosen = match answer {
        Answer::Set(v) => Some(v.parse::<ReviewProvider>().map_err(Error::Msg)?),
        Answer::Keep | Answer::Clear => suggested,
    };
    let p = match chosen {
        None => return Ok(Vec::new()),
        Some(ReviewProvider::None) if current == chosen => return Ok(Vec::new()),
        Some(ReviewProvider::None) => return Ok(review_none_edits(loaded)),
        Some(ReviewProvider::Provider(p)) => p,
    };
    let same = current == chosen;
    let r = &loaded.config.review;
    let s = &loaded.sources.review;
    let model_now = (same && s.model != Source::Default).then_some(r.model.as_str());
    let models = match provider::resolve_binary_in(p, &env.path) {
        Some(bin) => provider::discover_models(p, &bin, DISCOVER_TIMEOUT),
        None => p.curated_models().iter().map(|m| m.to_string()).collect(),
    };
    let models: Vec<(String, String)> = models.into_iter().map(|m| (m.clone(), m)).collect();
    let fallback = model_now.unwrap_or(p.default_model());
    q.say("Model (a number, or type any model):")?;
    let model = loop {
        let a = q.pick(
            &models,
            if fallback.is_empty() {
                "none"
            } else {
                fallback
            },
            true,
            false,
        )?;
        if a != Answer::Keep || !fallback.is_empty() {
            break a;
        }
        if q.eof {
            return Err(msg(format!("{p} has no default model and none was chosen")));
        }
        q.say(&format!("{p} has no default model: pick or type one"))?;
    };
    let levels = p.effort_levels();
    let effort = if levels.is_empty() {
        Answer::Keep
    } else {
        q.say("Effort:")?;
        let effort_now = r.effort.as_deref().filter(|_| same);
        q.pick(
            &plain(levels),
            effort_now.unwrap_or("CLI default"),
            false,
            false,
        )?
    };
    Ok(review_edits(loaded, p, model, effort))
}

fn ci_step(q: &mut Prompter, loaded: &Loaded) -> Result<Vec<Patch>> {
    q.say("\nCI — where a run's checks run")?;
    let options = vec![
        (
            "local".to_string(),
            "local — your gate on this machine; saves org Actions minutes, costs local CPU"
                .to_string(),
        ),
        (
            "remote".to_string(),
            "remote — GitHub Actions on every PR with auto-fix rounds; lighter locally, burns Actions minutes"
                .to_string(),
        ),
    ];
    let current = loaded.config.ci.mode.map(CiMode::as_str);
    let a = q.pick(&options, current.unwrap_or("not set"), false, false)?;
    Ok(answer_patch("ci", "mode", &a, current)
        .into_iter()
        .collect())
}
