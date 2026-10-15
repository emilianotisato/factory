# Slice 07 — The retro compares tracked retros

**Source**: [`prd.md`](prd.md) → §3 FR-4. **Depends on**: — (independent; it shares `src/closeout.rs` with slice 01, in another function)
**Created**: 2026-10-15

**Progress**:
Prove FR-4 (checkpoint 1): `evidence/cp-1-fr4-retro.txt` (the tracked-retros test green, and `factory retro` on this repo over a copy of the store) with the table read back in `evidence/cp-1-fr4-retro.md` (007 nine cells, 006 and 005 seven cells).
- Built: `numbers(db, run_id, root)` reads the `(this run)` row between the markers of every `docs/features/*/retro.md` and `docs/archive/*/retro.md` under `root`, skips the run's own feature by number, pads a 7-cell row with `— | — |`, sorts by the run id's date then feature number, newest first, and keeps 3 (`tracked_rows`, `this_run_row` in `src/retro.rs`). `NO_EARLIER` reads "No earlier retro in this repo." The closeout passes `repo`; `factory retro` passes `row.repo`, or the file's repo root with `--file` (AD-3). `compare_row` lost its `this` flag: it only prints this run's row now.
- Removed: `Db::earlier_runs`, its block in the db leaks test (renamed `the_leaks_figures_read_back_from_the_run_row`), the two store-cohort retro tests and `insert`, and `a_second_run_of_the_repo_compares_with_the_first`. Its exact check of this run's row (weighted, per slice, per line, `66.7% | 1`) moved into the new test.
- Tests: `the_retro_compares_with_the_three_newest_tracked_retros` (tests/closeout.rs), and unit tests for padding, the rowless and outside-the-markers skip, the tie, the cut and the own-feature skip.
- Inner loop: 1 run, 0 red, 2 min. Caught nothing.
- Deviations: none.

**Slice**: After this slice, a retro's "Compared with" table holds this run's row from the store, then the `(this run)` rows of the three newest tracked retros in the tree the retro is written in. A teammate's shipped features count; this machine's other runs no longer do. `earlier_runs` and its tests go.

## Current code / environment

- **Today's table**: `numbers(db, run_id)` (`src/retro.rs:152-221`) reads `db.earlier_runs(&run.repo, …, COHORT)` (`:204`), prints `Compared with: {NO_EARLIER}` when it is empty (`:205-206`), else the 9-column header, `compare_row(db, &run, true)` (`:121`) and one `compare_row` per earlier run. `NO_EARLIER` is `"No earlier finished run in this repo."` (`:15`), `COHORT = 3` (`:16`).
- **The two callers**: the closeout passes `run.id()` and writes into `feature_dir.join("retro.md")` (`src/closeout.rs:983,996`), where `feature_dir = repo.join(&board.dir)` and `repo` is the run's checkout (`:919-920`). `factory retro` writes into `--file` or the feature folder under `row.repo` (`src/cli/retro.rs:26-29,39`). The tree to read is the root holding that `retro.md`: `row.repo` without `--file`, else the root `paths::repo_root` finds from the file.
- **The tracked rows**: 002, 004, 005 and 006 hold 7 cells (run, feature, weighted, slices, per slice, lines changed, per line), e.g. `docs/features/006-*/retro.md:66-73`; 007 holds 9 (`docs/features/007-polish-in-the-binary/retro.md:69-76`, its row at `:73`). 001 says "No earlier finished run in this repo." (`docs/features/001-driver/retro.md:264`); 003 has no Numbers block. 001 also has a `| run | report |` table at `:218`, outside the markers: read only between `<!-- factory:numbers -->` and `<!-- /factory:numbers -->` (`src/retro.rs:13-14`).
- **Ties**: 002 and 004 share the run date 20261013, 005 and 006 share 20261014. The feature number breaks the tie.
- **What goes**: `earlier_runs` (`src/db.rs:454-474`) has no caller but `numbers` and the last assertion of `the_leaks_figures_read_back_from_both_selects` (`src/db.rs:1290-1301`). The retro tests that build earlier runs go with it: `numbers_compares_with_the_three_newest_earlier_runs_of_the_repo` (`src/retro.rs:371`), `a_stopped_run_is_not_compared_with` (`:470`) and their `insert` helper (`:353`), or clippy fails on dead code. So does `a_second_run_of_the_repo_compares_with_the_first` (`tests/closeout.rs:686-783`).
- **The text that changes**: `a_finished_board_polishes_ships_pushes_and_opens_the_draft_pr` asserts the old line (`tests/closeout.rs:579`). Its fixture has no tracked retro, so it reads the new line.
- **Deliberate non-changes**: *(prd §3 FR-4 accepted cost: "A copied row is the figure at its own run, not a recount.")* No recount of a copied row, and no attempt to reconcile 004's 7,099,681 with the 7,369,085 later retros carry.

