---
title: "Retire the spec size caps: the observatory watches builder context now"
state: "open"
added: "2026-10-13"
---

The 30 KB slice cap and the 120 KB PRD cap were proxies for a builder's context before the observatory existed. PRD-003 now records each build's pack composition, start context and peak context, and the retro's inner-loop row says when a slice was too big. The caps fire for the wrong reason too: 002's slice-07 crossed 30 KB only once its Code-review fixes were written in, a review section counting toward a spec size. Retire `slice_max_kb`, `prd_max_kb` and `W-SIZE` from `factory docs check` and `project.toml`, and let the retro numbers be the signal. Decided with the owner on 2026-10-13 during the slice-skill review; the skill no longer states a cap. Retrigger: the next small binary change, or PRD-004 if it touches check.
