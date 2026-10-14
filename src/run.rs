//! Every write of `state.json` is a locked read-modify-write; once the file names another pid, this driver's writes fail.
use std::collections::BTreeMap;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::db::Db;
use crate::error::{io, json};
use crate::events::{self, Event, EventKind};
use crate::{Error, Result, clock, paths};

pub const STATE_FILE: &str = "state.json";
pub const LOCK_FILE: &str = "state.lock";

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

/// `WaitingOwner` covers asks, blocks and the budget; `Paused` is the usage-limit pause.
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

    pub fn is_over(self) -> bool {
        matches!(self, Phase::Finished | Phase::Failed)
    }
}

impl fmt::Display for Phase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

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
    #[serde(default)]
    pub stopped: bool,
    /// Epoch second before which nothing spawns; persisted so a resumed driver honours a pause it did not see.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paused_until: Option<u64>,
    /// `paused_until` was guessed (no `resetsAt`); a limit that gives one replaces it.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub paused_guessed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pr_url: Option<String>,
    #[serde(default)]
    pub workers: Vec<WorkerRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lanes_set_up: Vec<u32>,
    /// The sha each lane was last reset to: the base a resumed driver commits the slice's lane on.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub lane_bases: BTreeMap<u32, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid_start: Option<u64>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub worker_starts: BTreeMap<String, u64>,
}

impl RunState {
    pub fn driver_alive(&self) -> bool {
        alive(self.pid, self.pid_start)
    }

    pub fn worker_alive(&self, w: &WorkerRef) -> bool {
        alive(w.pid, self.worker_starts.get(&w.session_id).copied())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerRef {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slice: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkpoint: Option<u32>,
    /// A slice's lane is also on the board; a checkpoint worker's lane is only here.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lane: Option<u32>,
    pub role: String,
    pub session_id: String,
    pub pid: u32,
    pub row_id: i64,
    pub spawned_at: String,
    pub outcome: Option<String>,
}

impl WorkerRef {
    pub fn died(&self) -> bool {
        self.outcome
            .as_deref()
            .and_then(WorkerOutcome::parse)
            .is_some_and(WorkerOutcome::counts_as_death)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerOutcome {
    Done,
    Blocked,
    Dead,
    Failed,
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

    /// Exhaustive on purpose: a new outcome must say whether the death caps see it.
    pub fn counts_as_death(self) -> bool {
        match self {
            WorkerOutcome::Dead | WorkerOutcome::Failed => true,
            WorkerOutcome::Done | WorkerOutcome::Blocked | WorkerOutcome::RateLimited => false,
        }
    }
}

pub struct Run {
    dir: PathBuf,
    state: RunState,
    db: Db,
    gate_cmds: Option<GateCmds>,
    owner: bool,
}

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

    pub fn create(
        data_dir: &Path,
        repo: &Path,
        feature: &str,
        branch: Option<String>,
        budget_weighted_tokens: u64,
    ) -> Result<Run> {
        let repo = canonical(repo)?;
        let runs = Self::runs_dir(data_dir);
        paths::private_dir_all(&runs).map_err(io(format!("mkdir {}", runs.display())))?;
        let mut id = RunId::new(feature);
        let mut bump = salt();
        while runs.join(id.as_str()).exists() {
            bump = bump.wrapping_add(1);
            id = RunId::with_salt(feature, bump);
        }
        let dir = runs.join(id.as_str());
        // Not recursive: a directory another run made meanwhile is an error, not a shared one.
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&dir)
            .map_err(io(format!("mkdir {}", dir.display())))?;
        let now = clock::now_rfc3339();
        let state = RunState {
            id,
            repo,
            feature: feature.to_string(),
            branch,
            phase: Phase::Starting,
            pid: std::process::id(),
            pid_start: start_time(std::process::id()),
            worker_starts: BTreeMap::new(),
            started_at: now.clone(),
            updated_at: now,
            budget_weighted_tokens,
            stop_requested: false,
            stopped: false,
            paused_until: None,
            paused_guessed: false,
            pr_url: None,
            workers: Vec::new(),
            lanes_set_up: Vec::new(),
            lane_bases: BTreeMap::new(),
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
            owner: true,
        })
    }

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
            owner: false,
        })
    }

    pub fn find_live(data_dir: &Path, repo: &Path, feature: &str) -> Result<Option<Run>> {
        let repo = canonical(repo)?;
        let found = list(data_dir, &repo, feature)?
            .into_iter()
            .find(|s| s.driver_alive());
        found
            .map(|s| Self::open(data_dir, s.id.as_str()))
            .transpose()
    }

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

    pub fn emit(&mut self, kind: EventKind) -> Result<Event> {
        events::emit(&self.dir, &self.db, self.state.id.as_str(), kind)
    }

    pub fn events(&self) -> Result<Vec<Event>> {
        events::read_all(&self.dir)
    }

    pub fn set_phase(&mut self, phase: Phase) -> Result<()> {
        self.update(|s| s.phase = phase)
    }

    pub fn set_branch(&mut self, branch: String) -> Result<()> {
        self.update(|s| s.branch = Some(branch))
    }

    pub fn set_pr_url(&mut self, url: String) -> Result<()> {
        self.update(|s| s.pr_url = Some(url))
    }

    pub fn set_budget(&mut self, budget_weighted_tokens: u64) -> Result<()> {
        self.update(|s| s.budget_weighted_tokens = budget_weighted_tokens)
    }

    pub fn request_stop(&mut self) -> Result<()> {
        self.update(|s| s.stop_requested = true)
    }

    pub fn set_stopped(&mut self) -> Result<()> {
        self.update(|s| {
            s.stopped = true;
            s.phase = Phase::Finished;
        })
    }

    pub fn set_pause(&mut self, until: u64, guessed: bool) -> Result<()> {
        self.update(|s| {
            s.paused_until = Some(until);
            s.paused_guessed = guessed;
        })
    }

    pub fn clear_pause(&mut self) -> Result<()> {
        self.update(|s| {
            s.paused_until = None;
            s.paused_guessed = false;
        })
    }

    pub fn add_worker(&mut self, w: WorkerRef) -> Result<()> {
        let start = start_time(w.pid);
        self.update(|s| {
            if let Some(start) = start {
                s.worker_starts.insert(w.session_id.clone(), start);
            }
            s.workers.push(w)
        })
    }

    pub fn mark_lane_set_up(&mut self, n: u32) -> Result<()> {
        self.update(|s| {
            if !s.lanes_set_up.contains(&n) {
                s.lanes_set_up.push(n);
            }
        })
    }

    pub fn set_lane_base(&mut self, n: u32, sha: &str) -> Result<()> {
        self.update(|s| {
            s.lane_bases.insert(n, sha.to_string());
        })
    }

    pub fn set_worker_outcome(&mut self, session_id: &str, outcome: &str) -> Result<()> {
        self.update(|s| {
            if let Some(w) = s
                .workers
                .iter_mut()
                .rev()
                .find(|w| w.session_id == session_id)
            {
                w.outcome = Some(outcome.to_string());
            }
        })
    }

    /// The parent is the one live driver that may be overridden: the `factory run` handing this run to its daemon.
    pub fn adopt(&mut self) -> Result<()> {
        let me = std::process::id();
        let parent = std::os::unix::process::parent_id();
        let _lock = lock_state(&self.dir)?;
        let mut disk = load_state(&self.dir)?;
        if disk.pid != me && disk.pid != parent && disk.driver_alive() {
            return Err(Error::Msg(format!(
                "run {} is driven by pid {} — not taken over",
                disk.id, disk.pid
            )));
        }
        disk.pid = me;
        disk.pid_start = start_time(me);
        self.owner = true;
        self.write(disk)
    }

    pub fn reload(&mut self) -> Result<()> {
        self.state = load_state(&self.dir)?;
        Ok(())
    }

    pub fn taken_over(&self) -> Option<u32> {
        (self.owner && self.state.pid != std::process::id()).then_some(self.state.pid)
    }

    pub fn lost_to(&self) -> Option<u32> {
        if !self.owner {
            return None;
        }
        load_state(&self.dir)
            .ok()
            .map(|s| s.pid)
            .filter(|pid| *pid != std::process::id())
    }

    fn update(&mut self, f: impl FnOnce(&mut RunState)) -> Result<()> {
        let _lock = lock_state(&self.dir)?;
        let mut disk = load_state(&self.dir)?;
        let me = std::process::id();
        if self.owner && disk.pid != me {
            self.state = disk;
            return Err(Error::Msg(format!(
                "run {} was taken over by pid {} — this driver (pid {me}) writes nothing more",
                self.state.id, self.state.pid
            )));
        }
        f(&mut disk);
        self.write(disk)
    }

    fn write(&mut self, mut state: RunState) -> Result<()> {
        state.updated_at = clock::now_rfc3339();
        write_state(&self.dir, &state)?;
        self.state = state;
        self.db.update_run(&self.state)
    }
}

