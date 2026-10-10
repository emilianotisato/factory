---
slice: 06
status: pending
depends_on: [5]
---

# Slice 06 — Checkpoints: driver-run gate, cache, fix workers (FR-4)

**Source**: [`prd.md`](prd.md) → §6 FR-4 (all bullets and acceptance); §3 FR-1 bullet 3 ("an unverified checkpoint runs first"); §10 FR-8 bullet 2 (`gates` rows, driver source). **Depends on**: 05 (a checkpoint commits the merge and the fix worker's files; both are slice 05's commit path).
**Created**: 2026-10-09 · **Status**: pending

**Progress**: _(written as it lands)_

**Slice**: After this slice the driver runs the full gate itself at every checkpoint the board declares: it drains the lanes, runs the declared command, caches the result against a hash of the code tree, and on red spawns a fix worker with the failing output, commits the fix and reruns. Three reds in a row block the run. Green hands the transcript to a checkpoint worker that writes the `CP-n` entry and drives the covered QA rows; then log merge, render, one checkpoint commit. A killed run resumes by running its unverified checkpoint first. This is also the slice that makes the fixture's `checkpoint_every: 10` workaround from slice 03 unnecessary; the default of 2 works from here.

## Current code / environment

- **What the board says**: `checkpointEvery` (`docs.mjs:318`, default 2), `checkpointDue` and `unverified[]` (`:1381-1416`): `checkpointDue` becomes true after `checkpoint_every` done slices with no `CP-n` entry covering them, and `spawnable` drops to 0 while it is. CP entries are read from `agent-log.md` (`:1392`), which is why the merge must happen before the next board read. A checkpoint is also due at the end (`allDone` with `unverified` non-empty).
- **Today's checkpoint contract**: `deliver/SKILL.md:137-163`: the orchestrator drains, runs the full gate, merges logs, renders, drives the Prove rows for the covered slices, writes the `CP-n` entry (shape `templates/agent-log.md`), one checkpoint commit; `:155` the return block. §6 moves the gate run and the commit into the driver and keeps a worker only for the judgment: "A checkpoint worker then reads the transcript, drives the Prove rows, writes CP-n."
- **The gate command**: `docs gate` prints it (`docs.mjs:2081`); in the fixture it is `./gate.sh`, scripted by `scenario/gate` (slice 01). Run through `sh -c <cmd>` in the repo root with the lane's env unset, output captured to `runs/<id>/gates/<n>.log`, duration measured. A `gates` row per run: `source = driver`, `kind = checkpoint`, `cmd`, `red` (0 or 1), `minutes` (§10).
- **The cache key**: §6 "a cache keyed on the code paths (hash of the tree minus `docs/**`) skips the rerun when nothing under test changed; only driver runs populate it." `git write-tree` on a temporary index (`GIT_INDEX_FILE=<tmp>`; `git read-tree HEAD`, `git add -A -- . ':!docs/**'`, `git write-tree`) hashes the working tree including uncommitted and untracked code, which is what "nothing under test changed" has to mean when a fix worker's files are not committed yet. Key = that tree id + the gate command string; store `runs/<id>/gate-cache.json {key: {red, minutes, at, log}}`. Cache hits still write a `gates` row with `minutes: 0` and a `cached: 1` column, so the count of gate runs stays honest in the report.
- **The fix-worker loop**: §6 "Red: the driver spawns a fix worker with the failing output, commits its files by pathspec, reruns the gate; three reds in a row block the run (`autonomy.md:77`-style cap)." The fix worker's pack is the slice pack layout with the slice section replaced by `# Gate failure` (the tail of the gate log, capped at 200 lines) and the contract replaced by `assets/contract-fix.md` (ported from the checkpoint contract's fix rules: fix the failure, run the inner loop, do not touch unrelated files, return STATUS). Its commit message is `fix: gate red at checkpoint n`.
- **Resume**: §3 "an unverified checkpoint runs first". `unverified[]` on the board after a resume → go straight to the checkpoint path before any spawn. Slice 03's resume test left a `checkpoint_due` event stub; replace it with the real path here.
- **The stub needs a `checkpoint` scenario**: for the checkpoint worker, apply nothing to the board, write a `CP-n` entry to `agent-log.md`'s `## Checkpoints` with the covered slices and the QA rows, emit `STATUS: done`. For the fix worker, step `fix{touch:["src/lib.txt"]}` writes the file and emits `STATUS: done`. Both are plan steps like any other, so the test orders them.
- **Deliberate non-changes**: *(prd §6 "Rejected: skipping the gate at checkpoints because the inner loop was green. The inner loop is a subset by design.")* *(prd §19 "Not handled: a stage-aware gate (fmt first, then clippy, then test)"; the gate is one string run as one command.)* *(prd §15: the owner asked for a cache keyed on code paths rather than a plain tree hash; the `docs/**` exclusion is that decision, nothing finer.)*

