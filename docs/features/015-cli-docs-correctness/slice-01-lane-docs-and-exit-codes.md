# Slice 01 — Lane docs and exit codes

**Source**: [`prd.md`](prd.md) → §3 FR-1, §8 FR-6, §2, §12. **Depends on**: —. Both FRs edit `src/cli/docs.rs` (`board_root`, `route`, `docs`), so they are one slice. It is independent of 02-04.
**Created**: 2026-11-12

**Highest risk in the feature**: FR-1. `check` must read prose from the lane and only `state.toml` and `log.jsonl` from the main tree, without breaking the other-repo guard or the whole-root verbs.

**Progress**:

- **Built:**
  - `board_root` adds `log show` to the whole-root verbs. The env and same-repo guard moved into `board_tree`, which `check` also uses.
  - `check::run_on(root, board)` reads each live feature's `state.toml` and `log.jsonl` from `board::board_dir`. That is the same-named folder under the main tree, or the lane's own folder when main has none (AD-4).
  - `check::run(root)` is `run_on(root, None)`, so the driver, preflight and doctor are unchanged. `board::scan_folder_on` gives the scan its optional board base.
  - `set` prints the absolute `state.toml` path when it wrote outside the cwd's repo.
  - `log add` prints the absolute `log.jsonl` path on stderr and keeps the id alone on stdout (AD-3).
  - `retro --set` needed no change: its row already prints `repo: <main tree>`, and the new test asserts that.
  - `docs` maps every `Err` to exit 2.
- **Inner loop:** 3 runs, 1 red, about 4 minutes. The red was clippy on a `log_rules` parameter left unused.
- **Deliberate reds**, each restored and green after:
  - `a_lane_reads_the_board_files_it_wrote`: with `check` passed no board, it failed at the `W-READY-TO-SHIP` assertion. With `log show` dropped from the whole-root list, it failed at the `log show` assertion.
  - `a_docs_verb_exits_2_when_it_cannot_read_a_file`: with the mapping back to `Msg` only, it exited `Some(1)` where `Some(2)` was expected.
- **Prove:** the FR-1 and FR-6 rows are left for the checkpoint.

**Slice**: After this slice:

- in a lane with `FACTORY_BOARD` set, `docs log show` reads the main tree's log, as `log add` writes it;
- `docs check` reads each feature's `state.toml` and `log.jsonl` from the main tree while validating the lane's prose;
- `set`, `log add` and `retro --set` name the main tree's path when they wrote outside the cwd;
- every `Err` under `factory docs` exits 2.

## Current code / environment

- **The routing**:
  - `route` (`src/cli/docs.rs:223-226`) does `let root = board_root(&root, args).unwrap_or(root);`, swapping the whole root.
  - `board_root` (`:253-264`) matches only `["set" | "retro" | "status", ..] | ["log", "add"]`, then requires `paths::git_common_dir(&board) == paths::git_common_dir(root)`.
  - `FACTORY_BOARD` is `crate::worker::BOARD_ENV`, set to the main tree for every worker (`src/worker.rs:55,260`).
- **`log show`** reads only the log (`:436`, `log::entries(&f.dir)`). Adding `["log", "show"]` to the whole-root list is exact.
- **`check`**: `check::run(root)` (`src/docs/check.rs:82`) scans the board under `root`.
  - The board scan reads each feature's state with `state::read(&dir)` (`src/docs/board.rs:174`).
  - `check` reads the log with `log::entries(&f.dir)` (`src/docs/check.rs:609`).
  - For a lane, those two reads must come from the same-named feature folder under the main tree's `docs/features/`. Everything else stays on the lane: `prd.md`, slices, `project.toml` and links. Thread an optional board base through `check::run` and the scan. Callers that pass none behave exactly as today.
- **Callers of `check::run` that must stay unchanged**:
  - the driver's in-process `check::run(&lane_dir)` (`src/scheduler.rs:2076`), which has no `FACTORY_BOARD`;
  - `src/preflight.rs:97`;
  - `src/doctor.rs:68`.
  - prd §3: "Unchanged: … the driver's in-process `check::run(&lane_dir)`."
