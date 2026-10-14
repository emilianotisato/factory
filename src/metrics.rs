//! FR-8 (PRD §10): every tool call of every worker as a `tool_calls` row,
//! and the worker's own inner-loop gate runs as `gates` rows with source
//! `worker`. Both come from the stream slice 03 already pairs
//! (`stream::Summary::tool_uses`) and are written once, when the worker's
//! row is closed, inside one transaction. The report (`cli::report`) is a
//! `SELECT` over what this module wrote, never a re-parse.
//!
//! Kinds follow §10 "by first command": `Read` → `read`; `Edit`, `Write`,
//! `NotebookEdit` → `edit`; `Grep`, `Glob` → `grep`; `Bash` by the first
//! word of `command` after `cd … &&` and `VAR=value` prefixes: `grep`, `rg` →
//! `bash-grep`, `sed` → `bash-sed`, `cat`, `head`, `tail` → `bash-cat`, a
//! command containing the feature's gate or inner command → `run-gate`,
//! else `run`. Any other tool is `other`.
//!
//! Slice 13 (§14 FR-12 decision 4) adds `ctx-def`, `ctx-refs` and
//! `ctx-outline` for `factory ctx <verb>`, ahead of the rest; the two labels
//! (`label_followup`: a file the ctx answer named is read within the next two
//! calls; `label_expanded`: the same target read with a wider `limit`, or
//! `refs` re-run with a larger `--max`, within five); and `pack_doc`, a read
//! of a file the worker's pack held (`pack_doc_read_chars`, §4).
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::Result;
use crate::clock;
use crate::db::{Db, GateInsert, ToolCallInsert};
use crate::events::EventKind;
use crate::measure::{self, Subagents, epoch_secs};
use crate::paths;
use crate::run::{GateCmds, Run};
use crate::shell;
use crate::stream::{Summary, ToolUse};

/// What a tool call did, for the `kind` column and the report's lookup set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Read,
    Edit,
    Grep,
    BashGrep,
    BashSed,
    BashCat,
    CtxDef,
    CtxRefs,
    CtxOutline,
    RunGate,
    Run,
    Other,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Read => "read",
            Kind::Edit => "edit",
            Kind::Grep => "grep",
            Kind::BashGrep => "bash-grep",
            Kind::BashSed => "bash-sed",
            Kind::BashCat => "bash-cat",
            Kind::CtxDef => "ctx-def",
            Kind::CtxRefs => "ctx-refs",
            Kind::CtxOutline => "ctx-outline",
            Kind::RunGate => "run-gate",
            Kind::Run => "run",
            Kind::Other => "other",
        }
    }

    /// A lookup: context the worker pulled in to read (`lookup_chars`,
    /// `ctx_share`, `repeat_rate`).
    pub fn is_lookup(self) -> bool {
        matches!(
            self,
            Kind::Read
                | Kind::Grep
                | Kind::BashGrep
                | Kind::BashSed
                | Kind::BashCat
                | Kind::CtxDef
                | Kind::CtxRefs
                | Kind::CtxOutline
        )
    }

    pub fn is_ctx(self) -> bool {
        matches!(self, Kind::CtxDef | Kind::CtxRefs | Kind::CtxOutline)
    }
}

/// The lookup kinds as the report's SQL names them.
pub const LOOKUP_KINDS: [&str; 8] = [
    "read",
    "grep",
    "bash-grep",
    "bash-sed",
    "bash-cat",
    "ctx-def",
    "ctx-refs",
    "ctx-outline",
];

/// The `ctx-*` kinds (slice 13).
pub const CTX_KINDS: [&str; 3] = ["ctx-def", "ctx-refs", "ctx-outline"];

/// `factory ctx def|refs|outline …` as the first command, by its verb.
pub fn ctx_kind(command: &str) -> Option<Kind> {
    let (words, _) = shell::first_command(command);
    if words.len() < 3 || shell::program(&words[0]) != "factory" || words[1] != "ctx" {
        return None;
    }
    match words[2].as_str() {
        "def" => Some(Kind::CtxDef),
        "refs" => Some(Kind::CtxRefs),
        "outline" => Some(Kind::CtxOutline),
        _ => None,
    }
}

/// The kind of one call. `gate` is the feature's gate pair when the driver
/// knows it; without it no `Bash` call is a gate run.
pub fn classify(t: &ToolUse, gate: Option<&GateCmds>) -> Kind {
    match t.name.as_str() {
        "Read" => Kind::Read,
        "Edit" | "Write" | "NotebookEdit" => Kind::Edit,
        "Grep" | "Glob" => Kind::Grep,
        "Bash" => bash_kind(command(t), gate),
        _ => Kind::Other,
    }
}

