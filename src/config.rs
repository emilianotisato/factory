//! `~/.config/factory/config.toml` (PRD §11 FR-9, §19). Only the keys the PRD
//! names: `budget_weighted_tokens` and the per-role model and effort. A
//! missing file is the defaults; an unknown key is a warning, never an error.
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{Error, Result};

pub const DEFAULT_BUDGET: u64 = 30_000_000;
pub const DEFAULT_MODEL: &str = "opus";
pub const DEFAULT_EFFORT: &str = "high";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub budget_weighted_tokens: u64,
    pub workers: Workers,
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
}

const TOP_KEYS: [&str; 2] = ["budget_weighted_tokens", "workers"];
const ROLE_KEYS: [&str; 4] = ["build", "fix", "checkpoint", "polish"];
const WORKER_KEYS: [&str; 2] = ["model", "effort"];

/// `<config_dir>/config.toml`, or the defaults when there is none.
pub fn load(config_dir: &Path) -> Result<Loaded> {
    let path = config_dir.join("config.toml");
    let text = match fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Loaded {
                config: Config::default(),
                path,
                warnings: Vec::new(),
            });
        }
        Err(e) => return Err(crate::error::io(format!("read {}", path.display()))(e)),
    };
    let table: toml::Table = toml::from_str(&text).map_err(|source| Error::Toml {
        what: format!("parse {}", path.display()),
        source,
    })?;
    let warnings = unknown_keys(&table)
        .into_iter()
        .map(|k| format!("{}: unknown key `{k}` ignored", path.display()))
        .collect();
    let config: Config = toml::Value::Table(table)
        .try_into()
        .map_err(|source| Error::Toml {
            what: format!("read {}", path.display()),
            source,
        })?;
    Ok(Loaded {
        config,
        path,
        warnings,
    })
}

fn unknown_keys(table: &toml::Table) -> Vec<String> {
    let mut out: Vec<String> = table
        .keys()
        .filter(|k| !TOP_KEYS.contains(&k.as_str()))
        .cloned()
        .collect();
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
