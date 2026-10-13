---
score:
gate_runs: 5
gate_red: 2
gate_minutes: 3.10
ci_cycles: 0
slices: 11
checkpoints: 3
build_minutes: 98.35
polish_minutes: 0.08
out_tokens: 731597
cache_read_tokens: 81081814
start_context: 32860
peak_context: 169795
---

# Retro — PRD-002

Written by `/deliver` in the closeout commit, from the agent's side of the run.

## Cost vs catch

| Step | Cost | Caught | Verdict |
|---|---|---|---|
| Inner loop, per slice | about 3 runs × 1 min per slice. 01: 3/1 red, 03: 3/1, 04: 3/1, 05: 2/1, 06: 3/1, 08: 7/1, 10: 4/2, 11: 4/1. 02, 07 and 09 did not record theirs. | compile errors and failing assertions inside the slice, all fixed before its commit | keep |
| Checkpoints (full gate) | 5 runs, 2 red, about 3 min (CP-1 2/1, CP-2 2/1, CP-3 1/0) | two clippy lints, `collapsible_if` and `double_ended_iterator_last`. It missed the real defect: HEAD did not compile, because every checkpoint gated the dirty tree, not the commits. | tune: gate what HEAD holds (a clean worktree of HEAD, or refuse while `git status --porcelain` is not empty) |
| Prove, at checkpoints | three drive scripts plus 11 transcripts under `evidence/` | the FR-13 to FR-19 rows on the real surface, with no defects found. It missed FR-20 entirely: no checkpoint's **Prove** line named it, so the slice 05 and 08 Prove tasks stayed open with no artifact. Polish drove them (`evidence/polish-fr20.txt`). | tune: a checkpoint lists every FR of the slices it covers and refuses to close with one unproven |
| Polish — local review | one `code-review` pass | **HEAD missing about 50 files of feature code** (high). `docs ship` named a broken `state.toml` as "slices not done". `factory answer BLOCK-n` had no way out across features. | keep |
| Polish — security pass | one subagent | nothing at confidence 7 or above. Notes: an idea id fed to git as a glob pathspec (fixed), `project.toml` unprotected and symlinks followed (parked), the vendored `docs.mjs` run by the hook (a PR-body proposal) | keep: the diff parses outside input and shells out to git |
| Polish — UX pass | none | not run: a CLI with no `.claude/skills/verify-*` harness | — |
| Polish — second opinion (Cursor, then 2 × Opus) | Cursor: 2 runs, both "Authentication required", unreviewed. Then two fresh Opus agents, one for the driver and one for migrate. | **two high data-loss bugs in `migrate`**: Review-line reasons dropped on 18 PRD-001 entries, and a PRD-002 summary cut about 2,600 characters short by a `|` in a code span. Also a silent fence drop, and `slice_status` swallowing a parse error. | keep, and get Cursor's auth back: the cross-vendor half did not happen |
| test-cleanup | one read-only classifier agent plus one gate run | **5 tests asserting exact counts of this repo's live `docs/` at HEAD**. This pass's own `ideas.md` lines would have turned one red on the polish commit (AD-9906). It also pruned 2 duplicates. | keep |
| Polish gate | 3 runs, 0 red, about 3 min | nothing red. Each run confirmed a batch of fixes. | keep |

## Where I guessed

- **The date shape.** The polish contract asks for `reviewed: 20261013` and AD headings dated `20261013`. Every other date in this repo, PRD-003's `reviewed` included, is `2026-10-13`. I followed the contract. `docs check` reads it without complaint (W-READY-TO-SHIP quotes it back).
- **FR-16 against itself.** "The retro file is left as it is" and "`check` is clean after migration" collide when a retro links to `agent-log.md`. The PRD does not say which wins. AD-9904 keeps the retro byte for byte and warns on each such link.
- **What polish may commit.** HEAD did not build without the untracked slice code. The contract says the driver commits what I name. Staging about 50 files of other slices' code is outside a review's usual reach, but a block would have left a branch that cannot compile. I staged them (AD-9901, decide).
- **Proving FR-20 in polish.** The polish skill calls a done slice with no Prove evidence "a finding". Neither the skill nor the contract says whether polish should drive the rows or block. I drove them, because the rows were mechanical and the binary was built.
- **test-cleanup on a dirty tree.** Its precondition wants the tree clean or "dirty only with the changes under review". The leftovers were the slices' own code, so I treated them as under review.