/// A `Bash` call's kind from its command, in §10's order: the lookup words
/// first, then the gate strings, else `run`.
pub fn bash_kind(command: &str, gate: Option<&GateCmds>) -> Kind {
    if let Some(k) = ctx_kind(command) {
        return k;
    }
    match first_word(command).as_deref() {
        Some("grep" | "rg" | "egrep" | "fgrep") => return Kind::BashGrep,
        Some("sed") => return Kind::BashSed,
        Some("cat" | "head" | "tail") => return Kind::BashCat,
        _ => {}
    }
    if let Some(g) = gate {
        let collapsed = collapse(command);
        let hits = |s: &str| !s.is_empty() && collapsed.contains(&collapse(s));
        if hits(&g.gate) || g.inner.as_deref().is_some_and(hits) {
            return Kind::RunGate;
        }
    }
    Kind::Run
}

/// The first command word after leading `cd … &&` (or `;`) segments, `env`,
/// `time` and `VAR=value` prefixes, with any directory prefix dropped: the
/// shell reading `ctx_kind` and the hook use, so a quoted prefix such as
/// `FOO="a b" grep …` still finds `grep` (AD-40006).
pub fn first_word(command: &str) -> Option<String> {
    let (words, _) = shell::first_command(command);
    words.first().map(|w| shell::program(w).to_string())
}

fn collapse(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn command(t: &ToolUse) -> &str {
    t.input.get("command").and_then(Value::as_str).unwrap_or("")
}

fn field<'a>(t: &'a ToolUse, key: &str) -> &'a str {
    t.input.get(key).and_then(Value::as_str).unwrap_or("")
}

/// The normalised target (§10 `target_hash`): the file path, the pattern
/// plus path, the command with whitespace collapsed; for other tools the
/// tool name plus its input.
pub fn target(t: &ToolUse) -> String {
    match t.name.as_str() {
        "Read" | "Edit" | "Write" => field(t, "file_path").to_string(),
        "NotebookEdit" => field(t, "notebook_path").to_string(),
        "Grep" | "Glob" => format!("{}\n{}", field(t, "pattern"), field(t, "path")),
        "Bash" => collapse(command(t)),
        name => format!("{name}\n{}", t.input),
    }
}

/// sha256 of [`target`], lowercase hex.
pub fn target_hash(t: &ToolUse) -> String {
    sha256_hex(target(t).as_bytes())
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(64);
    for b in digest.iter() {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

/// A gate run's verdict from its result: the block's `is_error`, a line
/// starting with `FAIL` (the fixture gate), or an `Exit code <n>` marker
/// with n ≠ 0 (a real gate under the agent's `Bash`).
pub fn is_red(t: &ToolUse) -> bool {
    if t.result_error {
        return true;
    }
    let Some(text) = t.result_head.as_deref() else {
        return false;
    };
    text.lines().any(|l| {
        let l = l.trim_start();
        l.starts_with("FAIL")
            || l.strip_prefix("Exit code ")
                .and_then(|rest| rest.split_whitespace().next())
                .and_then(|n| n.parse::<i64>().ok())
                .is_some_and(|n| n != 0)
    })
}

/// Minutes a call took: from its timestamp to its result's, else to the
/// next call's, else 0 (the column is `NOT NULL`; AD-803).
pub fn minutes(t: &ToolUse, next: Option<&ToolUse>) -> f64 {
    let Some(start) = t.ts.as_deref().and_then(epoch_secs) else {
        return 0.0;
    };
    let end = t
        .result_ts
        .as_deref()
        .or_else(|| next.and_then(|n| n.ts.as_deref()))
        .and_then(epoch_secs);
    match end {
        Some(e) if e >= start => (e - start) / 60.0,
        _ => 0.0,
    }
}

/// One worker's calls as rows, flags computed against `seen_run` (the
/// run's earlier targets, which this call extends).
#[derive(Debug, Clone, PartialEq)]
pub struct CallRow {
    pub seq: u32,
    pub tool: String,
    pub kind: Kind,
    pub result_chars: Option<i64>,
    pub target_hash: String,
    pub repeat_in_worker: bool,
    pub repeat_in_run: bool,
    pub ts: Option<String>,
    pub label_followup: Option<bool>,
    pub label_expanded: Option<bool>,
    /// A subagent's call: kept in `seq` and the repeat sets, so the windows
    /// above count it; only the report's lookup figures leave it out.
    pub in_subagent: bool,
}

pub fn call_rows(
    tool_uses: &[ToolUse],
    gate: Option<&GateCmds>,
    seen_run: &mut HashSet<String>,
) -> Vec<CallRow> {
    let mut seen_worker: HashSet<String> = HashSet::new();
    let kinds: Vec<Kind> = tool_uses.iter().map(|t| classify(t, gate)).collect();
    let labels = labels(tool_uses, &kinds);
    tool_uses
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let hash = target_hash(t);
            let repeat_in_worker = seen_worker.contains(&hash);
            let repeat_in_run = seen_run.contains(&hash);
            seen_worker.insert(hash.clone());
            seen_run.insert(hash.clone());
            CallRow {
                seq: i as u32 + 1,
                tool: t.name.clone(),
                kind: kinds[i],
                result_chars: t.result_chars.map(|n| n as i64),
                target_hash: hash,
                repeat_in_worker,
                repeat_in_run,
                ts: t.ts.clone(),
                label_followup: labels[i].0,
                label_expanded: labels[i].1,
                in_subagent: t.in_subagent,
            }
        })
        .collect()
}

