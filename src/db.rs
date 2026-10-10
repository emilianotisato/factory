//! The SQLite store at `<data_dir>/factory.db` (PRD §10 FR-8): `runs`,
//! `workers`, `gates`, `events`, written as events happen. Migrations are
//! idempotent: `CREATE TABLE IF NOT EXISTS` plus a `schema_version` row.
//! `tool_calls` is slice 08's.
use std::path::Path;
use std::time::Duration;

use rusqlite::{Connection, OptionalExtension, params};

use crate::run::RunState;
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
  ended_at        TEXT
);
CREATE TABLE IF NOT EXISTS gates (
  id        INTEGER PRIMARY KEY AUTOINCREMENT,
  run_id    TEXT NOT NULL REFERENCES runs(id),
  source    TEXT NOT NULL,
  kind      TEXT NOT NULL,
  cmd       TEXT NOT NULL,
  red       INTEGER NOT NULL,
  minutes   REAL NOT NULL,
  at        TEXT NOT NULL,
  worker_id INTEGER
);
CREATE TABLE IF NOT EXISTS events (
  id      INTEGER PRIMARY KEY AUTOINCREMENT,
  run_id  TEXT NOT NULL REFERENCES runs(id),
  seq     INTEGER NOT NULL,
  ts      TEXT NOT NULL,
  kind    TEXT NOT NULL,
  payload TEXT NOT NULL
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
        self.conn.execute(
            "INSERT OR IGNORE INTO schema_version (version, applied_at) VALUES (?1, ?2)",
            params![SCHEMA_VERSION, clock::now_rfc3339()],
        )?;
        Ok(())
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
                             finished_at = COALESCE(finished_at, ?5)
             WHERE id = ?1",
            params![
                s.id.as_str(),
                s.branch,
                s.phase.as_str(),
                s.budget_weighted_tokens as i64,
                finished_at,
            ],
        )?;
        Ok(())
    }

    pub fn run_row(&self, id: &str) -> Result<Option<RunRow>> {
        Ok(self
            .conn
            .query_row(
                "SELECT id, repo, feature, branch, started_at, finished_at, phase, budget_weighted_tokens
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

    /// The user tables, for a schema check.
    pub fn tables(&self) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
        )?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }
}
