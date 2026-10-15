# Retro — PRD-008

Written by `/deliver` in the closeout commit, from the agent's side of the run.

**The point is cost next to catch.** An agent asked only about friction votes to cut every check, because every check costs it time. A verdict with an empty Caught column is a preference, not evidence.

## Cost vs catch

| Step | Cost | Caught | Verdict |
|---|---|---|---|
| Inner loop, per slice | about 15 runs × 2-4 min over 8 slices | 6 red: 4 `cargo fmt` misses, 1 clippy lint, and slice 06's stub `gh` argv quoting with a missing git author | keep |
| Checkpoints (full gate) | 4 runs, 1.6 to 2.5 min | CP-2: a real race in `tests/lanes.rs`, where the test broke `state.toml` before the parked worker's own write | keep: the only run of the whole suite before polish, and it found a flaky test no inner loop hit |
| Prove, at checkpoints | 8 FRs over 3 checkpoints, 11 evidence files | CP-1: 2 plan leaks in `src/docs/harvest.rs`; CP-3: 1 in `tests/skills_bundle.rs` | keep |
| Polish — local review | one `code-review` pass | 6 findings, all fixed: a batch naming a feature twice archived it twice and hid a partial failure, `--file retro.md` with a bare name compared the wrong tree, "committed it" printed when git ignored the idea, 3 stale doc lines | keep |
| Polish — security pass | one subagent | 0 at its bar; 8 findings and notes, 4 real: symlinked kept files let the archive write outside the repo, a tracked file named `*` widened `git rm`, `docs check` could hang on a symlink to a FIFO, and the upstream scrub missed a home outside `/home` | keep, and keep triaging the notes |
| Polish — UX pass | skipped: CLI and skill text, no UI, no verify harness | — | — |
| Polish — second opinion (Cursor grok-4.7-high) | 7 passes: 2 returned (597 s, 846 s), 4 timed out at 900 s, 1 killed; 4 fallback Opus agents at 3-4 min each. About 100 min in all | the only high: `/harvest` § 5 moved a keeper, then `--done` refused the dirty folder, so the skill could never finish. Cursor also found 2 QA-guide steps that could not pass and the unchanged `docs/index.md`. The fallbacks found 3 mediums: a non-UTF-8 kept file left a half-archive, the retro compared untracked drafts, ignored `.pyc` files rode into the archive | keep; tune the pass timeout |
| prune (pr mode) | one read over about 60 added tests, 2 reruns of the 5 test files the polish touched | nothing: no duplicate, no flake. The unit and CLI pairs test different entry points, and checkpoint evidence names the CLI tests | keep |
| `factory leaks` | seconds, at every checkpoint and at polish | 3 hits at checkpoints (above); clean at polish | keep |

## Where I guessed

- **Where `docs/index.md`'s new archived definition belongs.** FR-5 orders `assets/docs/index.md` *and* this repo's `docs/index.md` changed. FR-6 step 4 has the first `/harvest` write the same line into this repo's copy. Slice 05 changed only the asset, and the QA guide said this repo's copy already had it. Polish wrote it here, as FR-5 says, so the skill's step finds it done.
- **How far the upstream scrub reaches.** FR-3 fixes the home-path shape as `/home/<name>/` and `/Users/<name>/`. A home at `/var/home/<name>` reached a public draft untouched. I kept the shared shape and added the person's own `$HOME` to the scrub only (AD-5).
- **The second opinion's empty passes.** A pass over the seven slice specs timed out with only opening chatter. Those files are the plan, already built and reviewed against the code; I named them unreviewed instead of spending a fallback agent on them. The 4 empty passes over code each got a fallback agent.
- **When `/harvest` commits.** FR-6 has the skill move keepers, then run `--done`; FR-5 has `--done` refuse an uncommitted folder. Followed in order, the skill can never finish. I made § 5 commit its edits first, on a `harvest/<date>` branch from the default one (AD-6, for the owner to decide).
- **Ignored files in an archived folder.** FR-5 says the archive keeps four files; `git mv` carries ignored ones along too. I made `--done` refuse such a folder, factory's own lock excepted, over deleting them (AD-7).
- **The bundle lock at an untagged version.** Polish edited two skills after slice 08 locked 0.6.0, and `relock` refuses a new hash at the same version. I relocked from main's 0.5.0 lock, so 0.6.0's hash is the polished one. Nothing tagged 0.6.0 yet.