/// How many calls after a `ctx` answer a read of a file it named counts
/// as a follow-up (`followup_read_within_2`, §14).
pub const FOLLOWUP_WITHIN: usize = 2;
/// How many calls after a lookup a wider one on the same target counts as
/// an expansion (`expanded_within_5`, §14).
pub const EXPANDED_WITHIN: usize = 5;

/// `(label_followup, label_expanded)` per call: the first on every `ctx-*`
/// call, the second on every `read` and `ctx-refs` call, `None` elsewhere.
pub fn labels(tool_uses: &[ToolUse], kinds: &[Kind]) -> Vec<(Option<bool>, Option<bool>)> {
    (0..tool_uses.len())
        .map(|i| {
            let after = |n: usize| i + 1..tool_uses.len().min(i + 1 + n);
            let followup = kinds[i].is_ctx().then(|| {
                let named = ctx_paths(tool_uses[i].result_head.as_deref().unwrap_or(""));
                after(FOLLOWUP_WITHIN).any(|j| {
                    matches!(kinds[j], Kind::Read | Kind::BashCat | Kind::BashSed)
                        && read_paths(&tool_uses[j])
                            .iter()
                            .any(|p| named.iter().any(|n| names(p, n)))
                })
            });
            let expanded = match kinds[i] {
                Kind::Read => {
                    let (file, limit) = read_window(&tool_uses[i]);
                    Some(limit.is_some_and(|l| {
                        after(EXPANDED_WITHIN).any(|j| {
                            kinds[j] == Kind::Read && {
                                let (f, w) = read_window(&tool_uses[j]);
                                f == file && w.is_none_or(|w| w > l)
                            }
                        })
                    }))
                }
                Kind::CtxRefs => {
                    let (target, max) = refs_window(command(&tool_uses[i]));
                    Some(after(EXPANDED_WITHIN).any(|j| {
                        kinds[j] == Kind::CtxRefs && {
                            let (t, m) = refs_window(command(&tool_uses[j]));
                            t == target && m > max
                        }
                    }))
                }
                _ => None,
            };
            (followup, expanded)
        })
        .collect()
}

/// The files a `ctx` answer names: `path:line  kind  …` hit lines and
/// `== path ==` outline headers, as printed (relative to where it ran).
pub fn ctx_paths(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for line in text.lines() {
        if line.starts_with(char::is_whitespace) {
            continue;
        }
        let path = if let Some(inner) = line.strip_prefix("== ").and_then(|l| l.strip_suffix(" =="))
        {
            Some(inner)
        } else {
            line.split("  ")
                .next()
                .and_then(|head| head.rsplit_once(':'))
                .filter(|(p, n)| {
                    !p.is_empty() && !n.is_empty() && n.chars().all(|c| c.is_ascii_digit())
                })
                .map(|(p, _)| p)
        };
        if let Some(p) = path
            && !out.iter().any(|o| o == p)
        {
            out.push(p.to_string());
        }
    }
    out
}

/// The paths a read-like call names: `Read`'s `file_path`, a `Bash` call's
/// first-command operands (a `sed` script among them matches nothing).
pub fn read_paths(t: &ToolUse) -> Vec<String> {
    match t.name.as_str() {
        "Read" => vec![field(t, "file_path").to_string()],
        "Bash" => {
            let (words, _) = shell::first_command(command(t));
            words
                .into_iter()
                .skip(1)
                .filter(|w| !w.starts_with('-'))
                .collect()
        }
        _ => Vec::new(),
    }
}

/// `path` (absolute, or relative to the repo) names the repo-relative `rel`.
pub fn names(path: &str, rel: &str) -> bool {
    let rel = rel.trim_start_matches("./");
    !rel.is_empty() && (path.trim_start_matches("./") == rel || path.ends_with(&format!("/{rel}")))
}

/// A `Read`'s file and `limit` (`None`: the whole file).
fn read_window(t: &ToolUse) -> (&str, Option<u64>) {
    (
        field(t, "file_path"),
        t.input.get("limit").and_then(Value::as_u64),
    )
}

