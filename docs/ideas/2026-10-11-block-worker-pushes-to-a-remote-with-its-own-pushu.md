---
title: "Block worker pushes to a remote with its own `pushurl`"
state: "open"
added: "2026-10-11"
origin: "PRD-001 owner review"
---

AD-10001's push-off env rewrites push URLs with `pushInsteadOf`, which git skips for a remote that sets `remote.<name>.pushurl`. Adding a second `pushurl` through the env does not help, since git pushes to every one listed. Today the deny rules and the hook still stop a literal `git push`; only a push from a script or alias gets through. Options to assess: wrap `git` on the worker's `PATH`, or a `pre-push` hook that refuses while `FACTORY_LANE` is set. Parked from the PRD-001 owner review (2026-10-11).
