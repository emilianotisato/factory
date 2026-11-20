---
title: "polish: let other review work run while the UX pass drives"
state: "open"
added: "2026-11-20"
origin: "factory upstream"
---

**What happened.** `/polish` §1 says to run no test and no fix in the checkout until the UX pass returns, because the drive shares the database with the tests. The UX pass took 8 to 21 minutes per feature, and the polish worker sat idle for that whole window. Agent results arrive only between turns, so the worker cannot even tell a long drive from a stalled one. In four features the worker guessed what was safe (front-end edits the drive does not load, read-only reviews) and the skill did not license it.

**What it should do.**
- Say that the security pass, the local review and the fallback reviewers (all read-only) start in the same window as the UX pass.
- Say which edits are safe during the drive: those that touch neither the built assets the drive loads nor the database.
- Better still: give the drive its own served database, separate from the test database, so tests may run during the drive.

**Minimal shape.** Polish starts: UX drive (15 min) | local review, security pass and fallback in parallel (5 min each) → the triage starts as soon as the drive returns, not 15 minutes later.
