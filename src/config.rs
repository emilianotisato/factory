//! A missing file is the defaults; an unknown key warns, never fails.
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
    pub source: Option<SourceRepo>,
    pub review: Review,
    pub ci: Ci,
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

/// `Source` is taken: it names the scope a knob came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceRepo {
    pub path: PathBuf,
}

/// The owner's own facts (a renamed binary, a rate-limited model) live here, never in the skills.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Review {
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
            ci: Ci::default(),
        }
    }
}

impl Default for Review {
    fn default() -> Self {
        Review {
            agent: "cursor-agent".into(),
            model: "grok-4.7-high".into(),
            timeout_secs: 1800,
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

const TOP_KEYS: [&str; 5] = [
    "budget_weighted_tokens",
    "workers",
    "source",
    "review",
    "ci",
];
const PROJECT_TOP_KEYS: [&str; 5] = ["docs", "lanes", "leaks", "run", "workers"];
const RUN_KEYS: [&str; 1] = ["budget"];
const ROLE_KEYS: [&str; 4] = ["build", "fix", "checkpoint", "polish"];
const WORKER_KEYS: [&str; 2] = ["model", "effort"];
const SOURCE_KEYS: [&str; 1] = ["path"];
const REVIEW_KEYS: [&str; 4] = ["agent", "model", "timeout_secs", "pass_lines"];
const CI_KEYS: [&str; 1] = ["mode"];
pub const CI_MODE_ENV: &str = "FACTORY_CI_MODE";

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

fn expand_home(path: &Path) -> PathBuf {
    match path.strip_prefix("~") {
        Ok(rest) => crate::paths::home().join(rest),
        Err(_) => path.to_path_buf(),
    }
}

/// `[docs]`, `[lanes]` and `[leaks]` are known here only so they do not warn; `docs::project` and `leaks` read them.
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

/// Sets `[ci] mode` in `config_dir/config.toml`, keeping every other line as written.
/// A symlinked `config.toml` keeps its link: the file it points at is the one rewritten, with its permissions.
pub fn write_ci_mode(config_dir: &Path, mode: CiMode) -> Result<PathBuf> {
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
    let text = with_ci_mode(&old, mode);
    let parsed = toml::from_str::<toml::Table>(&text).map_err(|source| Error::Toml {
        what: format!("check the new {}", path.display()),
        source,
    })?;
    let set = parsed
        .get("ci")
        .and_then(|c| c.get("mode"))
        .and_then(toml::Value::as_str);
    if set != Some(mode.as_str()) {
        return Err(Error::Msg(format!(
            "{} not changed: its `[ci]` table could not be patched line by line — set `mode = \"{mode}\"` under `[ci]` by hand",
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

/// The table a header line opens; an array-of-tables header is never `ci`.
fn table_header(line: &str) -> Option<String> {
    let t = line.trim();
    if t.starts_with("[[") {
        return Some(String::new());
    }
    let inner = t.strip_prefix('[')?;
    let name = &inner[..inner.find(']')?];
    Some(name.trim().trim_matches(['"', '\'']).to_string())
}

fn is_mode_key(line: &str) -> bool {
    line.split_once('=')
        .is_some_and(|(k, _)| matches!(k.trim(), "mode" | "\"mode\"" | "'mode'"))
}

fn with_ci_mode(old: &str, mode: CiMode) -> String {
    let entry = format!("mode = \"{mode}\"");
    let mut lines: Vec<String> = old.lines().map(str::to_string).collect();
    let Some(ci) = lines
        .iter()
        .position(|l| table_header(l).as_deref() == Some("ci"))
    else {
        let mut text = old.to_string();
        if !text.is_empty() {
            if !text.ends_with('\n') {
                text += "\n";
            }
            text += "\n";
        }
        return text + &format!("[ci]\n{entry}\n");
    };
    let end = lines[ci + 1..]
        .iter()
        .position(|l| table_header(l).is_some())
        .map_or(lines.len(), |i| ci + 1 + i);
    match (ci + 1..end).find(|&i| is_mode_key(&lines[i])) {
        Some(i) => lines[i] = entry,
        None => lines.insert(ci + 1, entry),
    }
    lines.join("\n") + "\n"
}

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
    ] {
        if let (true, Some(toml::Value::Table(t))) = (top.contains(&name), table.get(name)) {
            out.extend(
                t.keys()
                    .filter(|k| !keys.contains(&k.as_str()))
                    .map(|k| format!("{name}.{k}")),
            );
        }
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
}
