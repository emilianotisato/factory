# Retro — PRD-012

Written by `/deliver` in the closeout commit, from the agent's side of the run.

**The point is cost next to catch.** An agent asked only about friction votes to cut every check, because every check costs it time. A verdict with an empty Caught column is a preference, not evidence.

## Cost vs catch

| Step | Cost | Caught | Verdict |
|---|---|---|---|
| Inner loop, per slice | about 2 min a run | nothing red during the slices | keep |
| Checkpoints (full gate) | 1 run, 1.9 min | nothing | keep |
| Prove, at checkpoints | scratch-repo transcripts for FR-2's preflight half, FR-6 and FR-7; a read-through of the skill passages for the rest | the leaks hit at `tests/preflight.rs:186` (a literal plan id in the expected refusal) | keep |
| Polish — local review | one forked `code-review` at medium | 10 findings, 6 real: the hand-edit restore depends on lines `skills update` never prints on `ahead`; `/deliver` commits on the default branch; `/prd`'s branch rule came late with a false reason; a skill keeps following its stale text after updating itself; a test name claiming more than it checks; a duplicated trim | keep |
| Polish — security pass | skipped | — (no auth, outside input or built paths; one `git config` read) | keep the skip rule |
| Polish — UX pass | skipped | — (no UI) | keep the skip rule |
| Polish — second opinion (grok-4.7-high) | 2 passes, 926 s + 578 s | 10 claims, 7 real. 3 overlapped the local review (the restore bullet, the default-branch commits, the late `/prd` sentence). 4 only it found: the shipped-resume refusal, the title/author substitution, the untracked-docs pathspec commit, three skill-text tests that pass after the rule they guard is deleted | keep |
| prune | 3 scoped runs of 4 test files | nothing to cut: every new test pins a distinct behaviour | keep |
| Relock after a polish skill edit | one extra scoped `cargo test` | — | tune: see Framework friction |

## Where I guessed

- **`src/skills.rs` `relock` against polish.** The slice plan said "the one bump to 0.11.0". Polish changed skill text, and `relock` refuses a changed bundle under an unchanged version. I restored the lock to the default branch's copy and relocked under the unreleased 0.11.0 instead of bumping to 0.11.1 (AD-1). Nothing in `/polish` or the Autonomy contract says which one a polish pass should do.
- **`/deliver` §1 on the default branch.** The PRD has `/prd` commit on `feat/{slug}` and accepts a chore commit "in a feature PR", but it does not say where `/deliver`'s own commits land before `ensure_branch` runs. I had the cockpit switch first (AD-2).
- **Second opinion's stale-tooling bullets.** FR-4 says to run the `ahead` hint and commit "on the current branch" without asking. Both reviewers pushed on that. I treated it as settled by FR-4's decision and rebutted it.

## What I wished existed

- A `factory skills relock --from <base>` verb, or `relock` comparing against the base branch's lock rather than the working one, so a branch's skill edits are one bundle change until it is tagged.
- A cheap check that a skill-text test's needle is unique in its file. Three tests passed after the rule they guard was deleted, because another line held the same words.

## Project friction → `/gate audit`

None with evidence. The inner loop and the gate cost about 2 minutes each and stayed green. Nothing went red that the gate should have caught earlier.

## Framework friction → factory

- **`/polish` § 5 Fix, with `src/skills.rs` `relock`.** A polish fix to any `skills/` file trips the lock, and neither the skill nor the contract says whether to bump again or relock against the base. Say it in `/polish` § 5: relock from the base branch's lock while the version is untagged.
- **`/slice` and `/prd` skill-text tests.** Needle tests on skill prose are the only proof the gate can give for FR-1, FR-3, FR-4 and FR-5, and they were the weakest tests in the diff. `/slice` could ask each skill-text test to quote the passage, not a word the passage shares with others.

A comment or name the leaks check should have caught: `tests/skills_bundle.rs:227` `fn the_prd_skill_offers_the_build_and_never_says_clear()`. It names a claim the test did not check. That is not a plan leak, so the rule table is right to miss it; the local review caught it.

## Score

4 — the build was clean and the gate never went red. Both reviewers earned their cost: six real skill-flow defects were fixed before the owner's first live run, two of which would have put commits on the default branch.
