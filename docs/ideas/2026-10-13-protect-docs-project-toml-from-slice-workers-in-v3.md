---
title: "Protect `docs/project.toml` from slice workers in v3"
state: "open"
added: "2026-10-13"
---

`src/commit.rs` `protected_files` still names `docs/index.md`, which held the gate in v2; in v3 the gate and lanes live in `project.toml`, which a worker can now commit. Blocking it outright would also stop the preflight worker's one allowed write (`inner`, autonomy.md § May), so this needs a rule: protected for slices, allowed at preflight. PRD-002 polish review, 2026-10-13.
