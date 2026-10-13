//! The cockpit's own cost (PRD §10 FR-8 "cockpit_tokens from the cockpit
//! session's transcript, the same way `measure.py` does"): a port of the old
//! skills repo's `measure.py:42-67` (per assistant message `ctx = input +
//! cache_creation + cache_read`, out, cache reads, start and peak context,
//! messages deduped by `message.id` because streaming logs one message more
//! than once) and `:87-150` (subagent transcripts under
//! `<session>/subagents/*.jsonl` summed in, several sessions combined).
//! The build and polish minute markers stay out: the driver has its own
//! clock for those (AD-805).
//!
//! PRD-003 FR-21 reads a worker's subagents the same way, apart from its
//! own usage: [`transcript_path`] finds the session file under Claude
//! Code's projects dir, [`subagents`] sums what sits beside it.
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::error::io;
use crate::{Result, clock, quota};

/// What one or more cockpit transcripts amount to.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Measure {
    pub sessions: usize,
    pub subagents: usize,
    /// Assistant messages counted, after dedupe.
    pub calls: u64,
    pub input_tokens: u64,
    pub out_tokens: u64,
    pub cache_write_tokens: u64,
    pub cache_read_tokens: u64,
    /// The first message's context at or after the `skills/deliver` or
    /// `skills/factory-deliver` prompt (`measure.py:148`), else the first
    /// message's; 0 with no messages.
    pub start_context: u64,
    pub peak_context: u64,
}

impl Measure {
    /// The cockpit's weighted tokens, with slice 07's weights so it compares
    /// with the run's `weighted_tokens` (§13 FR-11).
    pub fn weighted(&self) -> u64 {
        quota::weigh(
            self.input_tokens,
            self.cache_write_tokens,
            self.cache_read_tokens,
            self.out_tokens,
        )
        .round() as u64
    }
}

/// One assistant message's usage, `measure.py:42-53`.
#[derive(Debug, Clone, PartialEq)]
struct Msg {
    ts: f64,
    input: u64,
    cache_write: u64,
    cache_read: u64,
    out: u64,
}

impl Msg {
    fn ctx(&self) -> u64 {
        self.input + self.cache_write + self.cache_read
    }
}

/// `measure.py` `ts()`: a transcript timestamp as epoch seconds, with the
/// fraction; `None` for anything that is not `YYYY-MM-DDTHH:MM:SS[.fff]Z`.
pub fn epoch_secs(ts: &str) -> Option<f64> {
    let body = ts.strip_suffix('Z')?;
    let (whole, frac) = match body.split_once('.') {
        Some((w, f)) => (w, f),
        None => (body, ""),
    };
    let secs = clock::from_rfc3339(&format!("{whole}Z"))? as f64;
    let frac = if frac.is_empty() {
        0.0
    } else {
        format!("0.{frac}").parse::<f64>().ok()?
    };
    Some(secs + frac)
}

fn load(path: &Path) -> Result<Vec<Value>> {
    let text = fs::read_to_string(path).map_err(io(format!("read {}", path.display())))?;
    Ok(text
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .collect())
}

fn num(u: &Value, key: &str) -> u64 {
    u.get(key).and_then(Value::as_u64).unwrap_or(0)
}

/// `assistant_msgs`: assistant rows with usage and a timestamp, deduped by
/// `message.id` (the last occurrence wins, as a dict assignment does),
/// sorted by timestamp.
fn assistant_msgs(rows: &[Value]) -> Vec<Msg> {
    let mut msgs: Vec<Msg> = assistant_by_id(rows).into_values().collect();
    msgs.sort_by(|a, b| a.ts.total_cmp(&b.ts));
    msgs
}

