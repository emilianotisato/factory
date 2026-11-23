# Retro — PRD-021

Written by `/deliver` in the closeout commit, from the agent's side of the run.

**The point is cost next to catch.** An agent asked only about friction votes to cut every check, because every check costs it time. A verdict with an empty Caught column is a preference, not evidence.

## Cost vs catch

| Step | Cost | Caught | Verdict |
|---|---|---|---|
| Inner loop, per slice | 6 runs × ~3 min (20 min) | slice-02's red: seven `tests/checkpoints.rs` tests depended on the whole-table fallback §5 removes. The fixture was fixed to name its row (AD-3) | keep |
| Checkpoints (full gate) | 2 runs × 2 min | nothing | keep: two green runs cost 4 min, and the gate is the only full-suite proof before polish |
| Prove, at checkpoints | 2 checkpoints, one worker each | CP-1 named the FR-2 half it could not reach yet instead of claiming it, and CP-2's mutation check went red as asked. Neither found a defect | keep |
| Polish — local review | one `code-review` pass at high | the restarted-driver bypass, the empty-reason block, the post-block record taken as the owner's, and three comments that overstated the code | keep |
| Polish — security pass | one subagent | agreed on the restarted-driver bypass. Its headline, "any worker can write a CP record", is the PRD's non-goal on dishonest workers. It confirmed the evidence paths and symlinks hold | keep. It earns its place on any diff that reads paths a worker writes |
| Polish — UX pass | skipped: no user surface | — | — |
| Polish — second opinion (Cursor grok-4.7-high) | 0: all five passes unreviewed on the vendor's usage limit | nothing. The fallback, three fresh Claude agents, found the `Human?` exact-match miss, four contract-wording gaps, a test-free freshness rule and the stale-evidence send-back | keep the fallback. The vendor leg needs a usage check before a run starts |
| prune | inline pass over the 24 tests the branch added | nothing to remove. The two stub-side assertions the reviewers flagged guard a precondition | keep |
| Polish mutation check of its own fixes | 2 runs of 4 tests | proved each new regression test goes red on the defect it names | keep |

## Where I guessed

- **What counts as `Human? = yes`.** PRD §4 says "a row marked `Human? = yes`" but never gives the cell's exact text. This repo's own tables write `**yes**: …` and `yes → …`. I chose a first-word match (AD-10).
- **Where the owner's record ends and a resumed worker's begins.** AD-6 kept everything after the block. Both block texts tell the owner to "add a record and answer", so I took the answer as the boundary (AD-8, `decide`). An owner who answers first and writes after now gets a re-run.
- **What a restarted driver does with a thin orphan record.** Neither the PRD nor slice 03/04 covers the orphan path, and the `scheduler.rs` code it lives in predates the feature. I chose: settle it `done` and let the checkpoint run again under the next number (AD-9).
- **The send-back's content.** §3 says "respawned once" and never says the respawn is told why. Parked as an idea rather than guessed.

## What I wished existed

- A way to read the board log path (`FACTORY_BOARD`) from the pack. The first `docs log add` wrote to the main checkout, and I had to confirm that was by design before going on.
- A stub step that writes a record and then dies. The orphan test had to kill the driver and hand-write the thin record to stand in for one.

## Project friction → `/gate audit`

None this run. The gate (fmt, clippy, test, the cockpit plugin's validate and test) ran in 108 s and was green each time. No tier cost more than it caught.

## Framework friction → factory

- **The push guard hook refuses a Bash command whose text holds `.push(`.** A Python heredoc editing Rust (`what.push(format!(…))`) was blocked as "workers never run gh or git push". The hook should match a `git push` or `gh` command word, not the substring.
- **`/polish` §1, the second opinion.** The collect spent a pass per file group only to learn the vendor's monthly limit was spent; all five failed in 3 s each. The PRD's own verification notes had already hit the same limit the day before. A `--list-models`-style usage probe before the split would turn this into one line, and the fallback could start at once.
- **`/polish` §7.** In a `factory` run, invoking the `prune` skill restates the whole full-mode workflow for a 24-test branch scope. A pr-mode-only entry would be cheaper.

The leaks check had nothing it should have caught. The one misplaced comment (on `records_seen`, explaining a filter about 100 lines below it) is a reader's question, not a rule hit.

## Score

4 — the feature landed whole and checkpoints stayed green. But the run's own checkpoints could not exercise the restart path, so polish was the step that found the bypass that skipped every new check. The second opinion never ran.
