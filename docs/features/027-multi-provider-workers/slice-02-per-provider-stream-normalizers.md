# Slice 02 — Per-provider stream normalizers, rate limits, and subagent telemetry

**Source**: [`prd.md`](prd.md) → §6 (FR-4), §9, §11. **Depends on**: — (independent of slice 01; touches only `src/stream.rs` and `src/metrics.rs`, both using `crate::provider::Provider` already merged from PRD-026)
**Created**: 2026-11-25

**Progress**: _(written as it lands — what was built, the gate numbers, what a review caught)_

**Slice**: Adds `stream::parse_for(provider: Provider, text: &str)` normalizing JSONL streams from all five providers (`claude`, `codex`, `cursor`, `agy`, `opencode`) into `Summary` — including `STATUS:`/`FINDINGS:` extraction, token usage, tool calls, and `quota::Limit::from`-compatible rate-limit detection — and adds `metrics::record_subagents_for(..., provider: Provider)` so non-Claude workers record `None` cleanly without looking up `~/.claude/projects/` or emitting spurious `SubagentsUnread` events. Leaves worker spawning and `live/workers.rs` wiring to slice 03.

## Current code / environment

- **Stream parser**: `src/stream.rs:1, :8-35` parses Claude NDJSON (`System`, `Assistant`, `User`, `RateLimitEvent`, `Result`, `Unknown`) into `Summary` (`:215-240`: `has_result`, `usage`, `model_usage`, `num_turns`, `duration_ms`, `terminal_reason`, `is_error`, `api_error_status`, `permission_denials`, `cost_usd`, `session_id`, `cwd`, `result_text`, `status_hint`, `peak_context`, `start_context`, `model_id`, `tool_uses`, `rate_limit`, `events`, `bad_lines`). `StatusHint::from_result` (`:117-132`) and `Findings::from_result` (`:146-151`) operate on `result_text`.
- **Rate-limit contract**: `src/quota.rs:37-68` (`Limit::from`) returns `Some(Limit { .. })` when:
  - `summary.rate_limit.status` is `Some(s)` and `!s.starts_with("allowed")` (returning immediately at `:39-49` with `until: resets_at`), OR
  - `summary.has_result && summary.is_error` and (`is_429(api_error_status)` or `result_text` lowercased contains `"usage limit"` or `"rate limit"`).
- **Subagent telemetry**: `src/metrics.rs:449-480` (`record_subagents`) unconditionally looks up `paths::claude_config_dir()` (`~/.claude/projects/...`). Existing callers (`src/scheduler.rs:1862`, `src/checkpoint.rs:1177`, `src/closeout.rs:2449`) stay compiling if `record_subagents` delegates to `record_subagents_for(..., Provider::Claude)` until slice 03 passes the worker's actual `Provider`.
- **Provider stream shapes (harmonized with `src/review.rs` from PRD-026)**:
  - **`claude`**: unchanged existing parser.
  - **`codex`** (`codex exec --json`): JSONL lines with `"type"`:
    - `"thread.started"` (`thread_id`)
    - `"item.completed"` with `item.type`: `"agent_message"` (`item.text` → `result_text`, `has_result = true`), `"command_execution"` (`item.command`, `item.aggregated_output`, `item.exit_code` → `ToolUse { name: "Bash", .. }`), `"file_change"` (`item.changes` → `ToolUse { name: "Edit", .. }`)
    - `"turn.completed"` (`usage.input_tokens`, `usage.cached_input_tokens` → `cache_read_input_tokens`, `usage.output_tokens`; increments `num_turns`)
    - `"error"` (`message`) / `"turn.failed"` (`error.message`) → `is_error = true`, `has_result = true`, plus rate-limit check.
  - **`cursor`** (`cursor-agent -p --output-format stream-json`): `"assistant"` (`message.content` text / tool blocks), `"tool_call"` (`subtype == "completed"`, tool name/args/result), `"result"` (`result`, `is_error`, `duration_ms`, optional `usage`), plus rate-limit check.
  - **`agy`** (`agy -p --output-format stream-json`): `"system"`, `"assistant"` (`message.content` text / `tool_use` blocks, `message.usage`, `message.model`), `"user"` (`tool_result`), `"result"` (`result`, `is_error`, `duration_ms`, `usage`, `cost_usd`), `"rate_limit_event"` / error rate-limit check.
  - **`opencode`** (`opencode run --standalone --format json`): `"text"` (`part.text` → appends/updates assistant text, `has_result = true`), `"tool_use"` (`part.tool`, `part.state.input`, `part.state.output`, `part.state.status == "error"` → `ToolUse`), `"step_finish"` (`part.tokens.{input,output,reasoning,cache.read,cache.write}`, `part.cost` → `usage` / `cost_usd`, increments `num_turns`), `"error"` (`error.message` or `data.message` → `is_error = true`, `has_result = true`, plus rate-limit check).
- **Deliberate non-changes (PRD-027 §11)**:
  - No private subagent transcript scraping for non-Claude providers.

## Tasks

### 1. Implement `stream::parse_for(Provider, &str)` with rate-limit normalization in `src/stream.rs` — high — blocked by: —
**Source**: §6 FR-4: "Add `pub fn parse_for(provider: Provider, text: &str) -> (Vec<StreamEvent>, Summary)` and keep `pub fn parse(text: &str)` delegating to `parse_for(Provider::Claude, text)`... Rate-limit contract for `quota::Limit::from`".

