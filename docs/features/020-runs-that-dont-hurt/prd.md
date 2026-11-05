---
number: 020
slug: runs-that-dont-hurt
title: "Runs that don't hurt the machine or lose work"
# type: feature | bugfix | enhancement | refactor
type: feature
created: 2026-11-05
author: Emiliano Tisato
summary: |
  A run cannot take the machine down or eat a worker's edits: gates run under a memory cap and are reaped on resume, lanes trim their build output, a respawn keeps its lane, seeds copy atomically, lanes refuse a rebuild under an open run, runs and worktrees resolve one main tree, and landing is written once with tests that wait on barriers. Draft from the 2026-11-05 idea triage, not interviewed.
---

# PRD-020 — Runs that don't hurt the machine or lose work

> **Draft from the 2026-11-05 idea triage.** The owner chose what is in and out, group by group; nothing here is interviewed or verified yet. `/prd` continues this draft: interview, recon, FRs, QA plan. `file:line` facts are from `main` at `ca128f7`; re-verify them.

## 1. Why now

Owner, 2026-11-01: a runaway gate killed the terminal and the session driving the run; four lanes' build output filled the disk, fixed for this repo only by `7a518f9`. Sleep-ordered driver tests have raced on main's CI twice.

## 2. Candidate FRs

Kept by the owner at the triage. `/prd` turns each into an FR or cuts it.

- **C-1 — Gates run under a memory cap** (a bare `sh -c` today, `src/checkpoint.rs:511`, `src/lanes.rs:120`). systemd-run `MemoryMax` or ulimit; macOS is an open question.
- **C-2 — The gate's pid is recorded and reaped on resume.**
- **C-3 — Lanes trim their build output:** a `lanes clean`, a size cap or a sweep (`src/cli/lanes.rs:13-26`).
- **C-4 — A polish or CI-fix respawn keeps its lane** (`prepare_lane(.., true, ..)`, `src/closeout.rs:195,1875`).
- **C-5 — A seed copies to a temp name, then renames** (`src/lanes.rs:275-283`).
- **C-6 — `lanes up` and `--prove` refuse during an open run** (`src/cli/lanes.rs:156`).
- **C-7 — One main tree:** a run's repo, `lanes prune` and `status --here` all compare `lanes::main_tree` (`src/cli/run.rs:39`, `src/run.rs:541`, `src/cli/status.rs:76-77,119-120`).
- **C-8 — One landing pipeline**, `land_lane` (`src/scheduler.rs:962`, `src/checkpoint.rs:711`, `src/closeout.rs:504`), with conflict and kill driver tests; the ~30 remaining `delay_ms` waits become `wait_sh` barriers.

## 3. Not in scope (decided 2026-11-05)

- **The other lane edge cases** (unlisted lane re-add, `undo_half_pick` renames, refs left after a hand landing, the landing-wait phase label, pre-lanes workers, non-UTF-8 paths, the low-confidence security notes) — dropped: narrow, nothing hit them in a month.
- **Run the CI checks locally with `act`** — dropped: the gate runs the same commands; C-8 fixes the race that caused the red round.
- **Persist CI rerun rows across a driver restart** — dropped: narrow.

## 4. Source ideas

The full text of each idea this draft absorbed; the idea files are deleted.

### Run gates under a memory cap

_added 2026-11-01, origin: owner_

A runaway gate kills the owner's terminal, and the session driving the run dies with it.

On 2026-11-01 sii-api's `/deliver 006` died twice (01:34 and 07:54), each time about
4 minutes into checkpoint 1's `make verify`. Its mutation tier ran a gremlins mutant of
`cut()` in `internal/printable/receipt/receipt.go` that made `wrap()` append empty lines
forever. Each test binary grew about 0.7 GB/s, 4 workers ran at once, and the terminal's
scope peaked at 80 GB RAM plus 67 GB swap. systemd-oomd then killed the whole terminal
scope: 183 and 177 processes, the Claude session, the factory driver and every worker.
Reproduced under `systemd-run --user --scope -p MemoryMax=12G -p MemorySwapMax=0`: the cap
killed the runaway tests and nothing else.

