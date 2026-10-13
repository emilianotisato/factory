//! `~/.config/factory/config.toml` (PRD §11 FR-9, §19). Only the keys the PRD
//! names: `budget_weighted_tokens` and the per-role model and effort. A
//! missing file is the defaults; an unknown key is a warning, never an error.
//!
//! PRD-003 FR-23 adds the project scope, `<repo>/docs/project.toml`, read
//! over the user file key by key, the scope each knob came from, and
//! [`knobs`], the JSON a run row records at its start.
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{Error, Result};

pub const DEFAULT_BUDGET: u64 = 30_000_000;
pub const DEFAULT_MODEL: &str = "opus";
pub const DEFAULT_EFFORT: &str = "high";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub budget_weighted_tokens: u64,
    pub workers: Workers,
    /// `[source]`: the skills checkout friction ideas land in (PRD-004 FR-7).
    pub source: Option<SourceRepo>,
    /// `[review]`: the cross-vendor second opinion (PRD-004 FR-6).
    pub review: Review,
}

/// `[source] path`: a checkout of the factory repo. `Source` is taken: it
/// names the scope a knob came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceRepo {
    pub path: PathBuf,
}

/// `[review]`: who `factory review second` runs and how. The owner's own
/// facts (a renamed binary, a rate-limited model) live in this file, never
/// in the skills.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Review {
    /// A name on PATH or a path.
    pub agent: String,
    pub model: String,
    pub timeout_secs: u64,
    pub pass_lines: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Workers {
    pub build: Worker,
    pub fix: Worker,
    pub checkpoint: Worker,
    pub polish: Worker,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Worker {
    pub model: String,
    pub effort: String,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            budget_weighted_tokens: DEFAULT_BUDGET,
            workers: Workers::default(),
            source: None,
            review: Review::default(),
        }
    }
}

impl Default for Review {
    fn default() -> Self {
        Review {
            agent: "cursor-agent".into(),
            model: "grok-4.7-high".into(),
            timeout_secs: 900,
            pass_lines: 600,
        }
    }
}

impl Default for Worker {
    fn default() -> Self {
        Worker {
            model: DEFAULT_MODEL.into(),
            effort: DEFAULT_EFFORT.into(),
        }
    }
}

/// A loaded config and the warnings reading it raised.
#[derive(Debug, Clone)]
pub struct Loaded {
    pub config: Config,
    pub path: PathBuf,
    pub warnings: Vec<String>,
    /// Which scope set each knob (PRD-003 FR-23).
    pub sources: Sources,
}

/// The scope a knob came from: `runs.knobs` records it (PRD-003 FR-23).
/// `Flag` is `--budget` on the command line, budget only.
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
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct RoleSources {
    pub build: WorkerSources,
    pub fix: WorkerSources,
    pub checkpoint: WorkerSources,
    pub polish: WorkerSources,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct WorkerSources {
    pub model: Source,
    pub effort: Source,
}

const TOP_KEYS: [&str; 4] = ["budget_weighted_tokens", "workers", "source", "review"];
const PROJECT_TOP_KEYS: [&str; 3] = ["docs", "run", "workers"];
const RUN_KEYS: [&str; 1] = ["budget"];
const ROLE_KEYS: [&str; 4] = ["build", "fix", "checkpoint", "polish"];
const WORKER_KEYS: [&str; 2] = ["model", "effort"];

/// The project scope, relative to the repo the run targets (PRD-003 FR-23).
pub const PROJECT_FILE: &str = "docs/project.toml";

/// `<config_dir>/config.toml`, or the defaults when there is none.
pub fn load(config_dir: &Path) -> Result<Loaded> {
    let path = config_dir.join("config.toml");
    let mut loaded = Loaded {
        config: Config::default(),
        path: path.clone(),
        warnings: Vec::new(),
        sources: Sources::default(),
    };
    let Some(table) = read_table(&path)? else {
        return Ok(loaded);
    };
    loaded.warnings = warn(&path, unknown_keys(&table, &TOP_KEYS));
    loaded.sources.budget = scope(&table, &["budget_weighted_tokens"], Source::User);
    for (role, s) in loaded.sources.workers.each_mut() {
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
    Ok(loaded)
}

/// A leading `~/` against `$HOME`; any other path as written.
fn expand_home(path: &Path) -> PathBuf {
    match path.strip_prefix("~") {
        Ok(rest) => crate::paths::home().join(rest),
        Err(_) => path.to_path_buf(),
    }
}

/// The user file, then `<repo>/docs/project.toml` when present, its
/// `[run] budget` and `[workers.<role>] model | effort` overriding key by key.
/// Unknown keys there warn and never fail. `[docs]` is the docs tool's
/// (PRD-002 FR-13, `docs::project`), known here and read there.
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
    let p = project.workers;
    let w = &mut loaded.config.workers;
    let s = &mut loaded.sources.workers;
    for (worker, sources, over) in [
        (&mut w.build, &mut s.build, p.build),
        (&mut w.fix, &mut s.fix, p.fix),
        (&mut w.checkpoint, &mut s.checkpoint, p.checkpoint),
        (&mut w.polish, &mut s.polish, p.polish),
    ] {
        if let Some(m) = over.model {
            worker.model = m;
            sources.model = Source::Project;
        }
        if let Some(e) = over.effort {
            worker.effort = e;
            sources.effort = Source::Project;
        }
    }
    Ok(loaded)
}

/// `runs.knobs`: per role `{model, effort, source}`, with `effort_source`
/// beside it only when the effort came from another scope than the model;
/// the budget and its source; the lanes; both versions.
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
        let mut o = json!({"model": worker.model, "effort": worker.effort, "source": src.model});
        if src.effort != src.model {
            o["effort_source"] = json!(src.effort);
        }
        workers.insert(role.to_string(), o);
    }
    json!({
        "workers": workers,
        "budget": {"value": loaded.config.budget_weighted_tokens, "source": loaded.sources.budget},
        "lanes": lanes,
        "factory_version": env!("CARGO_PKG_VERSION"),
        "claude_version": claude_version,
    })
}

