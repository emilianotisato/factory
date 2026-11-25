# Retro — PRD-025

Written by `/deliver` in the closeout commit, from the agent's side of the run.

**The point is cost next to catch.** An agent asked only about friction votes to cut every check, because every check costs it time. A verdict with an empty Caught column is a preference, not evidence.

## Cost vs catch

| Step | Cost | Caught | Verdict |
|---|---|---|---|
| Inner loop, per slice | not recorded: no slice wrote a Progress inner-loop line | unknown from the slices' own records | tune: a slice needs to write its runs and reds, or this row stays blank |
| Checkpoints (full gate) | 1 run, about 2 min | nothing; it was green the first time | keep: it is cheap at this size |
| Prove, at checkpoints | 4 rows, 14 named tests, one mutation probe | the mutation probe showed the `[skip ci]` ship stamp is pinned. Nothing about the defects below | keep. The probe chose the one stamp a test already covered, while the `refresh_numbers` stamp beside it has no test. Tune: probe the branch of a pair that no test names |
| Polish — local review | one `code-review` run at high | 3 important: the wrong "mark ready" recovery, the symlink/permissions loss in `write_ci_mode`, and `paths-ignore` on a required check. 7 nits | keep |
| Polish — security pass | one subagent, about 1 min | permissions dropped and symlink replaced, which the local review also found. The un-patchable `[ci]` header inside a string, which the second opinion also found. fsync | keep: config writes are exactly its lens |
| Polish — UX pass | skipped | n/a: CLI only, and there is no verify harness | n/a |
| Polish — second opinion (grok-4.7-high) | 3 passes, 2,520 s summed | the same wrong recovery, independently, in two passes. The `/deliver` review-commit order. `paths-ignore` and the squash claim. The un-patchable header. The `pr edit` block text. Four test gaps, two of them fixed here | keep. Pass 3, over the test files alone, found the test gaps that the other reviewers did not |
| prune | one classification over 16 branch tests | nothing removed | keep: cheap in PR mode |

## Where I guessed

- **The PRD's own recovery text is wrong, and the PRD is not mine to edit.** `prd.md` §2 (Blast radius), FR-2's ask wording and FR-3's `ci_local` line all say that marking the PR ready runs the required check. Three reviewers showed it does not while HEAD carries `[skip ci]`. I kept FR-3's stamps, corrected the four user-facing strings, logged AD-5 as `decide`, and put the PRD correction in the PR body as a proposal.
- **FR-3's fallback `state.ci_mode.or(cfg.ci.mode)` contradicts the non-goal "pre-upgrade runs finish remote".** That holds once `config.toml` sets a mode. I treated FR-3's explicit formula as settled and rebutted the finding. Only the field's doc comment changed.
- **FR-1 lists only `Flag`, `User` and `Default` sources**, so a mode set through the env var is recorded as `default` in knobs, while preflight prints `(env)`. I left that as the FR wrote it.

## What I wished existed

- A fixture knob that leaves `retro.md` dirty at push, and one that re-enters closeout after `open_pr`. The two untested local-mode branches (the `refresh_numbers` stamp, and `ci_local` once per head) are parked in `docs/ideas/` because no fixture reaches them cheaply.
- A "GitHub semantics" note the PRD interview could check against. Every reviewer here knew the `[skip ci]`/`ready_for_review` rule. The PRD and its adversarial pass both missed it, because neither was asked to check the platform's behaviour, only the code's.

## Project friction → `/gate audit`

None with evidence. The gate ran once at the checkpoint and once in polish, each about two minutes, and caught nothing either time. At this cost that is not a reason to cut it.

## Framework friction → factory

- **The worker hook refuses any Bash text containing the p-u-s-h verb.** That includes a Python heredoc that only edits a Rust string saying "push one commit". The run's Notes already record this. Polish hit it again and had to redo the edits with the Edit tool. The hook should match the command token, not free text (`hooks`, worker guard).
- **`/prd` § adversarial pass.** It verifies `file:line` claims but never asks "is the external platform behaviour this FR relies on true?" That gap let the wrong recovery reach four files.
- **`/slice` Progress lines.** Nothing forced a slice to record its inner-loop runs, so this retro's first row has no data.

The leaks check missed nothing: the branch adds no comment or name that points at the plan.

## Score

3/5. The code is sound and the review fixes were small, but the PRD shipped a wrong user-facing recovery into four places, and only the polish reviewers caught it.

## Numbers

<!-- factory:numbers -->
| role | workers | own | subagents | weighted | minutes | turns |
|---|---:|---:|---:|---:|---:|---:|
| build | 4 | 1095315 | 0 | 1095315 | 24 | 95 |
| checkpoint | 1 | 218455 | 0 | 218455 | 4 | 18 |
| polish | 1 | 807606 | 587328 | 1394934 | 11 | 40 |
| total | 6 | 2121377 | 587328 | 2708704 | 39 | 153 |

Knobs: build opus/high default · fix opus/high default · checkpoint opus/high default · polish opus/high default · budget 30000000 default · lanes 4 · factory 0.14.0 · claude 2.1.332

Hypothesis: none

quality: gate_red 0 · worker_gate_red 0 · fix_commits 0 · ci_red_rounds 0 · ci_reruns 0 · findings high=1 medium=2 low=13 fixed=15 parked=1 · decisions 5 decide 1 blocks 0

Pack (build): slice 8905 · prd_sections 6869 · log 611 · outline 8226 · lookup 1115 · contract 3685 · autonomy 9988 · inner_loop 86 · total 39484 · start_context 24519

Compared with:

| run | feature | weighted | slices | per slice | lines changed | per line | comment share | id hits |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| 025-20261125-021f (this run) | 025 | 2708704 | 4 | 677176 | 1798 | 1507 | 2.4% | 0 |
| 021-20261123-a324 | 021 | 4487033 | 5 | 897407 | 2193 | 2046 | 2.9% | 0 |
| 015-20261113-f9b6 | 015 | 4100530 | 5 | 820106 | 1786 | 2296 | 3.7% | 0 |
| 018-20261112-68dc | 018 | 2243893 | 4 | 560973 | 1343 | 1671 | 1.2% | 0 |
<!-- /factory:numbers -->
