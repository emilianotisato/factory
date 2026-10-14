# Slice 05 — Checkpoint covers

**Source**: [`prd.md`](prd.md) → §9 FR-7 (a checkpoint entry records the slices it covered), §13 OPEN-6. **Depends on**: nothing in code; it touches the log, the board and the checkpoint contract, which no other slice edits. It is numbered after 01 to 04 because the run building this PRD has four lanes and they are full until one of those lands.
**Created**: 2026-10-14

**Progress**: _(written as it lands — what was built, the gate numbers, what a review caught)_

**Slice**: After this slice a checkpoint entry in `log.jsonl` says which slices it covered, in a `covers` field. `factory docs log add --kind checkpoint --covers 1,2,4,5` writes it, the checkpoint contract passes it, and the stub does too. The board's `unverified` becomes every done slice that no entry covers, so a slice that finishes after a higher-numbered one is still checked. An entry without `covers`, which is every log written before this ships, keeps today's high-water rule, so PRD-004's board reads the same.

## Current code / environment

- **The high-water rule**: `src/docs/board.rs:332-346` builds `checkpoints` from each entry's `n` and `slice` (as `after`), takes the highest `after`, and calls every done slice above it `unverified`. `checkpoint_due` (`:347-348`) reads that list. `Checkpoint { n, after }` is at `src/docs/mod.rs:242-247`, serialized camelCase into `docs status --json`.
- **The log entry**: `Entry` (`src/docs/log.rs:74-96`) has no list field; every optional field is `#[serde(default, skip_serializing_if = "Option::is_none")]`, so an old line parses and a new field is omitted when empty. `Entry::new` (`:100-113`) and the test literals at `src/docs/log.rs:418` and `:514` build every field. Validation (`:180-196`) already refuses a checkpoint with no `--slice`; that rule stays, because `render` (`:354-362`, "after slice-NN"), W-CP-STALE (`src/docs/check.rs:676-688`) and migrate (`src/docs/migrate.rs:544-553`) all read `slice`.
- **The CLI**: `LogVerb::Add` (`src/cli/docs.rs:159-186`) is built into an `Entry` at `:397-404`.
- **Who passes the list today**: the driver computes `covers` from `board.unverified` (`src/checkpoint.rs:87`) and renders it into the pack: `{NN}` is the highest, `{covers}` is `"01, 02"` (`src/pack.rs:332-343`). The contract (`assets/contract-checkpoint.md:15`) passes only `--slice {NN}` and writes `**Covers:** slices {covers}` into the body. `{covers}` is prose; it cannot be the flag's value.
- **The stub mirrors the contract**: `tests/stubs/claude:244` reads the covered slices from the pack's `## slice-NN` headings, and `:270-271` adds the entry with `--slice <last>` and no list.
- **Consumers that need no change**: `src/scheduler.rs:471` and `:502` read `board.unverified`, and so does `src/checkpoint.rs:87`. They follow the new rule for free.
- **Tests that assert today's rule**: `tests/docs_log.rs:123-155` (`unverified` before and after a checkpoint with `--slice 2`), `tests/stubs.rs:231`, `tests/checkpoints.rs:364-407` (`assert_cp` reads `slice`, title and the `**Covers:**` body line). They all pass under the new rule, because their covered sets are contiguous.
- **The PRD-004 fixture**: `docs/features/004-skills/log.jsonl:8` is CP-1, `slice: 5`, body "Covers: slices 01, 02, 04, 05"; `:15` is CP-2, `slice: 7`, "06, 07". All seven slices are done. Neither line has `covers`. That folder can be archived later, so copy the log into `tests/fixture/` for the test.

## Tasks

### 1. `covers` in the log entry and the CLI — high — blocked by: —
**Where**: `src/docs/log.rs` (`Entry`, `Entry::new`, the test literals), `src/cli/docs.rs` (`LogVerb::Add`).

**Source**: §9 FR-7: "a checkpoint entry records the slices it covered, in a `covers` field written by `factory docs log add --kind checkpoint --covers 1,2,4,5`".

`Entry` gains `covers: Vec<u32>`, `#[serde(default, skip_serializing_if = "Vec::is_empty")]`. `LogVerb::Add` gains `--covers` taking a comma-separated list (`value_delimiter = ','`). Refuse `--covers` on any kind but `checkpoint`, as `--ref` is refused off an answer or a review (`src/docs/log.rs:181-186`). `--slice` stays required on a checkpoint.

**Acceptance:** in `tests/docs_log.rs`: `log add 001 --kind checkpoint --slice 5 --covers 1,2,4,5 --title t` writes a line whose `covers` is `[1,2,4,5]`; the same without `--covers` writes a line with no `covers` key; `--covers 1` on a `note` exits nonzero and writes nothing.

### 2. `unverified` by coverage — high — blocked by: 1
**Where**: `src/docs/board.rs:332-346`, `src/docs/mod.rs` (`Checkpoint`).

**Source**: §9 FR-7: "`unverified` becomes every done slice that no entry covers. An entry without `covers`, which is every log written before this ships, falls back to its `after` high-water mark."

A done slice is covered when some checkpoint entry lists it in `covers`, or some entry with an empty `covers` has `after >= n`. `unverified` is the done slices not covered. `Checkpoint` gains `covers: Vec<u32>` beside `after`, so `docs status --json` shows it.

**Acceptance:** a board test in `tests/docs_log.rs`: slices 1, 2, 4, 5 done, a checkpoint with `--slice 5 --covers 1,2,4,5`, then slice 3 set done: `docs status --json` lists `unverified` `[3]`; with all five done and `checkpoint_every` 2, `checkpointDue` is true. A second test copies PRD-004's `log.jsonl` into `tests/fixture/` and reads it beside a seven-slice `state.toml`, all done: `unverified` is `[]`, `checkpoints` is CP-1 after 5 and CP-2 after 7, both with empty `covers`. `tests/docs_log.rs:123-155` stays green unchanged.

### 3. The contract, the pack and the stub pass the list — high — blocked by: 1
**Where**: `assets/contract-checkpoint.md:15`, `src/pack.rs:332-343`, `tests/stubs/claude:270-271`.

**Source**: §9 FR-7: "(the contract's step 3 passes `{covers}`)".

`{covers}` renders `"01, 02"` and stays the prose list. Add a placeholder the pack renders as `1,2` (e.g. `{covers csv}`) and put `--covers {covers csv}` into step 3's command, after `--slice {NN}`. The stub passes `--covers` with the numbers from its `## slice-NN` headings.

**The trap**: passing `{covers}` as the flag gives `--covers 01, 02`, which the shell splits into two words and the parser refuses. That is why the PRD's "passes `{covers}`" needs a second placeholder.

**Acceptance:** a pack unit test in `src/pack.rs` (or `tests/checkpoints.rs`): a checkpoint pack covering 1, 2 and 4 holds `--covers 1,2,4` and `**Covers:** slices 01, 02, 04`. In `tests/checkpoints.rs`, `assert_cp` also asserts the entry's `covers` field: `[1,2]` for CP-1 and `[3]` for CP-2 of `assert_two_checkpoints`.

### 4. Verify — medium — blocked by: 1, 2, 3
`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 5. Prove — at the checkpoint — blocked by: 4
Rows from [`prd.md`](prd.md) §14 this slice proves, below the UI. **FR-7**: "run the board test and the PRD-004 log fixture" — the two tests of task 2.

**Acceptance:** after the checkpoint, Progress holds the transcript of both tests, with slice 3 in `unverified` and PRD-004's board unchanged.
