---
title: "Checkpoint: the Prove line must list every row it was handed"
state: "open"
added: "2026-11-23"
origin: "harvest 2026-11-23"
---

**What happened.** PRD-018's CP-1 covered 9 rows and drove 7. CP-2 covered 4 and drove 3. Both records read "all run green". FR-2, FR-4 and FR-6 were never driven and nobody noticed until the retro.

`assets/contract-checkpoint.md` step 2 says "a row you could not reach is reported, not skipped silently". That stops a row from being *refused*. It does not stop one from being *forgotten*, because nothing compares the rows driven with the rows handed.

**What it should do.** The record's `**Prove:**` line enumerates the rows it was handed, each with its result, so a dropped row is visible as a gap. Better still, the driver compares that list with the QA rows in the pack and blocks the checkpoint when they differ.

**Minimal shape.** CP-1 handed 9 rows, drove 7 → the record reads `Prove: FR-1 green · FR-2 not driven · FR-3 green …`, and the driver says `checkpoint CP-1: 2 of 9 Prove rows not driven`.

Related: PRD-021 (checkpoints-that-prove).
