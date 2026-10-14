---
title: "Lanes: the edge cases PRD-005 left"
state: "open"
added: "2026-10-14"
origin: "PRD-005 polish"
---

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
