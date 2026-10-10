//! `claude -p --output-format stream-json` parsed into a worker row (PRD §4
//! FR-2: "the stream is the record"). A serde enum over `type` with an
//! `Unknown` catch-all and `serde_json::Value` for every field the driver does
//! not read, so a Claude Code upgrade adds lines and fields without a panic.
//! `peak_context` follows `measure.py:56-67`: per assistant message
//! `input + cache_creation + cache_read`, peak is the maximum.
use std::collections::HashMap;

use serde::Deserialize;
use serde_json::{Map, Value};

/// One line of the stream.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamEvent {
    System {
        #[serde(default)]
        subtype: Option<String>,
        #[serde(flatten)]
        rest: Map<String, Value>,
    },
    Assistant {
        message: Message,
        #[serde(flatten)]
        rest: Map<String, Value>,
    },
    User {
        #[serde(default)]
        message: Value,
        #[serde(flatten)]
        rest: Map<String, Value>,
    },
    RateLimitEvent {
        #[serde(flatten)]
        rest: Map<String, Value>,
    },
    Result(Box<ResultEvent>),
    #[serde(other)]
    Unknown,
}

/// `assistant.message`: the content blocks and the usage of that turn.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Message {
    pub id: Option<String>,
    pub model: Option<String>,
    pub content: Vec<Value>,
    pub usage: Option<Usage>,
}

/// The five usage fields the `workers` row stores (§10 FR-8).
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Usage {
    pub input_tokens: Option<u64>,
    pub cache_creation_input_tokens: Option<u64>,
    pub cache_read_input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub output_tokens_details: Option<OutputDetails>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct OutputDetails {
    pub thinking_tokens: Option<u64>,
}

impl Usage {
    /// `measure.py:56` — the context the model read for this turn.
    pub fn context(&self) -> u64 {
        self.input_tokens.unwrap_or(0)
            + self.cache_creation_input_tokens.unwrap_or(0)
            + self.cache_read_input_tokens.unwrap_or(0)
    }

    pub fn thinking(&self) -> Option<u64> {
        self.output_tokens_details
            .as_ref()
            .and_then(|d| d.thinking_tokens)
    }
}

/// The final `result` line.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ResultEvent {
    pub subtype: Option<String>,
    pub is_error: bool,
    pub duration_ms: Option<u64>,
    pub duration_api_ms: Option<u64>,
    pub num_turns: Option<u64>,
    pub result: Option<String>,
    pub stop_reason: Option<String>,
    pub terminal_reason: Option<String>,
    pub api_error_status: Option<Value>,
    pub session_id: Option<String>,
    pub total_cost_usd: Option<f64>,
    pub usage: Option<Usage>,
    #[serde(rename = "modelUsage")]
    pub model_usage: Value,
    pub permission_denials: Vec<Value>,
    pub uuid: Option<String>,
    #[serde(flatten)]
    pub rest: Map<String, Value>,
}

/// The `STATUS:` line at the end of `result` — a hint; the board is the truth.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusHint {
    Done,
    Blocked,
    Failed,
}

impl StatusHint {
    pub fn as_str(self) -> &'static str {
        match self {
            StatusHint::Done => "done",
            StatusHint::Blocked => "blocked",
            StatusHint::Failed => "failed",
        }
    }

    /// The first `STATUS:` line of a result text.
    pub fn from_result(text: &str) -> Option<StatusHint> {
        let line = text
            .lines()
            .map(str::trim)
            .find_map(|l| l.strip_prefix("STATUS:"))?;
        let word = line
            .trim()
            .split(|c: char| !c.is_ascii_alphabetic())
            .next()?;
        match word.to_ascii_lowercase().as_str() {
            "done" => Some(StatusHint::Done),
            "blocked" => Some(StatusHint::Blocked),
            "failed" => Some(StatusHint::Failed),
            _ => None,
        }
    }
}

/// One `tool_use` block and the size of its result — what slice 05 (the
/// pathspec) and slice 08 (`tool_calls`) consume.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolUse {
    pub id: String,
    pub name: String,
    pub input: Value,
    /// Characters of the matching `tool_result` content; `None` when the
    /// stream ended before it came back.
    pub result_chars: Option<u64>,
    /// The `timestamp` of the assistant line that issued the call (FR-8).
    pub ts: Option<String>,
    /// The `timestamp` of the user line that carried the result.
    pub result_ts: Option<String>,
    /// The `tool_result` block's `is_error`.
    pub result_error: bool,
    /// The first [`RESULT_HEAD`] characters of the result text, for the
    /// gate verdict (`metrics::is_red`); `result_chars` counts the whole.
    pub result_head: Option<String>,
}

/// How much of a `tool_result` the summary keeps as text.
pub const RESULT_HEAD: usize = 4096;