`src/stream.rs`.
- Add `pub fn parse_for(provider: Provider, text: &str) -> (Vec<StreamEvent>, Summary)` dispatching:
  - `Provider::Claude` → existing parser logic unchanged.
  - `Provider::Codex`, `Provider::Cursor`, `Provider::Agy`, `Provider::Opencode` → dedicated normalizers populating `Summary` (`has_result`, `result_text`, `status_hint`, `is_error`, `usage`, `cost_usd`, `num_turns`, `duration_ms`, `model_id`, `tool_uses`, `rate_limit`, `events`, `bad_lines`).
- Compute `status_hint = result_text.as_deref().and_then(StatusHint::from_result)` at the end of every normalizer so `STATUS: done|blocked|failed` works identically across all 5 providers (and `Findings::from_result` works on `result_text` for polish).
- Implement a shared `detect_rate_limit(msg: &str, resets_at: Option<u64>) -> Option<RateLimit>` helper: whenever an error/failure event reports HTTP `429`, `"rate limit"`, `"rate_limit"`, or `"usage limit"` (case-insensitive), set `summary.rate_limit = Some(RateLimit { status: Some("rejected".into()), resets_at, utilization_pct: None })`, `summary.is_error = true`, `summary.has_result = true`, and fill `summary.result_text` with the error message if no assistant text was emitted — guaranteeing `crate::quota::Limit::from(&summary)` returns `Some(Limit { .. })`.
- Never panic on unknown event types, missing fields, or non-JSON lines (`bad_lines` incremented on non-JSON lines; unknown JSON objects ignored gracefully).
- Keep `pub fn parse(text: &str)` delegating to `parse_for(Provider::Claude, text)`.

**The trap**: When a Codex or OpenCode run emits multiple `agent_message` / `text` items across turns, `result_text` must include the final assistant message (or concatenated final turn text) so `STATUS: done` printed in the last message is never overwritten by an earlier turn's narration. Also check `quota::Limit::from`: if a stream reports a rate limit on a non-Claude provider AND also emitted earlier text, setting `summary.rate_limit = Some(RateLimit { status: Some("rejected".into()), .. })` still trips `quota::Limit::from` at `src/quota.rs:39-48` regardless of `result_text`!

**Acceptance:** Unit test `stream_normalizers_parse_status_findings_usage_and_rate_limits_for_all_five_providers` in `src/stream.rs` passes:
- For each of the 5 providers (`Claude`, `Codex`, `Cursor`, `Agy`, `Opencode`), parses a realistic multi-event JSONL fixture verifying: `has_result == true`, `status_hint == Some(StatusHint::Done)` (and `Blocked`), `Findings::from_result` parses `FINDINGS: high=1 medium=0 low=2 fixed=1 parked=0`, token `usage` (where emitted) and `tool_uses` are populated, and unknown event lines do not error.
- For each non-Claude provider, parses a 429 / rate-limit error fixture and asserts `crate::quota::Limit::from(&summary).is_some()` (fails if rate-limit normalization or `status: Some("rejected")` is removed).

### 2. Add provider-aware `metrics::record_subagents_for` in `src/metrics.rs` — high — blocked by: —
**Source**: §6 FR-4: "`metrics::record_subagents` takes `provider: Provider`; for `Provider::Claude` it reads `~/.claude/projects/` transcripts as today; for non-Claude providers it writes `None` (unmeasured) to the subagent columns and never looks up `~/.claude/projects/` or emits a spurious `EventKind::SubagentsUnread` cockpit warning."

`src/metrics.rs`.
- Add `pub fn record_subagents_for(run: &mut Run, worker_id: i64, session_id: &str, cwd: Option<&Path>, provider: Provider) -> Result<()>`:
  - When `provider != Provider::Claude`: call `run.db().set_worker_subagents(worker_id, None)?;` and return `Ok(())` immediately without touching `paths::claude_config_dir()` or emitting `EventKind::SubagentsUnread`.
  - When `provider == Provider::Claude`: run existing `~/.claude/projects/` lookup and `SubagentsUnread` logic unchanged.
- Keep `pub fn record_subagents(run, worker_id, session_id, cwd)` delegating to `record_subagents_for(..., Provider::Claude)` so existing callers keep compiling until slice 03 switches them to pass the worker's actual `provider`.

**Acceptance:** Unit test `non_claude_worker_skips_claude_subagent_lookup_without_warning` in `src/metrics.rs` creates a run + worker row with `CLAUDE_CONFIG_DIR` pointing at an unreadable mode-`000` directory, calls `record_subagents_for(..., Provider::Codex)`, and asserts `Ok(())`, zero `SubagentsUnread` events in `run.events()`, and `subagents == None` (and `sub_input == None`) on the worker row (fails if non-Claude still reads `~/.claude`).

### 3. Verify — medium — blocked by: 1, 2
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q`

**Acceptance:** inner loop green, one commit.

### 4. Prove — at the checkpoint — blocked by: 3
Drive these sub-rows from [`prd.md`](prd.md) §9 QA plan at CP-1 (live JSON proof follows in slice 03 at CP-2):
- **FR-4 (stream & subagents)**: run `cargo test stream_normalizers_parse_status_findings_usage_and_rate_limits_for_all_five_providers` and `cargo test non_claude_worker_skips_claude_subagent_lookup_without_warning`.

**Acceptance:** both tests have passing transcripts captured in CP-1 evidence and recorded in **Progress**.
