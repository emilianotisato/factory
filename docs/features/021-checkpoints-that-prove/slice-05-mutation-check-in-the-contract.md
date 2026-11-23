# Slice 05 — The mutation check in the contract

**Source**: [`prd.md`](prd.md) → §6 (FR-4). **Depends on**: 04 — _(both edit `assets/contract-checkpoint.md`; 04's Prove and Record steps land first so this slice only adds its own step.)_
**Created**: 2026-11-23

**Progress**: built — the checkpoint contract's Prove step asks for one mutation check per checkpoint (riskiest row, break its one line by hand never through git, see the test red, restore and see it green before recording), and the Record body gains a `**Mutation:**` field naming the `file:line`, the red and the restore; `the_checkpoint_contract_asks_for_one_mutation_check` pins all three passages unique in the Prove line, the Record line and the whole bundle (every pack text plus every skill file) · inner loop: 1 run, 0 red, 2 min — caught: nothing · deviations: none
- `the_checkpoint_contract_asks_for_one_mutation_check` seen red: with "one mutation check per checkpoint" cut from the contract, the Prove-step needle occurs 0 times, want exactly once
- Checkpoint 2, FR-4 (`the_checkpoint_contract_asks_for_one_mutation_check` green): cp-2/FR-4/tests.log
- Checkpoint 2, FR-3 (last checkpoint, handed the whole table; all three tests green): cp-2/FR-3/tests.log
- Checkpoint 2, inner loop after the leaks fix: cp-2/inner-loop.log

**Slice**: After this, the checkpoint contract asks the worker to prove one of its rows can go red: break the line the row's FR rests on, see the test fail, restore, and name the `file:line` and the red in the record. Wording only — the driver does not check it, by the owner's decision.

## Current code / environment

- **The step to add it beside**: `assets/contract-checkpoint.md:15` (Prove) and `:17` (Record), as slice 04 leaves them. The new ask belongs in the Prove step, after the rows are driven, and its output belongs in the Record body.
- **`assets/` is outside the skills bundle**: no version bump, no `skills/bundle.lock` relock, no cockpit pin change. A `skills/` edit would need all three; this does not.
- **The passage must be unique in the bundle** — *prd §6 Acceptance, and PRD-018 FR-11: a skill-text assertion that passes on a sibling line still passes after the rule it guards is deleted. `assert_unique_passage` lives in `tests/common/passage.rs` and `tests/asset_text.rs:85` already uses it on a checkpoint-contract passage.*
- **Nothing enforces this** — *prd §6: wording only, the owner's call, with the cost stated in the PRD. Do not add a driver check, a record field the driver reads, or a block. The signal that it bought nothing is a later retro naming a test that could not fail.*
- **The three features that earned it**: PRD-017's `a_cancel_check_kills_every_running_pass` and `last_reviewed_commit_is_the_one_that_set_reviewed` both passed two checkpoints and could not fail; PRD-024's second opinion found another; PRD-018's fallback found five needle gaps.
- **Say how to break it without git** — `assets/contract-slice.md:14` already tells a builder to break a feature "by hand, never through git: your work is uncommitted, so a `git checkout`, `restore` or `stash` loses it". Use the same words here, for the same reason.

## Tasks

### 1. The contract asks for one mutation check per checkpoint — high — blocked by: —
**Source**: §6 FR-4: "asks the worker, at each checkpoint, to take the riskiest row it drove, break the one line that row's FR rests on, run its test, see it red, restore, and name the `file:line` and the red in the record"; Acceptance: "The contract names the check in one step, with the record field it writes".

`assets/contract-checkpoint.md`. Add the ask to the Prove step: one row per checkpoint, the one whose FR rests on a single line; break that line by hand, never through git; run the row's test; see it red; restore; confirm it is green again. Add the field it writes to the Record body, naming the `file:line` broken and the red seen.

**The trap**: a worker that breaks a line and is interrupted leaves the tree broken and the next gate red for a reason nothing explains. Tell it to restore before it records, and to name the restore in the same field.

**Acceptance:** `the_checkpoint_contract_asks_for_one_mutation_check` in `tests/asset_text.rs`, via `assert_unique_passage` — the contract names the break-and-restore ask exactly once and the record field exactly once, and neither needle matches any other line in the bundle. **Fails if removed**: the needles are absent from today's contract, so the assertion goes red.

### 2. Verify — medium — blocked by: 1
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q`

**Acceptance:** inner loop green, one commit.

### 3. Prove — at the checkpoint — blocked by: 2
`prd.md` §8, the **FR-4** row: `the_checkpoint_contract_asks_for_one_mutation_check`. A `run` row, proven below the UI from the transcript. This slice is the last in the order, so its checkpoint runs with `all_done` true and is handed every row in the table (§5 FR-3).

**Acceptance:** after the checkpoint, the transcript shows the test green, and the record shows the whole table handed rather than this slice's row alone.
