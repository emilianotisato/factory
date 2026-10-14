use std::path::Path;
use std::time::Duration;

use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};
use serde::Serialize;

use crate::run::RunState;
use crate::stream::{Findings, Summary};
use crate::{Result, clock};

pub const SCHEMA_VERSION: i64 = 3;

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
CREATE TABLE IF NOT EXISTS retros (
  repo              TEXT    NOT NULL,
  feature           INTEGER NOT NULL,
  slug              TEXT    NOT NULL,
  shipped_at        TEXT,
  run_id            TEXT    REFERENCES runs(id),
  score             INTEGER,
  gate_runs         INTEGER,
  gate_red          INTEGER,
  gate_minutes      REAL,
  ci_cycles         INTEGER,
  slices            INTEGER,
  checkpoints       INTEGER,
  build_minutes     REAL,
  polish_minutes    REAL,
  out_tokens        INTEGER,
  cache_read_tokens INTEGER,
  start_context     INTEGER,
  peak_context      INTEGER,
  written_at        TEXT    NOT NULL,
  PRIMARY KEY (repo, feature)
);
";

/// SQLite's `ALTER TABLE ADD COLUMN` has no `IF NOT EXISTS`; each is guarded by the live column list.
const COLUMNS: [(&str, &str, &str); 32] = [
    ("workers", "commit_sha", "TEXT"),
    ("workers", "checkpoint", "INTEGER"),
    ("gates", "cached", "INTEGER NOT NULL DEFAULT 0"),
    ("gates", "checkpoint", "INTEGER"),
    ("gates", "attempt", "INTEGER"),
    ("workers", "start_context", "INTEGER"),
    ("runs", "pr_url", "TEXT"),
    ("workers", "pack_files", "TEXT"),
    ("tool_calls", "pack_doc", "INTEGER"),
    ("workers", "sub_input", "INTEGER"),
    ("workers", "sub_cache_write", "INTEGER"),
    ("workers", "sub_cache_read", "INTEGER"),
    ("workers", "sub_output", "INTEGER"),
    ("workers", "subagents", "INTEGER"),
    ("tool_calls", "in_subagent", "INTEGER NOT NULL DEFAULT 0"),
    ("workers", "pack_parts", "TEXT"),
    ("runs", "knobs", "TEXT"),
    ("runs", "hypothesis", "TEXT"),
    ("workers", "model_id", "TEXT"),
    ("runs", "findings_high", "INTEGER"),
    ("runs", "findings_medium", "INTEGER"),
    ("runs", "findings_low", "INTEGER"),
    ("runs", "findings_fixed", "INTEGER"),
    ("runs", "findings_parked", "INTEGER"),
    ("runs", "decisions", "INTEGER"),
    ("runs", "decide", "INTEGER"),
    ("runs", "blocks", "INTEGER"),
    ("runs", "lines_added", "INTEGER"),
    ("runs", "lines_removed", "INTEGER"),
    ("runs", "comment_lines", "INTEGER"),
    ("runs", "nonblank_lines", "INTEGER"),
    ("runs", "id_hits", "INTEGER"),
];

pub const RETRO_COLUMNS: [&str; 19] = [
    "repo",
    "feature",
    "slug",
    "shipped_at",
    "run_id",
    "score",
    "gate_runs",
    "gate_red",
    "gate_minutes",
    "ci_cycles",
    "slices",
    "checkpoints",
    "build_minutes",
    "polish_minutes",
    "out_tokens",
    "cache_read_tokens",
    "start_context",
    "peak_context",
    "written_at",
];

