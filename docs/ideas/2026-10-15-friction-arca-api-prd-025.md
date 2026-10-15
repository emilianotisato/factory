---
title: "friction: arca-api PRD-025"
state: "open"
added: "2026-10-15"
origin: "arca-api PRD-025 run 025-20261015-a235"
---

- **`polish` § 1, second opinion.** Two PRDs running, `factory review second` timed out on grok-4.7-high at 900 s on a pass of about 535 lines. Ten passes would take 2.5 hours. The pass prompt needs a time and tool-call budget, and passes should run in parallel.
- **`slice`, lane independence.** Slice 02 was marked independent of 01 while both rewrote the same four test files and the stub. Four blocks followed. The slicer should mark two slices dependent when their Current code sections name the same file.
- **`deliver`, deferred harness repair.** A checkpoint may defer a harness repair "to polish", but `/polish` has no step that receives it. Either the checkpoint repairs it, or the slice that broke the drive owns it.
- **`factory docs log`, lanes.** In a lane, `log add` writes the main checkout's log and `log show` reads the lane's, so a worker cannot read back what it just logged.
- **The bash hook** refuses any command containing `.push(` as if it were `git push`, so an inline script editing JavaScript had to go through a file. Match `git push` as a command, not a substring.

A comment or name the leaks check should have caught: none found; leaks was clean, and the local review read every added comment.

From ~/Projects/Prokey/arca-api/docs/features/025-manage-caea-pairing/retro.md
