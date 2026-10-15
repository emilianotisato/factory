# Slice 03 — Evidence in the run dir

**Source**: [`prd.md`](prd.md) → §3 FR-2. **Depends on**: — (independent; it shares `tests/stubs/claude` with slice 01 and `assets/pr-body.md` with slices 05 and 08, in other hunks)
**Created**: 2026-10-15

**Progress**: `FACTORY_EVIDENCE` set for every worker beside `FACTORY_BOARD`; checkpoint contract and PR body name `cp-{n}/<file>`; the stub checkpoint writes `cp-{n}/row.txt` when the variable is set; `checkpoint_evidence_lands_in_the_run_dir_and_never_in_the_branch` · inner loop 1 run, 0 red, 3 min (one targeted test run before it) · no deviations · Prove FR-2 left for the checkpoint
Prove FR-2 (checkpoint 1): `evidence/cp-1-fr2-evidence.txt` (the run-dir evidence test green).

**Slice**: After this slice, every worker gets `FACTORY_EVIDENCE=<run dir>/evidence`, the checkpoint contract saves evidence under `$FACTORY_EVIDENCE/cp-{n}/`, and Progress lines and checkpoint entries name each file as `cp-{n}/<file>`. Nothing the checkpoint saves lands in the lane commit, and the PR body's Proof cell names the checkpoint. The skill lines that still name the feature folder (prune) change in slice 08, with the other skill edits.

## Current code / environment

- **The env**: `Worker::spawn` (`src/worker.rs:170-262`) sets `FACTORY_REPO` and `FACTORY_BOARD` for every worker (`:242-243`). It resolves the absolute run dir first (`:182-183`); use that `run_dir`, not the relative `run.dir()`. Every role spawns through it, the checkpoint included.
- **The contract**: `assets/contract-checkpoint.md:15` sends evidence to "the harness's artifacts or `{feature dir}/evidence/cp-{n}-*`", `:16` appends "the evidence paths" to each covered slice's Progress line, `:17` records `**Prove:** {rows} — evidence: {paths}`, and the return line asks for `evidence: {paths}` (`:25`).
- **Pinned contract text**: `src/pack.rs:1019-1020`, `tests/checkpoints.rs:576-582` and `harness::assert_v3_contract` (`tests/harness/mod.rs:19-37`) pin other parts of the checkpoint pack. Keep those strings.
- **The lane commit takes everything**: `src/land.rs:47` is `git add -A`. A file under `$FACTORY_EVIDENCE` is outside the lane, so it never reaches the commit.
- **The stub**: the checkpoint branch (`tests/stubs/claude:241-290`) writes no file and reports `evidence: none (stub)` in the entry (`:277`) and the return (`:283`). It records every `FACTORY_*` variable of each call in `calls/<n>.json` (`:108,118`). Its `sh`, `touch` and `stray` steps are not read in that branch.
- **The run test to copy**: `the_driver_runs_the_gate_at_each_checkpoint_and_commits_the_cp_entries` (`tests/checkpoints.rs:484`); `only_run_dir` (`:46`) finds the run dir; `fx.git(["ls-tree","-r","--name-only","feat/fixture"])` lists the branch; `fx.docs(["log","show","001"])` prints the entries.
- **The PR body**: `assets/pr-body.md:30` is the Slices table row, Proof cell `{evidence path, or "no Prove rows: why"}`; `:32` says "Evidence lives on the machine that ran the build; harness artifacts are gitignored."
- **Autonomy still holds**: `assets/autonomy.md:57` forbids recording a Prove row "without an artifact that exists on disk". The disk is the run dir's now; no edit.
- **Deliberate non-changes**: *(prd §7 "Not handled: the driver keeping a stray `evidence/` out of the lane commit.")* No ignore line and no filter in `src/land.rs:47`. *(prd §7 "No evidence in the PR, text or image, and no `gh --attach`.")* *(prd §7 "Not handled: a checkpoint looking at its own screenshots.")* `skills/prune/SKILL.md:28` is slice 08's.

## Tasks

### 1. `FACTORY_EVIDENCE` for every worker — high — blocked by: —
**Where**: `src/worker.rs:242-243`.

**Source**: §3 FR-2: "The driver sets `FACTORY_EVIDENCE=<run dir>/evidence` for every worker, beside `FACTORY_REPO` and `FACTORY_BOARD`."

A constant beside `REPO_ENV` and `BOARD_ENV`, set to `run_dir.join("evidence")`. The worker creates the folder when it writes; the driver need not.

**Acceptance:** task 4's test reads the variable from the checkpoint call's `calls/<n>.json`.

### 2. The contract and the PR body — high — blocked by: —
**Where**: `assets/contract-checkpoint.md:15-17,25`, `assets/pr-body.md:30,32`.

**Source**: §3 FR-2: "The checkpoint contract saves evidence under `$FACTORY_EVIDENCE/cp-{n}/`, and a harness that saves elsewhere copies what it wrote there. Nothing goes under `docs/`. Progress lines and the checkpoint entry name each file as `cp-{n}/<file>`, never an absolute path. … The Slices table's Proof cell (`assets/pr-body.md:30`) says `CP-{n}: {rows}`, or "no Prove rows: why", and `:32` says evidence stays in the run dir of the machine that ran the build."

Reword step 2's evidence sentence, step 3, step 4's `evidence:` and the return line to `cp-{n}/<file>`. Reword the PR body's Proof cell and its evidence line.

**Acceptance:** the pinned contract tests stay green, and task 4's test.

### 3. The stub writes evidence — high — blocked by: —
**Where**: `tests/stubs/claude:241-290`.

**Source**: §3 FR-2 acceptance: "The stub writes a file for this; today it answers "evidence: none (stub)" (`tests/stubs/claude:277,283`)."

When `FACTORY_EVIDENCE` is set, the checkpoint branch writes `$FACTORY_EVIDENCE/cp-{n}/row.txt` and names `cp-{n}/row.txt` in the entry's `**Prove:**` line and the `PROVE:` return. Unset, it answers as today.

**Acceptance:** task 4's test.

### 4. The run test — high — blocked by: 1, 2, 3
**Where**: `tests/checkpoints.rs`.

**Source**: §3 FR-2 acceptance: "A stub run whose checkpoint writes `$FACTORY_EVIDENCE/cp-1/row.txt` ends with the file at `<run dir>/evidence/cp-1/row.txt`, no `evidence/` path in `git ls-tree -r` of the feature branch, and a checkpoint entry in `docs log show` naming `cp-1/row.txt`."

A run through checkpoint 1, then: the file exists under `only_run_dir(&fx)`, the branch lists no path with an `evidence/` segment, and the checkpoint entry names `cp-1/row.txt`.

The trap: the run dir is reached through `FACTORY_DATA_DIR`, and `/tmp` may be a symlink. Canonicalize both sides before comparing the variable with the run dir.

**Acceptance:** a test named for the behavior (e.g. `checkpoint_evidence_lands_in_the_run_dir_and_never_in_the_branch`).

### 5. Verify — medium — blocked by: 1, 2, 3, 4
`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 6. Prove — at the checkpoint — blocked by: 5
Rows from [`prd.md`](prd.md) §5 this slice proves, below the UI:

- **FR-2**: "run: a stub run whose checkpoint writes `$FACTORY_EVIDENCE/cp-1/row.txt`; `ls` the run dir, `git ls-tree -r` the feature branch, `factory docs log show`" — task 4's test transcript.

**Acceptance:** after the checkpoint, Progress names the transcript.
