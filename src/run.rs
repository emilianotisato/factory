//! A run's state on disk (PRD §3 FR-1): `<data_dir>/runs/<id>/state.json`
//! beside its `events.jsonl`, mirrored into the `runs` table. The board stays
//! the source of truth for what is built; this is only where the run is.
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::db::Db;
use crate::error::{io, json};
use crate::events::{self, Event, EventKind};
use crate::{Error, Result, clock};

pub const STATE_FILE: &str = "state.json";

/// `<NNN>-<yyyymmdd>-<4 hex>`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RunId(String);

impl RunId {
    pub fn new(feature: &str) -> RunId {
        Self::with_salt(feature, salt())
    }

    fn with_salt(feature: &str, salt: u32) -> RunId {
        RunId(format!(
            "{feature}-{}-{:04x}",
            clock::today(),
            salt & 0xffff
        ))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RunId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

fn salt() -> u32 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    nanos ^ std::process::id().rotate_left(7)
}

/// Where the run is. `waiting-owner` covers parked asks, blocks and the
/// budget; `paused` is the quota pause (FR-9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Phase {
    Starting,
    Building,
    Checkpoint,
    WaitingOwner,
    Paused,
    Closeout,
    Finished,
    Failed,
}

impl Phase {
    pub fn as_str(self) -> &'static str {
        match self {
            Phase::Starting => "starting",
            Phase::Building => "building",
            Phase::Checkpoint => "checkpoint",
            Phase::WaitingOwner => "waiting-owner",
            Phase::Paused => "paused",
            Phase::Closeout => "closeout",
            Phase::Finished => "finished",
            Phase::Failed => "failed",
        }
    }

    /// Nothing resumes a finished or failed run; `factory run` starts a new one.
    pub fn is_over(self) -> bool {
        matches!(self, Phase::Finished | Phase::Failed)
    }
}

impl fmt::Display for Phase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// `state.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunState {
    pub id: RunId,
    pub repo: PathBuf,
    pub feature: String,
    pub branch: Option<String>,
    pub phase: Phase,
    pub pid: u32,
    pub started_at: String,
    pub updated_at: String,
    pub budget_weighted_tokens: u64,
    pub stop_requested: bool,
    /// FR-7: the run ended on `factory stop`, not at its end. Phase is
    /// `finished`; `factory run NNN` resumes from the board.
    #[serde(default)]
    pub stopped: bool,
    /// FR-9: no spawn until this epoch second; `None` when not paused. Kept
    /// in `state.json` so a resumed driver honours a pause it did not see.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paused_until: Option<u64>,
    /// FR-6: the draft PR once `gh pr create` returned it; a resume edits
    /// the PR instead of opening a second one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pr_url: Option<String>,
    /// Every worker this run spawned, in order (slice 03). A resume reads the
    /// pids to tell a dead worker from a live one; the outcomes carry the
    /// once-only respawn cap across driver restarts.
    #[serde(default)]
    pub workers: Vec<WorkerRef>,
}

/// One spawned worker as `state.json` remembers it. A build worker has a
/// `slice`; a fix or checkpoint worker (FR-4) has the `checkpoint` it served.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerRef {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slice: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkpoint: Option<u32>,
    pub role: String,
    pub session_id: String,
    pub pid: u32,
    /// The `workers` table row.
    pub row_id: i64,
    pub spawned_at: String,
    /// `None` while running or unsettled; else a [`WorkerOutcome`] literal.
    pub outcome: Option<String>,
}

impl WorkerRef {
    /// Counts toward a death cap (FR-1, FR-4). Routed through
    /// [`WorkerOutcome::counts_as_death`] so a new outcome has to decide.
    pub fn died(&self) -> bool {
        self.outcome
            .as_deref()
            .and_then(WorkerOutcome::parse)
            .is_some_and(WorkerOutcome::counts_as_death)
    }
}

/// How a worker ended, as `state.json` and the `workers` table spell it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerOutcome {
    Done,
    Blocked,
    Dead,
    Failed,
    /// FR-9: the worker hit a usage limit; the run pauses, the slice goes
    /// back to `pending`.
    RateLimited,
}