/// `docs/project.toml` as FR-23 reads it: every key optional.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Project {
    run: ProjectRun,
    workers: ProjectWorkers,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ProjectRun {
    budget: Option<u64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ProjectWorkers {
    build: ProjectWorker,
    fix: ProjectWorker,
    checkpoint: ProjectWorker,
    polish: ProjectWorker,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ProjectWorker {
    model: Option<String>,
    effort: Option<String>,
}

impl RoleSources {
    fn each_mut(&mut self) -> [(&'static str, &mut WorkerSources); 4] {
        [
            ("build", &mut self.build),
            ("fix", &mut self.fix),
            ("checkpoint", &mut self.checkpoint),
            ("polish", &mut self.polish),
        ]
    }
}

/// A TOML file as a table; `None` when it does not exist.
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

/// `scope` when `table` holds the dotted key `at`, else `Default`.
fn scope(table: &toml::Table, at: &[&str], scope: Source) -> Source {
    let mut v = table.get(at[0]);
    for k in &at[1..] {
        v = v.and_then(|v| v.get(*k));
    }
    if v.is_some() { scope } else { Source::Default }
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
    if let Some(toml::Value::Table(workers)) = table.get("workers") {
        for (role, v) in workers {
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
    fn the_project_file_overrides_the_user_file_key_by_key() {
        let (cfg, repo) = dirs(
            Some("[workers.polish]\nmodel = \"opus\"\neffort = \"low\"\n"),
            Some("[workers.polish]\nmodel = \"sonnet\"\n\n[run]\nbudget = 7\n"),
        );
        let l = load_with_project(cfg.path(), repo.path()).unwrap();
        assert_eq!(l.config.workers.polish.model, "sonnet");
        assert_eq!(l.sources.workers.polish.model, Source::Project);
        assert_eq!(l.config.workers.polish.effort, "low");
        assert_eq!(l.sources.workers.polish.effort, Source::User);
        assert_eq!(l.config.workers.build.model, DEFAULT_MODEL);
        assert_eq!(l.sources.workers.build.model, Source::Default);
        assert_eq!(l.sources.workers.build.effort, Source::Default);
        assert_eq!(l.config.budget_weighted_tokens, 7);
        assert_eq!(l.sources.budget, Source::Project);
        assert!(l.warnings.is_empty(), "{:?}", l.warnings);

        let k = knobs(&l, Some(2), Some("2.1.296 (Claude Code)"));
        assert_eq!(
            k["workers"]["polish"],
            json!({"model": "sonnet", "effort": "low", "source": "project", "effort_source": "user"})
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
    fn an_unknown_project_table_warns_once_and_never_fails() {
        let (cfg, repo) = dirs(None, Some("[deploy]\nfoo = 1\n"));
        let l = load_with_project(cfg.path(), repo.path()).unwrap();
        assert_eq!(l.warnings.len(), 1, "{:?}", l.warnings);
        assert!(l.warnings[0].contains(PROJECT_FILE), "{}", l.warnings[0]);
        assert!(l.warnings[0].contains("`deploy`"), "{}", l.warnings[0]);
        assert_eq!(l.config, Config::default());
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
            ("cursor-agent", "grok-4.7-high", 900, 600)
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
}
