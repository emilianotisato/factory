---
title: "Polish §2 should name the leaks hits as something to read"
state: "open"
added: "2026-10-15"
origin: "PRD-007 polish"
---

`skills/polish/SKILL.md` §1 says "Read only the plan, the leaks verdict and each pass's status line now", and §2 says "Now read the second opinion's findings: the `### Pass` sections". Neither says to read the `## Leaks` part's hits. In a `factory` run the pack's `# Collected` section now carries the hits with the verdict (`pack::collected` in `src/pack.rs`), but an attended `/polish` reads `collect.md` itself and is told to read only the verdict.

Also: §1 says "`n fix hits` is hits", while one hit prints `verdict: 1 fix hit` (`collect.rs`, `count`). A literal reader misses the singular.

Fix: one sentence at the top of §2: "A leaks verdict with hits → read the `## Leaks` part too: each hit sits under its fix line", and "`n fix hit(s)`" in §1. Found at PRD-007's polish, where the bundle lock refused a skill edit at the unreleased 0.5.0 (the lock pins one hash per version), so it waits for the next skill bump.
