---
title: "Prove: a run row the gate already ran can cite the gate transcript"
state: "open"
added: "2026-11-23"
origin: "harvest 2026-11-23"
---

**What happened.** On a CLI-only PRD every Prove row is `run <test>`, and the gate at the same checkpoint already ran that test. Proving is then rerunning what just ran.

- **PRD-015**: 8 rows, all `run` rows. "Nothing new: every row was a test the slice already ran."
- **PRD-024**: 8 FR rows, all green first time. "Every row is a named test, so proving was rerunning tests the inner loop had already run."

Neither run caught anything at the Prove step. The cost is small per row but it is the whole Prove step on this shape of feature, and it teaches the agent that Prove is a formality.

**What it should do.** A `run` row whose test the checkpoint's gate already ran cites the gate transcript instead of running again. Rows that drive something the gate does not (an argv, a harness, a real binary over a fixture) still run on their own, and those are the rows worth the time.

**Minimal shape.** CP-1 on a CLI-only PRD: the gate transcript covers 6 of 8 rows, so Prove runs the 2 that drive the built binary and cites the gate for the rest.