/// A `ctx refs` call's target (symbol and `--in`) and `--max` (default 10).
fn refs_window(command: &str) -> (String, usize) {
    let (words, _) = shell::first_command(command);
    let mut symbol = String::new();
    let mut dir = String::new();
    let mut max = 10;
    let mut it = words.iter().skip(3);
    while let Some(w) = it.next() {
        let (flag, inline) = match w.split_once('=') {
            Some((f, v)) if f.starts_with("--") => (f, Some(v.to_string())),
            _ => (w.as_str(), None),
        };
        match flag {
            "--max" | "--in" => {
                let v = inline.or_else(|| it.next().cloned()).unwrap_or_default();
                if flag == "--max" {
                    max = v.parse().unwrap_or(max);
                } else {
                    dir = v;
                }
            }
            f if f.starts_with('-') => {
                if f == "--per-file" && inline.is_none() {
                    it.next();
                }
            }
            _ if symbol.is_empty() => symbol = w.clone(),
            _ => {}
        }
    }
    (format!("{symbol}\n{dir}"), max)
}

/// A `read` or `bash-cat` of a file the worker's pack held (`held`,
/// repo-relative): `pack_doc_read_chars` (§4) counts its chars.
pub fn pack_doc(t: &ToolUse, kind: Kind, held: &[String], repo: &Path) -> bool {
    if !matches!(kind, Kind::Read | Kind::BashCat) || held.is_empty() {
        return false;
    }
    let repo = repo.to_string_lossy();
    let repo = repo.trim_end_matches('/');
    let at_root = |p: &str| format!("{repo}/{}", p.trim_start_matches("./"));
    read_paths(t).iter().any(|p| {
        let full = if p.starts_with('/') {
            p.clone()
        } else {
            at_root(p)
        };
        held.iter().any(|h| full == at_root(h))
    })
}

/// The `workers` subagent columns of what `measure::subagents` read:
/// `(sub_input, sub_cache_write, sub_cache_read, sub_output, subagents)`.
pub fn sub_columns(usage: &measure::Measure, count: usize) -> (i64, i64, i64, i64, i64) {
    let n = |v: u64| v as i64;
    (
        n(usage.input_tokens),
        n(usage.cache_write_tokens),
        n(usage.cache_read_tokens),
        n(usage.out_tokens),
        count as i64,
    )
}

/// Why a session has no subagent columns: nulls on the row, never zeros.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubagentsUnread {
    /// No `<session>.jsonl` where Claude Code keeps it: the path rule no
    /// longer matches its layout, or the transcript was cleaned up.
    TranscriptMissing,
    /// The `subagents/` dir, or a file in it, could not be read.
    Unreadable(String),
}

impl std::fmt::Display for SubagentsUnread {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SubagentsUnread::TranscriptMissing => f.write_str("transcript missing"),
            SubagentsUnread::Unreadable(e) => f.write_str(e),
        }
    }
}

/// The `workers` subagent columns for the session whose transcript is
/// `transcript`, as `record_subagents` and `report --recount` store them.
/// No `subagents/` dir is zeros; a missing transcript is told apart, so the
/// recount can say its sum is own tokens only.
pub fn read_subagents(
    transcript: &Path,
) -> std::result::Result<(i64, i64, i64, i64, i64), SubagentsUnread> {
    if fs::symlink_metadata(transcript).is_err() {
        return Err(SubagentsUnread::TranscriptMissing);
    }
    match measure::subagents(transcript) {
        Subagents::None => Ok((0, 0, 0, 0, 0)),
        Subagents::Read { usage, count } => Ok(sub_columns(&usage, count)),
        Subagents::Unreadable(e) => Err(SubagentsUnread::Unreadable(e)),
    }
}

/// The dir a worker ran in: its stream's `init` cwd (a lane, PRD-005 FR-2),
/// else the run's repo, for a stream with no `init` line. Its transcript
/// slug and its Read paths both hang off it.
pub fn worker_dir<'a>(cwd: Option<&'a Path>, repo: &'a Path) -> &'a Path {
    cwd.unwrap_or(repo)
}

/// A worker's transcript, slugged by the dir it ran in ([`worker_dir`]).
pub fn transcript_of(config: &Path, cwd: Option<&Path>, repo: &Path, session: &str) -> PathBuf {
    measure::transcript_path(config, worker_dir(cwd, repo), session)
}