fn lock_state(run_dir: &Path) -> Result<File> {
    let path = run_dir.join(LOCK_FILE);
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&path)
        .map_err(io(format!("open {}", path.display())))?;
    file.lock()
        .map_err(io(format!("lock {}", path.display())))?;
    Ok(file)
}

pub fn load_state(run_dir: &Path) -> Result<RunState> {
    let path = run_dir.join(STATE_FILE);
    let text = fs::read_to_string(&path).map_err(io(format!("read {}", path.display())))?;
    serde_json::from_str(&text).map_err(json(format!("parse {}", path.display())))
}

/// The temp file carries the pid so two writers never share one; the rename keeps readers whole.
pub fn write_state(run_dir: &Path, state: &RunState) -> Result<()> {
    let path = run_dir.join(STATE_FILE);
    let tmp = run_dir.join(format!("{STATE_FILE}.{}.tmp", std::process::id()));
    let text = serde_json::to_string_pretty(state).map_err(json("serialize state"))?;
    fs::write(&tmp, text + "\n").map_err(io(format!("write {}", tmp.display())))?;
    fs::rename(&tmp, &path).map_err(io(format!("rename {}", path.display())))
}

fn list(data_dir: &Path, repo: &Path, feature: &str) -> Result<Vec<RunState>> {
    Ok(list_all(data_dir, repo)?
        .into_iter()
        .filter(|s| s.feature == feature)
        .collect())
}

pub fn list_open(data_dir: &Path, repo: &Path) -> Result<Vec<RunState>> {
    let repo = canonical(repo)?;
    Ok(list_all(data_dir, &repo)?
        .into_iter()
        .filter(|s| !s.phase.is_over())
        .collect())
}

fn list_all(data_dir: &Path, repo: &Path) -> Result<Vec<RunState>> {
    Ok(list_runs(data_dir)?
        .into_iter()
        .filter(|s| s.repo == repo)
        .collect())
}

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

/// The start time tells a reused pid from the process that was recorded.
pub fn alive(pid: u32, start: Option<u64>) -> bool {
    pid_alive(pid) && start.is_none_or(|s| start_time(pid) == Some(s))
}

pub fn start_time(pid: u32) -> Option<u64> {
    if pid == 0 {
        return None;
    }
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    // `starttime` is stat field 22; after `pid (comm)` the fields run from 3, so the 20th.
    let (_, rest) = stat.rsplit_once(')')?;
    rest.split_whitespace().nth(19)?.parse().ok()
}

/// Unlike `kill -0`, a zombie counts as gone.
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