/// What one worker's stream amounts to.
#[derive(Debug, Clone, Default)]
pub struct Summary {
    /// `false` when no `result` line was seen: the worker died (FR-1).
    pub has_result: bool,
    pub usage: Option<Usage>,
    pub model_usage: Value,
    pub num_turns: Option<u64>,
    pub duration_ms: Option<u64>,
    pub terminal_reason: Option<String>,
    pub is_error: bool,
    pub api_error_status: Option<Value>,
    pub permission_denials: Vec<Value>,
    pub cost_usd: Option<f64>,
    pub session_id: Option<String>,
    pub result_text: Option<String>,
    pub status_hint: Option<StatusHint>,
    pub peak_context: Option<u64>,
    /// The first assistant message's context (`measure.py` start): what the
    /// worker read before it did anything (FR-8 `start_context`).
    pub start_context: Option<u64>,
    pub tool_uses: Vec<ToolUse>,
    /// The last `rate_limit_event` seen (FR-9); `None` when the stream had none.
    pub rate_limit: Option<RateLimit>,
    /// Lines seen, including unknown kinds.
    pub events: usize,
    /// Lines that were not JSON objects; counted, never fatal.
    pub bad_lines: usize,
}

/// `rate_limit_event.rate_limit_info` as FR-9 reads it: the `status` literal
/// (`allowed`, `allowed_warning`, `rejected`, …) and `resetsAt` in epoch
/// seconds when the stream gave one; `utilization_pct` is the window named by
/// `rateLimitType` in `unifiedWindows` (else the fullest one), in percent,
/// for `factory status` (FR-7).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RateLimit {
    pub status: Option<String>,
    pub resets_at: Option<u64>,
    pub utilization_pct: Option<u32>,
}

impl RateLimit {
    fn from_event(rest: &Map<String, Value>) -> RateLimit {
        let info = rest.get("rate_limit_info");
        let field = |k: &str| info.and_then(|i| i.get(k));
        RateLimit {
            status: field("status").and_then(Value::as_str).map(str::to_string),
            resets_at: field("resetsAt").and_then(Value::as_u64),
            utilization_pct: utilization(info),
        }
    }
}

fn utilization(info: Option<&Value>) -> Option<u32> {
    let windows = info?.get("unifiedWindows")?.as_object()?;
    let of = |w: &Value| w.get("utilization").and_then(Value::as_f64);
    let named = info?
        .get("rateLimitType")
        .and_then(Value::as_str)
        .and_then(|k| windows.get(k))
        .and_then(of);
    let u = named.or_else(|| windows.values().filter_map(of).reduce(f64::max))?;
    Some((u * 100.0).round().clamp(0.0, 1000.0) as u32)
}

impl Summary {
    /// The summary of a worker that produced no stream at all.
    pub fn dead() -> Summary {
        Summary::default()
    }

    /// No `result` line: process gone with nothing to show.
    pub fn is_dead(&self) -> bool {
        !self.has_result
    }
}

/// Feeds lines as they arrive; `finish` gives the summary. The tee thread in
/// `worker.rs` owns one; tests feed it a recording.
#[derive(Debug, Default)]
pub struct Collector {
    summary: Summary,
    pending: HashMap<String, usize>,
}

impl Collector {
    pub fn new() -> Collector {
        Collector::default()
    }

    /// One line of the stream. Blank lines are skipped; a line that is not a
    /// JSON object is counted in `bad_lines`.
    pub fn push_line(&mut self, line: &str) -> Option<StreamEvent> {
        if line.trim().is_empty() {
            return None;
        }
        match serde_json::from_str::<StreamEvent>(line) {
            Ok(ev) => {
                self.push(&ev);
                Some(ev)
            }
            Err(_) => {
                self.summary.bad_lines += 1;
                None
            }
        }
    }

