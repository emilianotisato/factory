---
title: "prune runs after checkpoints have cited test names, so its own rule blocks every consolidation"
state: "open"
added: "2026-10-29"
origin: "factory upstream"
---

`prune` (via `polish` §7) runs at the end of a feature. By then the checkpoints have recorded test names in their evidence and in the PRD. `prune`'s Preconditions §5 says to prefer keeping names that evidence cites. So consolidating any test the branch added renames something cited, and the skill's own rule argues against it. In one run, two same-setup pairs looked mergeable and stayed separate for exactly this reason.

Two ways out, either one works:
- Run `prune` per checkpoint, scoped to the tests written since the previous one, before their names are cited.
- Say that a citation only in a frozen checkpoint transcript does not bind, so `prune` may rename freely and leave the transcript as history.

Related: scope the end-of-feature `prune` to tests written since the last checkpoint, since earlier ones were already pruned.
