//! Unknown line kinds and fields are kept as `Value`, so a Claude Code upgrade never panics the parser.
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::provider::Provider;

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

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Message {
    pub id: Option<String>,
    pub model: Option<String>,
    pub content: Vec<Value>,
    pub usage: Option<Usage>,
}

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

/// A hint the row keeps; the driver routes on the board. A build worker's `done` on a `building`
/// slice is the one hint that writes it: `done` when the lane passes `docs check`, `blocked` when it fails.
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

/// The polish worker's `FINDINGS` counts: a trend, not an audit.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct Findings {
    pub high: u32,
    pub medium: u32,
    pub low: u32,
    pub fixed: u32,
    pub parked: u32,
}

impl Findings {
    pub fn from_result(text: &str) -> Option<Findings> {
        text.lines()
            .map(str::trim)
            .filter_map(|l| l.strip_prefix("FINDINGS"))
            .find_map(Findings::from_pairs)
    }

    fn from_pairs(rest: &str) -> Option<Findings> {
        let rest = rest.strip_prefix(':').unwrap_or(rest);
        let mut f = [None; 5];
        for (k, v) in rest
            .split(|c: char| c.is_whitespace() || c == ',')
            .filter_map(|p| p.split_once('='))
        {
            let i = match k {
                "high" => 0,
                "medium" => 1,
                "low" => 2,
                "fixed" => 3,
                "parked" => 4,
                _ => continue,
            };
            f[i] = Some(v.parse::<u32>().ok()?);
        }
        Some(Findings {
            high: f[0]?,
            medium: f[1]?,
            low: f[2]?,
            fixed: f[3]?,
            parked: f[4]?,
        })
    }

    pub fn text(&self) -> String {
        format!(
            "high={} medium={} low={} fixed={} parked={}",
            self.high, self.medium, self.low, self.fixed, self.parked
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolUse {
    pub id: String,
    pub name: String,
    pub input: Value,
    pub result_chars: Option<u64>,
    pub ts: Option<String>,
    pub result_ts: Option<String>,
    pub result_error: bool,
    /// What the gate verdict (`metrics::is_red`) reads; `result_chars` counts the whole.
    pub result_head: Option<String>,
    pub in_subagent: bool,
}

pub const RESULT_HEAD: usize = 4096;

/// `label_followup` reads the paths a `factory ctx` answer names, and a def with bodies runs past 4096.
pub const CTX_RESULT_HEAD: usize = 65536;

fn is_ctx_call(t: &ToolUse) -> bool {
    if t.name != "Bash" {
        return false;
    }
    let cmd = t.input.get("command").and_then(Value::as_str).unwrap_or("");
    let (words, _) = crate::shell::first_command(cmd);
    words.len() > 1 && crate::shell::program(&words[0]) == "factory" && words[1] == "ctx"
}

#[derive(Debug, Clone, Default)]
pub struct Summary {
    pub has_result: bool,
    pub usage: Option<Usage>,
    pub model_usage: Value,
    /// Counted here: the `result`'s own `num_turns` mixes in subagent turns it cannot split.
    pub num_turns: Option<u64>,
    pub duration_ms: Option<u64>,
    pub terminal_reason: Option<String>,
    pub is_error: bool,
    pub api_error_status: Option<Value>,
    pub permission_denials: Vec<Value>,
    pub cost_usd: Option<f64>,
    pub session_id: Option<String>,
    pub cwd: Option<PathBuf>,
    pub result_text: Option<String>,
    pub status_hint: Option<StatusHint>,
    pub peak_context: Option<u64>,
    pub start_context: Option<u64>,
    /// The model that answered, not the one `--model` asked for.
    pub model_id: Option<String>,
    pub tool_uses: Vec<ToolUse>,
    pub rate_limit: Option<RateLimit>,
    pub events: usize,
    pub bad_lines: usize,
}

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
    pub fn dead() -> Summary {
        Summary::default()
    }

    pub fn is_dead(&self) -> bool {
        !self.has_result
    }
}

#[derive(Debug, Default)]
pub struct Collector {
    summary: Summary,
    pending: HashMap<String, usize>,
    own_turns: HashSet<String>,
    anonymous_turns: u64,
}

impl Collector {
    pub fn new() -> Collector {
        Collector::default()
    }

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
                let in_subagent = in_subagent(rest);
                // Claude Code writes `<synthetic>` on a message no model answered (an API error, an interrupt).
                if !in_subagent
                    && self.summary.model_id.is_none()
                    && message.model.as_deref() != Some("<synthetic>")
                {
                    self.summary.model_id = message.model.clone();
                }
                if let (Some(u), false) = (&message.usage, in_subagent) {
                    let ctx = u.context();
                    self.summary.peak_context =
                        Some(self.summary.peak_context.map_or(ctx, |p| p.max(ctx)));
                    self.summary.start_context.get_or_insert(ctx);
                    match &message.id {
                        Some(id) => {
                            self.own_turns.insert(id.clone());
                        }
                        None => self.anonymous_turns += 1,
                    }
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
                        in_subagent,
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
                        let head = if is_ctx_call(t) {
                            CTX_RESULT_HEAD
                        } else {
                            RESULT_HEAD
                        };
                        t.result_head = Some(content_head(b.get("content"), head));
                    }
                }
            }
            StreamEvent::Result(r) => {
                let s = &mut self.summary;
                s.has_result = true;
                s.usage = r.usage.clone();
                s.model_usage = r.model_usage.clone();
                s.num_turns = Some(self.own_turns.len() as u64 + self.anonymous_turns);
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
            StreamEvent::System { subtype, rest } => {
                if subtype.as_deref() == Some("init") && self.summary.cwd.is_none() {
                    self.summary.cwd = rest.get("cwd").and_then(Value::as_str).map(PathBuf::from);
                }
            }
            StreamEvent::Unknown => {}
        }
    }

    pub fn summary(&self) -> &Summary {
        &self.summary
    }

    pub fn finish(self) -> Summary {
        self.summary
    }
}

