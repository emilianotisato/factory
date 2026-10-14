# Retro — PRD-005

Written by `/deliver` in the closeout commit, from the agent's side of the run.

**The point is cost next to catch.** An agent asked only about friction votes to cut every check, because every check costs it time. A verdict with an empty Caught column is a preference, not evidence.

## Cost vs catch

| Step | Cost | Caught | Verdict |
|---|---|---|---|
| Inner loop, per slice | one run or more per slice, about 1 to 1.5 min each | each slice's own red, fixed before its commit; it never saw a sibling's uncommitted file, so it missed the 22 files HEAD lacked | keep |
| Checkpoints (full gate) | 5 runs over CP-1 to CP-4, 1 red, about 6 min | CP-3: a driver test that raced the driver's "it logged a block" line (4c3581c). The gates ran on the shared working tree, so none saw that HEAD did not build | tune: gate the commit, not the tree |
| Prove, at checkpoints | 21 evidence files over 4 checkpoints | nothing went red. It drove FR-1 to FR-10 through the stub, but every scenario was the happy path or the one conflict the slice named | keep, tune: add one hostile case per FR |
| Polish — local review | one pass over the whole diff | 10 findings, 4 high: unmerged paths with no markers landed (A); no rollback after a failed pick (B); an empty merge lost the ref (D); an open checkpoint block did not hold the run (H) | keep |
| Polish — security pass | one subagent | 1 finding: ignored files under a picked path got overwritten (N). 4 notes parked: a planted ref, a lane is not a sandbox, symlinked seeds, poisoned ignored files | keep |
| Polish — UX pass | skipped | nothing: no UI, and the CLI text changed only in block bodies | n/a |
| Polish — second opinion (Claude fallback: Cursor unauthenticated, exit 2) | 9 fresh agents, one per ~600 lines | 3 highs no other reviewer found: rename on the tip (C), a done slice's worker settled failed on a routing error (F), two runs sharing lanes (Q, parked). Also E, O, R, P, V, Y | keep; get Cursor back, since a same-vendor fallback is weak evidence |
| prune | 1 agent over 90 scoped tests | nothing to prune. It named 4 tests ordered by `delay_ms`; one (`tests/merge.rs`) now uses a `wait_sh` barrier | keep |
| Polish fixes and gate | 3 full-gate runs, about 1.3 min each | the 13 new regression tests stayed green; the gate found nothing new | keep |

## Where I guessed

- **HEAD did not build when polish started.** The v0.2.1 driver commits a slice by pathspec, and 22 files of slice code stayed in the tree uncommitted (AD-5 and AD-6 saw it as it happened). The contract says the driver commits what I wrote, but it says nothing about code no slice claimed. I claimed it (AD-20, `decide`).
- **`/polish` §1 picks the reviewer set by diff, and the contract says "in full".** I read "in full" as all of them except UX, because there is no screen.
- **`/polish` §8 says "the slice's Code-review fixes section", but a finding often spans two slices.** I filed each fix under the slice whose code it changed. For example, the lane claim (F, E) went to slice 07, though slice 08 called it.
- **A merge with no markers but an unmerged entry.** The slice-08 acceptance tested only marker conflicts, so I chose to block (AD-21).
- **The log Note about trimming setup quotes is now stale.** Slice 03 wrote it, and this polish fixed the trim. I left the Note alone, because only the tool writes the log.

## What I wished existed

- **A gate that runs on the commit.** A `git worktree` at HEAD for every checkpoint gate. FR-5 now does this for later runs. This run was built by the old driver, so it never benefited.
- **A barrier in the stub from the start.** `wait_sh` exists now. Four driver tests still order their lanes with `delay_ms`, and one of those already went red at CP-3.
- **A way to tell a reviewer subagent "these files are another slice's, mid-edit".** Three reviewers flagged code that was simply uncommitted.

## Project friction → `/gate audit`

- **`cargo test` stops at the first red test binary.** While a sibling's file is red, nobody reaches their own tests without `--no-fail-fast`. The log's Notes say so twice. The gate could pass `--no-fail-fast`: it costs nothing when green and shows every red at once.
- **`cargo fmt --check` over the crate goes red on a sibling's mid-edit file.** Lanes fix this for the next run. This run paid for it at every slice.

## Framework friction → skills repo

- **The worker hook's push rule matches the words, not the command.** It refused a Python heredoc that held Rust's `.push(` next to the word `git`. It also refuses recursive grep over `src`. Each refusal cost a round trip and a scratch file. Match the parsed command instead, in the factory hook that `factory init` installs.
- **`/polish` §1, second opinion.** The fallback when Cursor is out is a same-vendor agent. The skill says this is weak evidence, but nothing tells the owner before the run starts. Preflight could check `cursor-agent` auth once and name it.
- **`/polish` §8 and the bundle lock.** A skill edit needs a version bump, and the lock refuses a second change at one version. So a polish worker cannot fix a stale skill line it finds. It can only park the line.
- **`/deliver`, the slice commit by pathspec.** It is the cause of AD-5, AD-6 and AD-20. PRD-005 deletes it. The friction is recorded here because this was the last run to pay for it.

## Score

3: every FR landed and every checkpoint was green, but HEAD did not build until polish, and review found 7 high defects that 4 checkpoints passed.

## Numbers

<!-- factory:numbers -->
| role | workers | own | subagents | weighted | minutes | turns |
|---|---:|---:|---:|---:|---:|---:|
| build | 10 | 9646020 | 0 | 9646020 | 137 | 534 |
| fix | 1 | 117204 | 0 | 117204 | 2 | 10 |
| checkpoint | 4 | 1018937 | 0 | 1018937 | 8 | 73 |
| polish | 1 | 1574310 | 4212259 | 5786569 | 32 | 76 |
| total | 16 | 12356471 | 4212259 | 16568730 | 179 | 693 |

Knobs: build opus/high default · fix opus/high default · checkpoint opus/high default · polish opus/high default · budget 30000000 default · lanes 4 · factory 0.2.1 · claude 2.1.296

Hypothesis: none

quality: gate_red 1 · worker_gate_red 0 · fix_commits 1 · ci_red_rounds 0 · ci_reruns 0 · findings high=7 medium=13 low=20 fixed=15 parked=24 · decisions 24 decide 4 blocks 0

Pack (build): slice 12665 · prd_sections 7951 · log 5422 · outline 12215 · lookup 890 · contract 3328 · autonomy 10906 · inner_loop 91 · total 53466 · start_context 48692

Compared with:

| run | feature | weighted | slices | per slice | lines changed | per line |
|---|---|---:|---:|---:|---:|---:|
| 005-20261014-d1ad (this run) | 005 | 16568730 | 10 | 1656873 | 9217 | 1798 |
| 004-20261013-f6d3 | 004 | 7369085 | 7 | 1052726 | 9785 | 753 |
| 002-20261013-e729 | 002 | 15464292 | 11 | 1405845 | 24701 | 626 |
| 003-20261013-4139 | 003 | 6966177 | 5 | 1393235 | — | — |
<!-- /factory:numbers -->
