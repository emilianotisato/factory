# Retro — PRD-023

Written by `/deliver` in the closeout commit, from the agent's side of the run.

**The point is cost next to catch.** An agent asked only about friction votes to cut every check, because every check costs it time. A verdict with an empty Caught column is a preference, not evidence.

## Cost vs catch

| Step | Cost | Caught | Verdict |
|---|---|---|---|
| Inner loop, per slice | not in the run's events; each slice reported it green before its commit | nothing the events can show | keep |
| Checkpoints (full gate) | 1 run × 1.4 min | nothing: green first time | keep. One checkpoint for two slices is the floor already |
| Prove, at checkpoints | 5 rows, driven with the built binary over fixture and stubbed runs | nothing new: every row matched its test | keep. The FR-3 row ran the real `answer` argv, which no unit test does |
| Polish — local review | inline, one read of the source diff and the tests | nothing on its own that the other reviewers missed; it confirmed each second-opinion claim at `file:line` | keep |
| Polish — security pass | 1 subagent, about 1 min | a block id read from the repo's `log.jsonl` reaching the `answer` argv, where `--run=X-1` parses as a flag (fixed, AD-3); symlinked `prd.md` to a FIFO stalling the poll (parked) | keep. Neither other reviewer looked through that lens |
| Polish — UX pass | skipped | — | the surface is JSON read by a program; the one human line is the `--recent` help text |
| Polish — second opinion (grok-4.7-high) | 3 passes, 553 + 1076 + 824 s | `ci` stuck on `"pending"` for an ended run, raised by two passes independently (fixed, AD-2); `feature_folder` falling through to the archive (fixed); four weak or wrong-for-the-reason test asserts (fixed); question text being the block title and `seq` missing state-only changes (parked) | keep. It found every code defect this polish fixed except the security one |
| prune | scoped tests run 3 times, about 20 s | nothing: no duplicate or scaffolding test; every added test pins a distinct path | keep |
| Full gate after fixes | 2 runs × about 1.5 min | nothing: green both times | keep |

## Where I guessed

- **`docs/features/023-structured-status/prd.md` §4 FR-2, the `ci` rule.** It says a `CiRerun` or `CiFixCommitted` after the last verdict is `"pending"`, with no live/over qualifier, while the same bullet makes an over run with no verdict `"none"` because `"pending"` "would never clear". I took the reason over the letter (AD-2, marked `decide`).
- **FR-3, "one per open id".** Silent on an id that `factory answer` cannot parse as one. I left such ids out of `questions` rather than change the argv shape with `--` (AD-3).
- **Where `factory docs log add` writes from a lane.** From this lane worktree, AD-2 and AD-3 landed in the main checkout's `docs/features/023-structured-status/log.jsonl` (`~/Projects/factory`), not the lane's. I assumed that is the driver's design for the store and wrote nothing by hand.

## What I wished existed

- A per-slice inner-loop event (`inner_run` with red and minutes). The Cost column for the inner loop has no source in the run's own events, so this retro cannot say what it cost or caught.
- A note in the pack saying which checkout `docs log add` and `docs set` write to when run from a lane, so a worker checking that its entry landed does not go looking.

## Project friction → `/gate audit`

None with evidence. The gate ran green three times at about 1.5 min each, and the two `claude plugin` tiers are cheap; the PRD itself notes `claude plugin test mods/cockpit` cannot catch a status JSON change because it feeds hand-written JSON, so that tier proves the mod, not this feature.

## Framework friction → factory

- **`skills/polish/SKILL.md` § 1, "Local review first … Invoke the `code-review` skill".** On a diff this size, with the second opinion already collected, I ran the local review inline rather than through the skill; the skill's own multi-agent cost would have repeated the read. A size row for the local review, like the one the second opinion has, would make this a rule rather than a judgment.
- **`skills/polish/SKILL.md` § 1, the security trigger "a read … through a path the repo itself controls".** It fired on a title read whose only output is the local user's stdout, and the pass still found a real argv injection elsewhere in the diff, so the trigger earned its cost here.
- **Leaks.** Nothing it should have caught: the branch's added comments name behaviour, not the plan.

## Score

4 — the build and checkpoint were clean and the reviewers found real defects cheaply, but the PRD's own `ci` rule contradicted its reason and the inner-loop cost is invisible to the retro.
