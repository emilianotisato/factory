# Retro — PRD-020

Written by `/deliver` in the closeout commit, from the agent's side of the run.

**The point is cost next to catch.** An agent asked only about friction votes to cut every check, because every check costs it time. A verdict with an empty Caught column is a preference, not evidence.

## Cost vs catch

| Step | Cost | Caught | Verdict |
|---|---|---|---|
| Inner loop, per slice | — | — | keep |
| Checkpoints (full gate) | — | — | keep |
| Prove, at checkpoints | — | — | keep |
| Polish — local review | — | scheduler `Dirty` dropped protected-file warnings; `default_memory_max` unit bug | keep |
| Polish — security pass | — | cache trim via canonicalize + `remove_dir_all` outside lane | keep |
| Polish — UX pass | skipped — CLI-only | nothing | cut |
| Polish — second opinion (claude:opus) | — | memcap cap math, systemctl `.scope`, reap/OOM false positives | keep |
| prune | — | nothing (no redundant tests removed; only test tightening) | tune |

## Where I guessed

- Whether to treat `main/target/lane-N` as live when lane env uses relative `target/lane-N`: slice-02 acceptance and §11 decisions say keep; second opinion argued they are always stale here. Parked as an idea rather than reversing a documented acceptance test in polish.
- `reset-failed` before reusing a gate systemd unit name: slice silent; assumed checkpoint reap + `.scope` stop is enough for this PR and parked fuller unit lifecycle work.

## What I wished existed

- A single helper tested once for “systemd scope unit string” so second-opinion passes would not each re-find the bare-name `.service` mismatch.
- Collect output indexed by severity with file:line anchors pre-verified, to shrink the polish triage read of seven passes.

## Project friction → `/gate audit`

Nothing in this branch argues for lowering an existing gate tier. The memcap default-size bug slipped through because the unit test duplicated the wrong formula instead of calling `default_memory_max()` — a small mutation or property test on the formatted string would catch that class cheaply.

## Framework friction → factory

- `/polish` §1: second-opinion volume on a 3k-line diff is high-signal but expensive to triage; a `--since-reviewed` polish round on fixes-only would help large features.
- Leaks check: clean verdict; no missed plan-number comments in this pass.

## Score

4/5 — checkpoints and mutation catches were strong; polish still had to fix a shipping memcap math bug the gate green-lit because the test mirrored the mistake.

## Numbers

<!-- factory:numbers -->
| role | workers | own | subagents | weighted | minutes | turns |
|---|---:|---:|---:|---:|---:|---:|
| build | 4 | 2376062 | — | 2376062 | 46 | — |
| checkpoint | 2 | 420513 | — | 420513 | 6 | — |
| polish | 1 | 396145 | — | 396145 | 10 | — |
| total | 7 | 3192721 | — | 3192721 | 62 | — |

Knobs: build cursor:composer-2.5 user/default · fix cursor:composer-2.5 user/default · checkpoint cursor:composer-2.5 user/default · polish cursor:composer-2.5 user/default · budget 30000000 default · lanes 4 · factory 0.17.0 · cursor 2026.11.22-e9042b2

Hypothesis: none

quality: gate_red 0 · worker_gate_red 0 · fix_commits 0 · ci_red_rounds 0 · ci_reruns 0 · findings high=1 medium=6 low=5 fixed=12 parked=2 · decisions 2 decide 0 blocks 2

Pack (build): slice 12584 · prd_sections 9414 · log 480 · outline 11854 · lookup 1115 · contract 3685 · autonomy 10282 · inner_loop 86 · total 49499 · start_context —

Compared with:

| run | feature | weighted | slices | per slice | lines changed | per line | comment share | id hits |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| 020-20261126-3c05 (this run) | 020 | 3192721 | 4 | 798180 | 3476 | 919 | 0.4% | 0 |
| 022-20261126-a09f | 022 | 2872907 | 3 | 957636 | 2330 | 1233 | 0.3% | 0 |
| 019-20261126-eafc | 019 | 3022280 | 3 | 1007427 | 1673 | 1807 | 0.1% | 0 |
| 027-20261125-904f | 027 | 7178094 | 4 | 1794524 | 5173 | 1388 | 2.9% | 0 |
<!-- /factory:numbers -->
