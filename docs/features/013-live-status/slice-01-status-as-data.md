# Slice 01 — Status as data: --json, --line, --here

**Source**: [`prd.md`](prd.md) → §3 FR-1, §4 FR-2, §5 FR-3. **Depends on**: — (the three FRs share `src/cli/status.rs`, so they are one slice; nothing else in the PRD writes that file)
**Created**: 2026-10-30

**Progress**:
- Built: `StatusArgs` gains `--json`, `--line`, `--here`. `glance()` with its `Glance` input is the pure line function (`src/cli/status.rs`), and `line_of` fills it from state, events, the board and `clock::local_hhmm` (new, `localtime_r`). `cockpit::owed_from` now filters `needed()` (open questions then the park), and `cockpit::owner` adds the dead-driver line, shared with `wait` through `driver_dead_line`. `cockpit::park` returns the parked event, and `parked_on` uses it. `list_live` is split into `live_runs` and `print_live`. The board's start-to-end span is the shared `span()`, so the board's output does not change.
- Tests: `the_glance_line_names_the_first_state_that_applies` (unit), `status_json_carries_the_board_and_what_the_owner_owes`, `here_keeps_this_repos_runs_and_a_listing_exits_zero` (new). The budget-park test also asserts `line` and `owner`, and `tests/cockpit.rs` now expects a no-id `factory status` to exit 0.
- Inner loop: 1 run, green, ~3 min (the test suite ran twice in it). One red targeted run of the cockpit tests: my JSON test called `wait` with a cursor of 0, which relayed the BLOCK instead of the finish. It now waits on the phase.
- Deviations: none from the FRs. AD-1 (fyi) covers how the flags combine.
- Prove: left for the checkpoint, rows FR-1, FR-2 (plus `factory status <id> --line` on a fixture run), FR-3.

**Slice**: Makes `factory status` a data source. `--json` prints the versioned envelope, `--line` prints one glance line per run, `--here` keeps this repo's runs, and the no-id listing exits 0. Plain `factory status [RUN]` prints exactly what it prints today. The mod that reads this JSON is slice 02's; it is built against the envelope §3 defines, so both slices can run at once.

## Current code / environment

- **The args**: `src/cli/status.rs:15-19`, `StatusArgs { run: Option<String> }`, wired at `src/main.rs:27,89`. The three flags go here.
- **The no-id path**: `src/cli/status.rs:23-26` calls `cockpit::list_live` and returns `Ok(1)`. `list_live` (`src/cockpit.rs:277-308`) filters by phase only and prints every repo's runs.
- **The board**: `board()` at `src/cli/status.rs:39-134` returns 8 to 10 lines; `status` prints at most ten (`:30`). FR-1 says `board` is these lines "unchanged", so `board()` is reused as is.
- **What the owner owes, minus the cursor**: `owed_from` (`src/cockpit.rs:178-198`) builds the list `owner` needs: every open question from its latest opening event, then the latest `RunParked` when parked, sorted by seq. Its one difference from `owner` is `owed.retain(|e| e.seq <= cursor)` at `:191`. `owed` returns nothing for a run that is over (`:171-173`). Each event becomes its string through `cockpit::line` (`:43`). The dead-driver string is `wait`'s, verbatim: `driver dead; run: factory run {feature}` (`src/cli/wait.rs:83`).
- **The fixture BLOCK**: `tests/cockpit.rs:17-18` holds `BLOCK_LINE`, the exact `wait` line of the scenario in `wait_relays_the_block_the_still_running_line_the_dead_driver_and_the_end` (`:129`). That scenario opens BLOCK-1 and later answers it (`:208`), so it holds the case FR-1's acceptance is about.
- **The old exit code is pinned**: `tests/cockpit.rs:191-192` asserts `factory status` with no id exits 1. FR-3 changes it to 0.
- **Elapsed**: the board's `elapsed` (`src/cli/status.rs:136-143`) prints `12m 34s` under an hour and is pinned by `elapsed_reads_as_minutes_then_hours` (`:300`). The line needs its own form, `12m` / `1h 02m`. Keep the board's.
- **Time**: `clock::rfc3339` (`src/clock.rs:47-56`) is UTC only, and there is no time crate. `libc` is a dependency (`Cargo.toml`), so `libc::localtime_r` gives local `HH:MM`.
- **Git top level**: `paths::repo_root` (`src/paths.rs:172`) runs `git rev-parse --show-toplevel` and errs outside a repo. A run's `repo` is stored canonicalized (`canonical`, `src/run.rs:569-572`).
- **The phase string**: `Phase::as_str` (`src/run.rs:69-80`), `Display` at `:87-90`. The park reason is `RunParked { reason, .. }` (`src/events.rs:83-87`).
- **Deliberate non-changes**: no structured board fields (prd §12: "Not handled: structured board fields"). No `--follow` (§12). `factory wait` and `cockpit.cursor` stay as they are (§1, §12). `no run <id>` still exits 1 (§5).

