# Retro — PRD-029

Written by `/deliver` in the closeout commit, from the agent's side of the run.

**The point is cost next to catch.** An agent asked only about friction votes to cut every check, because every check costs it time. A verdict with an empty Caught column is a preference, not evidence.

## Cost vs catch

| Step | Cost | Caught | Verdict |
|---|---|---|---|
| Inner loop, per slice | four slice verify runs, each one green on first try | nothing | keep |
| Checkpoints (full gate) | four checkpoint gates, each one green | nothing (mutation at CP-1/2/4 proved skip logic) | keep |
| Prove, at checkpoints | manual evidence paths; BLOCK-1 once for missing FR-3 disk proof | dropped prove rows twice before block | tune — prove contract and pack must name tests that do not exist yet |
| Polish — local review | triage over pre-collected `collect.md` plus targeted reads | claims-index pipe split, preflight ask gap, classifier false positives | keep — driver collect amortizes cost |
| Polish — security pass | one subagent on `main...HEAD` | nothing | keep — cheap for CLI/signals diff |
| Polish — UX pass | skipped | n/a | cut on this feature — no GUI; correct skip |
| Polish — second opinion (claude:opus) | five passes, all counted | same defects local triage would have missed without cross-pass merge | keep |
| prune | skipped after green polish gate | n/a | keep — few new tests, fixes added regression assertions inline |

**A "cut" or "tune" names what the step failed to catch.** Checkpoint prove rows that reference slice-04 tests before slice-04 lands are a template bug, not a worker bug; tune the checkpoint pack to defer FR rows.

## Where I guessed

- **`marker_matches` for `"login"`** — PRD §5 lists bare `"login"` as an auth marker; polish narrowed it to phrases because cross-vendor review showed trace false positives. Kept `"not logged in"` and siblings as-is.
- **Whether docs-only planning branches should review their own PRD when absorbing ideas** — §3 FR-1 says drop `feature-doc` whenever any non-idea path changes; idea deletions count. Treated second-opinion concern as re-litigation, not fix.

## What I wished existed

- A single `factory review triage` output that merges Claims index, leaks, and local lint into one machine-readable file so polish does not re-walk five pass sections.
- `factory docs set reviewed=` optionally accepting `reviewed_head=<sha>` in one call for attended polish rounds.
- Shared-lane `CARGO_TARGET_DIR` guard in the gate that rebuilds `factory` before integration tests that shell out.

## Project friction → `/gate audit`

- Parallel lanes sharing a global `CARGO_TARGET_DIR` made integration tests flaky against a stale `factory` binary; the feature log Note caught it, but the default gate does not enforce rebuild-before-shell-out tests.

## Framework friction → factory

- **`/polish` §1** — when `# Collected` is present, the skill still says “local review first” but the worker pack already ran collect + second opinion; clarify that triage may start from Claims index without a second full local subagent when collect is fresh.
- **`/deliver` checkpoint Prove rows** — naming FR acceptance tests before the slice that adds them caused BLOCK-1; checkpoint templates should mark rows as `deferred until slice-NN`.
- **Leaks check** — comment-share was clean; no miss. (No `file:line` leak this run.)

## Score

4/5 — Feature delivered with strong test surface; polish caught real product gaps in parsing and settled-block shape. Lost a point to checkpoint/evidence friction and attended `reviewed_head` semantics left for a follow-up idea.

## Numbers

<!-- factory:numbers -->
| role | workers | own | subagents | weighted | minutes | turns |
|---|---:|---:|---:|---:|---:|---:|
| build | 4 | 3326945 | — | 3326945 | 80 | — |
| checkpoint | 3 | 784695 | — | 784695 | 11 | — |
| polish | 1 | 342894 | — | 342894 | 5 | — |
| total | 8 | 4454535 | — | 4454535 | 96 | — |

Knobs: build cursor:composer-2.5 user/default · fix cursor:composer-2.5 user/default · checkpoint cursor:composer-2.5 user/default · polish cursor:composer-2.5 user/default · budget 30000000 default · lanes 4 · factory 0.18.0 · cursor 2026.11.22-e9042b2

Hypothesis: none

quality: gate_red 0 · worker_gate_red 0 · fix_commits 0 · ci_red_rounds 0 · ci_reruns 0 · findings high=1 medium=3 low=3 fixed=5 parked=2 · decisions 0 decide 0 blocks 1

Pack (build): slice 10639 · prd_sections 9109 · log 1017 · outline 10727 · lookup 1115 · contract 3685 · autonomy 10282 · inner_loop 86 · total 46661 · start_context —

Compared with:

| run | feature | weighted | slices | per slice | lines changed | per line | comment share | id hits |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| 029-20261127-4c0a (this run) | 029 | 4454535 | 4 | 1113634 | 2815 | 1582 | 1.0% | 2 |
| 028-20261126-93f3 | 028 | 4190434 | 4 | 1047609 | 2374 | 1765 | 0.8% | 0 |
| 022-20261126-a09f | 022 | 2872907 | 3 | 957636 | 2330 | 1233 | 0.3% | 0 |
| 020-20261126-3c05 | 020 | 3192721 | 4 | 798180 | 3476 | 919 | 0.4% | 0 |
<!-- /factory:numbers -->