fn in_subagent(rest: &Map<String, Value>) -> bool {
    rest.get("parent_tool_use_id")
        .and_then(Value::as_str)
        .is_some()
}

/// Only `--verbose` streams carry a `timestamp`.
fn timestamp(rest: &Map<String, Value>) -> Option<String> {
    rest.get("timestamp")
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn content_head(content: Option<&Value>, head: usize) -> String {
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
    text.chars().take(head).collect()
}

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

pub fn parse(text: &str) -> (Vec<StreamEvent>, Summary) {
    parse_for(Provider::Claude, text)
}

/// Every provider's stream lands in the same `Summary`; the events are each line as a Claude stream would read it.
pub fn parse_for(provider: Provider, text: &str) -> (Vec<StreamEvent>, Summary) {
    let read: fn(&mut Norm, &Value, &StreamEvent) = match provider {
        Provider::Claude => {
            let mut c = Collector::new();
            let events = text.lines().filter_map(|l| c.push_line(l)).collect();
            return (events, c.finish());
        }
        Provider::Codex => Norm::codex,
        Provider::Cursor => Norm::cursor,
        Provider::Agy => Norm::agy,
        Provider::Opencode => Norm::opencode,
    };
    let mut n = Norm {
        claude: (provider == Provider::Agy).then(Collector::new),
        ..Norm::default()
    };
    let (mut events, mut count, mut bad) = (Vec::new(), 0, 0);
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let v = match serde_json::from_str::<Value>(line) {
            Ok(v) if v.is_object() => v,
            _ => {
                bad += 1;
                continue;
            }
        };
        count += 1;
        let ev = serde_json::from_value::<StreamEvent>(v.clone()).unwrap_or(StreamEvent::Unknown);
        read(&mut n, &v, &ev);
        events.push(ev);
    }
    let mut s = n.finish();
    s.events = count;
    s.bad_lines = bad;
    (events, s)
}

/// A 429, a rate limit or a usage limit in an error is a rejection `quota::Limit::from` pauses on.
pub fn detect_rate_limit(msg: &str, resets_at: Option<u64>) -> Option<RateLimit> {
    let m = msg.to_ascii_lowercase();
    let named = [
        "rate limit",
        "rate_limit",
        "rate-limit",
        "ratelimit",
        "usage limit",
        "usage_limit",
        "too many requests",
    ]
    .iter()
    .any(|p| m.contains(p));
    let b = m.as_bytes();
    // `src/lib.rs:429:10` is a line number, not a status.
    let status = m.match_indices("429").any(|(i, _)| {
        !(i > 0 && (b[i - 1].is_ascii_digit() || b[i - 1] == b':'))
            && !b.get(i + 3).is_some_and(u8::is_ascii_digit)
    });
    (named || status).then(|| RateLimit {
        status: Some("rejected".into()),
        resets_at,
        utilization_pct: None,
    })
}

/// A 429 in a status field anywhere in an error event; other numbers, such as a duration, never count.
fn status_429(v: &Value) -> bool {
    match v {
        Value::Object(m) => m.iter().any(|(k, v)| {
            (matches!(
                k.as_str(),
                "status" | "statusCode" | "status_code" | "code" | "httpStatus" | "http_status"
            ) && (v.as_u64() == Some(429) || v.as_str() == Some("429")))
                || status_429(v)
        }),
        Value::Array(a) => a.iter().any(status_429),
        _ => false,
    }
}

fn resets_at(v: &Value) -> Option<u64> {
    let Value::Object(m) = v else {
        return None;
    };
    ["resets_at", "resetsAt", "reset_at", "resetAt"]
        .iter()
        .find_map(|k| m.get(*k).and_then(Value::as_u64))
        .or_else(|| m.values().find_map(resets_at))
}

fn str_at<'a>(v: &'a Value, pointers: &[&str]) -> Option<&'a str> {
    pointers
        .iter()
        .find_map(|p| v.pointer(p).and_then(Value::as_str))
}

fn u64_at(v: &Value, pointer: &str) -> Option<u64> {
    v.pointer(pointer).and_then(Value::as_u64)
}

fn add(a: Option<u64>, b: Option<u64>) -> Option<u64> {
    match (a, b) {
        (None, None) => None,
        _ => Some(a.unwrap_or(0).saturating_add(b.unwrap_or(0))),
    }
}

fn add_usage(into: &mut Usage, u: &Usage) {
    into.input_tokens = add(into.input_tokens, u.input_tokens);
    into.cache_creation_input_tokens = add(
        into.cache_creation_input_tokens,
        u.cache_creation_input_tokens,
    );
    into.cache_read_input_tokens = add(into.cache_read_input_tokens, u.cache_read_input_tokens);
    into.output_tokens = add(into.output_tokens, u.output_tokens);
    if let Some(t) = u.thinking() {
        let d = into
            .output_tokens_details
            .get_or_insert_with(Default::default);
        d.thinking_tokens = add(d.thinking_tokens, Some(t));
    }
}

/// A usage object in either casing; `None` when it names no tokens at all.
fn usage_of(u: &Value) -> Option<Usage> {
    let pick = |keys: &[&str]| keys.iter().find_map(|k| u.get(*k).and_then(Value::as_u64));
    let usage = Usage {
        input_tokens: pick(&["input_tokens", "inputTokens"]),
        cache_creation_input_tokens: pick(&[
            "cache_creation_input_tokens",
            "cacheWriteTokens",
            "cache_write_tokens",
        ]),
        cache_read_input_tokens: pick(&[
            "cache_read_input_tokens",
            "cacheReadTokens",
            "cache_read_tokens",
        ]),
        output_tokens: pick(&["output_tokens", "outputTokens"]),
        output_tokens_details: None,
    };
    (usage != Usage::default()).then_some(usage)
}

/// File tools carry the path where `metrics` reads a Claude one.
fn claude_input(name: &str, mut input: Value) -> Value {
    if matches!(name, "Read" | "Edit" | "Write")
        && let Value::Object(m) = &mut input
        && !m.contains_key("file_path")
        && let Some(p) = ["filePath", "path"].iter().find_map(|k| m.get(*k).cloned())
    {
        m.insert("file_path".into(), p);
    }
    input
}

