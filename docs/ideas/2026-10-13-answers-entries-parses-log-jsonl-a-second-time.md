---
title: "`answers::entries` parses `log.jsonl` a second time"
state: "open"
added: "2026-10-13"
---

`src/answers.rs` re-implements the line reader `src/docs/log.rs` already has; one reader would keep the two from drifting on a format change. PRD-002 polish review, 2026-10-13.
