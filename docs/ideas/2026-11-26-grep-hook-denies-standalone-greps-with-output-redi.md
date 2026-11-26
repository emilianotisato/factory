---
title: "grep hook denies standalone greps with output redirects"
state: "open"
added: "2026-11-26"
origin: "PRD-019 polish"
---

**Recon:** `first_cmd_standalone` treats `end: None` (including `grep foo src > /tmp/out`) as standalone, so the hook denies and a batched `grep … > file && wc -l file` loses the redirect step.
**Next step:** treat stdout redirect as “not standalone” in `shell::first_cmd_standalone`, with acceptance cases in FR-1.
