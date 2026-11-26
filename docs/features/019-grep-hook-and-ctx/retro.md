# Retro — PRD-019

Written by `/deliver` in the closeout commit, from the agent's side of the run.

**The point is cost next to catch.** An agent asked only about friction votes to cut every check, because every check costs it time. A verdict with an empty Caught column is a preference, not evidence.

## Cost vs catch

| Step | Cost | Caught | Verdict |
|---|---|---|---|
| Inner loop, per slice | three slices × one green inner loop each | nothing | keep |
| Checkpoints (full gate) | two checkpoint gates, ~2 min for CP-1; CP-2 cached | CP-1 mutation drills on `standalone` and guard `return false` | keep |
| Prove, at checkpoints | bundled with checkpoints | transcript rows only | keep |
| Polish — local review | one pass over the branch diff | Bun/zx guard gap, deny-reason UTF-8 panic (merged with second opinion) | tune — Bugbot silent on the same bypass |
| Polish — security pass | one subagent on `hook.rs` / `shell.rs` | library-only `push` (accepted cost, not a fix) | keep |
| Polish — UX pass | skipped — cockpit diff is version strings; `claude plugin validate` ran in gate | nothing | cut — no live-session UX for a version bump |
| Polish — second opinion (claude:opus) | three passes, ~8 min wall in collect | guard bypass, truncation panic, vacuous storage test, corpus gaps, glued `rg -e` | keep |
| prune | not run — gate green after fixes; only additive regression tests | nothing | tune — PR mode still worth automating after every polish |

**A "cut" or "tune" names what the step failed to catch.** A step that caught a real defect is never "cut" on cost alone — say "tune", and how.

## Where I guessed

- **`skills/slice/SKILL.md` vs § QA plan for guard loosening:** second opinion flagged that “carry corpus into Prove” can contradict “Prove cites § QA plan only.” I parked it in the PR body rather than editing skills mid-closeout.
- **Multi-name `const` in `ctx def`:** tried comma-suffix regex patterns; tree-sitter `confirmed()` still drops untagged names. Reverted the regex and parked an idea instead of widening `confirmed()` without an FR.
- **Output redirects on standalone greps:** second opinion treated `grep … > file` as a bug; the PRD explicitly keys standalone on pipes/chains only. Parked as idea, no code change.

## What I wished existed

- A hook unit test helper that seeds non-ASCII literals at a chosen byte offset inside `MAX_DENY_BYTES` so denial truncation cannot pass by ASCII luck.
- Corpus generator from `git show main:src/cli/hook.rs` deny assertions so FR-1 loosenings replay old grep denials, not only `guard()`.

## Project friction → `/gate audit`

- Full gate runs `claude plugin test` spelling that does not execute tests (validator warns); only `validate` ran. Low signal for cockpit changes until the gate string matches the CLI.

## Framework friction → factory

- **`/polish` §1 + unattended contract:** second opinion findings are mandatory to verify, but easy to under-weight when Bugbot and local review are quiet — the cross-vendor pass paid for itself on a “green” branch.
- **`leaks` comment share:** verdict was clean; nothing to add at `file:line`.

## Score

4/5 — Second opinion found real ship-stoppers after green checkpoints; polish could fix them in one pass, but UX/review tiers mostly duplicated hook work except the opus pass.

## Numbers

<!-- factory:numbers -->
| role | workers | own | subagents | weighted | minutes | turns |
|---|---:|---:|---:|---:|---:|---:|
| build | 3 | 2018337 | — | 2018337 | 21 | — |
| checkpoint | 2 | 415991 | — | 415991 | 4 | — |
| polish | 1 | 587951 | — | 587951 | 10 | — |
| total | 6 | 3022280 | — | 3022280 | 34 | — |

Knobs: build cursor:composer-2.5 user/default · fix cursor:composer-2.5 user/default · checkpoint cursor:composer-2.5 user/default · polish cursor:composer-2.5 user/default · budget 30000000 default · lanes 4 · factory 0.16.0 · cursor 2026.11.22-e9042b2

Hypothesis: none

quality: gate_red 0 · worker_gate_red 0 · fix_commits 0 · ci_red_rounds 0 · ci_reruns 0 · findings high=1 medium=2 low=5 fixed=5 parked=4 · decisions 1 decide 0 blocks 1

Pack (build): slice 11142 · prd_sections 9651 · log 234 · outline 11245 · lookup 1115 · contract 3685 · autonomy 10282 · inner_loop 86 · total 47439 · start_context —

Compared with:

| run | feature | weighted | slices | per slice | lines changed | per line | comment share | id hits |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| 019-20261126-eafc (this run) | 019 | 3022280 | 3 | 1007427 | 1673 | 1807 | 0.1% | 0 |
| 022-20261126-a09f | 022 | 2872907 | 3 | 957636 | 2330 | 1233 | 0.3% | 0 |
| 027-20261125-904f | 027 | 7178094 | 4 | 1794524 | 5173 | 1388 | 2.9% | 0 |
| 026-20261125-a6be | 026 | 4801352 | 4 | 1200338 | 5075 | 946 | 2.8% | 0 |
<!-- /factory:numbers -->
