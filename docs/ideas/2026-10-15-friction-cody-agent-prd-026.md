---
title: "friction: cody-agent PRD-026"
state: "open"
added: "2026-10-15"
origin: "cody-agent PRD-026 run 026-20261015-da46"
---

- `polish` § 1, second opinion: the skill says to read the exit code of `factory review second`, but a run longer than the harness's background limit never returns one. Passes 5 and 6 were simply never reviewed, and only reading the log showed it. The step needs a per-pass budget, or a rule to launch the fallback the moment a pass times out instead of waiting for the run.
- `polish` § 1: the fallback prompt has to list everything already fixed or settled by the PRD, or the fallback re-reports it. A template for that brief would save rewriting it per pass.

A comment or name the leaks check should have caught: none found.

From cody-agent `docs/features/026-product-pulse/retro.md`