## Tasks

### 1. Checkpoint phase: drain, gate, `gates` row — high — blocked by: —
**Source**: §6 FR-4: "The driver, not a worker, runs the gate at every checkpoint: it drains the lanes (no new spawns, in-flight workers finish), runs the declared command, captures stdout and stderr with duration, and writes a `gates` row (source driver)." Acceptance: "every gate run has a `gates` row".

`src/checkpoint.rs`: when `checkpointDue` (or `allDone && !unverified.is_empty()`) → phase `checkpoint`, event `checkpoint_started {n, covers: [slices]}`; stop spawning, wait for running workers to exit (reaping as in slice 04); run the gate through `sh -c` with output to `runs/<id>/gates/<n>-<attempt>.log`, insert a `gates` row, emit `gate_run {n, attempt, red, minutes}`.

**Acceptance:** fixture default `checkpoint_every` (2), plan of three `done` and one `checkpoint` step: after slices 01 and 02 the driver runs the gate once before slice 03 spawns (the stub call order shows checkpoint before the third build), `gates` has one row with `source='driver'`, `kind='checkpoint'`, `red=0`, `minutes>0`; `gate.log` from slice 01 shows the invocation.

### 2. Gate cache on the code tree — high — blocked by: 1
**Source**: §6 FR-4: "a cache keyed on the code paths (hash of the tree minus `docs/**`) skips the rerun when nothing under test changed; only driver runs populate it."

`src/gatecache.rs`: `tree_key(repo) -> String` via the temporary-index `write-tree` described above plus the gate string; `GateCache::get/put` on `runs/<id>/gate-cache.json`. Before running the gate: a hit → `gate_cached` event, `gates` row with `cached=1`, `minutes=0`, red from the cache, no process spawned. Only this code path writes the cache; the worker's inner-loop runs (slice 08 counts them) never do.

**Acceptance:** two checkpoints with no code change between them (a fixture plan where slices 03 and 04 touch only `docs/`) → one `./gate.sh` invocation in `gate.log`, two `gates` rows, the second `cached=1`; a change to `src/lib.txt` between them → two invocations; an uncommitted change counts as a change.

### 3. Red: fix worker, pathspec commit, rerun, cap at three — high — blocked by: 1
**Source**: §6 FR-4: "Red: the driver spawns a fix worker with the failing output, commits its files by pathspec, reruns the gate; three reds in a row block the run." Acceptance: "a `red, red, green` scenario produces two fix workers, three gate runs and a green CP".

