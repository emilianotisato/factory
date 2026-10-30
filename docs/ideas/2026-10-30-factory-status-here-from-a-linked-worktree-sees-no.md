---
title: "factory status --here from a linked worktree sees no run"
state: "open"
added: "2026-10-30"
origin: "PRD-013 polish"
---

`--here` (`src/cli/status.rs`) compares a run's `repo` with `git rev-parse --show-toplevel` of the cwd, as PRD-013 FR-3 says. From a linked worktree (a lane, or the owner's own `git worktree`), toplevel is that worktree, so an interactive session there sees none of the main checkout's runs; worker sessions are unaffected (the mod is idle under FACTORY_REPO). `lanes::main_tree` (`src/lanes.rs`) already maps a worktree to its main tree; matching on it (both sides) would make "this repo" mean the repository, not the checkout. Needs a PRD-level call, since FR-3 defines the match.
