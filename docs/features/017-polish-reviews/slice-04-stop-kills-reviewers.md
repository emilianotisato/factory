# Slice 04 — Stop kills a running collect's reviewers

**Source**: [`prd.md`](prd.md) → §11 FR-9, §13. **Depends on**: 03 (this slice adds a cancel check to the collect options struct 03 introduces, and threads it through `run_passes`, which 02 and 03 already touched)
**Created**: 2026-11-12

**Progress**: _(written as it lands — what was built, the gate numbers, what a review caught)_

**Slice**: After this slice, `factory stop` during the driver's polish collect kills every running second-opinion reviewer within about a second. Each one reads `unreviewed (stopped)` in the collected file, and the driver returns `Stopped` as it does today. The CLI verbs pass no cancel check and behave as before.

## Current code / environment

- **The poll loop**: `run_logged` (`src/review.rs:224-259`) polls `child.try_wait()` every 50 ms and kills the process group (`libc::kill(-pid, SIGKILL)`) only when `started.elapsed() >= timeout` (`:250-255`), returning `Ended::Killed`. The cancel goes in this same loop, with the same group kill.
- **How a pass ends**: `Ended` has `Exited(Option<i32>)` and `Killed`. `body()` (`src/review.rs:540-…`) turns `Killed` into a timeout text. A stop is not a timeout, so give it its own ending. prd §11: "That pass reads `unreviewed (stopped)`".
- **Passes run in threads**: `run_passes` (`src/review.rs:416-475`) spawns one scoped thread per pass. The cancel check is shared across them, so it must be `Sync`, for example `Option<&(dyn Fn() -> bool + Sync)>`. prd §11: "read at most once a second, not every 50 ms poll". Throttle inside `run_logged` by elapsed time, not with a shared timer.
- **The driver's borrow**: `collected()` (`src/closeout.rs:337-416`) already lends `run` mutably to the `emit` closure for progress events (`:369-380`). A cancel closure cannot also borrow `run`. Read the state from disk instead: `run::load_state` (`src/run.rs:508`) on the run dir is read-only. That is also how it sees a `factory stop` written by another process.
- **After the collect**: `run.reload()` then `if run.state().stop_requested { return Ok(Some(Outcome::Stopped)) }` (`src/closeout.rs:205-209`). Keep it. With the cancel, it now returns within about a second of the stop instead of after `timeout_secs`.
- **Signal forwarding**: `Watched` and `forward` (`src/review.rs:163-220`) kill reviewer groups on Ctrl-C. They are unrelated to this, so leave them alone.
- **Existing patterns to reuse in tests**: the stub writes `cursor-agent-<n>.pid` per pass (`tests/stubs/cursor-agent:56-58`). `alive(pid)` and the leftover-reviewer check in `a_signal_to_factory_kills_every_reviewer_too` (`tests/review.rs:406-464`) show how to assert that no reviewer outlived the stop. Run fixtures reach `collect_started` and `review_pass_done` (`tests/run.rs:469-471`), and `factory stop` works on a fixture run (`tests/run.rs:675-700`).
- **Deliberate non-change**: the CLI verbs (`review second`, `review collect`) pass no cancel (prd §11). `factory stop`'s own behavior and output are unchanged.

## Tasks

### 1. A cancel check stops a pass — high — blocked by: —
**Source**: §11 FR-9: "`run_passes` takes a cancel check… `run_logged`'s poll loop kills the pass's process group when the check fires. That pass reads `unreviewed (stopped)`… The check is read at most once a second."

`src/review.rs`:

- `run_logged` takes an optional cancel check. It calls it at most once a second, and on `true` kills the group and returns a new `Ended::Stopped`.
- `run_pass` and `run_passes` pass it through.
- `body()` maps `Stopped` to `("unreviewed (stopped)\n", false)`.
- The collect options struct from slice 03 gains the check, and `second_opinion` (`src/collect.rs:137-202`) hands it to `run_passes`.
- The CLI callers pass `None`.

**Acceptance:** a unit or `tests/review.rs` test drives `run_passes` with a check that turns `true` after about a second, over a stub that sleeps 60 s. It returns within a few seconds, and every pass's text is `unreviewed (stopped)`.

### 2. The driver stops its collect on `factory stop` — high — blocked by: 1
**Source**: §11 FR-9: "The driver supplies one that reloads the run's `stop_requested`… the driver returns `Stopped` as today."

In `collected()`, build the check from the run dir with `run::load_state` and `stop_requested`. Pass it in the collect options.

**Acceptance:** `stop_kills_a_running_collects_reviewers`. A fixture run reaches polish with a stub reviewer that sleeps (`sleep_secs`), and `factory stop` is sent once `collect_started` is logged. Within a few seconds:

- no `cursor-agent-<n>.pid` process is alive;
- the run's `collect.md` names the passes `unreviewed (stopped)`;
- the run ends stopped.

### 3. Verify — medium — blocked by: 2
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q`

**Acceptance:** inner loop green, one commit.

### 4. Prove — at the checkpoint — blocked by: 3
prd §13 row **FR-9** (`stop_kills_a_running_collects_reviewers`). It is proven below the UI: the checkpoint runs it and keeps the transcript.

**Acceptance:** after the checkpoint, the transcript is in Progress.
