---
title: "Persist CI rerun rows across a driver restart"
state: "open"
added: "2026-10-10"
origin: "PRD-001 polish"
---

AD-30008 keeps the stale rerun rows in memory, so a driver restarted mid-watch forgets which red checks were rerun. A `links` field on the `CiRerun` event would let a resumed watch rebuild the set. Parked from PRD-001 polish (2026-10-10): the run's in-flight status already covers most of the case.
