---
title: "`renumber` and the board scan follow symlinks"
state: "open"
added: "2026-10-13"
---

A symlinked feature folder or slice file under `docs/features/` is read and renamed through the link. Refuse or skip symlinks, as `check` could flag them. PRD-002 polish review (security pass), 2026-10-13.