    pub fn push(&mut self, ev: &StreamEvent) {
        self.summary.events += 1;
        match ev {
            StreamEvent::Assistant { message, rest } => {
                if let Some(u) = &message.usage {
                    let ctx = u.context();
                    self.summary.peak_context =
                        Some(self.summary.peak_context.map_or(ctx, |p| p.max(ctx)));
                    self.summary.start_context.get_or_insert(ctx);
                }
                let ts = timestamp(rest);
                for block in &message.content {
                    if block.get("type").and_then(Value::as_str) != Some("tool_use") {
                        continue;
                    }
                    let id = block
                        .get("id")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string();
                    self.pending
                        .insert(id.clone(), self.summary.tool_uses.len());
                    self.summary.tool_uses.push(ToolUse {
                        id,
                        name: block
                            .get("name")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_string(),
                        input: block.get("input").cloned().unwrap_or(Value::Null),
                        result_chars: None,
                        ts: ts.clone(),
                        result_ts: None,
                        result_error: false,
                        result_head: None,
                    });
                }
            }
            StreamEvent::User { message, rest } => {
                let Some(blocks) = message.get("content").and_then(Value::as_array) else {
                    return;
                };
                let ts = timestamp(rest);
                for b in blocks {
                    if b.get("type").and_then(Value::as_str) != Some("tool_result") {
                        continue;
                    }
                    let Some(id) = b.get("tool_use_id").and_then(Value::as_str) else {
                        continue;
                    };
                    if let Some(&i) = self.pending.get(id) {
                        let t = &mut self.summary.tool_uses[i];
                        t.result_chars = Some(content_chars(b.get("content")));
                        t.result_ts = ts.clone();
                        t.result_error = b.get("is_error").and_then(Value::as_bool) == Some(true);
                        t.result_head = Some(content_head(b.get("content")));
                    }
                }
            }
            StreamEvent::Result(r) => {
                let s = &mut self.summary;
                s.has_result = true;
                s.usage = r.usage.clone();
                s.model_usage = r.model_usage.clone();
                s.num_turns = r.num_turns;
                s.duration_ms = r.duration_ms;
                s.terminal_reason = r.terminal_reason.clone();
                s.is_error = r.is_error;
                s.api_error_status = r.api_error_status.clone();
                s.permission_denials = r.permission_denials.clone();
                s.cost_usd = r.total_cost_usd;
                s.session_id = r.session_id.clone();
                s.result_text = r.result.clone();
                s.status_hint = r.result.as_deref().and_then(StatusHint::from_result);
            }
            StreamEvent::RateLimitEvent { rest } => {
                self.summary.rate_limit = Some(RateLimit::from_event(rest));
            }
            StreamEvent::System { .. } | StreamEvent::Unknown => {}
        }
    }

    pub fn summary(&self) -> &Summary {
        &self.summary
    }

    pub fn finish(self) -> Summary {
        self.summary
    }
}

/// A line's `timestamp`, when the stream carries one (`--verbose` does).
fn timestamp(rest: &Map<String, Value>) -> Option<String> {
    rest.get("timestamp")
        .and_then(Value::as_str)
        .map(str::to_string)
}

/// The first [`RESULT_HEAD`] characters of a `tool_result` content's text.
fn content_head(content: Option<&Value>) -> String {
    let text = match content {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(blocks)) => blocks
            .iter()
            .map(|b| match b.get("text") {
                Some(Value::String(s)) => s.clone(),
                _ => b.to_string(),
            })
            .collect::<Vec<_>>()
            .join("\n"),
        Some(other) => other.to_string(),
        None => String::new(),
    };
    text.chars().take(RESULT_HEAD).collect()
}

/// Characters of a `tool_result` content: a string, or text blocks.
pub fn content_chars(content: Option<&Value>) -> u64 {
    match content {
        Some(Value::String(s)) => s.chars().count() as u64,
        Some(Value::Array(blocks)) => blocks
            .iter()
            .map(|b| match b.get("text") {
                Some(Value::String(s)) => s.chars().count() as u64,
                _ => b.to_string().chars().count() as u64,
            })
            .sum(),
        Some(other) => other.to_string().chars().count() as u64,
        None => 0,
    }
}

/// A whole stream, as a recording or a `.jsonl` left by a dead worker.
pub fn parse(text: &str) -> (Vec<StreamEvent>, Summary) {
    let mut c = Collector::new();
    let events = text.lines().filter_map(|l| c.push_line(l)).collect();
    (events, c.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rate_limit_event_carries_the_named_windows_utilization() {
        let line =
            |info: &str| format!(r#"{{"type":"rate_limit_event","rate_limit_info":{info}}}"#);
        let (_, s) = parse(&line(
            r#"{"status":"allowed","resetsAt":1791592200,"rateLimitType":"five_hour","unifiedWindows":{"five_hour":{"utilization":0.21},"seven_day":{"utilization":0.5}}}"#,
        ));
        let rl = s.rate_limit.unwrap();
        assert_eq!(rl.status.as_deref(), Some("allowed"));
        assert_eq!(rl.resets_at, Some(1_791_592_200));
        assert_eq!(rl.utilization_pct, Some(21));
        let (_, s) = parse(&line(
            r#"{"status":"allowed","unifiedWindows":{"a":{"utilization":0.3},"b":{"utilization":0.04}}}"#,
        ));
        assert_eq!(s.rate_limit.unwrap().utilization_pct, Some(30));
        let (_, s) = parse(&line(r#"{"status":"rejected"}"#));
        assert_eq!(s.rate_limit.unwrap().utilization_pct, None);
    }
}