fn capitalized(name: &str) -> String {
    let mut c = name.chars();
    c.next()
        .map(|f| f.to_uppercase().chain(c).collect())
        .unwrap_or_default()
}

#[derive(Default)]
struct Norm {
    s: Summary,
    /// The Claude-shaped stream's own reader, for a provider that writes one.
    claude: Option<Collector>,
    turns: u64,
    /// The assistant text since the last tool call or step: the final message, never an earlier turn's narration.
    turn: Vec<(Option<String>, String)>,
    turn_closed: bool,
    final_text: Option<String>,
    error: Option<String>,
    pending: HashMap<String, usize>,
}

impl Norm {
    fn say(&mut self, id: Option<&str>, text: &str) {
        if text.trim().is_empty() {
            return;
        }
        if self.turn_closed {
            self.turn.clear();
            self.turn_closed = false;
        }
        match id.and_then(|id| self.turn.iter_mut().find(|(k, _)| k.as_deref() == Some(id))) {
            Some(slot) => slot.1 = text.to_string(),
            None => self.turn.push((id.map(str::to_string), text.to_string())),
        }
    }

    /// The id its result is matched on: a stream that names none gets one per call.
    fn tool(&mut self, id: &str, name: &str, input: Value) -> String {
        self.turn_closed = true;
        let id = match id {
            "" => format!("call-{}", self.s.tool_uses.len()),
            id => id.to_string(),
        };
        if let Some(&i) = self.pending.get(&id) {
            let named = match &input {
                Value::Object(m) => !m.is_empty(),
                v => !v.is_null(),
            };
            if named {
                self.s.tool_uses[i].input = claude_input(name, input);
            }
            return id;
        }
        self.pending.insert(id.clone(), self.s.tool_uses.len());
        self.s.tool_uses.push(ToolUse {
            id: id.clone(),
            input: claude_input(name, input),
            name: name.to_string(),
            result_chars: None,
            ts: None,
            result_ts: None,
            result_error: false,
            result_head: None,
            in_subagent: false,
        });
        id
    }

    fn tool_result(&mut self, id: &str, output: &Value, error: bool) {
        let Some(&i) = self.pending.get(id) else {
            return;
        };
        let t = &mut self.s.tool_uses[i];
        t.result_chars = Some(content_chars(Some(output)));
        t.result_error = error;
        let head = if is_ctx_call(t) {
            CTX_RESULT_HEAD
        } else {
            RESULT_HEAD
        };
        t.result_head = Some(content_head(Some(output), head));
    }

    fn step(&mut self, u: &Usage, context: bool) {
        self.turns += 1;
        self.turn_closed = true;
        add_usage(self.s.usage.get_or_insert_with(Default::default), u);
        if context {
            let ctx = u.context();
            self.s.peak_context = Some(self.s.peak_context.map_or(ctx, |p| p.max(ctx)));
            self.s.start_context.get_or_insert(ctx);
        }
    }

    /// A failure ends the worker the way a Claude error result does: its message, not earlier narration, is the result.
    fn fail(&mut self, msg: &str, event: &Value) {
        self.s.is_error = true;
        self.s.has_result = true;
        self.turn.clear();
        self.final_text = None;
        if !msg.trim().is_empty() {
            self.error = Some(msg.to_string());
        }
        let limit = detect_rate_limit(msg, resets_at(event)).or_else(|| {
            status_429(event).then(|| RateLimit {
                status: Some("rejected".into()),
                resets_at: resets_at(event),
                utilization_pct: None,
            })
        });
        if let Some(rl) = limit {
            self.s.rate_limit = Some(rl);
        }
    }

    fn init(&mut self, v: &Value) {
        if self.s.session_id.is_none() {
            self.s.session_id =
                str_at(v, &["/session_id", "/sessionID", "/thread_id"]).map(str::to_string);
        }
        if self.s.cwd.is_none() {
            self.s.cwd = str_at(v, &["/cwd"]).map(PathBuf::from);
        }
        if self.s.model_id.is_none() {
            self.s.model_id = str_at(v, &["/model"]).map(str::to_string);
        }
    }

    fn codex(&mut self, v: &Value, _: &StreamEvent) {
        match str_at(v, &["/type"]).unwrap_or("") {
            "thread.started" => self.init(v),
            "item.completed" => {
                let item = &v["item"];
                let id = str_at(item, &["/id"]).unwrap_or("");
                match str_at(item, &["/type"]).unwrap_or("") {
                    "agent_message" => {
                        // Each message is whole: the last one is the worker's answer.
                        self.turn_closed = true;
                        self.say(None, str_at(item, &["/text"]).unwrap_or(""));
                    }
                    "command_execution" => {
                        let cmd = str_at(item, &["/command"]).unwrap_or("");
                        let id = self.tool(id, "Bash", serde_json::json!({ "command": cmd }));
                        let failed = item["exit_code"].as_i64().is_some_and(|c| c != 0)
                            || str_at(item, &["/status"]) == Some("failed");
                        let out = item.get("aggregated_output").unwrap_or(&Value::Null);
                        self.tool_result(&id, out, failed);
                    }
                    "file_change" => {
                        let failed = str_at(item, &["/status"]) == Some("failed");
                        let changes = item["changes"].as_array().cloned().unwrap_or_default();
                        for (n, change) in changes.iter().enumerate() {
                            let path = str_at(change, &["/path"]).unwrap_or("");
                            let input = serde_json::json!({ "file_path": path, "change": change });
                            let call = match n {
                                0 => id.to_string(),
                                n => format!("{id}#{n}"),
                            };
                            let call = self.tool(&call, "Edit", input);
                            self.tool_result(&call, &Value::Null, failed);
                        }
                    }
                    _ => {}
                }
            }
            "turn.completed" => {
                let cached = u64_at(v, "/usage/cached_input_tokens");
                let u = Usage {
                    // An OpenAI input count includes its cached part; Claude's does not.
                    input_tokens: u64_at(v, "/usage/input_tokens")
                        .map(|i| i.saturating_sub(cached.unwrap_or(0))),
                    cache_read_input_tokens: cached,
                    output_tokens: u64_at(v, "/usage/output_tokens"),
                    ..Usage::default()
                };
                self.step(&u, false);
                // A completed turn outlived any error before it, such as a retried disconnect.
                self.s.has_result = true;
                self.s.is_error = false;
                self.s.rate_limit = None;
                self.error = None;
            }
            "turn.failed" => {
                let msg = str_at(v, &["/error/message", "/message"]).unwrap_or("turn failed");
                self.fail(msg, v);
            }
            "error" => {
                let msg = str_at(v, &["/message", "/error/message"]).unwrap_or("error");
                self.fail(msg, v);
            }
            _ => {}
        }
    }

