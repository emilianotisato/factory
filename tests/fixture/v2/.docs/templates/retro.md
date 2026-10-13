---
score:
gate_runs:
gate_red:
gate_minutes:
ci_cycles: 0
slices:
checkpoints:
build_minutes:
polish_minutes:
out_tokens:
cache_read_tokens:
start_context:
peak_context:
---

# Retro — PRD-{NNN}

Written by `/deliver` in the closeout commit, from the agent's side of the run.

**Frontmatter** — plain numbers, no comments on the line (the parser reads them for a later cross-project scan):
- `score` — 1 to 5, how well the framework served this feature
- `gate_runs` — every full gate run, red ones included
- `gate_red` — how many of those failed
- `gate_minutes` — total wall-clock across all runs, rounded
- `ci_cycles` — CI fix rounds; each CI fix commit bumps it in the same commit
- `slices`, `checkpoints`, `build_minutes`, `polish_minutes`, `out_tokens`, `cache_read_tokens`, `start_context`, `peak_context`: measured, not estimated. `python3 ~/.claude/skills/deliver/bin/measure.py --retro` reads this session's transcript and prints them. If the run spanned several sessions (a resume after a crash), pass every session id. These keys are optional; leave one blank rather than guess

**The point is cost next to catch.** An agent asked only about friction votes to cut every check, because every check costs it time. A verdict with an empty Caught column is a preference, not evidence.

## Cost vs catch

| Step | Cost | Caught | Verdict |
|---|---|---|---|
| Inner loop, per slice | {runs × minutes} | {what a red run found, or "nothing"} | keep · tune · cut |
| Checkpoints (full gate) | {runs × minutes} | {the real defects a red run found, or "nothing"} | keep · tune · cut |
| Prove, at checkpoints | | | |
| Polish — local review | | | |
| Polish — security pass | | | |
| Polish — UX pass | | | |
| Polish — second opinion ({model}) | | | |
| test-cleanup | | | |
| {any other step, tier or lint rule that cost real time} | | | |

**A "cut" or "tune" names what the step failed to catch.** A step that caught a real defect is never "cut" on cost alone — say "tune", and how.

## Where I guessed

Instructions that contradicted each other, or were silent where a decision was needed. File and section for each, and what was guessed.

## What I wished existed

## Project friction → `/guardrails audit`

A gate tier, floor or lint rule that cost more than it caught *in this project*. Evidence, not preference.

## Framework friction → skills repo

A step, rule or template in the framework itself. Name the skill and the section.

## Score

One line on why.
