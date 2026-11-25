//! A missing file is the defaults; an unknown key warns, never fails.
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::provider::{Provider, ReviewProvider};
use crate::{Error, Result};

pub const DEFAULT_BUDGET: u64 = 30_000_000;
pub const DEFAULT_MODEL: &str = "opus";
pub const DEFAULT_EFFORT: &str = "high";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub budget_weighted_tokens: u64,
    pub workers: Workers,
    pub source: Option<SourceRepo>,
    pub review: Review,
    pub ci: Ci,
    pub cockpit: Cockpit,
}

/// Where a run's gate CI runs: on this machine or on the remote's checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum CiMode {
    Local,
    Remote,
}

impl CiMode {
    pub fn as_str(self) -> &'static str {
        match self {
            CiMode::Local => "local",
            CiMode::Remote => "remote",
        }
    }
}

impl std::fmt::Display for CiMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Ci {
    pub mode: Option<CiMode>,
}

/// The agent CLI the owner drives factory from.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Cockpit {
    pub provider: Option<Provider>,
}

/// `Source` is taken: it names the scope a knob came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceRepo {
    pub path: PathBuf,
}

/// The owner's own facts (a renamed binary, a rate-limited model) live here, never in the skills.
/// `resolve_review` picks the reviewer; `agent` and `model` keep their legacy defaults for a file that names no provider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Review {
    pub provider: Option<ReviewProvider>,
    pub agent: String,
    pub model: String,
    pub effort: Option<String>,
    pub timeout_secs: u64,
    pub pass_lines: u64,
}

/// The `[workers]` scalars are defaults every role inherits while it keeps their provider.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Workers {
    pub provider: Option<Provider>,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub build: Worker,
    pub fix: Worker,
    pub checkpoint: Worker,
    pub polish: Worker,
}

/// `model` and `effort` hold what a Claude worker runs with; `Workers::resolved` gives any provider's.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Worker {
    pub provider: Option<Provider>,
    pub model: String,
    pub effort: String,
}

/// What a role runs with once role, `[workers]` and provider defaults are layered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedWorker {
    pub provider: Provider,
    pub model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
}

impl ResolvedWorker {
    /// The provider's own model and effort.
    pub fn defaults(provider: Provider) -> ResolvedWorker {
        ResolvedWorker {
            provider,
            model: provider.default_model().to_string(),
            effort: provider.default_effort().map(str::to_string),
        }
    }
}

impl std::fmt::Display for ResolvedWorker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.provider, self.model)?;
        match &self.effort {
            Some(e) => write!(f, " · {e}"),
            None => Ok(()),
        }
    }
}

/// Every role's worker, fixed for one run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedWorkers {
    pub build: ResolvedWorker,
    pub fix: ResolvedWorker,
    pub checkpoint: ResolvedWorker,
    pub polish: ResolvedWorker,
}

impl ResolvedWorkers {
    pub fn uniform(w: ResolvedWorker) -> ResolvedWorkers {
        ResolvedWorkers {
            build: w.clone(),
            fix: w.clone(),
            checkpoint: w.clone(),
            polish: w,
        }
    }

    /// A merge runs as `build`, and so does any role this table does not name.
    pub fn for_role(&self, role: &str) -> &ResolvedWorker {
        match role {
            "fix" => &self.fix,
            "checkpoint" => &self.checkpoint,
            "polish" => &self.polish,
            _ => &self.build,
        }
    }

    pub fn roles(&self) -> [(&'static str, &ResolvedWorker); 4] {
        [
            ("build", &self.build),
            ("fix", &self.fix),
            ("checkpoint", &self.checkpoint),
            ("polish", &self.polish),
        ]
    }

    /// In role order, each once.
    pub fn distinct_providers(&self) -> Vec<Provider> {
        let mut out: Vec<Provider> = Vec::new();
        for (_, w) in self.roles() {
            if !out.contains(&w.provider) {
                out.extend([w.provider]);
            }
        }
        out
    }
}

impl Workers {
    pub fn role(&self, role: &str) -> Option<&Worker> {
        match role {
            "build" => Some(&self.build),
            "fix" => Some(&self.fix),
            "checkpoint" => Some(&self.checkpoint),
            "polish" => Some(&self.polish),
            _ => None,
        }
    }

    /// A role that switches provider never inherits the `[workers]` model or effort: slugs are provider-specific.
    pub fn resolved(
        &self,
        sources: &RoleSources,
        role: &str,
        fallback_provider: Provider,
    ) -> ResolvedWorker {
        let (worker, set) = match (self.role(role), sources.role(role)) {
            (Some(w), Some(s)) => (Some(w), s.clone()),
            _ => (None, WorkerSources::default()),
        };
        let explicit = |src: Source, v: Option<&String>| {
            (src != Source::Default).then(|| v.cloned()).flatten()
        };
        let role_provider = worker.and_then(|w| w.provider);
        let provider = role_provider.or(self.provider).unwrap_or(fallback_provider);
        let inherits = provider == self.provider.unwrap_or(fallback_provider);
        let model = explicit(set.model, worker.map(|w| &w.model))
            .or_else(|| self.model.clone().filter(|_| inherits))
            .unwrap_or_else(|| provider.default_model().to_string());
        let effort = if provider.effort_levels().is_empty() {
            None
        } else {
            explicit(set.effort, worker.map(|w| &w.effort))
                .or_else(|| self.effort.clone().filter(|_| inherits))
                .or_else(|| provider.default_effort().map(str::to_string))
        };
        ResolvedWorker {
            provider,
            model,
            effort,
        }
    }

    /// Whether the role takes this `[workers]` default rather than its own value or the provider's.
    fn inherited(&self, sources: &RoleSources, role: &str, model: bool) -> bool {
        let Some(s) = sources.role(role) else {
            return false;
        };
        let (own, default) = if model {
            (s.model, self.model.is_some())
        } else {
            (s.effort, self.effort.is_some())
        };
        let claude = |p: Option<Provider>| p.unwrap_or(Provider::Claude) == Provider::Claude;
        own == Source::Default
            && default
            && claude(self.provider)
            && claude(self.role(role).and_then(|w| w.provider).or(self.provider))
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            budget_weighted_tokens: DEFAULT_BUDGET,
            workers: Workers::default(),
            source: None,
            review: Review::default(),
            ci: Ci::default(),
            cockpit: Cockpit::default(),
        }
    }
}

impl Default for Review {
    fn default() -> Self {
        Review {
            provider: None,
            agent: "cursor-agent".into(),
            model: "grok-4.7-high".into(),
            effort: None,
            timeout_secs: 1800,
            pass_lines: 600,
        }
    }
}

