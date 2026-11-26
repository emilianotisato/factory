---
title: "review collect: emit structured one-line-per-claim severity index"
state: "open"
added: "2026-11-26"
origin: "harvest 2026-11-26"
---

Reported across three retros (PRD-020, PRD-022, PRD-027): multi-pass second-opinion reviews (4–10 passes) arrive as long prose sections with overlapping claims, making triage/dedupe the most expensive step of `/polish`.

**Shape:** ask the reviewer prompt (or a collect post-pass) to include a compact structured summary table (`file:line | severity | one-line defect`) at the top of `collect.md` (with optional severity floor) so polish dedupe becomes mechanical.
