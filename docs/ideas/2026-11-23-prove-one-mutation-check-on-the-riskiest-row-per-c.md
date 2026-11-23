---
title: "Prove: one mutation check on the riskiest row per checkpoint"
state: "open"
added: "2026-11-23"
origin: "harvest 2026-11-23"
---

**What happened.** A Prove row runs a named test and records that it was green. It never asks whether that test *can* go red. Three features shipped tests that could not fail:

- **PRD-017**: `a_cancel_check_kills_every_running_pass` and `last_reviewed_commit_is_the_one_that_set_reviewed` both passed at CP-1 and CP-2. Polish then found that neither could fail on the defect it named.
- **PRD-024**: the second opinion found a test that could never fail, after every Prove row went green first time.
- **PRD-018**: the fallback found five test-needle gaps the gate's own skill-text tests had passed.

In each case the checkpoint was green and the defect reached polish, which is the most expensive place to find it.

**What it should do.** At each checkpoint, pick the riskiest row (the one whose FR the slice guards with a single line) and do one mutation check by hand: remove or invert the guarded line, run the row's test, see it red, restore. The record names the line it broke and the red it saw. One row per checkpoint, not every row.

**Minimal shape.** CP-2 covers 9 rows. It runs all 9 green, then breaks the one line FR-4 rests on, sees its test red, restores, and the record reads `Prove: 9/9 green · mutation check: src/review.rs:212 removed → cancel test red`.

Related: PRD-021 (checkpoints-that-prove), which records mutation survivors by `file:line`.