## Tasks

### 1. The rows from the tree — high — blocked by: —
**Where**: `src/retro.rs:13-16,121,152-221`, `src/closeout.rs:996`, `src/cli/retro.rs:39`.

**Source**: §3 FR-4: "The "Compared with" rows are the `(this run)` rows of the three newest tracked retros under `docs/features/*/retro.md` and `docs/archive/*/retro.md` in the tree the retro is written in, this feature's own excluded, and a retro with no `(this run)` row skipped. Newest means the date in the run id (`007-20261015-c8fa`), then the feature number. Each row is copied as written, with `(this run)` dropped, and a 7-cell row gets `—` in the two missing cells. This run's own row still comes from the store (`compare_row`, `src/retro.rs:121`). With no tracked row, the line reads "No earlier retro in this repo." (replacing `NO_EARLIER`, `:15`)."

`numbers` takes the tree's root. It reads both globs, skips the run's own feature by number, takes the line holding `(this run)` between the markers, drops the marker text, pads a 7-cell row to 9 with `—`, sorts by the run id's date then the feature number, newest first, and keeps `COHORT`. The closeout passes `repo`; `factory retro` passes `row.repo`, or the file's repo root with `--file`.

The trap: the retro being rewritten already holds its own `(this run)` row once the closeout wrote it (`src/closeout.rs:996`). Skip the run's feature by number, or a second `factory retro` compares the run with itself.

**Acceptance:** task 3's test, and unit tests: a 7-cell row padded, a retro with no row skipped, a `(this run)` line outside the markers ignored, a tie broken by the feature number.

### 2. `earlier_runs` goes — high — blocked by: 1
**Where**: `src/db.rs:454-474,1290-1301`, `src/retro.rs:353-495`, `tests/closeout.rs:579,686-783`.

**Source**: §3 FR-4: "`earlier_runs` and its tests go (`src/db.rs:1290`, `src/retro.rs:371`)."

Delete the function, the two retro tests and `insert`, the trailing `earlier_runs` block of the db test (keep its `run_row` assertions), and the closeout test that builds a second run. Point `:579` at the new line.

**Acceptance:** `cargo clippy --all-targets -- -D warnings` clean; `grep -rn earlier_runs src tests` empty.

### 3. The test — high — blocked by: 1, 2
**Where**: `tests/closeout.rs`.

**Source**: §3 FR-4 acceptance: "In a fixture whose store holds only the 008 run, with tracked retros for 005, 006 and 007, `factory retro <run>` rewrites 008's `retro.md`. Its Compared-with table holds the 008 row from the store, then exactly those three rows, newest first, 005 and 006 padded with `—`."

A finished stub run, so the store holds one run and its feature's `retro.md`. Commit three retros into the fixture's `docs/features/`, each with a `(this run)` row between the markers: two 7-cell rows with run ids dated 20261014, one 9-cell row dated 20261015. Add an older fourth one and one with no row, to show the cut and the skip. Then `factory retro <run>`:

- the table's first row is the run's own;
- the next three are the 9-cell one, then the two 7-cell ones by feature number descending, each ending in `| — | — |`;
- the older retro and the rowless one are absent;
- a tree with no other retro reads `Compared with: No earlier retro in this repo.`

**Acceptance:** a test named for the behavior (e.g. `the_retro_compares_with_the_three_newest_tracked_retros`).

### 4. Verify — medium — blocked by: 1, 2, 3
`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 5. Prove — at the checkpoint — blocked by: 4
Rows from [`prd.md`](prd.md) §5 this slice proves, below the UI:

- **FR-4**: "run: `factory retro <run>` on a fixture whose store holds only that run, with three tracked retros (7 and 9 cells); read back the Compared-with table in `retro.md`" — task 3's test transcript, with the table it read back.

**Acceptance:** after the checkpoint, Progress names the transcript.

**Code-review fixes (2026-10-15).**
- `factory retro --file retro.md` with a bare file name looked for the repo from an empty path and compared with the wrong tree. An empty parent now means the current folder.
- The doc on the Numbers builder still said "deterministic on the same store"; it now names the tree too.
- One unreadable `retro.md` in another feature's folder failed the read and cost the shipping feature its Numbers. An unreadable retro is skipped now.
- The comparison read every `retro.md` on disk, so another feature's uncommitted draft could land in a shipped table. It reads only the retros git tracks, as FR-4 says.
