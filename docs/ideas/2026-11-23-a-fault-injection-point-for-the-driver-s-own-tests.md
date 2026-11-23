---
title: "A fault-injection point for the driver's own tests"
state: "open"
added: "2026-11-23"
origin: "harvest 2026-11-23"
---

**What happened.** PRD-014 fixed a real defect: a `## Numbers` refresh written but not committed, when the driver is killed between the write and its commit, is never committed on resume (`retro::write` returns `Ok(false)`). The fix shipped **with no test that would go red without it**. The kill-and-resume tests can only kill while a worker runs, and the closeout fixture cannot stop between two driver steps. The slice extended a resume test to pin the road instead, and recorded the gap.

This is not a one-off. `src/closeout.rs` already guards against "killed between two driver steps" in three places, and every one of them is in the same position.

**What it should do.** A test-only fault-injection point: an env var naming a step at which the driver exits, for example `FACTORY_DIE_AT=numbers-written`. The driver checks it at named points in the closeout and the scheduler. Tests set it, resume, and assert the recovery.

**Minimal shape.** `FACTORY_DIE_AT=after-numbers-write` → the driver writes the refresh, exits before committing it; the test resumes and asserts the commit lands. Red without PRD-014's fix.
