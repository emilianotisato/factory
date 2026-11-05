# Retro — PRD-014

Written by `/deliver` in the closeout commit, from the agent's side of the run.

**The point is cost next to catch.** An agent asked only about friction votes to cut every check, because every check costs it time. A verdict with an empty Caught column is a preference, not evidence.

## Cost vs catch

| Step | Cost | Caught | Verdict |
|---|---|---|---|
| Inner loop, per slice | 2 runs × ~1 min in the build, plus 1 run (fmt, clippy, 688 tests) after the polish fixes | The red run caught `three_fix_rounds_on_a_red_check_block_the_closeout`, which pinned the old subject list. That was a change the PRD accepted, not a defect. | keep |
| Checkpoints (full gate) | 1 run × 1.4 min | nothing | keep: one slice, one cheap run |
| Prove, at checkpoints | `cargo test --test closeout`, 37 tests, under a minute | nothing new. It added the `ci_green.head == refresh_sha` assertion the row claimed and no test checked. | keep |
| Polish — local review | one forked `code-review` at medium | **The one real defect:** a refresh written but not committed (driver killed in between) is never committed on resume, because `retro::write` returns `Ok(false)`. Also a doc comment that overstated "never failing the push", and a scope gap (store row and lines/leaks figures stay at ship), which is parked. | keep |
| Polish — security pass | 1 agent, ~33 s, ~50k tokens | nothing at the bar. Four notes, all already true of the ship step's write. | tune: it ran only because the trigger list counts any write to a repo path |
| Polish — UX pass | skipped | — (no screen; the only surface is one driver-log line) | — |
| Polish — second opinion (grok-4.7-high) | 1 pass, 957 s, 252 lines, of which ~150 were the feature's own docs | The same retry defect, found independently: cross-vendor agreement. Two other claims rebutted. | keep, tune the scope (see Framework friction) |
| prune | 2 scoped re-runs of 4 tests, ~10 s | nothing to remove. All four tests the branch touched are KEEP. | keep |

## Where I guessed

- **`skills/polish/SKILL.md` § 1, UX trigger.** The rule is "Anything user-visible gets the UX pass". The only user-visible change is a new driver-log line, `push: no ## Numbers written — …`. I treated a CLI log line as not a screen and skipped the pass, since there is no UI harness for the driver log.
- **`skills/polish/SKILL.md` § 1, security trigger.** The trigger is "a read, write or delete through a path the repo itself controls". Read literally, it fires on every file the driver writes. I ran the pass, and it found nothing the ship step's identical write did not already have.
- **The contract's `FINDINGS` line.** It does not say whether a security pass's below-the-bar notes count. I counted the one note with a concrete scenario (the symlinked retro) as low and rebutted, and left out the three that only confirmed safety.
- **`factory docs set 014 reviewed=…` from the lane** wrote `reviewed` into the main checkout's `state.toml` (`/home/emiliano/Projects/factory`), not the lane's. The lane's `factory docs check` still warns `W-NOT-REVIEWED`. I assumed this is the driver's design, since state is the tool's to write, and left it alone.
- **The retry fix has no end-to-end regression test.** It needs a driver killed between the retro write and its commit, and the closeout fixture cannot stop there. I extended the kill-and-resume test to pin the resume road instead, and noted the gap in the slice.

## What I wished existed

A fault-injection point in the closeout tests: an env var that makes the driver exit at a named step, such as "after the Numbers write, before its commit". The kill-and-resume tests can only kill while a worker runs. So every "killed between two driver steps" fix, a class this file already guards against in three places, ships without a test that would go red without it.

## Project friction → `/gate audit`

None this run. The full gate's `claude plugin validate/test mods/cockpit` tiers do not touch this diff, but they cost seconds.

## Framework friction → factory

- **`factory review collect`, the second-opinion passes.** The pass sent the feature's own `prd.md`, slice, `state.toml` and `log.jsonl` with the code: about 150 of 252 lines. It ran 957 s. Every finding it made was in `src/closeout.rs`. Scoping the second opinion to non-`docs/features/` paths by default would roughly halve the input on a small bugfix.
- **`skills/polish/SKILL.md` § 1, the security trigger list.** "A read, write or delete through a path the repo itself controls" catches every driver write to `docs/`. Narrowing it to a path *built from input* or a *new* write target would have skipped a pass that could find nothing new here.

Leaks: clean. No comment or name it should have caught.

## Score

4: one small FR, built and proven in one slice with a single red inner-loop run. Polish found one real defect, which two vendors agreed on independently. The fix's own regression test could not be written with today's fixture.
