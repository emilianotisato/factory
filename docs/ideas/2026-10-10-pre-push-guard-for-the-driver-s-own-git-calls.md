---
title: "Pre-push guard for the driver's own git calls"
state: "open"
added: "2026-10-10"
origin: "PRD-001 polish"
---

The global `~/.config/git/hooks/pre-push` guard is active only under Claude Code (`CLAUDECODE=1`). The driver never sets it for its own git calls, so the guard is off for the gate, lane setup and hooks that the driver runs. Workers already cannot push (hook deny, AD-10001), and the driver pushes only the feature branch (AD-30009). This would add a third layer. Parked from PRD-001 polish (2026-10-10).