## Tasks

### 1. The glance line as a pure function — high — blocked by: —
**Source**: §4 FR-2: "The line is a pure function of a small input: phase, open ids, parked reason, pause instant, driver dead, slices done and total (or none), elapsed seconds." The table and its order, as corrected 2026-10-30: owner needed, parked, paused, driver dead, then the progress row as the fallback, and a run that is over goes straight to the over row.

`src/cli/status.rs` (or a small module beside it). A struct of those inputs and a function returning the line. Rows: `013 · BLOCK-1, ASK-2 open`, `013 · parked: budget`, `013 · paused until 14:30`, `013 · driver dead`, and the fallback `013 · {phase} · {n}/{m} done · {elapsed}`, which is the `building` row and, for a run that is over, the `finished` row. No board → drop the `n/m done` part. Elapsed is `12m` under an hour and `1h 02m` from one hour.

**The trap**: `·` is two bytes. The 60-character rule counts `chars()`, never `len()`. And the local `HH:MM` depends on the machine's time zone, so convert the pause instant to `HH:MM` at the edge (with `localtime_r`) and keep the function's test independent of `TZ`.

**Acceptance:** `the_glance_line_names_the_first_state_that_applies` calls the function once per table row and asserts each row's exact string. It also asserts that a BLOCK open while the phase is `building` shows the BLOCK row, that a dead driver in the `building` phase shows `driver dead`, and that no row exceeds 60 characters for feature `999` with three open ids.

### 2. `--json` with `owner` and `board` — high — blocked by: 1
**Source**: §3 FR-1: "`--json` prints one JSON object, with or without a run id: `{ "schema": 1, "factory": "0.12.0", "runs": [ … ] }`" with each run's `id`, `feature`, `phase`, `line`, `owner`, `board`; "`owner`: … open questions in seq order, then the park, then a dead driver last … They are not `wait`'s owed set."

`src/cli/status.rs`, `src/cockpit.rs`. `factory` is `env!("CARGO_PKG_VERSION")`. Split `owed_from` so the open-questions-then-park list exists without the cursor filter; `owed` keeps calling it and filtering, so `wait` does not change. `owner` is that list through `cockpit::line`, plus the dead-driver string last when `driver_dead` holds, and `[]` for a run that is over. `runs` is the named run, or the live runs the listing would show.

**The trap**: `wait` rereads state before it believes a dead driver (`src/cli/wait.rs:77-82`), because the driver saves `state.json` before the event that says why. `status` reads once, which FR-1 accepts; do not copy the settle loop.

**Acceptance:** `status_json_carries_the_board_and_what_the_owner_owes` drives the BLOCK scenario of `tests/cockpit.rs:129`. With BLOCK-1 open, `factory status <id> --json` parses, its `runs[0].board` equals the plain output line for line, and `owner` is exactly `[BLOCK_LINE]`. After `factory answer`, `owner` is `[]`.

### 3. `--line`, `--here` and a listing that exits 0 — high — blocked by: 1, 2
**Source**: §4 FR-2: "`factory status [RUN] --line`, which prints one `line` per run and nothing when there is none." §5 FR-3: "`--here` keeps only the runs whose `repo` is the git top level of the current directory, both sides canonicalized. Outside a git repo, `--here` lists nothing. A listing exits 0 whether or not it found runs … `--here` combines with `--json` and `--line`."

`src/cli/status.rs`, `src/cockpit.rs` (`list_live` takes the filter or returns the states). `--here` resolves `paths::repo_root(".")` and canonicalizes it. An error there means outside a repo, so the list is empty, never a failure. Change `tests/cockpit.rs:191-192` to expect exit 0.

**Acceptance:** `here_keeps_this_repos_runs_and_a_listing_exits_zero`: two live runs in two repos (two state files written under one `FACTORY_DATA_DIR` is enough). `factory status --here --json` from a subdirectory of one repo lists only its run and exits 0. From a directory outside both it prints `runs: []` and exits 0. `factory status --here --line` from the first repo prints one line. Plain `factory status` with no id exits 0 and still lists both.

### 4. Verify — medium — blocked by: 3
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 5. Prove — at the checkpoint — blocked by: 4
§10 QA plan rows, all proven below the UI:

- FR-1: run `status_json_carries_the_board_and_what_the_owner_owes`; keep the transcript.
- FR-2: run `the_glance_line_names_the_first_state_that_applies`, plus `factory status <id> --line` on a fixture run; keep the transcript.
- FR-3: run `here_keeps_this_repos_runs_and_a_listing_exits_zero`; keep the transcript.

**Acceptance:** after the checkpoint, each row has its test transcript in this slice's Progress.
