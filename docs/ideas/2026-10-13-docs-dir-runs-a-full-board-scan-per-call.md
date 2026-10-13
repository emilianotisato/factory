---
title: "`Docs::dir` runs a full `board::scan` per call"
state: "open"
added: "2026-10-13"
---

`src/docs/mod.rs` `dir()` scans every feature folder for each `set`, `set_slice_status` and `set_reviewed`; the driver calls them in loops. Cache the resolved folder on `Docs`, or take a `&Status` that already holds it. PRD-002 polish review, 2026-10-13.
