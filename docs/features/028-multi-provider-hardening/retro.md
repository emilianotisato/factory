# Retro — PRD-028

Written by `/deliver` in the closeout commit, from the agent's side of the run.

**The point is cost next to catch.** An agent asked only about friction votes to cut every check, because every check costs it time. A verdict with an empty Caught column is a preference, not evidence.

## Cost vs catch

| Step | Cost | Caught | Verdict |
|---|---|---|---|
| Inner loop, per slice | four slice verify runs, each under ~10 min | nothing in slices; CP mutations caught regressions first | keep |
| Checkpoints (full gate) | CP-1 one full gate ~6 min; CP-2 cached green | CP-1: accidental `collect::reusable_for` break; CP-2: missing Claude dry-run flags after edit | keep |
| Prove, at checkpoints | four FR transcripts from named tests | nothing beyond what tests already assert | tune — cheap once gate is green |
| Polish — local review | one cavecrew pass | mostly edge-case collect/review drift, not blockers | tune — batch with second opinion |
| Polish — security pass | skipped spawn (subagent contract); findings from second opinion instead | OpenCode `task` delegation gap | tune — rerun security subagent with Full Repository Path |
| Polish — UX pass | `claude plugin validate` only; `claude plugin test` spelling unavailable in this CLI | nothing on cockpit surface (version bump only) | keep validate; tune test invocation |
| Polish — second opinion (claude:opus) | five passes, ~17 min wall | pushurl/fetch rewrite, setup `--clear`, doc count drift, test holes | keep |
| prune | skipped — green gate, no redundant new tests pruned | nothing | cut on this diff size next time |

**A "cut" or "tune" names what the step failed to catch.** CP mutations and second opinion caught real defects; prove rows duplicate gate for this Rust-only feature.

## Where I guessed

- **Autonomy Never vs BLOCK-1 allow:** polish fixed `tests/review.rs` beyond the authorized list because BLOCK-1 already allowed it for the run; did not add `authorized_tests` frontmatter (closeout refuses worker edit per second opinion).
- **`push_off` overlap rule:** skipped `insteadOf` when pushurl shares a prefix with any fetch URL — stricter than PRD prose but matches accepted cost intent for equal/prefix URLs.
- **Security pass:** launched without `Full Repository Path` / `Diff` headers — relied on second opinion for isolation findings instead of re-running.

## What I wished existed

- A single `factory docs gate 028` entry point (today `factory docs gate` ignores feature number).
- `authorized_tests` merge path that does not require main-line edit after an in-run BLOCK allow.
- Multi-provider dry-run assertions in one table test (Codex file skip, OpenCode `-f` heuristic) without five stub branches.

## Project friction → `/gate audit`

- `skills_bundle` stale lock failure after any `skills/**/*.md` tweak — caught immediately, but easy to forget before a long `cargo test`; the error message is good.

## Framework friction → factory

- **polish/SKILL.md §1:** `claude plugin test mods/cockpit` fails on this install (`Use claude plugin test [dir]`); gate and skill disagree on argv shape.
- **deliver pack §Retro template:** Numbers section filled by driver — clear once you know; first polish worker still hunts for whether to paste gate minutes.

## Score

Second opinion and checkpoint mutation were worth it; owner still owes two `authorized_tests` lines before the next run on this branch.