/// The assistant rows of `assistant_msgs`, keyed by `message.id` (else the
/// row's `uuid`, else "").
fn assistant_by_id(rows: &[Value]) -> HashMap<String, Msg> {
    let mut by_id: HashMap<String, Msg> = HashMap::new();
    for r in rows {
        if r.get("type").and_then(Value::as_str) != Some("assistant") {
            continue;
        }
        let Some(m) = r.get("message") else { continue };
        let Some(u) = m.get("usage").filter(|u| u.is_object()) else {
            continue;
        };
        let Some(ts) = r
            .get("timestamp")
            .and_then(Value::as_str)
            .and_then(epoch_secs)
        else {
            continue;
        };
        let id = m
            .get("id")
            .and_then(Value::as_str)
            .or_else(|| r.get("uuid").and_then(Value::as_str))
            .unwrap_or("")
            .to_string();
        by_id.insert(
            id,
            Msg {
                ts,
                input: num(u, "input_tokens"),
                cache_write: num(u, "cache_creation_input_tokens"),
                cache_read: num(u, "cache_read_input_tokens"),
                out: num(u, "output_tokens"),
            },
        );
    }
    by_id
}

/// The prompt that starts a delivery: `measure.py`'s `skills/deliver`, or
/// this repo's cockpit skill, `skills/factory-deliver`.
fn is_deliver_prompt(text: &str) -> bool {
    ["skills/deliver", "skills/factory-deliver"]
        .iter()
        .any(|s| text.contains(s))
}

/// A user row's text, string or text blocks.
fn user_text(r: &Value) -> String {
    match r.get("message").and_then(|m| m.get("content")) {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(blocks)) => blocks
            .iter()
            .filter_map(|b| b.get("text").and_then(Value::as_str))
            .collect(),
        _ => String::new(),
    }
}

fn add_totals(m: &mut Measure, msgs: &[Msg]) {
    for x in msgs {
        m.calls += 1;
        m.input_tokens += x.input;
        m.out_tokens += x.out;
        m.cache_write_tokens += x.cache_write;
        m.cache_read_tokens += x.cache_read;
        m.peak_context = m.peak_context.max(x.ctx());
    }
}

/// One session: the main transcript plus `<path minus .jsonl>/subagents/*.jsonl`.
pub fn measure(path: &Path) -> Result<Measure> {
    let rows = load(path)?;
    let main = assistant_msgs(&rows);
    let deliver = rows
        .iter()
        .filter(|r| r.get("type").and_then(Value::as_str) == Some("user"))
        .filter(|r| is_deliver_prompt(&user_text(r)))
        .find_map(|r| {
            r.get("timestamp")
                .and_then(Value::as_str)
                .and_then(epoch_secs)
        });

    let mut m = Measure {
        sessions: 1,
        ..Measure::default()
    };
    add_totals(&mut m, &main);
    let start_at = deliver.or_else(|| main.first().map(|x| x.ts));
    m.start_context = start_at
        .and_then(|at| main.iter().find(|x| x.ts >= at))
        .map_or(0, Msg::ctx);

    // The cockpit has always skipped a dir it cannot read.
    if let Subagents::Read { usage, count } = subagents(path) {
        m.subagents += count;
        m.calls += usage.calls;
        m.input_tokens += usage.input_tokens;
        m.out_tokens += usage.out_tokens;
        m.cache_write_tokens += usage.cache_write_tokens;
        m.cache_read_tokens += usage.cache_read_tokens;
        m.peak_context = m.peak_context.max(usage.peak_context);
    }
    Ok(m)
}

/// What a session's subagents used, apart from its own messages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Subagents {
    /// No `<session>/subagents/` dir: Claude Code writes one only when the
    /// session spawned a subagent.
    None,
    /// Final usage per message summed over every `*.jsonl` there; `count`
    /// is the files that had an assistant message.
    Read { usage: Measure, count: usize },
    /// The dir exists but it, or a file in it, could not be read.
    Unreadable(String),
}

