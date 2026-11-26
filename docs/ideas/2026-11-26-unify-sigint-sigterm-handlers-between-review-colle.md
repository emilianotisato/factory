---
title: "Unify SIGINT/SIGTERM handlers between review collect and active gate"
state: "open"
added: "2026-11-26"
origin: "PRD-020 polish"
---

**Chose:** park
**Over:** piggybacking on review::forward_signals in this PR
**Because:** closeout collect installs review handlers that can win over gate signal forwarding until resume
**Undo:** delete idea
