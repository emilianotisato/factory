---
title: "Parallel polish fixers, each in its own lane"
state: "open"
added: "2026-10-14"
origin: "PRD-005 interview"
retrigger: "PRD-005 shipped and a polish Fix list ran past an hour with disjoint file sets"
---

PRD-005 runs the polish worker in one lane and its fixers one after another there (decided 2026-10-13, owner: "option one for now, parallel fixers can be good"). Parallel fixers would cut polish wall-clock when the Fix list splits by file set (server and UI). Two shapes to try:

- **Driver-spawned fixers.** Polish triages and writes a fix plan; the driver spawns one fixer per disjoint set, each in its own lane, lands each like a slice, then gates HEAD in a free lane. Overlaps the decompose-polish idea (2026-10-09).
- **`factory lane` verbs for a worker.** The polish worker asks the CLI for a lane (`factory lane take`), hands it to a subagent, and the driver lands it on return. Keeps polish one worker; costs a lane protocol between worker and driver.

Either needs the merge gate after the fixers that PRD-001's improvised recipe found useful (a red no fixer's own green gate could see). Measure first: polish wall-clock and fixer count per run from `factory report`.
