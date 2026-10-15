---
title: "Experiment on the builder levers PRD-009 left untested"
state: "open"
added: "2026-10-15"
origin: "PRD-009 interview 2026-10-15"
---

PRD-009 left out the builder levers no probe or run has measured yet. Each needs one run or a replay before it earns an FR. Figures are from [builder-cost-research.md](../reference/builder-cost-research.md), over 14 runs.

1. **Effort high to medium for builders.** Up to 2.6M. The risk is quality: compare the `quality:` line of `factory report` against the post-009 baseline.
2. **A multi-range read**, `factory ctx show f:a-b g:c-d`. Part of the 4.8M the research puts on batching: 90% of turns make one call, and builders made 912 Bash reads against 145 Read.
3. **A pack that pre-loads the slice's Current code ranges.** 1–4M net, uncertain, overlaps 2.
4. **A partial tool cut for checkpoint workers** (keep Bash, Read, Edit, Write, Skill and the MCP a harness needs). Needs a probe that deferred MCP tools still load under `--tools`.
5. **Optional: `dontAsk` instead of `bypassPermissions`.** It drops the bypass-mode reminder that tells workers to read with `cat` and `sed -n`. A probe on 2026-10-15 showed it denies writes into `.claude/` and `.git/`, and builders edit `.claude/skills/verify-*`. Revisit if Claude Code lets an allow rule cover those paths.

Method: one factory run per lever against the first run after PRD-009, on a PRD of the same kind. Read `pack_by_role`, `weighted_by_role` and `quality:` from `factory report`.
