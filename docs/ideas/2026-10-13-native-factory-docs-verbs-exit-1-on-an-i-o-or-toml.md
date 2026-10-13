---
title: "Native `factory docs` verbs exit 1 on an I/O or TOML error"
state: "open"
added: "2026-10-13"
---

`src/main.rs` maps every `Err` to exit 1, while v2's `die()` exited 2 and `check` uses 1 for "errors found"; a hook cannot tell a broken `state.toml` from a failed check. Map `Error::Io` and `Error::Toml` to 2 under `factory docs`. PRD-002 polish review (second opinion), 2026-10-13.
