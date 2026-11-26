# Slice 04 — Unified land_lane, respawn lane retention and wait_sh barriers

**Source**: [`prd.md`](prd.md) → §6 FR-4, §10 FR-8, §12 QA plan. **Depends on**: 03 (edits `src/checkpoint.rs`, `src/scheduler.rs`, `tests/checkpoints.rs`, `tests/lanes.rs` after slice 03).
**Created**: 2026-11-26

**Progress**: `closeout::polish` and `fix_round` pass `attempt == 1` into `prepare_lane`; `land::land_lane` + `LandSpec`/`LandOutcome`/`BeforeLand` unify scheduler, checkpoint, and closeout landing; `tests/closeout.rs` adds `polish_and_ci_fix_respawns_keep_their_lane_edits` and `unified_land_lane_handles_checkpoint_polish_conflicts_and_claim_kill_recovery`; three authorized `commits`/`lanes` ordering tests use `wait_sh` barriers. Inner loop: 1 run, 0 red, ~2.4 min — caught: nothing. Deliberate reds: `polish_and_ci_fix…` with `polish` `fresh=true` (polish respawn lost `kept-polish.txt` / two dead polish workers); `unified_land_lane…` with `LandOutcome::Conflict` mapped to `Landed` (no `checkpoint_blocked` / closeout block on shared.txt). Stub `tests/stubs/claude` extended (AD logged). cp-2: FR-4 `cp-2/FR-4/transcript.txt`; FR-8 `cp-2/FR-8/transcript.txt`.

**Slice**: Keeps polish and CI-fix lanes intact across dead/rate-limited respawns (`fresh = attempt == 1`), extracts the duplicated landing sequence from `scheduler::land_slice`, `checkpoint::land_worker`, and `closeout::land_worker` into one shared `land::land_lane` pipeline with a `before_commit` callback, adds driver tests for checkpoint conflict, polish conflict, and claim-to-spawn crash recovery, and replaces fixed `delay_ms` sleeps with deterministic `wait_sh` barriers in the three authorized multi-lane ordering tests.

## Current code / environment

