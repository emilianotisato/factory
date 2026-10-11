//! The cockpit's own cost (PRD §10 FR-8 "cockpit_tokens from the cockpit
//! session's transcript, the same way `measure.py` does"): a port of the old
//! skills repo's `measure.py:42-67` (per assistant message `ctx = input +
//! cache_creation + cache_read`, out, cache reads, start and peak context,
//! messages deduped by `message.id` because streaming logs one message more
//! than once) and `:87-150` (subagent transcripts under
//! `<session>/subagents/*.jsonl` summed in, several sessions combined).
//! The build and polish minute markers stay out: the driver has its own
//! clock for those (AD-805).
use std::collections::HashMap;
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
    let mut msgs: Vec<Msg> = by_id.into_values().collect();
    msgs.sort_by(|a, b| a.ts.total_cmp(&b.ts));
    msgs
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

    let sub_dir = path
        .to_str()
        .and_then(|p| p.strip_suffix(".jsonl"))
        .map(|p| PathBuf::from(p).join("subagents"));
    if let Some(dir) = sub_dir
        && let Ok(rd) = fs::read_dir(&dir)
    {
        let mut files: Vec<PathBuf> = rd
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "jsonl"))
            .collect();
        files.sort();
        for f in files {
            let msgs = assistant_msgs(&load(&f)?);
            if msgs.is_empty() {
                continue;
            }
            m.subagents += 1;
            add_totals(&mut m, &msgs);
        }
    }
    Ok(m)
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
}
