---
title: "Batch `def` lookups in the pack's recon"
state: "open"
added: "2026-10-10"
origin: "PRD-001 polish"
---

`Recon::of` in `src/pack.rs` calls `def::find` once per symbol, and each call walks the repo again. One walk that collects every wanted symbol would cut pack build time on large repos. Parked from PRD-001 polish (2026-10-10): correct today, only slow, and the pack is built once per worker.