- **`src/closeout.rs:238` (`polish`) and `:2112` (`fix_round`)**: both pass `fresh = true` to `scheduler::prepare_lane(run, lane, true, out)?`, resetting the lane even when `attempt > 1` (wiping a dead/rate-limited worker's edits). Build respawns (`src/scheduler.rs:699`) pass `fresh = false`.
- **Three duplicated landing pipelines**:
  - `src/scheduler.rs:974-1096` (`land_slice`): checks missing lane/base as hard `Err`, checks `base..HEAD` for `subject`, reuses held `ref_name` when `!merged || parent == base` (taking message from held ref when re-committing a merge), calls `land::commit_lane` (emitting `WorkerWroteProtectedFile` on dropped files; blocking on empty merge if `merged && held.is_some()`), calls `undo_half_pick`, calls `land::land` (routing `Conflict` to `start_merge`).
  - `src/checkpoint.rs:877-966` (`land_worker`): falls back to `commit_board` when `lane`/`base` is `None` or `commit_lane` returns `None`, checks `base..HEAD` for `subject`, reuses held `ref_name`, calls `land::commit_lane`, logs dropped files, calls `undo_half_pick`, calls `land::land`.
  - `src/closeout.rs:625-720` (`land_worker`): checks `base..HEAD` for `subject`, reuses held `ref_name`; ONLY on the fresh-commit path (when no held ref exists and not already landed!) runs `stand_ins` (for `Closer::Polish`) and the `prd.md` `authorized_tests` frontmatter guard (adding `prd_rel` to `protected` if altered), falls back to `nothing()` when `base` is `None` or `commit_lane` returns `None`, logs dropped files, calls `undo_half_pick`, calls `land::land`.
- **Real crash window in `scheduler::spawn_on` (`src/scheduler.rs:699-709, :815-818`)**: `prepare_lane` resets the lane and writes `lane_base` (`:818`) BEFORE `state.toml` flips to `building` + `lane = N` (`:701-709`), which happens BEFORE `Worker::spawn` records the worker in `state.json`. So the real crash window is: `state.toml` says `building` on `lane = 1`, `state.json` has `lane_bases[1]`, and `state.json` has NO worker for slice 1.
- **Authorized test edits (`prd.md` frontmatter `authorized_tests`)**: ONLY these three pre-existing test functions are authorized for removing `delay_ms` lines:
  - `tests/commits.rs::a_dirty_path_in_the_main_tree_parks_the_landing_and_keeps_the_owners_edit`
  - `tests/commits.rs::a_park_with_no_question_carries_its_reason`
  - `tests/lanes.rs::two_lanes_build_two_slices_at_once_and_the_third_waits_for_its_dependency`
  All other existing test functions (including `tests/merge.rs:139` and all tests in `tests/checkpoints.rs` / `tests/closeout.rs`) MUST NOT have lines removed (add new tests purely additively so `testguard` stays green!).

## Tasks

### 1. Keep polish and CI-fix lanes on respawn (`src/closeout.rs`) — high — blocked by: —
**Source**: §6 FR-4: "In `closeout::polish` (`src/closeout.rs:238`): pass `attempt == 1` instead of `true` to `scheduler::prepare_lane(run, lane, attempt == 1, out)?`. In `closeout::fix_round` (`src/closeout.rs:2112`): pass `attempt == 1` instead of `true` to `scheduler::prepare_lane(run, lane, attempt == 1, out)?`."

- `src/closeout.rs:238`: change `scheduler::prepare_lane(run, lane, true, out)?` to `scheduler::prepare_lane(run, lane, attempt == 1, out)?`.
- `src/closeout.rs:2112`: change `scheduler::prepare_lane(run, lane, true, out)?` to `scheduler::prepare_lane(run, lane, attempt == 1, out)?`.
- `tests/closeout.rs` (purely additive new test): add `polish_and_ci_fix_respawns_keep_their_lane_edits` proving: (1) when polish attempt 1 writes `kept-polish.txt` and exits dead, polish attempt 2 sees `kept-polish.txt` present in its lane (`sh: "test -f kept-polish.txt"`) and lands cleanly; (2) when CI-fix round 1 attempt 1 writes `kept-cifix.txt` and exits dead, round 1 attempt 2 sees `kept-cifix.txt` still present (`sh: "test -f kept-cifix.txt"`), while round 2 attempt 1 starts on a reset lane (`sh: "test ! -e stale-round1.txt"`).

**Trap**: On a fresh polish or CI-fix round (`attempt == 1`), `fresh` MUST still be `true` so stale untracked files from an earlier slice or earlier CI round are cleaned out!

**Acceptance:** `polish_and_ci_fix_respawns_keep_their_lane_edits` passes and fails if either call site still passes literal `true`.

### 2. Unify `land::land_lane` across `scheduler`, `checkpoint`, and `closeout`, and add conflict/crash driver tests — high — blocked by: 1
**Source**: §10 FR-8: "Move `undo_half_pick` into `src/land.rs` ... Add `LandSpec`, `LandOutcome`, and `pub fn land_lane(spec: &LandSpec, before_commit: Option<&mut dyn FnMut(&Path) -> Result<Vec<String>>>) -> Result<LandOutcome>` ... Refactor all three callers (`scheduler::land_slice`, `checkpoint::land_worker`, `closeout::land_worker`) to call `land::land_lane` ... Add `unified_land_lane_handles_checkpoint_polish_conflicts_and_claim_kill_recovery`".

- `src/land.rs`:
  - Move `undo_half_pick` from `src/scheduler.rs` into `src/land.rs` (keeping `pub(crate) use crate::land::undo_half_pick;` in `scheduler.rs` if any sibling module imports it).
  - Define `LandSpec<'a>` (`repo`, `lane_dir`, `base`, `board_files`, `protected`, `message`, `subject`, `ref_name`, `reuse_held`, `require_parent_eq_base`) and `LandOutcome` (`AlreadyLanded { sha }`, `Nothing { had_held: Option<String> }`, `Landed { sha, dropped }`, `Dirty { paths, dropped }`, `Conflict { sha, paths, dropped }`).
  - Implement `pub fn land_lane(spec: &LandSpec<'_>, before_commit: Option<&mut dyn FnMut(&Path) -> Result<Vec<String>>>) -> Result<LandOutcome>`:
    1. Check `git log --format=%H %s {base}..HEAD` in `repo` for exact `spec.subject`; if found, delete stale `spec.ref_name` if present and return `Ok(LandOutcome::AlreadyLanded { sha })`.
    2. Resolve `held = resolve(repo, spec.ref_name)?`. Compute `reuse = held.is_some() && spec.reuse_held && (!spec.require_parent_eq_base || parent_of(held) == Some(spec.base))`.
    3. If `reuse`, use `(held.unwrap(), Vec::new())`. Else:
       - Compute `let mut protected = spec.protected.to_vec();`
       - If `let Some(cb) = before_commit`, extend `protected` with `cb(spec.lane_dir)?`.
       - Compute commit message (if `held` exists and `!reuse`, preserve held commit's full `%B` message like `scheduler::land_slice:1033-1037` does for merge commits, else `spec.message`).
       - Call `commit_lane(spec.lane_dir, spec.base, spec.board_files, &protected, &message, spec.ref_name)?`.
       - If `None`, return `Ok(LandOutcome::Nothing { had_held: held })`.
       - Else `(c.sha, c.dropped)`.
    4. Call `undo_half_pick(spec.repo, &sha)?`.
    5. Call `land(spec.repo, &sha, spec.board_files)?` and return `Landed` / `Dirty` / `Conflict` carrying `dropped` (and `sha` on `Conflict`).
- Refactor all three callers to route through `land::land_lane`:
  - `src/scheduler.rs:974-1096` (`land_slice`): check `lane` and `base` exist (returning the exact existing missing-lane / missing-base errors when `None`!), call `land::land_lane(&spec, None)`, emit `WorkerWroteProtectedFile` for each `dropped`, handle `Nothing { had_held }` (empty-merge block if `merged && had_held.is_some()`, else delete stale ref & log nothing to commit), `AlreadyLanded` / `Landed` → `record_landing`, `Dirty` → `Landed::Waiting`, `Conflict { sha, paths, .. }` → `start_merge`.
  - `src/checkpoint.rs:877-966` (`land_worker`): if `lane` or `base` is `None`, run `commit_board` (existing fallback); else call `land::land_lane(&spec, None)`, log `dropped`, handle `Nothing` → `commit_board`, `AlreadyLanded` / `Landed` → `record`, `Dirty` → `Landed::Waiting`, `Conflict` → `Landed::Conflict`.
  - `src/closeout.rs:625-720` (`land_worker`): if `base` is `None`, run `nothing()` (existing fallback); else pass `before_commit` callback for `Closer::Polish` that runs `stand_ins` and checks `prd.md`'s `authorized_tests` frontmatter against `repo/prd.md` (returning `vec![prd_rel]` if tampered!), call `land::land_lane`, log `dropped`, handle `Nothing` → `nothing()`, `AlreadyLanded` / `Landed` → `record`, `Dirty` → `Landed::Waiting`, `Conflict` → `Landed::Conflict`.
- `tests/closeout.rs` (or `tests/checkpoints.rs` — purely additive new test): add `unified_land_lane_handles_checkpoint_polish_conflicts_and_claim_kill_recovery` proving all 3 missing driver scenarios:
  1. **Checkpoint conflict:** main tree commits a conflicting change to `shared.txt` while a checkpoint worker edits the same lines in its lane → driver parks with `CheckpointBlocked` naming `shared.txt` and leaves main tree clean.
  2. **Polish conflict:** main tree commits a conflicting change to `shared.txt` while the polish worker edits the same lines in its lane → driver parks with `CloseoutBlocked` naming `shared.txt` and leaves main tree clean.
  3. **Claim-to-spawn crash recovery:** put a run into the exact post-reset/post-claim, pre-worker-record crash state (`state.toml` slice 1 `status = "building"`, `lane = 1`, `state.json` has `lane_bases[1]`, no worker recorded for slice 1, plus a marker file in lane 1), resume the run, and verify the driver respawns cleanly on lane 1, does not duplicate commits, and finishes the slice green.

**Trap**: `before_commit` MUST NOT run when `land_lane` returns `AlreadyLanded` or reuses a held ref — otherwise a resumed polish driver would spuriously emit `TemplateStandIn` and flag `pr-body.md` for an already-landed commit!

**Acceptance:** `unified_land_lane_handles_checkpoint_polish_conflicts_and_claim_kill_recovery` passes alongside all existing `tests/{merge,commits,checkpoints,closeout}.rs` tests, and zero duplicated `commit_lane` → `undo_half_pick` → `land::land` loops remain in `scheduler.rs`, `checkpoint.rs`, or `closeout.rs`.

### 3. Convert the 3 authorized multi-lane ordering tests from `delay_ms` to `wait_sh` barriers (`tests/commits.rs`, `tests/lanes.rs`) — high — blocked by: 2
**Source**: §10 FR-8: "Convert the three multi-lane ordering tests listed in `authorized_tests` (`tests/commits.rs::a_dirty_path_in_the_main_tree_parks_the_landing_and_keeps_the_owners_edit`, `tests/commits.rs::a_park_with_no_question_carries_its_reason`, `tests/lanes.rs::two_lanes_build_two_slices_at_once_and_the_third_waits_for_its_dependency`) from fixed `delay_ms` sleeps to deterministic `wait_sh` file barriers. Leave single-worker kill/stop/status `delay_ms` tests (including `tests/merge.rs:139`) untouched."

- `tests/commits.rs`: in `a_dirty_path_in_the_main_tree_parks_the_landing_and_keeps_the_owners_edit` and `a_park_with_no_question_carries_its_reason`, replace the `delay_ms: 1500` / `delay_ms: 3000` plan steps with `wait_sh` barrier files (e.g. worker 1 waits on a release file written after the test observes both workers active / dirties the main tree; worker 2 waits on a second release file or completes after worker 1 — mirroring `two_lanes_land_out_of_order_*` in `tests/commits.rs`).
- `tests/lanes.rs`: in `two_lanes_build_two_slices_at_once_and_the_third_waits_for_its_dependency`, replace the two `delay_ms: 1500` plan steps with a `wait_sh` barrier file that the test touches right after asserting both slice 01 and slice 02 are simultaneously `building` and slice 03 is still `pending`.
- Verify zero `delay_ms` remain inside those three functions and run all three repeatedly (`cargo test --test commits --test lanes`) to confirm deterministic, fast passes.

**Trap**: Do NOT remove lines from any OTHER pre-existing test function outside the 3 listed in `prd.md`'s `authorized_tests` (notably leave `tests/merge.rs:139` untouched!) or `testguard` will block the checkpoint!

**Acceptance:** The three converted multi-lane ordering tests contain zero `delay_ms` and pass cleanly under `cargo test --test commits --test lanes`.

### 4. Verify — medium — blocked by: 1, 2, 3
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 5. Prove — at the checkpoint — blocked by: 4
**Source**: §12 QA plan rows FR-4 and FR-8:
- FR-4: `run polish_and_ci_fix_respawns_keep_their_lane_edits` (evidence: test transcript)
- FR-8: `run unified_land_lane_handles_checkpoint_polish_conflicts_and_claim_kill_recovery` (evidence: test transcript)

**Acceptance:** after the checkpoint, both tests (and all prior FR-1..FR-8 rows at the final checkpoint) run under `cargo test` with full gate green and transcripts recorded in Progress.
