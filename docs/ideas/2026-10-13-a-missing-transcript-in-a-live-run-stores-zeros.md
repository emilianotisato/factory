---
title: "A missing transcript in a live run stores zeros"
state: "open"
added: "2026-10-13"
---

FR-21 treats "no transcript" like "no `subagents/` dir", which is right for a worker killed before its first turn but hides a wrong path rule. Store nulls plus `subagents_unread` only when the worker ran turns. PRD-003 polish review, 2026-10-13 (`tests/checkpoints.rs` kill-and-resume relies on today's zeros).