pub struct Db {
    conn: Connection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventRow {
    pub seq: u64,
    pub ts: String,
    pub kind: String,
    pub payload: String,
}

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
    pub pr_url: Option<String>,
    /// JSON, from `config::knobs`.
    pub knobs: Option<String>,
    pub hypothesis: Option<String>,
    pub findings: Option<Findings>,
    /// Agent-log counts at ship: `AD` entries, `decide` ones, blocks.
    pub decisions: Option<u32>,
    pub decide: Option<u32>,
    pub blocks: Option<u32>,
    /// Changed lines against the base at ship; `None` when `git diff` failed.
    pub lines_added: Option<u64>,
    pub lines_removed: Option<u64>,
    /// The leaks scan against the base at ship; `None` when it failed.
    pub comment_lines: Option<u64>,
    pub nonblank_lines: Option<u64>,
    pub id_hits: Option<u64>,
}

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
    pub commit_sha: Option<String>,
    pub checkpoint: Option<u32>,
    pub start_context: Option<i64>,
    /// Repo-relative; empty when none was recorded.
    pub pack_files: Vec<String>,
    /// JSON of [`crate::pack::Part`]s.
    pub pack_parts: Option<String>,
    /// Null when the transcript dir was not read; zero when it had none.
    pub sub_input: Option<i64>,
    pub sub_cache_write: Option<i64>,
    pub sub_cache_read: Option<i64>,
    pub sub_output: Option<i64>,
    pub subagents: Option<i64>,
    /// What answered, where `model` is what was asked for.
    pub model_id: Option<String>,
}

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
    pub label_followup: Option<bool>,
    pub label_expanded: Option<bool>,
    pub pack_doc: bool,
    pub in_subagent: bool,
}

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
    pub pack_doc: bool,
    pub in_subagent: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GateInsert<'a> {
    pub run_id: &'a str,
    /// `driver` for the checkpoint gate, `worker` for a worker's inner run.
    pub source: &'a str,
    /// `checkpoint` for the driver's, `slice` for a worker's.
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

/// `repo` is the canonical root, the string `runs.repo` holds.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RetroRow {
    pub repo: String,
    pub feature: u32,
    pub slug: String,
    pub shipped_at: Option<String>,
    pub run_id: Option<String>,
    pub score: Option<i64>,
    pub gate_runs: Option<i64>,
    pub gate_red: Option<i64>,
    pub gate_minutes: Option<f64>,
    pub ci_cycles: Option<i64>,
    pub slices: Option<i64>,
    pub checkpoints: Option<i64>,
    pub build_minutes: Option<f64>,
    pub polish_minutes: Option<f64>,
    pub out_tokens: Option<i64>,
    pub cache_read_tokens: Option<i64>,
    pub start_context: Option<i64>,
    pub peak_context: Option<i64>,
    /// Overwritten on every write; the caller's value is ignored.
    pub written_at: String,
}

impl Db {
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

