//! The SQLite store at `<data_dir>/factory.db` (PRD §10 FR-8): `runs`,
//! `workers`, `gates`, `events`, written as events happen. Migrations are
//! idempotent: `CREATE TABLE IF NOT EXISTS` plus a `schema_version` row, and
//! a guarded `ALTER TABLE` per column added after a table shipped.
//! `tool_calls` landed in slice 08.
use std::path::Path;
use std::time::Duration;

use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};

use crate::run::RunState;
use crate::stream::{Findings, Summary};
use crate::{Result, clock};

pub const SCHEMA_VERSION: i64 = 2;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS schema_version (
  version    INTEGER NOT NULL PRIMARY KEY,
  applied_at TEXT    NOT NULL
);
CREATE TABLE IF NOT EXISTS runs (
  id                     TEXT PRIMARY KEY,
  repo                   TEXT NOT NULL,
  feature                TEXT NOT NULL,
  branch                 TEXT,
  started_at             TEXT NOT NULL,
  finished_at            TEXT,
  phase                  TEXT NOT NULL,
  budget_weighted_tokens INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS workers (
  id              INTEGER PRIMARY KEY AUTOINCREMENT,
  run_id          TEXT NOT NULL REFERENCES runs(id),
  role            TEXT NOT NULL,
  slice           INTEGER,
  session_id      TEXT,
  model           TEXT,
  effort          TEXT,
  input           INTEGER,
  cache_write     INTEGER,
  cache_read      INTEGER,
  output          INTEGER,
  thinking        INTEGER,
  cost_usd        REAL,
  num_turns       INTEGER,
  duration_ms     INTEGER,
  terminal_reason TEXT,
  outcome         TEXT,
  peak_context    INTEGER,
  started_at      TEXT NOT NULL,
  ended_at        TEXT,
  commit_sha      TEXT,
  checkpoint      INTEGER
);
CREATE TABLE IF NOT EXISTS gates (
  id         INTEGER PRIMARY KEY AUTOINCREMENT,
  run_id     TEXT NOT NULL REFERENCES runs(id),
  source     TEXT NOT NULL,
  kind       TEXT NOT NULL,
  cmd        TEXT NOT NULL,
  red        INTEGER NOT NULL,
  minutes    REAL NOT NULL,
  at         TEXT NOT NULL,
  worker_id  INTEGER,
  cached     INTEGER NOT NULL DEFAULT 0,
  checkpoint INTEGER,
  attempt    INTEGER
);
CREATE TABLE IF NOT EXISTS events (
  id      INTEGER PRIMARY KEY AUTOINCREMENT,
  run_id  TEXT NOT NULL REFERENCES runs(id),
  seq     INTEGER NOT NULL,
  ts      TEXT NOT NULL,
  kind    TEXT NOT NULL,
  payload TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS tool_calls (
  id               INTEGER PRIMARY KEY AUTOINCREMENT,
  run_id           TEXT NOT NULL REFERENCES runs(id),
  worker_id        INTEGER NOT NULL REFERENCES workers(id),
  seq              INTEGER NOT NULL,
  tool             TEXT NOT NULL,
  kind             TEXT NOT NULL,
  result_chars     INTEGER,
  target_hash      TEXT NOT NULL,
  repeat_in_worker INTEGER NOT NULL DEFAULT 0,
  repeat_in_run    INTEGER NOT NULL DEFAULT 0,
  ts               TEXT,
  label_followup   INTEGER,
  label_expanded   INTEGER,
  pack_doc         INTEGER
);
";

pub struct Db {
    conn: Connection,
}

/// One `events` row, as tests and the cockpit read it back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventRow {
    pub seq: u64,
    pub ts: String,
    pub kind: String,
    pub payload: String,
}

/// One `runs` row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunRow {
    pub id: String,
    pub repo: String,
    pub feature: String,
    pub branch: Option<String>,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub phase: String,
    pub budget_weighted_tokens: u64,
    /// The draft PR (FR-6), once opened.
    pub pr_url: Option<String>,
    /// PRD-003 FR-23: what the run resolved at its start, as JSON
    /// (`config::knobs`); null on a run from before it.
    pub knobs: Option<String>,
    /// The one line `factory run --hypothesis` gave; most runs have none.
    pub hypothesis: Option<String>,
    /// PRD-003 FR-24: the polish return's `FINDINGS` line; `None` when it
    /// had none or polish has not returned.
    pub findings: Option<Findings>,
    /// `docs status` `log` counts at ship: `AD` entries, `decide` ones, blocks.
    pub decisions: Option<u32>,
    pub decide: Option<u32>,
    pub blocks: Option<u32>,
    /// PRD-003 FR-25: the branch's changed lines against its base, stored at
    /// ship; `None` before it or when `git diff` failed.
    pub lines_added: Option<u64>,
    pub lines_removed: Option<u64>,
}

