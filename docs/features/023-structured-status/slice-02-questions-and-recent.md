# Slice 02 — Questions with their answer argv, and --recent

**Source**: [`prd.md`](prd.md) → §5 FR-3, §6 FR-4, §8. **Depends on**: 01 (same `RunJson` and `run_json` in `src/cli/status.rs`, and the same tests in `tests/cockpit.rs`)
**Created**: 2026-11-06

**Progress**:
- Built: `cockpit::questions` (each open id's latest opening event in `owner`'s order, none once over; `needed` without the park). `status.rs` maps each one to `QuestionJson` (`id`, `at`, `n`, `text`, `answer`) on `RunJson.questions`. `--recent` (`requires = "json"`, `conflicts_with = "run"`) swaps `live_runs` for `recent_runs`: `list_runs` filtered to live runs plus over runs whose `updated_at` falls within `RECENT_SECS` (7 days). `live_runs` is unchanged.
- Tests: `questions_come_apart_from_their_latest_opening_with_the_argv_that_answers_them` (unit, `src/cli/status.rs`: five kinds, one re-opened, one answered, over gives `[]`). `status_json_carries_the_board_and_what_the_owner_owes` pins the BLOCK question and answers through its argv (`answer_as_told`, run from `repo`); its existing asserts are kept. `status_json_gives_a_preflight_ask_the_command_that_answers_it` covers the ASK case. `recent_adds_runs_that_ended_this_week` covers the listings and the two refused flag mixes (exit 2, clap's message).
- Inner loop: 1 run, 0 red, ~1.6 min.
- Deviations: none.
- Prove (FR-3, FR-4 rows of prd §8): left for the checkpoint.
- Prove FR-3 (checkpoint 1): `cp-1/fr3-answers.txt`, `cp-1/fr3-answers-BLOCK-1.json`, `cp-1/fr3-answers-ASK-1.json`. Each `questions[].answer` argv was run with a text appended from the run's repo. Both exited 0 and wrote their answer files, and the next `--json` has `questions: []`. Pinned by `status_json_carries_the_board_and_what_the_owner_owes` and `status_json_gives_a_preflight_ask_the_command_that_answers_it` (`cp-1/test-status_json_carries_the_board_and_what_the_owner_owes.log`, `cp-1/test-status_json_gives_a_preflight_ask_the_command_that_answers_it.log`).
- Prove FR-4 (checkpoint 1): `cp-1/fr4-recent.txt`. Over the live, 2-day and 8-day fixture runs, `--json --recent` lists the 2-day and live runs in start order and `--json` lists the live one only. `--recent --here` from outside the repo lists none. `--recent` alone and `<id> --json --recent` both exit 2 with clap's message. Pinned by `recent_adds_runs_that_ended_this_week` (`cp-1/test-recent_adds_runs_that_ended_this_week.log`).

**Slice**: After this slice, each run object carries `questions`: every open id apart, with `at`, `n`, the full `text` and the exact `factory answer` argv. And `factory status --json --recent` also lists runs that ended in the last 7 days. The default listing, the plain and `--line` outputs, and the `owner` strings stay as they are.

## Current code / environment

- **Order and source event**: `needed` (`src/cockpit.rs:183-200`) maps each id of `open_questions` to its **latest** opening event, then sorts by that event's `seq`. `owner` (`:204-216`) renders exactly that list. prd §5: `questions` follows `owner`'s order and takes every field from that latest event. `needed` is private; expose what you need from `cockpit.rs` (a function returning the open opening events without the park) rather than re-deriving the order in `status.rs`. `describe` (`src/cli/status.rs:402`) takes the *first* event; do not copy it.
- **The five opening kinds**: `opens` (`src/cockpit.rs:156-167`): `SliceBlocked { slice, block_id: Some, question: Option }` → `at: "slice"`, `n: slice`; `CheckpointBlocked { n, block_id, question }` → `"checkpoint"`, `n`; `CloseoutBlocked { phase, block_id, question }` → `"closeout"`, `n: null`; `MergeRefused { block_id, detail }` → `"merge"`, `n: null`, `text: checkpoint::refusal_summary(detail)` (`src/checkpoint.rs:969`), as `cockpit::line` does (`src/cockpit.rs:62-66`); `AskOpened { id, text }` → `"preflight"`, `n: null`. A `SliceBlocked` with `question: None` gives `text: ""`.
- **A run that is over has none**: `owner` returns nothing once `phase.is_over()` (`src/cockpit.rs:205-207`). `questions` is `[]` then, never `null`.
- **The answer command**: `factory answer` takes `--feature <NNN>` for `BLOCK-n` and `--run <id>` for `ASK-n` (`src/cli/answer.rs:23-28`, routed at `:39-48`), the text is `trailing_var_arg` (`:20-21`), and the repo comes from the current directory (`:31-33`). So the argv puts every flag before the id and leaves the text to be appended last: `["factory","answer","--feature","<NNN>","<BLOCK-n>"]`, `["factory","answer","--run","<run id>","<ASK-n>"]`.
- **Why the FR-3 test needs real runs**: `factory answer BLOCK-n` looks for a `kind: block` entry in `docs/features/*/log.jsonl` (`src/cli/answer.rs:70-87`) and the slice file (`:125-126`); event lines alone get "no BLOCK-1 under…" and exit 1. An answered run with no live driver spawns one (`:380-381`).
  - BLOCK: extend `status_json_carries_the_board_and_what_the_owner_owes` (`tests/cockpit.rs:273`). It parks a stubbed run on `BLOCK-1` and already answers it at the end with `factory answer BLOCK-1 ANSWER`; replace that call by the JSON's argv plus `ANSWER`, run from `fx.path()`. Keep its existing asserts.
  - ASK: follow `a_preflight_answer_logs_one_ask_entry_and_a_retry_adds_none` (`tests/loop.rs:1202`): `Run::create` with this test's pid (so the answer finds a live driver and spawns none), `emit(AskOpened)`, `set_phase(WaitingOwner)`.
- **The listing**: with no run id, `status` takes `cockpit::live_runs` (`src/cli/status.rs:76`), filters by `--here`, then opens each run. `live_runs` (`src/cockpit.rs:311-316`) is `list_runs` minus runs whose phase is over; `list_runs` sorts by `started_at` then id (`src/run.rs:561-565`). prd §6: with `--recent`, one list in that order, live runs plus over runs whose `updated_at` is within 7 days. Keep `live_runs` as is: `resolve`, `list_live` and `print_live` use it.
- **Clap**: `StatusArgs` (`src/cli/status.rs:17-30`). `--json` has `conflicts_with = "line"` (`:22`), `--here` has `conflicts_with = "run"` (`:28`). `--recent` gets `requires = "json"` and `conflicts_with = "run"`. Its doc comment is the `--help` line; say "runs that ended in the last 7 days".
- **Time**: `updated_at` is RFC 3339 UTC; `clock::from_rfc3339` and `clock::unix_now` (`src/clock.rs`). A fixture writes `updated_at` 2 and 8 days before now from `clock::rfc3339(unix_now() - days * 86400)`.
- **Deliberate non-change**: prd §6, "A `--since <days>` knob lost". The 7 days is a constant.
- **Deliberate non-change**: prd §11, "Not handled: a slow `--recent` call". No lighter record for ended runs.
- **Deliberate non-change**: prd §11, "Any code change to the Claude Code cockpit mod". It calls `--here --json` without `--recent` and must keep seeing live runs only.

## Tasks

### 1. Questions apart — high — blocked by: —
**Source**: §5 FR-3: "Each run object gains `questions`, one per open id, in the order `owner` lists them … `id` … `at` … `n` … `text` … `answer`: the argv to run from `repo`, with the answer text appended as the last element."

`src/cockpit.rs` (expose the open opening events in `needed`'s order), `src/cli/status.rs` (the `Question` struct and the field on `RunJson`).

**Acceptance:** a unit test in `src/cockpit.rs` or `src/cli/status.rs` builds events opening a slice block, a checkpoint block, a refused merge, a closeout block and an ask, re-opens one id later, answers another, and pins the `questions` list: order by latest opening `seq`, the answered id gone, each `at`, `n`, `text` and `answer` argv exact. A run that is over gives `[]`.

### 2. The answer argv runs — high — blocked by: 1
**Source**: §5 acceptance: "a test reads `questions` and runs each `answer` argv with a text appended, from `repo`. Each exits 0, writes its answer file (`src/answers.rs`), and the next `--json` no longer lists that id."

`tests/cockpit.rs` (the BLOCK case, extending the existing test) and `tests/loop.rs` or `tests/cockpit.rs` (the ASK case), as Current code says. Run the argv's tail through the test's `factory` helper with the text appended, so the test binary is the one under test.

**Acceptance:** both cases exit 0, the run's `answers/BLOCK-1.json` and `answers/ASK-1.json` exist, and the next `--json` has no question with that id. `status_json_carries_the_board_and_what_the_owner_owes` keeps its existing asserts.

### 3. `--recent` — high — blocked by: —
**Source**: §6 FR-4: "`factory status --json --recent` also lists the runs whose phase is over and whose `updatedAt` is within 7 days of now. The window is fixed. It combines with `--here`. It takes `requires = "json"` and `conflicts_with = "run"` … Without the flag the listing is unchanged."

`src/cli/status.rs` (the flag and the listing), `src/cockpit.rs` only if a helper beside `live_runs` reads cleaner.

**Acceptance:** a test `recent_adds_runs_that_ended_this_week` writes fixture runs live, finished 2 days ago and finished 8 days ago: `--json --recent` lists the first two in start order, `--json` lists only the live one, and `--json --recent --here` from outside the repo lists none. `--recent` alone and `<id> --json --recent` both exit non-zero with clap's message.

### 4. Verify — medium — blocked by: 2, 3
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 5. Prove — at the checkpoint — blocked by: 4
prd §8 rows **FR-3** and **FR-4**: run each `questions[].answer` argv with a text appended from the run's repo, and keep both answers' transcripts, the answer files and the next `--json`; run `factory status --json --recent` and `factory status --json` over the live, 2-day and 8-day fixture runs, plus the two refused flag mixes, and keep the transcripts.

**Acceptance:** after the checkpoint, this slice's Progress holds those transcripts and names the tests that pin them.