/// The subagents beside `transcript`: `<transcript minus .jsonl>/subagents`.
pub fn subagents(transcript: &Path) -> Subagents {
    let Some(dir) = transcript
        .to_str()
        .and_then(|p| p.strip_suffix(".jsonl"))
        .map(|p| PathBuf::from(p).join("subagents"))
    else {
        return Subagents::None;
    };
    // `exists` is false on a permission error too; the read below tells.
    if fs::symlink_metadata(&dir).is_err() {
        return Subagents::None;
    }
    let rd = match fs::read_dir(&dir) {
        Ok(rd) => rd,
        Err(e) => return Subagents::Unreadable(format!("read {}: {e}", dir.display())),
    };
    let mut files: Vec<PathBuf> = rd
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "jsonl"))
        .collect();
    files.sort();
    // A forked subagent's file opens with copies of the parent's messages,
    // and sibling forks repeat each other's: a message id counts once per
    // session, and the parent's own are already in its `result.usage`.
    let mut seen: HashSet<String> = load(transcript)
        .map(|rows| assistant_by_id(&rows).into_keys().collect())
        .unwrap_or_default();
    let mut usage = Measure::default();
    let mut count = 0;
    for f in files {
        let rows = match load(&f) {
            Ok(rows) => rows,
            Err(e) => return Subagents::Unreadable(e.to_string()),
        };
        let msgs: Vec<Msg> = assistant_by_id(&rows)
            .into_iter()
            .filter(|(id, _)| id.is_empty() || seen.insert(id.clone()))
            .map(|(_, m)| m)
            .collect();
        if msgs.is_empty() {
            continue;
        }
        count += 1;
        add_totals(&mut usage, &msgs);
    }
    Subagents::Read { usage, count }
}

/// Where Claude Code keeps a session's transcript: `<config
/// dir>/projects/<cwd with every character but an ASCII letter or digit as
/// "-">/<session>.jsonl`. The cwd is the worker's, the run's repo. Not only
/// "/" and ".": `ICG/apk_fiscal_arca` is `-ICG-apk-fiscal-arca` on disk.
pub fn transcript_path(config_dir: &Path, repo: &Path, session_id: &str) -> PathBuf {
    let slug: String = repo
        .to_string_lossy()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    config_dir
        .join("projects")
        .join(slug)
        .join(format!("{session_id}.jsonl"))
}