/// One `workers` row.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct WorkerRow {
    pub id: i64,
    pub run_id: String,
    pub role: String,
    pub slice: Option<u32>,
    pub session_id: Option<String>,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub input: Option<i64>,
    pub cache_write: Option<i64>,
    pub cache_read: Option<i64>,
    pub output: Option<i64>,
    pub thinking: Option<i64>,
    pub cost_usd: Option<f64>,
    pub num_turns: Option<i64>,
    pub duration_ms: Option<i64>,
    pub terminal_reason: Option<String>,
    pub outcome: Option<String>,
    pub peak_context: Option<i64>,
    pub started_at: String,
    pub ended_at: Option<String>,
    /// The slice's commit (FR-5), once the driver made it.
    pub commit_sha: Option<String>,
    /// The checkpoint a `fix` or `checkpoint` worker served (FR-4).
    pub checkpoint: Option<u32>,
    /// The first assistant message's context (FR-8 `start_context`).
    pub start_context: Option<i64>,
    /// The repo-relative files the worker's pack held (slice 13), from the
    /// pack header; empty for a worker spawned before slice 13.
    pub pack_files: Vec<String>,
    /// The pack's sections and their chars as JSON, `[{"section", "chars"}]`
    /// (PRD-003 FR-22); null for a worker spawned before it.
    pub pack_parts: Option<String>,
    /// What the worker's subagents used, from its transcript dir (PRD-003
    /// FR-21); null when not read.
    pub sub_input: Option<i64>,
    pub sub_cache_write: Option<i64>,
    pub sub_cache_read: Option<i64>,
    pub sub_output: Option<i64>,
    /// How many subagents had an assistant message; null when not read.
    pub subagents: Option<i64>,
    /// The model of the worker's first own assistant message: what
    /// answered, beside `model`, what was asked for (PRD-003 FR-23).
    pub model_id: Option<String>,
}

/// One `tool_calls` row to insert (FR-8).
#[derive(Debug, Clone, PartialEq)]
pub struct ToolCallInsert<'a> {
    pub run_id: &'a str,
    pub worker_id: i64,
    pub seq: u32,
    pub tool: &'a str,
    pub kind: &'a str,
    pub result_chars: Option<i64>,
    pub target_hash: &'a str,
    pub repeat_in_worker: bool,
    pub repeat_in_run: bool,
    pub ts: Option<&'a str>,
    /// On a `ctx-*` row: a named file read within the next two calls.
    pub label_followup: Option<bool>,
    /// On a `read` or `ctx-refs` row: the same target widened within five.
    pub label_expanded: Option<bool>,
    /// A `read`/`bash-cat` of a file the worker's pack held (slice 13).
    pub pack_doc: bool,
    /// The call came from one of the worker's subagents (PRD-003 FR-21).
    pub in_subagent: bool,
}

/// One `tool_calls` row, as tests and the report read it back.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolCallRow {
    pub id: i64,
    pub run_id: String,
    pub worker_id: i64,
    pub seq: u32,
    pub tool: String,
    pub kind: String,
    pub result_chars: Option<i64>,
    pub target_hash: String,
    pub repeat_in_worker: bool,
    pub repeat_in_run: bool,
    pub ts: Option<String>,
    pub label_followup: Option<bool>,
    pub label_expanded: Option<bool>,
    /// Null (a row from before slice 13) reads as false.
    pub pack_doc: bool,
    pub in_subagent: bool,
}

/// One `gates` row to insert (FR-4, FR-8).
#[derive(Debug, Clone, PartialEq)]
pub struct GateInsert<'a> {
    pub run_id: &'a str,
    /// `driver` for the checkpoint gate, `worker` for a worker's inner run.
    pub source: &'a str,
    /// `checkpoint` for the driver's, `slice` for a worker's, later `ci`.
    pub kind: &'a str,
    pub cmd: &'a str,
    pub red: bool,
    pub minutes: f64,
    pub at: &'a str,
    pub worker_id: Option<i64>,
    /// The cache answered: no process ran, `minutes` is 0.
    pub cached: bool,
    pub checkpoint: Option<u32>,
    pub attempt: Option<u32>,
}

