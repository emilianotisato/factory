# Slice 09 — Checkpoint in a free lane

**Source**: [`prd.md`](prd.md) → §7 FR-5 (the driver works in the lowest free lane reset to HEAD; the gate, fix and checkpoint workers run there and land as FR-3; the cache key is the gated tree), §8 FR-6 (the checkpoint half of the guessing code goes), §13 OPEN-9. **Depends on**: 08. It needs 07's `Worker::spawn` cwd, lane base and landing, and it edits `src/scheduler.rs` (`settle_checkpoint_orphan`, `spawn_on`'s setup) right after 08 does. Slice 10 moves polish and CI fix the same way and deletes what this slice leaves.
**Created**: 2026-10-14

**Progress**: _(written as it lands — what was built, the gate numbers, what a review caught)_

**Slice**: After this slice a checkpoint runs in the lowest-numbered lane that no `building` or `blocked` slice holds, reset to the feature tip. The driver runs the lane's setup there once per run, runs the gate there with the lane's env, and keys the gate cache on that tree, so a dirty main tree neither turns the gate green nor pins a green. Fix and checkpoint workers run in the same lane; their work is committed whole and landed on the feature tip as slice 07 lands a slice, and the lane is reset to the new HEAD before the gate runs again. With no free lane the checkpoint waits, and the run parks on the blocks that hold the lanes. The checkpoint's pathspec commits, its leftover notes and its spawn baselines are deleted. Polish and CI fix still run in the main tree: slice 10 moves them.

## Current code / environment

- **The checkpoint**: `checkpoint::run` (`src/checkpoint.rs:66-355`) is synchronous: the scheduler calls it (`src/scheduler.rs:443`) once the table is empty (`:431-441`) and nothing else spawns until it returns. So between a reset and a landing only the owner can move HEAD. It gates (`:125-195`), then spawns the checkpoint worker (`:196-354`), and returns `Outcome::Done`, `Parked`, `Stopped` or `Budget`.
- **The gate**: `gate_once` (`:434-576`) keys the cache with `gatecache::tree_key(repo, …)` (`:450`) and runs `sh -c <gate>` with `.current_dir(repo)` and `.env_remove("FACTORY_LANE")` (`:504-508`). `tree_id` (`src/gatecache.rs:78-99`) is the working tree minus `docs/`, untracked files included: in the main tree an untracked file changes the key, and after a lane reset the key is HEAD's tree minus `docs/`. The dirty pin is `:532-544` (`commit::dirty_outside_docs`, `Entry.checkpoint`, `src/gatecache.rs:30-33`), read by `fits` (`:453`).
- **The fix**: `fix_once` (`:578-669`) snapshots (`:591`), spawns in the main tree (`:592`), and calls `commit_fix` (`:674-730`) for every exit, dead or limited too: `Pathspec::from_stream`, `scheduler::unnamed`, `note_leftovers` (`:735-771`), `commit::commit` by pathspec, `FixCommitted`. `tests/checkpoints.rs:1324` holds a dead fix worker's edit committed before the gate reruns.
- **The checkpoint commit**: `commit_checkpoint` (`:774-816`) commits the whole feature folder (`board.dir`, so the CP entry in `log.jsonl` and the Progress lines) plus what the stream named, as `docs(NNN): checkpoint n`. The stub checkpoint worker (`tests/stubs/claude:241-283`) writes only a log entry through `docs log add`, which slice 02's `FACTORY_BOARD` sends to the main tree: in a lane it changes no file.
- **Done-detection needs no change**: the worker's `CP-n` and blocks reach the main tree's `log.jsonl` through `FACTORY_BOARD`, and `docs.status(&feature)` (`:293`) and `open_checkpoint_blocks(&feature_dir)` (`:269`) read the main tree.
- **Free lanes**: the board's `free_lanes` (`src/docs/board.rs:322-328`) are the lanes no `building` or `blocked` slice holds. The scheduler's checkpoint branch (`src/scheduler.rs:442-463`) runs before the blocked park (`:464-468`), so a due checkpoint with every lane held would be entered each tick.
- **Setup runs only in `spawn_on`** (`src/scheduler.rs:699-706`, once per run through `lanes_set_up`, `src/run.rs:466-472`). A lane the run never gave a slice has had no setup, and slice 07 made setup run in the lane.
- **Orphans**: `settle_orphans` sends a dead driver's fix and checkpoint workers to `settle_checkpoint_orphan` (`src/scheduler.rs:1287-1410`), which loads the spawn baseline (`load_baseline`, `checkpoint_key`) and calls `commit_checkpoint` or `commit_fix`. `WorkerRef` (`src/run.rs:172-185`) records no lane, and only a slice has one on the board.
- **The contracts**: `assets/contract-checkpoint.md:1` ("the lanes are drained"), `:16` ("the driver … does the second when you return, staging `{feature dir}/` and the files you wrote"); `assets/contract-fix.md:1`, `:13` ("against the tree you leave"), `:17` ("A file you made with a command is yours only if you `git add` it by name or by its folder").
- **Tests that pin today**: `tests/checkpoints.rs:1340` writes the dead fix worker's edit into the main tree; `:1365` is the dirty pin (slice 07 makes it dirty from the test); `:475` reads `gate.log` (`:531-533`) without `pwd` or `head`.
- **Deliberate non-change**: *(prd §16 "Not handled: a driver gate after polish.")* That is slice 10's; nothing here gates closeout.