/// PRD-003 FR-21: what the worker's subagents used, read from Claude
/// Code's transcript dir once the worker exited, onto its row. No dir is
/// zeros, and so is no transcript: a worker killed before its first turn
/// leaves none. A dir that cannot be read is nulls and one
/// `subagents_unread` event. The stream's own subagent lines are never a
/// fallback: their output count is an early snapshot. `Err` only when that
/// event could not be written; the callers drop it, so an exit always
/// settles.
pub fn record_subagents(
    run: &mut Run,
    worker_id: i64,
    session_id: &str,
    cwd: Option<&Path>,
) -> Result<()> {
    let path = transcript_of(
        &paths::claude_config_dir(),
        cwd,
        &run.state().repo,
        session_id,
    );
    let written = match read_subagents(&path) {
        Ok(cols) => run.db().set_worker_subagents(worker_id, Some(cols)),
        Err(SubagentsUnread::TranscriptMissing) => run
            .db()
            .set_worker_subagents(worker_id, Some((0, 0, 0, 0, 0))),
        Err(why) => {
            let _ = run.db().set_worker_subagents(worker_id, None);
            Err(crate::Error::Msg(why.to_string()))
        }
    };
    if let Err(e) = written {
        run.emit(EventKind::SubagentsUnread {
            worker_id,
            session_id: session_id.to_string(),
            path: path.display().to_string(),
            error: e.to_string(),
        })?;
    }
    Ok(())
}

/// One worker's `tool_calls` rows, written as one transaction, flags
/// against `seen_run` (the run's earlier targets, which this extends).
/// `record_exit` and `report --recount` (PRD-003 FR-21) both write through
/// it; the rows come back for the gate rows.
pub fn insert_calls(
    db: &Db,
    run_id: &str,
    worker_id: i64,
    tool_uses: &[ToolUse],
    gate: Option<&GateCmds>,
    repo: &Path,
    seen_run: &mut HashSet<String>,
) -> Result<Vec<CallRow>> {
    let rows = call_rows(tool_uses, gate, seen_run);
    let held = db.worker_pack_files(worker_id)?;
    let inserts: Vec<ToolCallInsert> = rows
        .iter()
        .zip(tool_uses)
        .map(|(r, t)| ToolCallInsert {
            run_id,
            worker_id,
            seq: r.seq,
            tool: &r.tool,
            kind: r.kind.as_str(),
            result_chars: r.result_chars,
            target_hash: &r.target_hash,
            repeat_in_worker: r.repeat_in_worker,
            repeat_in_run: r.repeat_in_run,
            ts: r.ts.as_deref(),
            label_followup: r.label_followup,
            label_expanded: r.label_expanded,
            pack_doc: pack_doc(t, r.kind, &held, repo),
            in_subagent: r.in_subagent,
        })
        .collect();
    db.insert_tool_calls(&inserts)?;
    Ok(rows)
}

