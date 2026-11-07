# Slice 06 — What each worker is doing

**Source**: [`prd.md`](prd.md) → §5 FR-3. **Depends on**: 01 (the minimal `workers[]` and `src/live/workers.rs`, `tests/live/workers.rs` it lays out). Independent of 02, 03, 04, 05, 07.
**Created**: 2026-11-07

**Progress**:
- **Built:** `src/live/workers.rs` fills `job`, `doing`, `narration`, `running` and `outcome` for each `workers[]` entry. `each_line_back` reads the stream backwards in 64 KiB blocks, up to 1 MiB. It drops the partial last line, lines over 64 KiB and the line the budget cuts. `read_tail` parses only lines that can still decide a field: a byte check runs before `serde_json`. A live worker stops once every field is decided. A finished one stops at its `result` line (AD-2). `running` matches a heartbeat's `parent_tool_use_id` against `tool_result.tool_use_id`. A label is the Bash `description`, `Skill <name>`, `Agent <description>`, or the tool plus a path relative to the lane dir or then the repo, otherwise just the tool name. A label never holds a command, tool output or an absolute path. `outcome` reuses `StatusHint::from_result`. `job` comes from `fix_spawned` / `ci_fix_spawned` by session id.
- **Tests:** `tests/live/workers.rs` has the five task-1 acceptance tests, `live_tells_a_ci_fix_from_a_gate_fix` and `live_worker_outcome_reads_the_status_below_the_first_line`. `src/live/workers.rs` has unit tests for the backwards reader (block-crossing and oversized lines, the budget), path labels and the `BLOCK:` line.
- **Inner loop:** 2 runs, 1 red, ~2 min. The red run was a clippy `redundant_closure`.
- **Deviations:** none from the slice. AD-2 (fyi) covers what a finished worker's tail shows.
- **Prove (FR-3):** left for the checkpoint.

**Slice**: After this slice, each `workers[]` entry says what the worker is doing now: its `job` (a gate fix or a CI fix), the tool it is in and a safe label for it, whether that is inside a subagent, its last narration line, the tool still running and for how long, and its outcome once it has returned. A bounded tail reader gets all of it from the end of the worker's stream, never parsing the whole file, and never printing a command or tool output.

## Current code / environment

- **The stream**: claude's stdout at `runs/<id>/workers/<session>.jsonl` (`src/worker.rs:18`, `stream_path` `:177-179`, wired `:234`). It grows during the run and reaches 9.4 MB (prd §11). `stream.rs` parses whole files at worker exit and has no `tool_progress` variant (`src/stream.rs:10-35`); its timestamps come from `--verbose` lines (`:434-439`). prd §5: a new reader beside it, not a change to it.
- **`STATUS:` parsing exists**: `StatusHint::from_result` (`src/stream.rs:117-131`) finds the `STATUS:` line anywhere in the result text. Reuse it for `outcome.status`; the `BLOCK:` line is read the same way (`none` → null).
- **Roles and jobs**: `WorkerRef.role` (`src/run.rs:139-153`). `Job::CiFix` maps to role `"fix"` (`src/worker.rs:128-136`), so a CI fix is told apart only by its `ci_fix_spawned {round, attempt, session_id, pid}` event (`src/worker.rs:378`, `src/events.rs:214`); a gate fix has `fix_spawned {n, attempt, session_id, pid}` (`src/events.rs:143`).
- **Narrowed tools**: build, fix and merge workers run with `--tools Bash,Read,Edit,Write` (`src/worker.rs:93-95`, `:226-229`); checkpoint and polish keep `Skill`, `Agent` and MCP tools.
- **The fixtures, committed with the PRD** (`tests/fixture/recordings/`, strings over 300 chars cut, home `/home/user`):
  - `claude-stream-build-mid-bash-2.1.316.jsonl`, 125 lines: 40 assistant, 19 user, 63 system, 2 `tool_progress`, 1 `rate_limit_event`, no `result`. The last assistant `tool_use` is a Bash (`toolu_01DAy54snXrasn1qqKV6Jp8Y`) with only `command` and `timeout` in its input, **no `description`**, and no `tool_result` after it; the file ends on its two heartbeats (`elapsed_time_seconds` 30 and 60). A `system` `task_started` line before them carries the command text in its own `description` field: never read it for a label.
  - `claude-stream-polish-skill-2.1.316.jsonl`, 60 lines: line 1 is the mise banner (not JSON); a `Skill` call (`input.skill` `code-review`); three later Bash `tool_use` lines sit in a subagent (`parent_tool_use_id` set on the assistant line). No `result` line.
  - `tool_progress` lines have no `timestamp` field; most other lines do.
  - Neither fixture holds a `result` line or a planted secret: the `outcome` and the fake-token cases need small synthetic streams written by the test.
