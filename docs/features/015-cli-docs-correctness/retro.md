# Retro — PRD-015

Written by `/deliver` in the closeout commit, from the agent's side of the run.

**The point is cost next to catch.** An agent asked only about friction votes to cut every check, because every check costs it time. A verdict with an empty Caught column is a preference, not evidence.

## Cost vs catch

| Step | Cost | Caught | Verdict |
|---|---|---|---|
| Inner loop, per slice | 7 runs, about 12 min over 5 slices | 1 red: clippy on a `log_rules` parameter left unused (slice-01). A clippy pass also caught `type_complexity` in slice-05. Both were lint, neither a behaviour defect | keep |
| Checkpoints (full gate) | 2 runs, about 3 min | nothing | keep: cheap, and it is the only run of the `claude plugin` tiers |
| Prove, at checkpoints | 8 rows, test transcripts | nothing new: every row was a test the slice already ran | tune: on a CLI-only PRD whose rows are all `run <test>`, Prove repeats the gate. Fold such rows into the checkpoint's gate transcript |
| Polish — local review | 1 forked `code-review` run | 3 real defects: lane `check` naming main's board files by lane paths, `~~~` fences rebased, images not rebased. Plus the stale `check_lock` advice. 1 false positive (subject-prefix collision across CI rounds) | keep |
| Polish — security pass | 1 subagent | `renumber` still wrote through a linked `prd.md`, which the FR-8 acceptance claims it cannot do. The no-trim hard cut. `board_dir` following links (as a note) | keep: it found the one escape the symlink slice missed |
| Polish — UX pass | skipped: CLI only, no harness, no UI | — | — |
| Polish — second opinion (Cursor grok-4.7-high) | 3 passes, 4 s each, all unreviewed: quota out until 2026-11-24 | nothing. The three fresh-Opus fallback passes found: the lane-path labels again (the third reviewer to do so), `~~~` fences, the eager tag probe, test git config leaking (`tag.gpgSign`), no tests for AD-1's rules, and wrong file paths in slice-01 | keep the fallback. Cursor itself is unmeasured this run |
| prune | read only, no extra test run | nothing: 12 branch tests, 10 named by the QA plan, no duplicates | keep: cheap at this size |

## Where I guessed

- **AD-1 against FR-5 and check.** AD-1 left images unrebased "so `docs check` stays green". But `check`'s `hrefs` resolves images too, so that choice produced the E-LINK-BROKEN that FR-5 removes. I rebased images and logged AD-5, superseding AD-1's image clause (`fyi`).
- **`relock`'s shape.** PRD §5 says "the caller passes whether the version is tagged". I made that a closure, so the network probe runs only when it decides something (AD-6). Read strictly, the PRD wanted a value.
- **FR-2's text against the code.** prd.md §4 says the helpers "gain an optional body, passed as a second `-m`". The code builds the body into the one `-m`, as §4's own "Today" paragraph describes. The PRD's FR text is not mine to edit, so the conflict is noted in the PR body.
- **The `summary` overclaims.** It says "every docs verb" uses main's board files in a lane, but `ship`, `archive`, `renumber`, `new` and `slice` still use the lane. §3 lists the exceptions. Polish in a factory run skips closeout, so I left the summary alone.

## What I wished existed

- A way for `factory review second` to fall back to another model when Cursor's quota is out. It failed in 4 s per pass, and the fallback took three hand-briefed Opus agents. The settled block had to be dug out of the pass's `.jsonl` by script.
- `factory review collect` writing the settled block to its own file, so a fallback brief can point at it.

## Project friction → `/gate audit`

None measured. The gate cost about 2 min per run and was never red at a checkpoint. Its `claude plugin` tiers are declared but could not be checked ("cannot check \"claude\"").

## Framework friction → factory

- **`skills/polish/SKILL.md` § 1, the fallback.** "One fresh agent per pass" doubled the review on pass 1, which was 579 lines of planning docs. That pass still found real doc errors (wrong paths), but a docs-only pass could go to a cheaper model.
- **`src/cli/docs.rs`'s `board_root`.** The verb list is the whole lane rule, and it is easy to miss that `ship` and `archive` stay on the lane. `docs check` in a lane now mixes two trees (PRD §3's accepted cost). This is the kind of split that produced FR-1 in the first place.
- **The installed binary (0.13.4) showed FR-1's bug live in this very polish.** `docs set 015 reviewed=` wrote main's `state.toml` but printed the lane path, and `docs check` still warned `W-NOT-REVIEWED`. The branch's own binary (`cargo run -- docs check`) read main's file and showed `W-READY-TO-SHIP`. Until this ships and is installed, a lane worker on this repo still sees the old split.
- Leaks: no comment or name it should have caught. Every added comment and name checked clean in the local review.

## Score

4/5: every FR landed with its test. Polish fixed 11 real defects, 4 of them medium, so the slices' own review missed edge cases. The second opinion's vendor was down for the whole run.
