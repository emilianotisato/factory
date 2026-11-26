# Slice 01 — Main-tree identity, atomic seeds and open-run lane guards

**Source**: [`prd.md`](prd.md) → §7 FR-5, §8 FR-6, §9 FR-7, §12 QA plan. **Depends on**: — (foundational lane/repo identity fixes that slices 02–04 build on).
**Created**: 2026-11-26

**Progress**: _(written as it lands — what was built, the gate numbers, what a review caught)_

**Slice**: Unifies repository matching on `lanes::main_tree` across `run::list_all`, `cli::run` resume, `cli::lanes`, and `cli::status --here`; stages lane seed copies atomically outside the lane worktree before renaming; and makes `lanes up` and `lanes up --prove` refuse during an open run while having `--prove` reset each lane to `HEAD` before running lane setup. Leaves `lanes clean` to slice 02, gate memory caps / reaping to slice 03, and `land_lane` / respawn lane retention to slice 04.

## Current code / environment

- **`src/lanes.rs:139-150` (`main_tree`)**: resolves any linked worktree or lane back to the canonical main checkout via `git rev-parse --path-format=absolute --git-common-dir`.
- **`src/lanes.rs:209-289` (`ensure`, `seed_one`)**: `seed_one` copies directly into `to = lane.join(relative)`. A killed copy leaves a half-copied `to` that the next run sees as `Seeded::Kept` (`:269-270`). Staging inside the lane worktree would expose untracked `.tmp` files to `land::commit_lane`'s `git add -A` (`src/land.rs:47`) — stage outside the worktree in `lane.parent()` (`data/lanes/<key>/`), which is on the same filesystem so `fs::rename` is atomic!
- **`src/cli/lanes.rs:47-184` (`up`, `prove`, `prune`)**: `prune` (`:156-169`) refuses when `run::list_open(data, main)` is non-empty, while `up` (`:47-82`) and `prove` (`:84-151`) never check. `prove` calls `up` and then runs `inner` on whatever stale commit each lane sat on; resetting AFTER setup would wipe setup's non-ignored output with `clean -fd`, so `prove` must reset each lane to `HEAD` AFTER `lanes::ensure` and BEFORE `lane.run_setup`.
- **`src/run.rs:583-588` (`list_all`)** and **`src/cli/status.rs:178-180` (`same_repo`)**: compare `s.repo == repo` / `repo.canonicalize() == top` literally, missing runs started in a linked worktree (or from the main tree when queried from a worktree).
- **`src/cli/run.rs:84-102` (resume path)**: checks `commit::branch_refusal(&repo, &branch, ...)` against the caller's cwd `&repo` instead of `run.state().repo` and passes the caller's `&docs` instead of reopening `Docs::open(&run.state().repo)?`.
- **Deliberate non-changes (`prd.md` §14)**: do NOT edit `skills/lanes/SKILL.md`, do NOT touch `tests/lanes.rs`'s existing tests (purely additive new tests only so `testguard` stays green), and keep `src/lanes.rs` / `src/cli/lanes.rs` free of plan-id strings.

## Tasks

### 1. Stage seed copies atomically outside the lane worktree (`src/lanes.rs`) — high — blocked by: —
**Source**: §7 FR-5: "Stage the temp copy outside the lane worktree, in `lane.parent()` (`data/lanes/<key>/`, on the exact same filesystem so `fs::rename(&tmp, &to)` stays atomic while never sitting inside any git working tree): `tmp = lane_parent.join(format!(".seed-tmp-lane-{n}-{}.tmp", std::process::id()))` ... Sweep any stale `.seed-tmp-lane-{n}-*.tmp` entries in `lane_parent` inside `lanes::ensure` before seeding ... On any failure (copy or rename), `remove(&tmp)?` and return `Err`."

