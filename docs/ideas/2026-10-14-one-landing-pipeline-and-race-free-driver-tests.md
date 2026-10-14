---
title: "One landing pipeline, and race-free driver tests"
state: "open"
added: "2026-10-14"
origin: "PRD-005 polish"
---

From the PRD-005 polish review.

- **The landing pipeline is written three times**: `scheduler::land_slice`, `checkpoint::land_worker` and the closeout's polish/CI landing each do commit_lane → land → conflict/dirty handling with their own block text. One `land_lane(role, ref, message) -> Landed` would keep the three in step; the polish fixes (empty merge, rollback) had to be checked in each.
- **Test timing**: several driver tests poll with fixed sleeps in the stub plan (`delay_ms: 1500`, `3000`) to order two lanes; on a loaded machine they can race (one already did at CP-3, `4c3581c`). An explicit barrier in the stub (wait for a file) would remove the race.
- **Test gaps**: no driver test for a checkpoint worker's conflicting landing (AD-15, AD-23), for a polish/CI conflict (AD-17), or for a kill between the lane claim and the reset.