- **The heartbeat trap** (prd §5): a `tool_progress` line's `tool_use_id` is `<id>-heartbeat-N`; the tool it belongs to is `parent_tool_use_id`, and that is what a later `tool_result` names.
- **Deliberate non-change**: prd §11 "A transcript viewer: streams run to 9.4 MB and hold secrets." prd §14 "Not handled: a 'stuck' verdict on a worker. Long silences are normal." prd §5 "Every derived field is nullable. Claude Code's stream is not a contract."

## Tasks

### 1. The tail reader — high — blocked by: —
**Source**: §5 FR-3 "The tail reader: Reads backwards in 64 KiB blocks, at most 1 MiB per live worker. Skips lines that do not start with `{` (the mise banner), lines over 64 KiB unparsed, and a partial last line", and the field rules: "`doing` is `{tool, label, at, inSubagent}` from the newest assistant `tool_use`. `label` is Bash `input.description`, else `Skill <name>` or `Agent <description>`, else the tool name plus a lane-relative path. **Never `input.command` and never tool output.**", "`at` comes from the newest line with a `timestamp`, of any type, or null", "`narration` is the newest assistant text block, at most 160 chars", "`running` is `{tool, elapsedSeconds}` from the newest `tool_progress` whose tool_use has no later `tool_result`", "`outcome` is `{status: done|blocked, block}` from the stream's last `result` line. Its `STATUS:` header is not always on line 1."

`src/live/workers.rs`. Parse each kept line as `serde_json::Value`, newest first, stopping once every field is decided or the 1 MiB budget is spent. A Bash with no `description` is labelled `Bash`. A path label is relative to the worker's lane dir when the path is under it (`lanes::dir(data_dir, repo, lane)`, `src/lanes.rs:162`, a path computation; `WorkerRef.lane` and `state.repo` give the inputs), relative to `state.repo` when under that, else just the tool name. Never print an absolute path. Read only live workers' streams in full budget; a finished worker reads only as far as its `result` line.

**The trap:** `running` must match `parent_tool_use_id` of the heartbeat against later `tool_result.tool_use_id`; matching `tool_use_id` never clears, because no `tool_result` names `…-heartbeat-N`.

**Acceptance:** `live_worker_says_what_it_is_doing` (the build fixture as a live worker's stream: `doing.tool == "Bash"`, `doing.label == "Bash"`, `running {tool: "Bash", elapsedSeconds: 60}`, a `narration` ≤ 160 chars; the polish fixture: banner skipped, `doing.inSubagent == true` on its last Bash, and a stream cut before those lines gives `doing.label == "Skill code-review"`); `live_worker_running_clears_once_the_tool_returns` (the build fixture plus a `tool_result` for `toolu_01DAy54snXrasn1qqKV6Jp8Y` gives `running: null`); `live_worker_label_never_carries_the_command` (a synthetic stream whose Bash `input.command` and `tool_result` hold a fake token: no byte of the token in the output); `live_skips_an_oversized_line` (a 70 KiB line between two valid ones is skipped, the fields come from the valid lines); `live_worker_on_an_unknown_stream_shape_is_null_not_an_error` (a stream of JSON lines with unknown `type`s and a truncated last line: exit 0, every derived field null).

### 2. `job` and `outcome` — high — blocked by: 1
**Source**: §5 FR-3: "`job` is `gateFix|ciFix|null`, matched by session_id against the spawn events", and `outcome` as above.

`src/live/workers.rs`.

**Acceptance:** `live_tells_a_ci_fix_from_a_gate_fix` (two role-`fix` workers, one with a `fix_spawned` and one with a `ci_fix_spawned` for its session: `gateFix` and `ciFix`; a build worker: `null`); a synthetic stream ending in a `result` whose text has `STATUS: blocked` on line 3 and `BLOCK: BLOCK-2 …` gives `outcome {status: "blocked", block: "BLOCK-2 …"}`.

### 3. Verify — medium — blocked by: 2
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 4. Prove — at the checkpoint — blocked by: 3
prd §12 row **FR-3**: run `live_worker_says_what_it_is_doing`, `live_worker_label_never_carries_the_command` and `live_worker_on_an_unknown_stream_shape_is_null_not_an_error`, and keep the transcript.

**Acceptance:** after the checkpoint, this slice's Progress names that transcript.
