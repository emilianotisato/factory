# Slice 02 — Docs check feature scope, ctx def filter, and leaks attr

**Source**: [`prd.md`](prd.md) → §6 (FR-4 CLI & code-analysis core). **Depends on**: — (independent; writes no files touched by Slice 01)
**Created**: 2026-11-27

**Progress**: _(written as it lands — what was built, the gate numbers, what a review caught)_

**Slice**: Delivers the CLI and code-analysis core of FR-4: optional `[FEATURE]` scoping on `factory docs check`, shared `linguist-vendored`/`linguist-generated` filtering for `ctx::def::find`, and `-c core.attributesFile=` isolation in `review::check_attr_src`. Leaves `closeout` local PR-body note, `skills/deliver/SKILL.md`, and the `0.20.0` bump to Slice 03.

## Current code / environment

- **`src/cli/docs.rs:65, 297-305`**: `Verb::Check` takes no arguments and calls `check::run_on(root, board_tree(root).as_deref())`.
- **`src/docs/check.rs:87-170, 808-839, 883-914`**: `run_on` validates project, all live features, all archived features, `archive_frozen`, `home_paths`, `links`, `ideas`, `SECOND_BOARD_NAMES`. Neither `home_paths` (`:808`) nor `links` (`:883`) currently takes a path prefix filter.
- **`src/ctx/refs.rs:50-77`**: `refs::find` skips `linguist-vendored`/`linguist-generated` files via `crate::review::check_attr` when `!opts.vendor`.
- **`src/ctx/def.rs:132-153`**: `def::find` calls `walk::walk(root, opts)` with no `check_attr` filter.
- **`src/review.rs:364-415`**: `check_attr_src` runs `Command::new("git").args(["check-attr", ...])` without `-c core.attributesFile=`, so local/global `core.attributesFile` (or `~/.config/git/attributes`) can suppress files in `leaks` / `review` / `ctx`.
- **Deliberate non-change**: Do NOT set `GIT_CONFIG_GLOBAL=/dev/null` in `check_attr_src` (that drops CI `safe.directory`, per prd §2 & §6). Do NOT add plan-title/phase-label heuristics to `factory leaks` *(prd §10 Non-goals decided against it.)*

## Tasks

### 1. Add optional `[FEATURE]` scope to `factory docs check` — high — blocked by: —
**Source**: §6 FR-4: "Change `Verb::Check` to `Check { feature: Option<String> }`... When `feature` is `Some(f)`: add `pub fn run_feature_on(root: &Path, board: Option<&Path>, feature: &str) -> Result<Report>` scanning live + archived folders (`board::scan_folder_on`) and resolving `f` (error if unknown): If `f` is archived: run only the `E-STATUS-PLACE` check for that archived folder (and duplicate-number check for `f.number`). If `f` is live: run `read_project`, duplicate-number check for `f.number`, `feature(root, &board_dir, &limits, target, &mut r)`, plus `home_paths` and `links` scoped via an optional `only_under: Option<&str>` prefix (`target.dir`)."

`src/cli/docs.rs`, `src/docs/check.rs`.
- In `src/docs/check.rs`:
  - Add `only_under: Option<&str>` parameter to `home_paths` and `links`:
    - In `home_paths`: when `Some(prefix)` (normalized with trailing `/`), only check tracked paths starting with `prefix`.
    - In `links`: when `Some(prefix)`, walk only `root.join(prefix)` (still skipping `archive/`).
  - Add `pub fn run_feature_on(root: &Path, board: Option<&Path>, query: &str) -> Result<Report>`:
    - Scan `docs/features` (live, using `board_features`) and `docs/archive` (archived) via `board::scan_folder_on`.
    - Resolve `query` across live + archived (matching by number like `"001"`/`"1"`, slug, or dir_name — reuse `board::Board::resolve` logic cleanly!). Return `Err` if not found.
    - Check duplicate number across `live.iter().chain(&archived)` ONLY for `target.number`.
    - If `target.kind == Kind::Archive`: run only the `E-STATUS-PLACE` check for `target` and return `Ok(r)`.
    - If `target.kind == Kind::Live`: run `read_project(root, &mut r)`, `feature(root, &board_dir, &limits, target, &mut r)`, `home_paths(root, Some(&target_rel), &mut r)`, and `links(root, Some(&target_rel), &mut r)` — skipping unrelated features, `docs/ideas/`, `archive_frozen`, and `SECOND_BOARD_NAMES`.
- In `src/cli/docs.rs`:
  - Change `Verb::Check` (`:65`) to `Check { feature: Option<String> }`.
  - In `native` (`:297-305`): when `feature` is `Some(f)`, call `check::run_feature_on(root, board_tree(root).as_deref(), &f)?`, else `check::run_on(root, board_tree(root).as_deref())`.
