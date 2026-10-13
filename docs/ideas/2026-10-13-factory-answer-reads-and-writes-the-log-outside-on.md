---
title: "`factory answer` reads and writes the log outside one lock"
state: "open"
added: "2026-10-13"
---

Two `factory answer BLOCK-n` calls at once can both see the block open and both append an `answer` entry (`src/answers.rs` `resolve` has no already-resolved check under `docs.lock`), and `deliver` in `src/cli/answer.rs` reads the slice status and then calls `state::set` as two locked steps, so a driver write between them is overwritten. Fix: a `log::append_if` that re-reads under the lock, and a compare-and-set on `slices.N.status`. Owner-driven command, so the race needs two terminals; parked. PRD-002 polish review, 2026-10-13.
