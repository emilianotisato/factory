## Decisions

### AD-601 — How does the Prove task get a diff of `001-driver/retro.md` from run 024, whose `retro.md` lives in another repo? · slice-06 · 2026-10-13
**Chose:** `factory retro <run> --file <PATH>` writes the section into the named file instead of the feature's own `retro.md`; without `--file` the subcommand resolves `runs.repo` + `runs.feature` as the slice says.
**Over:** writing into `/home/emiliano/Projects/Prokey/wt-b/docs/features/024-admin-inspection-ux/retro.md` (another repo, and not the file the Prove task diffs), or a temporary copy of the store with a rewritten `repo`.
**Because:** run 024 is `repo = /home/emiliano/Projects/Prokey/wt-b`, feature 024; its resolved `retro.md` is Prokey's, while slice-06 task 7 asks for the `git diff` of `docs/features/001-driver/retro.md`. The flag is one optional argument; the default path is unchanged.
**Undo:** drop `file` from `RetroArgs` in `src/cli/retro.rs`; Prove then diffs Prokey's file.
**Review:** decide

### AD-602 — Which earlier runs make the cohort when a run starts in the same second the previous one finished? · slice-06 · 2026-10-13
**Chose:** `Db::earlier_runs(repo, before, except, limit)`: same `repo`, phase `finished`, `finished_at <= started_at`, `id != except`, newest `finished_at` first, then id.
**Over:** the slice's strict `finished_at < started_at`.
**Because:** timestamps are whole seconds; a second run started right after the first one finished (the harness does this) shares the second and would miss it. Excluding the run's own id keeps "never the run itself" exact, and the cohort is still fixed by `started_at`, so a rerun picks the same rows.
**Undo:** `<=` to `<` and drop the `except` argument in `src/db.rs` `earlier_runs`.
**Review:** no

### AD-603 — What weighted figure do the `total` row and the comparison print? · slice-06 · 2026-10-13
**Chose:** the role rows sum `--per-worker`'s rounded `weighted_own + weighted_sub` (so each role equals the report's per-worker rows exactly, the harness acceptance); the `total` row and the comparison print the run's `weighted_tokens` (`quota::weighted`, rounded once over the unrounded sum).
**Over:** a total that sums the role rows.
**Because:** per-worker rounding drifts: run 024's role rows add up to 18,465,010, its `weighted_tokens` is 18,465,008, the figure prd §7 names and the budget reads. The total may differ from the sum of the rows above it by a few tokens.
**Undo:** in `src/retro.rs` `numbers`, print `total.weighted()` and use the role sum in `compare_row`.
**Review:** no

### AD-604 — The old family's files outside the repo still name `measure.py` · slice-06 · 2026-10-13
**Chose:** leave them and list them here: `~/.claude/skills/feature-docs/templates/retro.md:27` (the retro template the ship step and the polish pack read) tells the writer to run `measure.py`; `feature-docs/MAINTAINING.md` and `deliver/SKILL.md` mention it too.
**Over:** editing them from this run.
**Because:** prd §7 and §12 make the old family PRD-002's and the owner's; the run may not write outside the repo. Until the template changes, a polish worker reading it can still be told to run `measure.py`, which is the one live instruction left.
**Undo:** n/a; the fix is one line in the template, for the owner or PRD-002.
**Review:** decide

### AD-605 — Repo files naming `measure.py` beyond the acceptance's list · slice-06 · 2026-10-13
**Chose:** keep them: `001-driver/agent-log.md`, `evidence/cp-2-*.txt`, `evidence/cp-4-*.txt`, `slice-03-one-worker.md`, `slice-09-closeout.md`, `002-docs-port/prd.md`, `docs/index.md`, and the origin citations in `src/stream.rs` and `src/cli/report.rs:93`, beside the ones task 5 names.
**Over:** rewording records so the grep list matches task 5's exactly.
**Because:** each is a record of what was done or the origin of a rule, not a step anyone follows; rewriting history would make them wrong. The only instruction, `001-driver/qa-guide.md` step 6, now names `factory report` and `factory retro`.
**Undo:** n/a.
**Review:** fyi

## Traps

- `tests/commits.rs` `a_file_git_added_by_name_is_committed_beside_a_live_sibling` failed once under the full `cargo test` load (slice A's commit held slice C's `src/c.txt`) and passed 5 of 5 alone and on the next full run: rerun before chasing it.
