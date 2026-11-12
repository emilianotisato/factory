# Retro — PRD-017

Written by `/deliver` in the closeout commit, from the agent's side of the run.

**The point is cost next to catch.** An agent asked only about friction votes to cut every check, because every check costs it time. A verdict with an empty Caught column is a preference, not evidence.

## Cost vs catch

| Step | Cost | Caught | Verdict |
|---|---|---|---|
| Inner loop, per slice | one green run per slice commit, about a minute each | nothing red on record | keep: it is the floor every slice commit stands on |
| Checkpoints (full gate) | 2 runs × about 2 min, 0 red | nothing red. CP-2's leaks scan caught 10 decision ids in `src/settled.rs` test strings, fixed in place | keep |
| Prove, at checkpoints | 19 named tests across CP-1 and CP-2, transcripts in `evidence/` | nothing: every row green on its first run. It also passed `a_cancel_check_kills_every_running_pass` and `last_reviewed_commit_is_the_one_that_set_reviewed`, which polish then found could not fail on the defect they named | tune: a Prove row runs a test, it never asks whether the test can go red. A one-line mutation check (remove the guarded line, see red) on the riskiest row per checkpoint would have caught both |
| Polish — local review | one `code-review` fork at high effort | 9 findings: 2 medium (a stopped collect reused on resume; `--since` taken from the commit that cleared `reviewed`), 3 low defects (colored or external diff defeating the splitter, the probe ignoring a stop, two-dot since range), 2 cleanups, 2 rebutted | keep |
| Polish — security pass | one subagent briefed with `main...HEAD` | 2: the since range skips the previous polish's own fixes (parked), the clearing-commit bug (duplicate of the local review's). 4 low-confidence notes, all rebutted | keep: its top finding was raised by three other reviewers but by none of the code-focused ones first |
| Polish — UX pass | skipped | no UI: the surface is two CLIs and skill text, and the repo has no `verify-*` harness | n/a |
| Polish — second opinion (grok-4.7-high, Cursor) | 6 passes × 4 s, all `unreviewed`: Cursor's monthly usage limit | nothing. The fallback, 6 fresh same-vendor agents (one per pass, about 70-110k tokens each), found 7 medium and 9 low findings, among them the stop test that could not fail, the weak `last_reviewed` test, the owner-overturned decision still listed as settled, and polish §8 writing `reviewed` where nothing reads it | keep the fallback. The per-pass split earned its cost: pass 3 fuzzed the hunk splitter against 3,000 real diffs and found the two file-header and pathspec cases |
| prune | 3 scoped runs, about 15 s each | nothing: no duplicate or flaky test among the 34 the branch adds | keep, it is cheap at PR scope |

## Where I guessed

- **`skills/bundle.lock` after a skill edit on a branch that already bumped.** The log's note says to restore the lock with `git checkout -- skills/bundle.lock`, then relock. On this branch that restores the 0.13.3 lock, and `relock` refuses a new hash under the same version. Restoring main's lock (`git show main:skills/bundle.lock`) and relocking worked. The note covers only the first edit of a release.
- **Tool-written files in the fallback passes.** The collect ran with the installed 0.13.2 binary, which predates FR-3, so passes 1 and 2 held `log.jsonl` and `state.toml`. The fallback briefs left them out, as FR-3 now does.
- **FR-6's anchor.** Four reviewers showed that `R..HEAD` skips the previous polish's own fixes. The PRD defines R exactly, so I parked the remedy (`docs/ideas/2026-11-12-polish-since-start-before-the-last-polish-s-own-fi.md`) and did not change the anchor.

## What I wished existed

- A way to print a feature's settled block from the CLI (`factory review settled 017`). The fallback brief needs it, and I rebuilt it by hand from the log and the PRD. The binary has `settled::for_feature`, but no verb prints it.
- A probe that sees a usage limit. `--list-models` succeeded while every pass failed in 4 s on "You've hit your usage limit". Catching that in the probe would say "fallback owed: usage limit" before spawning six reviewers.

## Project friction → `/gate audit`

Nothing in this project's gate cost more than it caught. The full gate ran in about 2 minutes, three times.

## Framework friction → factory

- **`/polish` §8** told attended runs to set `reviewed` in the PRD's frontmatter. Nothing has read that since v3 moved it to `state.toml`, and FR-6's `--since` depends on it. Fixed in this branch.
- **`/polish` §1, the fallback.** "One agent per pass" on a fully unreviewed second opinion means 6 Opus agents. That is the right call here: they found most of the defects. But a usage limit is known on the first pass, and the cost lands without warning. The plan line could say why nothing counted ("usage limit") so the polisher knows it is not transient.
- **The driver collects with the installed binary**, so a feature that changes collect is polished by the old collect. That is expected, but it is worth one line in the pack, so the polisher does not read the old behaviour as a regression.

A comment or name the leaks check should have caught: none found.

## Score

4/5: every slice and checkpoint went green on the first run, but polish found 7 medium defects the build's tests missed, two of them in tests that could not fail.

## Numbers

<!-- factory:numbers -->
| role | workers | own | subagents | weighted | minutes | turns |
|---|---:|---:|---:|---:|---:|---:|
| build | 5 | 1873805 | 0 | 1873805 | 40 | 131 |
| checkpoint | 2 | 298335 | 0 | 298335 | 4 | 24 |
| polish | 1 | 1155865 | 2375640 | 3531505 | 16 | 57 |
| total | 8 | 3328006 | 2375640 | 5703646 | 59 | 212 |

Knobs: build opus/high default · fix opus/high default · checkpoint opus/high default · polish opus/high default · budget 30000000 default · lanes 4 · factory 0.13.2 · claude 2.1.316

Hypothesis: none

quality: gate_red 0 · worker_gate_red 0 · fix_commits 0 · ci_red_rounds 0 · ci_reruns 0 · findings high=0 medium=7 low=15 fixed=18 parked=4 · decisions 8 decide 0 blocks 0

Pack (build): slice 7698 · prd_sections 7540 · log 2629 · outline 9712 · lookup 1115 · contract 3095 · autonomy 8610 · inner_loop 86 · total 40485 · start_context 23966

Compared with:

| run | feature | weighted | slices | per slice | lines changed | per line | comment share | id hits |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| 017-20261112-a618 (this run) | 017 | 5703646 | 5 | 1140729 | 3138 | 1818 | 4.1% | 0 |
| 024-20261107-2f04 | 024 | 6104793 | 9 | 678310 | 7078 | 863 | 3.0% | 0 |
| 023-20261106-360f | 023 | 1594857 | 2 | 797429 | 1500 | 1063 | 1.2% | 0 |
| 014-20261105-4bd7 | 014 | 971439 | 1 | 971439 | 319 | 3045 | 5.9% | 0 |
<!-- /factory:numbers -->