impl WorkerOutcome {
    pub fn as_str(self) -> &'static str {
        match self {
            WorkerOutcome::Done => "done",
            WorkerOutcome::Blocked => "blocked",
            WorkerOutcome::Dead => "dead",
            WorkerOutcome::Failed => "failed",
            WorkerOutcome::RateLimited => "rate_limited",
        }
    }

    pub fn parse(s: &str) -> Option<WorkerOutcome> {
        Some(match s {
            "done" => WorkerOutcome::Done,
            "blocked" => WorkerOutcome::Blocked,
            "dead" => WorkerOutcome::Dead,
            "failed" => WorkerOutcome::Failed,
            "rate_limited" => WorkerOutcome::RateLimited,
            _ => return None,
        })
    }

    /// FR-9: a pause counts toward no cap. Exhaustive on purpose: a new
    /// outcome must say whether the once-only respawn (FR-1) and the three
    /// red gates (FR-4) see it.
    pub fn counts_as_death(self) -> bool {
        match self {
            WorkerOutcome::Dead | WorkerOutcome::Failed => true,
            WorkerOutcome::Done | WorkerOutcome::Blocked | WorkerOutcome::RateLimited => false,
        }
    }
}

/// An open run: its directory, its state and the store.
pub struct Run {
    dir: PathBuf,
    state: RunState,
    db: Db,
    /// The gate and inner commands the feature declares, read once per
    /// driver process (FR-8: a worker's `Bash` call containing one is an
    /// inner-loop run). Not persisted: `None` until the scheduler sets it.
    gate_cmds: Option<GateCmds>,
}

/// The feature's `gate` and `inner` command strings, as `docs gate` prints them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GateCmds {
    pub gate: String,
    pub inner: Option<String>,
}

impl Run {
    pub fn runs_dir(data_dir: &Path) -> PathBuf {
        data_dir.join("runs")
    }

    pub fn db_path(data_dir: &Path) -> PathBuf {
        data_dir.join("factory.db")
    }

    /// A new run directory with its state file, an empty `events.jsonl` and
    /// its `runs` row, phase `starting`, `pid` this process.
    pub fn create(
        data_dir: &Path,
        repo: &Path,
        feature: &str,
        branch: Option<String>,
        budget_weighted_tokens: u64,
    ) -> Result<Run> {
        let repo = canonical(repo)?;
        let runs = Self::runs_dir(data_dir);
        fs::create_dir_all(&runs).map_err(io(format!("mkdir {}", runs.display())))?;
        let mut id = RunId::new(feature);
        let mut bump = salt();
        while runs.join(id.as_str()).exists() {
            bump = bump.wrapping_add(1);
            id = RunId::with_salt(feature, bump);
        }
        let dir = runs.join(id.as_str());
        fs::create_dir(&dir).map_err(io(format!("mkdir {}", dir.display())))?;
        let now = clock::now_rfc3339();
        let state = RunState {
            id,
            repo,
            feature: feature.to_string(),
            branch,
            phase: Phase::Starting,
            pid: std::process::id(),
            started_at: now.clone(),
            updated_at: now,
            budget_weighted_tokens,
            stop_requested: false,
            stopped: false,
            paused_until: None,
            pr_url: None,
            workers: Vec::new(),
        };
        write_state(&dir, &state)?;
        fs::write(dir.join(events::FILE), "")
            .map_err(io(format!("create {}", dir.join(events::FILE).display())))?;
        let db = Db::open(&Self::db_path(data_dir))?;
        db.insert_run(&state)?;
        Ok(Run {
            dir,
            state,
            db,
            gate_cmds: None,
        })
    }

    /// The run `id` under `data_dir`.
    pub fn open(data_dir: &Path, id: &str) -> Result<Run> {
        let dir = Self::runs_dir(data_dir).join(id);
        if !dir.join(STATE_FILE).is_file() {
            return Err(Error::Msg(format!(
                "no run {id} under {}",
                Self::runs_dir(data_dir).display()
            )));
        }
        let state = load_state(&dir)?;
        let db = Db::open(&Self::db_path(data_dir))?;
        Ok(Run {
            dir,
            state,
            db,
            gate_cmds: None,
        })
    }

