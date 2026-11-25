---
title: "Reviewer status wording and collect reuse after a provider switch"
state: "open"
added: "2026-11-25"
origin: "PRD-026 polish"
---

Loose ends from the multi-provider review:
- A `proceed` answer is collected as `second opinion: skipped (disabled in config)` although config.toml is unset (src/collect.rs). The behavior is right (no fallback is owed), but the wording is wrong. `factory review second` also ignores the run's `review_override`.
- At the same HEAD, closeout reuses a collect (src/closeout.rs `collected`) even after the owner switched `[review]` provider mid-run, so the pack triages the previous vendor's findings, or a stale "disabled".
- `factory docs preflight` drops `loaded.warnings`, so a typo'd `[review] proivder` shows only "reviewer not set".
- `detect_installed` lists providers in registry order, not PATH order, and the wizard's suggested default is `.first()`. AD-10 says "first CLI found on PATH".
- `is_executable` accepts any x bit, even one the caller cannot use. A probe hung in uninterruptible sleep makes `child.wait()` unbounded, and Ctrl-C does not reach the probe's own process group.
