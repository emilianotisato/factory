# Slice 01 — Passes at once

**Source**: [`prd.md`](prd.md) → §4 FR-2. **Depends on**: — (independent; slice 03's collect calls the runner this slice writes)
**Created**: 2026-10-15

**Progress**:
- **Built.** `review::run_passes` starts every pass in its own scoped thread, writes each progress line as it starts, and returns a `Ran` (outcome or error, elapsed) per pass in pass order. The CLI builds each prompt in a closure, keeps `body()`, the section format and the exit rule, and prints once every pass returned. The signal handler now kills every non-empty slot of `CHILD_GROUPS` (256 atomics, AD-2); each `run_logged` claims a slot as `-1` before the spawn and clears it on drop. The stub reads its pass number from the `review-N.diff` in its prompt: its result ends with `(replayed for pass N)`, `unlink_stream_on_call` and the pid file (`cursor-agent-N.pid`) key on it, and `sleep_secs` sleeps then replays normally.
- **Tests.** `the_passes_run_at_once_and_print_in_pass_order` (new); the pass-2 call found by its `review-2.diff` path; `a_signal_to_factory_kills_every_reviewer_too` runs three passes. Mutating the handler to kill only the first slot fails the signal test with two reviewers left alive.
- **Inner loop.** 2 runs, 1 red (rustfmt on the new tests), about 2 min.
- **Deviations.** None from the slice.

**Slice**: After this slice, `factory review second --diff` starts every pass at once, each in its own child process with its own log files, waits for all of them, then prints the `## Pass n` sections in pass order and the `log:` line. Exit codes stay as they are: 0 when any pass counted, 2 when none did. A Ctrl-C or SIGTERM to factory still kills every reviewer. The pass runner lives in the `review` library, so slice 03's `factory review collect` calls it instead of the CLI.

**Highest risk**: task 2. The signal handler today tracks one reviewer, and a parallel run that leaks reviewers past a killed factory leaves Cursor sessions running for up to 900 s each.

## Current code / environment

- **The sequential loop**: `src/cli/review.rs:97-133`. For each pass it prints `review pass {n}/{N}: {files} ({lines} lines)` to stderr (`:100-107`), calls `run_one` (`:109`), turns the outcome into text with `body()` (`:111-114`), counts it (`:115`), prints `## Pass {n} — {files}{over} ({lines} lines, {secs} s, {model})` and the text (`:121-129`), and flushes (`:130`). Then `log: {dir}` (`:132`) and the exit code (`:133`). The probe runs once before the loop (`:91-95`) and stays once.
- **Per-pass files already exist**: `run_one` (`:136-162`) writes `review-{n}.diff` into the log dir; `review::run_pass` (`src/review.rs:345-367`) writes `review-{n}.jsonl` and `review-{n}.err`. Parallel passes collide on no file. `body()` (`src/cli/review.rs:169-218`) is a pure function of one `Outcome`.
- **One signal slot**: `static CHILD_GROUP: AtomicI32` (`src/review.rs:162-163`) holds one process group. `run_logged` stores the child's pid there (`:229`), and `Watched` zeroes it on drop (`:165-171`). The handler `forward` (`:173-184`) kills that one group, resets to `SIG_DFL` and re-raises. `forward_signals` installs it once (`:186-204`) and leaves a signal factory started out ignoring (`nohup`) alone. With passes in parallel, each `run_logged` overwrites the slot, and the first pass to finish zeroes it.
- **The timeout kill is already per child**: `run_logged` kills `-(child.id())`, the child's own group (`:234-240`). Each pass keeps its own `timeout_secs` (900 by default, `src/config.rs:71-72`).
- **The stub `cursor-agent`** (`tests/stubs/cursor-agent`):
  - every review call replays the same `cursor-review.jsonl` (`:66`), so the text cannot tell passes apart. The three-pass test only counts the result's first line three times (`tests/review.rs:131-132`).
  - `unlink_stream_on_call: n` counts the `-p` lines in the call log (`:53-64`) and deletes `review-{call}.jsonl`. Under parallel calls that count is arrival order, not the pass number. The pass number is in the prompt, the last argv: `diff_prompt` names `review-{n}.diff` (`src/review.rs:371-377`).
  - every review call writes its pid to the one file `cursor-agent.pid` (`:51`), so parallel calls overwrite each other.
  - no key sleeps and then finishes normally. `sleep_after_assistant` (`:68-78`) sleeps 60 s and relies on the caller's timeout kill.
- **Tests that lean on call order**: `tests/review.rs:154-158` reads `reviews[1]` from the call log and expects `review-2.diff`. `a_pass_that_errors_is_unreviewed_and_the_others_still_run` (`:290-312`) uses `unlink_stream_on_call: 2` on three passes and `1` on one. `a_signal_to_factory_kills_the_reviewer_too` (`:371-405`) scopes the diff to `a, b`, one pass, and reads the one pid file. The `section()` helper (`:277-288`) finds a pass by its header, so it holds under any order.
- **The fixture**: `fixture(name, timeout)` (`tests/review.rs:17-33`) commits `a` 300, `b` 250, `c` 700, `d` 150 and `e` 100 lines on `feat/review`: three passes (`a, b`; `c` over 600, alone; `d, e`).
- **Deliberate non-changes**: *(prd §13 "Not handled: a configurable pass parallelism. Accepted 2026-10-14: no evidence Cursor refuses parallel passes.")* Every pass starts at once; no knob, no cap. *(prd §4 "Accepted cost: nothing prints until the slowest pass ends. Progress lines on stderr stay.")* No streaming of finished sections. *(prd §4 "A pass it refuses prints `unreviewed` like any failed pass today.")* No retry.

## Tasks

### 1. The passes run at once — high — blocked by: —
**Where**: `src/cli/review.rs:97-162`, `src/review.rs`.

**Source**: §4 FR-2: "All passes start at once, each its own child process and log, as today. The command waits for every pass, then prints the sections in pass order. Exit codes are unchanged: 0 when any pass counted, 2 when none did." And: "Progress lines on stderr stay."

Move the pass runner into the `review` library: it takes the passes and a prompt per pass, runs each in its own thread (`std::thread::scope` fits: every pass borrows the config and the dir), and returns each pass's outcome with its elapsed time, in pass order. Write the stderr progress line as each pass starts. The CLI keeps `body()`, the section format and the exit rule, and prints once every pass has returned. Doc mode is one pass and goes through the same runner.

Slice 03's collect calls this runner and needs each pass's header, text and whether it counted: return enough for that, without the CLI's writer.

**Acceptance:** task 3's timing test; the existing `tests/review.rs` tests stay green.

### 2. A signal still kills every reviewer — high — blocked by: —
**Where**: `src/review.rs:162-241`.

**Source**: §4 FR-2: "each its own child process and log, as today". Today a signal to factory kills its reviewer (`a_signal_to_factory_kills_the_reviewer_too`); that must hold for every pass that runs at once.

The trap: `CHILD_GROUP` is one slot (`:163`) that each `run_logged` overwrites and each `Watched` zeroes (`:165-171`). Give each running child its own slot, cleared when that child ends, and have `forward` kill every non-empty slot. The handler must stay async-signal-safe: atomics and `kill`/`signal`/`raise` only, no lock and no allocation.

**Acceptance:** task 3's signal test kills every reviewer of a three-pass diff.

### 3. The stub tells passes apart, and the tests prove the overlap — high — blocked by: 1, 2
**Where**: `tests/stubs/cursor-agent`, `tests/review.rs`.

**Source**: §4 FR-2 acceptance: "With a stub `cursor-agent` that sleeps 2 s per pass, a three-pass diff finishes in under 4 s of the test's wall clock. Its stdout holds the three `## Pass n` sections in pass order, each with the text the stub replayed for that pass, then the `log:` line."

- **Stub keys**, documented in its header:
  - a key that sleeps n seconds, then replays and exits 0 as a normal call does;
  - each review call reads its pass number from the `review-N.diff` path in its prompt, and its replayed text says which pass it was, so a section printed under the wrong header fails a test;
  - `unlink_stream_on_call` keys on that pass number, not the arrival count;
  - each review call writes its pid to a file of its own, by pass number.
- **Tests**:
  - three passes with the sleep key at 2 s finish in under 4 s, measured around the `factory` call; stdout holds `## Pass 1`, `2`, `3` in that order, each holding its own pass's text, then `log:`;
  - `tests/review.rs:154-158` finds the pass-2 call by its `review-2.diff` path, not by its index in the log;
  - the signal test runs a three-pass diff and checks that every pass's pid is dead after SIGTERM to factory.

**Acceptance:** the three tests above, plus `a_pass_that_errors_is_unreviewed_and_the_others_still_run` green with the pass-keyed unlink.

### 4. Verify — medium — blocked by: 1, 2, 3
`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 5. Prove — at the checkpoint — blocked by: 4
Rows from [`prd.md`](prd.md) §11 this slice proves, below the UI:

- **FR-2**: "run `factory review second --diff` over three passes with a slow stub: the passes overlap in time, the output order holds" — task 3's timing test, its transcript with the wall-clock figure and the three section headers.

**Acceptance:** after the checkpoint, Progress holds the timing test's transcript.