    /// The run for this repo and feature whose process is alive, if any.
    pub fn find_live(data_dir: &Path, repo: &Path, feature: &str) -> Result<Option<Run>> {
        let repo = canonical(repo)?;
        let found = list(data_dir, &repo, feature)?
            .into_iter()
            .find(|s| pid_alive(s.pid));
        found
            .map(|s| Self::open(data_dir, s.id.as_str()))
            .transpose()
    }

    /// The latest run for this repo and feature that is neither finished nor
    /// failed: the one `factory run` resumes (AD-203).
    pub fn find_resumable(data_dir: &Path, repo: &Path, feature: &str) -> Result<Option<Run>> {
        let repo = canonical(repo)?;
        let found = list(data_dir, &repo, feature)?
            .into_iter()
            .rev()
            .find(|s| !s.phase.is_over());
        found
            .map(|s| Self::open(data_dir, s.id.as_str()))
            .transpose()
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn state(&self) -> &RunState {
        &self.state
    }

    pub fn db(&self) -> &Db {
        &self.db
    }

    pub fn id(&self) -> &RunId {
        &self.state.id
    }

    pub fn gate_cmds(&self) -> Option<&GateCmds> {
        self.gate_cmds.as_ref()
    }

    pub fn set_gate_cmds(&mut self, cmds: GateCmds) {
        self.gate_cmds = Some(cmds);
    }

    /// Append to `events.jsonl` and the `events` table.
    pub fn emit(&mut self, kind: EventKind) -> Result<Event> {
        events::emit(&self.dir, &self.db, self.state.id.as_str(), kind)
    }

    /// Every event so far, from the file.
    pub fn events(&self) -> Result<Vec<Event>> {
        events::read_all(&self.dir)
    }

    pub fn set_phase(&mut self, phase: Phase) -> Result<()> {
        self.state.phase = phase;
        self.save()
    }

    pub fn set_branch(&mut self, branch: String) -> Result<()> {
        self.state.branch = Some(branch);
        self.save()
    }

    pub fn set_pr_url(&mut self, url: String) -> Result<()> {
        self.state.pr_url = Some(url);
        self.save()
    }

    pub fn set_budget(&mut self, budget_weighted_tokens: u64) -> Result<()> {
        self.state.budget_weighted_tokens = budget_weighted_tokens;
        self.save()
    }

    pub fn request_stop(&mut self) -> Result<()> {
        self.state.stop_requested = true;
        self.save()
    }

    /// FR-7: the stop honoured — `finished` with `stopped: true`, one save.
    pub fn set_stopped(&mut self) -> Result<()> {
        self.state.stopped = true;
        self.state.phase = Phase::Finished;
        self.save()
    }

    /// FR-9: set or clear the pause.
    pub fn set_paused_until(&mut self, until: Option<u64>) -> Result<()> {
        self.state.paused_until = until;
        self.save()
    }

    pub fn add_worker(&mut self, w: WorkerRef) -> Result<()> {
        self.state.workers.push(w);
        self.save()
    }

    /// Settle a worker: `done`, `blocked`, `dead` or `failed`.
    pub fn set_worker_outcome(&mut self, session_id: &str, outcome: &str) -> Result<()> {
        if let Some(w) = self
            .state
            .workers
            .iter_mut()
            .rev()
            .find(|w| w.session_id == session_id)
        {
            w.outcome = Some(outcome.to_string());
        }
        self.save()
    }

    /// This process takes the run over (a resume): `pid` becomes ours.
    pub fn adopt(&mut self) -> Result<()> {
        self.state.pid = std::process::id();
        self.save()
    }

    /// Reload `state.json` after another process (`factory stop`) wrote it.
    pub fn reload(&mut self) -> Result<()> {
        self.state = load_state(&self.dir)?;
        Ok(())
    }

    fn save(&mut self) -> Result<()> {
        // `factory stop` writes `stop_requested` from another process while
        // the loop runs; a save from the loop's stale copy must not undo it.
        if let Ok(disk) = load_state(&self.dir) {
            self.state.stop_requested |= disk.stop_requested;
        }
        self.state.updated_at = clock::now_rfc3339();
        write_state(&self.dir, &self.state)?;
        self.db.update_run(&self.state)
    }
}

/// `<run_dir>/state.json`, parsed.
pub fn load_state(run_dir: &Path) -> Result<RunState> {
    let path = run_dir.join(STATE_FILE);
    let text = fs::read_to_string(&path).map_err(io(format!("read {}", path.display())))?;
    serde_json::from_str(&text).map_err(json(format!("parse {}", path.display())))
}

/// Write `state.json` whole, through a rename so a reader never sees half.
pub fn write_state(run_dir: &Path, state: &RunState) -> Result<()> {
    let path = run_dir.join(STATE_FILE);
    let tmp = run_dir.join(format!("{STATE_FILE}.tmp"));
    let text = serde_json::to_string_pretty(state).map_err(json("serialize state"))?;
    fs::write(&tmp, text + "\n").map_err(io(format!("write {}", tmp.display())))?;
    fs::rename(&tmp, &path).map_err(io(format!("rename {}", path.display())))
}

/// Every state file for `repo` and `feature`, oldest first.
fn list(data_dir: &Path, repo: &Path, feature: &str) -> Result<Vec<RunState>> {
    Ok(list_all(data_dir, repo)?
        .into_iter()
        .filter(|s| s.feature == feature)
        .collect())
}

/// Every run of `repo`, any feature, that is neither finished nor failed,
/// oldest first: `factory answer ASK-n` finds the one that waits on the ask.
pub fn list_open(data_dir: &Path, repo: &Path) -> Result<Vec<RunState>> {
    let repo = canonical(repo)?;
    Ok(list_all(data_dir, &repo)?
        .into_iter()
        .filter(|s| !s.phase.is_over())
        .collect())
}

/// Every state file for `repo`, oldest first.
fn list_all(data_dir: &Path, repo: &Path) -> Result<Vec<RunState>> {
    Ok(list_runs(data_dir)?
        .into_iter()
        .filter(|s| s.repo == repo)
        .collect())
}

/// Every run under `data_dir`, any repo, oldest first: what `factory status`
/// lists when it is given no run or an unknown one (FR-7).
pub fn list_runs(data_dir: &Path) -> Result<Vec<RunState>> {
    let runs = Run::runs_dir(data_dir);
    let entries = match fs::read_dir(&runs) {
        Ok(rd) => rd,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(io(format!("read {}", runs.display()))(e)),
    };
    let mut out = Vec::new();
    for entry in entries {
        let entry = entry.map_err(io(format!("read {}", runs.display())))?;
        let dir = entry.path();
        if !dir.join(STATE_FILE).is_file() {
            continue;
        }
        out.push(load_state(&dir)?);
    }
    out.sort_by(|a, b| {
        a.started_at
            .cmp(&b.started_at)
            .then_with(|| a.id.as_str().cmp(b.id.as_str()))
    });
    Ok(out)
}

fn canonical(repo: &Path) -> Result<PathBuf> {
    repo.canonicalize()
        .map_err(io(format!("canonicalize {}", repo.display())))
}

/// Is a process with this pid running? `kill -0` semantics read from
/// `/proc/<pid>/stat`, so a zombie counts as gone (AD-202). Linux only, which
/// is the PRD's target (§19).
pub fn pid_alive(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    let Ok(stat) = fs::read_to_string(format!("/proc/{pid}/stat")) else {
        return false;
    };
    // `pid (comm) S ...` — comm may hold spaces and parens, so split after the last ')'.
    let state = stat
        .rsplit_once(')')
        .map(|(_, rest)| rest.trim_start())
        .and_then(|rest| rest.chars().next());
    !matches!(state, None | Some('Z') | Some('X'))
}