    fn content_blocks(&mut self, v: &Value) {
        let Some(blocks) = v.pointer("/message/content").and_then(Value::as_array) else {
            if let Some(text) = str_at(v, &["/message/content"]) {
                self.say(None, text);
            }
            return;
        };
        for b in blocks {
            match str_at(b, &["/type"]).unwrap_or("") {
                "text" => self.say(None, str_at(b, &["/text"]).unwrap_or("")),
                "tool_use" => {
                    let id = str_at(b, &["/id"]).unwrap_or("");
                    let name = str_at(b, &["/name"]).unwrap_or("");
                    let input = b.get("input").cloned().unwrap_or(Value::Null);
                    self.tool(id, name, input);
                }
                _ => {}
            }
        }
    }

    fn cursor(&mut self, v: &Value, _: &StreamEvent) {
        match str_at(v, &["/type"]).unwrap_or("") {
            "system" => self.init(v),
            "assistant" => self.content_blocks(v),
            "user" => self.tool_results(v),
            "tool_call" if str_at(v, &["/subtype"]) == Some("completed") => {
                let id = str_at(v, &["/call_id", "/id"]).unwrap_or("");
                let Some((key, call)) = v
                    .get("tool_call")
                    .and_then(Value::as_object)
                    .and_then(|m| m.iter().next())
                else {
                    return;
                };
                let (name, args) = if key == "function" {
                    let args = str_at(call, &["/arguments"])
                        .and_then(|a| serde_json::from_str(a).ok())
                        .unwrap_or(Value::Null);
                    (str_at(call, &["/name"]).unwrap_or("").to_string(), args)
                } else {
                    let short = key.strip_suffix("ToolCall").unwrap_or(key);
                    let name = match short {
                        "shell" => "Bash".to_string(),
                        "ls" => "LS".to_string(),
                        other => capitalized(other),
                    };
                    (name, call.get("args").cloned().unwrap_or(Value::Null))
                };
                let id = self.tool(id, &name, args);
                let result = call.get("result").unwrap_or(&Value::Null);
                let failed = ["error", "failure", "rejected"]
                    .iter()
                    .any(|k| result.get(*k).is_some())
                    || result
                        .pointer("/success/exitCode")
                        .and_then(Value::as_i64)
                        .is_some_and(|c| c != 0);
                let out = ["/success/stdout", "/success/content", "/success/output"]
                    .iter()
                    .find_map(|p| result.pointer(p))
                    .unwrap_or(result);
                self.tool_result(&id, out, failed);
            }
            "result" => {
                self.s.has_result = true;
                self.init(v);
                self.s.duration_ms = u64_at(v, "/duration_ms");
                if let Some(u) = v.get("usage").and_then(usage_of) {
                    self.s.usage = Some(u);
                }
                let text = str_at(v, &["/result"]).unwrap_or("");
                if !text.trim().is_empty() {
                    self.final_text = Some(text.to_string());
                }
                if v["is_error"].as_bool() == Some(true) {
                    self.fail(text, v);
                }
            }
            "error" => {
                let msg = str_at(v, &["/message", "/error/message", "/error"]).unwrap_or("error");
                self.fail(msg, v);
            }
            _ => {}
        }
    }

    fn tool_results(&mut self, v: &Value) {
        let Some(blocks) = v.pointer("/message/content").and_then(Value::as_array) else {
            return;
        };
        for b in blocks {
            if str_at(b, &["/type"]) != Some("tool_result") {
                continue;
            }
            let id = str_at(b, &["/tool_use_id"]).unwrap_or("");
            let out = b.get("content").unwrap_or(&Value::Null);
            self.tool_result(id, out, b["is_error"].as_bool() == Some(true));
        }
    }

