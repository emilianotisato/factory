---
title: "slice commits drop files the worker claimed by directory"
state: "open"
added: "2026-10-13"
origin: "PRD-004 checkpoint 1"
retrigger: "the next run whose slice adds a directory or a symlink"
priority: "high"
---

PRD-004's first three slice commits left their source in the tree, and HEAD did not compile: 9eed991 carried no `skills/` file and none of the nine `.claude/skills/` links, 34d9bca no `src/lib.rs` `pub mod review;`, 393ddbd only `tests/closeout.rs` of slice 05's code. The gate stayed green because it runs on the working tree, not on the commit.

Two holes in `src/commit.rs` `Pathspec::from_stream`, as read at checkpoint 1:

- A `git add <dir>` names a directory, and a directory's new files are swept only while no other worker is live (`scheduler::unnamed`, `alone`). Two lanes ran at once, so nothing was swept.
- A symlink to a directory counts as a directory (`Path::is_dir` follows the link), so `git add .claude/skills/prd` never claims the link itself. Checkpoint 1 had to name each link through a failing `cat > link` redirect.

A worker's files changed before a checkpoint are in its baseline and never swept either, so nothing later in the run would have committed them. Worth a gate-side check too: the commit, checked out alone, must build (checkpoint 1 did it with `git checkout-index -a`).
