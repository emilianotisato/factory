# Slice 02 — No home paths in tracked docs

**Source**: [`prd.md`](prd.md) → §3 FR-3. **Depends on**: — (independent; slices 05 and 06 build on the home-path matcher it adds)
**Created**: 2026-10-15

**Progress**:
Prove FR-3 (checkpoint 1): `evidence/cp-1-fr3-home-paths.txt` (the `docs check` unit test, the red-gate block-body test, and `factory docs check` on this repo: 45 `W-HOME-PATH` warnings, 0 errors).
- Built: `paths::find_home_path` (bytes, first `/home/<name>/` or `/Users/<name>/` that starts a path) and `paths::home_to_tilde` (rewrites `<home>/` at a path start; a home of `/` rewrites nothing), with unit tests. `answers::add_block`, `add_note` and `resolve` rewrite title and body before `log::add`. `check::home_paths` warns `W-HOME-PATH` as `path:line` per tracked `docs/` file, skipping `docs/archive/` and non-work-trees. `init` appends each missing ignore line (`docs.lock`, `__pycache__/`); this repo's `docs/.gitignore` has both.
- Tests: `a_tracked_doc_holding_a_home_path_warns_once_with_its_first_line` (`src/docs/check.rs`); `a_block_body_names_the_run_dir_under_the_home_as_tilde` (`tests/checkpoints.rs`), seen failing on its `~/` assert with the rewrite disabled.
- Inner loop: 3 runs, 1 red, ~3 min. The red run caught `cargo fmt` diffs only. A `docs check` on this repo from the lane build then showed 43 hits where `git grep` finds 45 files: two `.pyc` files hold the path right after a length byte. Fixed by AD-2 (binary files skip the path-start rule); it now shows 45.
- Not done here: the 3 tracked `.pyc` files under `docs/features/001-driver/` stay tracked and keep warning. Untracking them edits another feature's folder; the ignore line only stops new ones.
- Prove (FR-3) left for the checkpoint: the two test transcripts, and `factory docs check` on this repo from this build.

**Slice**: After this slice, the driver writes any path under the home dir as `~/…` in the log entries it adds, `factory docs check` warns `W-HOME-PATH` on a tracked file under `docs/` that holds a home path, and `docs/.gitignore` ignores `__pycache__/`. The matcher that finds a home path is one library function: slice 05 rewrites kept files with it and slice 06's upstream check reports hits with it.

**Highest risk**: the tests passing vacuously. Every fixture lives under `/tmp`, so a BLOCK body there never held `/home/` to begin with.

## Current code / environment

- **The driver's log writes**: the driver adds entries only through `answers::add_block` (`src/answers.rs:223-234`), `add_note` (`:236-240`) and `resolve` (`:242-247`). The BLOCK bodies the PRD names reach `add_block` from `src/checkpoint.rs:149,235,951`, `src/closeout.rs:2176` (via `block()`, `:2166`, which `:182,1153,1915` call) and `src/scheduler.rs:1142,1177,1302,2021`; notes from `src/checkpoint.rs:1015`, `src/closeout.rs:2320`, `src/scheduler.rs:1786,1904,1922`. The checkpoint's red-gate body names the transcript as `result.log.display()` (`src/checkpoint.rs:147`) and quotes gate output above it (`:134-139`).
- **Not `log::add`**: `src/docs/log.rs:195` also writes agents' `factory docs log add` entries (`src/cli/docs.rs:410-413`) and the owner's answers (`src/cli/answer.rs:307-315,466-471`). The PRD says "the driver writes", so the rewrite goes in the three `answers` wrappers, title and body.
- **The home dir**: `paths::home()` (`src/paths.rs:78-82`) falls back to `/` when `HOME` is unset or empty. Rewriting every `/` to `~/` would wreck the text, so skip the rewrite when home is `/`.
- **The fixtures never set `HOME`**: `tests/harness/mod.rs:530-551` sets `FACTORY_DATA_DIR` to `<base>/data` and leaves `HOME` as the real one. Set `HOME` to the fixture base on the run command, and the run dir falls under it.
- **The check**: `check::run` (`src/docs/check.rs:82`) ends with `archive_frozen(root, &mut r)` (`:151`), which runs `git` through the helper at `:680-691` and returns early outside a work tree (`:695-701`). W-SIZE (`:278-288`) is the warning template. The unit fixture `tree()` (`:842-858`) runs `git init` but never `git add`, so a file it writes is not tracked.
- **Non-UTF-8 files**: 3 tracked `.pyc` files under `docs/` hold home paths. Read bytes (the `regex` crate is a dependency, `Cargo.toml:31`; use `regex::bytes`).
- **This repo after the change**: 42 tracked `docs/` files match `/home/<name>/` or `/Users/<name>/` with a name of `[A-Za-z0-9._-]`, this feature's `prd.md` among them (`:121,126,146,207` are examples). None is under `docs/archive/`, which holds nothing yet. They warn until the first harvest (prd §3 FR-3 accepted cost).
- **The clean-path tests stay clean**: `tests/docs_check.rs:96-101` asserts `docs check: clean`, and its fixtures sit under `/tmp`.
- **`docs/.gitignore`**: `GITIGNORE_LINE = "docs.lock"` (`src/docs/scaffold.rs:23`); `init` appends it when missing (`:78-89`). `tests/docs_v3.rs:212-215` asserts the file is exactly `"docs.lock\n"`. `src/docs/migrate.rs:1723-1745` writes `docs.lock` for a migrated repo; leave it, a migrated repo gets the new line from `init`.
- **Deliberate non-changes**: *(prd §7 "Not handled: scratch paths (`/tmp/claude-…`, `/mnt/devcache/…`) in tracked docs. `W-HOME-PATH` looks for home paths only.")* The leaks scan keeps skipping `docs/` (`src/leaks.rs:766`).