- `src/lanes.rs:209-289`:
  - Add a small `sweep_seed_temps(parent: &Path, n: u32)` helper called in `lanes::ensure` before seeding: removes any entry in `parent` whose filename starts with `format!(".seed-tmp-lane-{n}-")` and ends with `".tmp"`.
  - Update `seed_one(main: &Path, lane: &Path, n: u32, rel: &str)` (or derive `lane_parent = lane.parent()`) to stage into `tmp = lane_parent.join(format!(".seed-tmp-lane-{n}-{}.tmp", std::process::id()))`:
    - Keep the fast `to.symlink_metadata().is_ok()` → `Ok(Seeded::Kept)` check first.
    - Remove any leftover `tmp` (`remove(&tmp)?`).
    - Ensure `to.parent()` exists (`fs::create_dir_all`).
    - Try `cp(CLONE, &from, &tmp)`: if it succeeds, `fs::rename(&tmp, &to)` (removing `tmp` on rename error) → `Ok(Seeded::Clone)`.
    - Otherwise `remove(&tmp)?`, try `cp(COPY, &from, &tmp)`: if it succeeds, `fs::rename(&tmp, &to)` (removing `tmp` on rename error) → `Ok(Seeded::Copy)`.
    - Otherwise `remove(&tmp)?` and return `Err(...)`.
- Add unit test `seed_copy_uses_atomic_temp_sibling_and_cleans_up_on_failure` in `src/lanes.rs` proving: (1) a successful seed produces `to` with identical contents and leaves zero `.seed-tmp-*` files in either `lane` or `lane.parent()`; (2) a failed copy (e.g. unwritable destination parent or failing copy) leaves `to` absent and zero `.seed-tmp-*` behind so a subsequent `seed_one` does not return `Seeded::Kept`; (3) a pre-existing stale `.seed-tmp-lane-1-99999.tmp` in `lane.parent()` is swept by `lanes::ensure` and never enters the lane worktree.

**Trap**: `lane.parent()` is `data/lanes/<key>/` (same filesystem as `data/lanes/<key>/lane-N/<rel>`), so `fs::rename` is atomic without risking `git add -A` ever picking up a partial seed inside the worktree.

**Acceptance:** `seed_copy_uses_atomic_temp_sibling_and_cleans_up_on_failure` passes and fails if `seed_one` copies directly to `to` or fails to sweep stale `.seed-tmp-lane-*` files.

### 2. Unify `lanes::same_repo` across `run`, `lanes`, and `status --here`, and fix worktree resume (`src/lanes.rs`, `src/run.rs`, `src/cli/run.rs`, `src/cli/status.rs`) — high — blocked by: 1
**Source**: §9 FR-7: "Add `pub fn same_repo(a: &Path, b: &Path) -> bool` in `src/lanes.rs`: Resolve `key(p) = main_tree(p).ok().or_else(|| p.canonicalize().ok())` on both `a` and `b` ... Wire `lanes::same_repo` into `run::list_all` ... `cli::status::same_repo` ... in `src/cli/run.rs:84-93`, check `commit::branch_refusal(&run.state().repo, &branch, default.as_deref())?` against `run.state().repo` and re-open `Docs::open(&run.state().repo)?`".

- `src/lanes.rs`: add `pub fn same_repo(a: &Path, b: &Path) -> bool` using `main_tree(p).ok().or_else(|| p.canonicalize().ok())` on both sides.
- `src/run.rs:583-588` (`list_all`): change `.filter(|s| s.repo == repo)` to `.filter(|s| crate::lanes::same_repo(&s.repo, repo))` (keep `list_open` calling `list_all` with canonical/raw path cleanly).
- `src/cli/status.rs:178-180` (`same_repo`): delegate to `crate::lanes::same_repo(repo, top)`.
- `src/cli/run.rs:84-108`: on resume (`Run::find_resumable`), check `commit::branch_refusal(&run.state().repo, &branch, default.as_deref())?` on `run.state().repo`, and open `let run_docs = Docs::open(&run.state().repo)?;` so `launch(&mut run, &run_docs, ...)` operates on the run's own checkout tree!
- `tests/lanes.rs` (purely additive new test): add `linked_worktree_shares_main_tree_across_status_lanes_and_run` proving: on a repo with a linked `git worktree`, (1) an open run in the main tree is returned by `factory status --here` and `factory status --here --json` from the linked worktree, and `factory lanes prune` / `factory lanes up` from the linked worktree refuse; (2) a resumable run created in the linked worktree on its feature branch (while main tree is on `main`) is found by `Run::find_resumable` from the main tree, shown by `factory status --here` from the main tree, and `commit::branch_refusal` checks the run's own linked worktree branch rather than refusing because main is on `main`.

