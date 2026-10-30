---
title: "Scope the skills-update commit line to the skill dirs"
state: "open"
added: "2026-10-30"
origin: "PRD-012 polish"
---

From PRD-012 polish (second opinion, grok). `factory skills update` prints `commit: git add <skill dirs> && git commit -m "chore: skills X"` (`src/cli/skills.rs` near the end of `update`). `git commit` with no pathspec takes the whole index, so anything already staged rides in the chore commit. PRD-012 made `/prd`, `/slice`, `/deliver` and `/lanes` run that line without asking (`skills/_shared/stale-tooling.md`), which raises the odds. Shape: print `git commit -m "…" -- <dirs>` (paths are tracked after the add, so new files are included); tests that match the commit line (`tests/init.rs`, `tests/skills_bundle.rs`, `src/skills.rs` unit tests) move with it. Acceptance: with `src/x.rs` staged, running the printed line commits only the skill dirs.
