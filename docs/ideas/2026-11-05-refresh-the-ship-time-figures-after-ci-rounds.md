---
title: "Refresh the ship-time figures after CI rounds"
state: "open"
added: "2026-11-05"
origin: "PRD-014 polish"
---

PRD-014 made every push after ship rewrite the retro's `## Numbers` from the store, so CI fix workers are counted. Two other ship-time figures still stop at ship:

- **The store's `retros` row.** `retro_row` (`src/closeout.rs`, called once in the ship step) upserts `ci_cycles`, `out_tokens`, `cache_read_tokens`, `peak_context` and the minutes from `report::compute` at ship. CI rounds happen after ship, so `ci_cycles` is stored before any round and the token figures miss the CI fix workers. Anything that reads the `retros` table across features (trends, `factory docs retro`) under-reports exactly as the Numbers section did before PRD-014.
- **Lines changed and leaks figures.** `set_run_lines` and `set_run_leaks` are computed once at ship (`retro::changed_lines`, `leaks::scan` against the base). A CI fix adds lines, so the `(this run)` row of `Compared with` (lines changed, per line, comment share, id hits) describes the tree before the fix while the role table above it counts the fix worker.

Likely shape: one helper both ship and `refresh_numbers` call, recomputing lines, leaks and the retro row before `retro::numbers`. Keep the refresh's rule that a compute failure is said, never fails the push. Found by the local code review of PRD-014.