    fn agy(&mut self, v: &Value, ev: &StreamEvent) {
        if let Some(c) = &mut self.claude {
            c.push(ev);
        }
        match str_at(v, &["/type"]).unwrap_or("") {
            "assistant" => {
                for b in v
                    .pointer("/message/content")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    match str_at(b, &["/type"]) {
                        Some("text") => self.say(None, str_at(b, &["/text"]).unwrap_or("")),
                        Some("tool_use") => self.turn_closed = true,
                        _ => {}
                    }
                }
            }
            "result" => {
                self.s.cost_usd = v["cost_usd"].as_f64();
                if v["is_error"].as_bool() == Some(true) {
                    let msg = str_at(v, &["/result", "/error/message", "/subtype"]).unwrap_or("");
                    self.fail(msg, v);
                }
            }
            "error" => {
                let msg = str_at(v, &["/message", "/error/message", "/error"]).unwrap_or("error");
                self.fail(msg, v);
            }
            _ => {}
        }
    }

    fn opencode(&mut self, v: &Value, _: &StreamEvent) {
        let part = &v["part"];
        match str_at(v, &["/type"]).unwrap_or("") {
            "text" => {
                if self.s.session_id.is_none() {
                    self.init(v);
                }
                self.say(
                    str_at(part, &["/id"]),
                    str_at(part, &["/text"]).unwrap_or(""),
                );
            }
            "tool_use" => {
                let id = str_at(part, &["/callID", "/id"]).unwrap_or("");
                let name = capitalized(str_at(part, &["/tool"]).unwrap_or(""));
                let input = part.pointer("/state/input").cloned().unwrap_or(Value::Null);
                let id = self.tool(id, &name, input);
                let failed = str_at(part, &["/state/status"]) == Some("error");
                let out = ["/state/output", "/state/error"]
                    .iter()
                    .find_map(|p| part.pointer(p))
                    .unwrap_or(&Value::Null);
                self.tool_result(&id, out, failed);
            }
            "step_finish" => {
                let reasoning = u64_at(part, "/tokens/reasoning");
                let u = Usage {
                    input_tokens: u64_at(part, "/tokens/input"),
                    cache_creation_input_tokens: u64_at(part, "/tokens/cache/write"),
                    cache_read_input_tokens: u64_at(part, "/tokens/cache/read"),
                    // Reasoning rides inside output, as a Claude stream counts thinking.
                    output_tokens: add(u64_at(part, "/tokens/output"), reasoning),
                    output_tokens_details: reasoning.map(|r| OutputDetails {
                        thinking_tokens: Some(r),
                    }),
                };
                self.step(&u, true);
                if str_at(part, &["/reason"]) != Some("tool-calls") {
                    self.s.has_result = true;
                }
                if let Some(c) = part["cost"].as_f64() {
                    self.s.cost_usd = Some(self.s.cost_usd.unwrap_or(0.0) + c);
                }
            }
            "error" => {
                let msg = str_at(
                    v,
                    &[
                        "/error/message",
                        "/error/data/message",
                        "/data/message",
                        "/message",
                    ],
                )
                .unwrap_or("error");
                self.fail(msg, v);
            }
            _ => {}
        }
    }

    fn finish(mut self) -> Summary {
        let mut s = match self.claude.take() {
            Some(c) => {
                let mut s = c.finish();
                s.is_error |= self.s.is_error;
                s.has_result |= self.s.has_result;
                s.cost_usd = s.cost_usd.or(self.s.cost_usd);
                if let Some(mut rl) = self.s.rate_limit.take() {
                    rl.resets_at = rl
                        .resets_at
                        .or(s.rate_limit.as_ref().and_then(|r| r.resets_at));
                    s.rate_limit = Some(rl);
                }
                s
            }
            None => {
                let mut s = std::mem::take(&mut self.s);
                s.num_turns = (self.turns > 0).then_some(self.turns);
                s
            }
        };
        let said = (!self.turn.is_empty()).then(|| {
            self.turn
                .iter()
                .map(|(_, t)| t.as_str())
                .collect::<Vec<_>>()
                .join("\n")
        });
        s.result_text = s
            .result_text
            .filter(|t| !t.trim().is_empty())
            .or(self.final_text)
            .or(said)
            .or(self.error);
        s.status_hint = s.result_text.as_deref().and_then(StatusHint::from_result);
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_init_line_gives_the_worker_cwd() {
        let (_, s) = parse(concat!(
            r#"{"type":"system","subtype":"init","cwd":"/x/lane-2","session_id":"s"}"#,
            "\n",
            r#"{"type":"system","subtype":"init","cwd":"/x/other"}"#,
        ));
        assert_eq!(s.cwd, Some(PathBuf::from("/x/lane-2")));
        let (_, s) = parse(r#"{"type":"system","subtype":"init","session_id":"s"}"#);
        assert_eq!(s.cwd, None);
    }

    #[test]
    fn the_findings_line_needs_all_five_keys_colon_or_not() {
        let f = Findings::from_result(
            "STATUS: done\nFINDINGS high=1 medium=2 low=3 fixed=4 parked=2\nBLOCK: none",
        );
        assert_eq!(
            f,
            Some(Findings {
                high: 1,
                medium: 2,
                low: 3,
                fixed: 4,
                parked: 2
            })
        );
        assert_eq!(
            Findings::from_result("FINDINGS high=1 medium=2 low=3 fixed=4"),
            None
        );
        let f = Findings::from_result("FINDINGS: parked=0 high=0 medium=5 low=0 fixed=5").unwrap();
        assert_eq!((f.high, f.medium, f.fixed), (0, 5, 5));
        assert_eq!(Findings::from_result("STATUS: done\nBLOCK: none"), None);
        assert_eq!(f.text(), "high=0 medium=5 low=0 fixed=5 parked=0");
        let f = Findings::from_result(
            "FINDINGS below, after dedupe\nFINDINGS: high=1, medium=2, low=0, fixed=3, parked=0",
        );
        assert_eq!(f.map(|f| (f.high, f.medium, f.fixed)), Some((1, 2, 3)));
    }

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

    #[test]
    fn subagent_lines_stay_out_of_the_context_and_the_turns() {
        let assistant = |id: &str, parent: &str, cache_read: u64, tool: &str| {
            format!(
                r#"{{"type":"assistant","parent_tool_use_id":{parent},"message":{{"id":"{id}","content":[{{"type":"tool_use","id":"{tool}","name":"Read","input":{{}}}}],"usage":{{"input_tokens":1,"cache_creation_input_tokens":0,"cache_read_input_tokens":{cache_read},"output_tokens":5}}}}}}"#
            )
        };
        let text = [
            assistant("m1", "null", 100, "t1"),
            assistant("m2", r#""toolu_x""#, 9000, "t2"),
            r#"{"type":"user","parent_tool_use_id":"toolu_x","message":{"content":[{"type":"tool_result","tool_use_id":"t2","content":"abc"}]}}"#.to_string(),
            assistant("m3", "null", 300, "t3"),
            r#"{"type":"result","num_turns":9}"#.to_string(),
        ]
        .join("\n");
        let (_, s) = parse(&text);
        assert_eq!(s.peak_context, Some(301));
        assert_eq!(s.start_context, Some(101));
        assert_eq!(s.num_turns, Some(2));
        let flags: Vec<_> = s.tool_uses.iter().map(|t| t.in_subagent).collect();
        assert_eq!(flags, [false, true, false]);
        assert_eq!(s.tool_uses[1].result_chars, Some(3));
    }

    #[test]
    fn model_id_is_the_first_own_assistant_lines_model() {
        let line = |parent: &str, model: &str| {
            format!(
                r#"{{"type":"assistant","parent_tool_use_id":{parent},"message":{{"model":"{model}","content":[]}}}}"#
            )
        };
        let text = [
            line(r#""toolu_x""#, "claude-sonnet-x"),
            line("null", "<synthetic>"),
            line("null", "claude-opus-x"),
            line("null", "claude-haiku-x"),
        ]
        .join("\n");
        let (_, s) = parse(&text);
        assert_eq!(s.model_id.as_deref(), Some("claude-opus-x"));
        assert_eq!(parse("").1.model_id, None);
    }

    const FINAL: &str =
        "Built it.\nSTATUS: done\nFINDINGS: high=1 medium=0 low=2 fixed=1 parked=0\nBLOCK: none";

    fn fixture(provider: Provider, last: &str) -> String {
        let last = serde_json::to_string(last).unwrap();
        let early =
            serde_json::to_string("Looking around.\nSTATUS: failed if the build breaks").unwrap();
        let lines = match provider {
            Provider::Claude => vec![
                r#"{"type":"system","subtype":"init","cwd":"/x/lane-2","session_id":"s1"}"#.to_string(),
                r#"{"type":"assistant","message":{"id":"m1","model":"claude-x","content":[{"type":"text","text":"Running tests"},{"type":"tool_use","id":"t1","name":"Bash","input":{"command":"cargo test"}}],"usage":{"input_tokens":10,"output_tokens":5}}}"#.to_string(),
                r#"{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"t1","content":"ok"}]}}"#.to_string(),
                format!(r#"{{"type":"result","is_error":false,"result":{last},"usage":{{"input_tokens":10,"cache_read_input_tokens":100,"output_tokens":5}}}}"#),
            ],
            Provider::Codex => vec![
                r#"{"type":"thread.started","thread_id":"th1"}"#.to_string(),
                r#"{"type":"turn.started"}"#.to_string(),
                format!(r#"{{"type":"item.completed","item":{{"id":"i0","type":"agent_message","text":{early}}}}}"#),
                r#"{"type":"item.started","item":{"id":"i1","type":"command_execution","command":"cargo test","status":"in_progress"}}"#.to_string(),
                r#"{"type":"item.completed","item":{"id":"i1","type":"command_execution","command":"cargo test","aggregated_output":"test result: ok","exit_code":0,"status":"completed"}}"#.to_string(),
                r#"{"type":"item.completed","item":{"id":"i2","type":"file_change","changes":[{"path":"src/a.rs","kind":"update"}],"status":"completed"}}"#.to_string(),
                r#"{"type":"item.completed","item":{"id":"i3","type":"reasoning","text":"thinking"}}"#.to_string(),
                format!(r#"{{"type":"item.completed","item":{{"id":"i4","type":"agent_message","text":{last}}}}}"#),
                r#"{"type":"turn.completed","usage":{"input_tokens":1000,"cached_input_tokens":800,"output_tokens":50}}"#.to_string(),
            ],
            Provider::Cursor => vec![
                r#"{"type":"system","subtype":"init","cwd":"/x/lane-2","session_id":"c1","model":"gpt-5"}"#.to_string(),
                format!(r#"{{"type":"assistant","message":{{"role":"assistant","content":[{{"type":"text","text":{early}}}]}}}}"#),
                r#"{"type":"tool_call","subtype":"started","call_id":"c-1","tool_call":{"shellToolCall":{"args":{"command":"cargo test"}}}}"#.to_string(),
                r#"{"type":"tool_call","subtype":"completed","call_id":"c-1","tool_call":{"shellToolCall":{"args":{"command":"cargo test"},"result":{"success":{"stdout":"test result: ok","exitCode":0}}}}}"#.to_string(),
                r#"{"type":"tool_call","subtype":"completed","call_id":"c-2","tool_call":{"editToolCall":{"args":{"path":"src/a.rs"},"result":{"success":{}}}}}"#.to_string(),
                format!(r#"{{"type":"assistant","message":{{"role":"assistant","content":[{{"type":"text","text":{last}}}]}}}}"#),
                format!(r#"{{"type":"result","subtype":"success","is_error":false,"duration_ms":1200,"result":{last},"session_id":"c1","usage":{{"inputTokens":40,"outputTokens":7,"cacheReadTokens":300}}}}"#),
            ],
            Provider::Agy => vec![
                r#"{"type":"system","subtype":"init","cwd":"/x/lane-2","session_id":"a1"}"#.to_string(),
                format!(r#"{{"type":"assistant","message":{{"id":"m1","model":"gemini-x","content":[{{"type":"text","text":{early}}},{{"type":"tool_use","id":"t1","name":"Bash","input":{{"command":"cargo test"}}}}],"usage":{{"input_tokens":10,"output_tokens":5}}}}}}"#),
                r#"{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"t1","content":"test result: ok"}]}}"#.to_string(),
                format!(r#"{{"type":"assistant","message":{{"id":"m2","model":"gemini-x","content":[{{"type":"text","text":{last}}}],"usage":{{"input_tokens":12,"output_tokens":9}}}}}}"#),
                format!(r#"{{"type":"result","is_error":false,"duration_ms":900,"result":{last},"cost_usd":0.03,"usage":{{"input_tokens":22,"output_tokens":14}}}}"#),
            ],
            Provider::Opencode => vec![
                r#"{"type":"step_start","sessionID":"o1","part":{"type":"step-start"}}"#.to_string(),
                format!(r#"{{"type":"text","sessionID":"o1","part":{{"id":"p1","type":"text","text":{early}}}}}"#),
                r#"{"type":"tool_use","part":{"id":"p2","callID":"call_1","tool":"bash","state":{"status":"completed","input":{"command":"cargo test"},"output":"test result: ok"}}}"#.to_string(),
                r#"{"type":"tool_use","part":{"id":"p3","callID":"call_2","tool":"read","state":{"status":"error","input":{"filePath":"src/b.rs"},"error":"no such file"}}}"#.to_string(),
                r#"{"type":"step_finish","part":{"tokens":{"input":10,"output":5,"reasoning":2,"cache":{"read":100,"write":20}},"cost":0.01}}"#.to_string(),
                r#"{"type":"step_start","part":{"type":"step-start"}}"#.to_string(),
                format!(r#"{{"type":"text","part":{{"id":"p4","type":"text","text":{last}}}}}"#),
                r#"{"type":"step_finish","part":{"tokens":{"input":3,"output":4,"reasoning":0,"cache":{"read":120,"write":0}},"cost":0.02}}"#.to_string(),
            ],
        };
        let mut lines = lines;
        lines.insert(1, r#"{"type":"brand_new_kind","x":{"y":1}}"#.to_string());
        lines.insert(2, "not json at all".to_string());
        lines.join("\n")
    }

    fn rate_limited(provider: Provider) -> String {
        match provider {
            Provider::Claude => unreachable!("Claude reports its own rate_limit_event"),
            Provider::Codex => [
                r#"{"type":"thread.started","thread_id":"th1"}"#,
                r#"{"type":"item.completed","item":{"id":"i0","type":"agent_message","text":"Working on it"}}"#,
                r#"{"type":"error","message":"exceeded retry limit, last status: 429 Too Many Requests"}"#,
                r#"{"type":"turn.failed","error":{"message":"stream disconnected before completion"}}"#,
            ]
            .join("\n"),
            Provider::Cursor => [
                r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Working on it"}]}}"#,
                r#"{"type":"result","subtype":"error","is_error":true,"duration_ms":10,"result":"HTTP 429: Too Many Requests"}"#,
            ]
            .join("\n"),
            Provider::Agy => [
                r#"{"type":"assistant","message":{"id":"m1","content":[{"type":"text","text":"Working on it"}]}}"#,
                r#"{"type":"result","subtype":"error_during_execution","is_error":true,"result":"RESOURCE_EXHAUSTED (429)","error":{"resets_at":1791592200}}"#,
            ]
            .join("\n"),
            Provider::Opencode => [
                r#"{"type":"text","part":{"id":"p1","type":"text","text":"Working on it"}}"#,
                r#"{"type":"error","error":{"name":"APIError","data":{"message":"Provider returned error","statusCode":429,"isRetryable":true}}}"#,
            ]
            .join("\n"),
        }
    }

    #[test]
    fn stream_normalizers_parse_status_findings_usage_and_rate_limits_for_all_five_providers() {
        let findings = Findings {
            high: 1,
            medium: 0,
            low: 2,
            fixed: 1,
            parked: 0,
        };
        for p in Provider::ALL {
            let (events, s) = parse_for(p, &fixture(p, FINAL));
            let who = p.as_str();
            assert!(s.has_result, "{who}");
            assert!(!s.is_error, "{who}");
            assert_eq!(s.status_hint, Some(StatusHint::Done), "{who}");
            let text = s.result_text.as_deref().unwrap();
            assert_eq!(Findings::from_result(text), Some(findings), "{who}");
            let usage = s.usage.as_ref().expect(who);
            assert!(usage.output_tokens.unwrap_or(0) > 0, "{who}");
            assert!(!s.tool_uses.is_empty(), "{who}");
            assert_eq!(s.tool_uses[0].name, "Bash", "{who}");
            assert_eq!(s.tool_uses[0].input["command"], "cargo test", "{who}");
            assert!(s.tool_uses[0].result_chars.is_some(), "{who}");
            assert_eq!(s.bad_lines, 1, "{who}");
            assert_eq!(s.events, events.len(), "{who}");
            assert_eq!(crate::quota::Limit::from(&s), None, "{who}");

            let blocked = FINAL.replace("STATUS: done", "STATUS: blocked");
            let (_, s) = parse_for(p, &fixture(p, &blocked));
            assert_eq!(s.status_hint, Some(StatusHint::Blocked), "{who}");
        }

        let (_, codex) = parse_for(Provider::Codex, &fixture(Provider::Codex, FINAL));
        let u = codex.usage.unwrap();
        assert_eq!(
            (u.input_tokens, u.cache_read_input_tokens, u.output_tokens),
            (Some(200), Some(800), Some(50))
        );
        assert_eq!(codex.num_turns, Some(1));
        assert_eq!(codex.session_id.as_deref(), Some("th1"));
        let names: Vec<_> = codex.tool_uses.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, ["Bash", "Edit"]);
        assert_eq!(codex.tool_uses[1].input["file_path"], "src/a.rs");

        let (_, cursor) = parse_for(Provider::Cursor, &fixture(Provider::Cursor, FINAL));
        assert_eq!(cursor.duration_ms, Some(1200));
        assert_eq!(cursor.model_id.as_deref(), Some("gpt-5"));
        assert_eq!(cursor.usage.unwrap().cache_read_input_tokens, Some(300));
        assert_eq!(cursor.tool_uses[1].input["file_path"], "src/a.rs");

        let (_, agy) = parse_for(Provider::Agy, &fixture(Provider::Agy, FINAL));
        assert_eq!(agy.model_id.as_deref(), Some("gemini-x"));
        assert_eq!(agy.cost_usd, Some(0.03));
        assert_eq!(agy.tool_uses[0].result_chars, Some(15));

        let (_, oc) = parse_for(Provider::Opencode, &fixture(Provider::Opencode, FINAL));
        let u = oc.usage.unwrap();
        assert_eq!(
            (
                u.input_tokens,
                u.cache_read_input_tokens,
                u.cache_creation_input_tokens
            ),
            (Some(13), Some(220), Some(20))
        );
        assert_eq!((u.output_tokens, u.thinking()), (Some(11), Some(2)));
        assert_eq!(oc.num_turns, Some(2));
        assert!((oc.cost_usd.unwrap() - 0.03).abs() < 1e-9);
        assert!(oc.tool_uses[1].result_error);
        assert_eq!(oc.tool_uses[1].name, "Read");
        assert_eq!(oc.tool_uses[1].input["file_path"], "src/b.rs");

        for p in [
            Provider::Codex,
            Provider::Cursor,
            Provider::Agy,
            Provider::Opencode,
        ] {
            let who = p.as_str();
            let (_, s) = parse_for(p, &rate_limited(p));
            assert!(s.has_result && s.is_error, "{who}");
            let rl = s.rate_limit.as_ref().expect(who);
            assert_eq!(rl.status.as_deref(), Some("rejected"), "{who}");
            assert!(s.result_text.is_some(), "{who}");
            assert!(crate::quota::Limit::from(&s).is_some(), "{who}");
        }
        let (_, agy) = parse_for(Provider::Agy, &rate_limited(Provider::Agy));
        assert_eq!(
            crate::quota::Limit::from(&agy).unwrap().until,
            Some(1_791_592_200)
        );
    }

    #[test]
    fn a_worker_killed_mid_narration_is_dead() {
        let codex = [
            r#"{"type":"thread.started","thread_id":"th1"}"#,
            r#"{"type":"item.completed","item":{"id":"i0","type":"agent_message","text":"Running tests"}}"#,
        ]
        .join("\n");
        assert!(parse_for(Provider::Codex, &codex).1.is_dead());
        let opencode = [
            r#"{"type":"text","part":{"id":"p1","type":"text","text":"Running tests"}}"#,
            r#"{"type":"step_finish","part":{"reason":"tool-calls","tokens":{"input":1,"output":1}}}"#,
        ]
        .join("\n");
        assert!(parse_for(Provider::Opencode, &opencode).1.is_dead());
    }

    #[test]
    fn a_failure_outranks_earlier_narration_and_a_completed_turn_outranks_a_retry() {
        let done = serde_json::to_string(FINAL).unwrap();
        let failed = [
            format!(r#"{{"type":"item.completed","item":{{"id":"i0","type":"agent_message","text":{done}}}}}"#),
            r#"{"type":"turn.failed","error":{"message":"429 Too Many Requests"}}"#.to_string(),
        ]
        .join("\n");
        let (_, s) = parse_for(Provider::Codex, &failed);
        assert_eq!(s.status_hint, None);
        assert!(crate::quota::Limit::from(&s).is_some());

        let retried = [
            r#"{"type":"error","message":"Reconnecting... 1/5 (429 Too Many Requests)"}"#.to_string(),
            r#"{"type":"item.completed","item":{"id":"i0","type":"agent_message","text":"STATUS: failed if the build breaks"}}"#.to_string(),
            format!(r#"{{"type":"item.completed","item":{{"id":"i1","type":"agent_message","text":{done}}}}}"#),
            r#"{"type":"turn.completed","usage":{"input_tokens":5,"output_tokens":1}}"#.to_string(),
        ]
        .join("\n");
        let (_, s) = parse_for(Provider::Codex, &retried);
        assert!(!s.is_error && s.rate_limit.is_none());
        assert_eq!(s.status_hint, Some(StatusHint::Done));
    }

    #[test]
    fn a_tool_call_updated_under_its_id_keeps_its_last_input() {
        let opencode = [
            r#"{"type":"tool_use","part":{"callID":"c1","tool":"bash","state":{"status":"pending","input":{}}}}"#,
            r#"{"type":"tool_use","part":{"callID":"c1","tool":"bash","state":{"status":"completed","input":{"command":"cargo test"},"output":"ok"}}}"#,
        ]
        .join("\n");
        let (_, s) = parse_for(Provider::Opencode, &opencode);
        assert_eq!(s.tool_uses.len(), 1);
        assert_eq!(s.tool_uses[0].input["command"], "cargo test");

        let codex = r#"{"type":"item.completed","item":{"id":"i1","type":"file_change","changes":[{"path":"src/a.rs"},{"path":"src/b.rs"}],"status":"completed"}}"#;
        let (_, s) = parse_for(Provider::Codex, codex);
        let paths: Vec<_> = s.tool_uses.iter().map(|t| &t.input["file_path"]).collect();
        assert_eq!(paths, ["src/a.rs", "src/b.rs"]);
    }

    #[test]
    fn an_agy_usage_limit_keeps_the_reset_its_rate_limit_event_named() {
        let agy = [
            r#"{"type":"rate_limit_event","rate_limit_info":{"status":"rejected","resetsAt":1791592200}}"#,
            r#"{"type":"result","is_error":true,"result":"You've hit your usage limit"}"#,
        ]
        .join("\n");
        let (_, s) = parse_for(Provider::Agy, &agy);
        assert_eq!(s.rate_limit.unwrap().resets_at, Some(1_791_592_200));
    }

    #[test]
    fn only_an_error_naming_a_limit_is_a_rate_limit() {
        assert!(detect_rate_limit("Usage Limit reached", None).is_some());
        assert!(detect_rate_limit("status 429", Some(5)).unwrap().resets_at == Some(5));
        assert!(detect_rate_limit("rate_limit_exceeded", None).is_some());
        assert!(detect_rate_limit("line 14290 failed", None).is_none());
        assert!(detect_rate_limit("compile error", None).is_none());
        assert!(detect_rate_limit("rustc: src/lib.rs:429:10 mismatched types", None).is_none());
        let (_, s) = parse_for(
            Provider::Cursor,
            r#"{"type":"result","is_error":true,"duration_ms":429,"result":"sandbox denied"}"#,
        );
        assert!(s.rate_limit.is_none());
        let (_, s) = parse_for(
            Provider::Codex,
            r#"{"type":"turn.failed","error":{"message":"sandbox denied"}}"#,
        );
        assert!(s.has_result && s.is_error && s.rate_limit.is_none());
        assert_eq!(s.result_text.as_deref(), Some("sandbox denied"));
        assert_eq!(crate::quota::Limit::from(&s), None);
    }
}