impl Default for Worker {
    fn default() -> Self {
        Worker {
            provider: None,
            model: DEFAULT_MODEL.into(),
            effort: DEFAULT_EFFORT.into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Loaded {
    pub config: Config,
    pub path: PathBuf,
    pub warnings: Vec<String>,
    pub sources: Sources,
}

/// `Flag` is `--budget` or `--ci` on the command line.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    #[default]
    Default,
    User,
    Project,
    Flag,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Sources {
    pub budget: Source,
    pub workers: RoleSources,
    pub ci: Source,
    pub cockpit: Source,
    pub review: ReviewSources,
}

/// Which `[review]` keys the file set; `Default` means left out.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ReviewSources {
    pub provider: Source,
    pub agent: Source,
    pub model: Source,
    pub effort: Source,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct RoleSources {
    pub build: WorkerSources,
    pub fix: WorkerSources,
    pub checkpoint: WorkerSources,
    pub polish: WorkerSources,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
/// Set only by the role's own table: a `[workers]` default the role inherits stays `Default` here.
pub struct WorkerSources {
    pub provider: Source,
    pub model: Source,
    pub effort: Source,
}

const TOP_KEYS: [&str; 6] = [
    "budget_weighted_tokens",
    "workers",
    "source",
    "review",
    "ci",
    "cockpit",
];
const PROJECT_TOP_KEYS: [&str; 4] = ["docs", "lanes", "leaks", "run"];
const RUN_KEYS: [&str; 1] = ["budget"];
const ROLE_KEYS: [&str; 4] = ["build", "fix", "checkpoint", "polish"];
const WORKER_KEYS: [&str; 3] = ["provider", "model", "effort"];
const SOURCE_KEYS: [&str; 1] = ["path"];
const REVIEW_KEYS: [&str; 6] = [
    "provider",
    "agent",
    "model",
    "effort",
    "timeout_secs",
    "pass_lines",
];
const CI_KEYS: [&str; 1] = ["mode"];
const COCKPIT_KEYS: [&str; 1] = ["provider"];
pub const CI_MODE_ENV: &str = "FACTORY_CI_MODE";
pub const REVIEW_PROVIDER_ENV: &str = "FACTORY_REVIEW_PROVIDER";
pub const WORKER_PROVIDER_ENV: &str = "FACTORY_WORKER_PROVIDER";

pub const PROJECT_FILE: &str = "docs/project.toml";

pub fn load(config_dir: &Path) -> Result<Loaded> {
    let path = config_dir.join("config.toml");
    let mut loaded = Loaded {
        config: Config::default(),
        path: path.clone(),
        warnings: Vec::new(),
        sources: Sources::default(),
    };
    let Some(mut table) = read_table(&path)? else {
        return Ok(loaded);
    };
    loaded.warnings = warn(&path, unknown_keys(&table, &TOP_KEYS));
    if drop_pathless_source(&mut table) {
        loaded.warnings.push(format!(
            "{}: `[source]` has no `path` string, ignored",
            path.display()
        ));
    }
    loaded.sources.budget = scope(&table, &["budget_weighted_tokens"], Source::User);
    loaded.sources.ci = scope(&table, &["ci", "mode"], Source::User);
    loaded.sources.cockpit = scope(&table, &["cockpit", "provider"], Source::User);
    let r = &mut loaded.sources.review;
    for (key, s) in [
        ("provider", &mut r.provider),
        ("agent", &mut r.agent),
        ("model", &mut r.model),
        ("effort", &mut r.effort),
    ] {
        *s = scope(&table, &["review", key], Source::User);
    }
    for (role, s) in loaded.sources.workers.each_mut() {
        s.provider = scope(&table, &["workers", role, "provider"], Source::User);
        s.model = scope(&table, &["workers", role, "model"], Source::User);
        s.effort = scope(&table, &["workers", role, "effort"], Source::User);
    }
    loaded.config = toml::Value::Table(table)
        .try_into()
        .map_err(|source| Error::Toml {
            what: format!("read {}", path.display()),
            source,
        })?;
    if let Some(s) = &mut loaded.config.source {
        s.path = expand_home(&s.path);
    }
    inherit_worker_defaults(&mut loaded.config.workers, &loaded.sources.workers);
    Ok(loaded)
}

/// Today's workers are Claude sessions, so a role that keeps Claude takes the `[workers]` model and effort.
fn inherit_worker_defaults(workers: &mut Workers, sources: &RoleSources) {
    let resolved: Vec<ResolvedWorker> = ROLE_KEYS
        .iter()
        .map(|role| workers.resolved(sources, role, Provider::Claude))
        .collect();
    for ((role, worker), r) in [
        ("build", &mut workers.build),
        ("fix", &mut workers.fix),
        ("checkpoint", &mut workers.checkpoint),
        ("polish", &mut workers.polish),
    ]
    .into_iter()
    .zip(resolved)
    {
        if r.provider != Provider::Claude {
            continue;
        }
        let s = sources.role(role).expect("a known role");
        if s.model == Source::Default {
            worker.model = r.model;
        }
        if let (Source::Default, Some(e)) = (s.effort, r.effort) {
            worker.effort = e;
        }
    }
}

fn expand_home(path: &Path) -> PathBuf {
    match path.strip_prefix("~") {
        Ok(rest) => crate::paths::home().join(rest),
        Err(_) => path.to_path_buf(),
    }
}

/// `[docs]`, `[lanes]` and `[leaks]` are known here only so they do not warn; `docs::project` and `leaks` read them.
/// `[workers]` warns: model slugs are provider-specific, so they live only in the user's file.
pub fn load_with_project(config_dir: &Path, repo: &Path) -> Result<Loaded> {
    let mut loaded = load(config_dir)?;
    let path = repo.join(PROJECT_FILE);
    let Some(table) = read_table(&path)? else {
        return Ok(loaded);
    };
    loaded
        .warnings
        .extend(warn(&path, unknown_keys(&table, &PROJECT_TOP_KEYS)));
    let project: Project = toml::Value::Table(table)
        .try_into()
        .map_err(|source| Error::Toml {
            what: format!("read {}", path.display()),
            source,
        })?;
    if let Some(b) = project.run.budget {
        loaded.config.budget_weighted_tokens = b;
        loaded.sources.budget = Source::Project;
    }
    Ok(loaded)
}

pub fn knobs(loaded: &Loaded, lanes: Option<u32>, claude_version: Option<&str>) -> Value {
    let w = &loaded.config.workers;
    let s = &loaded.sources.workers;
    let mut workers = serde_json::Map::new();
    for (role, worker, src) in [
        ("build", &w.build, &s.build),
        ("fix", &w.fix, &s.fix),
        ("checkpoint", &w.checkpoint, &s.checkpoint),
        ("polish", &w.polish, &s.polish),
    ] {
        let from = |own: Source, model: bool| {
            if w.inherited(s, role, model) {
                Source::User
            } else {
                own
            }
        };
        let (model_source, effort_source) = (from(src.model, true), from(src.effort, false));
        let mut o = json!({"model": worker.model, "effort": worker.effort, "source": model_source});
        if effort_source != model_source {
            o["effort_source"] = json!(effort_source);
        }
        workers.insert(role.to_string(), o);
    }
    json!({
        "workers": workers,
        "budget": {"value": loaded.config.budget_weighted_tokens, "source": loaded.sources.budget},
        "ci": {"value": loaded.config.ci.mode, "source": loaded.sources.ci},
        "lanes": lanes,
        "factory_version": env!("CARGO_PKG_VERSION"),
        "claude_version": claude_version,
    })
}

/// `--ci`, then the user's `[ci] mode`, then `FACTORY_CI_MODE`; the env is read here so `load` stays file-only.
/// The third value warns of an env value that names no mode.
pub fn resolve_ci_mode(
    loaded: &Loaded,
    flag: Option<CiMode>,
) -> (Option<CiMode>, Source, Option<String>) {
    let env = std::env::var(CI_MODE_ENV).ok();
    ci_mode_from(loaded, flag, env.as_deref())
}

fn ci_mode_from(
    loaded: &Loaded,
    flag: Option<CiMode>,
    env: Option<&str>,
) -> (Option<CiMode>, Source, Option<String>) {
    if let Some(m) = flag {
        return (Some(m), Source::Flag, None);
    }
    if let Some(m) = loaded.config.ci.mode {
        return (Some(m), Source::User, None);
    }
    let raw = env.unwrap_or("");
    let v = raw.trim();
    if v.is_empty() {
        return (None, Source::Default, None);
    }
    match [CiMode::Local, CiMode::Remote]
        .into_iter()
        .find(|m| v.eq_ignore_ascii_case(m.as_str()))
    {
        Some(m) => (Some(m), Source::Default, None),
        None => (
            None,
            Source::Default,
            Some(format!(
                "{CI_MODE_ENV}={raw:?} is neither `local` nor `remote`, ignored"
            )),
        ),
    }
}

/// The reviewer a run gets a second opinion from, fully resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedReview {
    pub provider: ReviewProvider,
    /// The owner's own binary for the provider, when `[review] agent` names one.
    pub binary: Option<String>,
    pub model: String,
    pub effort: Option<String>,
    pub timeout_secs: u64,
    pub pass_lines: u64,
}

/// `[review] provider`, then a legacy `[review] agent`, then `FACTORY_REVIEW_PROVIDER`; `None` means nobody chose.
/// The third value warns of an env value that names no provider.
pub fn resolve_review(loaded: &Loaded) -> (Option<ResolvedReview>, Source, Option<String>) {
    let env = std::env::var(REVIEW_PROVIDER_ENV).ok();
    review_from(loaded, env.as_deref())
}

fn review_from(
    loaded: &Loaded,
    env: Option<&str>,
) -> (Option<ResolvedReview>, Source, Option<String>) {
    let r = &loaded.config.review;
    let set = &loaded.sources.review;
    let agent = (set.agent != Source::Default).then(|| r.agent.clone());
    let (provider, source) = if let Some(p) = r.provider {
        (p, Source::User)
    } else if let Some(a) = &agent {
        let p = Provider::infer_from_binary(a).unwrap_or(Provider::Cursor);
        (ReviewProvider::Provider(p), Source::User)
    } else {
        let raw = env.unwrap_or("");
        let v = raw.trim();
        if v.is_empty() {
            return (None, Source::Default, None);
        }
        match v.parse::<ReviewProvider>() {
            Ok(p) => (p, Source::Default),
            Err(_) => {
                return (
                    None,
                    Source::Default,
                    Some(format!(
                        "{REVIEW_PROVIDER_ENV}={raw:?} names no reviewer (claude, codex, cursor, agy, opencode or none), ignored"
                    )),
                );
            }
        }
    };
    let (binary, model, effort) = match provider {
        ReviewProvider::None => (None, String::new(), None),
        ReviewProvider::Provider(p) => {
            let model = if set.model != Source::Default {
                r.model.clone()
            } else {
                p.default_model().to_string()
            };
            let effort = r.effort.clone().filter(|_| !p.effort_levels().is_empty());
            let binary = agent.filter(|a| Provider::infer_from_binary(a).is_none_or(|n| n == p));
            (binary, model, effort)
        }
    };
    let resolved = ResolvedReview {
        provider,
        binary,
        model,
        effort,
        timeout_secs: r.timeout_secs,
        pass_lines: r.pass_lines,
    };
    (Some(resolved), source, None)
}

/// One `--worker` flag: every role, or the one it names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerSpec {
    pub role: Option<String>,
    pub provider: Provider,
    pub model: Option<String>,
    pub effort: Option<String>,
}

/// `[<role>=]<provider>[:<model>[:<effort>]]`.
pub fn parse_worker_spec(raw: &str) -> std::result::Result<WorkerSpec, String> {
    let raw = raw.trim();
    let (role, rest) = match raw.split_once('=') {
        Some((r, rest)) if !r.contains(':') => {
            let r = r.trim().to_ascii_lowercase();
            if !ROLE_KEYS.contains(&r.as_str()) {
                return Err(format!(
                    "`{r}` is not a worker role: expected build, fix, checkpoint or polish"
                ));
            }
            (Some(r), rest)
        }
        _ => (None, raw),
    };
    let (provider, model, effort) = parse_provider_choice(rest)?;
    Ok(WorkerSpec {
        role,
        provider,
        model,
        effort,
    })
}

/// `<provider>[:<model>[:<effort>]]`; the model is `None` when left out and the provider has a default.
/// A model may hold `:` itself: only a trailing level some provider knows is read as the effort, and it must be this
/// provider's.
pub fn parse_provider_choice(
    raw: &str,
) -> std::result::Result<(Provider, Option<String>, Option<String>), String> {
    let (name, rest) = raw.split_once(':').unwrap_or((raw, ""));
    let p: Provider = name.trim().parse()?;
    let rest = rest.trim();
    let (model, effort) = match rest.rsplit_once(':') {
        Some((m, e)) if p.effort_levels().contains(&e.trim()) => (m.trim(), Some(e.trim())),
        Some((_, e)) if is_effort_word(e.trim()) => {
            let e = e.trim();
            return Err(if p.effort_levels().is_empty() {
                format!("{p} takes no effort level, not {e:?}")
            } else {
                format!(
                    "{e:?} is not a {p} effort ({})",
                    p.effort_levels().join(", ")
                )
            });
        }
        Some((m, "")) => (m.trim(), None),
        _ => (rest, None),
    };
    let model = match model {
        "" if p.default_model().is_empty() => {
            return Err(format!(
                "{p} has no default model, so name one as `{p}:<model>`"
            ));
        }
        "" => None,
        m if !crate::provider::is_model_name(m) => return Err(format!("{m:?} is not a model id")),
        m => Some(m.to_string()),
    };
    Ok((p, model, effort.map(str::to_string)))
}

fn is_effort_word(e: &str) -> bool {
    Provider::ALL.iter().any(|p| p.effort_levels().contains(&e))
}

/// Per role: its `--worker`, an all-role `--worker`, `[workers.<role>] provider`, `[workers] provider`, then
/// `FACTORY_WORKER_PROVIDER`. `None` when any role is left without a provider; the second value warns of an env value
/// that names none.
pub fn resolve_workers(
    loaded: &Loaded,
    specs: &[WorkerSpec],
) -> (Option<ResolvedWorkers>, Option<String>) {
    let env = std::env::var(WORKER_PROVIDER_ENV).ok();
    resolve_workers_from(loaded, specs, env.as_deref())
}

pub fn resolve_workers_from(
    loaded: &Loaded,
    specs: &[WorkerSpec],
    env: Option<&str>,
) -> (Option<ResolvedWorkers>, Option<String>) {
    let raw = env.unwrap_or("");
    let (env_provider, warning) = match raw.trim() {
        "" => (None, None),
        v => match v.parse::<Provider>() {
            Ok(p) => (Some(p), None),
            Err(_) => (
                None,
                Some(format!(
                    "{WORKER_PROVIDER_ENV}={raw:?} names no worker provider (claude, codex, cursor, agy or opencode), ignored"
                )),
            ),
        },
    };
    let w = &loaded.config.workers;
    let default_provider = w.provider.or(env_provider);
    let role = |role: &str| {
        let spec = specs
            .iter()
            .rev()
            .find(|s| s.role.as_deref() == Some(role))
            .or_else(|| specs.iter().rev().find(|s| s.role.is_none()));
        resolve_role(w, &loaded.sources.workers, role, spec, default_provider)
    };
    let resolved = (|| {
        Some(ResolvedWorkers {
            build: role("build")?,
            fix: role("fix")?,
            checkpoint: role("checkpoint")?,
            polish: role("polish")?,
        })
    })();
    (resolved, warning)
}

/// A model or effort is taken only from a layer that names the provider the role resolved to.
fn resolve_role(
    w: &Workers,
    sources: &RoleSources,
    role: &str,
    spec: Option<&WorkerSpec>,
    default_provider: Option<Provider>,
) -> Option<ResolvedWorker> {
    let (worker, set) = (w.role(role)?, sources.role(role)?);
    let config_provider = worker.provider.or(default_provider);
    let provider = spec.map(|s| s.provider).or(config_provider)?;
    let own = config_provider == Some(provider);
    let inherits = default_provider == Some(provider);
    let model = spec
        .and_then(|s| s.model.clone())
        .or_else(|| (own && set.model != Source::Default).then(|| worker.model.clone()))
        .or_else(|| w.model.clone().filter(|_| inherits))
        .unwrap_or_else(|| provider.default_model().to_string());
    let effort = if provider.effort_levels().is_empty() {
        None
    } else {
        spec.and_then(|s| s.effort.clone())
            .or_else(|| (own && set.effort != Source::Default).then(|| worker.effort.clone()))
            .or_else(|| w.effort.clone().filter(|_| inherits))
            .or_else(|| provider.default_effort().map(str::to_string))
    };
    Some(ResolvedWorker {
        provider,
        model,
        effort,
    })
}

/// Sets `[ci] mode` in `config_dir/config.toml`, keeping every other line as written.
pub fn write_ci_mode(config_dir: &Path, mode: CiMode) -> Result<PathBuf> {
    patch(
        config_dir,
        &[Patch {
            table: "ci".into(),
            key: "mode".into(),
            value: Some(toml::Value::String(mode.as_str().into())),
        }],
    )
}

/// One scalar key under a table header; `value: None` removes the key.
#[derive(Debug, Clone, PartialEq)]
pub struct Patch {
    /// Dotted, as in its header: `workers.build`.
    pub table: String,
    pub key: String,
    pub value: Option<toml::Value>,
}

/// Applies every edit to `config_dir/config.toml` in one atomic write, keeping every other line as written.
/// A symlinked `config.toml` keeps its link: the file it points at is the one rewritten, with its permissions.
/// An edit the lines cannot express, such as a key inside an inline table, refuses and leaves the file alone.
pub fn patch(config_dir: &Path, edits: &[Patch]) -> Result<PathBuf> {
    fs::create_dir_all(config_dir)
        .map_err(crate::error::io(format!("create {}", config_dir.display())))?;
    let path = config_dir.join("config.toml");
    let (target, old, perms) = match fs::canonicalize(&path) {
        Ok(target) => {
            let old = fs::read_to_string(&target)
                .map_err(crate::error::io(format!("read {}", target.display())))?;
            let perms = fs::metadata(&target)
                .map_err(crate::error::io(format!("stat {}", target.display())))?
                .permissions();
            (target, old, Some(perms))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (path.clone(), String::new(), None),
        Err(e) => return Err(crate::error::io(format!("read {}", path.display()))(e)),
    };
    let mut text = old;
    let mut refused = None;
    for (i, edit) in edits.iter().enumerate() {
        text = patched_text(&text, edit);
        let holds = |t: &toml::Table, e: &Patch| lookup(t, &e.table, &e.key) == e.value.as_ref();
        let ok = toml::from_str::<toml::Table>(&text).is_ok_and(|t| {
            edits[..=i].iter().enumerate().all(|(j, e)| {
                let later = edits[j + 1..=i]
                    .iter()
                    .any(|l| (&l.table, &l.key) == (&e.table, &e.key));
                later || holds(&t, e)
            })
        });
        if !ok {
            refused = Some(edit);
            break;
        }
    }
    if let Some(edit) = refused {
        let table = &edit.table;
        let by_hand = match &edit.value {
            Some(v) => format!("set `{} = {v}` under `[{table}]`", edit.key),
            None => format!("remove `{}` from `[{table}]`", edit.key),
        };
        return Err(Error::Msg(format!(
            "{} not changed: its `[{table}]` table could not be patched line by line — {by_hand} by hand",
            path.display()
        )));
    }
    let dir = target.parent().unwrap_or(config_dir);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let tmp = dir.join(format!(".config.toml.{}.{nanos}.tmp", std::process::id()));
    let written = write_synced(&tmp, &text, perms)
        .map_err(crate::error::io(format!("write {}", tmp.display())))
        .and_then(|()| {
            fs::rename(&tmp, &target).map_err(crate::error::io(format!(
                "rename {} over {}",
                tmp.display(),
                target.display()
            )))
        });
    if written.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    written.map(|()| path)
}

fn write_synced(path: &Path, text: &str, perms: Option<fs::Permissions>) -> std::io::Result<()> {
    use std::io::Write as _;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    if let Some(p) = perms {
        file.set_permissions(p)?;
    }
    file.write_all(text.as_bytes())?;
    file.sync_all()
}

fn lookup<'a>(table: &'a toml::Table, dotted: &str, key: &str) -> Option<&'a toml::Value> {
    let mut t = table;
    for part in dotted.split('.').filter(|p| !p.is_empty()) {
        t = t.get(part)?.as_table()?;
    }
    t.get(key)
}

/// The table a header line opens, its dotted parts unquoted; an array-of-tables header names no table.
fn table_header(line: &str) -> Option<String> {
    let t = line.trim();
    if t.starts_with("[[") {
        return Some(String::new());
    }
    let inner = t.strip_prefix('[')?;
    let name = &inner[..inner.find(']')?];
    Some(
        name.split('.')
            .map(|p| p.trim().trim_matches(['"', '\'']))
            .collect::<Vec<_>>()
            .join("."),
    )
}

fn is_key(line: &str, key: &str) -> bool {
    line.split_once('=').is_some_and(|(k, _)| {
        let k = k.trim();
        k == key || k.trim_matches(['"', '\'']) == key && k.len() == key.len() + 2
    })
}

fn is_entry(line: &str) -> bool {
    let t = line.trim();
    !t.is_empty() && !t.starts_with('#')
}

fn patched_text(old: &str, edit: &Patch) -> String {
    let entry = edit.value.as_ref().map(|v| format!("{} = {v}", edit.key));
    let mut lines: Vec<String> = old.lines().map(str::to_string).collect();
    let start = if edit.table.is_empty() {
        Some(0)
    } else {
        lines
            .iter()
            .position(|l| table_header(l).as_deref() == Some(edit.table.as_str()))
            .map(|i| i + 1)
    };
    let Some(start) = start else {
        let Some(entry) = entry else {
            return old.to_string();
        };
        let mut text = old.to_string();
        if !text.is_empty() {
            if !text.ends_with('\n') {
                text += "\n";
            }
            text += "\n";
        }
        return text + &format!("[{}]\n{entry}\n", edit.table);
    };
    let end = lines[start..]
        .iter()
        .position(|l| table_header(l).is_some())
        .map_or(lines.len(), |i| start + i);
    match ((start..end).find(|&i| is_key(&lines[i], &edit.key)), entry) {
        (Some(i), Some(entry)) => lines[i] = entry,
        (Some(i), None) => {
            lines.remove(i);
        }
        (None, Some(entry)) => {
            let after = (start..end)
                .rev()
                .find(|&i| is_entry(&lines[i]))
                .map_or(start, |i| i + 1);
            lines.insert(after, entry);
        }
        (None, None) => return old.to_string(),
    }
    lines.join("\n") + "\n"
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Project {
    run: ProjectRun,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ProjectRun {
    budget: Option<u64>,
}

impl RoleSources {
    pub fn role(&self, role: &str) -> Option<&WorkerSources> {
        match role {
            "build" => Some(&self.build),
            "fix" => Some(&self.fix),
            "checkpoint" => Some(&self.checkpoint),
            "polish" => Some(&self.polish),
            _ => None,
        }
    }

    fn each_mut(&mut self) -> [(&'static str, &mut WorkerSources); 4] {
        [
            ("build", &mut self.build),
            ("fix", &mut self.fix),
            ("checkpoint", &mut self.checkpoint),
            ("polish", &mut self.polish),
        ]
    }
}

fn read_table(path: &Path) -> Result<Option<toml::Table>> {
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(crate::error::io(format!("read {}", path.display()))(e)),
    };
    toml::from_str(&text)
        .map(Some)
        .map_err(|source| Error::Toml {
            what: format!("parse {}", path.display()),
            source,
        })
}

fn scope(table: &toml::Table, at: &[&str], scope: Source) -> Source {
    let mut v = table.get(at[0]);
    for k in &at[1..] {
        v = v.and_then(|v| v.get(*k));
    }
    if v.is_some() { scope } else { Source::Default }
}

/// Taken out before the typed read, so a bad `[source]` warns instead of failing every command.
fn drop_pathless_source(table: &mut toml::Table) -> bool {
    let pathless = table
        .get("source")
        .is_some_and(|s| !s.get("path").is_some_and(toml::Value::is_str));
    if pathless {
        table.remove("source");
    }
    pathless
}

fn warn(path: &Path, keys: Vec<String>) -> Vec<String> {
    keys.into_iter()
        .map(|k| format!("{}: unknown key `{k}` ignored", path.display()))
        .collect()
}

fn unknown_keys(table: &toml::Table, top: &[&str]) -> Vec<String> {
    let mut out: Vec<String> = table
        .keys()
        .filter(|k| !top.contains(&k.as_str()))
        .cloned()
        .collect();
    if let (true, Some(toml::Value::Table(run))) = (top.contains(&"run"), table.get("run")) {
        out.extend(
            run.keys()
                .filter(|k| !RUN_KEYS.contains(&k.as_str()))
                .map(|k| format!("run.{k}")),
        );
    }
    for (name, keys) in [
        ("source", &SOURCE_KEYS[..]),
        ("review", &REVIEW_KEYS[..]),
        ("ci", &CI_KEYS[..]),
        ("cockpit", &COCKPIT_KEYS[..]),
    ] {
        if let (true, Some(toml::Value::Table(t))) = (top.contains(&name), table.get(name)) {
            out.extend(
                t.keys()
                    .filter(|k| !keys.contains(&k.as_str()))
                    .map(|k| format!("{name}.{k}")),
            );
        }
    }
    if let (true, Some(toml::Value::Table(workers))) =
        (top.contains(&"workers"), table.get("workers"))
    {
        for (role, v) in workers {
            if WORKER_KEYS.contains(&role.as_str()) {
                continue;
            }
            if !ROLE_KEYS.contains(&role.as_str()) {
                out.push(format!("workers.{role}"));
                continue;
            }
            if let toml::Value::Table(w) = v {
                out.extend(
                    w.keys()
                        .filter(|k| !WORKER_KEYS.contains(&k.as_str()))
                        .map(|k| format!("workers.{role}.{k}")),
                );
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dirs(user: Option<&str>, project: Option<&str>) -> (tempfile::TempDir, tempfile::TempDir) {
        let (cfg, repo) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        if let Some(t) = user {
            fs::write(cfg.path().join("config.toml"), t).unwrap();
        }
        if let Some(t) = project {
            fs::create_dir_all(repo.path().join("docs")).unwrap();
            fs::write(repo.path().join(PROJECT_FILE), t).unwrap();
        }
        (cfg, repo)
    }

    #[test]
    fn the_project_file_overrides_the_budget_and_never_the_workers() {
        let (cfg, repo) = dirs(
            Some("[workers.polish]\nmodel = \"sonnet\"\neffort = \"low\"\n"),
            Some("[workers.polish]\nmodel = \"haiku\"\n\n[run]\nbudget = 7\n"),
        );
        let l = load_with_project(cfg.path(), repo.path()).unwrap();
        assert_eq!(l.config.workers.polish.model, "sonnet");
        assert_eq!(l.sources.workers.polish.model, Source::User);
        assert_eq!(l.config.workers.polish.effort, "low");
        assert_eq!(l.sources.workers.polish.effort, Source::User);
        assert_eq!(l.config.workers.build.model, DEFAULT_MODEL);
        assert_eq!(l.sources.workers.build.model, Source::Default);
        assert_eq!(l.sources.workers.build.effort, Source::Default);
        assert_eq!(l.config.budget_weighted_tokens, 7);
        assert_eq!(l.sources.budget, Source::Project);
        assert_eq!(l.warnings.len(), 1, "{:?}", l.warnings);
        assert!(
            l.warnings[0].contains(PROJECT_FILE)
                && l.warnings[0].contains("unknown key `workers` ignored"),
            "{:?}",
            l.warnings
        );

        let k = knobs(&l, Some(2), Some("2.1.296 (Claude Code)"));
        assert_eq!(
            k["workers"]["polish"],
            json!({"model": "sonnet", "effort": "low", "source": "user"})
        );
        assert_eq!(
            k["workers"]["build"],
            json!({"model": "opus", "effort": "high", "source": "default"})
        );
        assert_eq!(k["budget"], json!({"value": 7, "source": "project"}));
        assert_eq!(k["lanes"], 2);
        assert_eq!(k["factory_version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(k["claude_version"], "2.1.296 (Claude Code)");
    }

    #[test]
    fn config_loads_multi_provider_schema_maps_legacy_agent_and_ignores_project_workers() {
        let user = "[cockpit]\nprovider = \"codex\"\n\n\
                    [workers]\nprovider = \"claude\"\nmodel = \"sonnet\"\neffort = \"medium\"\n\n\
                    [workers.build]\nprovider = \"codex\"\n\n\
                    [workers.fix]\nprovider = \"cursor\"\neffort = \"max\"\n\n\
                    [workers.checkpoint]\neffort = \"low\"\n\n\
                    [review]\nprovider = \"agy\"\nmodel = \"gemini-x\"\neffort = \"xhigh\"\n\n\
                    [ci]\nmode = \"local\"\n";
        let (cfg, repo) = dirs(
            Some(user),
            Some("[workers]\nmodel = \"haiku\"\n[workers.polish]\nmodel = \"haiku\"\n"),
        );
        let l = load_with_project(cfg.path(), repo.path()).unwrap();
        assert_eq!(l.warnings.len(), 1, "{:?}", l.warnings);
        assert!(
            l.warnings[0].contains(PROJECT_FILE)
                && l.warnings[0].contains("unknown key `workers` ignored"),
            "{:?}",
            l.warnings
        );
        assert_eq!(l.config.workers, load(cfg.path()).unwrap().config.workers);
        assert_eq!(l.config.cockpit.provider, Some(Provider::Codex));
        assert_eq!(l.sources.cockpit, Source::User);
        assert_eq!(l.config.ci.mode, Some(CiMode::Local));

        let w = &l.config.workers;
        let s = &l.sources.workers;
        let resolved = |role| w.resolved(s, role, Provider::Claude);
        let want = |p, m: &str, e: Option<&str>| ResolvedWorker {
            provider: p,
            model: m.to_string(),
            effort: e.map(str::to_string),
        };
        assert_eq!(
            resolved("build"),
            want(Provider::Codex, "gpt-5.4", Some("high"))
        );
        assert_eq!(
            resolved("fix"),
            want(Provider::Cursor, "grok-4.7-high", None)
        );
        assert_eq!(
            resolved("checkpoint"),
            want(Provider::Claude, "sonnet", Some("low"))
        );
        assert_eq!(
            resolved("polish"),
            want(Provider::Claude, "sonnet", Some("medium"))
        );
        assert_eq!(
            (w.polish.model.as_str(), w.polish.effort.as_str()),
            ("sonnet", "medium")
        );
        assert_eq!(w.checkpoint.effort, "low");
        let k = knobs(&l, None, None);
        assert_eq!(
            k["workers"]["polish"],
            json!({"model": "sonnet", "effort": "medium", "source": "user"})
        );

        let (cfg, _) = dirs(
            Some("[workers]\nmodel = \"sonnet\"\n[workers.build]\nprovider = \"claude\"\n"),
            None,
        );
        let l = load(cfg.path()).unwrap();
        let (w, s) = (&l.config.workers, &l.sources.workers);
        assert_eq!(w.resolved(s, "build", Provider::Claude).model, "sonnet");
        assert_eq!(w.resolved(s, "build", Provider::Codex).model, "opus");
        assert_eq!(w.resolved(s, "fix", Provider::Codex).model, "sonnet");
        assert_eq!(
            w.resolved(s, "fix", Provider::Opencode),
            want(Provider::Opencode, "sonnet", None)
        );

        let (rev, source, warning) = review_from(&load(cfg.path()).unwrap(), None);
        assert_eq!((rev, source, warning), (None, Source::Default, None));

        let (cfg, _) = dirs(Some(user), None);
        let (rev, source, warning) = review_from(&load(cfg.path()).unwrap(), Some("none"));
        assert_eq!((source, warning), (Source::User, None));
        assert_eq!(
            rev.unwrap(),
            ResolvedReview {
                provider: ReviewProvider::Provider(Provider::Agy),
                binary: None,
                model: "gemini-x".into(),
                effort: Some("xhigh".into()),
                timeout_secs: 1800,
                pass_lines: 600,
            }
        );

        for (agent, want) in [
            ("cursor-agent", Provider::Cursor),
            ("/opt/bin/agent", Provider::Cursor),
            ("codex", Provider::Codex),
            ("my-wrapper", Provider::Cursor),
        ] {
            let (cfg, _) = dirs(
                Some(&format!(
                    "[review]\nagent = \"{agent}\"\neffort = \"high\"\n"
                )),
                None,
            );
            let l = load(cfg.path()).unwrap();
            assert!(l.warnings.is_empty(), "{:?}", l.warnings);
            let (rev, source, warning) = review_from(&l, Some("claude"));
            assert_eq!((source, warning), (Source::User, None), "{agent}");
            let rev = rev.unwrap();
            assert_eq!(rev.provider, ReviewProvider::Provider(want), "{agent}");
            assert_eq!(rev.binary.as_deref(), Some(agent));
            assert_eq!(rev.model, want.default_model());
            let effort = (want == Provider::Codex).then(|| "high".to_string());
            assert_eq!(rev.effort, effort, "{agent}");
        }

        let (cfg, _) = dirs(None, None);
        let unset = load(cfg.path()).unwrap();
        let (rev, source, warning) = review_from(&unset, Some(" Codex "));
        assert_eq!((source, warning), (Source::Default, None));
        let rev = rev.unwrap();
        assert_eq!(rev.provider, ReviewProvider::Provider(Provider::Codex));
        assert_eq!((rev.binary, rev.model.as_str()), (None, "gpt-5.4"));
        let (rev, _, _) = review_from(&unset, Some("NONE"));
        assert_eq!(rev.unwrap().provider, ReviewProvider::None);
        assert_eq!(
            review_from(&unset, Some("  ")),
            (None, Source::Default, None)
        );
        let (rev, source, warning) = review_from(&unset, Some("gemini"));
        assert_eq!((rev, source), (None, Source::Default));
        assert!(warning.unwrap().contains(REVIEW_PROVIDER_ENV));

        let (cfg, _) = dirs(
            Some(
                "[cockpit]\nprovider = \"codex\"\ntheme = 1\n\n[workers]\nprovider = \"claude\"\nlanes = 2\n\n\
                 [workers.build]\nprovider = \"codex\"\nvariant = \"x\"\n\n[review]\nprovider = \"none\"\nflavor = 1\n",
            ),
            None,
        );
        let l = load(cfg.path()).unwrap();
        let warned: Vec<&str> = l
            .warnings
            .iter()
            .map(|w| w.rsplit('`').nth(1).unwrap())
            .collect();
        assert_eq!(
            warned,
            [
                "review.flavor",
                "cockpit.theme",
                "workers.build.variant",
                "workers.lanes"
            ]
        );
        assert_eq!(l.config.review.provider, Some(ReviewProvider::None));
        let (rev, _, _) = review_from(&l, None);
        assert_eq!(rev.unwrap().model, "");

        let (cfg, _) = dirs(Some("[review]\nprovider = \"gemini\"\n"), None);
        assert!(matches!(load(cfg.path()), Err(Error::Toml { .. })));
    }

    #[test]
    fn a_review_agent_runs_only_for_the_provider_it_names() {
        let binary = |toml: &str| {
            let (cfg, _) = dirs(Some(toml), None);
            review_from(&load(cfg.path()).unwrap(), None)
                .0
                .unwrap()
                .binary
        };
        assert_eq!(
            binary("[review]\nprovider = \"codex\"\nagent = \"cursor-agent\"\n"),
            None
        );
        assert_eq!(
            binary("[review]\nprovider = \"codex\"\nagent = \"/opt/bin/codex\"\n"),
            Some("/opt/bin/codex".into())
        );
        assert_eq!(
            binary("[review]\nprovider = \"claude\"\nagent = \"/opt/bin/claude-work\"\n"),
            Some("/opt/bin/claude-work".into())
        );
        assert_eq!(
            binary("[review]\nagent = \"agent\"\n"),
            Some("agent".into())
        );
    }

    #[test]
    fn an_unknown_project_table_warns_once_and_never_fails() {
        let (cfg, repo) = dirs(None, Some("[deploy]\nfoo = 1\n"));
        let l = load_with_project(cfg.path(), repo.path()).unwrap();
        assert_eq!(l.warnings.len(), 1, "{:?}", l.warnings);
        assert!(l.warnings[0].contains(PROJECT_FILE), "{}", l.warnings[0]);
        assert!(l.warnings[0].contains("`deploy`"), "{}", l.warnings[0]);
        assert_eq!(l.config, Config::default());
    }

    #[test]
    fn a_lanes_table_loads_without_a_warning() {
        let (cfg, repo) = dirs(None, Some("[docs]\nframework = 3\n\n[lanes]\nseed = []\n"));
        let l = load_with_project(cfg.path(), repo.path()).unwrap();
        assert!(l.warnings.is_empty(), "{:?}", l.warnings);
    }

    #[test]
    fn a_leaks_table_loads_without_a_warning() {
        let text = "[docs]\nframework = 3\n\n[leaks]\nallow = [{ path = \"tests/lanes.rs\" }]\n";
        let (cfg, repo) = dirs(None, Some(text));
        let l = load_with_project(cfg.path(), repo.path()).unwrap();
        assert!(l.warnings.is_empty(), "{:?}", l.warnings);
    }

    #[test]
    fn no_project_file_is_the_user_load() {
        let user = "budget_weighted_tokens = 5\n[workers.fix]\nmodel = \"sonnet\"\n";
        let (cfg, repo) = dirs(Some(user), None);
        let a = load_with_project(cfg.path(), repo.path()).unwrap();
        let b = load(cfg.path()).unwrap();
        assert_eq!(a.config, b.config);
        assert_eq!(a.sources, b.sources);
        assert_eq!(a.warnings, b.warnings);
        assert_eq!(a.sources.budget, Source::User);
        assert_eq!(a.sources.workers.fix.model, Source::User);
        assert_eq!(a.sources.workers.fix.effort, Source::Default);
        let k = knobs(&a, None, None);
        assert!(k["lanes"].is_null() && k["claude_version"].is_null());
    }

    #[test]
    fn the_source_path_is_read_and_its_tilde_expanded() {
        let (cfg, _) = dirs(None, None);
        assert_eq!(load(cfg.path()).unwrap().config.source, None);
        for (written, want) in [
            ("/x", PathBuf::from("/x")),
            ("~/p", crate::paths::home().join("p")),
        ] {
            let (cfg, _) = dirs(Some(&format!("[source]\npath = \"{written}\"\n")), None);
            let l = load(cfg.path()).unwrap();
            assert_eq!(l.config.source.map(|s| s.path), Some(want));
            assert!(l.warnings.is_empty(), "{:?}", l.warnings);
        }
    }

    #[test]
    fn a_source_without_a_path_warns_and_loads_as_none() {
        for written in [
            "[source]\ndir = \"/x\"\n",
            "[source]\npath = 1\n",
            "source = \"/x\"\n",
        ] {
            let (cfg, _) = dirs(Some(written), None);
            let l = load(cfg.path()).unwrap();
            assert_eq!(l.config.source, None, "{written}");
            assert!(
                l.warnings
                    .iter()
                    .any(|w| w.contains("`[source]` has no `path`")),
                "{written}: {:?}",
                l.warnings
            );
        }
        let (cfg, _) = dirs(Some("[source]\ndir = \"/x\"\n"), None);
        let l = load(cfg.path()).unwrap();
        assert!(
            l.warnings.iter().any(|w| w.contains("`source.dir`")),
            "{:?}",
            l.warnings
        );
    }

    #[test]
    fn an_unknown_key_in_review_or_source_warns_and_the_default_stands() {
        let (cfg, _) = dirs(
            Some("[review]\ntimeout = 60\n\n[source]\npath = \"/x\"\nbranch = \"main\"\n"),
            None,
        );
        let l = load(cfg.path()).unwrap();
        assert_eq!(l.config.review, Review::default());
        assert_eq!(l.config.source.map(|s| s.path), Some(PathBuf::from("/x")));
        assert_eq!(l.warnings.len(), 2, "{:?}", l.warnings);
        assert!(
            l.warnings[0].contains("unknown key `source.branch`"),
            "{:?}",
            l.warnings
        );
        assert!(
            l.warnings[1].contains("unknown key `review.timeout`"),
            "{:?}",
            l.warnings
        );
    }

    #[test]
    fn ci_mode_reads_user_config_flag_and_env_never_project_toml() {
        for (written, want) in [("local", CiMode::Local), ("remote", CiMode::Remote)] {
            let (cfg, _) = dirs(Some(&format!("[ci]\nmode = \"{written}\"\n")), None);
            let l = load(cfg.path()).unwrap();
            assert_eq!(l.config.ci.mode, Some(want));
            assert_eq!(l.sources.ci, Source::User);
            assert!(l.warnings.is_empty(), "{:?}", l.warnings);
            assert_eq!(
                knobs(&l, None, None)["ci"],
                json!({"value": written, "source": "user"})
            );
        }

        let (cfg, _) = dirs(None, None);
        let l = load(cfg.path()).unwrap();
        assert_eq!((l.config.ci.mode, l.sources.ci), (None, Source::Default));
        assert_eq!(
            knobs(&l, None, None)["ci"],
            json!({"value": null, "source": "default"})
        );

        let (cfg, _) = dirs(Some("[ci]\nmode = \"local\"\nrunner = \"x\"\n"), None);
        let l = load(cfg.path()).unwrap();
        assert_eq!(l.config.ci.mode, Some(CiMode::Local));
        assert_eq!(l.warnings.len(), 1, "{:?}", l.warnings);
        assert!(
            l.warnings[0].contains("unknown key `ci.runner` ignored"),
            "{:?}",
            l.warnings
        );

        let (cfg, _) = dirs(Some("[ci]\nmode = \"cloud\"\n"), None);
        assert!(matches!(load(cfg.path()), Err(Error::Toml { .. })));

        let (cfg, repo) = dirs(
            Some("[ci]\nmode = \"local\"\n"),
            Some("[ci]\nmode = \"remote\"\n"),
        );
        let l = load_with_project(cfg.path(), repo.path()).unwrap();
        assert_eq!(l.config.ci.mode, Some(CiMode::Local));
        assert_eq!(l.sources.ci, Source::User);
        assert_eq!(l.warnings.len(), 1, "{:?}", l.warnings);
        assert!(
            l.warnings[0].contains(PROJECT_FILE) && l.warnings[0].contains("unknown key `ci`"),
            "{:?}",
            l.warnings
        );

        let (cfg, _) = dirs(None, None);
        let unset = load(cfg.path()).unwrap();
        let (cfg, _) = dirs(Some("[ci]\nmode = \"remote\"\n"), None);
        let user = load(cfg.path()).unwrap();
        let local = Some(CiMode::Local);
        let remote = Some(CiMode::Remote);
        assert_eq!(
            ci_mode_from(&user, local, Some("remote")),
            (local, Source::Flag, None)
        );
        assert_eq!(
            ci_mode_from(&user, None, Some("local")),
            (remote, Source::User, None)
        );
        assert_eq!(
            ci_mode_from(&unset, None, Some(" LOCAL ")),
            (local, Source::Default, None)
        );
        assert_eq!(
            ci_mode_from(&unset, None, Some("Remote")),
            (remote, Source::Default, None)
        );
        assert_eq!(
            ci_mode_from(&unset, None, None),
            (None, Source::Default, None)
        );
        assert_eq!(
            ci_mode_from(&unset, None, Some("")),
            (None, Source::Default, None)
        );
        let (mode, source, warning) = ci_mode_from(&unset, None, Some("cloud"));
        assert_eq!((mode, source), (None, Source::Default));
        assert!(warning.unwrap().contains(CI_MODE_ENV));
        assert_eq!(resolve_ci_mode(&unset, local), (local, Source::Flag, None));
    }

    #[test]
    fn write_ci_mode_preserves_comments_and_updates_idempotently() {
        let leftovers = |dir: &Path| -> Vec<String> {
            fs::read_dir(dir)
                .unwrap()
                .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
                .filter(|n| n != "config.toml")
                .collect()
        };

        let base = tempfile::tempdir().unwrap();
        let fresh = base.path().join("factory");
        let path = write_ci_mode(&fresh, CiMode::Local).unwrap();
        assert_eq!(path, fresh.join("config.toml"));
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "[ci]\nmode = \"local\"\n"
        );
        assert_eq!(load(&fresh).unwrap().config.ci.mode, Some(CiMode::Local));
        assert!(leftovers(&fresh).is_empty(), "{:?}", leftovers(&fresh));

        let original = "# the owner's notes\n# keep me\nbudget_weighted_tokens = 5 # inline\n\n\
                        [source]\npath = \"/x\"\n\n[review]\n# timeout note\ntimeout_secs = 2\n\n\
                        [workers.polish]\nmodel = \"sonnet\"";
        let (cfg, _) = dirs(Some(original), None);
        let path = write_ci_mode(cfg.path(), CiMode::Local).unwrap();
        let appended = fs::read_to_string(&path).unwrap();
        assert_eq!(appended, format!("{original}\n\n[ci]\nmode = \"local\"\n"));
        let l = load(cfg.path()).unwrap();
        assert_eq!(l.config.ci.mode, Some(CiMode::Local));
        assert_eq!(l.config.workers.polish.model, "sonnet");
        assert_eq!(l.config.review.timeout_secs, 2);
        assert!(l.warnings.is_empty(), "{:?}", l.warnings);

        write_ci_mode(cfg.path(), CiMode::Remote).unwrap();
        let updated = fs::read_to_string(&path).unwrap();
        assert_eq!(updated, appended.replace("\"local\"", "\"remote\""));
        write_ci_mode(cfg.path(), CiMode::Remote).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), updated);
        assert_eq!(updated.matches("[ci]").count(), 1);
        assert!(
            leftovers(cfg.path()).is_empty(),
            "{:?}",
            leftovers(cfg.path())
        );

        let (cfg, _) = dirs(
            Some("[ci]\n# chosen once\n\n[other]\nmode = \"keep\"\n"),
            None,
        );
        let path = write_ci_mode(cfg.path(), CiMode::Remote).unwrap();
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "[ci]\nmode = \"remote\"\n# chosen once\n\n[other]\nmode = \"keep\"\n"
        );

        let (cfg, _) = dirs(
            Some("[ci]\nmode = \"local\"\n[review]\nmodel = \"m\"\nmode = 1"),
            None,
        );
        let path = write_ci_mode(cfg.path(), CiMode::Remote).unwrap();
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "[ci]\nmode = \"remote\"\n[review]\nmodel = \"m\"\nmode = 1\n"
        );
    }

    #[test]
    fn write_ci_mode_rewrites_a_symlinked_config_in_place_with_its_permissions() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let dotfiles = tempfile::tempdir().unwrap();
        let real = dotfiles.path().join("config.toml");
        fs::write(&real, "[review]\nmodel = \"m\"\n").unwrap();
        fs::set_permissions(&real, fs::Permissions::from_mode(0o600)).unwrap();
        let cfg = tempfile::tempdir().unwrap();
        symlink(&real, cfg.path().join("config.toml")).unwrap();

        write_ci_mode(cfg.path(), CiMode::Local).unwrap();
        let link = cfg.path().join("config.toml");
        assert!(
            fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(
            fs::read_to_string(&real).unwrap(),
            "[review]\nmodel = \"m\"\n\n[ci]\nmode = \"local\"\n"
        );
        let mode = fs::metadata(&real).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        assert_eq!(fs::read_dir(dotfiles.path()).unwrap().count(), 1);
    }

    #[test]
    fn write_ci_mode_refuses_a_ci_header_it_cannot_patch() {
        let original = "sample = \"\"\"\n[ci]\n\"\"\"\n";
        let (cfg, _) = dirs(Some(original), None);
        let e = write_ci_mode(cfg.path(), CiMode::Local).unwrap_err();
        assert!(e.to_string().contains("set `mode = \"local\"`"), "{e}");
        let path = cfg.path().join("config.toml");
        assert_eq!(fs::read_to_string(path).unwrap(), original);
    }

    #[test]
    fn config_patch_updates_multiple_sections_atomically_preserving_comments() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let leftovers = |dir: &Path| -> Vec<String> {
            fs::read_dir(dir)
                .unwrap()
                .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
                .filter(|n| n != "config.toml")
                .collect()
        };
        let s = |v: &str| Some(toml::Value::String(v.into()));
        let edit = |table: &str, key: &str, value| Patch {
            table: table.into(),
            key: key.into(),
            value,
        };

        let base = tempfile::tempdir().unwrap();
        let fresh = base.path().join("factory");
        let path = patch(
            &fresh,
            &[
                edit("cockpit", "provider", s("claude")),
                edit("workers.build", "provider", s("codex")),
                edit("cockpit", "theme", None),
            ],
        )
        .unwrap();
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "[cockpit]\nprovider = \"claude\"\n\n[workers.build]\nprovider = \"codex\"\n"
        );
        assert!(leftovers(&fresh).is_empty(), "{:?}", leftovers(&fresh));

        let dotfiles = tempfile::tempdir().unwrap();
        let real = dotfiles.path().join("config.toml");
        let original = "# the owner's notes\nbudget_weighted_tokens = 5 # inline\n\n\
                        [workers]\n# shared defaults\nmodel = \"opus\"\neffort = \"high\" # keep high\n\n\
                        [ \"workers\" . build ]\nmodel = \"opus\"\n\n\
                        [review]\nagent = \"cursor-agent\" # legacy\ntimeout_secs = 2\n\n\
                        [workers.fix]\neffort = \"low\"\n";
        fs::write(&real, original).unwrap();
        fs::set_permissions(&real, fs::Permissions::from_mode(0o600)).unwrap();
        let cfg = tempfile::tempdir().unwrap();
        let link = cfg.path().join("config.toml");
        symlink(&real, &link).unwrap();

        patch(
            cfg.path(),
            &[
                edit("cockpit", "provider", s("codex")),
                edit("workers", "provider", s("claude")),
                edit("workers", "effort", None),
                edit("workers.build", "provider", s("codex")),
                edit("workers.build", "model", None),
                edit("review", "provider", s("cursor")),
                edit("review", "agent", None),
                edit("review", "pass_lines", Some(toml::Value::Integer(400))),
                edit("ci", "mode", s("local")),
            ],
        )
        .unwrap();
        assert!(
            fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(
            fs::metadata(&real).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let want = "# the owner's notes\nbudget_weighted_tokens = 5 # inline\n\n\
                    [workers]\n# shared defaults\nmodel = \"opus\"\nprovider = \"claude\"\n\n\
                    [ \"workers\" . build ]\nprovider = \"codex\"\n\n\
                    [review]\ntimeout_secs = 2\nprovider = \"cursor\"\npass_lines = 400\n\n\
                    [workers.fix]\neffort = \"low\"\n\n\
                    [cockpit]\nprovider = \"codex\"\n\n\
                    [ci]\nmode = \"local\"\n";
        assert_eq!(fs::read_to_string(&real).unwrap(), want);
        assert!(leftovers(dotfiles.path()).is_empty());
        assert!(leftovers(cfg.path()).is_empty());
        let l = load(cfg.path()).unwrap();
        assert!(l.warnings.is_empty(), "{:?}", l.warnings);
        assert_eq!(l.config.cockpit.provider, Some(Provider::Codex));
        assert_eq!(l.config.workers.provider, Some(Provider::Claude));
        assert_eq!(l.config.workers.effort, None);
        assert_eq!(l.config.workers.build.provider, Some(Provider::Codex));
        assert_eq!(l.sources.workers.build.model, Source::Default);
        assert_eq!(
            l.config.review.provider,
            Some(ReviewProvider::Provider(Provider::Cursor))
        );
        assert_eq!(l.sources.review.agent, Source::Default);
        assert_eq!(l.config.review.pass_lines, 400);
        assert_eq!(l.config.ci.mode, Some(CiMode::Local));

        patch(cfg.path(), &[edit("review", "provider", s("cursor"))]).unwrap();
        assert_eq!(fs::read_to_string(&real).unwrap(), want);

        let inline = "workers = { build = { model = \"opus\" } }\n\n[review]\nmodel = \"m\"\n";
        let (cfg, _) = dirs(Some(inline), None);
        let e = patch(
            cfg.path(),
            &[
                edit("review", "model", s("n")),
                edit("workers.build", "model", s("sonnet")),
            ],
        )
        .unwrap_err();
        assert!(
            e.to_string()
                .contains("set `model = \"sonnet\"` under `[workers.build]` by hand"),
            "{e}"
        );
        let e = patch(cfg.path(), &[edit("workers.build", "model", None)]).unwrap_err();
        assert!(e.to_string().contains("remove `model`"), "{e}");
        assert_eq!(
            fs::read_to_string(cfg.path().join("config.toml")).unwrap(),
            inline
        );
        assert!(leftovers(cfg.path()).is_empty());
    }

    #[test]
    fn review_defaults_fill_every_key_the_file_leaves_out() {
        let (cfg, _) = dirs(None, None);
        assert_eq!(load(cfg.path()).unwrap().config.review, Review::default());
        let d = Review::default();
        assert_eq!(
            (
                d.agent.as_str(),
                d.model.as_str(),
                d.timeout_secs,
                d.pass_lines
            ),
            ("cursor-agent", "grok-4.7-high", 1800, 600)
        );
        let (cfg, _) = dirs(Some("[review]\ntimeout_secs = 2\n"), None);
        let l = load(cfg.path()).unwrap();
        assert_eq!(
            l.config.review,
            Review {
                timeout_secs: 2,
                ..Review::default()
            }
        );
        assert!(l.warnings.is_empty(), "{:?}", l.warnings);
    }

    fn rw(p: Provider, m: &str, e: Option<&str>) -> ResolvedWorker {
        ResolvedWorker {
            provider: p,
            model: m.to_string(),
            effort: e.map(str::to_string),
        }
    }

    fn spec(raw: &str) -> WorkerSpec {
        parse_worker_spec(raw).unwrap()
    }

    #[test]
    fn worker_config_mixes_roles_without_leaking_claude_defaults() {
        let (cfg, _) = dirs(
            Some(
                "[workers]\nprovider = \"claude\"\nmodel = \"opus\"\neffort = \"high\"\n\n\
                 [workers.build]\nprovider = \"codex\"\n\n\
                 [workers.fix]\nprovider = \"cursor\"\n\n\
                 [workers.checkpoint]\nmodel = \"sonnet\"\n",
            ),
            None,
        );
        let l = load(cfg.path()).unwrap();
        let (got, warning) = resolve_workers_from(&l, &[], None);
        assert_eq!(warning, None);
        let got = got.unwrap();
        assert_eq!(got.build, rw(Provider::Codex, "gpt-5.4", Some("high")));
        assert_eq!(got.fix, rw(Provider::Cursor, "grok-4.7-high", None));
        assert_eq!(got.checkpoint, rw(Provider::Claude, "sonnet", Some("high")));
        assert_eq!(got.polish, rw(Provider::Claude, "opus", Some("high")));
        assert_eq!(got.for_role("merge"), &got.build);
        assert_eq!(
            got.distinct_providers(),
            [Provider::Codex, Provider::Cursor, Provider::Claude]
        );
        let json = serde_json::to_value(&got).unwrap();
        assert_eq!(
            json["fix"],
            json!({"provider": "cursor", "model": "grok-4.7-high"})
        );
        assert_eq!(
            serde_json::from_value::<ResolvedWorkers>(json).unwrap(),
            got
        );

        let flags = [
            spec("codex:o3"),
            spec("checkpoint=agy::max"),
            spec("polish=claude"),
        ];
        let (got, _) = resolve_workers_from(&l, &flags, None);
        let got = got.unwrap();
        assert_eq!(got.build, rw(Provider::Codex, "o3", Some("high")));
        assert_eq!(got.fix, rw(Provider::Codex, "o3", Some("high")));
        assert_eq!(
            got.checkpoint,
            rw(Provider::Agy, "gemini-4-neon-high", Some("max"))
        );
        assert_eq!(got.polish, rw(Provider::Claude, "opus", Some("high")));

        let (cfg, _) = dirs(
            Some("[workers]\nmodel = \"sonnet\"\n[workers.build]\nprovider = \"codex\"\n"),
            None,
        );
        let l = load(cfg.path()).unwrap();
        assert_eq!(resolve_workers_from(&l, &[], None), (None, None));
        let (none, warning) = resolve_workers_from(&l, &[], Some("gemini"));
        assert!(none.is_none());
        assert!(warning.unwrap().contains(WORKER_PROVIDER_ENV));
        let (got, warning) = resolve_workers_from(&l, &[], Some(" Claude "));
        assert_eq!(warning, None);
        let got = got.unwrap();
        assert_eq!(got.build, rw(Provider::Codex, "gpt-5.4", Some("high")));
        assert_eq!(got.fix, rw(Provider::Claude, "sonnet", Some("high")));
        let (got, _) = resolve_workers_from(&l, &[spec("fix=codex")], None);
        assert!(got.is_none(), "checkpoint and polish have no provider");
        let (got, _) = resolve_workers_from(&l, &[spec("cursor")], Some("claude"));
        let got = got.unwrap();
        assert_eq!(got.build, rw(Provider::Cursor, "grok-4.7-high", None));
        assert_eq!(got.polish, rw(Provider::Cursor, "grok-4.7-high", None));

        let (cfg, _) = dirs(Some("[workers]\nprovider = \"opencode\"\n"), None);
        let (got, _) = resolve_workers_from(&load(cfg.path()).unwrap(), &[], None);
        assert_eq!(got.unwrap().build, rw(Provider::Opencode, "", None));
    }

    #[test]
    fn a_worker_spec_keeps_colons_in_the_model_and_refuses_what_no_cli_takes() {
        assert_eq!(
            spec("build=codex:gpt-5.4:xhigh"),
            WorkerSpec {
                role: Some("build".into()),
                provider: Provider::Codex,
                model: Some("gpt-5.4".into()),
                effort: Some("xhigh".into()),
            }
        );
        assert_eq!(
            spec("opencode:ollama/llama3:8b"),
            WorkerSpec {
                role: None,
                provider: Provider::Opencode,
                model: Some("ollama/llama3:8b".into()),
                effort: None,
            }
        );
        let s = spec("Fix=claude");
        assert_eq!(
            (s.role.as_deref(), s.model, s.effort),
            (Some("fix"), None, None)
        );
        for bad in [
            "opencode",
            "review=claude",
            "cursor:grok-4.7-high:high",
            "codex:o3:max",
            "codex:--auto",
            "gemini",
            "build=",
        ] {
            assert!(parse_worker_spec(bad).is_err(), "{bad}");
        }
    }
}