    /// Immediate: two first opens must not both see a column missing and the second fail on a duplicate.
    fn migrate(&self) -> Result<()> {
        let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Immediate)?;
        tx.execute_batch(SCHEMA)?;
        for (table, column, decl) in COLUMNS {
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

    pub fn schema_version(&self) -> Result<i64> {
        Ok(self
            .conn
            .query_row("SELECT max(version) FROM schema_version", [], |r| r.get(0))?)
    }

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
                        decisions, decide, blocks, knobs, hypothesis, lines_added, lines_removed,
                        comment_lines, nonblank_lines, id_hits
                 FROM runs WHERE id = ?1",
                [id],
                run_from_row,
            )
            .optional()?)
    }

    /// Bounded by `before` so a rerun sees the same cohort; inclusive since the store keeps whole seconds.
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
                    decisions, decide, blocks, knobs, hypothesis, lines_added, lines_removed,
                    comment_lines, nonblank_lines, id_hits
             FROM runs
             WHERE repo = ?1 AND phase = 'finished' AND finished_at IS NOT NULL AND finished_at <= ?2
               AND id != ?3
               AND NOT EXISTS (SELECT 1 FROM events e WHERE e.run_id = runs.id AND e.kind = 'run_stopped')
             ORDER BY finished_at DESC, id DESC LIMIT ?4",
        )?;
        let rows = stmt.query_map(params![repo, before, except, limit], run_from_row)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn set_run_lines(&self, run_id: &str, lines: Option<(u64, u64)>) -> Result<()> {
        self.conn.execute(
            "UPDATE runs SET lines_added = ?2, lines_removed = ?3 WHERE id = ?1",
            params![run_id, lines.map(|l| l.0 as i64), lines.map(|l| l.1 as i64)],
        )?;
        Ok(())
    }

    /// `(comment lines, non-blank added lines, id hits)`.
    pub fn set_run_leaks(&self, run_id: &str, leaks: Option<(u64, u64, u64)>) -> Result<()> {
        self.conn.execute(
            "UPDATE runs SET comment_lines = ?2, nonblank_lines = ?3, id_hits = ?4 WHERE id = ?1",
            params![
                run_id,
                leaks.map(|l| l.0 as i64),
                leaks.map(|l| l.1 as i64),
                leaks.map(|l| l.2 as i64)
            ],
        )?;
        Ok(())
    }

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

    pub fn set_run_knobs(&self, run_id: &str, knobs: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE runs SET knobs = ?2 WHERE id = ?1",
            params![run_id, knobs],
        )?;
        Ok(())
    }

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

    /// A build worker has a `slice`; a fix or checkpoint worker a `checkpoint`.
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

    pub fn update_worker_usage(&self, id: i64, summary: &Summary) -> Result<()> {
        let u = summary.usage.as_ref();
        let as_i64 = |v: Option<u64>| v.map(|n| n as i64);
        self.conn.execute(
            "UPDATE workers SET input = ?2, cache_write = ?3, cache_read = ?4, output = ?5,
                                thinking = ?6, cost_usd = ?7, num_turns = ?8, peak_context = ?9,
                                start_context = ?10, model_id = COALESCE(?11, model_id)
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
                summary.model_id,
            ],
        )?;
        Ok(())
    }

    pub fn set_worker_pack_files(&self, id: i64, files: &[String]) -> Result<()> {
        let text = serde_json::to_string(files)
            .map_err(|e| crate::Error::Msg(format!("pack files as JSON: {e}")))?;
        self.conn.execute(
            "UPDATE workers SET pack_files = ?2 WHERE id = ?1",
            params![id, text],
        )?;
        Ok(())
    }

    pub fn set_worker_pack_parts(&self, id: i64, parts: &[crate::pack::Part]) -> Result<()> {
        let text = serde_json::to_string(parts)
            .map_err(|e| crate::Error::Msg(format!("pack parts as JSON: {e}")))?;
        self.conn.execute(
            "UPDATE workers SET pack_parts = ?2 WHERE id = ?1",
            params![id, text],
        )?;
        Ok(())
    }

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

    pub fn set_worker_commit(&self, id: i64, sha: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE workers SET commit_sha = ?2 WHERE id = ?1",
            params![id, sha],
        )?;
        Ok(())
    }

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

    pub fn delete_tool_calls(&self, run_id: &str) -> Result<usize> {
        Ok(self
            .conn
            .execute("DELETE FROM tool_calls WHERE run_id = ?1", [run_id])?)
    }

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

    pub fn run_targets(&self, run_id: &str) -> Result<std::collections::HashSet<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT DISTINCT target_hash FROM tool_calls WHERE run_id = ?1")?;
        let rows = stmt.query_map([run_id], |r| r.get::<_, String>(0))?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    /// Not `INSERT OR REPLACE`: that deletes the row first and would erase a score set before the numbers.
    pub fn upsert_retro(&self, row: &RetroRow) -> Result<()> {
        let updates: Vec<String> = RETRO_COLUMNS[2..]
            .iter()
            .map(|c| match *c {
                "score" => "score = COALESCE(excluded.score, retros.score)".to_string(),
                c => format!("{c} = excluded.{c}"),
            })
            .collect();
        let slots: Vec<String> = (1..=RETRO_COLUMNS.len()).map(|i| format!("?{i}")).collect();
        self.conn.execute(
            &format!(
                "INSERT INTO retros ({}) VALUES ({}) ON CONFLICT(repo, feature) DO UPDATE SET {}",
                RETRO_COLUMNS.join(", "),
                slots.join(", "),
                updates.join(", ")
            ),
            params![
                row.repo,
                row.feature,
                row.slug,
                row.shipped_at,
                row.run_id,
                row.score,
                row.gate_runs,
                row.gate_red,
                row.gate_minutes,
                row.ci_cycles,
                row.slices,
                row.checkpoints,
                row.build_minutes,
                row.polish_minutes,
                row.out_tokens,
                row.cache_read_tokens,
                row.start_context,
                row.peak_context,
                clock::now_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn retro(&self, repo: &str, feature: u32) -> Result<Option<RetroRow>> {
        let sql = format!(
            "SELECT {} FROM retros WHERE repo = ?1 AND feature = ?2",
            RETRO_COLUMNS.join(", ")
        );
        Ok(self
            .conn
            .query_row(&sql, params![repo, feature], |r| {
                Ok(RetroRow {
                    repo: r.get(0)?,
                    feature: r.get(1)?,
                    slug: r.get(2)?,
                    shipped_at: r.get(3)?,
                    run_id: r.get(4)?,
                    score: r.get(5)?,
                    gate_runs: r.get(6)?,
                    gate_red: r.get(7)?,
                    gate_minutes: r.get(8)?,
                    ci_cycles: r.get(9)?,
                    slices: r.get(10)?,
                    checkpoints: r.get(11)?,
                    build_minutes: r.get(12)?,
                    polish_minutes: r.get(13)?,
                    out_tokens: r.get(14)?,
                    cache_read_tokens: r.get(15)?,
                    start_context: r.get(16)?,
                    peak_context: r.get(17)?,
                    written_at: r.get(18)?,
                })
            })
            .optional()?)
    }

    pub fn set_retro_score(&self, repo: &str, feature: u32, slug: &str, score: i64) -> Result<()> {
        self.conn.execute(
            "INSERT INTO retros (repo, feature, slug, score, written_at) VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(repo, feature) DO UPDATE SET score = excluded.score, written_at = excluded.written_at",
            params![repo, feature, slug, score, clock::now_rfc3339()],
        )?;
        Ok(())
    }

    pub fn tables(&self) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
        )?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }
}

