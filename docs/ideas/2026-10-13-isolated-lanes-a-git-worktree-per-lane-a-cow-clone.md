---
title: "Isolated lanes: a git worktree per lane, a CoW clone of ignored state, the driver lands each slice"
state: "open"
added: "2026-10-13"
---

Root fix for unnamed files (PRD-001 run B; PRD-002 AD-305, AD-9901, 54 leftovers on slice 07): lanes share one tree (PRD-001 `prd.md:74`), so `Pathspec` misses every Bash edit (`git mv`, inline Python, `printf >`, rustfmt) and the checkpoint gates a dirty tree that HEAD does not match. Shape, brainstormed 2026-10-13: long-lived lane worktrees reset to the feature tip at each spawn; `cp --reflink` for `target/` (17G here, btrfs), `vendor/`, `node_modules/`, `.env`; commit is `git add -A` in the lane; the driver rebases onto the tip, a conflict spawns a merge worker, its failure is a BLOCK; the inner loop and every checkpoint run in a clean gate worktree of HEAD. Deletes `Pathspec`, `unnamed`, `leftovers`, baselines. Plus a `/lanes` skill that interviews a project into `lane-N.md` setups (per-lane DB, ports, Redis DB, queue prefix) and proves two lanes' suites run at once without colliding. Open: who writes slice status (lean: the worker reports, the driver writes the main board), the non-CoW fallback, disk growth.
