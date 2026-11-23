---
title: "An inner_run event, so a retro can cost the inner loop"
state: "open"
added: "2026-11-23"
origin: "harvest 2026-11-23"
---

**What happened.** PRD-023's retro could not fill the Cost column for the inner loop: "not in the run's events; each slice reported it green before its commit". The retro's whole point is cost next to catch, and the step every slice runs most often is the one step the run cannot measure.

Other retros in the same batch give inner-loop figures ("7 runs, about 12 min over 5 slices"), but those come from the slices' own Progress lines, written by the agent. They are a worker's recollection, not a record, and PRD-018 showed the same lines double-count deliberate reds unless the contract says not to.

**What it should do.** The builder emits an `inner_run` event per inner-loop run: the slice, whether it was red, and the minutes. Then `factory live`, the retro's Numbers and the PR body all read one source, and a deliberate red can carry a flag so it is countable but excluded.

**Minimal shape.** A 5-slice feature's retro reads `Inner loop: 7 runs, 1 red, 11.4 min` from events, not from prose.
