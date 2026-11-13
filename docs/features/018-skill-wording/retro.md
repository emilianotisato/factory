# Retro — PRD-018

Written by `/deliver` in the closeout commit, from the agent's side of the run.

**The point is cost next to catch.** An agent asked only about friction votes to cut every check, because every check costs it time. A verdict with an empty Caught column is a preference, not evidence.

## Cost vs catch

| Step | Cost | Caught | Verdict |
|---|---|---|---|
| Inner loop, per slice | 8 runs × ~14 min across four slices, plus 1 run (~2 min) in polish | slice 01: two compile errors from `src/pack.rs`'s `#[path]` resolving under `src/pack/tests/`, and 13 needles that occurred twice, which is the defect FR-11 exists to catch | keep |
| Checkpoints (full gate) | 2 runs × ~2 min | nothing | keep: cheap, and the gate holds the cockpit mod's plugin tests that the inner loop skips |
| Prove, at checkpoints | 10 evidence files over 2 checkpoints | nothing. Every row is a skill-text test the gate already runs. It also **missed** three rows the slices named: FR-2, FR-4 and FR-6 were never driven, and CP-1's record still says "all run green" | tune: a checkpoint's Prove line should list every row it was handed, so a dropped row shows. A `run` row that names a test the gate already ran could cite the gate transcript |
| Polish — local review | 1 `code-review` (medium) fork, ~5 min | 10 findings, 0 high. Three it shared with the second opinion became fixes: polish §1's contradictory "only", the duplicate `/slice` bullet, and harvest's uncovered branch date | keep |
| Polish — security pass | skipped | n/a: skill text and tests, no input handling | keep the skip rule |
| Polish — UX pass | skipped | n/a: no UI, only agent-read text | keep the skip rule |
| Polish — second opinion (Cursor `grok-4.7-high`) | 3 passes, 3 s each, all unreviewed. The fallback was 3 fresh Opus agents, ~2-5 min each | Cursor: nothing. Fallback: the one **high** (a builder told to "revert" can wipe its uncommitted work with git, and the stash stack is shared across lanes), two mediums (deliberate reds inflating the PR body's inner-loop count; the May-list precedence binding only the builder), and five test-needle gaps | keep the fallback. Cursor itself: tune (see Framework friction) |
| prune | one read pass, 0 test runs (polish's green gate replaced the 3 repeat runs) | nothing pruned. One test split (the `/slice` half of break-it-once) and three section helpers folded during the fix step | keep |
| Bundle relock | ~1 min per skill edit after the bump | nothing. It exists only because of PRD-015 C-3's relock trip | tune, in PRD-015 |

## Where I guessed

- **`docs/features/018-skill-wording/prd.md` §4 (FR-2):** the rule says "every skill that asks an agent to fill a date", but the list names five skills and misses prune's and harvest's branch dates. I followed the rule over the list (AD-1).
- **`prd.md` §8 (FR-6), precedence:** it names only the builder's contract as winning over the new May items, yet the checkpoint, fix and CI-fix packs read the same text. I extended it to every worker's contract (AD-2).
- **`prd.md` §9 (FR-7):** it keeps the deliberate reds out of the `INNER` line only, while FR-4's PR-body clause sums the Progress lines. I kept them out of Progress too (AD-3).
- **`skills/polish/SKILL.md` §5, "Run the gate":** `factory docs gate` only prints the command, and `--run` does not exist. I ran the printed command by hand.
- **The contract vs. polish §0:** a done slice without Prove evidence is a finding, but the contract lets polish write only the PR body in the run dir. So FR-2, FR-4 and FR-6 ran green without a kept transcript, and they go to the owner instead.

## What I wished existed

- A `factory docs gate --run` that runs the gate and records the run, so polish's own gate count lands in the same store as the checkpoints'.
- A collect that says *why* every pass came back unreviewed in 3 s (auth, quota, model), rather than leaving that to be guessed from the PRD's note that Cursor's quota is out until 2026-11-24.

## Project friction → `/gate audit`

None measured. The full gate ran 3 times in ~2 min each, all green. The bundle relock after a second skill edit is PRD-015's, not a gate tier.

## Framework friction → factory

- **`assets/contract-checkpoint.md` step 2, Prove:** CP-1 drove 7 of the 9 rows it covered, and CP-2 3 of 4. Each record still read "all run green". "A row you could not reach is reported, not skipped silently" did not stop a row from being *forgotten*. The record's `**Prove:**` line should enumerate the rows it was handed, and the driver could compare them with the QA rows in the pack.
- **`skills/polish/SKILL.md` §1, second opinion:** for the second feature running, Cursor's three passes came back unreviewed in seconds, and the fallback cost three Opus agents. Those agents found the run's only high, so the fallback earned its cost. The collect should still report the cause.
- **`assets/contract-slice.md` and `skills/slice/SKILL.md`, as written by this PRD:** "(a revert or a stub)" reached two green checkpoints with a git-destroying reading. Text that tells an agent to undo something should say *how*.

Leaks: nothing the check should have caught. The verdict was clean before and after the fixes.

## Score

3/5. Every FR landed and every gate was green. But two checkpoints silently dropped three Prove rows, and the run's only high, in contract text, got past both checkpoints and surfaced only through the fallback second opinion.