/// `combine`: several sessions of one run. Tokens and calls add; the start
/// context is the first session's, the peak the max.
pub fn combine(paths: &[PathBuf]) -> Result<Measure> {
    let mut all = Measure::default();
    for (i, p) in paths.iter().enumerate() {
        let m = measure(p)?;
        if i == 0 {
            all.start_context = m.start_context;
        }
        all.sessions += m.sessions;
        all.subagents += m.subagents;
        all.calls += m.calls;
        all.input_tokens += m.input_tokens;
        all.out_tokens += m.out_tokens;
        all.cache_write_tokens += m.cache_write_tokens;
        all.cache_read_tokens += m.cache_read_tokens;
        all.peak_context = all.peak_context.max(m.peak_context);
    }
    Ok(all)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixture/cockpit-session.jsonl")
    }

    /// Pinned on 2026-10-10 with
    /// `python3 ~/Dropbox/dev/claude-code-skills/deliver/bin/measure.py tests/fixture/cockpit-session.jsonl --retro`
    /// (the slice's `~/.claude/skills/feature-docs/bin/measure.py` path does
    /// not exist; the script lives in the old skills repo):
    /// `out_tokens: 2524`, `cache_read_tokens: 181841`,
    /// `start_context: 45507`, `peak_context: 58351` (build and polish
    /// minutes empty). The input and cache-write sums (98, 32926) come from
    /// the same deduped messages and give the weighted figure.
    #[test]
    fn the_fixture_transcript_matches_measure_py() {
        let m = measure(&fixture()).unwrap();
        assert_eq!(m.out_tokens, 2524);
        assert_eq!(m.cache_read_tokens, 181841);
        assert_eq!(m.start_context, 45507);
        assert_eq!(m.peak_context, 58351);
        assert_eq!(m.calls, 4, "nine assistant lines, four message ids");
        assert_eq!(m.input_tokens, 98);
        assert_eq!(m.cache_write_tokens, 32926);
        assert_eq!(m.subagents, 0);
        // 98 + 32926 + 181841 × 0.1 + 2524 × 5
        assert_eq!(m.weighted(), 63_828);
    }

    #[test]
    fn combine_adds_tokens_keeps_the_first_start_and_the_max_peak() {
        let one = measure(&fixture()).unwrap();
        let two = combine(&[fixture(), fixture()]).unwrap();
        assert_eq!(two.sessions, 2);
        assert_eq!(two.out_tokens, one.out_tokens * 2);
        assert_eq!(two.cache_read_tokens, one.cache_read_tokens * 2);
        assert_eq!(two.start_context, one.start_context);
        assert_eq!(two.peak_context, one.peak_context);
        assert_eq!(combine(&[]).unwrap(), Measure::default());
    }

    #[test]
    fn the_start_is_the_deliver_or_factory_deliver_prompt() {
        let dir = tempfile::tempdir().unwrap();
        let msg = |id: &str, ts: &str, ctx: u64| {
            format!(
                r#"{{"type":"assistant","timestamp":"{ts}","message":{{"id":"{id}","usage":{{"input_tokens":{ctx},"output_tokens":1}}}}}}"#
            )
        };
        let prompt = |ts: &str, skill: &str| {
            format!(
                r#"{{"type":"user","timestamp":"{ts}","message":{{"content":[{{"type":"text","text":"Base directory for this skill: /h/.claude/{skill}"}}]}}}}"#
            )
        };
        for (skill, start) in [
            ("skills/factory-deliver", 300),
            ("skills/deliver", 300),
            ("skills/other", 100),
        ] {
            let path = dir.path().join("s.jsonl");
            let lines = [
                msg("a", "2026-10-10T10:00:00Z", 100),
                prompt("2026-10-10T10:00:05Z", skill),
                msg("b", "2026-10-10T10:00:09Z", 300),
            ];
            fs::write(&path, lines.join("\n")).unwrap();
            assert_eq!(measure(&path).unwrap().start_context, start, "{skill}");
        }
    }

    #[test]
    fn epoch_secs_reads_fractions_and_rejects_other_shapes() {
        assert_eq!(epoch_secs("1970-01-01T00:00:01Z"), Some(1.0));
        assert_eq!(epoch_secs("1970-01-01T00:00:01.500Z"), Some(1.5));
        assert_eq!(epoch_secs("1970-01-01T00:00:01+00:00"), None);
        assert_eq!(epoch_secs("nope"), None);
    }

    #[test]
    fn a_missing_transcript_is_an_error_not_a_zero() {
        assert!(measure(Path::new("/nonexistent/x.jsonl")).is_err());
    }
    #[test]
    fn the_transcript_path_slugs_the_repo() {
        assert_eq!(
            transcript_path(
                Path::new("/tmp/c"),
                Path::new("/home/e/Projects/Prokey/wt-b"),
                "abc"
            ),
            PathBuf::from("/tmp/c/projects/-home-e-Projects-Prokey-wt-b/abc.jsonl")
        );
        assert_eq!(
            transcript_path(Path::new("/c"), Path::new("/a/b.c/d"), "s"),
            PathBuf::from("/c/projects/-a-b-c-d/s.jsonl")
        );
        // Claude Code's rule: every character but an ASCII letter or digit.
        assert_eq!(
            transcript_path(Path::new("/c"), Path::new("/p/ICG/apk_fiscal arca+1"), "s"),
            PathBuf::from("/c/projects/-p-ICG-apk-fiscal-arca-1/s.jsonl")
        );
    }

    /// Two lines on one message id with growing usage: the final one counts.
    fn agent_lines(id: &str, out: u64) -> String {
        let line = |o: u64, ts: &str| {
            format!(
                r#"{{"type":"assistant","timestamp":"{ts}","message":{{"id":"{id}","usage":{{"input_tokens":2,"cache_creation_input_tokens":10,"cache_read_input_tokens":100,"output_tokens":{o}}}}}}}"#
            )
        };
        format!(
            "{}\n{}\n",
            line(1, "2026-10-13T10:00:00Z"),
            line(out, "2026-10-13T10:00:01Z")
        )
    }

    #[test]
    fn subagents_sit_apart_beside_the_session() {
        let dir = tempfile::tempdir().unwrap();
        let path = transcript_path(dir.path(), Path::new("/r/x"), "s1");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, agent_lines("own", 7)).unwrap();
        assert_eq!(subagents(&path), Subagents::None);

        let sub = path.parent().unwrap().join("s1/subagents");
        fs::create_dir_all(&sub).unwrap();
        assert_eq!(
            subagents(&path),
            Subagents::Read {
                usage: Measure::default(),
                count: 0
            }
        );
        fs::write(sub.join("agent-1.jsonl"), agent_lines("a", 50)).unwrap();
        fs::write(sub.join("agent-2.jsonl"), agent_lines("b", 30)).unwrap();
        fs::write(sub.join("agent-1.meta.json"), r#"{"agentType":"x"}"#).unwrap();
        fs::write(sub.join("agent-3.jsonl"), "{\"type\":\"user\"}\n").unwrap();
        let Subagents::Read { usage, count } = subagents(&path) else {
            panic!("{:?}", subagents(&path));
        };
        assert_eq!(count, 2);
        assert_eq!(
            (
                usage.input_tokens,
                usage.cache_write_tokens,
                usage.cache_read_tokens,
                usage.out_tokens
            ),
            (4, 20, 200, 80)
        );
        // The cockpit's single figure keeps adding them in.
        let m = measure(&path).unwrap();
        assert_eq!((m.out_tokens, m.subagents), (87, 2));
    }

    /// A fork's file opens with copies of the parent's messages and of its
    /// siblings': each message id counts once, and a file holding only
    /// copies is not a subagent.
    #[test]
    fn a_forks_copied_messages_count_once() {
        let dir = tempfile::tempdir().unwrap();
        let path = transcript_path(dir.path(), Path::new("/r/x"), "s1");
        let sub = path.parent().unwrap().join("s1/subagents");
        fs::create_dir_all(&sub).unwrap();
        fs::write(&path, agent_lines("own", 7)).unwrap();
        fs::write(sub.join("agent-1.jsonl"), agent_lines("a", 50)).unwrap();
        fs::write(
            sub.join("agent-2.jsonl"),
            format!("{}{}", agent_lines("own", 7), agent_lines("a", 50)),
        )
        .unwrap();
        fs::write(
            sub.join("agent-3.jsonl"),
            format!("{}{}", agent_lines("a", 50), agent_lines("b", 30)),
        )
        .unwrap();
        let Subagents::Read { usage, count } = subagents(&path) else {
            panic!("{:?}", subagents(&path));
        };
        assert_eq!(count, 2);
        assert_eq!((usage.input_tokens, usage.out_tokens), (4, 80));
    }

    #[cfg(unix)]
    #[test]
    fn a_dir_that_cannot_be_read_is_unreadable_not_none() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s.jsonl");
        fs::write(&path, "").unwrap();
        let sub = dir.path().join("s/subagents");
        fs::create_dir_all(&sub).unwrap();
        fs::write(sub.join("agent-1.jsonl"), agent_lines("a", 5)).unwrap();
        fs::set_permissions(&sub, fs::Permissions::from_mode(0o000)).unwrap();
        // Root reads through any mode: nothing to test there.
        let root = fs::read_dir(&sub).is_ok();
        let got = subagents(&path);
        fs::set_permissions(&sub, fs::Permissions::from_mode(0o755)).unwrap();
        if root {
            return;
        }
        assert!(matches!(got, Subagents::Unreadable(_)), "{got:?}");
    }
}
