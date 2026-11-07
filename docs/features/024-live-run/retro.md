# Retro — PRD-024

Written by `/deliver` in the closeout commit, from the agent's side of the run.

**The point is cost next to catch.** An agent asked only about friction votes to cut every check, because every check costs it time. A verdict with an empty Caught column is a preference, not evidence.

## Cost vs catch

| Step | Cost | Caught | Verdict |
|---|---|---|---|
| Inner loop, per slice | one run per slice, about a minute each | nothing that reached a checkpoint red | keep |
| Checkpoints (full gate) | 3 runs × 1.7–1.9 min, 0 red | the gate: nothing. The leaks scan at each checkpoint caught plan ids in test strings and names (7 hits at CP-1, 35 at CP-2, 5 at CP-3) | keep: the leaks pass is what earns the checkpoint here |
| Prove, at checkpoints | 8 FR rows, all green first time | nothing red. Every row is a named test, so proving was rerunning tests the inner loop had already run | tune: for a CLI-only PRD whose rows are all `run` rows, the checkpoint gate's transcript is the proof |
| Polish — local review | one high-effort pass | stale CI excerpt on reds with no log, `gate.running` stuck after a dead driver, evidence root followed through a symlink, two doc comments moved onto the wrong function | keep |
| Polish — security pass | one subagent, ~3.5 min | strings cut before redaction, so a secret across the 300/160-char edge printed in part. Its notes also flagged the unchecked run id, a gate log path read from events, the bare home path and the unredacted block title | keep |
| Polish — UX pass | skipped | n/a: no screen. The surface is the `live --json` contract; `mods/cockpit` changed only version pins, and the gate runs `claude plugin validate`/`test` | keep the skip rule |
| Polish — second opinion (grok-4.7-high) | 15 passes, 596–1381 s each, about 4.3 h of reviewer time | agreed with the local review on the stale CI excerpt and `gate.running`, and with the security pass on redact-before-cut. On its own it found the FIFO/symlink gate log read, a heartbeat kept `running` past an oversized `tool_result`, the blank line before `make: ***` titling a block with nothing, `resumesAt` ignoring an extended pause, a 1 MiB single line read as empty, the CI-waiting gap (AD-16), and a test that could never fail | keep: the most findings and the most cross-agreement. Tune the pass size: the doc-only passes 1–2 spent 28 min re-reviewing code through the PRD |
| prune | one PR-mode pass over 79 added tests, 3 scoped runs | nothing to prune: the unit and end-to-end tests cover different layers, and most are named acceptance proofs | keep |

## Where I guessed

- **AD-13 against FR-7.** FR-7 says the redactor runs "at serialization time" over every string, while the sections cut strings to a width first. AD-13 accepted the partial leak. Polish reversed it (AD-14): redact, then cut, and run the final pass anyway.
- **What counts as the current CI log.** §6 numbers `ci/<round>.log` by pack round, and `ci_red` carries `rounds_done`, so no event names the file. I compared the log's mtime with the newest `ci_red`'s timestamp (AD-15). A `log` field on an event would be exact; it was not in the PRD.
- **CI waiting with no checks registered.** AD-4 left the set-is-empty case silent, and §4's `waitingOn` order then reads `nothing` for the grace window after every push. Logged for the owner (AD-16) rather than reversing a slice's decision in polish.
- **Bare `$HOME`.** `paths::home_to_tilde` has a test that keeps `/home/x` unchanged when it is not followed by `/`, while FR-7 says home paths become `~`. I did the bare rewrite in `live`'s redactor only and left the shared helper alone.

## What I wished existed

- A `log` or `round` field on `ci_red` that names the CI log file, written after `on_red` decides the path. Then neither `live` nor a block would need a clock comparison.
- A stream fixture with a `tool_result` over 64 KiB. The recordings are cut to 300-char strings, so the reader's skip path was never exercised against real data.

## Project friction → `/gate audit`

None with evidence. The gate ran 3 times at about 1.8 min each and was green each time; polish's fixes went through it once more, green after one `cargo fmt`.

## Framework friction → factory

- **The worker hook refuses a Bash call whose body contains `push`.** `out.push(` in a Python edit script and a heredoc AD body were both refused as "gh or git push". That cost two retries and pushed every multi-line edit through a scratch file. The log's Notes already record the heredoc case. The match should be on the command words, not on the body (`factory` hook, the worker Bash guard).
- **`docs log add` writes the main tree's `log.jsonl`, not the lane's.** That is right for the driver, but nothing in the polish contract says so, and `git status` in the lane shows no log change after an add. Say it in the contract's `docs log add` line.
- **The second opinion reviews docs as code.** Passes 1 and 2 were the PRD, slices and log, and their findings were all about the code those docs cite, which passes 3–15 covered again. `factory review collect` could fold doc-only files into the code passes they describe, or skip `docs/features/**` when the code is in the diff (`collect.rs`, pass grouping).

A comment or name the leaks check should have caught: none found. The two misplaced doc comments (`src/checkpoint.rs`, `failure_line`; `src/paths.rs`, `data_dir_path`) were placement errors, not plan references.

## Score

4. Every slice and Prove row was green first time, and polish closed 16 findings, including a real partial-secret leak the slice-09 tests could not see. A full point is off because the redaction and stale-CI defects were designed in at slicing, and only a cross-vendor reviewer and a security pass caught them.

## Numbers

<!-- factory:numbers -->
| role | workers | own | subagents | weighted | minutes | turns |
|---|---:|---:|---:|---:|---:|---:|
| build | 9 | 3284851 | 0 | 3284851 | 64 | 242 |
| checkpoint | 3 | 546219 | 0 | 546219 | 9 | 43 |
| polish | 1 | 1385299 | 888425 | 2273724 | 16 | 56 |
| total | 13 | 5216368 | 888425 | 6104793 | 89 | 341 |

Knobs: build opus/high default · fix opus/high default · checkpoint opus/high default · polish opus/high default · budget 30000000 default · lanes 4 · factory 0.13.0 · claude 2.1.316

Hypothesis: none

quality: gate_red 0 · worker_gate_red 0 · fix_commits 0 · ci_red_rounds 0 · ci_reruns 0 · findings high=2 medium=10 low=34 fixed=16 parked=26 · decisions 16 decide 3 blocks 0

Pack (build): slice 7910 · prd_sections 5455 · log 4225 · outline 9234 · lookup 1115 · contract 3095 · autonomy 8610 · inner_loop 86 · total 39730 · start_context 23968

Compared with:

| run | feature | weighted | slices | per slice | lines changed | per line | comment share | id hits |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| 024-20261107-2f04 (this run) | 024 | 6104793 | 9 | 678310 | 7078 | 863 | 3.0% | 0 |
| 023-20261106-360f | 023 | 1594857 | 2 | 797429 | 1500 | 1063 | 1.2% | 0 |
| 014-20261105-4bd7 | 014 | 971439 | 1 | 971439 | 319 | 3045 | 5.9% | 0 |
| 013-20261030-bf19 | 013 | 2582078 | 3 | 860693 | 2072 | 1246 | 1.7% | 0 |
<!-- /factory:numbers -->

**Owner review (2026-11-07):** 2 kept, 1 changed (AD-16). Only the decide ADs were reviewed; the 13 fyi ADs were left as the run took them.
