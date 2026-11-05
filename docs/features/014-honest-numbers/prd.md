---
number: 014
slug: honest-numbers
title: "Honest numbers"
# type: feature | bugfix | enhancement | refactor
type: feature
created: 2026-11-05
author: Emiliano Tisato
summary: |
  The observatory's figures say when they are partial: a worker whose transcript was not found stores nulls, not zeros; the retro's Numbers table marks a sum missing a part; the section says it is as of ship. Draft from the 2026-11-05 idea triage, not interviewed.
---

# PRD-014 — Honest numbers

> **Draft from the 2026-11-05 idea triage.** The owner chose what is in and out, group by group; nothing here is interviewed or verified yet. `/prd` continues this draft: interview, recon, FRs, QA plan. `file:line` facts are from `main` at `ca128f7`; re-verify them.

## 1. Why now

Three PRD-003 review findings, each a number that looks whole and is not. Every knob experiment (one variable per PRD) reads these figures, so a silent zero or a partial sum can fake a saving.

## 2. Candidate FRs

Kept by the owner at the triage. `/prd` turns each into an FR or cuts it.

- **C-1 — A missing transcript stores nulls.** Today `TranscriptMissing` stores `Some((0,0,0,0,0))` (`src/metrics.rs:463-465`). Store nulls plus a `subagents_unread` flag, only when the worker ran turns.
- **C-2 — Partial figures are marked.** `add_opt` treats a null as 0 (`src/retro.rs:85`). Print `*` and a footnote when a role's sum misses a worker's `sub_*`, or a "Compared with" row is pre-v2 (own tokens only).
- **C-3 — Numbers say "as of ship".** The section is written at ship only (`src/closeout.rs:936`), before CI fix rounds. Label it, rather than rerun it after CI.

## 3. Not in scope (decided 2026-11-05)

- **Retire the spec size caps** — dropped: the caps cost nothing and back "slice, don't fatten".
- **`factory status --json` reads twice** — dropped: measured 13 ms.
- **Builder-lever experiments** — stay an idea: they need measured runs, one knob each.

## 4. Source ideas

The full text of each idea this draft absorbed; the idea files are deleted.

### A missing transcript in a live run stores zeros

_added 2026-10-13_

FR-21 treats "no transcript" like "no `subagents/` dir", which is right for a worker killed before its first turn but hides a wrong path rule. Store nulls plus `subagents_unread` only when the worker ran turns. PRD-003 polish review, 2026-10-13 (`tests/checkpoints.rs` kill-and-resume relies on today's zeros).

### Mark partial figures in the retro's Numbers table

_added 2026-10-13_

A role with one worker whose `sub_*` are null sums the others' and prints a figure that looks whole; pre-v2 rows in "Compared with" are own tokens only. Print a `*` and a footnote in both cases. PRD-003 polish review (second opinion), 2026-10-13.

### Refresh `## Numbers` after the CI rounds

_added 2026-10-13_

`factory retro` runs at ship, so CI fix rounds and later polish tokens are missing from the section until the owner reruns it. Either the driver reruns it after CI green, or the section says "as of ship". PRD-003 polish review (second opinion), 2026-10-13; FR-25 settles today's behaviour.