The driver and every gate, setup and worker it spawns live in the terminal's cgroup, so
any one of them can take down everything. The repo-side fix (a per-process memory cap in
sii-api's `scripts/mutation.sh`) covers one gate in one repo.

Options for factory itself:
- Run each gate (`src/checkpoint.rs`, the `sh -c` spawn) and each lane setup
  (`src/lanes.rs`) in its own scope with `MemoryMax` and `MemorySwapMax=0` when
  `systemd-run --user` is available. A runaway then fails the gate with a clear
  "gate hit its memory cap" message, which the run can report like any red gate.
- Or start the driver in its own scope, so oomd picks the run and spares the session.
- Default the cap to a share of RAM; let `[docs]` or `[lanes]` in `docs/project.toml`
  override it. Without systemd (macOS, containers), fall back to running uncapped and
  say so once in preflight.

### Lanes never trim their build output

_added 2026-11-01, origin: owner_

Lane worktrees keep their build output forever. On 2026-11-01 this repo's four
lanes held 148 GB of `target/` (20-55 GB each) plus 89 GB of stale `target/lane-*`
folders in the main checkout from before lanes moved to worktrees. Nothing in
factory trims either.

This repo now caps it per repo: each `docs/reference/lanes/lane-N.md` `setup:` wipes
`$CARGO_TARGET_DIR` at run start past 15 GB, and `[profile.dev] debug =
"line-tables-only"` halves a fresh build (7.3 GB to 3.2 GB). Every other Rust repo
with lanes still grows unbounded, and other stacks have their own caches.

Options for factory itself:
- `factory lanes clean`: report and trim build output across a repo's lanes, plus
  stale lane dirs in the main checkout.
- A size cap factory checks at run start, declared under `[lanes]`, instead of a
  hand-written shell line per lane file.
- Use `cargo sweep --time N` when installed: it drops only artifacts unused for N
  days, so a trim keeps the warm dependencies a full wipe throws away.

Rejected: one shared `CARGO_TARGET_DIR` across lanes or projects. Cargo's build
lock serializes every build that shares it, which undoes parallel lanes, and
same-named binaries from different checkouts overwrite each other.

### Lanes: the edge cases PRD-005 left

_added 2026-10-14, origin: PRD-005 polish_

Found by the PRD-005 polish review (local, security, nine fresh passes); verified, not fixed because each is narrow or a design call.

- **Two runs or a linked worktree on one repo share lanes.** `run::list_open` (`src/run.rs:615`) and prune key by the run's `repo`, lanes by `lanes::main_tree`; a run started from a linked worktree is not seen by `factory lanes prune` (`src/cli/lanes.rs:175`). Compare by main tree.
- **`factory lanes up` / `--prove` do not refuse during an open run**, and `--prove` runs on a lane it did not reset (`src/cli/lanes.rs`).
- **A seed copy killed midway is kept** next time as `kept` (`src/lanes.rs` `seed_one`): copy to a temp name, then rename.
- **`lanes::ensure` on a `lane-N` dir git no longer lists fails** instead of re-adding it.
- **`undo_half_pick` matches an exact pick only** (`src/scheduler.rs`); since `land` now rolls back a failed commit, only a driver kill mid-pick reaches it, and a pick onto a renamed path does not match. Compare against the merge-tree tree.
- **A polish or CI-fix respawn resets its lane** (`src/closeout.rs` around the polish spawn and the CI round), losing a dead worker's edits; build workers keep theirs.
- **Refs leak**: `checkpoint-n-<role>`, `polish`, `ci-fix-n` refs a conflict left stay after the owner lands them by hand.
- **The landing-wait clear sets phase `building`** (`src/scheduler.rs` drive loop) even when the next step is the checkpoint or closeout; harmless today since each sets its own phase.
- **Workers of a pre-lanes driver (lane `None`) are dropped** by the lane routing.
- **Non-UTF-8 paths** go through `from_utf8_lossy` in `src/land.rs` and `nul_split`.
- **Security notes (low confidence):** a planted `refs/factory/...` ref is landed as is and skips the board/protected drop; a lane is not a sandbox (the repo's hooks and `core.hooksPath` run there); a seed path through a symlinked component is followed.

### One landing pipeline, and race-free driver tests

_added 2026-10-14, origin: PRD-005 polish_

From the PRD-005 polish review.

- **The landing pipeline is written three times**: `scheduler::land_slice`, `checkpoint::land_worker` and the closeout's polish/CI landing each do commit_lane → land → conflict/dirty handling with their own block text. One `land_lane(role, ref, message) -> Landed` would keep the three in step; the polish fixes (empty merge, rollback) had to be checked in each.
- **Test timing**: several driver tests poll with fixed sleeps in the stub plan (`delay_ms: 1500`, `3000`) to order two lanes; on a loaded machine they can race (one already did at CP-3, `4c3581c`). An explicit barrier in the stub (wait for a file) would remove the race.
  - **2026-10-14:** the barrier exists (`wait_sh` in the stub plan). `two_lanes_land_out_of_order_…` in `tests/commits.rs` raced on main's CI after the PRD-006 merge (a worker's board write landed in another slice's commit) and now orders its lanes with `wait_sh`. About 30 `delay_ms` uses remain across the driver tests; each one that orders lanes should move to `wait_sh`.
- **Test gaps**: no driver test for a checkpoint worker's conflicting landing (AD-15, AD-23), for a polish/CI conflict (AD-17), or for a kill between the lane claim and the reset.

### factory status --here from a linked worktree sees no run

_added 2026-10-30, origin: PRD-013 polish_

`--here` (`src/cli/status.rs`) compares a run's `repo` with `git rev-parse --show-toplevel` of the cwd, as PRD-013 FR-3 says. From a linked worktree (a lane, or the owner's own `git worktree`), toplevel is that worktree, so an interactive session there sees none of the main checkout's runs; worker sessions are unaffected (the mod is idle under FACTORY_REPO). `lanes::main_tree` (`src/lanes.rs`) already maps a worktree to its main tree; matching on it (both sides) would make "this repo" mean the repository, not the checkout. Needs a PRD-level call, since FR-3 defines the match.

### Also from

checkpoint-covers-enforcement, its "gate's pid is not recorded" bullet (C-2): full text in *Checkpoints that actually prove*.