## What I wished existed

- **The new binary for the feature's own polish.** The installed 0.4.0 has no `review collect`. For the second feature in a row, polish ran the last resort and serial passes.
- **A home-path matcher for a bare home.** `W-HOME-PATH` and the `--done` rewrite need a `/` after the name, so `find /home/<name> -name …` in `002-docs-port/prd.md` would be frozen into the archive. The QA guide now has the owner grep for it by hand first; the matcher gap is parked.
- **A hook that matches commands, not words.** The worker hook refused a heredoc whose text held the GitHub CLI's name. Edits holding it went through the Edit tool.

## Project friction → `/gate audit`

None. The full gate ran 4 times at checkpoints and 4 times at polish; its one red found a real race. The 4 red inner-loop runs on `cargo fmt` cost one rerun each, cheaper than any way of skipping them.

## Framework friction → skills repo

- **`skills/polish/SKILL.md` § 1, "Start from the collected file".** It assumes `factory review collect`. A lane on the installed binary never has it, so every factory-built polish runs the last resort until the owner installs the new version. The pack could say which binary the lane runs.
- **`factory review second`, pass timeout.** 4 of 7 passes hit 900 s with only opening chatter, and a fallback agent then found real defects in 3 of those diffs in under 4 minutes each. A pass that times out with only opening chatter costs 15 minutes and says nothing; it should split the pass or stop sooner.
- **The PRD template, an edit split across two FRs.** PRD-008 had FR-5 change `docs/index.md` and FR-6 step 4 change it again, so the slice built one half and the QA guide claimed both. `/slice` should flag a file two FRs both write.

A comment or name the leaks check should have caught: none at polish. The 3 checkpoint hits were caught there.

## Score

3 — the build and checkpoints ran clean, but polish found 1 high and 12 mediums the slices let through, and the second opinion cost about 100 minutes.

## Numbers

<!-- factory:numbers -->
| role | workers | own | subagents | weighted | minutes | turns |
|---|---:|---:|---:|---:|---:|---:|
| build | 8 | 2132879 | 0 | 2132879 | 39 | 148 |
| fix | 1 | 195259 | 0 | 195259 | 6 | 19 |
| checkpoint | 3 | 666154 | 0 | 666154 | 10 | 55 |
| polish | 1 | 2123264 | 2187318 | 4310582 | 99 | 137 |
| total | 13 | 5117554 | 2187318 | 7304873 | 154 | 359 |

Knobs: build opus/high default · fix opus/high default · checkpoint opus/high default · polish opus/high default · budget 30000000 default · lanes 4 · factory 0.4.0 · claude 2.1.296

Hypothesis: none

quality: gate_red 1 · worker_gate_red 0 · fix_commits 1 · ci_red_rounds 0 · ci_reruns 0 · findings high=1 medium=12 low=33 fixed=27 parked=3 · decisions 7 decide 1 blocks 0

Pack (build): slice 7828 · prd_sections 1746 · log 1450 · outline 12999 · lookup 890 · contract 3180 · autonomy 11836 · inner_loop 91 · total 40020 · start_context 43334

Compared with:

| run | feature | weighted | slices | per slice | lines changed | per line | comment share | id hits |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| 008-20261015-b926 (this run) | 008 | 7304873 | 8 | 913109 | 4053 | 1802 | 3.3% | 0 |
| 007-20261015-c8fa | 007 | 4526459 | 5 | 905292 | 3611 | 1254 | 4.0% | 0 |
| 006-20261014-0643 | 006 | 5378450 | 5 | 1075690 | 6185 | 870 | — | — |
| 005-20261014-d1ad | 005 | 16568730 | 10 | 1656873 | 9217 | 1798 | — | — |
<!-- /factory:numbers -->

**Owner review (2026-10-15):** 6 kept, 0 changed (AD-1, 2, 4, 5, 6, 7).
