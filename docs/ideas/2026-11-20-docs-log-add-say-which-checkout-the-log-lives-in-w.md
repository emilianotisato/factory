---
title: "docs log add: say which checkout the log lives in when called from a lane"
state: "open"
added: "2026-11-20"
origin: "factory upstream"
---

**What happened.** Run from a lane worktree, `factory docs log add` writes the main checkout's `docs/features/NNN-*/log.jsonl` and prints that path, while `factory docs idea add` writes the idea into the lane. A worker checking `git status` in its lane sees no log change and cannot tell whether the write went somewhere wrong. Seven features' retros raised the same surprise.

**What it should do.** If one log per feature in the main checkout is by design, say so: in the command's output ("log is the driver's, in the main checkout") and in the polish and builder contracts. If it is not by design, write the lane's copy like `idea add` does.

**Minimal shape.** In lane-2: `factory docs log add …` → prints `…/main-checkout/docs/features/042-x/log.jsonl updated`; `git status` in lane-2 is clean.