/// Positional: both callers' `SELECT` lists share this column order.
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
        lines_added: count(r, "lines_added")?,
        lines_removed: count(r, "lines_removed")?,
        comment_lines: count(r, "comment_lines")?,
        nonblank_lines: count(r, "nonblank_lines")?,
        id_hits: count(r, "id_hits")?,
    })
}

fn count(r: &rusqlite::Row, column: &str) -> rusqlite::Result<Option<u64>> {
    Ok(r.get::<_, Option<i64>>(column)?.map(|n| n.max(0) as u64))
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
            // A version-1 store: today's base tables plus the columns version 1 added by `ALTER`.
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
        assert_eq!(db.schema_version().unwrap(), SCHEMA_VERSION);
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

    fn schema_v2() -> &'static str {
        SCHEMA
            .split("CREATE TABLE IF NOT EXISTS retros")
            .next()
            .unwrap()
    }

    fn runs_row(conn: &Connection) -> Vec<rusqlite::types::Value> {
        let n = conn.prepare("SELECT * FROM runs").unwrap().column_count();
        conn.query_row("SELECT * FROM runs WHERE id = 'r2'", [], |r| {
            (0..n).map(|i| r.get(i)).collect()
        })
        .unwrap()
    }

    #[test]
    fn a_version_2_store_gains_retros_and_keeps_its_runs() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("factory.db");
        let before = {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(schema_v2()).unwrap();
            for (table, column, decl) in COLUMNS {
                let has: i64 = conn
                    .query_row(
                        "SELECT count(*) FROM pragma_table_info(?1) WHERE name = ?2",
                        [table, column],
                        |r| r.get(0),
                    )
                    .unwrap();
                if has == 0 {
                    conn.execute_batch(&format!("ALTER TABLE {table} ADD COLUMN {column} {decl}"))
                        .unwrap();
                }
            }
            conn.execute_batch(
                "INSERT INTO schema_version (version, applied_at) VALUES (1, 'then'), (2, 'later');
                 INSERT INTO runs (id, repo, feature, branch, started_at, finished_at, phase,
                                   budget_weighted_tokens, knobs, findings_high, lines_added)
                   VALUES ('r2', '/repo', '001', 'feat/x', 'then', 'later', 'done', 9, '{}', 1, 40);",
            )
            .unwrap();
            runs_row(&conn)
        };
        let db = Db::open(&path).unwrap();
        assert_eq!(db.schema_version().unwrap(), 3);
        assert!(db.tables().unwrap().contains(&"retros".to_string()));
        assert_eq!(runs_row(db.conn()), before);
        drop(db);
        let db = Db::open(&path).unwrap();
        let versions: i64 = db
            .conn()
            .query_row("SELECT count(*) FROM schema_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(versions, 3);
    }

    #[test]
    fn migrate_body_runs_on_a_version_3_store() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("factory.db");
        drop(Db::open(&path).unwrap());
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(schema_v2()).unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO schema_version (version, applied_at) VALUES (2, 'now')",
            [],
        )
        .unwrap();
    }

    fn retro_row(score: Option<i64>, numbers: bool) -> RetroRow {
        let n = |v: i64| numbers.then_some(v);
        let m = |v: f64| numbers.then_some(v);
        RetroRow {
            repo: "/repo".into(),
            feature: 1,
            slug: "fixture".into(),
            shipped_at: Some("2026-10-13T10:00:00Z".into()),
            run_id: Some("r1".into()),
            score,
            gate_runs: n(3),
            gate_red: n(1),
            gate_minutes: m(2.23),
            ci_cycles: n(1),
            slices: n(3),
            checkpoints: n(1),
            build_minutes: m(16.0),
            polish_minutes: m(4.5),
            out_tokens: n(1200),
            cache_read_tokens: n(90000),
            start_context: n(30000),
            peak_context: n(80000),
            written_at: String::new(),
        }
    }

    /// A store with run `r1`, which `retro_row` names: `run_id` is a foreign key.
    fn store(dir: &tempfile::TempDir) -> Db {
        let db = Db::open(&dir.path().join("factory.db")).unwrap();
        db.conn()
            .execute(
                "INSERT INTO runs (id, repo, feature, started_at, phase, budget_weighted_tokens)
                   VALUES ('r1', '/repo', '001', 'then', 'done', 0)",
                [],
            )
            .unwrap();
        db
    }

    fn retros(db: &Db) -> i64 {
        db.conn()
            .query_row("SELECT count(*) FROM retros", [], |r| r.get(0))
            .unwrap()
    }

    #[test]
    fn upserting_the_same_retro_twice_leaves_one_equal_row() {
        let dir = tempfile::tempdir().unwrap();
        let db = store(&dir);
        let row = retro_row(Some(4), true);
        db.upsert_retro(&row).unwrap();
        let first = db.retro("/repo", 1).unwrap().unwrap();
        db.upsert_retro(&row).unwrap();
        let second = db.retro("/repo", 1).unwrap().unwrap();
        assert_eq!(retros(&db), 1);
        assert!(!second.written_at.is_empty());
        let blank = |r: RetroRow| RetroRow {
            written_at: String::new(),
            ..r
        };
        assert_eq!(blank(first), row);
        assert_eq!(blank(second), row);
        assert_eq!(db.retro("/repo", 2).unwrap(), None);
    }

    #[test]
    fn a_score_set_first_survives_the_numbers_written_after() {
        let dir = tempfile::tempdir().unwrap();
        let db = store(&dir);
        db.set_retro_score("/repo", 1, "fixture", 4).unwrap();
        let only = db.retro("/repo", 1).unwrap().unwrap();
        assert_eq!(only.score, Some(4));
        assert_eq!(
            RetroRow {
                written_at: String::new(),
                ..only
            },
            RetroRow {
                shipped_at: None,
                run_id: None,
                ..retro_row(Some(4), false)
            }
        );

        db.upsert_retro(&retro_row(None, true)).unwrap();
        let r = db.retro("/repo", 1).unwrap().unwrap();
        assert_eq!(r.score, Some(4));
        assert_eq!((r.gate_runs, r.peak_context), (Some(3), Some(80000)));
        assert_eq!(r.gate_minutes, Some(2.23));
        assert_eq!(r.build_minutes, Some(16.0));

        db.upsert_retro(&retro_row(Some(3), true)).unwrap();
        assert_eq!(db.retro("/repo", 1).unwrap().unwrap().score, Some(3));

        db.set_retro_score("/repo", 1, "fixture", 5).unwrap();
        let r = db.retro("/repo", 1).unwrap().unwrap();
        assert_eq!((r.score, r.gate_runs), (Some(5), Some(3)));
        assert_eq!(retros(&db), 1);
    }

    #[test]
    fn the_leaks_figures_read_back_from_both_selects() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(&dir.path().join("factory.db")).unwrap();
        for id in ["a", "b", "c"] {
            db.conn()
                .execute(
                    "INSERT INTO runs (id, repo, feature, started_at, finished_at, phase, budget_weighted_tokens)
                       VALUES (?1, '/repo', '001', 'then', 'then', 'finished', 0)",
                    [id],
                )
                .unwrap();
        }
        db.set_run_leaks("a", Some((2, 3, 1))).unwrap();
        db.set_run_leaks("b", None).unwrap();
        let figures = |r: &RunRow| (r.comment_lines, r.nonblank_lines, r.id_hits);

        let a = db.run_row("a").unwrap().unwrap();
        assert_eq!(figures(&a), (Some(2), Some(3), Some(1)));
        let b = db.run_row("b").unwrap().unwrap();
        assert_eq!(figures(&b), (None, None, None));

        let earlier = db.earlier_runs("/repo", "then", "c", 3).unwrap();
        let read: Vec<_> = earlier
            .iter()
            .map(|r| (r.id.as_str(), figures(r)))
            .collect();
        assert_eq!(
            read,
            [
                ("b", (None, None, None)),
                ("a", (Some(2), Some(3), Some(1)))
            ]
        );
    }
}
