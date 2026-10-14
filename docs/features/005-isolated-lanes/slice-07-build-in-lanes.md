# Slice 07 — Build workers in lanes

**Highest risk**: this slice moves every build worker out of the shared tree and replaces the pathspec commit with a landing. It is the spine of PRD-005: 08 to 10 build on it, and a mistake here loses a worker's edits or lands a slice before its dependency.

**Source**: [`prd.md`](prd.md) → §4 FR-2 (every worker runs in its lane: build workers here), §5 FR-3 (the lane committed whole, landed by cherry-pick, a dirty main tree parks, dependents wait for the landing), §3 FR-1 (a slice taking a free lane resets it; setup in the lane), §8 FR-6 (the slice half of the guessing code goes). **Depends on**: 01 (the stub's `sh` step and `scenario/` through the common dir, which every test here runs on), 02 (`FACTORY_BOARD`, without which a lane worker's `docs set` writes the lane's board), 03 (`lanes::dir`, `ensure`, `reset`, setup in a lane), 04 (`land::commit_lane` and `land::land`).
**Created**: 2026-10-14

**Progress**: _(written as it lands — what was built, the gate numbers, what a review caught)_

**Slice**: After this slice a build worker runs in its own lane worktree, with the lane's env, `FACTORY_REPO` set to the lane and `FACTORY_BOARD` to the main tree, so its board writes still land in the main tree. A slice taking a free lane resets it to the feature tip first; a respawn keeps the lane as the worker left it. When the worker exits with its slice `done`, the driver commits the lane whole and lands it on the feature tip by cherry-pick, one commit per slice with today's subject and the board files. A dependent slice waits until its dependency has landed. A dirty path in the main tree parks the run; a conflicting landing blocks the slice for now (slice 08 replaces that with a merge worker). The slice pathspec commit and its leftover tests are deleted. Checkpoint, fix, polish and CI-fix workers still run in the main tree: slices 09 and 10 move them.

## Current code / environment

- **The spawn**: `Worker::spawn` (`src/worker.rs:253-341`) sets `.current_dir(&repo)` (`:294`) and `cmd.env(REPO_ENV, &repo)` (`:322`), `repo` being `run.state().repo`. The env is built from scratch (`:298-322`): only what the code sets reaches the worker. Five callers: `src/scheduler.rs:733` (build), `src/checkpoint.rs:274` and `:592`, `src/closeout.rs:240` and `:1647`. `FACTORY_BOARD` is slice 02's `pub const` beside `REPO_ENV`.
- **Lane assignment**: a `building` slice with a lane keeps it (`src/scheduler.rs:379-391`, `b.lane`); one without takes `free.remove(0)`. A ready slice takes `free.remove(0)` and gets `building` plus its lane in `state.toml` (`:411-429`). Neither resets anything today. `free_lanes` (`:659-673`) is the board's free lanes minus `blocked` ones and the table's. The board frees a slice's lane as soon as the slice is `done` (`src/docs/board.rs:322-328`), before the driver has reaped its worker.
- **`spawn_on`** (`:687-757`): loads the lane file from the main tree (`lanes::load(&repo, lane)`), runs setup once per run with `lane_def.setup(&repo)` (`:699-706`, `lanes_set_up` in `state.json`, `src/run.rs:145`, `:466-472`), then snapshots the tree (`:732`, `snapshot(run, &slice_key(slice))`) into `Running.changed_before` (`:81`) for the leftover check.
- **The exit**: `route_exit` (`:789-864`), on `done`, calls `commit_slice` (`:958-1056`): `Pathspec::from`, `leftovers`, the `UntrackedLeftovers` note and event, `commit::commit` by pathspec, `set_worker_commit`, `SliceCommitted { slice, sha, files }`, and only then `settle` ("The commit before the outcome: a driver killed in between leaves the worker unsettled, and the resume commits the slice", `:802-803`).
- **The resume**: `settle_orphans` (`:1172+`) runs every tick before any spawn (`:317`, and `:273` on a stop). Its `done` arm (`:1223-1245`) loads the spawn baseline and calls `commit_slice` for a dead, unsettled worker. That is the retry path a landing needs.
- **The off-branch park** (`:215-244`): nothing is reaped and nothing spawns while HEAD is off the run's branch; `park_with` (`:605`) says the line once and sets `waiting-owner`; "HEAD is back on {b} — the run goes on" resumes it.
- **Dependents**: `ready` is every `pending` slice whose `depends_on` are all `done` (`src/docs/board.rs:306`). A worker writes `done` before it exits (`assets/contract-slice.md:19`), so today a dependent can be ready while its dependency is unlanded. `board.ready` holds `SliceRef`s with no deps; `board.slices[].depends_on` has them (`src/docs/mod.rs:209-220`). `has_unsettled(run, slice)` (`:675`) exists.
- **What stays for 09 and 10**: `snapshot`, `load_baseline`, `unnamed`, `checkpoint_key` and `Pathspec::from_stream` are still called by `src/checkpoint.rs:273`, `:591`, `:697`, `:788` and `src/closeout.rs:239`, `:447`, `:1646`, `:1756`, `:1852`, `:1885`. Only the slice path goes here: `commit_slice`, `leftovers` (its `skip` at `:1161`), `Running.changed_before`, the slice baseline at `:732` and `:1236`. `Pathspec::from` (`src/commit.rs`) is `pub` in a lib crate, so clippy will not flag it: leave it for slice 10's deletion order.
- **The stub**: its call record (`tests/stubs/claude:111-125`) holds `n`, `argv`, `cwd`, the `FACTORY_*` env and more, but not HEAD. It sets `building` and sleeps `delay_ms` before any edit (`:389-391`), touches `step.touch ?? ['src/lib.txt']` (`:430`), then logs a decision and sets `done` (`:456-466`). Its decision body says "the fixture has one code file on purpose". Two lanes appending to `src/lib.txt` from one base conflict on the second pick, so every two-lane test that runs slices 1 and 2 side by side would block.
- **The contract**: `assets/contract-slice.md:20`: "do not commit: the driver commits your slice's files when you return … `git add -A` and `git add .` claim nothing, and an unclaimed new file is left out and listed." `assets/` is outside the skill bundle (slice 06's lock).
- **Tests that pin today's cwd**: `tests/loop.rs:228` (`FACTORY_REPO`) and `:230` (`cwd`) equal the fixture; `tests/worker.rs:156` (`FACTORY_REPO`). `tests/stubs.rs:136` and `:193` assert the default touch is `src/lib.txt`; they run the stub directly, so they change with the default and nothing else.
- **`tests/commits.rs`**: `:223` (pathspec, no markdown log), `:356` (two lanes out of order), `:407` (killed at the commit; its pre-commit hook kills the driver at the first `git commit`, and hooks are shared by every worktree of the repo). The rest are the leftover rules FR-6 deletes: `:465`, `:517`, `:544`, `:564`, `:617`, `:644`, `:669`, with helpers `leftover_notes` (`:158`) and `leftovers_of` (`:609`).
- **A test that dirties the main tree through a slice**: `tests/checkpoints.rs:1365` makes every checkpoint gate run on a dirty tree through slice 1's `stray: ["notes.txt"]` (`:1373`). In a lane that stray is committed with slice 1, the main tree stays clean, the green is no longer pinned, and checkpoint 2 reuses it: `:1390-1391` go red. Write `notes.txt` into the main tree from the test before `start`, untracked, in place of the stray; slice 09 deletes the pin and rewrites the test.
- **Deliberate non-changes**: *(prd §16 "A shared-tree mode. Deleted, with no flag to bring it back.")* *(prd §16 "Not handled: a worker landing a change to `docs/project.toml`. Accepted 2026-10-13, owner's cut" — the drop list is the board and protected files only.)* *(prd §16 "Not handled: an abandoned slice's unlanded edits. … the next reset discards what never landed." The reset saves nothing.)* *(prd §16 "Not handled: re-seeding after a lockfile change mid-run." `ensure` never re-seeds a path the lane has.)*

## Tasks

### 1. The stub: one file per slice, HEAD in the call, a linger — high — blocked by: —
**Where**: `tests/stubs/claude` (`:111-125`, `:430`, `:456-466`, the header comment), `tests/stubs.rs:136`, `:193`.

**Source**: §5 FR-3 acceptance: "the dependent's first stub call records a HEAD that holds its dependency's commit." "A dependent slice whose dependency's worker has written `done` but not yet exited is not spawned until that dependency lands". "Two lanes landing in turn give two commits".

Three changes. The call record gains `head`: `git rev-parse HEAD` in cwd, `null` when it fails. A slice step with no `touch` touches `src/slice-NN.txt` (its own slice number), so slices in two lanes never edit the same file by default; fix, polish and checkpoint keep `src/lib.txt`. A new step key `linger_ms` sleeps after `done` is set and before the stream is written, so a test can hold a worker that has written `done` and not exited. Update the decision body and the header.

**Acceptance:** `tests/stubs.rs`: a slice step with no `touch` writes `src/slice-02.txt` for slice 02; `calls/1.json`'s `head` equals the fixture's HEAD; a step with `linger_ms: 1500` shows `done` in `state.toml` while the child is still running. The `:136` and `:193` asserts follow the new default.

### 2. Build workers spawn in their lane — high — blocked by: —
**Where**: `src/worker.rs` (`spawn`), its five callers, `tests/loop.rs:228-230`, `tests/worker.rs:156`.

**Source**: §4 FR-2: "Every role runs in a lane … Cwd is the lane, the env is the lane's (`FACTORY_LANE` first), and `FACTORY_REPO` is the lane path, so the hook's guards hold there." "The driver sets an env var … that points `factory docs set`, `factory docs log add`, `factory docs retro --set` and `factory docs status` at the main tree".

`spawn` takes the worker's cwd. It runs the worker there, sets `FACTORY_REPO` to it, and always sets `FACTORY_BOARD` to the run's main tree. The build caller passes its lane's dir (`lanes::dir`); the other four pass the main tree until slices 09 and 10.

**Acceptance:** `tests/loop.rs:228-230` assert the lane's canonical path for `cwd` and `FACTORY_REPO`, and `FACTORY_BOARD` equal to the fixture; `tests/worker.rs:156` likewise. A two-lane run in `tests/lanes.rs`: every slice call's `cwd` is the canonical `lane-N` path of the lane `state.toml` gave it, its env holds `FACTORY_LANE=N` and the lane file's `FACTORY_LANE_DB=fixture_N`, and the main tree's `state.toml` and `log.jsonl` hold each stub's `building`, `done` and decision.

### 3. A slice taking a free lane resets it; setup in the lane — high — blocked by: 2
**Where**: `src/scheduler.rs` (`:379-391`, `:411-429`, `spawn_on`), `src/run.rs` (state beside `lanes_set_up`).

**Source**: §3 FR-1: "A slice taking a free lane resets it to the feature tip … A respawn, or a resume after an answer, keeps the lane as the worker left it: a `building` or `blocked` slice holds its lane". "Setup runs inside the lane, cwd the lane, with the lane's env. In a run, once per lane, at that lane's first spawn (today's rule)."

At both `free.remove(0)` sites, before the spawn: `lanes::ensure` the lane, `lanes::reset` it to the main tree's HEAD, and record that sha as the lane's base in `state.json` (a map lane → sha with `serde(default)`), so a resumed driver can commit the lane. A respawn with `b.lane` set does neither, but still calls `ensure`, which leaves an existing lane as it is (slice 03 left a resumed run's lanes to this slice). Setup runs with the lane dir as cwd, still once per run.

**The trap**: a resumed run skips preflight (`src/cli/run.rs:79-107`), so a lane may be missing at a respawn. `ensure` it at every spawn, not only at a reset.

**Acceptance:** `tests/lanes.rs`, `lanes = 1`: the lane file's setup appends `pwd -P` to `scenario/lane-setup.log` through the common dir; after the run that log names the lane's path once. Slice 1's step blocks; the test writes `kept.txt` into slice 1's lane (`calls/1.json`'s `cwd`) and answers `BLOCK-1`; the respawn's step is `{"outcome":"done","sh":"test -f kept.txt"}` and the landed slice-01 commit holds `kept.txt`. Then the test writes `junk.txt` into the lane, and slice 2's step `{"outcome":"done","sh":"test ! -e junk.txt"}` succeeds; its `head` is slice 01's landed commit.

### 4. A done slice lands — high — blocked by: 2, 3
**Where**: `src/scheduler.rs` (`route_exit`, `settle_orphans`, `commit_slice` and `leftovers` deleted, `Running.changed_before` deleted), `assets/contract-slice.md:20`, `tests/commits.rs`.

**Source**: §5 FR-3: "When a worker exits and the board says its slice is `done` (routing unchanged …), the driver runs `git add -A` in the lane. It drops the feature's `state.toml` and `log.jsonl` and the protected files … A slice landing emits `WorkerWroteProtectedFile` … then makes one commit on the lane's base with today's subject. A commit the worker made itself is squashed into it." "The driver lands it by cherry-pick onto the feature tip in the main tree, and adds the main tree's board files to the same commit." "The contracts say so: `assets/contract-slice.md:20` … become 'everything not gitignored in your lane is committed'." §8 FR-6: "the pathspec and leftover tests in `tests/commits.rs` … replaced by FR-3".

In the `done` arm, in place of `commit_slice`: `land::commit_lane(lane, base, tool_files + protected_files, subject, refs/factory/<NNN>/slice-NN)` with today's subject (`commit::subject(board, "feat", "slice-NN <title>")`), a `WorkerWroteProtectedFile` event and today's line for each `dropped` path, then `land::land(main, sha, tool_files)`. `Landed` → `set_worker_commit` with the landed sha, `SliceCommitted` with its files, a line, then `settle`, in that order. `None` from `commit_lane` → "nothing to commit", as today.

The same function serves `settle_orphans`'s `done` arm, and it must be safe to call again: the worker row already has a commit → settle only; the ref resolves → land it; else `commit_lane` first. Contract line 20 becomes: do not commit; the driver commits everything not gitignored in your lane when you return, so leave the lane as the commit should be.

`tests/commits.rs`: rewrite `:223` as one commit per slice from its lane, no markdown log; keep `:356` with the new default touch; rewrite `:407` twice, killed at the lane commit (the first `git commit`: the resume runs `commit_lane` and lands) and killed at the landing (the second: the ref survives and the resume lands it). Delete `:465`, `:517`, `:544`, `:564`, `:617`, `:644`, `:669`, `leftover_notes` and `leftovers_of`.

**The trap**: the board frees a `done` slice's lane before the worker is reaped (`src/docs/board.rs:322-328`). On a resume, a ready slice could take and reset that lane before its edits are committed. `settle_orphans` already runs before the spawn loop (`:317`); keep `commit_lane` first inside it, so the ref holds the work before any reset.

**Acceptance:** in `tests/commits.rs`: (a) a slice whose step is `sh` only (`git mv` of a tracked file, `printf >` a new file, `sed -i` on a tracked file, `mkdir` with a file inside, `ln -s` to that dir), ending with `cp -a` of those paths into `$FACTORY_STUB_DIR/lane-copy`: `git show --stat` of its landed commit lists every path, and `git status --porcelain` in the main tree is empty after the run; (b) that commit, checked out alone with `git worktree add --detach`, holds each of those files byte-equal to `lane-copy` and the symlink with the same target; (c) slices 1 and 2 side by side land as two commits, one parent each, subjects `feat(001): slice-01 A` and `feat(001): slice-02 B`, each holding `state.toml` and `log.jsonl`; (d) both kill variants end with one slice-01 commit, a settled worker and one `slice_done` event; (e) no `refs/factory/` ref remains after the run.

### 5. A dirty main tree parks the run — high — blocked by: 4
**Where**: `src/scheduler.rs` (the landing, the loop).

**Source**: §5 FR-3: "A dirty main tree parks the run. When a path the pick touches is dirty in the main tree (board files aside), the run parks with a line naming the paths, as the off-branch refusal does (`src/commit.rs:435-451`). The owner's edit stays untouched." Acceptance: "A main-tree file dirty in a path the pick touches parks the run, the park line names it, and the file keeps the owner's content."

`Dirty { paths }` leaves the worker unsettled (its ref holds the work) and parks through `park_with`: "slice NN cannot land: <paths> changed in the main tree — commit or discard them". Nothing spawns while a landing waits; live workers run on. `settle_orphans` retries the landing each tick; once it lands the run goes on, as after the off-branch park.

**Acceptance:** `tests/commits.rs`: slice 1 with `delay_ms: 2000`; meanwhile the test writes `owner\n` to `src/slice-01.txt` in the main tree. The driver log holds one park line naming `src/slice-01.txt`, `state.json`'s phase is `waiting-owner`, the file still reads `owner\n`, and slice 2 does not spawn. The test deletes the file: slice 01 lands and the run goes on to finish.

### 6. A conflicting landing blocks the slice (until slice 08) — medium — blocked by: 4
**Where**: `src/scheduler.rs`.

**Source**: §6 FR-4: "A conflicting cherry-pick goes to a merge worker … The driver aborts the pick in the main tree, which stays untouched." §13 OPEN-2: "BLOCK on markers left, three attempts …, or a third conflict". The merge worker is slice 08; until then a conflict must not lose the work or leave the run spinning.

`Conflict { paths }`: settle the worker, set the slice `blocked`, and add a BLOCK through `answers::add_block` (`src/answers.rs:251`) with the slice and no phase, naming the paths and the ref. The ref stays. Slice 08 replaces this branch.

**Acceptance:** `tests/commits.rs`: a committed `shared.txt`; slices 1 and 2 side by side, each with `sh` rewriting its line 3: one lands, the other is `blocked` with a block entry naming `shared.txt`, its `refs/factory/001/slice-NN` resolves, and the main tree's `git status --porcelain` is empty.

### 7. A dependent waits for the landing — high — blocked by: 4
**Where**: `src/scheduler.rs:411-429` (the ready loop).

**Source**: §5 FR-3: "A slice counts as done for its dependents only once it has landed. … So the driver does not spawn a ready slice while one of its dependencies is still in its table". Acceptance (quoted in task 1).

Skip a ready slice when any of its `depends_on` (from `board.slices`) is in the table or `has_unsettled`.

**Acceptance:** `tests/lanes.rs`, the default fixture (slice 3 depends on 2): slice 2's step has `linger_ms: 3000`. Slice 3's first call has a `head` with slice 02's landed commit as an ancestor (`git merge-base --is-ancestor`), and its `n` is after slice 2's call.

### 8. Verify — medium — blocked by: 1, 2, 3, 4, 5, 6, 7
`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 9. Prove — at the checkpoint — blocked by: 8
Rows from [`prd.md`](prd.md) §14 this slice proves, below the UI:

- **FR-3**: "run a harness slice that edits only through Bash; check out its landed commit alone and compare its files with the lane's; run a dependent slice" — the task 4 (a), (b) tests and the task 7 test, plus task 5's park.
- **FR-2**, the build half: "run a two-lane harness run; read each `calls/N.json` and the main tree's board" — the task 2 test.

**Acceptance:** after the checkpoint, Progress holds the transcripts, the `git show --stat` of the Bash-only commit, the dependent's recorded HEAD, and the park line.