**Trap**: `Run::create` (`src/run.rs:247`) stores the canonical checkout path where the run was started — keep that intact so commits land in the tree the run owns, while `same_repo` ensures every repo-wide lookup/refusal sees all worktrees of that git repo as one!

**Acceptance:** `linked_worktree_shares_main_tree_across_status_lanes_and_run` passes and fails if `list_all` or `cli::status::same_repo` compares raw checkout paths or if `cli::run` checks `branch_refusal` on `&repo` instead of `&run.state().repo`.

### 3. Refuse `lanes up` and `--prove` during an open run, and reset lanes before setup in `--prove` (`src/cli/lanes.rs`) — high — blocked by: 2
**Source**: §8 FR-6: "Extract `fn refuse_if_open(data: &Path, main: &Path, verb: &str, out: &mut dyn Write) -> Result<bool>` ... Call `refuse_if_open` at the top of `up` (`"up"`), `prove` (`"prove"` — checked BEFORE `up` so `--prove` always prints `"prove refused"`), `prune` (`"prune"`) ... extract a shared `up_lanes(data, main, reset_to_head: bool, out)` helper ... when `reset_to_head` is `true`, reset each lane to `main`'s `HEAD` immediately after `lanes::ensure` and BEFORE running `lane.run_setup`".

- `src/cli/lanes.rs:47-184`:
  - Add `pub(crate) fn refuse_if_open(data: &Path, main: &Path, verb: &str, out: &mut dyn Write) -> Result<bool>` using `run::list_open(data, main)?` and printing `"  ✗ {verb} refused: this repo has an unfinished run, which holds its lanes"` + each open run line.
  - Extract `fn up_lanes(data: &Path, main: &Path, reset_to_head: bool, out: &mut dyn Write) -> Result<i32>`:
    - Checks `project::seed(main)?` as `up` does today.
    - When `reset_to_head` is `true`, resolves `head = commit::git(main, &["rev-parse", "HEAD"])?.trim().to_string()` once.
    - For each lane `1..=count(main)?`: runs `lanes::ensure`, if `reset_to_head` calls `lanes::reset(&report.dir, &head)?` **BEFORE** running `lanes::load(main, n).and_then(|l| l.run_setup(&report.dir))`, then prints the line.
  - `up`: calls `if refuse_if_open(data, main, "up", out)? { return Ok(1); }`, then `up_lanes(data, main, false, out)`.
  - `prove`: calls `if refuse_if_open(data, main, "prove", out)? { return Ok(1); }`, then `up_lanes(data, main, true, out)` (returning early if non-zero), then runs `inner` in parallel across lanes.
  - `prune`: replaces its inline open-run check with `if refuse_if_open(data, main, "prune", out)? { return Ok(1); }`.
- `tests/lanes.rs` (purely additive new test): add `lanes_up_and_prove_refuse_open_run_and_prove_resets_to_head` proving: (1) `factory lanes up` prints `up refused: this repo has an unfinished run` and `factory lanes up --prove` prints `prove refused: this repo has an unfinished run` (both exit `1`, running no setup); (2) with no open run, `factory lanes up --prove` resets a lane sitting on an older commit with a stray untracked file to `HEAD` BEFORE running setup (so a non-ignored file created by setup survives into `inner` while seeded ignored state is kept and the older stray file is gone).

**Trap**: Resetting AFTER `up` would run `lanes::reset` (`clean -fd`) AFTER `lane.run_setup`, deleting non-ignored files setup created! Always reset BEFORE running `run_setup`.

**Acceptance:** `lanes_up_and_prove_refuse_open_run_and_prove_resets_to_head` passes and fails if `up`/`prove` allow running during an open run or if `prove` resets after setup instead of before.

### 4. Verify — medium — blocked by: 1, 2, 3
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 5. Prove — at the checkpoint — blocked by: 4
**Source**: §12 QA plan rows FR-5, FR-6, FR-7:
- FR-5: `run seed_copy_uses_atomic_temp_sibling_and_cleans_up_on_failure` (evidence: test transcript)
- FR-6: `run lanes_up_and_prove_refuse_open_run_and_prove_resets_to_head` (evidence: test transcript)
- FR-7: `run linked_worktree_shares_main_tree_across_status_lanes_and_run` (evidence: test transcript)

**Acceptance:** after the checkpoint, all three tests run under `cargo test` with full gate green and transcripts recorded in Progress.