- **The messages**:
  - `set` prints `docs/features/{name}/state.toml updated` (`src/cli/docs.rs:706`).
  - When the root came from `board_root`, i.e. it differs from the cwd's repo root, print the absolute main-tree path.
  - Do the same for `log add`'s and `retro --set`'s write messages.
- **Exit codes**:
  - `docs` (`src/cli/docs.rs:213-221`) maps only `Error::Msg` to `Ok(2)`. Variants are listed in `src/error.rs:7-28`.
  - `Io`, `Toml`, `Json` and `Sql` fall to `src/main.rs:109-113`, which exits 1.
  - Keep every deliberate `Ok(1)`: `check` (`:290-297`), `retro` with no row (`:574`), `gate` (`src/cli/gate.rs:481,519`) and preflight (`src/preflight.rs:221,418`).
- **Tests that pin this**:
  - `a_lane_writes_the_main_trees_board_and_ignores_another_repos` (`tests/docs_v3.rs`, helper `board_docs` at `:566`, lane created `--detach` at `:595`). Its `status` JSON at `:637` carries the git branch, and its `retro` read is at `:646`. Both stay on the whole-root path.
  - `tests/docs_retro.rs:30` pins `Some(1)` for a missing retro row.
- **Trap**: the same-repo guard (`:261-263`) keeps a fixture's `docs` call inside `cargo test` in a worker off the worker's board. The new `check` read must go through the same guard.
- **Deliberate non-changes**: `docs new` and `docs slice` keep writing the cwd's `state.toml` (prd §3, "No run has created a feature inside a lane"), and `status` keeps its whole-root swap.

## Tasks

### 1. `log show` and `check` read the main tree's board files — high — blocked by: —
**Source**: §3 FR-1: "`log show` joins the whole-root list… `check` keeps the lane as its root… but when `board_root` would resolve for it, it reads each feature's `state.toml` and `log.jsonl` from the matching folder in the main tree."

Edit `src/cli/docs.rs` (`board_root`'s verb list, plus a board-only resolution for `check` through the same guard), `src/docs/check.rs` and `src/docs/board.rs` (an optional board base for the state and log reads).

**Acceptance:** `a_lane_reads_the_board_files_it_wrote`, in `tests/docs_v3.rs` beside the existing lane test. In a lane with `FACTORY_BOARD` set:

- `docs set N reviewed=<date>`, with every slice done, then `docs check`, shows no `W-NOT-REVIEWED`;
- `docs log add`, then `docs log show`, shows the entry.

It fails today, because check reads the lane's `state.toml`. The existing lane test, other-repo half included, still passes.

### 2. Write messages name the main tree — medium — blocked by: 1
**Source**: §3 FR-1: "A write's message names the main tree's path when it is not the cwd: `set`, `log add`, and `retro --set`."

**Acceptance:** in the same test, `set`'s stdout holds the main tree's absolute `state.toml` path.

### 3. Every docs error exits 2 — high — blocked by: —
**Source**: §8 FR-6: "Under `factory docs`, every `Err` exits 2: the `Io`, `Toml`, `Json` and `Sql` variants join `Msg`… Exits 1 that a verb returns on purpose as `Ok(1)` are unchanged."

Edit `docs` in `src/cli/docs.rs:213-221`.

**Acceptance:** `a_docs_verb_exits_2_when_it_cannot_read_a_file`. Despite the name, kept as the QA row names it, the test runs `docs log show 001` where `log.jsonl` is a directory, and it exits 2. It exits 1 on v0.13.4. A `check` with errors still exits 1, and `tests/docs_retro.rs:30` still passes.

### 4. Verify — medium — blocked by: 1, 2, 3
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q`

**Acceptance:** inner loop green, one commit.

### 5. Prove — at the checkpoint — blocked by: 4
prd §12 rows:

- **FR-1:** `run a_lane_reads_the_board_files_it_wrote`.
- **FR-6:** `run a_docs_verb_exits_2_when_it_cannot_read_a_file`.

They are proven below the UI: the checkpoint runs them and keeps the transcript.

**Acceptance:** after the checkpoint, the transcripts are in Progress.
