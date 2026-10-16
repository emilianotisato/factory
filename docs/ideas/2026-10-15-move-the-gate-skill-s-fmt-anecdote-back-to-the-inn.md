---
title: "Move the gate skill's fmt anecdote back to the inner-loop bullet"
state: "open"
added: "2026-10-15"
origin: "PRD-009 polish"
retrigger: "the next skill bundle bump"
---

PRD-009 slice 06 rewrote `/gate` round 3 (`skills/gate/SKILL.md:80-81`). The anecdote "PRD-001 of the factory ran `cargo test` alone as its inner loop: 83 runs, 46 red, and the first checkpoint spent four red runs on fmt and clippy nobody had run…" moved from the inner-loop bullet, where it argues that format and lint belong in the inner loop, to the end of the "Quiet tests in the inner loop" bullet, where it argues nothing.

Fix: move the sentence back to the end of the inner-loop bullet (`:80`). It is a bundle edit, so it rides the next version bump and relock: `relock` refuses a second hash at 0.7.0 (`src/skills.rs`), which is why polish could not fix it on the PRD-009 branch.
