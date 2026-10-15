---
title: "friction: factory PRD-008"
state: "open"
added: "2026-10-15"
origin: "factory PRD-008 run 008-20261015-b926"
---

- **`skills/polish/SKILL.md` § 1, "Start from the collected file".** It assumes `factory review collect`. A lane on the installed binary never has it, so every factory-built polish runs the last resort until the owner installs the new version. The pack could say which binary the lane runs.
- **`factory review second`, pass timeout.** 4 of 7 passes hit 900 s with only opening chatter, and a fallback agent then found real defects in 3 of those diffs in under 4 minutes each. A pass that times out with only opening chatter costs 15 minutes and says nothing; it should split the pass or stop sooner.
- **The PRD template, an edit split across two FRs.** PRD-008 had FR-5 change `docs/index.md` and FR-6 step 4 change it again, so the slice built one half and the QA guide claimed both. `/slice` should flag a file two FRs both write.

A comment or name the leaks check should have caught: none at polish. The 3 checkpoint hits were caught there.

From ~/Projects/factory/docs/features/008-team-and-harvest/retro.md