## Tasks

### 1. The checkpoint takes the lowest free lane — high — blocked by: —
**Where**: `src/scheduler.rs` (the checkpoint branch, a lane-preparing fn shared with `spawn_on`), `src/checkpoint.rs` (`run`), `src/run.rs` (`WorkerRef`).

**Source**: §7 FR-5: "The driver works in the lowest-numbered free lane, reset to HEAD. … the driver takes the lowest lane that no `building` or `blocked` slice holds, lane 1 when free. No free lane: the checkpoint waits, and the run is already parked on those blocks."

The checkpoint branch runs only when `board.free_lanes` is not empty; else it falls through to the blocked park. `checkpoint::run` takes the first free lane and, before each gate run and each worker spawn: `lanes::ensure`, `lanes::reset` to the main tree's HEAD, the lane's base recorded in `state.json` as slice 07 records it. Move `spawn_on`'s once-per-run setup into one fn both callers use, cwd the lane. `WorkerRef` gains `lane: Option<u32>` (`serde(default)`), set at every spawn into a lane, so an orphan finds its lane.

**Acceptance:** in `tests/checkpoints.rs`, two lanes, the fixture's three slices: slice 1's step blocks; slices 2 and 3 are done, so checkpoint 1 is due while slice 1 holds lane 1. `fx.gate_log()`'s one line has `pwd` the canonical `lane-2` path; the checkpoint call's `cwd` is the same path; the run then parks on `BLOCK-1`. The lane file's setup line in `scenario/lane-setup.log` names lane 2 once.

### 2. The gate runs in the lane — high — blocked by: 1
**Where**: `src/checkpoint.rs` (`gate_once`), `src/gatecache.rs` (`Entry`).

**Source**: §7 FR-5: "The gate runs in that lane, cwd the lane, with the lane's env. The cache key is the tree the gate ran on: HEAD's tree without `docs/`. A dirty main tree no longer pins it."

`gate_once` takes the lane: `tree_key(lane, …)`, `.current_dir(lane)`, the lane's env (`Lane::env()`) in place of `env_remove`. Delete the dirty pin (`:532-544`) and its comment. `Entry.checkpoint` and the `fits` check that reads it go too: nothing writes it any more.

**The trap**: `tree_id` counts untracked files, so the key is only HEAD's tree because the lane was reset just before, not because of anything in `tree_key`. Reset before every gate run, including the rerun after a fix lands.

**Acceptance:** in `tests/checkpoints.rs`: `:475` asserts, for each `gate.log` line, `pwd` a canonical `lane-N` path and `head` the sha of the last slice commit before `docs(001): checkpoint n` (`feat(001): slice-02 B`, then `feat(001): slice-03 C`). A new test: the declared gate is `./gate.sh && test -f needed.txt`, and the test writes `needed.txt` untracked into the main tree before `start`: checkpoint 1's first `gate_run` is red, and the run parks on the three-red block. `:1365` becomes "a dirty main tree does not pin the cache": with `notes.txt` untracked in the main tree, checkpoint 2's gate is cached green (`gate_shape(&rows, 2)` `[(false, true, 1)]`) and `gate.log` has one line.

