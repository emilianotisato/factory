---
title: "Protect `docs/project.toml` from slice workers in v3"
state: "open"
added: "2026-10-13"
---

`src/commit.rs` `protected_files` still names `docs/index.md`, which held the gate in v2; in v3 the gate and lanes live in `project.toml`, which a worker can now commit. Blocking it outright would also stop the preflight worker's one allowed write (`inner`, autonomy.md § May), so this needs a rule: protected for slices, allowed at preflight. PRD-002 polish review, 2026-10-13.

**Recon, PRD-005 (2026-10-13).** The premise above is out of date: v3 has no preflight worker (job kinds at `src/worker.rs:192-210`), and autonomy § May (`assets/autonomy.md:43`) no longer mentions `inner`, so protecting `project.toml` for every worker needs no exception. PRD-005 makes the risk real, though: its landing commits the lane with `git add -A`, so any worker edit to `project.toml` would land. The fix drafted there was one more entry in `protected_files`, dropped at landing per role like the others. The owner cut it at PRD-005's final review for want of an incident (PRD-005 §10, §16). Retrigger: a landed commit that changes the gate, the inner loop or the lane count.
