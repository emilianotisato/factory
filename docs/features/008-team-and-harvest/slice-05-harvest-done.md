# Slice 05 — The harvest's --done and docs archive

**Source**: [`prd.md`](prd.md) → §3 FR-5 (the moves, the texts). **Depends on**: 02 (the home-path rewrite of the kept files is 02's matcher; both edit `src/docs/check.rs`), 04 (`--done` is a flag on the verb 04 adds, in `src/docs/harvest.rs` and `src/cli/docs.rs`)
**Created**: 2026-10-15

**Progress**:
- Built: `ship::to_archive` (`src/docs/ship.rs`) is the one move both verbs run: refuse the whole batch first (dirty folder, `??` included, or an existing destination), banner after the frontmatter once, `paths::home_paths_to_tilde` over the four kept files, `git rm` the rest, `git mv`, `state::set` archived, `git add` the four last, print `dropped <path>` per file. git failures are errors carrying git's stderr; the `fs::rename` fallback is gone. `factory docs harvest NNN… --done` archives the resolved batch after the shipped-only refusal; `docs archive` keeps `live()` and `all_done()`. Texts: PR body and `W-READY-TO-SHIP` lose the archive clause, both index files define archived by the folder's contents, the convention block gains the history line (`tests/init.rs` pins it).
- Tests: `harvest_done_leaves_the_records_only_with_the_banner_staged_and_passing_the_hook`, `harvest_done_refuses_an_uncommitted_folder_and_moves_nothing`, `archive_leaves_the_same_records_only_shape` (`tests/docs_lifecycle.rs`); the old archive test commits its fixture first.
- Inner loop: 2 runs, 1 red (cargo fmt), 2 min.
- Deviations: the dirty-folder test runs `harvest 001 002 --done` with 002 dirty, so it also proves 001 is left in place; the archive-verb test archives a fresh fixture's 001, which has slices, a QA guide and evidence to drop.
- Prove (CP-2) FR-5: `evidence/cp-2-fr5-harvest-done.txt`, the tests' transcript, then the branch binary on a scratch repo with two shipped features: `harvest`; `harvest 001 --done` (`git ls-files` lists the four files, banner after the frontmatter, home paths as `~/`, status archived); `docs check` 0 errors; `git commit` through the installed hook exit 0; a dirty 002 refused with exit 2; then `docs archive 002` leaves the same shape.

**Slice**: After this slice, `factory docs harvest NNN… --done` and `factory docs archive NNN` leave one archived shape: the folder under `docs/archive/` holding exactly `prd.md` with a history banner, `state.toml` set to archived, `log.jsonl` and `retro.md`, home paths written `~/`, everything staged, nothing committed. The texts follow: the closeout ships and never archives, `docs/index.md` defines archived by what the folder holds, and the convention block says feature docs are history. The skill lines (polish, slice) are slice 08's.

**Highest risk**: task 1's order of steps. One edit after the staging, or one `M` in the porcelain, and the commit the skill makes next fails the hook.

## Current code / environment

- **Today's archive**: `ship::archive` (`src/docs/ship.rs:91-110`) runs `live()` (`:12-25`: refuses an archived feature and an unreadable state) and `all_done()` (`:27-42`), refuses an existing destination (`:96-98`), creates `docs/archive` (`:99`), git-mvs the folder and falls back to `fs::rename` when git fails (`:100-102`), sets `status = "archived"` and stages only `state.toml` (`:103-104`). Its `git` helper (`:44-53`) returns a bool and swallows stderr.
- **The frozen archive**: `archive_frozen` (`src/docs/check.rs:694-719`) errors `E-ARCHIVE-MODIFIED` on `M` in either column of `git status --porcelain -- docs/archive`. The hook (`src/docs/hook.rs:13-28`) refuses a staged `M` under `docs/archive/` (`:18-21`), then runs `factory docs check` on any staged `docs/` path when `factory` is on `PATH` (`:22-25`) and skips it silently when it is not (`:23`).
- **The porcelain, checked 2026-10-15** in a scratch repo: after `git mv` plus an edit plus `git add`, `git status --porcelain -- docs/archive` shows `A` for every kept file, never `R` (the pathspec hides the source side), and never `M`. Without the pathspec an edited small file can show `D` plus `A`. Staging a file and then editing it shows `AM`, which is the error. The PRD's "each kept file shows `R`" is corrected in FR-5.
- **What is tracked in a feature folder**: slices, `qa-guide.md`, `research.md` (006), `evidence/` and 001's recordings, 3 of them `.pyc` files. `docs.lock` is ignored and moves with the folder; "exactly four" means four tracked files (`git ls-files`).
- **Archived folders are barely checked**: `check::run` gives archived folders only the status check (`src/docs/check.rs:133-150`), so the slice table left in an archived `state.toml` is fine. The status check (`E-STATUS-PLACE`, `:436`) stays.
- **Links**: `links()` (`src/docs/check.rs:734-767`) skips files inside `docs/archive/`, but a live doc that links into a folder that moved fails `E-LINK-BROKEN`, and the hook runs that check. A test fixture must not link into the folder it archives.
- **The existing archive test**: `archive_moves_the_folder_and_leaves_the_index_alone` (`tests/docs_lifecycle.rs:287-319`) archives an uncommitted fixture. The dirty-folder refusal makes it fail: commit first (`commit(fx)`, `:23-26`).
- **The hook test to copy**: `the_hook_refuses_an_archive_edit_and_a_broken_link_and_skips_without_factory` (`tests/docs_v3.rs:441-500`): `fx.docs(&["hook"])`, then `commit_with_path` (`:433-439`) with the test binary's directory first on `PATH` (`:451-452`).
- **The texts**: `assets/pr-body.md:10` ends "If nothing is left after review, run `docs archive {NNN}` on this branch before merging." `W-READY-TO-SHIP`'s hint ends "(or `factory docs archive {nnn}` if nothing is left)" (`src/docs/check.rs:525`). `docs/index.md:22,61` and `assets/docs/index.md:22,59` define archived as "every slice done *and* nothing left to do". `assets/docs/convention.md:3-8` has no history line; `convention()` appends the block only when its heading is absent (`src/docs/scaffold.rs:137-197`).
- **Deliberate non-changes**: *(prd §3 FR-5 "No verb can check that the routing happened, so the definition does not promise it.")* Neither verb asks whether lessons were routed. `docs archive` keeps its `live()` and `all_done()` guards and does not gain harvest's shipped-only refusal. *(prd §7 "Not handled: scratch paths … `W-HOME-PATH` looks for home paths only.")* The kept files' rewrite touches home paths only.

## Tasks

### 1. The moves — high — blocked by: —
**Where**: `src/docs/harvest.rs` (or a shared function `ship::archive` also calls), `src/docs/ship.rs:91-110`.

**Source**: §3 FR-5: "**`--done`** archives the batch. For each feature it refuses when the folder has uncommitted changes or `docs/archive/<dir>` exists. Then it puts one banner under `prd.md`'s frontmatter: `> **History.** The code, \`docs/reference\` and \`docs/decisions.md\` win over this document.` It rewrites `/home/<name>/` and `/Users/<name>/` to `~/` in the files it keeps. It `git rm`s every tracked file except `prd.md`, `state.toml`, `log.jsonl` and `retro.md`, git-mvs the folder to `docs/archive/`, sets `status = "archived"` and stages everything … It prints what it dropped and leaves the commit to its caller." And: "**`factory docs archive NNN`** runs the same moves as `harvest NNN --done`, so an archived folder has one shape."

One function, in this order:

1. For every feature in the batch, refuse before touching any: `git status --porcelain -- <dir>` not empty (an untracked `??` counts), or `docs/archive/<dir>` exists.
2. Insert the banner after the frontmatter's closing `---`, once.
3. Rewrite home paths with slice 02's matcher in the four kept files.
4. `git rm -q` every other tracked file in the folder.
5. `create_dir_all(docs/archive)`, then `git mv` the folder.
6. `state::set(<dest>, status=archived)`.
7. `git add` the four files at the destination.
8. Print the dropped paths.

The trap: stage last. A file staged and then edited shows `AM`, and `E-ARCHIVE-MODIFIED` fires. A failed `git rm` or `git mv` is an error naming git's stderr: never fall back to `fs::rename` (`src/docs/ship.rs:100-102`), which moves the folder unstaged.

**Acceptance:** task 3's tests.

### 2. The flag and the archive verb — high — blocked by: 1
**Where**: `src/cli/docs.rs` (the `Harvest` variant from slice 04, `Verb::Archive` at `:95,379`), `tests/docs_lifecycle.rs:287-319`.

**Source**: §3 FR-5, as task 1.

`--done` on `Harvest` runs task 1 over the batch the read resolves, after the same shipped-only refusal. `docs archive NNN` keeps `live()` and `all_done()` and runs task 1 for its one feature. Commit the fixture in the existing archive test before archiving.

**Acceptance:** the existing archive test passes after its commit, and task 3's tests.

### 3. The tests — high — blocked by: 2
**Where**: `tests/docs_lifecycle.rs`.

**Source**: §3 FR-5 acceptance: "`factory docs harvest 001 --done` leaves `docs/archive/001-x/` holding exactly the four files, the banner right after the frontmatter, no `/home/` in them, and status archived. Then `factory docs check` reports no `E-ARCHIVE-MODIFIED`, and `git commit` passes the hook `factory docs hook` installs."

Slice 04's two shipped features, committed, with a slice file, a `qa-guide.md`, an `evidence/` file and a `/home/x/` line in `prd.md` and `log.jsonl`. Then:

- `harvest 001 --done`: `git ls-files docs/archive/001-*` lists exactly the four files; the banner is the first line after the frontmatter; no kept file holds `/home/`; `state.toml` says archived; stdout names the dropped files; `git status --porcelain -- docs/archive` holds no `M`;
- `factory docs check` holds no `E-ARCHIVE-MODIFIED`, and `docs hook` plus `git commit` with the binary on `PATH` exits 0;
- an uncommitted edit in 002's folder: `harvest 002 --done` exits 2 and moves nothing;
- `docs archive 002` after a commit: the same four-file shape.

**Acceptance:** the four cases above, in tests named for the behavior.

### 4. The texts — medium — blocked by: —
**Where**: `assets/pr-body.md:10`, `src/docs/check.rs:525`, `docs/index.md:22,61`, `assets/docs/index.md:22,59`, `assets/docs/convention.md`.

**Source**: §3 FR-5: "**The closeout ships, never archives.** … `assets/pr-body.md:10` loses its archive sentence. `docs/index.md` and `assets/docs/index.md` define archived by what the folder holds: moved, records only, history banner on. The definition adds that `/harvest` routes the lessons first." And: "`assets/docs/convention.md` gains: "`docs/features` and `docs/archive` are history: the code, `docs/reference` and `docs/decisions.md` win.""

Drop the PR body's archive sentence and the hint's archive clause. Rewrite the two archived lines in both index files. Add the convention line. Slice 08 edits `assets/docs/convention.md:8` (the skill list): keep to your own line.

**Acceptance:** any test that pins the old hint or the convention block passes with the new text; `factory init` in a fresh repo writes the history line into `CLAUDE.md`.

### 5. Verify — medium — blocked by: 1, 2, 3, 4
`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 6. Prove — at the checkpoint — blocked by: 5
Rows from [`prd.md`](prd.md) §5 this slice proves, below the UI:

- **FR-5**: "run: `factory docs harvest`, then `--done`, on a fixture with two shipped features; `factory docs check`; `git commit` through the installed hook" — task 3's test transcript, with the `git ls-files` listing of the archived folder.

**Acceptance:** after the checkpoint, Progress names the transcript.
