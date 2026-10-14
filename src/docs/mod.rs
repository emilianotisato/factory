//! The feature-docs library (PRD-002 FR-18): `factory docs` is built on it
//! and the driver calls it in process through [`Docs`]. The board
//! (`status --json`) and preflight (`preflight --json`) are the contract every
//! caller reads. No routing logic is derived from the board here: the docs
//! decide, the driver reads.
//!
//! Every optional key carries `#[serde(default)]` so JSON written by an older
//! or newer build still parses (PRD-001 slice 02).
//!
//! The model lives in the submodules: free functions over a repo root, and
//! the board structs below are their output contract.
pub mod board;
pub mod check;
pub mod doctor;
pub mod frontmatter;
pub mod gate;
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

/// The text `Docs::open` refuses a v2 repo with (FR-18): the driver reads v3
/// only, so `factory run` stops before preflight.
pub const V2_HINT: &str = "docs are v2: run `factory docs migrate --dry`";

/// A v3 repo the driver reads and writes in process (PRD-002 FR-18).
pub struct Docs {
    root: PathBuf,
}

/// What `ship` did: shipped (with its output, whose `⚠` lines the closeout
/// forwards), or refused with the reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ship {
    Done(String),
    Refused(String),
}

impl Docs {
    /// The repo when it is on v3. A v2 repo (a `docs/index.md`, no
    /// `[docs] framework = 3`) is refused with [`V2_HINT`]; a repo off the
    /// framework with its own text. Both are `Error::NotV3`.
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

    /// The feature's folder, live or archived.
    pub fn dir(&self, feature: &str) -> Result<PathBuf> {
        Ok(board::scan(&self.root)?.resolve(feature)?.dir.clone())
    }

    /// The board of one feature (`factory docs status NNN --json`).
    pub fn status(&self, feature: &str) -> Result<Status> {
        board::status(&self.root, feature)
    }

    /// The preflight verdict with its exit code: 0 pass, 1 refused, 2 asks
    /// pending.
    pub fn preflight(&self, feature: &str) -> Result<(Preflight, i32)> {
        preflight::run(&self.root, feature)
    }

    fn project(&self) -> Result<project::Project> {
        project::load(&self.root)?
            .ok_or_else(|| Error::NotV3("docs/project.toml has no [docs] table".into()))
    }

    /// The declared gate command; an error when none is declared.
    pub fn gate(&self) -> Result<String> {
        self.project()?
            .gate
            .ok_or_else(|| Error::Msg("no gate declared: `factory docs gate set \"<cmd>\"`".into()))
    }

    /// The declared inner-loop command, else the full gate (as `gate --inner`).
    pub fn gate_inner(&self) -> Result<String> {
        let p = self.project()?;
        p.inner
            .or(p.gate)
            .ok_or_else(|| Error::Msg("no gate declared: `factory docs gate set \"<cmd>\"`".into()))
    }

    /// `ship NNN`. A refusal (a slice not done) is a verdict the caller
    /// routes, so it comes back as `Ship::Refused` rather than an error.
    pub fn ship(&self, feature: &str) -> Result<Ship> {
        let mut out = Vec::new();
        match ship::ship(&self.root, feature, &mut out) {
            Ok(()) => Ok(Ship::Done(String::from_utf8_lossy(&out).into_owned())),
            Err(Error::Msg(m)) => Ok(Ship::Refused(m)),
            Err(e) => Err(e),
        }
    }

    /// `state::set` on the feature's `state.toml` (FR-13): the one write path.
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

    /// Append one log entry (FR-14); the id the tool allocated, `None` for an
    /// `answer`, `review` or `note`.
    pub fn log_add(&self, feature: &str, entry: log::Entry) -> Result<Option<String>> {
        log::add(&self.dir(feature)?, entry)
    }

    /// Every entry of the feature's `log.jsonl`, in file order. A torn line
    /// is skipped here; `check` reports it.
    pub fn log_entries(&self, feature: &str) -> Result<Vec<log::Entry>> {
        Ok(log::entries(&self.dir(feature)?)?.0)
    }
}

// ── status --json ────────────────────────────────────────────────────────────

/// `docs status NNN --json` (PRD §20).
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

/// One slice as the board lists it; `depends_on` is the one snake_case key.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Slice {
    pub number: u32,
    /// Empty when a `[slices.N]` entry has no file; printed as `null`.
    #[serde(with = "nullable")]
    pub file: String,
    pub title: Option<String>,
    pub status: Option<String>,
    pub lane: Option<u32>,
    pub depends_on: Vec<u32>,
}

/// `next` and `ready[]` entries.
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
    /// The slices the entry says it covered; empty on an entry written
    /// before PRD-005 FR-7, which covers every slice up to `after`. Left out
    /// of the JSON when empty, so an old log's board reads as it did.
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

/// A string that is `null` in JSON when empty, either way round.
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

/// `docs preflight NNN --json` (PRD §20).
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

/// Where the repo is and where the feature lives. `feature` is already the
/// branch to use: the tool swaps in `alt` when that is the one that exists.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Branch {
    pub current: Option<String>,
    pub default: Option<String>,
    pub feature: Option<String>,
    pub alt: Option<String>,
    pub feature_exists: bool,
}
