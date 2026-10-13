---
score:
gate_runs: 4
gate_red: 2
gate_minutes: 2.23
ci_cycles: 0
slices: 5
checkpoints: 2
build_minutes: 98.51
polish_minutes: 41.64
out_tokens: 321597
cache_read_tokens: 40618667
start_context: 32244
peak_context: 167069
---

# Retro — PRD-003

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
| Inner loop, per slice | 33 runs, 23 red, about 64 min across six slices (from each slice's Progress) | A v1-store migration that re-added columns (slice 01). A harness assertion that wrongly expected no `effort_source`, and `lanes == 2` (slice 04). The missing `pack_by_role` key (AD-403). Most of the other reds were sibling lanes mid-edit in the one shared checkout: all 5 of slice 05's, 2 of slice 04's. One was the known flaky `tests/commits.rs` test. | tune: about half of the reds were noise from the shared checkout, not defects |
| Checkpoints (full gate) | CP-1 3 runs, 2 red, 1.1 min · CP-2 1 run, 0 red, 1.1 min | `cargo fmt` drift and a clippy `type_complexity` error the slices' inner loops missed | keep |
| Prove, at checkpoints | FR-21 to FR-24 at CP-1, FR-25 at CP-2, each on the fixture and the real store | Real-store figures within 0.35% of the A/B table. It did not catch the fork double count: run 024's polish session has no forks. | keep |
| Polish — local review | one `code-review` pass | the transcript slug rule (confirmed against `~/.claude/projects`), `main..HEAD` counting main's commits, recount order, recount not writing `model_id`, a FINDINGS parse that stored nulls on a comma, a resume `--budget` not updating the knobs | keep |
| Polish — security pass | one subagent | 3 notes, none a defect: no `--` before the diff range (the base comes from repo metadata), a project file raising the budget (by design, OPEN-2), marker injection through the hypothesis (low, now covered by the marker refusal) | keep: cheap, and the diff builds a shell command |
| Polish — UX pass | skipped | CLI output only, and the repo has no `verify-*` harness | n/a |
| Polish — second opinion (Cursor `grok-4.7-high`, then Opus fallback) | Cursor: 15 min, killed by `timeout 900` with no result · Opus fallback: one pass | Cursor: nothing. Opus: the one high finding, a forked subagent's copied messages counted twice (verified on session `788a618e`), plus partial recount output on a closed pipe, unbalanced retro markers, stopped runs in the cohort, a `<synthetic>` model id | tune: the Cursor run cost 15 minutes and returned nothing |
| Polish — full gate | 3 runs, 1 red, about 2.4 min | my own regression: storing nulls for a missing transcript in a live run broke the kill-and-resume event list in `tests/checkpoints.rs` | keep |
| test-cleanup (pr mode) | 3 scoped runs of 195 tests, about 14 s each, 0 flaky | nothing to prune; two of polish's own new tests folded into their neighbours | keep |

## Where I guessed

- **Changed lines, `..` or `...`.** `slice-06-retro-numbers.md` task 1 and FR-25 both say `main..HEAD`. That counts commits landed on main after the branch, so I used three dots. AD-9901, marked decide.
- **The transcript slug.** FR-21 says "every `/` and `.` as `-`", and §13 listed the rule as "to confirm". The disk shows every non-alphanumeric character is mapped. I widened the rule (AD-9902).
- **A missing transcript in a live run.** FR-21 says "no `subagents/` dir → zeros", and slice 02 says the recount stores nulls when the transcript is gone. The live path is silent on a missing transcript. I first made it nulls plus an event. The gate went red: a worker killed before its first turn has no transcript. I reverted to zeros and parked the question in `docs/ideas.md`.
- **Commit or not.** `/polish` § 9 says "commit as you go", and test-cleanup says "commit each logical unit". The factory contract says the worker never commits. The contract won.
- **Renaming a test that a Progress line names.** test-cleanup § Preconditions 5 protects test names in evidence transcripts only. Slice 04's Progress names `model_id_is_the_first_own_assistant_lines_model`, and I had renamed it while folding a test in. I put the name back.
- **Severity of second-opinion findings.** The FINDINGS line wants "severity as found". The fallback reviewer gave none, so the severities are mine.

## What I wished existed

- **A forked-subagent fixture in the stub.** The double count passed every slice and checkpoint because the one fixture, cut from `ddbb0927`, has no fork. A second fixture with copied message ids would have caught it at slice 01.
- **Partial output from the second opinion.** A 900 s kill leaves a log with two preamble messages. The only choices are "unreviewed" or a fallback, and the 15 minutes buy nothing.
- **A guard in `retro.md`'s template.** The retro and the driver's numbers section share one file. A writer who quotes the marker text breaks the next write. The refusal now makes this safe, but the template does not warn.

## Project friction → `/guardrails audit`

- **`tests/commits.rs` `a_file_git_added_by_name_is_committed_beside_a_live_sibling` is flaky under load.** It went red in slice 06's inner loop and in earlier runs, then green 5 of 5 alone. Each time it costs a full gate rerun and a Trap lookup. It needs a stabilise pass, not a retry rule.
- **Four lanes on one checkout redden each other's inner loop.** Slice 05's Progress: "Every red was a sibling lane mid-edit". Slice 04 lost 2 of 9 runs the same way. The lanes give separate databases, not separate trees. Worktree-per-lane would remove this whole class of red.

## Framework friction → skills repo

- **`polish` § 1 + `feature-docs/models.md` "A diff".** The invocation runs under a 900 s timeout. For a 31-file, 3.8k-line branch, Cursor produced nothing before the kill. Either scope it to the risky subset by default past ~30 files, or have the invocation stream findings so a kill keeps what it found.
- **`polish` § 9 vs the factory contract.** § 9's Unattended line still says to commit `reviewed:` on its own, and the test-cleanup skill commits per unit. A factory worker must not commit at all. Add a `factory` case to the Unattended lines, or point them at the contract.
- **`test-cleanup` § Preconditions 5.** It protects test names in evidence transcripts, but slice Progress lines name test methods too. Extend the rule to the feature folder.
- **`feature-docs` retro template.** It still tells the writer to run `measure.py` (AD-604, open). FR-25 retired it; `factory retro` replaces it.

## Score

The owner's to set. From the agent's side, the framework served well: checkpoints and the Opus fallback caught real defects, and the main cost was noise from the shared checkout and a second opinion that timed out.

**Owner review (2026-10-13):** 18 kept, 0 changed (AD-102, 105, 202, 203, 206, 301, 303, 401, 402, 601, 604, 605, 901, 9901, 9902, 9903, 9904, 9905)