On `red`: `fix_spawned {n, attempt}`, worker role `fix` with the `# Gate failure` pack (last 200 lines of the log, the gate command, the inner command), model/effort from `config.workers.fix`; on exit, commit its pathspec with `fix: gate red at checkpoint n` (slice 05's `commit.rs`, with the slice-file and fragment additions skipped since there is no slice), rerun the gate (cache miss by construction when the fix touched code; a fix that touched nothing is a hit, which is red again, which is correct). Three consecutive reds → `BLOCK-<NNN>-cp<n>` in `agent-log.md` with phase `checkpoint`, the last gate log tail as the question, phase `waiting-owner`, event `checkpoint_blocked`. A dead fix worker counts as a red attempt.

**Acceptance:** `gate` scripted `red, red, green` with plan `done, done, fix, fix, checkpoint, done`: two `fix` workers rows, three `gates` rows (`red=1,1,0`), two `fix:` commits in `git log`, then the checkpoint worker runs; scripted `red, red, red` → `checkpoint_blocked`, the BLOCK entry exists, phase `waiting-owner`, and `factory answer` on it (slice 04) reruns the gate.

### 4. Green: checkpoint worker, merge, render, one commit — high — blocked by: 1, 3
**Source**: §6 FR-4: "A checkpoint worker then reads the transcript, drives the Prove rows, writes CP-n. The driver commits the checkpoint." `deliver/SKILL.md:137-163` (what the CP entry holds); §7 FR-5 (merge at each checkpoint).

Checkpoint worker pack: `# Gate transcript` (the green log), `# Covered slices` (the slice files' Prove tasks and Progress sections), `# QA rows` (the PRD § QA plan rows those Prove tasks name), `assets/contract-checkpoint.md` (ported from `:137-163`: drive each row, paste evidence into the slice's Progress, write `CP-n` under `## Checkpoints` in `agent-log.md`, do not run the gate, return STATUS). After it exits: `docs log merge NNN`, `docs render --write`, `git add -- docs/features/NNN-*/ docs/index.md` and commit `docs: checkpoint n`. Move slice 05's end-of-run merge into this path; the end of the run is just the last checkpoint. Event `checkpoint_done {n}`. If the checkpoint worker dies or returns `blocked`, treat it as slice 04 treats a slice worker: once more, then block.

Trap: the checkpoint worker writes `agent-log.md`, the file the slice contract forbids slice workers. The checkpoint contract permits it, and no slice worker runs during a checkpoint (the lanes are drained), so there is no race; keep the drain, never relax it to "spawn while the checkpoint worker runs".

**Acceptance:** after the task 1 run, `agent-log.md` holds `CP-1` naming slices 01 and 02 and `CP-2` naming 03, the fragments are merged, the PRD slice table shows all `done`, `git log --format=%s` holds `docs: checkpoint 1` and `docs: checkpoint 2`, and `status --json` has `unverified == []`.

### 5. Resume runs the unverified checkpoint first — high — blocked by: 4
**Source**: §3 FR-1: "done slices skip, an unverified checkpoint runs first, open blocks wait."

On resume: `unverified` non-empty → the checkpoint path before any spawn, with `checkpoint_resumed` event. The gate cache survives in the run dir, so a resume after a green gate and a killed checkpoint worker does not rerun the gate.

**Acceptance:** plan `done, done, checkpoint{delay_ms:3000}`: kill the driver's process group during the checkpoint worker; `factory run 001` → the first events after `run_resumed` are `checkpoint_resumed` and `gate_cached`, then a new checkpoint worker, then `CP-1`; `gate.log` shows one gate invocation in total.

### 6. Verify — medium — blocked by: 1–5
`cargo test`. The full gate runs at this slice's checkpoint.

**Acceptance:** inner loop green, one commit.

### 7. Prove — at the checkpoint — blocked by: 6
Rows from [`prd.md`](prd.md) § QA plan: **FR-4** ("`red, red, green` → two fix workers, three gate runs, green CP; every gate run has a `gates` row", evidence: test transcript plus `SELECT * FROM gates`). At the checkpoint: run the task 3 scenario on the fixture and paste the `gates` rows and the `fix:` commits into this slice's **Progress**.

**Acceptance:** three `gates` rows with `red` 1, 1, 0 and two `fix` worker rows are attached.
