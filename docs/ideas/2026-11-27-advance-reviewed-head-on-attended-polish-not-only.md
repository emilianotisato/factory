---
title: "Advance reviewed_head on attended polish, not only at ship"
state: "open"
added: "2026-11-27"
origin: "PRD-029 polish"
---

Attended `/polish` sets `reviewed=<date>` but never bumps `reviewed_head`, so round 3 still diffs from ship-time head and re-reviews polish fixes. Options: stamp head in `factory docs set reviewed=` when collect ran this session, or document that attended polish must re-ship.
