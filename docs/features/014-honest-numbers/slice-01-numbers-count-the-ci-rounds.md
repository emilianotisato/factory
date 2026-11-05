# Slice 01 — Numbers count the CI rounds

**Source**: [`prd.md`](prd.md) → §2 FR-1, §3 QA plan. **Depends on**: — (the PRD's only slice; a `factory` run builds from slice files)
**Created**: 2026-11-05

**Progress**:
- **Built.** `push` (`src/closeout.rs`) takes the board instead of the feature dir and calls the new `refresh_numbers` after `branch_guard` and before `ci::push`. When `retro.md` exists, `refresh_numbers` computes and writes `## Numbers`, and on `Ok(true)` commits that one path as `docs(NNN): refresh numbers` through `commit::commit` and `commit::subject`. A compute or write error is said as `push: no ## Numbers written — {e}` and the push goes ahead. A commit error still propagates (AD-1). The ship step is unchanged.
- **Tests** (`tests/closeout.rs`): `a_red_check_gets_one_fix_round_then_green` now ends on `docs(001): refresh numbers` at `HEAD`. That commit holds only the retro, it matches the working tree, and its `fix` row counts one more worker than the ship commit's. `three_fix_rounds_on_a_red_check_block_the_closeout` also pinned the subjects, so it now expects one refresh commit after each fix (the PRD's accepted cost). The new `a_numbers_section_that_cannot_be_written_still_lets_the_fix_round_push` uses polish friction text with a stray opening marker and asserts both pushes, the skip line, no refresh commit and a clean tree. `a_finished_board_polishes_ships_pushes_and_opens_the_draft_pr` and `a_flaky_check_is_rerun_once_then_green` are untouched and green.
- **FR-1 proven** (checkpoint 1): `cargo test --test closeout` passed 37 of 37, including the three named tests and the new one. The red-check test now also asserts that `ci_green.head` is the refresh commit. Transcript: `cp-1/closeout-tests.txt`.
- **Inner loop:** 2 runs, 1 red, ~2 min. The red run caught `three_fix_rounds_on_a_red_check_block_the_closeout`, which pinned the old subject list. Before that, one targeted run of the 4 named closeout tests was green.
- **Prove (FR-1 row):** left for the checkpoint.

**Slice**: `push` in `src/closeout.rs` rewrites the retro's `## Numbers` from the store before it pushes, and when the section changed, commits the retro alone as `docs(NNN): refresh numbers`. A CI fix round's push then carries the fix and the refreshed section together; the push in `open_pr` right after ship changes nothing and commits nothing. A failure to compute or write the section is said in one line and never stops the push.

## Current code / environment

