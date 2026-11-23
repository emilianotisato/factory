# Slice 04 — Evidence proves the row

**Source**: [`prd.md`](prd.md) → §4 (FR-2, the driver side and the contract text). **Depends on**: 02 and 03 — _(02 returns the row keys this check reads; 03 restructures the same close path and its send-back, which this slice extends rather than rewrites.)_
**Created**: 2026-11-23

**Progress**:
- Built: `src/checkpoint.rs` accepts a checkpoint only when the newest record covers every handed slice **and** every handed row not marked human has a real, non-empty `cp-{n}/<key>/` (lstat only: a link or a file at any level counts as missing). On accept, the driver log records `0 rows handed` or the owed and deferred keys. The send-back and the block now cover missing rows as well as missing slices, the block names every missing key, and says that the disk decides. `accepted_only` (applied in `src/scheduler.rs`) refuses records of a checkpoint this run never finished, so a parked run re-runs the checkpoint (AD-6). `src/worker.rs` gained `evidence_dir`. In `assets/contract-checkpoint.md`, the Prove, Evidence and Record steps name the per-row key directory and the deferred marking, and the return line is unchanged. The stub writes per-key dirs (AD-7).
- Inner loop: 1 run, 0 red, 2 min — caught: nothing.
- Seen red: with `unproven_rows` stubbed empty, `a_row_with_no_evidence_directory_is_sent_back`, `an_empty_evidence_directory_counts_as_missing`, `a_link_or_a_file_in_place_of_a_row_directory_counts_as_missing`, `a_block_names_every_missing_row`, `one_fr_with_three_rows_needs_three_directories`, `a_rejected_checkpoint_does_not_verify_its_slices` and `a_record_the_owner_writes_after_the_block_stands` each failed: a prompts mismatch, or a waiting-owner timeout. With the human filter removed, `a_human_row_needs_no_evidence` failed on a finished-phase timeout because the run blocked. With `accepted_only` a no-op, `a_rejected_checkpoint_does_not_verify_its_slices` failed: the checkpoints started were `[1:[1,2], 3:[3]]`, and it expects `3:[1,2]`. With the log-order exception removed, `a_record_the_owner_writes_after_the_block_stands` failed because a third checkpoint re-ran `[1,2]`. With the `0 rows handed` text changed, `a_prd_with_no_fr_rows_hands_nothing` failed on its driver-log assertion. With the old contract restored, `checkpoint_evidence_goes_in_one_directory_per_row_key` failed: the needle occurs 0 times.
- Deviation: the refused-record rule keeps an owner's record written after the block, and a checkpoint landing conflict emits `CheckpointDone` (AD-6, decide). `src/scheduler.rs` and `tests/stubs/claude` are touched outside the listed files (AD-6, AD-7).
- Prove: the FR-2 row is left for the checkpoint.

**Slice**: After this, a checkpoint closes only when every machine-provable row it was handed left a non-empty evidence directory on disk, and the contract tells the worker which directory each row owns. This is the slice that gives `assets/autonomy.md:64` its teeth. It leaves FR-4's mutation wording to slice 05.

**Highest risk in this feature.** A wrong exclusion blocks every run in every repo on the framework: the `Human? = yes` rows and the no-rows case are not edge cases, they are live in this repo's own PRDs. Get task 1's exclusions right before anything else.

## Current code / environment

- **Where evidence lives**: `src/worker.rs:57-58` (`EVIDENCE_ENV`) and `:261-263` — the driver creates `<run dir>/evidence` and passes it as `$FACTORY_EVIDENCE`. The contract tells the worker to write under `$FACTORY_EVIDENCE/cp-{n}/` (`assets/contract-checkpoint.md:15`) with free-form names, so nothing maps a file to a row today.
- **One more directory level is already safe to read**: `src/live/evidence.rs:15` (`ROOT`) and `:18` (`DEPTH = 4`) — `walk` starts at depth 1 under `cp-{n}` and admits three more levels, so `live --json` needs no change and its schema stays 1.
- **Five `Human? = yes` rows are live in this repo**: `docs/archive/013-live-status/prd.md:159`, `docs/archive/004-skills/prd.md:219,225,228`, `docs/archive/003-observatory/prd.md:126`. Nothing in `src/` reads that column — `grep -rni human src/` finds only `src/closeout.rs:909`, a PR-body pointer. Slice 02 marks those rows; this slice must skip them.
- **The accept path to extend**: `src/checkpoint.rs:296` as slice 03 leaves it — the newest entry, its covers compared. Add the evidence check to the same test, so one reject path serves both.
- **A rejected record's slices must not read as verified**: unlike slice 03's case, here the entry's `covers` are correct, so `covered()` (`src/docs/board.rs:338-346`) marks them covered and `board.unverified` goes empty. *prd §3 left the shape to this slice and named two. Take the events one: a slice counts as verified only when it is covered by an entry **and** that checkpoint has a `CheckpointDone` event (`src/events.rs:177-180`), which the driver already emits on the accept path. No schema change, no new log field. **Accepted cost:** `factory docs status` stays optimistic about a parked run, which the PRD states.*
- **Do not read the evidence** — *prd §4: the check is existence and non-emptiness. Reading it puts the driver in the business of judging a screenshot.*
- **Do not parse the `PROVE:` line** — *prd §4: the disk wins and the block says so.*
- **The run builds with the installed binary (0.13.5)**: this slice's own checkpoints are driven by the old code and the old contract, so write this run's evidence flat under `cp-{n}/` as the contract says today.

