# Slice 01 — The run record: where it belongs and its progress as fields

**Source**: [`prd.md`](prd.md) → §2, §3 FR-1, §4 FR-2, §8. **Depends on**: — (first; slice 02 adds to the same struct and reuses these fixtures)
**Created**: 2026-11-06

**Progress**:
- **Built**: `RunJson` (`src/cli/status.rs`) is `camelCase` and carries the eight placement keys and the nine progress fields. `run_json` reads the docs `Status` once for `slices`, `slug` and the PRD folder. When the board is unavailable it scans `docs/features/{NNN}-*` and then `docs/archive/{NNN}-*`, and `title` comes from that folder's `prd.md` frontmatter. `checkpoints_line` and the field both read `checkpoints_done` and `last_gate`, and the quota line and `budget` both read `budget_used`. `ci(events, pr, over)` is pure, and its unit test is `ci_reads_the_latest_verdict_and_a_missing_one_by_whether_the_run_is_over`. The tests are `status_json_says_where_a_run_belongs` and `status_json_draws_each_run_state_as_fields`, which covers ten states and checks the board's `slices`, `checkpoints:`, `quota:`, `PR:`, `driver dead` and `(stopped)` text against the fields. `status_json_carries_the_board_and_what_the_owner_owes` only gained asserts (`repo`, `slug`, `slices`).
- **Inner loop**: 2 runs, 1 red, about 2 min. The red run was clippy `cloned_ref_to_slice_refs` in the CI unit test. Both behaviour tests passed on their first run.
- **Deviations**: none. AD-1: a slice with no recorded status reads as `"pending"`.
- **Prove (left for the checkpoint)**: rows FR-1, FR-2 and all. Their transcripts are not taken yet.
- **Prove FR-1** (checkpoint 1): `cp-1/fr1-status-json.txt`. `factory status --json` over the two fixture runs, read back against `state.json`, the `prd.md` title and the last `events.jsonl` seq. All match, and the repo with no `docs/features` gives `slug`/`title` `null`. Pinned by `status_json_says_where_a_run_belongs` (`cp-1/test-status_json_says_where_a_run_belongs.log`).
- **Prove FR-2** (checkpoint 1): `cp-1/fr2-status-json-per-state.txt`. `factory status <id> --json` once for each of the ten states, with the fields and the `slices`/`checkpoints:`/`quota:`/`PR:` board lines side by side, and they agree. Pinned by `status_json_draws_each_run_state_as_fields` (`cp-1/test-status_json_draws_each_run_state_as_fields.log`).
- **Prove all** (checkpoint 1): `cp-1/all-status-id-json.txt`. Plain, `--json` and `--line` over the stubbed run parked on BLOCK-1: the six old keys are there, and `board[1..]` matches the plain output. Pinned by `status_json_carries_the_board_and_what_the_owner_owes` (`cp-1/test-status_json_carries_the_board_and_what_the_owner_owes.log`), whose asserts from `main` are all still there.

**Slice**: After this slice, every run object in `factory status --json` also carries where it belongs (`repo`, `branch`, `slug`, `title`, `startedAt`, `updatedAt`, `elapsedSeconds`, `seq`) and its progress as fields (`slices`, `budget`, `checkpoints`, `pr`, `ci`, `pausedUntil`, `parked`, `driverDead`, `stopped`). Schema stays 1, and the six existing keys print exactly as today. `questions` and `--recent` are slice 02's.

## Current code / environment

