---
score: 4
gate_runs: 29
gate_red: 6
gate_minutes: 16
ci_cycles: 0
slices: 13
checkpoints: 7
build_minutes: 627
polish_minutes: 157
out_tokens: 3772425
cache_read_tokens: 229975490
start_context: 55259
peak_context: 182520
---

# Retro — PRD-001

Written by `/deliver` in the closeout commit, from the agent's side of the run.

Gate numbers come from the `CP-1`..`CP-7` entries in `agent-log.md` (17 runs, 5 red, about 5 min) and from polish (12 runs, 1 red, about 11 min). The inner loop is counted separately below. Measured keys come from `measure.py --retro` over session `f2d0ea47`. The slicing ran earlier, in session `9b8e157d`, and is not in these numbers.

## Cost vs catch

| Step | Cost | Caught | Verdict |
|---|---|---|---|
| Inner loop, per slice (`cargo test`) | 83 runs, 46 red, about 27 min | Real bugs: zero-padded AD/BLOCK seeds (slice-03); the stub read a quoted Resolution as an answer (slice-04); an orphaned checkpoint worker got no Traps line (slice-06); the `scheduler::finish` event-vs-phase race (slice-10); the harness `Fixture::calls()` race (slice-08/09); the no-merge scan tripping on a deny literal (slice-13). Most other reds were the builder's own test expectations. | tune: add `cargo fmt --check` and clippy. The inner loop missed both, and CP-2 paid 4 red runs for it. Here the full gate takes about 35-60 s, so the inner loop can simply be the gate. |
| Checkpoints (full gate) | 17 runs, 5 red, about 5 min | CP-2: fmt on 11 files plus 7 clippy sites, which builders had not run. CP-5: its own fmt. No application defect. | keep: about 20 s a run; the fmt/clippy catch moves to the inner loop. |
| Prove, at checkpoints | real Haiku workers twice (about $0.12), a clean checkout (12.7 s), `ctx` timing on two owner repos | CP-6: `def` signature on PHP attributes (AD-9003). CP-7: the hook hint and pack name a bare `factory`, which exits 127 from an uninstalled build (K1). | keep: both were real and invisible to the stub harness. |
| Polish — local review | 2 opus agents, about 24 agent-min | 24 findings and 18 notes. 2 HIGH: `factory answer` bound a BLOCK id to the wrong feature, and the death cap counted the whole run. Also: a resume never checked the branch, so commits could land on `main` (L12); a bare `git commit` swept the whole index (L3). | keep |
| Polish — security pass | 1 opus agent, 8 min | S1, the most important finding of the run. Workers could `gh pr ready/close/merge` through REST or GraphQL with the owner's own `gh` credentials, because the deny list covered only `gh pr merge*`. | keep |
| Polish — UX pass | skipped | no screens, and no `verify-*` harness | n/a |
| Polish — second opinion (Cursor `grok-4.7-high`) | 9 passes; 3 hit the 900 s timeout (about 45 min lost) and were re-scoped | About 55 claims, about 40 beyond the locals. Highlights: the CI watch read a stale suite after a fix push; `timed_out` and `startup_failure` were treated as fix rounds; answers could not be retried; a guessed quota pause outlived a reported reset. Independently agreed with security on the missing default-branch push guard. | tune: every pass that timed out covered 1000+ diff lines across two modules. Cap a pass at about 600 lines or one module, up front. |
| Polish — fixes (5 fixers, 3 in detached worktrees) | 12 gate runs, 1 red, about 11 min | The merge gate caught one interaction: the driver-core branch guard parked silently before the closeout's push guard could block. Each fixer was green alone. | keep: detached worktrees with their own `CARGO_TARGET_DIR` made 4 parallel fixers safe under `lanes: 1`. |
| test-cleanup (pr mode) | 3 gate runs, 3 min | Removed 2 duplicate unit tests and consolidated 1 (30 hook cases moved into the `tests/hook.rs` tables); 256 → 253 tests | keep |
| Builders pinned to `fable` | slices 01-09 and CP-1..4 ran on Fable after the session had moved to Sonnet, then Opus | nothing: it only burned the owner's Fable weekly quota | cut: fixed for this owner by a memory entry; see Framework friction. |

## Where I guessed

- **PRD FR-1 vs `/deliver` (BLOCK-201).** "`judge[]` parks the run" parked every run, the fixture's included, which contradicts an unattended delivery. I blocked. The owner answered "Deliver should be unattended", and FR-1/FR-7 were corrected inline.
- **`polish` § 5 with `lanes: 1`.** It says no test suite runs in a fixer. The suite isolates every fixture with `FACTORY_DATA_DIR`, so I gave each fixer a detached worktree and its own target dir, and let it run the full gate. I guessed that a worktree counts as isolation.
- **AD grading (`autonomy.md` § The test).** "A behaviour chosen where the slice was silent" (fyi) and "a public interface shape the PRD did not specify" (decide) overlap for a CLI, where every flag and exit code is public. I regraded AD-604, AD-607 and AD-1101 up to `decide`. 31 of 145 are `decide`, which is still more than the owner can comfortably review.
- **Fragment shape.** The builder contract and the polish fixer prompt never say a fragment must start with a `## ` heading. `docs log merge` refused a title line at CP-4, and again refused two fixer fragments' prose preambles at the polish merge. Both times I stripped the lines by hand.

## What I wished existed

- `docs log merge` that tolerates a `#` title line and prose above the first section, or a fragment template the contracts point to.
- A heartbeat in the Cursor stream-json for "still reading" vs "stuck", so a 900 s timeout is not the first signal.
- `measure.py` finding polish's end without depending on the literal `docs.mjs ship` string in the transcript.

## Project friction → `/guardrails audit`

- **The inner loop is `cargo test` only.** CP-2's 4 red runs were all fmt and clippy, which no builder had run. From slice-05 on, builders ran fmt and clippy by hand, a Trap, and every later checkpoint was green on the first run. Declare the inner loop as the full gate, since it takes under a minute here.
- **`gate_parallel` was never measured** (a preflight `judge` line). With `lanes: 1` it cost nothing this run. It matters before the first `lanes: 2` run.

## Framework friction → skills repo

- **`feature-docs/models.md` § The four roles, and `deliver` § 2.** "`fable` when it is the session model" was read once at preflight, and the pin outlived the session model. It should be re-checked at every spawn. The owner noticed it in the quota meter.
- **`deliver` § 2 contracts and `polish` § 5.** Say "your fragment starts with `## Decisions`", or have `docs log merge` accept a title.
- **`polish` § 1 and `models.md` § Second opinion.** "~30 files or ~2000 changed lines" is too loose for `grok-4.7-high` at 900 s. Three passes of 1000-1400 lines timed out with nothing. Use about 600 lines or one module per pass.
- **`polish` § 5.** Write down the worktree recipe for parallel fixers when `lanes: 1` (detached HEAD, own target dir, cherry-pick, then one merge gate). The merge gate is not optional: it caught a red that each fixer's own green gate could not see.

## Score

4. The run built 13 slices and got through polish with one owner answer. The review layers caught about 100 real findings that seven green checkpoints had not, including a worker sandbox hole. Lost: about 45 min of Cursor timeouts, and a model pin that burned the wrong quota.
