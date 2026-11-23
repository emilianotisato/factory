# Slice 03 — The driver accepts or sends back

**Source**: [`prd.md`](prd.md) → §3 (FR-1, the driver side). **Depends on**: — _(independent of 01 and 02: it edits the close path in `src/checkpoint.rs`, which neither of them touches. Slice 04 depends on this one because it extends the same close path.)_
**Created**: 2026-11-23

**Progress**: _(written as it lands)_

**Slice**: After this, the driver stops closing a checkpoint on the number alone. It reads the newest checkpoint entry, checks that it names every slice the driver handed it, sends a short record back once, then blocks naming the missing slices. It leaves the evidence check to slice 04 and the contract text alone.

## Current code / environment

- **The whole close condition**: `src/checkpoint.rs:296` — `if after.checkpoints.iter().any(|c| c.n == n)`. `land_here` (`:828`) only lands a commit; it never reads the record. So existence of the number is the entire test today.
- **`n` is frozen before the worker runs**: `src/checkpoint.rs:65-66` — `n` is `max(existing) + 1` and `covers` is `board.unverified.clone()`, both computed once at the top of `run`.
- **A record cannot be withdrawn, so the respawned worker's record is `CP-(n+1)`**: `src/docs/log.rs:217-237` — `add()` assigns `id` itself as `{seq}-{last+1}` and appends with `OpenOptions::append(true)`. `src/cli/docs.rs:162-191` has only `Add` and `Show`: no remove, no amend, and the contract forbids hand edits. **So the acceptance test must read the newest checkpoint entry, not the one numbered `n`.** The thin record stays in the log, which is the record of the send-back.
- **The respawn loop already exists, but its cap counts deaths, not thin records**: `src/checkpoint.rs:190-215` — the loop computes `deaths` from `w.died()` and blocks at `DEATH_CAP`. A worker that exits cleanly having written a short record has not died, so it will not be counted. `attempt` at `:253` counts `mine(w)` workers and is the number to cap the send-back on, separately from `DEATH_CAP`.
- **A short record leaves its unnamed slices unverified by itself**: `src/docs/board.rs:338-346` — `covered()` honours a non-empty `covers` exactly, so a record naming fewer slices already leaves the rest in `unverified`. No extra bookkeeping is needed for *this* slice's reject case; slice 04's is different, because there the covers are right.
- **Superset is not a failure** — *prd §3: the failure is "a record that names fewer slices than the driver handed it". A worker that names more is not wrong, so test `handed ⊆ entry.covers`, not equality.*
- **Do not parse the worker's `PROVE:` line** — *prd §4 decided the driver checks artifacts, not the worker's prose. `assets/contract-checkpoint.md:25` stays advisory.*
- **The leaks allowlist already covers the files you will touch**: `docs/project.toml` lists `src/checkpoint.rs` and `tests/checkpoints.rs` (scope `all`), so a fixture may narrate by slice number. A new test file would not be allowlisted.
- **The run builds with the installed binary (0.13.5)**: this slice's own checkpoints are driven by the old close path, so do not expect the new send-back to fire during this run.

## Tasks

### 1. The acceptance test reads the newest entry and compares covers — high — blocked by: —
**Source**: §3 FR-1: "The driver's acceptance test reads **the newest checkpoint entry**, not the entry numbered `n`"; "A record that names fewer slices than the driver handed it is sent back once, then blocks".

`src/checkpoint.rs`. Replace the condition at `:296` with one that takes the newest checkpoint entry by `n` and accepts only when every slice in the driver's `covers` appears in that entry's `covers`. Keep the landing that follows unchanged on the accept path.

**The trap**: on a respawn the newest entry's `n` is `n+1` while the driver's `n` stays what `:66` computed, so comparing `c.n == n` would read the *thin* record forever. Take the max, not the match.

**Acceptance:** `a_record_naming_fewer_slices_is_sent_back_once` — a fixture checkpoint handed slices 1 and 2 whose worker records only slice 1 spawns a second worker, and the driver's line names slice 2. A second case records both slices and lands on the first attempt. **Fails if removed**: with the old existence test the first case lands immediately, so the second-worker assertion goes red.

### 2. The second short record blocks — high — blocked by: 1
**Source**: §3 Acceptance: "a second short record writes a block naming the missing slices and the run parks, with the slices still reading as unverified".

`src/checkpoint.rs`. Cap the send-back at one, using `attempt` (`:253`), separately from `DEATH_CAP`. On the second short record write a block whose title and body name the slices the records never covered, and park the run, in the shape the death block at `:218-235` already uses.

**The trap**: `DEATH_CAP`'s own block (`:210-240`) must keep firing for actually dead workers. Do not fold the two counters into one, or two deaths and two thin records become indistinguishable in the log.

**Acceptance:** `a_second_short_record_blocks_and_leaves_the_slices_unverified` — two consecutive short records produce one block entry naming the uncovered slices, the run parks, and `factory docs status` still lists those slices as unverified. **Fails if removed**: without the cap the driver respawns forever, so the test's block assertion never arrives.

### 3. Verify — medium — blocked by: 1, 2
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q`

**Acceptance:** inner loop green, one commit.

### 4. Prove — at the checkpoint — blocked by: 3
`prd.md` §8, the **FR-1** row — this slice's half: `a_record_naming_fewer_slices_is_sent_back_once` and `a_second_short_record_blocks_and_leaves_the_slices_unverified`. Both are `run` rows, proven below the UI from the checkpoint's transcript. The write-side test in the same row is slice 01's.

**Acceptance:** after the checkpoint, the transcript shows both tests green.
