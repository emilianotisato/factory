# Retro — PRD-031

Written by `/deliver` in the closeout commit, from the agent's side of the run.

**The point is cost next to catch.** An agent asked only about friction votes to cut every check, because every check costs it time. A verdict with an empty Caught column is a preference, not evidence.

## Cost vs catch

| Step | Cost | Caught | Verdict |
|---|---|---|---|
| Inner loop, per slice | 4 slices × ~1 run × ~3 min | lane `CARGO_TARGET_DIR` resolution vs assert (CP-1 mutation) | keep |
| Checkpoints (full gate) | 2 × ~2.5 min | stale `reviewed_head` assertion (CP-2 mutation) | keep |
| Prove, at checkpoints | 2 checkpoints | nothing beyond gate/tests already green | keep |
| Polish — local review | 1 pass (in-worker) | polish/fix pack lane-env gaps aligned with FR-1 acceptance | tune |
| Polish — security pass | inlined on shell/lane path | same as local (process group, read cap) | keep |
| Polish — UX pass | skipped — CLI/cockpit unchanged this round | n/a | cut |
| Polish — second opinion (claude:opus) | 5 passes, ~12 min collected pre-spawn | unprefixed fix inner loop, missing polish `# Lane env`, shell probe tty/hang holes, setup `env_for` drift | keep |
| prune | not run — no new redundant tests | nothing | cut |

**A "cut" or "tune" names what the step failed to catch.** A step that caught a real defect is never "cut" on cost alone — say "tune", and how.

## Where I guessed

- **PRD §3 vs AD-1:** Slice 03 logged `tail + 1` for deferral scanning; second opinion argued the integration test still sat at the boundary. I left behavior as implemented and did not widen the fixture mid-polish — the deferral scenario needs a slice numbered past `max(covers)+1` with post-Depends FR cites.
- **`settled::last_reviewed` widening:** Second opinion flagged manual `reviewed_head=` clears advancing `collect --since`. PRD §7 explicitly chose the widen; I treated manual clear after a rebase as acceptable tradeoff and did not revert the regex.

## What I wished existed

- A pack contract test matrix keyed by `Pack::for_*` variant so "all worker packs" in the decisions log cannot miss polish/fix again.
- `handed_qa` deferral fixtures that name which slice number exercises tail+1 vs Depends-on cutoff without re-reading `pack.rs` every time.

## Project friction → `/gate audit`

- Full `cargo test` on every inner loop is the right bar for this crate, but it dominates polish fix turns (~3+ min per iteration). No cheaper tier failed silently here; tune would be optional targeted test filters in worker contracts, not a weaker gate.

## Framework friction → factory

- **`/polish` §1 + FR-1 acceptance:** "all worker packs" is easy to read as slice/checkpoint only; polish was spawned with `env_for` on the process but the pack omitted `# Lane env` until second opinion. `/lanes` skill could list pack kinds explicitly.
- **`/slice` Prove templates:** checkpoint pack has no `# Inner loop`; reviewers assumed prefixing was proven at CP-1 while fix pack stayed raw — name which packs must prefix in the slice acceptance table.
- **Leaks:** clean on this branch — no comment to add.

## Score

4/5 — feature delivery was solid; second opinion paid for itself on pack/shell gaps slice tests did not fully lock, but deferral integration and `collect --since` after manual `reviewed_head` clear remain known edges.

## Numbers

<!-- factory:numbers -->
| role | workers | own | subagents | weighted | minutes | turns |
|---|---:|---:|---:|---:|---:|---:|
| build | 4 | 3791260 | — | 3791260 | 165 | — |
| checkpoint | 2 | 800909 | — | 800909 | 12 | — |
| polish | 1 | 636232 | — | 636232 | 6 | — |
| total | 7 | 5228400 | — | 5228400 | 183 | — |

Knobs: build cursor:composer-2.5 user/default · fix cursor:composer-2.5 user/default · checkpoint cursor:composer-2.5 user/default · polish cursor:composer-2.5 user/default · budget 30000000 default · lanes 4 · factory 0.19.0-edge.2 · cursor 2026.11.22-e9042b2

Hypothesis: none

quality: gate_red 0 · worker_gate_red 0 · fix_commits 0 · ci_red_rounds 0 · ci_reruns 0 · findings high=0 medium=6 low=7 fixed=7 parked=4 · decisions 2 decide 0 blocks 1

Pack (build): slice 10002 · prd_sections 6692 · log 873 · outline 9127 · lookup 1115 · contract 3685 · autonomy 10282 · inner_loop 86 · total 41861 · start_context —

Compared with:

| run | feature | weighted | slices | per slice | lines changed | per line | comment share | id hits |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| 031-20261127-a312 (this run) | 031 | 5228400 | 4 | 1307100 | 2748 | 1903 | 0.0% | 0 |
| 030-20261127-4dbf | 030 | 9266309 | 5 | 1853262 | 3582 | 2587 | 0.5% | 0 |
| 029-20261127-4c0a | 029 | 4454535 | 4 | 1113634 | 2815 | 1582 | 1.0% | 2 |
| 016-20261127-dca0 | 016 | 3399075 | 4 | 849769 | 2670 | 1273 | 0.4% | 0 |
<!-- /factory:numbers -->

**Owner review (2026-11-28):** 0 kept, 2 changed (AD-1, AD-2)