/// Write a settled worker's `tool_calls` rows (one transaction) and its
/// inner-loop runs as `gates` rows. Called from both `settle`s
/// (`scheduler`, `checkpoint`) once the `workers` row is closed.
pub fn record_exit(run: &Run, worker_id: i64, summary: &Summary) -> Result<()> {
    if summary.tool_uses.is_empty() {
        return Ok(());
    }
    let db = run.db();
    let run_id = run.id().as_str();
    let mut seen_run = db.run_targets(run_id)?;
    let rows = insert_calls(
        db,
        run_id,
        worker_id,
        &summary.tool_uses,
        run.gate_cmds(),
        worker_dir(summary.cwd.as_deref(), &run.state().repo),
        &mut seen_run,
    )?;

    let now = clock::now_rfc3339();
    for (i, (t, r)) in summary.tool_uses.iter().zip(&rows).enumerate() {
        if r.kind != Kind::RunGate {
            continue;
        }
        let next = summary.tool_uses.get(i + 1);
        db.insert_gate(&GateInsert {
            run_id,
            source: "worker",
            kind: "slice",
            cmd: command(t),
            red: is_red(t),
            minutes: minutes(t, next),
            at: t.ts.as_deref().unwrap_or(&now),
            worker_id: Some(worker_id),
            cached: false,
            checkpoint: None,
            attempt: None,
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stream;
    use serde_json::json;

    fn use_(name: &str, input: Value) -> ToolUse {
        ToolUse {
            id: format!("toolu_{name}"),
            name: name.to_string(),
            input,
            result_chars: Some(3),
            ts: None,
            result_ts: None,
            result_error: false,
            result_head: None,
            in_subagent: false,
        }
    }

    fn bash(cmd: &str) -> ToolUse {
        use_("Bash", json!({ "command": cmd }))
    }

    fn gate() -> GateCmds {
        GateCmds {
            gate: "./gate.sh".to_string(),
            inner: Some("./gate.sh --inner".to_string()),
        }
    }

    #[test]
    fn bash_kinds_follow_the_first_command_after_cd_and_env() {
        let g = gate();
        let k = |c: &str| bash_kind(c, Some(&g));
        assert_eq!(k("cd x && grep -rn foo ."), Kind::BashGrep);
        assert_eq!(k("sed -n 1,40p f"), Kind::BashSed);
        assert_eq!(k("RUST_LOG=debug cat a.txt | head"), Kind::BashCat);
        assert_eq!(k("cd /r; tail -n 5 log"), Kind::BashCat);
        assert_eq!(k("/usr/bin/rg foo"), Kind::BashGrep);
        assert_eq!(k("./gate.sh --inner"), Kind::RunGate);
        assert_eq!(k("cd /r && ./gate.sh 2>&1 | tee out"), Kind::RunGate);
        assert_eq!(k("cat ./gate.sh"), Kind::BashCat);
        assert_eq!(k("cargo build"), Kind::Run);
        // A quoted prefix, a glued operator, a time prefix (AD-40006).
        assert_eq!(k("FOO=\"a b\" grep x ."), Kind::BashGrep);
        assert_eq!(k("cd /r&&sed -n 1p f"), Kind::BashSed);
        assert_eq!(k("time head -5 f"), Kind::BashCat);
        assert_eq!(k("echo grep"), Kind::Run);
        assert_eq!(k(""), Kind::Run);
        assert_eq!(bash_kind("./gate.sh", None), Kind::Run);
    }

    #[test]
    fn tools_map_to_kinds_and_targets() {
        let read = use_("Read", json!({ "file_path": "/r/a.rs" }));
        let edit = use_("Edit", json!({ "file_path": "/r/a.rs", "old_string": "x" }));
        let grep = use_("Grep", json!({ "pattern": "foo", "path": "/r" }));
        let glob = use_("Glob", json!({ "pattern": "**/*.rs" }));
        let agent = use_("Agent", json!({ "prompt": "p" }));
        assert_eq!(classify(&read, None), Kind::Read);
        assert_eq!(classify(&edit, None), Kind::Edit);
        assert_eq!(classify(&grep, None), Kind::Grep);
        assert_eq!(classify(&glob, None), Kind::Grep);
        assert_eq!(classify(&agent, None), Kind::Other);
        assert_eq!(target_hash(&read), target_hash(&edit));
        assert_eq!(target(&grep), "foo\n/r");
        assert_eq!(target(&glob), "**/*.rs\n");
        assert_eq!(target(&bash("  grep  -rn  foo . ")), "grep -rn foo .");
        assert_eq!(target_hash(&read).len(), 64);
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn repeat_flags_are_per_worker_and_per_run() {
        let f = || use_("Read", json!({ "file_path": "f" }));
        let mut seen = HashSet::new();
        let first = call_rows(
            &[f(), bash("sed -n 1,40p f"), f()],
            Some(&gate()),
            &mut seen,
        );
        assert_eq!(
            first
                .iter()
                .map(|r| (r.seq, r.kind, r.repeat_in_worker, r.repeat_in_run))
                .collect::<Vec<_>>(),
            vec![
                (1, Kind::Read, false, false),
                (2, Kind::BashSed, false, false),
                (3, Kind::Read, true, true),
            ]
        );
        let second = call_rows(&[f()], None, &mut seen);
        assert!(!second[0].repeat_in_worker);
        assert!(second[0].repeat_in_run);
    }

    #[test]
    fn a_subagent_call_keeps_its_place_in_the_sequence() {
        let mut sub = bash("cat g");
        sub.in_subagent = true;
        let r = rows(&[use_("Read", json!({ "file_path": "f" })), sub, bash("ls")]);
        assert_eq!(
            r.iter().map(|r| (r.seq, r.in_subagent)).collect::<Vec<_>>(),
            vec![(1, false), (2, true), (3, false)]
        );
    }

    fn ctx(cmd: &str, answer: &str) -> ToolUse {
        let mut t = bash(cmd);
        t.result_head = Some(answer.to_string());
        t
    }

    fn read(file: &str, limit: Option<u64>) -> ToolUse {
        match limit {
            Some(l) => use_(
                "Read",
                json!({ "file_path": file, "offset": 1, "limit": l }),
            ),
            None => use_("Read", json!({ "file_path": file })),
        }
    }

    const REFS: &str = "src/a.rs:3  ref  let x = helper();\n    y\n\n\
                        src/b.rs:9  ref  helper()\n\n2 hits in 2 files (searched 4 files)\n";

    fn rows(uses: &[ToolUse]) -> Vec<CallRow> {
        call_rows(uses, Some(&gate()), &mut HashSet::new())
    }

    #[test]
    fn ctx_calls_get_their_own_kinds_ahead_of_the_rest() {
        let g = gate();
        let k = |c: &str| bash_kind(c, Some(&g));
        assert_eq!(k("factory ctx def helper"), Kind::CtxDef);
        assert_eq!(
            k("cd /r && factory ctx refs helper --max 30"),
            Kind::CtxRefs
        );
        assert_eq!(k("/usr/bin/factory ctx outline src/a.rs"), Kind::CtxOutline);
        assert_eq!(k("factory ctx tags"), Kind::Run);
        assert_eq!(k("factory run 001"), Kind::Run);
        assert!(Kind::CtxRefs.is_lookup() && Kind::CtxRefs.is_ctx());
        assert!(LOOKUP_KINDS.contains(&"ctx-outline"));
        assert_eq!(
            ctx_paths("== src/x.rs ==\n3  fn  main()\n\n== src/y.vue ==\n"),
            vec!["src/x.rs", "src/y.vue"]
        );
        assert_eq!(ctx_paths(REFS), vec!["src/a.rs", "src/b.rs"]);
    }

    #[test]
    fn a_read_of_a_named_file_within_two_calls_is_a_followup() {
        let r = rows(&[
            ctx("factory ctx refs helper", REFS),
            read("/repo/src/b.rs", None),
        ]);
        assert_eq!(r[0].label_followup, Some(true));
        assert_eq!(r[1].label_followup, None);

        let r = rows(&[
            ctx("factory ctx refs helper", REFS),
            read("/repo/src/c.rs", None),
            bash("cargo build"),
            read("/repo/src/a.rs", None),
        ]);
        assert_eq!(
            r[0].label_followup,
            Some(false),
            "the third call is too late"
        );

        let r = rows(&[
            ctx("factory ctx def helper", REFS),
            bash("ls"),
            bash("sed -n 1,20p src/a.rs"),
        ]);
        assert_eq!(r[0].label_followup, Some(true), "bash-sed counts");
        let r = rows(&[
            ctx("factory ctx def helper", REFS),
            bash("grep -n x src/a.rs"),
        ]);
        assert_eq!(r[0].label_followup, Some(false), "a grep is no read");
    }

    #[test]
    fn a_wider_read_or_a_larger_max_within_five_calls_is_an_expansion() {
        let r = rows(&[
            read("/r/f.rs", Some(40)),
            bash("ls"),
            read("/r/f.rs", Some(200)),
        ]);
        assert_eq!(r[0].label_expanded, Some(true));
        assert_eq!(r[2].label_expanded, Some(false));
        assert_eq!(r[1].label_expanded, None);

        let r = rows(&[read("/r/f.rs", Some(40)), read("/r/f.rs", None)]);
        assert_eq!(r[0].label_expanded, Some(true), "the whole file is wider");
        let r = rows(&[read("/r/f.rs", None), read("/r/f.rs", None)]);
        assert_eq!(
            r[0].label_expanded,
            Some(false),
            "nothing is wider than whole"
        );
        let r = rows(&[read("/r/f.rs", Some(40)), read("/r/g.rs", Some(400))]);
        assert_eq!(r[0].label_expanded, Some(false), "another file");
        let mut late = vec![read("/r/f.rs", Some(40))];
        late.extend((0..5).map(|_| bash("ls")));
        late.push(read("/r/f.rs", Some(400)));
        assert_eq!(rows(&late)[0].label_expanded, Some(false), "six calls on");

        let r = rows(&[
            ctx("factory ctx refs helper", REFS),
            ctx("factory ctx refs helper --max=40", REFS),
            ctx("factory ctx refs other --max 99", REFS),
        ]);
        assert_eq!(r[0].label_expanded, Some(true));
        assert_eq!(r[1].label_expanded, Some(false));
        let r = rows(&[
            ctx("factory ctx refs helper --max 30 --in src", REFS),
            ctx("factory ctx refs helper --max 50 --in tests", REFS),
        ]);
        assert_eq!(r[0].label_expanded, Some(false), "another --in");
        assert_eq!(
            rows(&[ctx("factory ctx def helper", REFS)])[0].label_expanded,
            None
        );
    }

    #[test]
    fn a_lane_worker_is_measured_where_it_ran() {
        let config = tempfile::tempdir().unwrap();
        let (lane, repo) = (Path::new("/x/lane-2"), Path::new("/x/repo"));
        let t = transcript_of(config.path(), Some(lane), repo, "s1");
        assert_eq!(t, measure::transcript_path(config.path(), lane, "s1"));
        assert_eq!(
            transcript_of(config.path(), None, repo, "s1"),
            measure::transcript_path(config.path(), repo, "s1"),
            "no init cwd: the run's repo, as before lanes"
        );
        let sub = t.with_extension("").join("subagents");
        fs::create_dir_all(&sub).unwrap();
        fs::write(&t, "").unwrap();
        let line = r#"{"type":"assistant","timestamp":"2026-10-13T10:00:00Z","message":{"id":"a","usage":{"input_tokens":2,"cache_creation_input_tokens":10,"cache_read_input_tokens":100,"output_tokens":5}}}"#;
        fs::write(sub.join("agent-1.jsonl"), format!("{line}\n")).unwrap();
        assert_eq!(read_subagents(&t), Ok((2, 10, 100, 5, 1)));
        assert_eq!(
            read_subagents(&transcript_of(config.path(), None, repo, "s1")),
            Err(SubagentsUnread::TranscriptMissing)
        );

        let held = vec!["src/lib.txt".to_string()];
        let r = read("/x/lane-2/src/lib.txt", None);
        let kind = classify(&r, None);
        assert!(pack_doc(&r, kind, &held, worker_dir(Some(lane), repo)));
        assert!(!pack_doc(&r, kind, &held, worker_dir(None, repo)));
    }

    #[test]
    fn a_read_of_a_held_file_is_a_pack_doc() {
        let held = vec![
            "docs/features/001-x/slice-01-a.md".to_string(),
            "docs/features/001-x/prd.md".to_string(),
        ];
        let repo = Path::new("/repo");
        let r = |t: &ToolUse| pack_doc(t, classify(t, None), &held, repo);
        assert!(r(&read("/repo/docs/features/001-x/prd.md", None)));
        assert!(r(&read("/repo/docs/features/001-x/prd.md", Some(40))));
        assert!(r(&bash("cat docs/features/001-x/slice-01-a.md")));
        assert!(
            !r(&bash("sed -n 1,9p docs/features/001-x/prd.md")),
            "sed is not counted"
        );
        assert!(!r(&read("/repo/docs/features/002-y/prd.md", None)));
        // Anchored at the repo root: the same tail under another directory,
        // or another checkout, is not the held file.
        assert!(!r(&read("/repo/vendor/docs/features/001-x/prd.md", None)));
        assert!(!r(&read("/other/docs/features/001-x/prd.md", None)));
        assert!(!r(&bash("cat sub/docs/features/001-x/prd.md")));
        assert!(r(&bash("cat ./docs/features/001-x/prd.md")));
        assert!(pack_doc(
            &read("/repo/docs/features/001-x/prd.md", None),
            Kind::Read,
            &held,
            Path::new("/repo/")
        ));
        assert!(!pack_doc(
            &read("/repo/docs/features/001-x/prd.md", None),
            Kind::Read,
            &[],
            repo
        ));
    }

    #[test]
    fn red_comes_from_is_error_fail_or_a_nonzero_exit() {
        let mut t = bash("./gate.sh --inner");
        assert!(!is_red(&t));
        t.result_head = Some("ok\n".to_string());
        assert!(!is_red(&t));
        t.result_head = Some("FAIL tests/fake.rs: expected 2 got 3\n".to_string());
        assert!(is_red(&t));
        t.result_head = Some("warning: FAILURE is a word\nExit code 0".to_string());
        assert!(!is_red(&t)); // only a line starting with FAIL is the marker
        t.result_head = Some("compiling\nExit code 101".to_string());
        assert!(is_red(&t));
        t.result_head = Some("all good\nExit code 0".to_string());
        assert!(!is_red(&t));
        t.result_error = true;
        assert!(is_red(&t));
    }

    #[test]
    fn minutes_come_from_the_timestamps_when_present() {
        let mut t = bash("./gate.sh --inner");
        assert_eq!(minutes(&t, None), 0.0);
        t.ts = Some("2026-10-10T10:00:00.000Z".to_string());
        t.result_ts = Some("2026-10-10T10:01:30.000Z".to_string());
        assert!((minutes(&t, None) - 1.5).abs() < 1e-9);
        t.result_ts = None;
        let mut next = bash("ls");
        next.ts = Some("2026-10-10T10:03:00Z".to_string());
        assert!((minutes(&t, Some(&next)) - 3.0).abs() < 1e-9);
    }

    #[test]
    fn the_recorded_stream_yields_one_row_per_tool_use_with_its_result_chars() {
        let text = include_str!("../tests/fixture/recordings/claude-p-oneshot-2.1.280.jsonl");
        let (events, summary) = stream::parse(text);
        let tool_uses = events
            .iter()
            .filter_map(|e| match e {
                stream::StreamEvent::Assistant { message, .. } => Some(
                    message
                        .content
                        .iter()
                        .filter(|b| b.get("type").and_then(Value::as_str) == Some("tool_use"))
                        .count(),
                ),
                _ => None,
            })
            .sum::<usize>();
        assert!(tool_uses > 0);
        let rows = call_rows(&summary.tool_uses, None, &mut HashSet::new());
        assert_eq!(rows.len(), tool_uses);
        for (r, t) in rows.iter().zip(&summary.tool_uses) {
            assert!(r.result_chars.is_some(), "{}", t.name);
            assert_eq!(r.result_chars, t.result_chars.map(|n| n as i64));
            assert!(!r.target_hash.is_empty());
            assert!(r.ts.is_some(), "recording lines carry timestamps");
        }
        assert_eq!(rows.first().map(|r| r.seq), Some(1));
    }
}
