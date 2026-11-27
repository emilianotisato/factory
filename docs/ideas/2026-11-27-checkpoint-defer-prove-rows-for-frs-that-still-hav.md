---
title: "checkpoint: defer Prove rows for FRs that still have pending slices"
state: "open"
added: "2026-11-27"
origin: "harvest 2026-11-27"
---

In PRD-029 (`BLOCK-1`), Slice 02 cited `FR-3` (implementing the `state.toml` `reviewed_head` half) while the full FR-3 Prove test lived in pending Slice 04. Checkpoint 1 demanded non-empty `evidence/cp-1/FR-3/` even though Slice 04 had not run yet, causing two short checkpoint attempts and a block.

**Shape:** when computing `unproven_rows` for checkpoint N, defer any FR row that is also cited by a slice still `pending` or `building` in `state.toml` (leaving it for the later checkpoint that covers the final slice of that FR).
