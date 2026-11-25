# Retro — PRD-026

Written by `/deliver` in the closeout commit, from the agent's side of the run.

**The point is cost next to catch.** An agent asked only about friction votes to cut every check, because every check costs it time. A verdict with an empty Caught column is a preference, not evidence.

## Cost vs catch

| Step | Cost | Caught | Verdict |
|---|---|---|---|
| Inner loop, per slice | 5 runs × ~2 min | slice-02: the cockpit mod's version pins had to follow `Cargo.toml` (AD-2). slice-01's targeted runs: unknown-key warning order, and a patch error blamed on the wrong edit | keep |
| Checkpoints (full gate) | 2 runs × ~3 min | nothing at either checkpoint. The leaks pass at CP-2 caught one ask-id literal in a test | keep |
| Prove, at checkpoints | 2 rounds, ~20 transcripts + 2 mutations | the mutations went red as they should (process-group kill, ask push). No defect found | keep |
| Polish — local review | 1 forked review, ~10 min | 8 findings. Real ones: an `agent` binary run under another provider; the Claude reviewer unable to read the diff outside the repo; a bare `opencode` answer impossible from `/deliver`; a stale `/harvest` knob | keep |
| Polish — security pass | 1 subagent, ~4 min | 5 findings + notes. Real: `-`-led model ids reaching argv, OpenCode `--variant` not existing in v2.4.1, OpenCode background service surviving a kill. False: the `.agents` symlink deletion is already refused by `render`. Side effect: its `opencode run --help` installed opencode@2.4.1 through a mise shim | keep, tune: tell it never to run a provider CLI through a shim |
| Polish — UX pass | skipped | no GUI and no `verify-*` harness; the CLI surface is covered by `tests/setup.rs` | — |
| Polish — second opinion (grok-4.7-high) | 10 passes, ~2.6 h wall in total | the most valuable set: the AD-6 contradiction in the answer path, wizard EOF and placeholder saves, `--show` dropping flags, the cockpit install exit code being dropped, all-passes-failed printing no `unreviewed:` line, and needles too weak to catch a skill regression. Several findings agreed independently with the local review | keep |
| prune | ~3 min | nothing: every added test proves a QA row or a fix | keep |

## Where I guessed

- **FR-4 argv vs a live binary.** FR-4 orders `opencode … --variant <effort>`, but its verification notes say OpenCode was never checked. The live v2.4.1 help has no `--variant`. I followed the binary and logged AD-11 as `decide`.
- **FR-4 "`<provider>` → model (given or provider default), and optional effort"** against the slice text `else p.default_effort()`. AD-6 and `factory setup` both leave effort unset, so I made the answer path match them (AD-12, `decide`).
- **AD-10's "first CLI found on PATH"** against the code's registry order. I left the code alone and parked it: the PRD says "first detected", and the registry order is deterministic.
- **slice-02's deliver note "with no `Other`"** cannot work for a provider with no default model. I added `Other` (AD-13).

## What I wished existed

- A `factory review second --dry-run` that prints each provider's argv. FR-4's flags were specified without a live check, and the only test is a stub that accepts any argv.
- A recorded `--help` snapshot per provider CLI in the repo, so a reviewer could check flags without running a lazy shim.

## Project friction → `/gate audit`

None that cost more than it caught. The two full gate runs in polish took about 2 minutes each.

## Framework friction → factory

- **`/polish` § 1, the security pass brief**: it does not forbid running third-party CLIs. My subagent ran `opencode run --help` through a mise shim, which installed a tool on the owner's machine. The skill should tell reviewer subagents never to run a binary they are not reviewing.
- **`/slice` for FR-4**: a slice that orders external CLI flags should carry a task to verify them against a live binary, or block. "Confirm at slice entry" in the PRD's notes was never acted on.
- **Leaks check**: nothing it should have caught.

## Score

4 — the build was clean and every checkpoint green, but the second opinion and polish found about fifteen real defects in setup and review edge cases that no slice test exercised.

## Numbers

<!-- factory:numbers -->
| role | workers | own | subagents | weighted | minutes | turns |
|---|---:|---:|---:|---:|---:|---:|
| build | 4 | 2105136 | 0 | 2105136 | 34 | 95 |
| checkpoint | 2 | 425902 | 0 | 425902 | 6 | 33 |
| polish | 1 | 1301652 | 968662 | 2270314 | 19 | 63 |
| total | 7 | 3832689 | 968662 | 4801352 | 59 | 191 |

Knobs: build opus/high default · fix opus/high default · checkpoint opus/high default · polish opus/high default · budget 30000000 default · lanes 4 · factory 0.15.0 · claude 2.1.332

Hypothesis: none

quality: gate_red 0 · worker_gate_red 0 · fix_commits 0 · ci_red_rounds 0 · ci_reruns 0 · findings high=5 medium=10 low=13 fixed=18 parked=10 · decisions 14 decide 3 blocks 0

Pack (build): slice 11840 · prd_sections 13611 · log 3004 · outline 11614 · lookup 1115 · contract 3685 · autonomy 9988 · inner_loop 86 · total 54942 · start_context 30829

Compared with:

| run | feature | weighted | slices | per slice | lines changed | per line | comment share | id hits |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| 026-20261125-a6be (this run) | 026 | 4801352 | 4 | 1200338 | 5075 | 946 | 2.8% | 0 |
| 025-20261125-021f | 025 | 2708704 | 4 | 677176 | 1798 | 1507 | 2.4% | 0 |
| 021-20261123-a324 | 021 | 4487033 | 5 | 897407 | 2193 | 2046 | 2.9% | 0 |
| 015-20261113-f9b6 | 015 | 4100530 | 5 | 820106 | 1786 | 2296 | 3.7% | 0 |
<!-- /factory:numbers -->

**Owner review (2026-11-25):** 13 kept, 0 changed (AD-1, AD-2, AD-3, AD-5, AD-6, AD-7, AD-8, AD-9, AD-10, AD-11, AD-12, AD-13, AD-14)
