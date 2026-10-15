---
title: "factory stop should kill a polish collect's reviewers"
state: "open"
added: "2026-10-15"
origin: "PRD-007 polish"
---

The driver runs `collect::collect` in-process before the polish worker spawns (`collected` in `src/closeout.rs`). PRD-007's polish added a stop check after it, so no worker spawns after a `factory stop`, but the collect itself keeps going: its `cursor-agent` passes are not workers, `factory stop` does not see them, and they run until `[review] timeout_secs` (900 s) ends them.

Shape: `run_passes` (`src/review.rs`) takes a cancel check (the run's `stop_requested`, reloaded) and its `run_logged` poll loop kills the pass's group when it fires, the pass reading `unreviewed (stopped)`. Cost: a callback through `collect`, `review::run_passes` and `run_logged`. Worth it once a stop during a long collect is seen in a real run.
