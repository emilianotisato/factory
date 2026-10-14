# Slice 08 — Merge worker

**Source**: [`prd.md`](prd.md) → §6 FR-4 (a conflicting pick spawns a merge worker in the slice's lane; the BLOCK rules; a clean exit lands), §13 OPEN-2. **Depends on**: 07, which lands slices from lanes and, on a conflict, blocks the slice with the ref kept. This slice replaces that interim branch. It also edits `src/scheduler.rs` and `src/worker.rs`, which 09 and 10 edit next, so they follow it one at a time.
**Created**: 2026-10-14

**Progress**: built 2026-10-14.

- **Built.** `Job::Merge { slice, attempt }`, role `merge`, after `build` in `ROLE_ORDER`; `merge_spawned` and `landing_conflicted` events. `Pack::for_merge` (`# Conflict` with the paths, the lane commit's message and `--stat`, then `# Slice`, the answer when there is one, the log, `assets/contract-merge.md`, autonomy, inner loop). On `Landing::Conflict`, `start_merge` resets the slice's lane to HEAD as its new base, redoes `cherry-pick --no-commit`, logs `landing_conflicted`, and sets the slice `building`; the build worker settles `done`. The building loop treats a slice with a lane and a resolving ref as merging. It spawns `Job::Merge` on the lane as left, with `workers.build`'s config, and counts merge deaths only (`deaths_since_answer` takes a role). `route_merge` routes a merge worker by role in `route_exit` and `settle_orphans`, in this order: an open block entry, a usage limit (the lane is kept, AD-11), a death, `<<<<<<<`/`>>>>>>>` at a line start in a conflicted path (a driver BLOCK), else `land_slice(merged)`. That commits the lane on its new base with the ref commit's message, lands it, and sets the slice `done`. A conflict on that landing goes round again, and the third since the last answer is a driver BLOCK (`CONFLICT_CAP`). `commit_lane` stages unmerged entries before `reset --soft`. A resumed driver settles a worker whose `landing_conflicted` follows its spawn.
- **Tests.** `tests/merge.rs`: the spawn (cwd, `# Conflict` pack, `workers.role`, slice 02 `building` and HEAD slice 01 while it runs), (a) resolved and landed, (b) markers left, (c) two merge deaths with no third call, (d) a block the worker logged. Unit tests: the merge pack, the conflict count around an answer, the marker rule. The slice-07 conflict test in `tests/commits.rs` is gone.
- **Inner loop.** 1 run, 0 red, 1.4 min.
- **Deviations.** AD-10 (the merge respawn keeps the ref and the lane; answers only after the conflict), AD-11 (a rate-limited merge stays `building`), AD-12 (the two events, the conflict logged after the pick), AD-13 (tests (a) and (b) read their acceptance slightly differently).
- **Prove.** FR-4's two runs, (a) and (b), are left for the checkpoint.
- **Prove (CP-3), FR-4 (a) resolved.** `tests/merge.rs` spawn and resolved tests green. The merge call (`3.json`, `# Conflict`) runs with `cwd` `…/lane-2`, the same lane as slice 02's build call. Final `git log --oneline`: `chore: a shared file`, `feat(001): slice-01 A`, `feat(001): slice-02 B`, `feat(001): slice-03 C`, `docs(001): checkpoint 1`, then polish and ship; `shared.txt` line 3 is `both`: `evidence/cp-3-fr-4-merge-worker.txt`.
- **Prove (CP-3), FR-4 (b) markers left.** The markers test green; `factory status` reads `blocked: 02` and `lanes: 2 (1 parked) — 1: free · 2: slice 02 blocked`, BLOCK-1 "slice 02's merge left conflict markers in shared.txt", main tree HEAD `feat(001): slice-01 A`, lane 2's `shared.txt` keeps the markers: `evidence/cp-3-fr-4-merge-worker.txt`.

**Slice**: After this slice a landing that conflicts no longer blocks at once. The driver leaves the main tree untouched, resets the slice's lane to the new feature tip, redoes the pick there with the conflict left in the files, sets the slice back to `building`, and spawns a merge worker in that lane. The merge worker gets the slice file, the slice commit's message and diff stat, and the conflicted paths. It may change anything in its lane, runs the inner loop until green, and never touches the slice's status. When it exits clean and no conflicted file still holds a marker, the driver lands the lane as slice 07 does and sets the slice `done`. Markers left, a BLOCK the worker logged, two merge deaths, or a third conflict for the slice block it instead.

## Current code / environment

- **The interim branch**: slice 07 handles `land::Landing::Conflict { paths }` by settling the worker, setting the slice `blocked` and adding a driver BLOCK naming the paths; `refs/factory/<NNN>/slice-NN` still holds the lane commit. Its test in `tests/commits.rs` (two slices rewriting line 3 of `shared.txt`) is rewritten here.
- **`land` never writes on a conflict**: slice 04 checks with `git merge-tree --write-tree` before any pick, so "the driver aborts the pick in the main tree" (prd §6) is already true: there is no pick to abort.
- **Job kinds**: `Job` (`src/worker.rs:166-197`) has `Build`, `Fix`, `Checkpoint`, `Polish`, `CiFix`; `role()` is the `workers.role` column; `slice()` (`:198-203`) is `Some` for `Build` only. Roles in reports follow `ROLE_ORDER` (`src/cli/report.rs:518`, `["build", "fix", "checkpoint", "polish"]`), also read by `src/retro.rs:172`. Config roles are `ROLE_KEYS` (`src/config.rs:143`); a new key there is a new knob.
- **Packs and contracts**: each role has a builder in `src/pack.rs` (`for_fix` at `:262-316` is the short pattern: a first section, the log, `# Contract`, `# Autonomy`, `# Inner loop`, written to `packs/<role>-…md`) and a contract `include_str!`'d at `:23-27`. The stub tells a merge pack by its first section, `# Conflict` (slice 01, `tests/stubs/claude` role detection); it may run `sh`, log a block with `--slice` and no phase, and never sets status.
- **The table and routing**: the table is keyed by slice (`src/scheduler.rs:84`), so a merge worker in it holds the slice's lane and its dependents wait (slice 07's check). `route_exit` (`:789-864`) routes by the slice's board status: a merge worker exits with the slice `building`, which is the `_` arm, a dead worker. `settle_orphans` (`:1214-1223`) routes any orphan with a slice the same way.
- **Respawns and deaths**: the `building` loop (`:350-405`) gives a slice with nothing alive a fresh build worker on its lane, or blocks it after `DEATH_CAP` (`:66`, 2) deaths through `block_dead_slice` (`:1595`). `deaths_since_answer` (`:1554-1580`) counts every dead worker of the slice since its last answer, whatever its role.
- **Blocks**: `open_block` (`:938`) finds the slice's open block entry; `emit_blocked` (`:943`) emits `SliceBlocked`. A driver-written block goes through `answers::add_block` (`src/answers.rs:251`).
- **git, checked 2026-10-14 on 2.55.0**: `git cherry-pick --no-commit` that conflicts leaves `UU` entries and no `CHERRY_PICK_HEAD`. `git reset --soft` then refuses: "Cannot do a soft reset in the middle of a merge." After `git add -A` it succeeds.
- **Deliberate non-changes**: *(prd §6 "It may change anything in its lane (owner's call)". No file allowlist, no diff check against the slice's files.)* No `[workers.merge]` config: the merge worker uses `workers.build`'s model and effort.

## Tasks

### 1. The merge job, pack and contract — high — blocked by: —
**Where**: `src/worker.rs` (`Job`), `src/pack.rs` (`for_merge`, a `CONTRACT_MERGE` const), `assets/contract-merge.md` (new), `src/cli/report.rs:518`.

**Source**: §6 FR-4: "It spawns a merge worker there: a new job kind and contract, the pack holding the slice file, the slice commit's message and diff stat, and the conflicted paths." "It may change anything in its lane". "It runs the inner loop until green." "the merge worker does not touch the slice's status".

`Job::Merge { slice, attempt }`, role `merge`, `slice()` `Some`. `Pack::for_merge`: first section `# Conflict` (the conflicted paths, one per line; the lane commit's message; `git show --stat` of it), then `# Slice` (the slice file's text), the log section, `# Contract`, `# Autonomy`, `# Inner loop`. The prompt: resolve the conflicted landing of slice NN of PRD-NNN as the appended system prompt says, and return the STATUS block. `merge` goes into `ROLE_ORDER` after `build`.

The contract, in the other contracts' voice: you are in the slice's lane, mid-pick, the conflict left in the files; resolve every conflicted path and remove every marker; you may change anything in the lane; run the inner loop in the foreground until green, 3 attempts on the same failure → block; do not commit and do not run `docs set` (the driver owns the status); a block is `docs log add {NNN} --kind block --slice {N}`; everything not gitignored in the lane is committed when you return. Return `STATUS: done | blocked`, `INNER:`, `BLOCK:`.

**Acceptance:** a unit test in `src/pack.rs`: a merge pack for slice 2 with conflicted `shared.txt` starts with `# Conflict`, names `shared.txt`, holds the lane commit's subject and its `--stat` line, the slice file's text, and the contract with `{NNN}` and `{N}` filled. `Job::Merge { slice: 2, .. }.role()` is `merge` and `.slice()` is `Some(2)`.

### 2. A conflict spawns the merge worker in the slice's lane — high — blocked by: 1
**Where**: `src/scheduler.rs` (07's conflict branch, the spawn), `src/events.rs` (a conflict event), `src/run.rs` (the lane's base).

**Source**: §6 FR-4: "The driver aborts the pick in the main tree, which stays untouched. It redoes the pick in the slice's lane on the new tip, leaving the conflict in that worktree. … The driver sets the slice back to `building`, so it keeps its lane, its dependents wait, and no checkpoint counts it."

On `Conflict { paths }`: emit a new `LandingConflicted { slice, paths }` event; `lanes::reset` the slice's lane to the main tree's HEAD and record that as the lane's base; `git cherry-pick --no-commit <ref sha>` in the lane, whose nonzero exit is expected; set `slices.N.status=building` (the lane stays); spawn `Job::Merge` with `cwd` the lane, the lane's env, and `workers.build`'s config, into the table under the slice. Settle the build worker as `done`: its work is in the ref.

**Acceptance:** `tests/merge.rs` (new), the fixture with a committed `shared.txt`; slices 1 and 2 side by side, each `sh` rewriting line 3; slice 2 with `delay_ms: 1500` so slice 1 lands first. The third stub call's `cwd` is slice 2's lane, its pack starts with `# Conflict` and names `shared.txt`, and the run DB's `workers` row for its session has role `merge`. While it runs (`delay_ms` on the merge step), `state.toml` shows slice 2 `building` and HEAD is slice 01's commit.

### 3. The merge worker's exit — high — blocked by: 2
**Where**: `src/scheduler.rs` (`route_exit`, `settle_orphans`, the `building` respawn loop, `deaths_since_answer`), `src/land.rs` (`commit_lane`).

**Source**: §6 FR-4: "It is a BLOCK when a conflict marker remains in a file that was conflicted (the driver checks), when the worker logs a BLOCK after three attempts on the same failure …, or when it dies twice. A BLOCK sets the slice `blocked`, answered like any other." "A clean exit lands as FR-3. If that landing conflicts again because the tip moved, one more merge worker runs; the third conflict for one slice is a BLOCK." "The driver sets it `done` again once the merge lands".

Route a `merge` worker by its role before the board status, in `route_exit` and in `settle_orphans`:

- an open block entry for the slice → set the slice `blocked`, `SliceBlocked`, settle `blocked`;
- dead or rate-limited → as today's build worker, but the respawn is a merge worker: a `building` slice whose `refs/factory/` ref resolves gets `Job::Merge` on its lane as left, and an answered one gets the answer section too. Two merge deaths since the last answer → `block_dead_slice`;
- a clean exit → read each path of the last `LandingConflicted` in the lane; a line starting `<<<<<<<` or `>>>>>>>` → a driver BLOCK naming the files, slice `blocked`. Else `commit_lane` with the lane's base and the ref commit's message (`git log -1 --format=%B`), then `land` as slice 07 does: `Landed` → set `done`, `SliceCommitted`, settle `done`; `Dirty` → slice 07's park; `Conflict` → task 2 again, unless this is the slice's third `LandingConflicted` since its last answer, which is a driver BLOCK naming the paths.

**The trap**: `commit_lane` runs `git reset --soft <base>` before `git add -A` (slice 04, task 1), and a merge worker that resolved a file without `git add` leaves `UU` entries, so the reset dies (see Current code). Stage first when `git ls-files -u` is not empty, then reset, then `add -A`. And `deaths_since_answer` counts the build worker's deaths too: count `merge` deaths only for the merge cap.

Markers are checked at the start of a line and only for `<` and `>`: a line of `=======` is a Markdown heading underline as often as a marker.

**Acceptance:** in `tests/merge.rs`: (a) the merge step `sh` writes the resolved line 3: HEAD's last two subjects are `feat(001): slice-01 A` then `feat(001): slice-02 B`, `shared.txt` holds the resolved text, `state.toml` shows slice 2 `done`, the main tree's `git status --porcelain` is empty, and no `refs/factory/` ref remains; (b) the merge step has no `sh`, so the markers stay: `factory status 001` shows slice 2 `blocked`, the log holds a block naming `shared.txt`, HEAD is slice 01's commit, and `git grep -n '^<<<<<<<'` in the main tree finds nothing; (c) two merge steps `dead`: slice 2 `blocked` after the second, and no third merge call; (d) the merge step logs a block (`outcome: blocked`): slice 2 `blocked`, `state.toml` written by the driver, not the stub. A unit test on the conflict count: a slice with two `LandingConflicted` events since its last answer blocks on the next conflict; one with two before an answer and none after spawns a merge worker.

### 4. Verify — medium — blocked by: 1, 2, 3
`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 5. Prove — at the checkpoint — blocked by: 4
Rows from [`prd.md`](prd.md) §14 this slice proves, below the UI. **FR-4**: "run the two-lane conflict scenario, once resolved and once with markers left" — tests (a) and (b) of task 3, with `factory status` read after (b).

**Acceptance:** after the checkpoint, Progress holds both transcripts, the merge call's recorded `cwd`, the final `git log --oneline` of (a), and `factory status` showing slice 2 `blocked` in (b).