/// One `gates` row, as tests and the report read it back.
#[derive(Debug, Clone, PartialEq)]
pub struct GateRow {
    pub id: i64,
    pub run_id: String,
    pub source: String,
    pub kind: String,
    pub cmd: String,
    pub red: bool,
    pub minutes: f64,
    pub at: String,
    pub worker_id: Option<i64>,
    pub cached: bool,
    pub checkpoint: Option<u32>,
    pub attempt: Option<u32>,
}

impl Db {
    /// Open (creating the parent directory) and migrate.
    pub fn open(path: &Path) -> Result<Db> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(crate::error::io(format!("mkdir {}", parent.display())))?;
        }
        let conn = Connection::open(path)?;
        conn.busy_timeout(Duration::from_secs(5))?;
        let db = Db { conn };
        db.migrate()?;
        Ok(db)
    }

    /// One migrator at a time: the column check and its `ALTER` run in one
    /// write transaction, so two first opens (a driver and a hook) cannot
    /// both see a column missing and the second fail on a duplicate.
    fn migrate(&self) -> Result<()> {
        let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Immediate)?;
        tx.execute_batch(SCHEMA)?;
        // Columns added after a table shipped: `ALTER TABLE` is not
        // `IF NOT EXISTS`, so each is guarded by the live column list.
        for (table, column, decl) in [
            ("workers", "commit_sha", "TEXT"),
            ("workers", "checkpoint", "INTEGER"),
            ("gates", "cached", "INTEGER NOT NULL DEFAULT 0"),
            ("gates", "checkpoint", "INTEGER"),
            ("gates", "attempt", "INTEGER"),
            ("workers", "start_context", "INTEGER"),
            ("runs", "pr_url", "TEXT"),
            ("workers", "pack_files", "TEXT"),
            ("tool_calls", "pack_doc", "INTEGER"),
            // v2 (PRD-003). Null `sub_*` means not read: an older row, or a
            // transcript dir that could not be read; zero means none.
            ("workers", "sub_input", "INTEGER"),
            ("workers", "sub_cache_write", "INTEGER"),
            ("workers", "sub_cache_read", "INTEGER"),
            ("workers", "sub_output", "INTEGER"),
            ("workers", "subagents", "INTEGER"),
            ("tool_calls", "in_subagent", "INTEGER NOT NULL DEFAULT 0"),
            // FR-22: the pack's sections and their chars, as JSON.
            ("workers", "pack_parts", "TEXT"),
            // FR-23: the knobs a run resolved, its hypothesis, and the model
            // that answered a worker (its first own assistant message).
            ("runs", "knobs", "TEXT"),
            ("runs", "hypothesis", "TEXT"),
            ("workers", "model_id", "TEXT"),
            // FR-24: the polish `FINDINGS` line (null when it had none) and
            // the agent log's counts at ship.
            ("runs", "findings_high", "INTEGER"),
            ("runs", "findings_medium", "INTEGER"),
            ("runs", "findings_low", "INTEGER"),
            ("runs", "findings_fixed", "INTEGER"),
            ("runs", "findings_parked", "INTEGER"),
            ("runs", "decisions", "INTEGER"),
            ("runs", "decide", "INTEGER"),
            ("runs", "blocks", "INTEGER"),
            // FR-25: `git diff --shortstat <base>..HEAD` at ship.
            ("runs", "lines_added", "INTEGER"),
            ("runs", "lines_removed", "INTEGER"),
        ] {
            if !self.has_column(table, column)? {
                tx.execute(
                    &format!("ALTER TABLE {table} ADD COLUMN {column} {decl}"),
                    [],
                )?;
            }
        }
        tx.execute(
            "INSERT OR IGNORE INTO schema_version (version, applied_at) VALUES (?1, ?2)",
            params![SCHEMA_VERSION, clock::now_rfc3339()],
        )?;
        tx.commit()?;
        Ok(())
    }

    fn has_column(&self, table: &str, column: &str) -> Result<bool> {
        let mut stmt = self
            .conn
            .prepare(&format!("SELECT name FROM pragma_table_info('{table}')"))?;
        let names = stmt.query_map([], |r| r.get::<_, String>(0))?;
        for n in names {
            if n? == column {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    /// The highest applied version.
    pub fn schema_version(&self) -> Result<i64> {
        Ok(self
            .conn
            .query_row("SELECT max(version) FROM schema_version", [], |r| r.get(0))?)
    }

    /// The run row, written when the run dir is created.
    pub fn insert_run(&self, s: &RunState) -> Result<()> {
        self.conn.execute(
            "INSERT INTO runs (id, repo, feature, branch, started_at, finished_at, phase, budget_weighted_tokens)
             VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6, ?7)",
            params![
                s.id.as_str(),
                s.repo.to_string_lossy(),
                s.feature,
                s.branch,
                s.started_at,
                s.phase.as_str(),
                s.budget_weighted_tokens as i64,
            ],
        )?;
        Ok(())
    }

    /// Mirror the state file's mutable columns; `finished_at` is set once the
    /// phase is terminal and never cleared.
    pub fn update_run(&self, s: &RunState) -> Result<()> {
        let finished_at = s.phase.is_over().then(|| s.updated_at.clone());
        self.conn.execute(
            "UPDATE runs SET branch = ?2, phase = ?3, budget_weighted_tokens = ?4,
                             finished_at = COALESCE(finished_at, ?5), pr_url = ?6
             WHERE id = ?1",
            params![
                s.id.as_str(),
                s.branch,
                s.phase.as_str(),
                s.budget_weighted_tokens as i64,
                finished_at,
                s.pr_url,
            ],
        )?;
        Ok(())
    }

    pub fn run_row(&self, id: &str) -> Result<Option<RunRow>> {
        Ok(self
            .conn
            .query_row(
                "SELECT id, repo, feature, branch, started_at, finished_at, phase, budget_weighted_tokens, pr_url,
                        findings_high, findings_medium, findings_low, findings_fixed, findings_parked,
                        decisions, decide, blocks, knobs, hypothesis, lines_added, lines_removed
                 FROM runs WHERE id = ?1",
                [id],
                run_from_row,
            )
            .optional()?)
    }

    /// PRD-003 FR-25: the `limit` runs of `repo` that finished by `before`
    /// (a run's `started_at`), newest first, never `except` itself. A run
    /// that finished later is never in it, so a rerun picks the same cohort.
    /// The bound is inclusive: the store keeps whole seconds, and a run can
    /// start in the second the one before it finished.
    pub fn earlier_runs(
        &self,
        repo: &str,
        before: &str,
        except: &str,
        limit: u32,
    ) -> Result<Vec<RunRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, repo, feature, branch, started_at, finished_at, phase, budget_weighted_tokens, pr_url,
                    findings_high, findings_medium, findings_low, findings_fixed, findings_parked,
                    decisions, decide, blocks, knobs, hypothesis, lines_added, lines_removed
             FROM runs
             WHERE repo = ?1 AND phase = 'finished' AND finished_at IS NOT NULL AND finished_at <= ?2
               AND id != ?3
             ORDER BY finished_at DESC, id DESC LIMIT ?4",
        )?;
        let rows = stmt.query_map(params![repo, before, except, limit], run_from_row)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// FR-25: the changed lines at ship, or two nulls when `git diff` failed.
    pub fn set_run_lines(&self, run_id: &str, lines: Option<(u64, u64)>) -> Result<()> {
        self.conn.execute(
            "UPDATE runs SET lines_added = ?2, lines_removed = ?3 WHERE id = ?1",
            params![run_id, lines.map(|l| l.0 as i64), lines.map(|l| l.1 as i64)],
        )?;
        Ok(())
    }

    /// FR-24: the polish `FINDINGS` counts, or five nulls.
    pub fn set_run_findings(&self, run_id: &str, f: Option<Findings>) -> Result<()> {
        self.conn.execute(
            "UPDATE runs SET findings_high = ?2, findings_medium = ?3, findings_low = ?4,
                             findings_fixed = ?5, findings_parked = ?6
             WHERE id = ?1",
            params![
                run_id,
                f.map(|f| f.high),
                f.map(|f| f.medium),
                f.map(|f| f.low),
                f.map(|f| f.fixed),
                f.map(|f| f.parked),
            ],
        )?;
        Ok(())
    }

    /// FR-24: the agent log's `AD` entries, `decide` ones and blocks at ship.
    pub fn set_run_log_counts(
        &self,
        run_id: &str,
        decisions: u32,
        decide: u32,
        blocks: u32,
    ) -> Result<()> {
        self.conn.execute(
            "UPDATE runs SET decisions = ?2, decide = ?3, blocks = ?4 WHERE id = ?1",
            params![run_id, decisions, decide, blocks],
        )?;
        Ok(())
    }

    /// FR-23: the knobs JSON, written once at run start.
    pub fn set_run_knobs(&self, run_id: &str, knobs: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE runs SET knobs = ?2 WHERE id = ?1",
            params![run_id, knobs],
        )?;
        Ok(())
    }

    /// FR-23: the run's hypothesis, written once at run creation.
    pub fn set_run_hypothesis(&self, run_id: &str, hypothesis: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE runs SET hypothesis = ?2 WHERE id = ?1",
            params![run_id, hypothesis],
        )?;
        Ok(())
    }

    pub fn insert_event(
        &self,
        run_id: &str,
        seq: u64,
        ts: &str,
        kind: &str,
        payload: &str,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO events (run_id, seq, ts, kind, payload) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![run_id, seq as i64, ts, kind, payload],
        )?;
        Ok(())
    }

    /// A run's events in `seq` order.
    pub fn events(&self, run_id: &str) -> Result<Vec<EventRow>> {
        let mut stmt = self
            .conn
            .prepare("SELECT seq, ts, kind, payload FROM events WHERE run_id = ?1 ORDER BY seq")?;
        let rows = stmt.query_map([run_id], |r| {
            Ok(EventRow {
                seq: r.get::<_, i64>(0)? as u64,
                ts: r.get(1)?,
                kind: r.get(2)?,
                payload: r.get(3)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    /// A worker at spawn: `outcome = running`, usage still null. A build
    /// worker has a `slice`, a fix or checkpoint worker a `checkpoint`.
    /// Returns the row id.
    #[allow(clippy::too_many_arguments)]
    pub fn insert_worker(
        &self,
        run_id: &str,
        role: &str,
        slice: Option<u32>,
        checkpoint: Option<u32>,
        session_id: &str,
        model: &str,
        effort: &str,
        started_at: &str,
    ) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO workers (run_id, role, slice, checkpoint, session_id, model, effort, outcome, started_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'running', ?8)",
            params![
                run_id,
                role,
                slice.map(i64::from),
                checkpoint.map(i64::from),
                session_id,
                model,
                effort,
                started_at
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// The row completed from the stream's `result` (FR-2, FR-8). A dead
    /// worker's usage stays null; its `outcome` says why.
    pub fn finish_worker(
        &self,
        id: i64,
        summary: &Summary,
        outcome: &str,
        ended_at: &str,
    ) -> Result<()> {
        let u = summary.usage.as_ref();
        let as_i64 = |v: Option<u64>| v.map(|n| n as i64);
        self.conn.execute(
            "UPDATE workers SET input = ?2, cache_write = ?3, cache_read = ?4, output = ?5,
                                thinking = ?6, cost_usd = ?7, num_turns = ?8, duration_ms = ?9,
                                terminal_reason = ?10, outcome = ?11, peak_context = ?12, ended_at = ?13,
                                start_context = ?14, model_id = ?15
             WHERE id = ?1",
            params![
                id,
                as_i64(u.and_then(|u| u.input_tokens)),
                as_i64(u.and_then(|u| u.cache_creation_input_tokens)),
                as_i64(u.and_then(|u| u.cache_read_input_tokens)),
                as_i64(u.and_then(|u| u.output_tokens)),
                as_i64(u.and_then(|u| u.thinking())),
                summary.cost_usd,
                as_i64(summary.num_turns),
                as_i64(summary.duration_ms),
                summary.terminal_reason,
                outcome,
                as_i64(summary.peak_context),
                ended_at,
                as_i64(summary.start_context),
                summary.model_id,
            ],
        )?;
        Ok(())
    }

    /// The usage half of [`Db::finish_worker`], for `report --recount`
    /// (PRD-003 FR-21): `outcome`, `terminal_reason`, `duration_ms` and
    /// `ended_at` say how the worker ended and stay as they are.
    pub fn update_worker_usage(&self, id: i64, summary: &Summary) -> Result<()> {
        let u = summary.usage.as_ref();
        let as_i64 = |v: Option<u64>| v.map(|n| n as i64);
        self.conn.execute(
            "UPDATE workers SET input = ?2, cache_write = ?3, cache_read = ?4, output = ?5,
                                thinking = ?6, cost_usd = ?7, num_turns = ?8, peak_context = ?9,
                                start_context = ?10
             WHERE id = ?1",
            params![
                id,
                as_i64(u.and_then(|u| u.input_tokens)),
                as_i64(u.and_then(|u| u.cache_creation_input_tokens)),
                as_i64(u.and_then(|u| u.cache_read_input_tokens)),
                as_i64(u.and_then(|u| u.output_tokens)),
                as_i64(u.and_then(|u| u.thinking())),
                summary.cost_usd,
                as_i64(summary.num_turns),
                as_i64(summary.peak_context),
                as_i64(summary.start_context),
            ],
        )?;
        Ok(())
    }

    /// The files the worker's pack holds, as JSON (slice 13): what
    /// `pack_doc_read_chars` counts reads of.
    pub fn set_worker_pack_files(&self, id: i64, files: &[String]) -> Result<()> {
        let text = serde_json::to_string(files)
            .map_err(|e| crate::Error::Msg(format!("pack files as JSON: {e}")))?;
        self.conn.execute(
            "UPDATE workers SET pack_files = ?2 WHERE id = ?1",
            params![id, text],
        )?;
        Ok(())
    }

    /// The pack's sections and their chars, as JSON (PRD-003 FR-22).
    pub fn set_worker_pack_parts(&self, id: i64, parts: &[crate::pack::Part]) -> Result<()> {
        let text = serde_json::to_string(parts)
            .map_err(|e| crate::Error::Msg(format!("pack parts as JSON: {e}")))?;
        self.conn.execute(
            "UPDATE workers SET pack_parts = ?2 WHERE id = ?1",
            params![id, text],
        )?;
        Ok(())
    }

    /// The files the worker's pack held; empty when none was recorded.
    pub fn worker_pack_files(&self, id: i64) -> Result<Vec<String>> {
        let text: Option<String> = self
            .conn
            .query_row(
                "SELECT pack_files FROM workers WHERE id = ?1",
                params![id],
                |r| r.get(0),
            )
            .optional()?
            .flatten();
        Ok(text
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default())
    }

    /// The worker's subagent usage as `(input, cache_write, cache_read,
    /// output, count)`; `None` writes nulls (the dir could not be read).
    pub fn set_worker_subagents(
        &self,
        id: i64,
        sub: Option<(i64, i64, i64, i64, i64)>,
    ) -> Result<()> {
        self.conn.execute(
            "UPDATE workers SET sub_input = ?2, sub_cache_write = ?3, sub_cache_read = ?4,
                                sub_output = ?5, subagents = ?6
             WHERE id = ?1",
            params![
                id,
                sub.map(|s| s.0),
                sub.map(|s| s.1),
                sub.map(|s| s.2),
                sub.map(|s| s.3),
                sub.map(|s| s.4),
            ],
        )?;
        Ok(())
    }

    /// The slice's commit on the worker that built it (FR-5).
    pub fn set_worker_commit(&self, id: i64, sha: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE workers SET commit_sha = ?2 WHERE id = ?1",
            params![id, sha],
        )?;
        Ok(())
    }

    /// A run's workers in spawn order.
    pub fn workers(&self, run_id: &str) -> Result<Vec<WorkerRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, run_id, role, slice, session_id, model, effort, input, cache_write, cache_read,
                    output, thinking, cost_usd, num_turns, duration_ms, terminal_reason, outcome,
                    peak_context, started_at, ended_at, commit_sha, checkpoint, start_context,
                    pack_files, sub_input, sub_cache_write, sub_cache_read, sub_output, subagents, model_id,
                    pack_parts
             FROM workers WHERE run_id = ?1 ORDER BY id",
        )?;
        let rows = stmt.query_map([run_id], |r| {
            Ok(WorkerRow {
                id: r.get(0)?,
                run_id: r.get(1)?,
                role: r.get(2)?,
                slice: r.get::<_, Option<i64>>(3)?.map(|n| n as u32),
                session_id: r.get(4)?,
                model: r.get(5)?,
                effort: r.get(6)?,
                input: r.get(7)?,
                cache_write: r.get(8)?,
                cache_read: r.get(9)?,
                output: r.get(10)?,
                thinking: r.get(11)?,
                cost_usd: r.get(12)?,
                num_turns: r.get(13)?,
                duration_ms: r.get(14)?,
                terminal_reason: r.get(15)?,
                outcome: r.get(16)?,
                peak_context: r.get(17)?,
                started_at: r.get(18)?,
                ended_at: r.get(19)?,
                commit_sha: r.get(20)?,
                checkpoint: r.get::<_, Option<i64>>(21)?.map(|n| n as u32),
                start_context: r.get(22)?,
                pack_files: r
                    .get::<_, Option<String>>(23)?
                    .and_then(|t| serde_json::from_str(&t).ok())
                    .unwrap_or_default(),
                pack_parts: r.get("pack_parts")?,
                sub_input: r.get(24)?,
                sub_cache_write: r.get(25)?,
                sub_cache_read: r.get(26)?,
                sub_output: r.get(27)?,
                subagents: r.get(28)?,
                model_id: r.get("model_id")?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    /// One gate run (or cache hit) as a row. Returns the row id.
    pub fn insert_gate(&self, g: &GateInsert) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO gates (run_id, source, kind, cmd, red, minutes, at, worker_id, cached, checkpoint, attempt)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                g.run_id,
                g.source,
                g.kind,
                g.cmd,
                g.red as i64,
                g.minutes,
                g.at,
                g.worker_id,
                g.cached as i64,
                g.checkpoint.map(i64::from),
                g.attempt.map(i64::from),
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// A run's gate rows in insertion order.
    pub fn gates(&self, run_id: &str) -> Result<Vec<GateRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, run_id, source, kind, cmd, red, minutes, at, worker_id, cached, checkpoint, attempt
             FROM gates WHERE run_id = ?1 ORDER BY id",
        )?;
        let rows = stmt.query_map([run_id], |r| {
            Ok(GateRow {
                id: r.get(0)?,
                run_id: r.get(1)?,
                source: r.get(2)?,
                kind: r.get(3)?,
                cmd: r.get(4)?,
                red: r.get::<_, i64>(5)? != 0,
                minutes: r.get(6)?,
                at: r.get(7)?,
                worker_id: r.get(8)?,
                cached: r.get::<_, i64>(9)? != 0,
                checkpoint: r.get::<_, Option<i64>>(10)?.map(|n| n as u32),
                attempt: r.get::<_, Option<i64>>(11)?.map(|n| n as u32),
            })
        })?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    /// A worker's tool calls as one transaction (FR-8): one `INSERT` per
    /// call, all or none.
    pub fn insert_tool_calls(&self, rows: &[ToolCallInsert]) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO tool_calls (run_id, worker_id, seq, tool, kind, result_chars, target_hash,
                                         repeat_in_worker, repeat_in_run, ts, label_followup,
                                         label_expanded, pack_doc, in_subagent)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            )?;
            for t in rows {
                stmt.execute(params![
                    t.run_id,
                    t.worker_id,
                    i64::from(t.seq),
                    t.tool,
                    t.kind,
                    t.result_chars,
                    t.target_hash,
                    t.repeat_in_worker as i64,
                    t.repeat_in_run as i64,
                    t.ts,
                    t.label_followup.map(i64::from),
                    t.label_expanded.map(i64::from),
                    t.pack_doc as i64,
                    t.in_subagent as i64,
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Every `tool_calls` row of the run, before `report --recount` writes
    /// them again (PRD-003 FR-21).
    pub fn delete_tool_calls(&self, run_id: &str) -> Result<usize> {
        Ok(self
            .conn
            .execute("DELETE FROM tool_calls WHERE run_id = ?1", [run_id])?)
    }

    /// A run's tool calls in insertion order.
    pub fn tool_calls(&self, run_id: &str) -> Result<Vec<ToolCallRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, run_id, worker_id, seq, tool, kind, result_chars, target_hash,
                    repeat_in_worker, repeat_in_run, ts, label_followup, label_expanded, pack_doc,
                    in_subagent
             FROM tool_calls WHERE run_id = ?1 ORDER BY id",
        )?;
        let rows = stmt.query_map([run_id], |r| {
            Ok(ToolCallRow {
                id: r.get(0)?,
                run_id: r.get(1)?,
                worker_id: r.get(2)?,
                seq: r.get::<_, i64>(3)? as u32,
                tool: r.get(4)?,
                kind: r.get(5)?,
                result_chars: r.get(6)?,
                target_hash: r.get(7)?,
                repeat_in_worker: r.get::<_, i64>(8)? != 0,
                repeat_in_run: r.get::<_, i64>(9)? != 0,
                ts: r.get(10)?,
                label_followup: r.get::<_, Option<i64>>(11)?.map(|n| n != 0),
                label_expanded: r.get::<_, Option<i64>>(12)?.map(|n| n != 0),
                pack_doc: r.get::<_, Option<i64>>(13)?.is_some_and(|n| n != 0),
                in_subagent: r.get::<_, i64>(14)? != 0,
            })
        })?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    /// The distinct `target_hash` values a run's earlier workers hit
    /// (`repeat_in_run`, FR-8).
    pub fn run_targets(&self, run_id: &str) -> Result<std::collections::HashSet<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT DISTINCT target_hash FROM tool_calls WHERE run_id = ?1")?;
        let rows = stmt.query_map([run_id], |r| r.get::<_, String>(0))?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    /// The user tables, for a schema check.
    pub fn tables(&self) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
        )?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }
}

/// A `runs` row read by [`Db::run_row`] and [`Db::earlier_runs`]'s shared
/// column list.
fn run_from_row(r: &rusqlite::Row) -> rusqlite::Result<RunRow> {
    Ok(RunRow {
        id: r.get(0)?,
        repo: r.get(1)?,
        feature: r.get(2)?,
        branch: r.get(3)?,
        started_at: r.get(4)?,
        finished_at: r.get(5)?,
        phase: r.get(6)?,
        budget_weighted_tokens: r.get::<_, i64>(7)? as u64,
        pr_url: r.get(8)?,
        knobs: r.get("knobs")?,
        hypothesis: r.get("hypothesis")?,
        findings: match (r.get(9)?, r.get(10)?, r.get(11)?, r.get(12)?, r.get(13)?) {
            (Some(high), Some(medium), Some(low), Some(fixed), Some(parked)) => Some(Findings {
                high,
                medium,
                low,
                fixed,
                parked,
            }),
            _ => None,
        },
        decisions: r.get(14)?,
        decide: r.get(15)?,
        blocks: r.get(16)?,
        lines_added: r
            .get::<_, Option<i64>>("lines_added")?
            .map(|n| n.max(0) as u64),
        lines_removed: r
            .get::<_, Option<i64>>("lines_removed")?
            .map(|n| n.max(0) as u64),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_opens_that_race_all_migrate() {
        for _ in 0..20 {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("factory.db");
            let gate = std::sync::Arc::new(std::sync::Barrier::new(6));
            let opens: Vec<_> = (0..6)
                .map(|_| {
                    let (path, gate) = (path.clone(), gate.clone());
                    std::thread::spawn(move || {
                        gate.wait();
                        Db::open(&path).map(|_| ()).map_err(|e| e.to_string())
                    })
                })
                .collect();
            for open in opens {
                open.join().unwrap().unwrap();
            }
            let db = Db::open(&path).unwrap();
            assert!(db.has_column("tool_calls", "pack_doc").unwrap());
            assert!(db.has_column("workers", "subagents").unwrap());
            assert!(db.has_column("tool_calls", "in_subagent").unwrap());
        }
    }

    #[test]
    fn a_version_1_store_migrates_in_place_and_keeps_nulls() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("factory.db");
        {
            // The v1 store: the base schema plus the v1 guarded columns it lacks.
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(SCHEMA).unwrap();
            conn.execute_batch(
                "ALTER TABLE workers ADD COLUMN start_context INTEGER;
                 ALTER TABLE workers ADD COLUMN pack_files TEXT;
                 ALTER TABLE runs ADD COLUMN pr_url TEXT;
                 INSERT INTO schema_version (version, applied_at) VALUES (1, 'then');
                 INSERT INTO runs (id, repo, feature, started_at, phase, budget_weighted_tokens)
                   VALUES ('r1', '/repo', 'f', 'then', 'done', 0);
                 INSERT INTO workers (run_id, role, started_at, output)
                   VALUES ('r1', 'build', 'then', 7);",
            )
            .unwrap();
        }
        let db = Db::open(&path).unwrap();
        assert_eq!(db.schema_version().unwrap(), 2);
        let w = &db.workers("r1").unwrap()[0];
        assert_eq!(w.output, Some(7));
        assert_eq!(
            (
                w.sub_input,
                w.sub_cache_write,
                w.sub_cache_read,
                w.sub_output,
                w.subagents
            ),
            (None, None, None, None, None)
        );
    }
}
