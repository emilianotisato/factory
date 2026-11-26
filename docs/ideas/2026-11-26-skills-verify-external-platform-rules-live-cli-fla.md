---
title: "skills: verify external platform rules, live CLI flags, and forbid third-party CLI execution in security pass"
state: "open"
added: "2026-11-26"
origin: "harvest 2026-11-26"
---

From PRD-025 and PRD-026 retros:
- `/prd` §4 adversarial pass verifies `file:line` claims in code but does not ask whether external platform behavior an FR relies on (e.g. GitHub Actions `[skip ci]` + `ready_for_review`) is actually true.
- `/slice` did not enforce verifying external CLI flags against a live binary before specifying them (OpenCode `--variant` did not exist in v2.4.1).
- `/polish` §1 security pass brief should explicitly forbid running third-party CLIs/shims not under review (running `opencode run --help` triggered a lazy `mise` install on the owner's machine).
