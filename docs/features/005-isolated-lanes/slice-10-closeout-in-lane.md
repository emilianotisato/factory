# Slice 10 — Closeout in a lane, guessing deleted

**Source**: [`prd.md`](prd.md) → §7 FR-5 (polish and CI-fix workers run in the lowest free lane at HEAD and land as FR-3; no driver gate after polish), §8 FR-6 (the guessing is deleted), §4 FR-2 (the polish half of its proof), §11 FR-9 (this repo's lane files drop the shared-tree prose), §13 OPEN-9. **Depends on**: 09. It needs 09's lane-preparing fn, its `WorkerRef.lane` and its Dirty wait, and it edits `src/closeout.rs` and `src/scheduler.rs` after 08 and 09. FR-6's deletions also need every caller gone first: 07 moved the slice path, 09 the checkpoint path, and this slice moves the last two.
**Created**: 2026-10-14

**Progress**:

- **Built.** `polish()` and `fix_round()` prepare the lowest free lane through `scheduler::prepare_lane` (lane 1 at closeout), delete a stale `refs/factory/<NNN>/polish` or `ci-fix-<round>`, and spawn with the lane's env, cwd and `WorkerRef.lane`. On a clean exit one `land_worker` (`src/closeout.rs`, the `Closer` enum for the two roles) lands before the worker settles. It takes the first match of: the row's commit, then the subject already on `base..HEAD`, then the ref, then `commit_lane` (polish drops the board files and `docs/index.md`, CI fix the protected files), then `land`. `Landed` records the commit and emits `PolishDone` / `CiFixCommitted` once per sha, then settle `done`, then `file_friction` for polish. `None`: polish commits the main tree's board files under its subject. `Dirty` parks and waits as slice 09 does. `Conflict` writes a closeout block (AD-17). The retro stand-in goes into the lane. `settle_orphan` lands through the same fn on `w.lane`; a dead worker, or one with no lane, settles dead and the closeout runs again.
- **Deleted (FR-6).** `POLISH_KEY`, `ci_fix_key`, `note_leftovers`, `commit_polish`, `commit_ci_fix`; the scheduler baselines and `unnamed`; `src/commit.rs`'s `Pathspec` family with `WRITING_TOOLS`, `slice_path`, `relativize`, `changed`, `untracked`, `dirty_outside_docs`, `blobs` and their unit tests (`push_once` kept for `expand`); the three leftover events; `tests/closeout.rs`'s unnamed-file test and `track_extra`. New module doc in `src/commit.rs`. A unit test reads the three deleted kinds as `EventKind::Unknown` with nothing skipped.
- **Contracts.** `contract-polish.md` and `contract-ci-fix.md` say the worker runs in a lane reset to the feature tip, and the driver commits everything not gitignored there; the `git add` claim lines are gone.
- **Tests.** The green-closeout test asserts polish's cwd is lane 1 and the main tree clean at the end. New: a polish `sh` file lands in the one polish commit; a CI fix orphan killed mid-step lands once on resume. The polish orphan test asserts one polish commit holding `src/lib.txt` and one polish worker. The CI fix test asserts lane 1. `tests/metrics.rs` slugs the polish call's cwd, and the recount test asserts it is lane 1. Two earlier tests now expect polish in a lane (AD-19).
- **Lane files and README.** The shared-tree paragraph is gone from `docs/reference/lanes/lane-1..4.md` (frontmatter untouched). `README.md:60` names lanes as worktrees. `README.md:17`'s "committing by pathspec" became "committing a lane", since the task 5 grep covers the whole README.
- **Greps.** `grep -rnE "from_stream|fn unnamed|fn leftovers|untracked-before" src`: exit 1. `grep -rnE "Leftovers|_leftovers|Pathspec" src tests`: three hits, all prose in `tests/fixture/prd-004/log.jsonl`, left as is (AD-18, decide). `grep -rnE "same working tree|by pathspec|next driver PRD" docs/reference/lanes README.md`: exit 1.
- **Inner loop.** 4 runs, 2 red, 4.4 min. Red 1 was `cargo fmt --check` on this slice's files. Red 2 was `tests/lanes.rs` still keeping polish in the main tree. Last run: 562 passed.
- **Left for the checkpoint (task 7).** The FR-6 grep and full gate; FR-5's polish row (recorded cwd, `git show --stat` of the polish commit with `src/polished.txt`); FR-2's polish row (`factory report --per-worker` polish `subagents` 2).
- **CP-4 FR-6:** the §8 grep over `src` prints nothing, exit 1; the gate is green, 1 run, 562 passed, 1.4 min — evidence: `evidence/cp-4-fr-6-grep-and-gate.txt`
- **CP-4 FR-5 (polish):** the polish call's `cwd` is `data/lanes/repo-…/lane-1` at head `docs(001): checkpoint 2`; `fix(001): polish review fixes` holds `src/polished.txt`, main tree clean — evidence: `evidence/cp-4-fr-5-polish-in-lane.txt`
- **CP-4 FR-2 (polish):** `factory report --per-worker` reads `worker 5 polish … subagents 2`, the call's `cwd` lane 1, subagents read under the lane's slug — evidence: `evidence/cp-4-fr-2-polish-report.txt`


**Slice**: After this slice the polish worker and each CI fix worker run in the lowest free lane, reset to the feature tip, with the lane's env. Their result lands as slices do: `commit_lane`, then `land`, with today's subjects. No driver gate runs after polish. With no caller left, the guessing code goes: `Pathspec` and its stream readers, `unnamed`, the spawn baselines, the three leftover events and the leftover notes. This repo's own lane files lose their shared-tree paragraph, and the README stops calling isolated lanes "next". It runs alone and last.

## Current code / environment

- **Polish today**: `polish()` (`src/closeout.rs:164-339`) takes a snapshot (`scheduler::snapshot(run, POLISH_KEY)`, `:239`) and spawns `Job::Polish` with `&[]` for env (`:240-246`), so in the main tree. On a clean exit it settles `done` first (`:312`), records `FINDINGS` (`:313-326`), then calls `commit_polish` (`:327`), emits `PolishDone { sha, files }` (`:328-331`) and calls `file_friction` (`:332`).
- **`commit_polish`** (`:398-468`) writes the stand-in retro into the main tree when missing (`:408-419`, `repo.join(&retro_rel)`), writes the PR body fallback into the run dir (`:420-432`) and runs `flag_body`. It then reads `Pathspec::from_stream` (`:434`). It keeps `prd.md` and strips only `docs/index.md` (`:436-441`), adds `state.toml` and the retro (`:441-445`), sweeps through `scheduler::unnamed` (`:447`) and `note_leftovers` (`:448`), and commits `fix(NNN): polish review fixes` by pathspec (`:450-454`).
- **`PolishDone` is how the closeout knows polish is over**: `polished` is true once the events hold one (`src/closeout.rs:104-108`). A polish worker settled but never landed has no `PolishDone`, so a resumed closeout spawns a fresh polish worker.
- **`file_friction` reads the main tree's retro**: `repo.join(&board.dir).join("retro.md")` (`:348-349`).
- **CI fix today**: `fix_round` (`:1579`) keeps `branch_guard` first (`:1591-1595`). It takes a snapshot (`scheduler::snapshot(run, &ci_fix_key(round))`, `:1646`) and spawns `Job::CiFix` with `&[]` (`:1647-1653`). A clean exit settles `done` (`:1717`) before `commit_ci_fix` (`:1718-1727`). `commit_ci_fix` (`:1736-1787`) reads `from_stream` (`:1747`) and prints a line per protected path (`:1748-1753`). It sweeps through `unnamed` (`:1756`) and `note_leftovers` (`:1757`), commits `fix(NNN): CI <check>` by pathspec and emits `CiFixCommitted { round, sha, files }`.
- **Orphans**: `settle_orphan` (`:1794-1887`) settles a returned polish or CI fix worker `done`, then commits through `load_baseline` (`:1852`, `:1885`) and `commit_polish` or `commit_ci_fix`. It reads the CI round from `CiFixSpawned` and the check from the last `CiRed`. Slice 09 added `WorkerRef.lane`, set at every spawn into a lane.
- **The contracts**: `assets/contract-polish.md:1` ("You are the only worker running: every slice is done and the last checkpoint is committed") and `:19` ("the driver commits what you wrote … A file you made with a command is yours only if you `git add` it by name or by its folder"). `assets/contract-ci-fix.md:1` ("You are the only worker running") and `:15` (the same `git add` rule). `contract-slice.md:20` is slice 07's, and `contract-fix.md` and `contract-checkpoint.md` are slice 09's.
- **What FR-6 deletes, after slices 07 and 09**: in `src/scheduler.rs`, the baselines `baseline_path`, `checkpoint_key`, `snapshot`, `load_baseline` (`:866-919`, `runs/<id>/untracked-before/<key>.json`), `Unnamed` and `unnamed` (`:1061-1137`). In `src/closeout.rs`, `POLISH_KEY`, `ci_fix_key` and `note_leftovers` (`:1889-1933`). In `src/commit.rs`, `WRITING_TOOLS` (`:29`), `slice_path` (`:47`), `Pathspec` with `from` and `from_stream` (`:57-123`), `written_paths`, `heredoc_paths`, `added_paths`, `rooted_scripts`, `program`, `literal`, `real_dir` (`:128-235`), `relativize` (`:245`), `Changed` and `changed` (`:285`), `untracked` (`:326`), `dirty_outside_docs` (`:336`, its one caller `src/checkpoint.rs:536` is gone in 09) and `blobs` (`:342`). In `src/events.rs`, `UntrackedLeftovers` (`:127`), `CheckpointLeftovers` (`:139`) and `CloseoutLeftovers` (`:149`). Line numbers are today's; 07 to 09 move `scheduler.rs`.
- **What stays in `src/commit.rs`**: `protected_files`, `tool_files`, `commit` (`:379`), the branch hold (`:426-474`), `expand` (`:483`, called by `src/closeout.rs:655` for the ship commit) with `push_once` (`:236`), and `subject`. `src/shell.rs` stays: `src/cli/hook.rs:113-226` and `src/stream.rs:238` use it. The module doc (`:1-17`) describes the pathspec rule ("Never `git add -A`: another lane may be mid-write in the same tree"), which is no longer true.
- **Old event logs still read**: an unknown kind parses as `EventKind::Unknown` (`src/events.rs:342-346`, `#[serde(other)]`), so a past run's `untracked_leftovers` line does not break `factory report` once the variant is gone.
- **Tests that pin today**: `tests/closeout.rs:1801` (unnamed and listed files, `closeout_leftovers`) goes with FR-6. `tests/closeout.rs:367` asserts the polish commit's files are the retro, `state.toml` and `src/lib.txt` (`:469-476`) and `polish_done` has 3 files (`:661`). `:981` asserts `fix(001): CI test` holds `src/lib.txt` only (`:1029`). `:2227` is the polish orphan. The `src/commit.rs` unit tests `:586`, `:599`, `:630`, `:726` and `:780` test deleted functions. `:681`, `:746` and `:807` test `commit` and the branch hold, which stay.
- **The subagent transcript is keyed by the worker's cwd**: the stub writes subagents under the slug of its cwd (`tests/stubs/claude:603-604`), and slice 02 made the driver read them there. `tests/metrics.rs:684-689` `subagents_dir` still slugs the run's repo, so `:722`'s `is_dir` fails once polish runs in a lane. `recount_rewrites_the_tokens_and_tool_calls_from_the_streams` (`tests/metrics.rs:907`) reads `factory report --json --per-worker` and asserts polish `subagents` 2 (`:989`): that is FR-2's polish row.
- **This repo's lane files**: `docs/reference/lanes/lane-1..4.md:12-18` say "All lanes edit the same working tree … the driver commits by pathspec". The frontmatter (`env: [CARGO_TARGET_DIR=target/lane-N]`) and `:7-10` stay. `README.md:60` says "Lanes share a working tree today; isolated lanes are the next driver PRD."
- **Deliberate non-changes**: *(prd §16 "Not handled: a driver gate after polish. Accepted 2026-10-13: the polish worker's last gate ran on the tree that lands (FR-5).")* Add no gate between the polish landing and the ship. *(prd §16 "Parallel polish fixers.")* One polish worker, one lane. *(prd §16 "A shared-tree mode. Deleted, with no flag to bring it back.")* Delete the guessing outright; keep no fallback. `skills/gate/SKILL.md:61` is stale too, but no FR names it: leave it, since a `skills/` edit needs a version bump (slice 06).

## Tasks

### 1. Polish runs in a lane and lands — high — blocked by: —
**Where**: `src/closeout.rs` (`polish`, `commit_polish`), `assets/contract-polish.md:1`, `:19`, `tests/closeout.rs`, `tests/metrics.rs:684-689`.

**Source**: §7 FR-5: "Fix, checkpoint, polish and CI-fix workers run in that lane, and their result lands as FR-3." "No driver gate after polish. Nothing else is live, so the pick lands the tree the polish worker's last gate ran on, less any protected path it wrote (FR-3)." §5 FR-3: "every role drops `docs/index.md`, and every role but polish drops the feature's `prd.md`." "Driver work lands the same way: a fix, checkpoint, polish or CI-fix result, with its subject from today". Acceptance: "The polish stub's recorded cwd is a lane, and its edits land as one `fix({NNN}): polish review fixes` commit."

Before the spawn, prepare the lane with slice 09's lane-preparing fn: the first of `board.free_lanes`, which is lane 1 at closeout since every slice is done. Run `ensure`, reset to the main tree's HEAD, record the base, and run setup once per run. Delete the snapshot (`:239`). Spawn `Job::Polish` with `cwd` the lane and the lane's env, `WorkerRef.lane` set. On a clean exit, record `FINDINGS` as today, then land:

1. The retro stand-in and the PR body fallback, as today, but the retro goes into the lane: `<lane>/<board.dir>/retro.md`.
2. `land::commit_lane(lane, base, tool_files + ["docs/index.md"], subject(board, "fix", "polish review fixes"), refs/factory/<NNN>/polish)`. Print a line for a dropped `docs/index.md`.
3. `land::land(main, sha, tool_files)`.
4. `Landed` → `set_worker_commit`, `PolishDone` with the landed commit's sha and files, then settle `done`, then `file_friction`.

Slice 07 sets the order: land before settle. `None` from `commit_lane` → commit the main tree's board files by pathspec under the same subject, as slice 09 does for a checkpoint. `Dirty` → slice 09's wait, with `polish` in the park line. `Conflict` → a closeout block with phase `polish`, naming the paths and the ref, then `Parked`.

Contract `:1`: you run in a lane, reset to the feature tip, and nothing else is live. `:19`: do not commit; the driver commits everything not gitignored in your lane, except `state.toml` and `log.jsonl`, which the tool writes. It commits as `fix({NNN}): polish review fixes` when you return. Leave the lane as the commit should be. Keep the `docs ship` / `git push` / `gh` line.

**The trap**: today the stand-in retro goes to `repo.join(&retro_rel)` (`:409-411`), the main tree. In a lane that file is not in the lane commit, so the polish commit loses it. Write it into the lane, and test for it in the lane. `file_friction` reads the main tree's retro (`:348-349`), so call it only after `Landed`. Today's order settles before committing (`:312`, `:327`). A driver killed between the two leaves polish settled with no `PolishDone`, and the next polish worker's lane reset discards the edits. Land first.

**Acceptance:** `tests/closeout.rs:367` passes with these extra asserts:
- the polish call's `cwd` (the sixth `calls/N.json`) is a canonical `lane-1` path, not the fixture root;
- the polish commit's files are still the retro, `state.toml` and `src/lib.txt`;
- `polish_done` holds 3 files;
- the main tree's `git status --porcelain` is empty at the end.

The stand-in tests `:847`, `:890` and `:908` pass unchanged. A new test: the polish step's `sh` writes `src/polished.txt` with `printf`, and no tool call names it. `fix(001): polish review fixes` holds `src/polished.txt`, and it is the only commit with that subject. In `tests/metrics.rs`, `subagents_dir` slugs the polish call's `cwd`. `recount_rewrites_the_tokens_and_tool_calls_from_the_streams` passes and also asserts that the polish call's `cwd` is a lane path.

### 2. CI fix runs in a lane and lands — high — blocked by: 1
**Where**: `src/closeout.rs` (`fix_round`, `commit_ci_fix`), `assets/contract-ci-fix.md:1`, `:15`.

**Source**: §7 FR-5: "Fix, checkpoint, polish and CI-fix workers run in that lane, and their result lands as FR-3." §5 FR-3: "Driver work lands the same way: … CI-fix result, with its subject from today". §8 FR-6: the contract lines about claiming a file by `git add` go, "and the fix and CI-fix twins".

`branch_guard` stays first. Before each spawn, prepare the lane as task 1 does, and delete the snapshot (`:1646`). Spawn `Job::CiFix` with `cwd` the lane and the lane's env. On a clean exit, run `commit_lane(lane, base, tool_files + protected_files, subject(board, "fix", "CI <check>"), refs/factory/<NNN>/ci-fix-<round>)`. Print today's line for each dropped path (`:1748-1753`). Then run `land(main, sha, tool_files)`. `Landed` → `set_worker_commit`, `CiFixCommitted { round, sha, files }`, today's line, then settle `done`. `None` → today's "changed nothing to commit" line. `Dirty` and `Conflict` → as task 1, with phase `ci`. The push and the next watch follow as today.

Contract `:1`: you run in a lane reset to the feature tip, nothing else live. `:15`: do not commit, push or run `gh`. The driver commits everything not gitignored in your lane as `fix({NNN}): CI {check}`, pushes, and watches again. Leave the lane as the commit should be.

**The trap**: today settles `done` (`:1717`) before `commit_ci_fix` (`:1718`). Land first, as in task 1, so a killed driver leaves the worker unsettled and task 3 lands it.

**Acceptance:** `tests/closeout.rs:981` passes with the CI fix call's `cwd` a lane path, `fix(001): CI test` still holding `src/lib.txt` only (`:1029`), and the retro at HEAD equal to the ship commit's (`:1031-1036`). `:1199` (three rounds block) and `:1663` (the cap answer) pass unchanged.

### 3. Closeout orphans land — high — blocked by: 1, 2
**Where**: `src/closeout.rs` (`settle_orphan`).

**Source**: §5 FR-3: "The lane commit stays reachable until it lands, through a ref under `refs/factory/`". §7 FR-5: polish and CI-fix results land as FR-3.

A returned orphan lands through slice 07's idempotent order, on `w.lane` and the base recorded for it:
- the row has a commit → settle only;
- the ref resolves → land it;
- else `commit_lane`, then land.

A polish orphan then emits `PolishDone` and calls `file_friction`, as today. A CI fix orphan emits `CiFixCommitted`. Delete both `load_baseline` calls (`:1852`, `:1885`). `w.lane` is `None` only for a worker spawned before slice 09. Settle that one dead, and the closeout runs again.

**The trap**: "settle only" is not enough for polish. The closeout knows polish is over only from a `PolishDone` event (`:104-108`). A driver killed after `set_worker_commit` and before `PolishDone` leaves a row with a commit and no event. The resume would then run a second polish over a landed one. A polish row with a commit and no `PolishDone` for its sha emits one from that sha first.

**Acceptance:** `tests/closeout.rs:2227` passes, and HEAD's history holds exactly one `fix(001): polish review fixes`, with `src/lib.txt` in it. A new test kills the driver while the CI fix step runs (`delay_ms`), then runs `factory run 001` again. The log says "of an earlier driver returned", and `fix(001): CI test` exists once and holds `src/lib.txt`.

### 4. The guessing is deleted — high — blocked by: 1, 2, 3
**Where**: `src/scheduler.rs`, `src/closeout.rs`, `src/commit.rs`, `src/events.rs`, `tests/closeout.rs:1794-1834`.

**Source**: §8 FR-6: "all of it goes, with its tests (the pathspec and leftover tests in `tests/commits.rs`, and the unnamed-file test at `tests/closeout.rs:1801`), replaced by FR-3." Acceptance: "`grep -rnE "from_stream|fn unnamed|fn leftovers|untracked-before" src` finds nothing, and the gate passes."

Delete in this order, so each step compiles:
1. `note_leftovers`, `POLISH_KEY` and `ci_fix_key` in `src/closeout.rs`.
2. The baselines (`baseline_path`, `checkpoint_key`, `snapshot`, `load_baseline`), `Unnamed` and `unnamed` in `src/scheduler.rs`.
3. In `src/commit.rs`: `Pathspec` and every helper only it reaches (the list under Current code), and the unit tests `:586`, `:599`, `:630`, `:726` and `:780`.
4. The three leftover variants in `src/events.rs`, and their arms wherever the compiler names them.
5. `tests/closeout.rs:1801` and its `track_extra` helper (`:1794`).

Rewrite the `src/commit.rs` module doc: workers' work lands through `land` (FR-3), and `commit` by pathspec is left for the driver's own commits (the board files, the ship).

**The trap**: `Pathspec::from`, `heredoc_paths` and `added_paths` are `pub` in a lib crate, so clippy flags none of them once their callers are gone. Delete by the list, not by the warnings. `push_once` looks like a `Pathspec` helper, but `expand` uses it (`:499`): keep it.

**Acceptance:** `grep -rnE "from_stream|fn unnamed|fn leftovers|untracked-before" src` exits 1. `grep -rnE "Leftovers|_leftovers|Pathspec" src tests` exits 1. A unit test in `src/events.rs`: an `events.jsonl` line of kind `closeout_leftovers` reads as `EventKind::Unknown`, and nothing is skipped.

### 5. This repo's lane files and the README — low — blocked by: —
**Where**: `docs/reference/lanes/lane-1.md` to `lane-4.md` (`:12-18`), `README.md:60`.

**Source**: §11 FR-9: "The `factory docs lanes` template and this repo's lane files drop the shared-tree prose." §1: "The README names isolated lanes as the next driver PRD."

Delete the "What this lane does not isolate" paragraph from each file. Keep the frontmatter and `:7-10`. In `README.md:60`, replace "Lanes share a working tree today; isolated lanes are the next driver PRD." with one sentence: each lane is its own git worktree, and a worker's work lands as one commit (PRD-005). Slice 06 already changed the template (`src/docs/lanes.rs`). This slice is the last, because the run building this PRD reads these files until then.

**Acceptance:** `grep -rnE "same working tree|by pathspec|next driver PRD" docs/reference/lanes README.md` exits 1. `git diff` of each lane file touches no line inside the frontmatter. `factory docs check` reports nothing new.

### 6. Verify — medium — blocked by: 1, 2, 3, 4, 5
`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 7. Prove — at the checkpoint — blocked by: 6
Rows from [`prd.md`](prd.md) §14 this slice proves, below the UI:

- **FR-6**: "run the grep in §8 and the gate". The task 4 greps and the full gate.
- **FR-5**, its polish acceptance: "The polish stub's recorded cwd is a lane, and its edits land as one `fix({NNN}): polish review fixes` commit". These are the task 1 tests.
- **FR-2**, its polish part: "run a polish stub with `subagents:` and read `factory report`". This is `recount_rewrites_the_tokens_and_tool_calls_from_the_streams`, with the polish call's `cwd` a lane.

**Acceptance:** after the checkpoint, Progress holds:
- the grep output (empty, exit 1) and the gate's numbers;
- the polish call's recorded `cwd`, and `git show --stat` of `fix(001): polish review fixes` holding `src/polished.txt`;
- the `factory report --per-worker` polish row with `subagents` 2.