- **The struct to grow**: `src/cli/status.rs:42-50` — `RunJson` derives `Serialize` with six snake-free keys. Add the new fields with `#[serde(rename_all = "camelCase")]` on the struct, or a rename per field; the six old keys are single words, so a struct-wide rename leaves them unchanged. prd §3: "New keys are camelCase, like `factory docs status --json`".
- **The builder**: `run_json` (`src/cli/status.rs:123-134`) loads `run.events()` once, then calls `line_of` and `board(run)`. Both of those call `Docs::open(&s.repo).and_then(|d| d.status(&s.feature))` again (`:180`, `:236`). Read the docs `Status` once in `run_json` for `slices`, `slug` and the PRD path; do not add a third open. `board()` and `line_of` keep their own calls: their plain and `--line` callers have no `run_json`.
- **Slug and title**: the docs `Status` has `slug` and `dir` (`src/docs/mod.rs:136-139`); `dir` is repo-relative (`retro::feature_dir` joins it, `src/retro.rs:309-313`). The title is the `title` key of `prd.md`'s frontmatter, parsed by `docs::frontmatter::parse` (`src/docs/frontmatter.rs:65`, `data: Option<Vec<(String, Value)>>` at `:46`). prd §3: `null` when no feature folder or `prd.md` is found, or it does not parse; the run still lists.
- **Elapsed**: `span(s)` (`src/cli/status.rs:204-213`) is already start-to-now, or start-to-last-update for a run that is over. Reuse it.
- **Seq**: `Event.seq` (`src/events.rs:277`); `run.events()` returns readable events only, so the last one's `seq`, `0` when empty.
- **Budget**: `quota::weighted_sum(run.db(), id)?.ceil() as u64` (`src/cli/status.rs:280`), limit `s.budget_weighted_tokens`. Same rounding as the board line.
- **Checkpoints and gate**: `checkpoints_line` (`src/cli/status.rs:371-399`) counts `CheckpointDone` and takes the latest of `GateRun` / `GateCached` for red or green. Factor the two facts out so the line and the field read one function; the line's text must not change.
- **CI**: events `CiGreen`, `CiRed`, `CiRerun`, `CiFixCommitted` (`src/events.rs`, after `PrOpened`). A run with no checks reported emits no CI verdict at all (`Verdict::NoChecks`, `src/closeout.rs:1376`), which is why prd §4 gives `"none"` for a run that is over with a PR and no verdict.
- **Park, pause, dead driver**: `cockpit::park` (`src/cockpit.rs:252-262`) returns the `RunParked` event; its `reason` is the field. `line_of` (`src/cli/status.rs:172-178`) already extracts it the same way. `cockpit::driver_dead` (`src/cockpit.rs:264-272`). `pausedUntil` only while `paused_until > clock::unix_now()`, formatted with `clock::rfc3339` (`src/clock.rs:47`).
- **Stopped**: `set_stopped` sets `stopped` and phase `finished` together (`src/run.rs:377-381`).
- **The existing contract test**: `status_json_carries_the_board_and_what_the_owner_owes` (`tests/cockpit.rs:273`) pins `schema`, `factory`, `id`, `feature`, `phase`, `line`, `board` and `owner` on a stubbed parked run. prd §8's "all" row: it must pass with its existing asserts unedited. Adding asserts to it is fine.
- **The fixture**: `write_live_run` (`tests/cockpit.rs:347-363`) writes a bare `state.json` (phase `building`, pid 0, no events, no docs). For FR-2's states, write `state.json` plus `events.jsonl` lines per state; check one existing event line in a stubbed run's folder for the exact JSON shape (`kind`, `seq`, `at`) before hand-writing them. A pid-0 `building` run reads as driver dead; that is the driver-dead fixture for free, and every other state needs `phase` set to match.
- **Deliberate non-change**: prd §11, "Any code change to the Claude Code cockpit mod". Do not touch `mods/cockpit/`.
- **Deliberate non-change**: prd §11, "Not handled: a slow `--recent` call". No caching, no lighter record.
- **Deliberate non-change**: prd §3 accepted cost, the title is read from disk every call; do not cache it in `state.json`.

## Tasks

### 1. Where a run belongs — high — blocked by: —
**Source**: §3 FR-1: "Each run object gains … `repo`, `branch`, `slug`, `title`, `startedAt`, `updatedAt`, `elapsedSeconds`, `seq`."

`src/cli/status.rs`. Add the eight fields to `RunJson` and fill them in `run_json`. `repo` is `s.repo` as a string, `branch` and `title` and `slug` are `Option`s that serialize as `null` (no `skip_serializing_if`: a reader needs the key).

**Acceptance:** a test `status_json_says_where_a_run_belongs` reads one fixture run with a real PRD folder in its repo and pins all eight keys: `slug` and `title` from the folder and its frontmatter, `seq` equal to the last `events.jsonl` line's. A second fixture run whose repo has no `docs/features` folder lists with `"slug": null` and `"title": null`.

### 2. Progress as fields — high — blocked by: 1
**Source**: §4 FR-2: "`slices` … `budget` … `checkpoints` … `pr` … `ci` … `pausedUntil` … `parked` … `driverDead` … `stopped`", and its CI rule: "the latest of `CiGreen` → `"green"`, `CiRed` → `"red"`, a `CiRerun` or `CiFixCommitted` after the last verdict → `"pending"`. With a PR and no verdict yet: `"pending"` while the run is live, `"none"` once it is over."

`src/cli/status.rs`. `slices` is `Option<Vec<{n, status}>>`, `null` when the docs board is unavailable. `budget` and `checkpoints` are small serialized structs. Write the CI rule as one pure function over `(&[Event], pr: bool, over: bool)` so a unit test covers it without a run.

**Acceptance:** a test `status_json_draws_each_run_state_as_fields` writes one fixture run per state of prd §4's list (building with mixed slice statuses, blocked, parked on budget, paused, driver dead, shipped with CI green, CI red, finished with no checks, stopped, failed) and pins every FR-2 field for each. A unit test pins the CI function on: no PR → `null`, red then fix committed → `"pending"`, green → `"green"`, PR and no verdict live → `"pending"`, over → `"none"`.

### 3. The prose stays as it is — high — blocked by: 2
**Source**: §4 accepted cost: "A test that builds both from one fixture and checks they agree is the guard"; §8 row "all": "`id`, `feature`, `phase`, `line`, `owner` and `board` read as today".

`tests/cockpit.rs`. In `status_json_draws_each_run_state_as_fields`, for each state also assert the board's `checkpoints:`, `quota:` and `PR:` lines say what the fields say (the same counts, `red`/`green`, budget and URL). Leave `status_json_carries_the_board_and_what_the_owner_owes` with its existing asserts; add `repo`, `slug`, `slices` asserts to it if they help.

**Acceptance:** both tests green; `status_json_carries_the_board_and_what_the_owner_owes`'s existing assert lines are byte-identical to `main` (`git diff main -- tests/cockpit.rs` shows only added lines inside it).

### 4. Verify — medium — blocked by: 3
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 5. Prove — at the checkpoint — blocked by: 4
prd §8 rows **FR-1**, **FR-2** and **all**: run `factory status --json` over the fixture runs (FR-1, FR-2) and `factory status <id> --json` over the parked stubbed run ("all"), and keep each transcript.

**Acceptance:** after the checkpoint, this slice's Progress holds the three transcripts, and the test names that pin them.
