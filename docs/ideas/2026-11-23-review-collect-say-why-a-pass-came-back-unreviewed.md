---
title: "review collect: say why a pass came back unreviewed"
state: "open"
added: "2026-11-23"
origin: "harvest 2026-11-23"
---

**What happened.** Three features in a row had every second-opinion pass return `unreviewed` in 3 to 4 seconds, because Cursor's monthly usage limit was out. The collect reports the count, never the cause.

- **PRD-015**: 3 passes, 4 s each. The polisher had to read the PRD's note to learn the quota was out until 2026-11-24.
- **PRD-017**: 6 passes, 4 s each. `--list-models` succeeded while every pass failed on "You've hit your usage limit", so the probe said the vendor was up.
- **PRD-018**: 3 passes, 3 s each. The retro asks for a collect that says "auth, quota, model" rather than leaving it to be guessed.

The cost is real: the polisher cannot tell a transient failure from a month-long one, and the fallback it then spawns is the most expensive step in polish.

**What it should do.** The probe should recognise a usage limit, and the plan line should name the cause: `fallback owed: usage limit`, `fallback owed: auth`, `fallback owed: no such model`. A known-not-transient cause means the polisher stops retrying and goes straight to the fallback.

**Minimal shape.** `factory review collect` with the vendor's quota out → `6 passes unreviewed — fallback owed: usage limit (vendor quota)`, not `6 passes unreviewed`.
