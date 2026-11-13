# Slice 05 — One log reader and no symlinks

**Source**: [`prd.md`](prd.md) → §9 FR-7, §10 FR-8, §12, §14. **Depends on**: 01. Both edit `src/docs/board.rs`: 01 threads a board base through the scan, and this slice changes the same scan's `is_dir` and slice-file reads. Land 01 first, so this diff is only about links.
**Created**: 2026-11-12

**Progress**:

- **Built:**
  - `docs::log::numbered` returns each entry with its 1-based line (`Numbered = (usize, Entry)`). `entries` and `add` share its lossy `parse`.
  - `answers::entries` now calls `log::numbered` and does no JSON parsing or file reading of its own.
  - The board's feature-folder filter and `slice_files` use `DirEntry::file_type()`, so a symlinked folder or slice file is skipped. `renumber`'s `walk_md` skips every link, so it neither walks down one nor rewrites through one.
- **Tests:**
  - `answers_read_the_log_through_the_docs_reader` (`src/answers.rs`): a block, a line of invalid UTF-8, then an answer. It expects lines [1, 3], the block resolved, and no `serde_json` in the body of `entries`.
  - `the_board_and_renumber_skip_symlinks` (`tests/docs_lifecycle.rs`) uses three links: `002-linked` → `001-fixture`, slice 03 → `outside/slice.md`, and `docs/features/x/{loop → docs, out → outside}`.
- **Inner loop:** 1 run, 0 red, 2 min (1m45s). One earlier clippy pass caught `type_complexity` on `numbered`'s return type; the `Numbered` alias fixed it.
- **Seen red** (each fix reverted by hand, then restored):
  - Reader: with the old `read_to_string` loop, `entries` errors with `InvalidData: stream did not contain valid UTF-8`.
  - Folder filter: with `is_dir()` restored, the board lists features `[1, 2]`, not `[1]`.
  - Slice-file skip: with it removed, slice 03's `file` is the linked file.
  - `renumber` walk: with the old walk, `outside/slice.md` was rewritten to `PRD-005 and 005-fixture`.
- **Deviations:** none.
- **Prove (left for the checkpoint):**
  - FR-7: run `answers_read_the_log_through_the_docs_reader`.
  - FR-8: run `the_board_and_renumber_skip_symlinks`.

**Slice**: After this slice:

- `answers::entries` reads `log.jsonl` through `docs::log`'s reader, which returns each entry with its 1-based line, so a line of invalid UTF-8 no longer fails `factory answer`;
- the board scan skips a symlinked feature folder and slice file;
- `docs renumber` never follows or rewrites through a link.

## Current code / environment

- **FR-7**:
  - `answers::entries` (`src/answers.rs:150-160`) uses `fs::read_to_string` (`:152-155`), which errors on invalid UTF-8, then `serde_json::from_str` per line. A line that does not parse is skipped (`:156-158`).
  - `docs::log` has `parse` (`src/docs/log.rs:131`, private, reads with `from_utf8_lossy` at `:134`, drops line numbers) and `entries` (`:154`).
  - Expose one reader that yields `(line, Entry)`, and use it from both callers.
  - `a_block_is_resolved_by_an_answer_and_its_line_stays` (`src/answers.rs:272`) must still pass.
- **FR-8**:
  - `src/docs/board.rs:160`: `.filter(|e| e.path().is_dir())` follows links.
  - `:190-199` `slice_files` reads any `slice-NN-*.md`.
  - `src/docs/renumber.rs:22-33` `walk_md` recurses with `p.is_dir()`, and `:96-110` rewrites every `.md` found.
  - Use `fs::symlink_metadata` (or `DirEntry::file_type()`, which does not follow links), as `src/docs/check.rs:732-733` already does.
  - A link to an ancestor ends only at ELOOP (40 links) today, not "never" (prd §10, corrected).
- **What the board exposes**:
  - slice rows come from `state.toml` (`board.rs:111-124`), and a file only fills the row's `file` field;
  - so the acceptance checks that no feature exists for the linked folder and that no `file` points at the linked slice.
- **Not handled** (prd §14), so do not add them:
  - the `factory answer` double-append race;
  - the slice-status read-then-set race;
  - `settle_dead`'s two appends.
- **Out of scope** (prd §10): `check`'s own `walk_md` (`src/docs/check.rs:777`) and `migrate.rs:1332` also follow directory links. Leave them.

## Tasks

### 1. One reader for the run log — medium — blocked by: —
**Source**: §9 FR-7: "`docs::log` exposes one reader that returns each entry with its 1-based line. `answers::entries` uses it."

Edit `src/docs/log.rs` and `src/answers.rs`.

**Acceptance:** `answers_read_the_log_through_the_docs_reader`. With a log holding a valid block, a line of invalid UTF-8, then an answer:

- `answers::entries` returns both valid entries, with lines 1 and 3;
- `answers::entries` has no `serde_json` call of its own.

It fails today with a read error.

### 2. Skip symlinks in the board and renumber — medium — blocked by: —
**Source**: §10 FR-8: "The board scan and `renumber` use `symlink_metadata`. They skip a symlinked feature folder or slice file, and `renumber` never follows a link down."

Edit `src/docs/board.rs:160,190-199` and `src/docs/renumber.rs:22-33`.

**Acceptance:** `the_board_and_renumber_skip_symlinks`, with a fixture holding:

- a feature folder that is a symlink to another feature;
- a slice file that is a symlink;
- `docs/features/x/loop`, a link to `docs/`.

Then:

- `docs status --json` has no feature for the linked folder;
- no slice `file` points at the linked slice;
- `docs renumber` leaves every file reached only through a link byte-for-byte unchanged.

### 3. Verify — medium — blocked by: 1, 2
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q`

**Acceptance:** inner loop green, one commit.

### 4. Prove — at the checkpoint — blocked by: 3
prd §12 rows:

- **FR-7:** `run answers_read_the_log_through_the_docs_reader`.
- **FR-8:** `run the_board_and_renumber_skip_symlinks`.

They are proven below the UI.

**Acceptance:** after the checkpoint, the transcripts are in Progress.
