//! The SQLite store at `<data_dir>/factory.db` (PRD §10 FR-8): `runs`,
//! `workers`, `gates`, `events`, written as events happen. Migrations are
//! idempotent: `CREATE TABLE IF NOT EXISTS` plus a `schema_version` row, and
//! a guarded `ALTER TABLE` per column added after a table shipped.
//! `tool_calls` landed in slice 08.
use std::path::Path;
use std::time::Duration;

use rusqlite::{Connection, OptionalExtension, params};

use crate::run::RunState;
use crate::stream::Summary;
use crate::{Result, clock};

pub const SCHEMA_VERSION: i64 = 1;

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
}

/// One `workers` row.
#[derive(Debug, Clone, PartialEq)]
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

    fn migrate(&self) -> Result<()> {
        self.conn.execute_batch(SCHEMA)?;
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
        ] {
            if !self.has_column(table, column)? {
                self.conn.execute(
                    &format!("ALTER TABLE {table} ADD COLUMN {column} {decl}"),
                    [],
                )?;
            }
        }
        self.conn.execute(
            "INSERT OR IGNORE INTO schema_version (version, applied_at) VALUES (?1, ?2)",
            params![SCHEMA_VERSION, clock::now_rfc3339()],
        )?;
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
                "SELECT id, repo, feature, branch, started_at, finished_at, phase, budget_weighted_tokens, pr_url
                 FROM runs WHERE id = ?1",
                [id],
                |r| {
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
                    })
                },
            )
            .optional()?)
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
                                start_context = ?14
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
                    pack_files
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
                                         label_expanded, pack_doc)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
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
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// A run's tool calls in insertion order.
    pub fn tool_calls(&self, run_id: &str) -> Result<Vec<ToolCallRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, run_id, worker_id, seq, tool, kind, result_chars, target_hash,
                    repeat_in_worker, repeat_in_run, ts, label_followup, label_expanded, pack_doc
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
