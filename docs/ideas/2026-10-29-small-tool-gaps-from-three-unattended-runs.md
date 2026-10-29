---
title: "Small tool gaps from three unattended runs"
state: "open"
added: "2026-10-29"
origin: "factory upstream"
---

Each of these came up in a `/deliver` retro as something an agent did by hand or worked around.

1. **`--no-scripts` in the slice contract's Traps for Laravel projects using Boost.** Boost's `post-update-cmd` runs `boost:update`, which pulls Boost's newest guideline text and skills into `CLAUDE.md`, `AGENTS.md` and the skill folders. Inside a feature, an agent running `composer update` gets dozens of unrelated files in its diff. One slice knew to pass `--no-scripts`; the polish fixer in the same run did not, and spent 5 minutes restoring files.
2. **A verb to append a slice's `**Code-review fixes**` section.** Polish hand-edits every slice file for this; it is the one docs write polish still does without the tool.
3. **Per-slice inner-loop run counts readable from the store.** The PR body's Slices table asks for "inner {runs}/{red}", and the polish worker cannot see them.
4. **`measure.py` splitting a checkpoint's Prove minutes from its gate minutes.** Retros currently estimate the split.
5. **`docs log merge` renaming a superseded AD's heading.** After a merge, the superseded AD still reads as current unless you reach the one that supersedes it.
6. **The PreToolUse grep hook blocks literal text searches that contain markdown punctuation** (searching for `**Back off.**` is treated as a symbol lookup), so the agent had to read whole files instead.
7. **A `ctx refs` mode that searches `vendor/`.** The hook blocks `grep` for code symbols, and the one lookup that mattered in a run was inside a vendor package.