## Tasks

### 1. The driver checks one directory per handed row — high — blocked by: —
**Source**: §4 FR-2: "Evidence gets one directory per **row** … `$FACTORY_EVIDENCE/cp-{n}/<key>/`"; "The check is existence and non-emptiness, not content"; "A row marked `Human? = yes` is never checked"; "A PRD with no `| FR-` rows hands nothing, and the record says `0 rows handed`".

`src/checkpoint.rs`. On the accept path, for every row slice 02 handed that is **not** marked human, require `<evidence>/cp-{n}/<key>/` to exist and hold at least one entry. Zero handed rows passes and records `0 rows handed`. A human row is recorded as deferred, never as evidence owed.

**The trap**: `src/live/evidence.rs` reads this tree with `symlink_metadata` and follows no link (`:33-34`, `:47-52`). Use the same discipline here: a `cp-{n}/<key>` that is a symlink or a file, not a directory, counts as missing, or a worker can satisfy the check with a link to somewhere that already has files.

**Acceptance:** `a_row_with_no_evidence_directory_is_sent_back` and `an_empty_evidence_directory_counts_as_missing` — a checkpoint handed `FR-1` and `FR-2` with `cp-1/FR-2/` absent in the first case and present-but-empty in the second spawns a second worker both times. `a_human_row_needs_no_evidence` — a handed row marked human with no directory still closes. `a_prd_with_no_fr_rows_hands_nothing` — a PRD whose only table is unrelated closes with `0 rows handed` recorded. `one_fr_with_three_rows_needs_three_directories` — three `FR-6` rows need `FR-6.1`, `FR-6.2` and `FR-6.3`, and two of three is a reject. **Fails if removed**: without the check every case closes on the first attempt, so each second-worker assertion goes red.

### 2. The block names every missing row, and a rejected record is not coverage — high — blocked by: 1
**Source**: §4 Acceptance: "is respawned once, then blocks naming **every** missing row, not the first"; §3: "A rejected record must not count as coverage".

`src/checkpoint.rs`. Extend slice 03's block so it lists every missing row key alongside any uncovered slices. Then make the driver treat a slice as verified only when its covering checkpoint also has a `CheckpointDone` event, so a parked run re-runs the checkpoint instead of reading the rejected record as proof.

**The trap**: `CheckpointDone` is emitted on the accept path only — check that before relying on it, and emit nothing on a reject, or the rule reads a rejected checkpoint as accepted.

**Acceptance:** `a_block_names_every_missing_row` — a checkpoint handed four rows missing three of them blocks with all three keys in the body. `a_rejected_checkpoint_does_not_verify_its_slices` — after the block, the driver's own view still lists the covered slices as needing a checkpoint, although the log entry names them. **Fails if removed**: without the event rule the second test sees zero slices needing a checkpoint, so its assertion goes red.

### 3. The contract tells the worker which directory each row owns — high — blocked by: —
**Source**: §4 FR-2: "the pack must print each row's key beside it, and a harness that saves elsewhere copies into the row's directory"; §2: "Evidence paths gain one directory level".

`assets/contract-checkpoint.md`. Change the Prove step (`:15`) so evidence goes to `$FACTORY_EVIDENCE/cp-{n}/<the row's key>/`, and the Record step (`:17`) so the `**Prove:**` body line lists each handed row by key with its evidence, a human row marked deferred. Keep the return line (`:25`) as it is: it stays advisory, and the disk decides.

**The trap**: this is a change under `assets/`, which is **outside** the skills bundle, so it needs no version bump and no relock. Do not bump `Cargo.toml` for it.

**Acceptance:** in `tests/asset_text.rs`, using `assert_unique_passage` from `tests/common/passage.rs` as the existing checkpoint-contract assertions at `:85` do — the contract names `cp-{n}/` with a per-row key exactly once, and the `**Prove:**` record line names the key and the deferred marking exactly once. **Fails if removed**: the old wording has no key, so the needle is absent and the assertion goes red.

### 4. Verify — medium — blocked by: 1, 2, 3
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q`

**Acceptance:** inner loop green, one commit.

### 5. Prove — at the checkpoint — blocked by: 4
`prd.md` §8, the **FR-2** row in full: `a_row_with_no_evidence_directory_is_sent_back`, `an_empty_evidence_directory_counts_as_missing`, `a_block_names_every_missing_row`, `one_fr_with_three_rows_needs_three_directories`, `a_human_row_needs_no_evidence`, `a_prd_with_no_fr_rows_hands_nothing`. All `run` rows, proven below the UI from the transcript.

**Acceptance:** after the checkpoint, the transcript shows each named test green, including the two exclusion cases.
