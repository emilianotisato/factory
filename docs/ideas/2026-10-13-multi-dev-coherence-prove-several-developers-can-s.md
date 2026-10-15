---
title: "Multi-dev coherence: prove several developers can share a repo on this framework without colliding"
state: "open"
added: "2026-10-13"
origin: "owner, feature-docs review"
---

The framework was built for one developer and the assumption leaks everywhere: `models.md` carries one person's Cursor path and default model; feature folders commit evidence with local paths (see the team-repo idea); lane files may hold a developer's DB names and ports; `docs new` reserves numbers only across one machine's worktrees; the metrics store and run dirs are per machine; the retro compares against "the last three runs on this repo" meaning this machine's. Before the first repo with a second developer, review every one of these for one question: what is the repo's, what is the machine's, what is the person's. Lean: the repo commits specs, state, log and retro only; the machine holds runs, evidence and the store; the person holds model preferences and vendor paths in `~/.config/factory/` as user-scope overlays (PRD-004). Also decide what two developers running `factory run` on different features of the same repo at once need: number allocation across clones (`docs check` in CI catches the duplicate today), lane name collisions, and PR bodies that cite the other run. Retrigger: the first repo with a second developer, or when PRD-004's scopes land.

**2026-10-14, after PRD-007.** Three refinements:
- **The retro's comparison should read the tracked retros.** Every clone has every feature's `retro.md` and its numbers table; only this machine's runs are in the local store. Compare against the last three tracked retros, and keep the store as the source of the run's own row. That makes the tracked retros the team's shared metrics.
- **`DEC-n` collides like PRD numbers.** Two developers polishing at once both write `DEC-22`. The harvest idea (`2026-10-15-archive-harvests-what-a-feature-taught`) replaces the counter with named slugs.
- **The split, refined.** Repo: the PRD, state, log, retro and decisions, plus slices until the harvest. Machine: the run dir, evidence included (see the team-repo idea). Person: preferences. A team member's agent outside a run learns that `docs/features` is history from the `CLAUDE.md` convention line in the harvest idea.
