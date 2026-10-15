---
title: "friction: factory PRD-007"
state: "open"
added: "2026-10-15"
origin: "factory PRD-007 run 007-20261015-c8fa"
---

- **`polish` §1, the second-opinion paragraph.** "A pass marked `unreviewed`, or `timed out … partial`, is never *no findings*" is right, but the binary marks those passes `— counted`, and the pack lifts only the marks. Two of three passes here were empty and still counted. The fix belongs in the binary (parked idea); the skill should say a `— counted` pass that timed out owes the fallback for its files until then.
- **`polish` §1, the leaks paragraph.** It names "`n fix hits`" while one hit prints `verdict: 1 fix hit`, and it never says to read the hits under the verdict (parked idea).
- **`polish` §9 against the factory contract.** The closeout steps (commit, ship) are all replaced by the contract. A one-line "in a factory run, skip to Report" at the top of §9 would save reading it.

A comment or name the leaks check should have caught: none found at polish. CP-2's three hits were caught by the check itself.

From /home/emiliano/Projects/factory/docs/features/007-polish-in-the-binary/retro.md
