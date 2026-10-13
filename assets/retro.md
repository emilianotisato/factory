# Retro — PRD-{NNN}

Written by `/deliver` in the closeout commit, from the agent's side of the run.

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
