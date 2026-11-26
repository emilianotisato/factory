# Retro — PRD-022

Written by `/deliver` in the closeout commit, from the agent's side of the run.

**The point is cost next to catch.** An agent asked only about friction votes to cut every check, because every check costs it time. A verdict with an empty Caught column is a preference, not evidence.

## Cost vs catch

| Step | Cost | Caught | Verdict |
|---|---|---|---|
| Inner loop, per slice | 3 slices × ~2 gate runs × ~2 min | slice-01 mutation: accidental `docs/project.toml` drop from `protected_files` | keep |
| Checkpoints (full gate) | 1 × ~6 min | nothing at CP-1 after prove | keep |
| Prove, at checkpoints | 5 FR transcripts at CP-1 | nothing new beyond tests | tune — transcripts are heavy; the gate already runs the same `cargo test` names |
| Polish — local review | 1 pass (Bugbot validator cleared raw findings) | relied on second opinion + security for signal | tune — local pass added little after validator strip |
| Polish — security pass | 1 pass | `sync_allow_answers` trusting worker-writable `log.jsonl` | keep |
| Polish — UX pass | skipped — CLI-only surface | n/a | cut (for this PRD class) |
| Polish — second opinion (claude:opus) | 4 passes × ~2 min | CI-fix bypass, merge-base panic, allowlist notice on main drift, hunk attribution gaps | keep |
| prune | skipped — green gate, no redundant new tests | nothing | keep |

## Where I guessed

- **Second opinion vs PRD on additive hunks:** reviewers flagged `removed == 0` skips as contradicting “never weaken tests”; PRD FR-5 explicitly allows purely additive hunks. Treated as rebut, not fix.
- **`sync_allow_answers` contract:** PRD says owner `allow` extends the run snapshot; security pass clarified that “run” means driver `BlockAnswered` events, not any resolved block in `log.jsonl`. Wired events without an AD — the skill is silent on log forgery.
- **Hunk → test name:** slice-03 and PRD endorse upward scan from hunk header; second opinion showed false positives when the header context names the prior `fn`. Parked rather than redesign mid-polish.

## What I wished existed

- A single `testguard_after_land` (or shared helper) from day one so CI-fix, checkpoint, and ship could not drift.
- Integration tests that call checkpoint/ship wiring, not only `testguard::check` in isolation.
- `factory review` output tagged “contradicts settled PRD” when a finding fights an FR acceptance line.

## Project friction → `/gate audit`

Nothing in this project’s gate tiers clearly cost more than they caught on this branch. The red worth remembering was mutation on slice-01 catching a one-line `protected_files` regression — that tier earned its keep.

## Framework friction → factory

- **`/polish` §1 + Bugbot validator:** the local review subagent reported issues, then the validator removed all of them, leaving polish dependent on second opinion for actionable defects. Skill: `polish` §1 local review.
- **Collected second opinion volume:** four Opus passes on ~2k lines produced overlapping findings (testguard attributed ~16 items). Triage cost dominated polish time. Skill: `polish` §2 verify/dedup — a severity floor in collect would help.

`collect.md:52-58` — second opinion defect 1 (CI-fix path) was the highest-signal line the local pass should have echoed in the pack.

## Score

4/5 — the guards shipped real hardening, but polish had to patch a serious allow-sync hole and a CI-fix bypass the slice tests would not have caught without cross-vendor review.