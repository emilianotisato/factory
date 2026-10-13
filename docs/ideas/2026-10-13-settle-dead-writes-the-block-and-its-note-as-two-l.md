---
title: "`settle_dead` writes the block and its note as two log appends"
state: "open"
added: "2026-10-13"
---

A crash between them leaves a `block` with no note. One append of both under the lock, or the note folded into the block body. PRD-002 polish review, 2026-10-13.
