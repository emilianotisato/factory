//! The `docs.mjs` wrapper (PRD §2, §3 FR-1). The board (`status --json`) and
//! preflight (`preflight --json`) are the contract every slice reads; the rest
//! are the commands the driver runs on `/deliver`'s behalf. No routing logic is
//! derived from the board here: the tool decides, the driver reads.
//!
//! Every optional key carries `#[serde(default)]` so a tool upgrade that adds
//! a key does not break the driver (slice 02 Current code).
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{Error, Result, paths};

/// A located `docs.mjs` and the repo it runs in.
pub struct Docs {
    tool: PathBuf,
    root: PathBuf,
}

/// What a tool run returned; the caller judges the exit where it matters.
#[derive(Debug, Clone)]
pub struct ToolOutput {
    pub stdout: String,
    pub stderr: String,
    pub code: i32,
}

impl Docs {
    /// The repo's `.docs/bin/docs.mjs` when vendored, else the skill's copy
    /// under `~/.claude/skills/feature-docs/bin/` — the same preference the
    /// tool's own hook applies.
    pub fn locate(repo_root: &Path) -> Result<Docs> {
        let vendored = repo_root.join(".docs/bin/docs.mjs");
        let global = paths::home().join(".claude/skills/feature-docs/bin/docs.mjs");
        let tool = if vendored.is_file() {
            vendored
        } else if global.is_file() {
            global
        } else {
            return Err(Error::Msg(format!(
                "no docs.mjs: neither {} nor {} exists",
                vendored.display(),
                global.display()
            )));
        };
        Ok(Docs {
            tool,
            root: repo_root.to_path_buf(),
        })
    }

    pub fn tool(&self) -> &Path {
        &self.tool
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// `node <tool> <args>` in the repo root, whatever it exits.
    pub fn run(&self, args: &[&str]) -> Result<ToolOutput> {
        let out = Command::new("node")
            .arg(&self.tool)
            .args(args)
            .current_dir(&self.root)
            .output()
            .map_err(|e| Error::Tool {
                cmd: self.cmdline(args),
                detail: format!("could not start node: {e}"),
            })?;
        Ok(ToolOutput {
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
            code: out.status.code().unwrap_or(-1),
        })
    }

    fn cmdline(&self, args: &[&str]) -> String {
        format!("docs {}", args.join(" "))
    }

    /// `run`, with a non-zero exit turned into an error quoting stderr.
    fn ok(&self, args: &[&str]) -> Result<ToolOutput> {
        let out = self.run(args)?;
        if out.code != 0 {
            return Err(Error::Tool {
                cmd: self.cmdline(args),
                detail: format!("exit {}: {}", out.code, quote(&out)),
            });
        }
        Ok(out)
    }

    fn parse<T: DeserializeOwned>(&self, args: &[&str], out: &ToolOutput) -> Result<T> {
        serde_json::from_str(&out.stdout).map_err(|e| Error::Tool {
            cmd: self.cmdline(args),
            detail: format!(
                "exit {}, stdout is not the expected JSON ({e}): {}",
                out.code,
                quote(out)
            ),
        })
    }

    /// `status NNN --json`: the board.
    pub fn status(&self, feature: &str) -> Result<Status> {
        let args = ["status", feature, "--json"];
        let out = self.ok(&args)?;
        self.parse(&args, &out)
    }

    /// `preflight NNN --json` with its exit code: 0 pass, 1 refused, 2 asks
    /// pending. Any other exit is the tool failing, not a verdict.
    pub fn preflight(&self, feature: &str) -> Result<(Preflight, i32)> {
        let args = ["preflight", feature, "--json"];
        let out = self.run(&args)?;
        match out.code {
            0..=2 => Ok((self.parse(&args, &out)?, out.code)),
            code => Err(Error::Tool {
                cmd: self.cmdline(&args),
                detail: format!("exit {code}: {}", quote(&out)),
            }),
        }
    }

    /// `gate`: the declared gate command.
    pub fn gate(&self) -> Result<String> {
        Ok(self.ok(&["gate"])?.stdout.trim().to_string())
    }

    /// `gate --inner`: the declared inner-loop command.
    pub fn gate_inner(&self) -> Result<String> {
        Ok(self.ok(&["gate", "--inner"])?.stdout.trim().to_string())
    }

    /// `log merge NNN`. A refusal (an AD clash) is a verdict the caller routes,
    /// so the exit comes back rather than erroring.
    pub fn log_merge(&self, feature: &str) -> Result<ToolOutput> {
        self.run(&["log", "merge", feature])
    }

    /// `ship NNN`; same contract as `log_merge`.
    pub fn ship(&self, feature: &str) -> Result<ToolOutput> {
        self.run(&["ship", feature])
    }

    /// `render --write`.
    pub fn render_write(&self) -> Result<ToolOutput> {
        self.ok(&["render", "--write"])
    }
}

fn quote(out: &ToolOutput) -> String {
    let err = out.stderr.trim();
    if err.is_empty() {
        out.stdout.trim().to_string()
    } else {
        err.to_string()
    }
}

// ── status --json ────────────────────────────────────────────────────────────

/// `docs status NNN --json` (PRD §20).
#[derive(Debug, Clone, Default, Deserialize)]
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

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Gate {
    pub declared: Option<String>,
    pub status: String,
    pub detail: Option<String>,
    pub candidates: Value,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Harness {
    pub present: bool,
    pub detail: Option<String>,
}

/// One slice as the board lists it; `depends_on` is the one snake_case key.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Slice {
    pub number: u32,
    pub file: String,
    pub title: Option<String>,
    pub status: Option<String>,
    pub lane: Option<u32>,
    pub depends_on: Vec<u32>,
}

/// `next` and `ready[]` entries.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct SliceRef {
    pub number: u32,
    pub file: String,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Building {
    pub number: u32,
    pub file: String,
    pub lane: Option<u32>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Checkpoint {
    pub n: u32,
    pub after: Option<u32>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct OpenBlock {
    pub n: u32,
    pub slice: Option<u32>,
    pub phase: Option<String>,
    pub title: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct LogSummary {
    pub decisions: u32,
    pub flagged: Vec<u32>,
    pub decide: Vec<u32>,
    pub blocks: u32,
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
