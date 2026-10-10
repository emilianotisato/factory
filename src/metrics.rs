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
use std::collections::HashSet;

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::Result;
use crate::clock;
use crate::db::{GateInsert, ToolCallInsert};
use crate::measure::epoch_secs;
use crate::run::{GateCmds, Run};
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
            Kind::Read | Kind::Grep | Kind::BashGrep | Kind::BashSed | Kind::BashCat
        )
    }
}

/// The lookup kinds as the report's SQL names them.
pub const LOOKUP_KINDS: [&str; 5] = ["read", "grep", "bash-grep", "bash-sed", "bash-cat"];

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

/// The first command word after leading `cd … &&` (or `;`) segments, `env`
/// and `VAR=value` tokens, with any directory prefix dropped.
pub fn first_word(command: &str) -> Option<String> {
    let mut tokens = command.split_whitespace().peekable();
    loop {
        let tok = *tokens.peek()?;
        if tok == "cd" || tok == "pushd" {
            for t in tokens.by_ref() {
                if t == "&&" || t == ";" || t == "||" || t.ends_with(';') {
                    break;
                }
            }
            continue;
        }
        if tok == "env" || tok == "time" || is_assignment(tok) {
            tokens.next();
            continue;
        }
        let word = tok.rsplit('/').next().unwrap_or(tok);
        return Some(word.to_string());
    }
}

fn is_assignment(tok: &str) -> bool {
    let Some((name, _)) = tok.split_once('=') else {
        return false;
    };
    !name.is_empty()
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        && !name.starts_with(|c: char| c.is_ascii_digit())
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
}

pub fn call_rows(
    tool_uses: &[ToolUse],
    gate: Option<&GateCmds>,
    seen_run: &mut HashSet<String>,
) -> Vec<CallRow> {
    let mut seen_worker: HashSet<String> = HashSet::new();
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
                kind: classify(t, gate),
                result_chars: t.result_chars.map(|n| n as i64),
                target_hash: hash,
                repeat_in_worker,
                repeat_in_run,
                ts: t.ts.clone(),
            }
        })
        .collect()
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
    let gate = run.gate_cmds();
    let mut seen_run = db.run_targets(run_id)?;
    let rows = call_rows(&summary.tool_uses, gate, &mut seen_run);
    let inserts: Vec<ToolCallInsert> = rows
        .iter()
        .map(|r| ToolCallInsert {
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
        })
        .collect();
    db.insert_tool_calls(&inserts)?;

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
        let text =
            include_str!("../docs/features/001-driver/recordings/claude-p-oneshot-2.1.280.jsonl");
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