- Add unit tests in `src/docs/check.rs` covering single live feature check ignoring broken sibling feature & broken ideas, archived feature `E-STATUS-PLACE` check, unknown feature error, and no-arg full check unchanged.

**Acceptance:** Unit tests in `src/docs/check.rs` prove `run_feature_on(..., "001")` reports only feature `001` issues while ignoring broken sibling `002` and broken ideas, checks `E-STATUS-PLACE` on an archived feature, and returns `Err` on unknown feature (fails if scoping or `only_under` filtering is removed).

### 2. Share `linguist-generated` filtering with `ctx::def::find` — high — blocked by: —
**Source**: §6 FR-4: "Extract the gitattributes filtering block from `src/ctx/refs.rs:50-77` into a shared `pub(super) fn drop_generated(root: &Path, files: &mut Vec<(PathBuf, Lang)>, opts: &WalkOpts)` helper in `src/ctx/walk.rs`, and call it from BOTH `refs::find` and `def::find` when `!opts.vendor`."

`src/ctx/walk.rs`, `src/ctx/refs.rs`, `src/ctx/def.rs`.
- Move `repo_relative` + the `!opts.vendor` `crate::review::check_attr` filtering block from `src/ctx/refs.rs:50-77` into `pub(super) fn drop_generated(root: &Path, files: &mut Vec<(PathBuf, Lang)>, opts: &WalkOpts)` in `src/ctx/walk.rs`.
- Call `walk::drop_generated(root, &mut walked.files, opts)` in BOTH `src/ctx/refs.rs:60` and `src/ctx/def.rs:152`.
- Add a unit test in `src/ctx/def.rs` (using a tempdir git repo with `.gitattributes` marking `gen.rs linguist-generated`) proving `def::find` skips `gen.rs` when `opts.vendor == false` and finds the definition when `opts.vendor == true`.

**Acceptance:** Unit test in `src/ctx/def.rs` proves `def::find` omits definitions in a `linguist-generated` file by default and returns them with `WalkOpts { vendor: true }` (fails if `drop_generated` is not called in `def::find`).

### 3. Pass `-c core.attributesFile=` in `review::check_attr_src` and add integration tests — high — blocked by: 1, 2
**Source**: §6 FR-4: "Pass `[\"-c\", \"core.attributesFile=\"]` before `\"check-attr\"` in `check_attr_src` (do NOT set `GIT_CONFIG_GLOBAL=/dev/null`, so CI container `safe.directory` keeps working!)."

`src/review.rs`, `tests/cli_and_ctx_hardening.rs` (new).
- In `src/review.rs:382-386`: build `let mut args = vec!["-c", "core.attributesFile=", "check-attr"];` (keeping `--source <rev>` and `-z --stdin linguist-vendored linguist-generated` and the fallback when `--source` is unsupported). Do NOT set `GIT_CONFIG_GLOBAL=/dev/null`.
- Create `tests/cli_and_ctx_hardening.rs` testing:
  1. `factory docs check 001` passes clean when feature `001` is clean even though sibling feature `002` and `docs/ideas/bad.md` have errors, while `factory docs check` (no arg) exits `1` catching them.
  2. `factory ctx def` skips a symbol in a `linguist-generated` file by default and finds it with `--vendor`.
  3. Setting local `git config core.attributesFile <temp-file>` marking `src/leak.rs linguist-generated` does NOT hide a `PRD-1` hit from `factory leaks --base main` or `factory leaks --all`, while a `.gitattributes` committed on `main` (marking `src/gen.rs linguist-generated`) DOES skip `src/gen.rs`.
- **The trap**: For `--base main`, `leaks::scan` reads `.gitattributes` from `--source main`, so the positive test MUST commit `.gitattributes` on `main` before branching!

**Acceptance:** `cargo test --test cli_and_ctx_hardening` passes all 3 integration scenarios (fails if `docs check [FEATURE]`, `def::find` filtering, or `-c core.attributesFile=` is removed).

### 4. Verify — medium — blocked by: 1, 2, 3
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 5. Prove — at the checkpoint — blocked by: 4
None yet — FR-4 finishes in Slice 03 (`closeout` token-free local PR note + `skills/deliver/SKILL.md` + `0.20.0` bump) and is proven whole at Checkpoint 2. (Slice 02's CLI, `ctx def`, and `leaks` integration tests run in the full gate at Checkpoint 1.)

**Acceptance:** Full gate green at Checkpoint 1 including `tests/cli_and_ctx_hardening.rs`.