### 3. Fix and checkpoint workers run in the lane and land — high — blocked by: 1
**Where**: `src/checkpoint.rs` (`run`, `fix_once`, `commit_fix`, `commit_checkpoint`), `src/scheduler.rs` (`settle_checkpoint_orphan`), `assets/contract-checkpoint.md`, `assets/contract-fix.md`.

**Source**: §7 FR-5: "Fix, checkpoint, polish and CI-fix workers run in that lane, and their result lands as FR-3. After a fix lands, the gate runs again in the lane reset to the new HEAD." §8 FR-6: "`note_leftovers` (`src/checkpoint.rs:735`)" goes; "The contract lines about claiming a file by `git add` (… the fix and CI-fix twins) go with it."

Spawn both with `cwd` the lane and the lane's env. On exit, in place of `commit_fix` and `commit_checkpoint`'s bodies: `land::commit_lane(lane, base, tool_files + protected_files, subject, ref)` with today's subjects (`fix(NNN): gate red at checkpoint n`, `docs(NNN): checkpoint n`) and refs `refs/factory/<NNN>/checkpoint-n-fix` and `…-checkpoint`, then `land::land(main, sha, tool_files)`. `Landed` → `set_worker_commit`, `FixCommitted` or `CheckpointDone`, then settle, as today. Each dropped protected path gets today's line ("… which no fix may — left out of the commit"). A fix commits however its worker ended, as today. `settle_checkpoint_orphan` lands the same way, through slice 07's order: a row with a commit → settle only; a ref that resolves → land it; else `commit_lane` on `w.lane`.

The other outcomes: `Dirty` → say slice 07's park line once with `checkpoint n` in place of `slice NN`, set `waiting-owner`, and retry the landing each tick inside `checkpoint::run` (nothing else is live), returning `Outcome::Stopped` on a stop; landed, the phase goes back to `checkpoint`. `Conflict` (only the owner moved HEAD) → a checkpoint block (`answers::add_block`, phase `checkpoint`) naming the paths and the ref, and `Parked`.

Delete `note_leftovers`, the two `scheduler::snapshot` calls (`:273`, `:591`), the two `load_baseline` calls in `settle_checkpoint_orphan`, and the `Pathspec::from_stream` calls in this file. Contracts: line 1 of each says the worker runs in a lane reset to the feature tip, nothing else live; `contract-checkpoint.md:16` and `contract-fix.md:17` say the driver commits everything not gitignored in the lane when you return, except `state.toml` and `log.jsonl`, which the tool writes; leave the lane as the commit should be. `contract-fix.md:13` keeps "against the tree you leave".

**The trap**: the stub's checkpoint worker, and any real one that proves without fixing, changes no file in its lane, so `commit_lane` returns `None` while the main tree holds the new `CP-n` entry. Then commit the main tree's board files by pathspec (`commit::commit(main, tool_files, subject)`), or `docs(NNN): checkpoint n` never carries its entry and `assert_cp` (`tests/checkpoints.rs:421-433`) fails.

**Acceptance:** in `tests/checkpoints.rs`: `:688` (red, red, green) passes with each fix call's `cwd` a lane and each `fix(001): gate red at checkpoint 1` holding `src/lib.txt` only; `:1324` writes `src/lib.txt` into the fix call's `cwd` (from `calls/3.json`) instead of the main tree and keeps its asserts; `:1399` (the orphan checkpoint worker) passes unchanged; `:475`'s `assert_two_checkpoints` passes, and the main tree's `git status --porcelain` is empty at the end. A new test: the fix step `sh` writes `src/fixed.txt` and runs no tool call naming it; `fix(001): gate red at checkpoint 1` holds `src/fixed.txt`.

### 4. Verify — medium — blocked by: 1, 2, 3
`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 5. Prove — at the checkpoint — blocked by: 4
Rows from [`prd.md`](prd.md) §14 this slice proves, below the UI. **FR-5**: "run a harness checkpoint with `gate.sh` recording `pwd`, a main-tree-only file, and lane 1 blocked" — the task 2 tests (`pwd` and HEAD at each checkpoint; the main-tree-only `needed.txt`) and the task 1 test (lane 1 blocked, gate in lane 2). Its polish rows are slice 10's.

**Acceptance:** after the checkpoint, Progress holds the three transcripts, with the `gate.log` lines showing each `pwd` and `head`.
