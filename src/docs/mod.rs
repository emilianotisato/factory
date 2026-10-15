//! The board and preflight are the contract: the docs decide, the driver reads. Every key defaults so older JSON parses.
pub mod board;
pub mod check;
pub mod doctor;
pub mod frontmatter;
pub mod gate;
pub mod harvest;
pub mod hook;
pub mod ideas;
pub mod lanes;
pub mod log;
pub mod migrate;
pub mod migrate_ideas;
pub mod preflight;
pub mod project;
pub mod renumber;
pub mod retro;
pub mod scaffold;
pub mod ship;
pub mod state;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{Error, Result};

/// The refusal for a v2 repo; `factory run` stops on it before preflight.
pub const V2_HINT: &str = "docs are v2: run `factory docs migrate --dry`";

/// A v3 repo the driver reads and writes in process.
pub struct Docs {
    root: PathBuf,
}

/// Ship's output, whose `⚠` lines the closeout forwards, or the refusal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ship {
    Done(String),
    Refused(String),
}

impl Docs {
    pub fn open(repo_root: &Path) -> Result<Docs> {
        if project::is_v3(repo_root) {
            return Ok(Docs {
                root: repo_root.to_path_buf(),
            });
        }
        if repo_root.join("docs/index.md").is_file() {
            return Err(Error::NotV3(V2_HINT.into()));
        }
        Err(Error::NotV3(format!(
            "not on the framework: {} has no docs/project.toml with [docs] framework = 3 — run `factory docs init`",
            repo_root.display()
        )))
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn dir(&self, feature: &str) -> Result<PathBuf> {
        Ok(board::scan(&self.root)?.resolve(feature)?.dir.clone())
    }

    pub fn status(&self, feature: &str) -> Result<Status> {
        board::status(&self.root, feature)
    }

    /// The exit code is 0 pass, 1 refused, 2 asks pending.
    pub fn preflight(&self, feature: &str) -> Result<(Preflight, i32)> {
        preflight::run(&self.root, feature)
    }

    fn project(&self) -> Result<project::Project> {
        project::load(&self.root)?
            .ok_or_else(|| Error::NotV3("docs/project.toml has no [docs] table".into()))
    }

    pub fn gate(&self) -> Result<String> {
        self.project()?
            .gate
            .ok_or_else(|| Error::Msg("no gate declared: `factory docs gate set \"<cmd>\"`".into()))
    }

    pub fn gate_inner(&self) -> Result<String> {
        let p = self.project()?;
        p.inner
            .or(p.gate)
            .ok_or_else(|| Error::Msg("no gate declared: `factory docs gate set \"<cmd>\"`".into()))
    }

    /// A refusal is a verdict the caller routes, so it is `Ship::Refused`, not an error.
    pub fn ship(&self, feature: &str) -> Result<Ship> {
        let mut out = Vec::new();
        match ship::ship(&self.root, feature, &mut out) {
            Ok(()) => Ok(Ship::Done(String::from_utf8_lossy(&out).into_owned())),
            Err(Error::Msg(m)) => Ok(Ship::Refused(m)),
            Err(e) => Err(e),
        }
    }

    pub fn set(&self, feature: &str, pairs: &[(&str, &str)]) -> Result<()> {
        state::set(&self.dir(feature)?, pairs).map(|_| ())
    }

    pub fn set_slice_status(&self, feature: &str, slice: u32, status: &str) -> Result<()> {
        self.set(feature, &[(&format!("slices.{slice}.status"), status)])
    }

    pub fn set_status(&self, feature: &str, status: &str) -> Result<()> {
        self.set(feature, &[("status", status)])
    }

    pub fn set_reviewed(&self, feature: &str, date: &str) -> Result<()> {
        self.set(feature, &[("reviewed", date)])
    }

    pub fn log_add(&self, feature: &str, entry: log::Entry) -> Result<Option<String>> {
        log::add(&self.dir(feature)?, entry)
    }

    /// A torn line is dropped here; `check` reports it.
    pub fn log_entries(&self, feature: &str) -> Result<Vec<log::Entry>> {
        Ok(log::entries(&self.dir(feature)?)?.0)
    }
}

// ── status --json ────────────────────────────────────────────────────────────

/// `docs status NNN --json`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Status {
    pub number: u32,
    pub slug: String,
    pub dir: String,
    pub kind: Option<String>,
    pub status: Option<String>,
    pub reviewed: Value,
    pub gate: Gate,
    pub qa_plan: bool,
    pub harness_line: Option<String>,
    pub harness: Harness,
    pub rubric: bool,
    pub sliced: bool,
    pub all_done: bool,
    pub slices: Vec<Slice>,
    pub checkpoint_every: u32,
    pub checkpoints: Vec<Checkpoint>,
    pub unverified: Vec<u32>,
    pub checkpoint_due: bool,
    pub next: Option<SliceRef>,
    pub ready: Vec<SliceRef>,
    pub building: Vec<Building>,
    pub lanes: u32,
    pub free_lanes: Vec<u32>,
    pub spawnable: u32,
    pub blocked: Vec<u32>,
    pub open_blocks: Vec<OpenBlock>,
    pub open_opens: Vec<Value>,
    pub log: LogSummary,
    pub retro: Value,
    pub branch: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Gate {
    pub declared: Option<String>,
    pub status: String,
    pub detail: Option<String>,
    pub candidates: Value,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Harness {
    pub present: bool,
    pub detail: Option<String>,
}

/// `depends_on` is the contract's one snake_case key.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Slice {
    pub number: u32,
    /// Empty when a `[slices.N]` entry has no file.
    #[serde(with = "nullable")]
    pub file: String,
    pub title: Option<String>,
    pub status: Option<String>,
    pub lane: Option<u32>,
    pub depends_on: Vec<u32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct SliceRef {
    pub number: u32,
    pub file: String,
    /// Only `next` carries it; `ready[]` entries print without the key.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Building {
    pub number: u32,
    pub file: String,
    pub lane: Option<u32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Checkpoint {
    pub n: u32,
    pub after: Option<u32>,
    /// Empty means every slice up to `after`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub covers: Vec<u32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct OpenBlock {
    pub n: u32,
    pub slice: Option<u32>,
    pub phase: Option<String>,
    pub title: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct LogSummary {
    pub decisions: u32,
    pub flagged: Vec<u32>,
    pub decide: Vec<u32>,
    pub blocks: u32,
}

mod nullable {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(s: &str, ser: S) -> Result<S::Ok, S::Error> {
        if s.is_empty() {
            ser.serialize_none()
        } else {
            ser.serialize_str(s)
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(de: D) -> Result<String, D::Error> {
        Ok(Option::<String>::deserialize(de)?.unwrap_or_default())
    }
}

// ── preflight --json ─────────────────────────────────────────────────────────

/// `docs preflight NNN --json`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Preflight {
    pub feature: Option<FeatureRef>,
    pub pass: bool,
    pub refuse: Vec<String>,
    pub ask: Vec<String>,
    pub judge: Vec<String>,
    pub ok: Vec<String>,
    pub branch: Branch,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct FeatureRef {
    pub number: u32,
    pub slug: String,
    pub status: Option<String>,
}

/// `feature` is already the branch to use: `alt` is swapped in when that one exists.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Branch {
    pub current: Option<String>,
    pub default: Option<String>,
    pub feature: Option<String>,
    pub alt: Option<String>,
    pub feature_exists: bool,
}
