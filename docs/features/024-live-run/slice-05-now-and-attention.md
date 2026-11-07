# Slice 05 — What the run waits on, and what the owner owes

**Source**: [`prd.md`](prd.md) → §4 FR-2. **Depends on**: 01 (the `src/live/now.rs` stub, `tests/live/now.rs`, `run_waiting.reason` and the new kinds `waitingOn` reads) and 03 (`attention[].reason.excerpt` is the excerpt of the block's gate or CI log, read through the extractor and the log reader slice 03 adds in `src/live/gate.rs`). Independent of 02, 04, 06, 07.
**Created**: 2026-11-07

**Progress**:
- **Built**: `cockpit::Question` and `cockpit::question` (moved from `status`, which now maps through them, its JSON unchanged); `src/live/now.rs` fills `now` (the ten-step `waitingOn`, `label`, `since`, review `progress`, quota `resumesAt`, `idleSeconds` over events, worker streams and gate logs, `driverAlive`, `stopping`) and `attention[]` (questions with status's `answer` argv and their `log.jsonl` body, `gate`/`ci`/`driver` reasons with the excerpt from slice 03's reader, a `park-<seq>` entry for a park with no question, `warn-<seq>` entries). Tests: seven in `tests/live/now.rs`, and `a_park_with_no_question_carries_its_reason` in `tests/commits.rs`.
- **Inner loop**: 2 runs, 1 red, 1.7 min. The red run caught a clippy `cloned_ref_to_slice_refs` in a new test. Targeted runs of the new tests were green first time.
- **Deviations**: AD-11 (a new end-to-end test by the Prove row's name, not asserts inside the dirty-landing test); AD-12 (what `now` and `attention` say where the slice is silent).

**Slice**: After this slice, `factory live` fills `now` (what the run waits on, in factory's words, since when, review progress, when a quota pause resumes, how long nothing moved, whether the driver lives, what a stop waits for) and `attention[]` (every open block and ask with the exact `answer` argv `status --json` gives, its log body and the failing excerpt behind it, a park with no question and its reason, and the two warning kinds). The question list moves into one function that `status` and `live` share.

## Current code / environment

- **Open questions**: `cockpit::questions` (`src/cockpit.rs:204-209`) is each open id's latest opening event (`needed`, `:183-200`; `opens`, `:156-167`), none once the run is over. `status` turns each into `QuestionJson {id, at, n, text, answer}` (`src/cli/status.rs:77-83`) through `question()` (`:251-296`), whose `answer` is `["factory","answer","--feature",<NNN>,<BLOCK-n>]` or `["factory","answer","--run",<run id>,<ASK-n>]` and which drops ids not shaped `BLOCK-n`/`ASK-n` (`:283-286`). prd §4: "`id`, `title` and `answer` come from one function shared with status". Move `question()` (and its struct, or a plain one `status` maps from) into `cockpit.rs`, keep `status`'s output byte for byte, and call it from `live`.
- **Parks**: `park_with` (`src/scheduler.rs:603-627`) emits `run_waiting` once per park and sets `waiting-owner`; from slice 01 the event carries `reason` (`what`). A park with no question has `blocked` and `asks` empty: the dirty-landing park (`Landed::Waiting`) through `scheduler.rs:181`, `:275`, `:438`, `:450`, `:467`, `checkpoint.rs:829`, `closeout.rs:622`. `enter_park` (`:581-600`) parks on blocks or asks, which are questions already. Preflight asks set `waiting-owner` with no `run_waiting` (`src/cli/run.rs:114-128`).
- **Budget and quota**: a budget park is `run_parked {reason, weighted, budget}` (`src/quota.rs:202-206`), found by `cockpit::park` (`src/cockpit.rs:260-268`, only while `waiting-owner`). A quota pause is phase `paused` (`src/run.rs:62`) after `run_paused {literal, resets_at}` (`src/quota.rs:161-164`, epoch seconds). No reset time exists for a budget park.
- **Driver**: `RunState::driver_alive` (`src/run.rs:129-131`). `cockpit::driver_dead` (`src/cockpit.rs:272-280`) is false on a budget park and on preflight asks; `waitingOn` has its own order (below) and does not reuse it.
- **Stop**: `stop_requested` (`src/run.rs:104`).
- **The log body**: the feature's `log.jsonl` (`src/docs/log.rs:127-128`, `entries` at `:154`), found through the feature-folder lookup slice 01 shared. An `Entry` has `id`, `title`, `body` and no author (`:67-91`).
- **Workers in flight**: `cockpit::in_flight` (`src/cockpit.rs:247-253`): `outcome.is_none() && worker_alive`.
- **Warnings**: `polish_findings_missing {session_id}` (`src/closeout.rs:307`), `pack_section_missing {slice, section}` (`src/scheduler.rs:742`).
- **The dirty-landing end-to-end**: `a_dirty_path_in_the_main_tree_parks_the_landing_and_keeps_the_owners_edit` (`tests/commits.rs:536`) parks slice 01's landing on an owner edit and waits for `waiting-owner` (`:549-552`).
- **Deliberate non-change**: prd §4 "A park whose `what` changes mid-park still emits once, so `reason` is the park's first reason." prd §4 "A log entry has no author field, so `live` does not say who wrote it." prd §14 "Not handled: a 'stuck' verdict on a worker."

## Tasks

### 1. One question function for status and live — high — blocked by: —
**Source**: §4 FR-2: "`id`, `title` and `answer` come from **one function shared with status**, so `answer` is exactly status's `questions[].answer` argv, `--feature` included."

`src/cockpit.rs`, `src/cli/status.rs`.

**Acceptance:** `live_attention_answer_is_status_answer`: over a fixture run with an open slice block and a preflight ask, each `attention[]` entry of kind `block`/`ask` has the same `id`, `title` (status's `text`) and `answer` as `status <id> --json`'s `questions[]`. The existing status tests (`status_json_carries_the_board_and_what_the_owner_owes`, `status_json_gives_a_preflight_ask_the_command_that_answers_it`) stay green unchanged.

### 2. `attention[]` — high — blocked by: 1
**Source**: §4 FR-2: "`attention[]`: `{ id, kind: block|ask|park|warn, openedAt, title, body, reason, answer }`", "`body` is the body of the `log.jsonl` entry whose `id` is the question's id, else null", "`reason` is `{source: gate|ci|land|driver, log, excerpt}`", "With no question, it becomes a `kind: park` entry", "`kind: warn` carries `polish_findings_missing` and `pack_section_missing`."

`src/live/now.rs`. Questions first, in `cockpit::questions` order (`kind` `ask` for `ASK-n`, else `block`; `openedAt` the opening event's `ts`). A park entry when the phase is `waiting-owner`, no question is open and the newest `run_waiting` has empty `blocked` and `asks`: `title` its `reason`, `answer` null, an id of its own that cannot collide with a log id (e.g. `park-<seq>`). `reason.source`: `gate` for a checkpoint block whose newest gate before it is red (`log` that gate's log, `excerpt` from slice 03's reader), `ci` for a closeout block in phase `ci` with a `ci/<round>.log` (its excerpt), `land` for a park entry, `driver` otherwise; `log`/`excerpt` null when there is none. One `warn` entry per warning event, `title` naming what is missing.

**Acceptance:** `a_park_with_no_question_carries_its_reason`: extend `a_dirty_path_in_the_main_tree_parks_the_landing_and_keeps_the_owners_edit` so that while parked, the `run_waiting` event's `reason` names the dirty path and `factory live <id> --json` has one `park` entry with that `reason` as its title. A fixture with a red-gate checkpoint block and its log gives a `block` entry with `reason.source == "gate"` and a non-empty `excerpt`, and `body` equal to the planted log entry's body.

### 3. `now` — high — blocked by: 2
**Source**: §4 FR-2: "`now`: `{ waitingOn, label, since, progress, resumesAt, idleSeconds, driverAlive, stopping }`", the ten-step `waitingOn` order (done, owner, budget, quota, driver, gate, review, ci, worker, nothing; "a quota pause with live workers is `quota`"; driver "includes an answer nobody picked up"), "`label` is factory's own sentence: 'checkpoint 2 gate, attempt 2'", "`progress` is `{done, of}` from FR-5's review events", "`resumesAt` is set for quota … null for a budget park", "`idleSeconds` is now minus the newest event, stream write or gate log write", "`stopping` is null or `{on: "worker|gate", what}`."

`src/live/now.rs`. `owner` is an open question or a park entry. `gate` is a `gate_started` with no later `gate_run`; `review` a `collect_started` with no later `collect_done`; `ci` when the newest of `ci_waiting`, `ci_no_checks`, `ci_red`, `ci_green` is one of the first two. `since` is the `ts` of the event that put the run in that state (null for `nothing`). `progress` only under `review`: `of` is `collect_started.passes`, `done` the `review_pass_done` count after it. `resumesAt` RFC3339 from `resets_at`. `stopping` when `stop_requested`: `gate` while a gate runs, else `worker` while one is in flight, `what` naming it.

**Acceptance:** `live_names_what_the_run_waits_on`: one fixture run per `waitingOn` value, ten in all, each asserting `waitingOn` and a non-empty `label`; the quota fixture also has a live worker (this test's own pid in `state.json`) and still reads `quota` with `resumesAt` set; the budget fixture has `resumesAt: null`; the review fixture has `progress {done: 1, of: 3}`. `live_says_driver_after_an_answer_nobody_picked_up`: a parked run whose newest event is `block_answered` and whose driver pid is dead reads `driver`.

### 4. Verify — medium — blocked by: 3
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 5. Prove — at the checkpoint — blocked by: 4
prd §12 row **FR-2**: run `live_names_what_the_run_waits_on`, `a_park_with_no_question_carries_its_reason` and `live_attention_answer_is_status_answer`, and keep the transcript.

**Acceptance:** after the checkpoint, this slice's Progress names that transcript.
