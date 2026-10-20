---
title: "The Outline deny costs a turn and the builder reads the file anyway"
state: "open"
added: "2026-10-20"
origin: "PRD-009 after-merge"
---

## What happened

On 0.7.0 the hook's Outline deny is the builders' top denial: 9 of the 12 across the four runs (010-20261016-61d1, 026-20261017-f6ac, 012-20261017-3bb0, 016-20261020-0bba). `Held::section` (`src/cli/hook.rs`) answers "that file is in your pack under # Outline; read a range if you need more" to a whole `Read`, or to a `cat` that names any Outline file, often among several files in one call.

The pack's Outline holds a file's definitions and their lines, not its body. A builder that wants the body cannot get it from the pack. In all 9 cases the next call read the same file anyway: `Read` with `offset: 1` and a `limit`, or `sed -n 1,80p`. The deny bought nothing and cost a turn, plus a re-issued multi-file `cat` in two cases. PRD-009's retro § After merge has the counts.

## What it should do

- Deny a whole read only for the Slice file, which the pack carries in full. Let a whole read of an Outline file through.
- Or, if the Outline deny is kept for a reason, allow a `cat` of several files when the listed one is not the only file, and say in the hint that the Outline has signatures only.
- Measure it like PRD-009: denials and builder turns per run, before and after.
