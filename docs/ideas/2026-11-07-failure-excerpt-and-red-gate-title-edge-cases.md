---
title: "Failure excerpt and red-gate title: edge cases"
state: "open"
added: "2026-11-07"
origin: "PRD-024 polish"
---

Edge cases in the failure excerpt (`src/excerpt.rs`) and the red-gate block title (`failure_line`, `src/checkpoint.rs`), found by the PRD-024 polish second opinion:

- **The title is the first of the last 20 hits.** A log with errors 0..49 keeps 30..49, so the title says `error: number 30`, neither the first failure nor the last line. FR-8 says "the excerpt's first line"; consider titling from the first hit in the whole log.
- **`"failed":0` is a generic hit.** `\bfailed\b` matches the key of a passing JSON summary (`{"result":"passed","failed":0}`), which can become the title.
- **`}{` inside a message** breaks the `failures()` split, so the object falls back to a generic hit cut at 300 chars before the test name.
- **`\r` rewrites:** `uncarriage` keeps the last segment, so `error TS2307 …\r[100%] Built` keeps only `[100%] Built`.
- **SGR `38:2:<cs>:r:g:b`** (six fields with a colourspace id) makes `has_red` read the blue value as a parameter: `\x1b[38:2::0:0:31m` counts as red.
- **Colour-only failures in CI logs:** `on_red` stores `ci/<round>.log` through `excerpt::clean`, which strips SGR, so a CI failure shown only in red (a CRAP row) gives an empty `ci.excerpt`. Store the raw log and strip at read time.
- **`gate_failure = ["^"]` or `[""]`** passes `docs check` and matches every line, so the project tier wins on every log. Reject patterns that match the empty string.
- **Exclusive tiers (AD-6):** one `gate_failure` or `failures[]` hit hides a later `error TS2307` from a different tool in the same log.
