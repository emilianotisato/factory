# Slice 03 — Autonomy, contracts and the PR body

**Source**: [`prd.md`](prd.md) → §6 FR-4, §8 FR-6, §9 FR-7 (the contract half), §12 FR-10, §15. **Depends on**: 01 (its tests call slice 01's helper). It is independent of 02 and 04: it edits only `assets/`, which is outside the skills bundle, so no relock. Its tests go in a new file, not `tests/skills_bundle.rs`.
**Created**: 2026-11-12

**Progress**:

- **Built.** `assets/autonomy.md` § What an unattended run may write: three new May items (repair what the run wrote; research notes, runbooks and operations procedures the FRs made wrong, text only; a later defect fix to a PRD-ordered file with an `AD` marked `decide`), a sign-off Never item, and a **Never wins over every May.** precedence paragraph. The cut markers did not move. `assets/contract-slice.md` has a new **See each test fail once** Do bullet. `assets/contract-checkpoint.md` has the spec-not-output rule in Prove, the `verify-*` repair exception in the touch rule, and the repair named under `**Fixed:**` in Record. `assets/pr-body.md`'s Gate line says where its numbers come from and adds an `inner loops:` clause. The tests are in a new `tests/asset_text.rs`, which reads `pack::builder_autonomy()`, `pack::CONTRACT`, `pack::CONTRACT_CHECKPOINT` and `pack::PR_BODY`.
- **Inner loop:** 1 run, 0 red, 2 min.
- **Seen red once** (all four assets reverted with `git checkout`, `cargo test -q --test asset_text`, then restored and green):
  - `autonomy_names_research_notes_runbooks_and_sign_offs`: "repair anything the run itself wrote on this branch" occurs 0 times.
  - `a_behavioural_test_is_seen_red_once`: the break-the-feature-once passage occurs 0 times.
  - `checkpoint_takes_its_expected_result_from_the_spec`: the spec-not-output sentence occurs 0 times.
  - `checkpoint_repairs_the_drive_a_covered_slice_broke`: the `verify-*` repair passage occurs 0 times.
  - `pr_body_names_full_gate_runs_and_inner_loops`: the checkpoint-`**Gate:**` passage occurs 0 times.
- **Deviations:** none.
- **Checkpoint 1** FR-7 `a_behavioural_test_is_seen_red_once`, `checkpoint_takes_its_expected_result_from_the_spec`: `cp-1/fr-07-asset-text.txt`
- **Checkpoint 1** FR-10 `checkpoint_repairs_the_drive_a_covered_slice_broke`: `cp-1/fr-10-checkpoint-repairs-drive.txt`

**Slice**: After this slice:

- `assets/autonomy.md` names research notes, runbooks, a later fix to a PRD-ordered file and human sign-off items, with a precedence sentence;
- the builder contract asks a builder to see each test it runs go red once with the feature broken;
- the checkpoint contract takes expected results from the spec and may repair a `verify-*` drive a covered slice broke;
- the PR body says where its gate numbers come from.

The `/slice` half of FR-7 is slice 04's.

## Current code / environment

- **These assets are embedded** with `include_str!` (`src/pack.rs:17` for autonomy; the contracts and `pr-body.md` alongside). They are not in `src/skills.rs` `FILES`, so no version bump and no relock. A lane runs the installed binary, so this run's own builders read the old text (prd §2).
- **FR-6**, `assets/autonomy.md:46-52`, "What an unattended run may write":
  - **May** (`:48`): application code and tests, the feature's folder, `factory docs idea add`, `.claude/skills/verify-*` ("Extend or repair a recipe, but never drop a step an earlier checkpoint drove").
  - **Never** (`:50`): `docs/decisions.md`, `docs/ux-rubric.md`, CI workflow files, `CLAUDE.md`, `AGENTS.md`, `.ai/guidelines/`, another feature's folder, `docs/archive/`, and the PRD's FRs.
  - **The PRD outranks** (`:52`).
  - The section sits outside every builder cut (`:31-45`, `:95-100`, `:106-109`, `:112-125`), so builders read it.
  - "Never, to get green" (`:54-64`) has "edit a CI workflow" at `:61`.
- **FR-6 precedence** (prd §8): the Never lists win over every new May. A PRD-ordered CI workflow may get a defect fix a review found, never an edit that makes a red check pass. `CLAUDE.md` and `docs/decisions.md` stay Never. For a builder, "anything the run wrote" means its slice's files (`assets/contract-slice.md:19`).
- **FR-7, the builder half**: `assets/contract-slice.md`.
  - `:13` runs the inner loop and times every run.
  - `:14`: "Do not run the full gate and do not drive the harness".
  - `:25`: `INNER: {runs} runs, {red} red, …`.
  - The new rule: see each test it runs go red once with the feature broken (a revert or a stub), then restore the feature. Only for tests its tasks' Acceptance names and `run` rows. One Progress line per test. These reds do not count in `INNER`. A harness-driven row only names its failure.
- **FR-7, the checkpoint half**: `assets/contract-checkpoint.md:15` (Prove) gains a rule: the expected result comes from the PRD, the spec or the input file, never from what the command printed.
- **FR-10**: `assets/contract-checkpoint.md:19`: "Touch nothing a row, a leaks hit or the record does not name. Slice files other than their Progress line, `prd.md`, `state.toml` and `log.jsonl` are off limits." It gains the `.claude/skills/verify-*` repair when a covered slice moved the surface it drives. The repair is named under **Fixed** (`:17`). Never drop a step an earlier checkpoint drove. The driver commits it with the checkpoint (`:18`).
- **FR-4**: `assets/pr-body.md`.
  - `:43`: "- Gate `{command}`: {runs} runs, {red} red, {minutes} min in total".
  - `:30`'s per-slice cell already says `inner {runs}/{red} red`.
  - The polish pack carries no gate counts (`src/pack.rs:537-572`). So the template says: `{runs}`, `{red}` and `{minutes}` are summed from each checkpoint entry's `**Gate:**` line in the feature's log, plus polish's own gate runs. A second clause, `inner loops: {n} runs, {red} red`, is summed from the slices' Progress inner-loop lines.
- **Existing pins**: `autonomy_has_the_overturn_and_safe_contract_rows` (`tests/skills_bundle.rs:340`) pins `autonomy.md:27` and `:38`. `src/pack.rs:1377-1395` pins the cut markers. `tests/loop.rs:273-277` pins the contract's "quiet flag" and the autonomy copy. Keep all of these green, and do not move the cut markers.
- **Traps**:
  - No line may contain the uppercase decision prefix that `no_skill_or_asset_asks_for_a_numbered_decision` (`tests/skills_bundle.rs:76`) rejects. Write "a decision in `docs/decisions.md`".
  - The leaks check flags a concrete plan id inside a test string (`AD-3`, `slice 04`). Quote placeholder forms like `AD-n`, or passages without ids.
- **Deliberate non-change**: prd §17 "Not handled: one hostile case per FR in the checkpoint contract". Do not add it.

## Tasks

### 1. What an unattended run may write — high — blocked by: —
**Source**: §8 FR-6: the three **May** items, the sign-off **Never** item, and the precedence bullets.

Edit `assets/autonomy.md:46-52`, keeping the section outside the cut.

**Acceptance:** `autonomy_names_research_notes_runbooks_and_sign_offs`, in a new `tests/asset_text.rs` through the helper. The section holds:

- the research-note and runbook passage;
- the PRD-ordered-file passage;
- the sign-off passage under **Never**;
- the precedence passage.

Each occurs exactly once, and each survives `pack::builder_autonomy()` (check through the public fn).

### 2. Builder and checkpoint Prove rules — high — blocked by: —
**Source**: §9 FR-7: "the expected result comes from the PRD, the spec or the input file, never from what the command printed"; "The builder sees it red only for tests it runs … These deliberate reds do not count in the `INNER: … red` return line … A harness-driven row only names its failure." §12 FR-10: "lets the checkpoint repair `.claude/skills/verify-*` when a covered slice moved the surface it drives… names the repair under **Fixed**… never drops a step an earlier checkpoint drove."

Edit `assets/contract-slice.md` (Do list) and `assets/contract-checkpoint.md:15` and `:19`.

**Acceptance:** in `tests/asset_text.rs`:

- `a_behavioural_test_is_seen_red_once`: the builder contract holds the break-it-once passage and the not-counted-in-INNER passage, once each.
- `checkpoint_takes_its_expected_result_from_the_spec`: the checkpoint contract holds the spec-not-output passage once. Slice 04 tests the `/slice` half.
- `checkpoint_repairs_the_drive_a_covered_slice_broke`: the checkpoint contract's touch rule holds the `verify-*` repair passage once.

### 3. The PR body's gate numbers — medium — blocked by: —
**Source**: §6 FR-4: "`{runs}` is the full-gate runs: the sum of each checkpoint entry's `**Gate:** n runs` in the feature's log, plus polish's own gate runs… A second clause, `inner loops: {n} runs, {red} red`, is the sum of the slices' Progress inner-loop lines."

Edit `assets/pr-body.md:43`.

**Acceptance:** `pr_body_names_full_gate_runs_and_inner_loops`, in `tests/asset_text.rs`: the Gate line holds the where-from passage and the inner-loop clause, once each.

### 4. Verify — medium — blocked by: 1, 2, 3
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q`

**Acceptance:** inner loop green, one commit.

### 5. Prove — at the checkpoint — blocked by: 4
prd §15 rows:

- **FR-4:** `pr_body_names_full_gate_runs_and_inner_loops`.
- **FR-6:** `autonomy_names_research_notes_runbooks_and_sign_offs`.
- **FR-7:** `a_behavioural_test_is_seen_red_once` and `checkpoint_takes_its_expected_result_from_the_spec`.
- **FR-10:** `checkpoint_repairs_the_drive_a_covered_slice_broke`.

They are proven below the UI: the checkpoint runs them and keeps the transcript.

**Acceptance:** after the checkpoint, the transcripts are in Progress.