## What I wished existed

- A driver check that the tree a checkpoint gated is the tree HEAD holds. The worker hook already sees every Write. A `git status --porcelain` before the gate, outside the run's scratch paths, would have caught AD-9901 at CP-1.
- A coverage line per checkpoint: "FR-13…FR-20: proven at CP-n | not yet". With it, a missing FR is one glance, not a polish finding.
- A Cursor auth probe in `factory doctor` or the polish preflight, so "unreviewed" shows up before the review instead of after it.
- `factory docs idea add` working in a v2 repo. Parking here meant hand-editing `docs/ideas.md`.

## Project friction → `/guardrails audit`

- **Tests that read the repo's own live docs.** Slices 07 and 08 asserted exact counts of `docs/ideas.md`, the index and the PRD-001 log at HEAD. Any commit to `docs/` can turn them red (AD-9906 freezes them). Rule: a test reads fixtures, never the repo's live `docs/`.
- `W-SIZE` (30 KB ceiling) fires on `slice-07-migrate.md` once its Code-review fixes are in. It is a warning and costs nothing, but a review section should not count toward a slice's spec size.
- Nothing in the gate cost more than it caught: the full gate runs in about 1 minute.

## Framework friction → skills repo

- **The factory driver's pathspec commit** (`src/commit.rs` `Pathspec`, the leftovers handling in `src/scheduler.rs`, and `skills/factory-deliver` § the slice commit). Files a worker changed with a command and never named stay out of the commit. The run listed them under `## Untracked leftovers` and went on, and the next checkpoint gated them anyway. That one gap cost the branch its buildability. The leftovers list should block the checkpoint, not annotate it.
- **`deliver` § checkpoint Prove.** Each checkpoint names its own FRs, and nothing cross-checks them against the slices' Prove tasks. FR-20 fell through.
- **`polish` § second opinion / `feature-docs/models.md`.** Cursor's auth expired, and the only signal was exit 1 after the call. The fallback is right, but nobody learns the cross-vendor half is gone until the review is over.
- **The polish contract's date format** (`20261013`) differs from the ISO dates the rest of the framework writes.

## Score

Left to the owner. From the agent's side: the slices built clean, but the checkpoints vouched for a tree HEAD did not hold, and an FR went unproven until polish.

## Numbers

<!-- factory:numbers -->
| role | workers | own | subagents | weighted | minutes | turns |
|---|---:|---:|---:|---:|---:|---:|
| build | 11 | 7775808 | 2436701 | 10212509 | 94 | 691 |
| fix | 2 | 110372 | 0 | 110372 | 4 | 11 |
| checkpoint | 3 | 1123042 | 0 | 1123042 | 10 | 74 |
| polish | 1 | 8797 | 3657786 | 3666583 | 0 | 101 |
| total | 17 | 9018020 | 6094487 | 15112506 | 108 | 877 |

Knobs: build opus/high default · fix opus/high default · checkpoint opus/high default · polish opus/high default · budget 30000000 default · lanes 4 · factory 0.1.0 · claude 2.1.296

Hypothesis: none

quality: gate_red 2 · worker_gate_red 0 · fix_commits 2 · ci_red_rounds 0 · ci_reruns 0 · findings high=3 medium=6 low=18 fixed=13 parked=7 · decisions 50 decide 8 blocks 0

Pack (build): slice 19970 · prd_sections 17030 · outline 10607 · lookup 890 · contract 3033 · autonomy 11246 · inner_loop 25 · total 62802 · start_context 53232

Compared with:

| run | feature | weighted | slices | per slice | lines changed | per line |
|---|---|---:|---:|---:|---:|---:|
| 002-20261013-e729 (this run) | 002 | 15112506 | 11 | 1373864 | 24701 | 612 |
| 003-20261013-4139 | 003 | 6966177 | 5 | 1393235 | — | — |
<!-- /factory:numbers -->
