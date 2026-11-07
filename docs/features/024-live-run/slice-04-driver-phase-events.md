# Slice 04 — Driver phase events: gate start, CI wait, no checks, empty fix

**Source**: [`prd.md`](prd.md) → §7 FR-5 (`gate_started`, `ci_waiting`, `ci_no_checks`, `fix_empty`). **Depends on**: 01 (the event kinds it declares). Independent of 02, 03, 05, 06, 07: it only emits, and every reader of these events works from fixture events.
**Created**: 2026-11-07

**Progress**:
- Built: `gate_started` in `gate_once` after the cache miss, before the transcript is created; `fix_empty {n, attempt, session_id}` beside the gate fix's "changed nothing" line, and `fix_empty {round}` in `nothing()` for a CI fix; `ci_no_checks {minutes}` on `Verdict::NoChecks`; `ci_waiting {checks}` in `watch_ci` with its own dedup (`running`, reset with `waiting` on a rerun and after a fix round), the set being the non-stale rows `ci::class` calls `Pending`.
- Tests: `an_empty_fix_is_an_event` (one in `tests/checkpoints.rs`, one in `tests/closeout.rs`), `ci_waiting_emits_on_a_change_of_checks_only`, `checks_never_reported_past_the_grace_are_one_event`. The red-red-green test asserts a `gate_started` before each `gate_run` and no `fix_empty`. The in-grace test asserts no `ci_no_checks` and no `ci_waiting`.
- Inner loop: 5 runs, 4 red, about 7 min. The reds were older tests that pin the exact event list (`tests/checkpoints.rs`, `commits.rs`, `loop.rs`, `run.rs`). Each gained `gate_started` before its `gate_run`, and the counts and indexes in `run.rs` moved by one.
- Deviation: a set with no running check emits no `ci_waiting` (AD-4, fyi).
- Prove (this slice's FR-5 rows): left for the checkpoint.
- **Proved (checkpoint 1):** FR-5, this slice's part: `ci_waiting_emits_on_a_change_of_checks_only` and both `an_empty_fix_is_an_event`, 3 passed — `cp-1/fr-5-phase-events.txt`

**Slice**: After this slice, the driver phases that are silent in `events.jsonl` today write an event: a gate's start (before it runs for 9 to 33 min), each change in the set of CI checks still running, a PR that reports no checks, and a fix worker (gate or CI) that changed nothing to commit. `run_waiting.reason` landed in slice 01; the collect events are slice 02's.

## Current code / environment

- **Gate start**: `gate_once` (`src/checkpoint.rs:439`) returns early on a cache hit with `gate_cached` (`:460-494`). After the miss it builds `rel = "{GATES_DIR}/{n}-{attempt}.log"` (`:496`), makes the dir, and calls `File::create` (`:501`). prd §7: emit `gate_started {n, attempt, cmd, log}` "after the cache miss and before `File::create`", so `log` is `rel` and `cmd` is the `gate` string. `gate_run` follows at `:547`.
- **Empty gate fix**: `fix_once` (`src/checkpoint.rs:579`) says "fix worker {session_id} changed nothing to commit" on `Ok(None) if !dead && limit.is_none()` (`:621-625`). `session_id` (`:607`) and `attempt` are in scope. Emit `fix_empty {n, attempt, session_id}` in that arm, beside the line.
- **Empty CI fix**: `nothing()` (`src/closeout.rs:636-648`) says "ci: fix worker of round {round} changed nothing to commit" for `Closer::CiFix { round, .. }` and returns. Emit `fix_empty {round}` there. `nothing()` has `run: &mut Run`.
- **CI waiting**: `watch_ci` (`src/closeout.rs:1251-1450`). `rows` is declared per poll (`:1307`) and filled from `Checks::Rows` (`:1363-1365`); it stays empty on a head lag, `Checks::None`, or an unreadable view. The driver-log line dedups on the whole `why` text through `waiting: Option<String>` (`:1295`, `:1442-1445`), which is reset to `None` on a rerun or a new push (`:1397`, `:1414`). prd §7: `ci_waiting {checks}` "emitted only when the set of running check names changes … rebuilt from `rows` … with `ci::class` (`ci.rs:196`). Pending reasons that are not checks are not in `checks`: head lag, rerun, grace."
  - Keep a separate `Option<BTreeSet<String>>` of the last emitted set, starting `None` and reset wherever `waiting` is reset, so the first wait of a watch emits once. `checks` is the sorted names of rows whose `ci::class` is `Class::Pending` (`src/ci.rs:187-205`).
- **No checks**: `Verdict::NoChecks` (`src/closeout.rs:1376-1384`) logs "no checks reported … nothing to watch" and returns `Ok(None)`. Emit `ci_no_checks {minutes}` from `since.elapsed()` there.
- **Stubs that drive these**: the claude stub's fix role writes `step.touch ?? ['src/lib.txt']` (`tests/stubs/claude:226-230`), so a plan step `{"outcome": "fix", "touch": []}` (or the CI-fix equivalent) changes nothing. The gh stub scripts polls as space-separated rows `<kind>:<check>[/<workflow>][@<sha>]` with kinds `pass | red | flaky | pending | …` (`tests/stubs/gh:10-20`), through `Fixture::script_ci` (`tests/harness/mod.rs:481`).
- **Tests to extend**: `red_red_green_runs_two_fix_workers_and_three_gates_before_the_cp` (`tests/checkpoints.rs:883`), `a_red_check_gets_one_fix_round_then_green` (`tests/closeout.rs:1042`), `no_checks_right_after_the_push_are_waited_on_for_the_grace` (`tests/closeout.rs:1523`, the in-grace case: no `ci_no_checks` there, since its verdict is `Pending`).
- **Deliberate non-change**: the driver-log lines stay as they are; `status`, `wait` and `report` are unchanged (prd §1 "Left alone").

## Tasks

### 1. `gate_started` — high — blocked by: —
**Source**: §7 FR-5: "`gate_started {n, attempt, cmd, log}`, in `gate_once` after the cache miss and before `File::create` (`checkpoint.rs:495-501`)."

`src/checkpoint.rs`.

**Acceptance:** `gate_started_comes_before_gate_run`: extend `red_red_green_runs_two_fix_workers_and_three_gates_before_the_cp` to assert that each `gate_run` has a `gate_started` with the same `n`, `attempt` and `log` and a smaller `seq`, and that the count of `gate_started` equals the count of `gate_run` (a `gate_cached` has none).

### 2. `fix_empty` for a gate fix and a CI fix — high — blocked by: —
**Source**: §7 FR-5: "`fix_empty {n, attempt, session_id}` in `fix_once` (`checkpoint.rs:621-625`), and `fix_empty {round}` for CI in `nothing()` (`closeout.rs:643-648`)."

`src/checkpoint.rs`, `src/closeout.rs`.

**Acceptance:** `an_empty_fix_is_an_event`, two tests: a checkpoint run whose fix step has `touch: []` emits one `fix_empty` with that `n`, `attempt` and the fix worker's `session_id`; a CI round whose fix step has `touch: []` emits one `fix_empty` with `round: 1` and no `n`. A fix that commits emits none (assert on the existing red-red-green test).

### 3. `ci_waiting` on a change of checks, and `ci_no_checks` — high — blocked by: —
**Source**: §7 FR-5: "`ci_waiting {checks}`, in `watch_ci` beside the waiting line (`closeout.rs:1442-1444`), emitted only when the set of running check names changes. Today's line dedups on the whole `why` text, so the event needs its own dedup", "`ci_no_checks {minutes}` at `closeout.rs:1375-1384`."

`src/closeout.rs` `watch_ci`.

**Acceptance:** `ci_waiting_emits_on_a_change_of_checks_only`: polls scripted `pending:test`, `pending:test`, `pending:test pending:lint`, `pass:test pending:lint`, then green give exactly three `ci_waiting` events with `checks` `["test"]`, `["lint","test"]`, `["lint"]`, in that order. A watch whose PR reports no checks past the grace emits one `ci_no_checks`: the grace is `GRACE_POLLS * interval()` (`src/ci.rs:22`, `:41`), so with `FAST` (`FACTORY_CI_INTERVAL=1`, `tests/closeout.rs:304`) script more than 5 `none` polls.

### 4. Verify — medium — blocked by: 1, 2, 3
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 5. Prove — at the checkpoint — blocked by: 4
prd §12 row **FR-5**, this slice's part: run `ci_waiting_emits_on_a_change_of_checks_only` and `an_empty_fix_is_an_event`, and keep the transcript. (`review_passes_report_as_they_finish` is slice 02's.)

**Acceptance:** after the checkpoint, this slice's Progress names that transcript.