## Tasks

### 1. The home-path matcher — high — blocked by: —
**Where**: a library home for it, e.g. `src/paths.rs` beside `home()`.

**Source**: §3 FR-3: "a tracked file under `docs/` that holds `/home/<name>/` or `/Users/<name>/`", and "The driver writes any path under the home dir as `~/…` in a log body."

Two functions: one finds the first `/home/<name>/` or `/Users/<name>/` in a text (bytes), the other rewrites the current user's home prefix plus `/` to `~/`. Slices 05 and 06 call them, so give them names that say what they do.

The trap: match at a path boundary. `/home/x` must not rewrite inside `/home/xy/`, and a mid-path `/tmp/home/x/` is not a home path.

**Acceptance:** unit tests on both functions: the boundary cases above, `/Users/<name>/`, and a `HOME` of `/` that rewrites nothing.

### 2. The driver writes `~/` — high — blocked by: 1
**Where**: `src/answers.rs:223-247`.

**Source**: §3 FR-3: "The driver writes any path under the home dir as `~/…` in a log body. The person answering a block is on the same machine, so `~/` still opens."

Rewrite the title and the body in `add_block`, `add_note` and `resolve` before `log::add`.

**Acceptance:** task 4's run test.

### 3. `W-HOME-PATH` and the ignore line — high — blocked by: 1
**Where**: `src/docs/check.rs:151` (a new function beside `archive_frozen`), `src/docs/scaffold.rs:23,78-89`, `tests/docs_v3.rs:212-215`, this repo's `docs/.gitignore`.

**Source**: §3 FR-3: "`factory docs check` warns `W-HOME-PATH` for a tracked file under `docs/` that holds `/home/<name>/` or `/Users/<name>/`: one line per file, with the first hit's line number. `docs/archive/` is skipped, because it is frozen and cannot be fixed." And: "`docs/.gitignore` gains `__pycache__/`."

List the files with `git ls-files -- docs` through the `git` helper, skip `docs/archive/`, read each as bytes, and warn once per file with `path:line`. Outside a work tree, skip as `archive_frozen` does. `init` appends each missing line, `docs.lock` and `__pycache__/`; update the docs_v3 assertion to both lines.

**Acceptance:** a unit test in `check.rs` that `git add`s `docs/features/001-x/slice-01-a.md` holding `/home/x/a` and the same line under `docs/archive/`, and gets one `W-HOME-PATH` naming the first file and its line. A second file holding `/tmp/home/x/` warns nothing.

### 4. The run test — high — blocked by: 2
**Where**: `tests/checkpoints.rs`, beside `three_reds_block_the_run_and_the_answer_reruns_the_gate` (`:1026`).

**Source**: §3 FR-3 acceptance: "A stub run blocked by a red gate writes a BLOCK body with `~/` and no `/home/`."

The same red-gate run, with `HOME` set to the fixture base on the run command. The BLOCK body names the transcript as `~/data/runs/…`, and `log.jsonl` holds no `/home/` and no copy of the fixture base path.

The trap: without the `HOME` override the test passes against today's code. Assert the body holds `~/` first, so a vacuous pass fails.

**Acceptance:** a test named for the behavior (e.g. `a_block_body_names_the_run_dir_under_the_home_as_tilde`) fails against today's code.

### 5. Verify — medium — blocked by: 1, 2, 3, 4
`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 6. Prove — at the checkpoint — blocked by: 5
Rows from [`prd.md`](prd.md) §5 this slice proves, below the UI:

- **FR-3**: "run: `factory docs check` on a fixture with a home path under `docs/features` and under `docs/archive`; a stub run blocked by a red gate, then `grep /home/` its log" — tasks 3 and 4's test transcripts, and `factory docs check` on this repo showing the warnings.

**Acceptance:** after the checkpoint, Progress names the transcripts.

**Code-review fixes (2026-10-15).**
- `W-HOME-PATH` read every tracked path under `docs/`, symlinks included. A cloned repo could point one at `/dev/zero` or a FIFO and hang `docs check`. It now reads regular files only; the check test adds a symlink to an outside file holding a home path and still expects one hit.
- The byte-regex class before a home path ran in Unicode mode, so a home path after a Latin-1 byte went unreported. The class is byte-wise now.
