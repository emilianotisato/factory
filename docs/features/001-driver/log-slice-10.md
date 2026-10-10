## Traps

- **A test that waits on a phase and then reads the event tail races the driver (2026-10-10)**: `state.json` is saved before the event that explains it is appended. `scheduler::finish` used to run `report::compute` in that gap, and `tests/checkpoints.rs::two_dead_checkpoint_workers_block_the_run_and_the_answer_spawns_a_third` read a tail with no `run_finished` once in three runs. The gap is now one append; a new test should wait on the event (`kinds().last() == "run_finished"`), not on the phase.

## Blockers

## Decisions

### AD-1001 — How does `factory wait` tail `events.jsonl`? · slice-10 · 2026-10-10

**Chose:** a 2-second poll (`cli::wait::POLL`) that re-reads `state.json` and `events.jsonl`, napping no longer than the time left to `--timeout`.
**Over:** inotify through the `notify` crate, which the slice offers as the other option.
**Because:** the driver appends a few lines an hour; a 2 s lag on a question the owner answers in minutes costs nothing, and `notify` would be a new runtime dependency for it.
**Undo:** swap the `sleep` in `cli::wait::wait` for a watcher.
**Review:** no

### AD-1002 — How does a stopped run end, and does `wait` return on it? · slice-10 · 2026-10-10

**Chose:** `run_stopped {slices, weighted_tokens, duration_minutes}` is the last event of a run ended by `factory stop`, in place of `run_finished` (fields from `report::compute`, all `#[serde(default)]`), and it is cockpit-relevant: `wait` prints `stopped: N slice(s) done — factory run NNN resumes` and exits 0. `factory stop` on a run with no live driver writes the same state (`stopped: true`, phase `finished`) and the same event, where slice 02 set `finished` silently. The stop path's tests in `tests/loop.rs` and `tests/run.rs` now end on `run_stopped`.
**Over:** `run_stopped` as progress, which the slice's list of `wait` kinds implies ("everything else is progress"). A cockpit waiting on a stopped run would then never get an event; `wait` would fall through to "the run is over" with nothing to say why.
**Because:** task 3 names `run_stopped` but not its payload, and the list of `wait` kinds was written before it existed; the skill needs one line that says the run ended on a stop and how to resume it.
**Undo:** drop `RunStopped` from `cockpit::is_cockpit`; its payload is one variant in `events.rs`.
**Review:** decide

### AD-1003 — Where do `wait` and `status` learn which blocks and asks are open? · slice-10 · 2026-10-10

**Chose:** from the run's own events: every `slice_blocked`, `checkpoint_blocked`, `merge_refused`, `closeout_blocked` and `ask_opened` id, minus those a later `block_answered` or `ask_answered` closes (`cockpit::open_questions`).
**Over:** the board's `openBlocks` from `docs status --json`: a `node` call on every 2 s poll, and asks are not on the board.
**Because:** `factory answer` writes those answer events, so the event log is the same source the answer path uses; the slice does not say where `BLOCK-n still open` comes from.
**Undo:** one function in `cockpit.rs`.
**Review:** no

### AD-1004 — What does the cockpit do with a budget park, whose driver exits by design? · slice-10 · 2026-10-10

**Chose:** a run whose last `run_parked` is newer than its last `run_started`/`run_resumed`, in phase `waiting-owner`, is not a dead driver (`cockpit::parked_on`): `wait` relays `parked: budget X/Y` once, then exits 3 at the timeout with `still running: waiting-owner, parked: budget X/Y, no open question`, and `status` marks the budget `(parked)` instead of printing `driver dead`. The skill asks the owner for a new budget on `parked: …` and runs `factory run NNN --budget <n>` on an answer, then waits again either way.
**Over:** exit 4 `driver dead; run: factory run NNN` (the slice's rule for a dead pid), which would send the cockpit to restart a run that re-parks at once with a second `run_parked`; and the slice's "on `paused`/`parked` relay as information and keep waiting", which on a budget park waits forever, since nothing but `--budget` resumes it (AD-703).
**Because:** slice 07 made the budget park exit its driver (AD-703); the slice's dead-driver rule and its skill recipe were written as if every park kept one.
**Undo:** drop the `parked_on` term from `cockpit::driver_dead` and the `parked:` bullet of the skill.
**Review:** decide

### AD-1005 — How does `wait` avoid acting on a half-written ending? · slice-10 · 2026-10-10

**Chose:** the driver saves `state.json` before it appends the event that says why, and exits after both. So `wait` reads the state first and the events second, re-reads up to 20 times at 100 ms when the phase is over but no `run_finished`/`run_stopped`/`run_failed` is on disk yet, and re-reads once before it believes a dead pid. `scheduler::finish` and `mark_stopped` now compute the report *before* they save the phase, so the gap between the two writes is one file append instead of a database query.
**Over:** emitting the event before the phase: a reader that waits on `run_finished` and then asserts phase `finished` (the cockpit test, and the skill) would race the other way.
**Because:** without it a `wait` landing between the two writes prints an old block line as the run's end, or exits 4 on a driver that has just finished cleanly. The query in the gap also cost one red inner run: `tests/checkpoints.rs::two_dead_checkpoint_workers_…` waits on phase `finished`, then read an event tail with no `run_finished` yet.
**Undo:** the `rereads` counter in `cli::wait::wait`; the order of three statements in each of `finish` and `mark_stopped`.
**Review:** no

### AD-1006 — What do `wait` and `status` print on a run that is already over, or given no run? · slice-10 · 2026-10-10

**Chose:** `wait` on a finished, stopped or failed run with nothing past the cursor prints its ending line again and exits 0; it never blocks. `factory status` with no run id, or an unknown one, lists the live runs of every repo (`no live runs under …` when there are none) and exits 1. Line 6's utilisation is the `rateLimitType` window of the last worker's `rate_limit_event` (`unifiedWindows.<type>.utilization`, else the fullest window), now kept as `stream::RateLimit.utilization_pct`.
**Over:** blocking to the timeout on an ended run (a reattached cockpit would sleep 25 minutes on a finished run before it learnt the PR URL); `status` with no id exiting 0.
**Because:** the slice gives the exit code for a missing id but not the reattach case, nor where the utilisation sits in the stream.
**Undo:** the `is_over` branch of `cli::wait::wait`; the `Ok(1)` in `cli::status::status`.
**Review:** no

### AD-1007 — Long wait or 25-minute poll, with the slice-entry test not run? · slice-10 · 2026-10-10

**Chose:** the skill runs `factory wait <id> --timeout 1500` in a background Bash, the 25-minute poll §9 names as the fallback, and treats exit 3 as "wait again". The test (`--timeout 7200` in a background Bash, does the session wake?) is in `qa-guide.md` FR-7 step 2 for the owner, and the result is noted in `prd.md` §20 and this slice's Progress as "not tested".
**Over:** running the test at slice entry as the slice says. An unattended builder runs under `cargo test` and a subagent, with no interactive Claude Code session whose wake-up could be watched; `/deliver` told this builder not to try.
**Because:** the poll works whether or not a long background Bash wakes the session; the long wait only saves the session a wake every 25 minutes. Switching later is one number in the skill and one needle in `tests/cockpit.rs`.
**Undo:** `--timeout 1500` → `--timeout 7200` in `skills/factory-deliver/SKILL.md` and the test.
**Review:** decide
