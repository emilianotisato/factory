# Retro — PRD-006

Written by `/deliver` in the closeout commit, from the agent's side of the run.

**The point is cost next to catch.** An agent asked only about friction votes to cut every check, because every check costs it time. A verdict with an empty Caught column is a preference, not evidence.

## Cost vs catch

| Step | Cost | Caught | Verdict |
|---|---|---|---|
| Inner loop, per slice | about 7 runs over five slices, about 2 min each (slice 03's Progress does not record its count) | one red, a `cargo fmt` slip in slice 05 | keep |
| Checkpoints (full gate) | 2 runs × 1.5 min | nothing | keep: 3 minutes in total is cheap |
| Prove, at checkpoints | the `--all` runs over cody-agent and arca-api, about 4 s each, plus a hit-by-hit verdict read of 3452 hits | about 160 false hits on foreign names and non-history senses, tuned away in AD-3, AD-4 and AD-5. That kept the rule table honest before it shipped | keep |
| Polish — local review | one `code-review` pass | `factory leaks` exiting 1 when the scan did not run, so a bad ref read as hits to fix. Also tab-ended and quoted diff paths filed under the wrong name, and a fixed fence a hit could close | keep |
| Polish — security pass | one subagent | `--base` starting with `-` reached `git diff` as an option (`--output=<file>` writes a file). Also a silent allowlist glob, and git's whole stderr in the report | keep |
| Polish — UX pass | skipped: no UI, no `verify-*` harness | n/a | n/a |
| Polish — second opinion (Cursor, unreviewed: exit 2; fallback: three fresh Claude agents, one per pass) | three agents, about 39 raw findings, most duplicates | the template-placeholder friction line that still filed an idea, the untested hostile local git config, and the multi-line splitter gaps that were parked | tune: Cursor's auth failed silently until the run, so the fallback cost three agents where one call was planned |
| prune | three scoped runs, about 2 min | nothing to prune or consolidate: each new test covers a distinct behaviour | keep: it is cheap, and an empty result is a real answer |
| `factory leaks` on this branch | under 1 s | nothing: the branch is `clean` | keep |

## Where I guessed

- **The exit code of a scan that could not run.** FR-3 names the report and the `fix` hits but not the exit code. Polish § 1 reads "anything else means the check did not run". I chose 2 and logged it as AD-6 (fyi).
- **What an allowlist `path` may hold.** The PRD names paths, not patterns. I made a glob or an absolute path an error rather than a silent no-match. This is AD-6 too.
- **Where cross-slice review fixes go.** Polish § 8 says "the slice's **Code-review fixes** section", and for feature-wide fixes "the last slice touched". The fixes split cleanly by slice, so each went to the slice that owns the code: 01, 03, 04 and 05.
- **The polish skill's own closeout.** § 9 commits, ships and sets `reviewed` in the frontmatter. The contract says do not commit or ship, and set `reviewed` through `docs set`. The contract won, as its header says.

## What I wished existed

- **A Cursor auth check in preflight.** It would have caught the `unreviewed` second opinion before polish, not during it.
- **Inner-loop runs read from the store.** A slice whose Progress prose leaves the count out leaves this table guessing.
- **One base-ref resolver** shared by pack, closeout and leaks. Filed as an idea.

## Project friction → `/gate audit`

None. The full gate costs about 1.5 minutes, and the inner loop's one red was a real formatting slip. Nothing here cost more than it caught.

## Framework friction → skills repo

- **polish § 1, second opinion.** When Cursor is `unreviewed`, the fallback costs one fresh agent per 600-line pass. On this 2000-line diff that was three agents, mostly re-finding the local review's points. A cheap auth probe in `/deliver` preflight would move the failure to where it costs nothing.
- **The worker PreToolUse hook.** It refuses any Bash command whose text contains the push or GitHub CLI words, even inside a heredoc of an edit script. Every multi-line edit had to go through a script file first.
- **polish § 8.** "The slice's Code-review fixes section" assumes one slice. A per-slice rule ("each fix goes to the slice that owns the code it touches") would remove the guess.

## Score

4/5: the feature shipped with field-verified counts, and polish caught two real medium defects (the exit code, and option injection through `--base`) that two green checkpoints did not. One point off because the second opinion's vendor was down and the fallback was costly.

## Numbers

<!-- factory:numbers -->
| role | workers | own | subagents | weighted | minutes | turns |
|---|---:|---:|---:|---:|---:|---:|
| build | 5 | 2746335 | 0 | 2746335 | 40 | 141 |
| checkpoint | 2 | 453155 | 0 | 453155 | 4 | 34 |
| polish | 1 | 877206 | 1301755 | 2178961 | 17 | 45 |
| total | 8 | 4076695 | 1301755 | 5378450 | 60 | 220 |

Knobs: build opus/high default · fix opus/high default · checkpoint opus/high default · polish opus/high default · budget 30000000 default · lanes 4 · factory 0.3.0 · claude 2.1.296

Hypothesis: none

quality: gate_red 0 · worker_gate_red 0 · fix_commits 0 · ci_red_rounds 0 · ci_reruns 0 · findings high=0 medium=13 low=12 fixed=16 parked=9 · decisions 6 decide 1 blocks 0

Pack (build): slice 13017 · prd_sections 16124 · log 1563 · outline 8775 · lookup 890 · contract 3180 · autonomy 10906 · inner_loop 91 · total 54546 · start_context 49332

Compared with:

| run | feature | weighted | slices | per slice | lines changed | per line |
|---|---|---:|---:|---:|---:|---:|
| 006-20261014-0643 (this run) | 006 | 5378450 | 5 | 1075690 | 6185 | 870 |
| 005-20261014-d1ad | 005 | 16568730 | 10 | 1656873 | 9217 | 1798 |
| 004-20261013-f6d3 | 004 | 7369085 | 7 | 1052726 | 9785 | 753 |
| 002-20261013-e729 | 002 | 15464292 | 11 | 1405845 | 24701 | 626 |
<!-- /factory:numbers -->

**Owner review (2026-10-14):** 4 kept, 1 changed (AD-4)