- **The one place to change**: `fn push` at `src/closeout.rs:1107`. It is called from `open_pr` at `:1015` (before `ci::pr_create`, `:1079`, on every `closeout::run`, a resume included) and from the watch loop at `:1378`, right after `fix_round` returns and `run.reload()`. Both callers pass `feature_dir` and `branch`; `push` already has `run` and `docs`.
- **The store is current when `push` runs**: `fix_round` lands (`land_here`, `:1951`), then `settle` (`:1963`, defined `:2267`) writes usage, subagents and exit before it returns `Ok(None)`. On a resume, `settle_orphan` (`:1971`, landing `:2060`, settle `:2075`) has settled the earlier driver's CI fix worker before `closeout::run` reaches `open_pr`.
- **The pattern to copy is the ship step**, `src/closeout.rs:924-942`: `let retro = feature_dir.join("retro.md")`, `retro::write(&retro, &retro::numbers(run.db(), run.id().as_str(), &repo)?)`, then `commit::commit(&repo, &want, &commit::subject(board, "docs", …))`. Two differences here: a `retro::numbers` error must not fail the push (ship's `?` does fail ship, and prd §2 leaves ship alone), and the commit's pathspec is the retro file alone, not the whole feature dir. `push` has no `board`: build the subject from the run's feature the way `commit::subject` does, or pass what it needs.
- **`retro::write` returns `Ok(false)` when nothing changed** (`src/retro.rs:277`) and refuses a marker count other than one ordered pair (`:294-299`); `commit::commit` returns `None` on an empty pathspec diff. Commit only on `Ok(true)`.
- **The retro may be missing** if a resume reaches `push` from a state where ship never wrote it; skip silently when `retro.md` does not exist (ship creates it, `:924-933`).
- **CI fix workers are role `fix`** (`src/worker.rs:133`), so the refreshed table's `fix` row gains one worker per round; its `workers` cell is the visible change in the stub, whose workers may report no usage.
- **Tests that pin today's behaviour** (`tests/closeout.rs`): `a_red_check_gets_one_fix_round_then_green` (`:1042`) pins the subjects to end on `fix(001): CI test` (`:1082-1088`) and the working-tree retro equal to the ship commit's (`:1094-1097`). Both move: the subjects gain `docs(001): refresh numbers`, and the retro now equals that commit's. `ci_fix_committed.sha` (`:1149`) stays the fix commit, since the event is emitted at land time. `a_finished_board_polishes_ships_pushes_and_opens_the_draft_pr` (`:366`, `git log -1` at `:461-464`) and `a_flaky_check_is_rerun_once_then_green` (`:1157`) must not move. Helpers: `driver_log` (`:104`), `subjects`, `sha_of`, `files_of`, `origin_head`, `RETRO` (`:19`); markers are `retro::OPEN` / `retro::CLOSE` (`src/retro.rs:13-14`).
- **Deliberate non-changes**: prd §5 *Not handled: a missing transcript stores zeros* (keep `src/metrics.rs:448-466` as is) and *Not handled: partial sums in the Numbers table* (no `*` or footnote). `factory retro <run>` stays as it is. Do not touch the ship step.

## Tasks

### 1. Refresh Numbers in `push` — high — blocked by: —
**Source**: §2 FR-1: "`push` rewrites `## Numbers` before it pushes, and when the section changed, commits the retro alone as `docs(NNN): refresh numbers` through `commit::commit` … A failure to compute or write the section never fails the push. It is said in one line … and the push goes ahead."

`src/closeout.rs` `fn push`. Before `ci::push`, after `branch_guard`: if `feature_dir/retro.md` exists, compute `retro::numbers`, `retro::write` it, and on `Ok(true)` commit that one path with the subject `docs(NNN): refresh numbers`. On a compute or write error, `say` one line, `push: no ## Numbers written — {e}`, and go on to the push.

**The trap:** `retro::numbers(..)?` in the ship step propagates its error (`src/closeout.rs:936`); copied as is, a store error would stop every push of the run.

**Acceptance:** `a_red_check_gets_one_fix_round_then_green` ends with subjects `… "docs(001): ship PRD-001", "fix(001): CI test", "docs(001): refresh numbers"`; the refresh commit's files are the retro alone; the working-tree retro equals that commit's; its Numbers `fix` row's `workers` cell counts the CI fix worker (one more than the ship commit's); `origin_head` is `head`.

### 2. Pin the no-change and the failure roads — high — blocked by: 1
**Source**: §2 FR-1 acceptance 2: "The PR opens on the ship commit: the push in `open_pr` adds no docs commit when no worker ran after ship." Acceptance 3: "A Numbers write that fails (the retro holds a second opening marker …) still pushes, and the driver log says why the section was not written."

`tests/closeout.rs`. Keep `a_finished_board_polishes_ships_pushes_and_opens_the_draft_pr` and `a_flaky_check_is_rerun_once_then_green` unchanged and green (no refresh commit, tip `docs(001): ship PRD-001`). Add a test, by behaviour name, that runs one red CI round with a retro whose polish step leaves a second `<!-- factory:numbers -->` opening marker, and asserts the push happened (`origin_head` is `head`, `pr: pushed` in `driver_log`), the driver log carries `no ## Numbers written`, and no `docs(001): refresh numbers` commit exists.

**Acceptance:** the new test fails if `push` propagates the write error or commits anyway.

### 3. Verify — medium — blocked by: 1, 2
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q` (the inner loop; the full gate runs at this slice's checkpoint)

**Acceptance:** inner loop green, one commit.

### 4. Prove — at the checkpoint — blocked by: 3
§3 QA plan, row FR-1: `run cargo test --test closeout`, the three named tests and the new one, transcript kept in Progress.

**Acceptance:** after the checkpoint, the FR-1 row has its transcript in Progress.

**Code-review fixes (2026-11-05).**
- **A refresh left on disk was never committed.** `refresh_numbers` committed only when `retro::write` returned `Ok(true)`. A driver killed after the write and before its commit resumes to an `Ok(false)` write: the file already matches, so the refreshed section never reached a commit and the push sent the old one. Both the local review and the second opinion raised this. It now commits on any successful write, and `commit::commit` still makes no commit when nothing differs.
- **The doc comment said "never failing the push"**, but a failed commit still stops it (AD-1). It now says both, and why a no-change write still commits.
- **The resume road had no test.** The PRD put the refresh in `push` so a resumed driver's `open_pr` push would carry it. `a_ci_fix_orphan_adopted_by_a_resumed_driver_lands_once` now asserts `docs(001): refresh numbers` is `HEAD` and on origin after the resume.
- `a_numbers_section_that_cannot_be_written_still_lets_the_fix_round_push` reads the subjects once. The tail check already rules out a refresh commit.
