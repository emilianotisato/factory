# Slice 03 — Authorized test snapshot and diff guard

**Source**: [`prd.md`](prd.md) → §7 FR-5, §9 QA plan. **Depends on**: 01 (uses `commit::resolve_base_ref` and modifies `src/closeout.rs` and `assets/autonomy.md` after slice 01). Independent of slice 02 (zero shared files).
**Created**: 2026-11-26

**Progress**: `testguard`, `RunState.authorized_tests`, checkpoint/closeout blocks + `allow` persistence, polish `prd.md` drop, `assets/autonomy.md`. Tests: `authorized_tests_frontmatter_rules`, `testguard::tests::*`, `authorized_tests_snapshot_blocks_unlisted_test_edits_and_persists_allow_for_run`. Inner loop: 1 run, 0 red, ~2 min. Deliberate reds: `authorized_tests_frontmatter_rules` when `prd_keys` validation removed; `deleted_test_file_violates…` when delete diff path parsing broken; integration when `sync_allow_answers` skipped on allow.

**Slice**: Adds optional flow-array `authorized_tests` validation to `docs check`, snapshots `authorized_tests` from `prd.md` into `RunState` at run start (on both immediate-start and ask-parked paths), refuses polish edits that alter `prd.md`'s `authorized_tests` frontmatter, and enforces a `-U0` diff guard (`src/testguard.rs`) on pre-existing test files at checkpoint and closeout — opening a `BLOCK-n` whose `"allow"` answer persists the approved paths onto `RunState.authorized_tests` for the rest of the run.

## Current code / environment

- **Frontmatter parser (`src/docs/frontmatter.rs:65-119`)**: already parses YAML flow arrays (`authorized_tests: ["tests/a.rs", "tests/b.rs::fn_name"]`) into `Value::List(Vec<String>)`. **Deliberate non-change (`prd.md` §11)**: YAML block-list syntax (`authorized_tests:\n  - ...`) was cut 2026-11-26 — do NOT modify `src/docs/frontmatter.rs`.
- **`prd_keys` (`src/docs/check.rs:384-452`)**: validates required keys on `f.prd` frontmatter and uses `invalid(r, prd, key, why)` to emit `E-FM-INVALID`. Unknown optional keys currently pass unchecked.
- **`RunState` and `Run::create` (`src/run.rs:95-138, :240-310`)**: `RunState` serializes to `~/.local/share/factory/runs/<id>/state.json` (outside the repo). `Run::create` is called at **two** sites in `src/cli/run.rs`: `:141` (ask-parked path) and `:174` (immediate-start path). Both paths must snapshot `authorized_tests` before any worker spawns.
- **Checkpoint driver (`src/checkpoint.rs:57-64, :105-122`)**: enters `run()`, checks `open_checkpoint_blocks`, then runs the gate loop until `!result.red`, scans leaks against `base = closeout::default_branch(run, docs)`, and breaks to spawn the checkpoint worker. Running `testguard::check` right after the gate turns green (using `commit::resolve_base_ref(&lane_dir, Some(&base))`) catches any unauthorized pre-existing test edit before the checkpoint worker runs or closes the checkpoint.
- **Closeout driver (`src/closeout.rs:120-127, :663-685, :1010-1080`)**:
  - In `land_worker` (`:668-680`), when `matches!(who, Closer::Polish)`, `Closer::Polish::protected` leaves `prd.md` writable for Status prose. Before `land::commit_lane`, compare `authorized_tests` parsed from `lane_dir.join(&prd_rel)` against `run.state().authorized_tests` (or base `prd.md` at `lane_base`): if polish changed `authorized_tests`, add `prd_rel` to the `protected` slice passed to `commit_lane` (so `prd.md` is dropped from the polish commit and logged via `c.dropped` — leaving `prd.md` untouched in the main tree).
  - In `run()` (`:120-126`), once `polished` is true, `ship()` runs (`:124`). Checking `testguard::check(&repo, &base_ref, &run.state().authorized_tests)` at the very start of `ship()` (BEFORE `docs.ship(&feature)` flips `state.toml`) ensures that if polish introduced an unauthorized test edit, closeout opens `CloseoutBlocked` and parks cleanly — and resuming after `"allow"` or a manual revert re-enters `ship()` directly without re-running polish.
- **Answering blocks (`src/cli/answer.rs:124-156`)**: `answer_block` has the resolved `entry: answers::BlockEntry` (with `entry.title` and `entry.body` — inspect `src/answers.rs` to confirm whether `BlockEntry` carries `body` or if `log::entries(&feature_dir)` gives the full body) and `mut run: Option<Run>`.
- **`assets/autonomy.md:62`**: currently `- skip, delete or weaken a test this run did not write`.
- **Leaks guard trap for new file `src/testguard.rs`**: `src/testguard.rs` is NOT in `docs/project.toml` `[leaks] allow` (and FR-1 protects `docs/project.toml`!). Never put plan-id shaped strings (`PRD-`, `FR-`, `BLOCK-1`, `slice-1`, etc.) in `src/testguard.rs` comments, identifiers, or string literals! Keep `BLOCK_PREFIX = "unauthorized test change:"` free of plan IDs.
- **Deliberate non-changes (`prd.md` §11)**:
  - Do NOT build the merge base to execute changed tests red-on-base.
  - Do NOT match inline `#[cfg(test)]` edits inside non-test production source files (`src/*.rs`).

## Tasks

### 1. Validate `authorized_tests` in `docs check` and snapshot it into `RunState` at run start — high — blocked by: —
**Source**: §7 FR-5: "Validate `authorized_tests` in `src/docs/check.rs` (`prd_keys`): if present, it must be a list of non-empty strings (no leading `/`, no `..` traversal, no globs `*`/`?`/`[`), else emit `E-FM-INVALID` ... Add `#[serde(default, skip_serializing_if = "Vec::is_empty")] pub authorized_tests: Vec<String>` on `RunState` ... plus `Run::set_authorized_tests` / `Run::add_authorized_tests` ... read `authorized_tests` from `docs/features/NNN-slug/prd.md` frontmatter and persist it onto `RunState` ... on both `src/cli/run.rs:141` and `:174`".

- `src/docs/check.rs:384-448`: in `prd_keys`, if `fm.get("authorized_tests")` is `Some(v)`:
  - Must be `Value::List(items)` (otherwise call `invalid(r, prd, "authorized_tests", "must be a list of test paths".into())`).
  - For each item in `items`: validate via a shared helper (e.g. `testguard::validate_entry(item)` or inline helper) that `item.trim()` is non-empty, does not start with `'/'`, does not contain `".."` path components, does not contain `'*'`, `'?'`, or `'['`, and if it contains `"::"`, both the path half (before `"::"`) and the test name half (after `"::"`) are non-empty. On failure emit `E-FM-INVALID`.
- `src/run.rs:95-138`: add `#[serde(default, skip_serializing_if = "Vec::is_empty")] pub authorized_tests: Vec<String>` to `RunState`, initialized to `Vec::new()` in `Run::create`. Add `Run::set_authorized_tests(&mut self, tests: Vec<String>) -> Result<()>` and `Run::add_authorized_tests(&mut self, paths: &[String]) -> Result<()>` (deduplicating while preserving order, persisting via `self.save()`).
- `src/cli/run.rs:141` and `:174`: extract a small helper `read_authorized_tests(&docs, &feature) -> Result<Vec<String>>` (reading `authorized_tests` from the feature's `prd.md` frontmatter via `crate::docs::frontmatter::parse`) and call `run.set_authorized_tests(authorized_tests.clone())?` immediately after `Run::create` on BOTH the ask-parked path (`:141`) and the immediate-start path (`:174`).

**Trap**: Both `Run::create` call sites (`src/cli/run.rs:141` AND `:174`) must snapshot `authorized_tests` — if only `:174` does it, any run that parks on a preflight `ASK-n` and resumes later loses its `authorized_tests` snapshot!

**Acceptance:** Unit/integration tests prove: (1) `docs check` accepts `authorized_tests: ["tests/a.rs", "tests/b.rs::fn_name"]` and emits `E-FM-INVALID` for `../x`, `/abs/test.rs`, `tests/*.rs`, `""`, and non-list values; (2) `Run::set_authorized_tests` / `add_authorized_tests` round-trip in `state.json` and omit the field when empty; (3) both immediate-start and ask-parked `factory run` paths snapshot `authorized_tests` into `state.json`. Removing validation in `prd_keys` or skipping `:141` breaks these tests.

### 2. Implement `src/testguard.rs` (`-U0` pre-existing test diff checker and `path::test_name` matcher) — high — blocked by: 1
**Source**: §7 FR-5: "Pre-existing test diff checker (`src/testguard.rs`, exported from `src/lib.rs`): `pub fn check(repo: &Path, base_ref: &str, authorized: &[String]) -> Result<Vec<Violation>>` ... `git diff -U0 --diff-filter=MD --no-renames <base_ref>...HEAD` ... `is_test_path(path)` ... deleted `D` ... modified `M` with 0 `-` lines allowed ... hunk with >= 1 `-` line allowed if exact `<path>` or `<path>::<test_name>` matches hunk header context or enclosing function declaration in `<base_ref>:<path>`".

- `src/lib.rs`: export `pub mod testguard;`.
- `src/testguard.rs`:
  - `pub const BLOCK_PREFIX: &str = "unauthorized test change:";`
  - `#[derive(Debug, Clone, PartialEq, Eq)] pub struct Violation { pub path: String, pub test_name: Option<String>, pub deleted: bool }`
  - `pub fn is_test_path(path: &str) -> bool`: true when `path` starts with `"tests/"`, `"test/"`, `"__tests__/"`, or `"spec/"`, or contains `"/tests/"` or `"/__tests__/"`, or its filename ends with `_test.<ext>`, `.test.<ext>`, `.spec.<ext>`, `Test.<ext>`, or `Tests.<ext>` (where `<ext>` is a non-empty extension after `.`).
  - `pub fn prd_authorized_tests(prd_path: &Path) -> Result<Vec<String>>`: parses `prd.md` frontmatter and returns the `authorized_tests` string list (empty if absent).
  - `pub fn check(repo: &Path, base_ref: &str, authorized: &[String]) -> Result<Vec<Violation>>`:
    1. Run `git -c core.quotepath=off diff -U0 --diff-filter=MD --no-renames --no-color --no-ext-diff --no-textconv --src-prefix=a/ --dst-prefix=b/ {base_ref}...HEAD` in `repo`.
    2. Parse per-file diffs (filtering by `is_test_path(path)`):
       - Detect deleted files (`deleted file mode` or `+++ /dev/null`): if `!authorized.iter().any(|a| a == path)`, emit `Violation { path, test_name: None, deleted: true }`.
       - For modified files (`M`): inspect each `@@ -old_start[,old_count] +new_start[,new_count] @@ [ctx]` hunk. Count removed lines (`-` lines not starting with `--- `). If `removed == 0`, skip hunk (purely additive).
       - If `removed > 0`:
         - If `authorized.iter().any(|a| a == path)`, allowed.
         - Else find the enclosing test name: first inspect `ctx` (the text after the closing `@@`) and/or scan upward in `git show {base_ref}:{path}` from `old_start` (1-indexed) for the nearest enclosing test function declaration matching `fn <name>`, `function <name>`, `def <name>`, `func <name>`, `test("<name>"` / `test('<name>'`, `it("<name>"` / `it('<name>'` (handling `async fn`, `pub fn`, etc.).
         - If any `authorized` entry equals `format!("{path}::{name}")` where either `ctx` contains `<name>` (as a token) or the upward-scanned enclosing test name equals `<name>`, allowed.
         - Otherwise emit `Violation { path, test_name: enclosing_name, deleted: false }` (deduplicating identical `(path, test_name, deleted)` tuples).
  - Helper functions for formatting/parsing block bodies cleanly so `answer` and resume can extract violated paths deterministically:
    - `pub fn format_block_body(violations: &[Violation]) -> String` (lists each violation with its exact path, optional `::test_name`, and options `1. allow ...` / `2. revert ...`, keeping exact path tokens parseable).
    - `pub fn violated_paths_from_body(body: &str) -> Vec<String>` (extracts the unique paths listed in the block body).

**Trap**: When scanning upward in `<base_ref>:<path>` for `old_start`, remember `-U0` hunks with `old_count > 0` start at `old_start` (1-based line number of the first removed line). If the removed line itself IS the `fn foo(` line, scanning from `old_start` down to line 1 (inclusive!) finds `foo` right away. Also ensure `src/testguard.rs` contains ZERO plan-id strings (`PRD-`, `FR-`, `BLOCK-`, `slice-`) so `factory leaks` stays clean.

**Acceptance:** Unit tests in `src/testguard.rs` prove: (1) `is_test_path` matches all specified patterns and rejects `src/config.rs` / `src/lib.rs`; (2) newly added test files (`A`) and purely additive hunks (0 `-` lines) in pre-existing test files produce zero violations; (3) removing/modifying lines in a pre-existing test file returns a `Violation` with `test_name` populated when unauthorized, returns empty when whole path or matching `path::test_name` is in `authorized`, and still returns a `Violation` when only a *different* `path::other_test` is in `authorized`; (4) deleting a pre-existing test file returns `Violation { deleted: true }` unless the whole path is in `authorized` (`path::fn` does NOT authorize deleting the whole file). Breaking any of these branches fails its unit test.

### 3. Enforce `testguard` at checkpoint & closeout, protect polish `prd.md` frontmatter, persist `"allow"`, and update `assets/autonomy.md` — high — blocked by: 2
**Source**: §7 FR-5: "In `Closer::Polish` closeout ... if the polish lane modified `prd.md` and its parsed `authorized_tests` frontmatter differs from `RunState.authorized_tests`, refuse to land that `prd.md` change ... At checkpoint (after gate green and before closing/landing) and at closeout (after polish lands and before `ship`): run `testguard::check` ... open a `BLOCK-n` (`CheckpointBlocked` / `CloseoutBlocked`) whose title starts with `TESTGUARD_BLOCK_PREFIX` ... When the owner answers `"allow"` ... violated paths from that block are persisted onto `RunState.authorized_tests` ... Update `assets/autonomy.md` (`Never, to get green`)".

- `src/testguard.rs`: add `pub fn sync_allow_answers(run: &mut Run, feature_dir: &Path) -> Result<()>` that reads `log::entries(feature_dir)` (or `answers`), finds every resolved `Kind::Block` whose title starts with `BLOCK_PREFIX` and whose `Kind::Answer` (referencing that block's id) has trimmed case-insensitive text `"allow"`, extracts `violated_paths_from_body(&block.body)`, and calls `run.add_authorized_tests(&paths)?`.
- `src/cli/answer.rs:124-156`: in `answer_block`, when `entry.title.starts_with(crate::testguard::BLOCK_PREFIX)` and `text.trim().eq_ignore_ascii_case("allow")`, immediately call `crate::testguard::sync_allow_answers(run, &feature_dir)?` (after `answers::resolve` lands the answer entry, before `deliver` wakes the driver) so `state.json` reflects the newly authorized paths right away!
- `src/checkpoint.rs:110-125`: right after `!result.red` (after gate turns green and `base = crate::closeout::default_branch(run, docs)` is computed):
  - Call `crate::testguard::sync_allow_answers(run, &feature_dir)?;` (covers both `cli::answer` and direct answer resolution in tests).
  - Resolve `base_ref = crate::commit::resolve_base_ref(&lane_dir, Some(&base))?;`.
  - Run `let violations = crate::testguard::check(&lane_dir, &base_ref, &run.state().authorized_tests)?;`.
  - If `!violations.is_empty()`: open a `BLOCK-n` via `answers::add_block(&feature_dir, None, Some("checkpoint"), &title, &body)?` where `title = answers::block_title(&format!("{} {}", testguard::BLOCK_PREFIX, summary))`, emit `EventKind::CheckpointBlocked { n, block_id, question }`, log it with `say`, and return `Ok(Outcome::Parked { id, what: park_line(&id, &title), fresh: true })`.
- `src/closeout.rs`:
  - In `land_worker` (`:666-681`), when `matches!(who, Closer::Polish)`: read `prd_rel = format!("{}/prd.md", board.dir)`. Build `let mut protected = who.protected(board);`. If `lane_dir.join(&prd_rel)` exists and `crate::testguard::prd_authorized_tests(&lane_dir.join(&prd_rel))` differs from `run.state().authorized_tests` (or fails to parse frontmatter), push `prd_rel` into `protected` before `land::commit_lane` so the polish lane's `prd.md` change is dropped and logged (leaving `prd.md` untouched in the main tree).
  - At the top of `ship()` (before `docs.ship(&feature)` flips `state.toml`): call `crate::testguard::sync_allow_answers(run, &feature_dir)?;`, resolve `base_ref = crate::commit::resolve_base_ref(&repo, Some(&default_branch(run, docs)))?;`, run `crate::testguard::check(&repo, &base_ref, &run.state().authorized_tests)?;`. If non-empty, open a `BLOCK-n` via `block(run, &feature_dir, "polish", &title, &body, out)?` (which emits `EventKind::CloseoutBlocked`) and return `Ok(Some(parked))`.
- `assets/autonomy.md:62`: update `- skip, delete or weaken a test this run did not write` to `- skip, delete or weaken a test this run did not write — unless the PRD's frontmatter \`authorized_tests\` list (snapshotted at run start) names it`.
- `tests/checkpoints.rs` and `tests/closeout.rs`: add `authorized_tests_snapshot_blocks_unlisted_test_edits_and_persists_allow_for_run` (plus a polish `prd.md` frontmatter tamper test and an authorized-pass test) proving:
  1. A checkpoint run where a slice edited an unlisted pre-existing `tests/existing_test.rs` opens a `BLOCK-n` (`CheckpointBlocked`) starting with `unauthorized test change:`.
  2. Answering `"allow"` appends `tests/existing_test.rs` to `RunState.authorized_tests`, and BOTH the resumed checkpoint AND a subsequent closeout (after another commit!) pass without re-blocking.
  3. Listing `tests/existing_test.rs` (or `tests/existing_test.rs::fn_name`) in `prd.md`'s `authorized_tests` lets the checkpoint pass with zero blocks.
  4. A polish worker editing `prd.md`'s `authorized_tests` frontmatter has `prd.md` dropped from the polish commit (`prd.md` stays untouched in main tree).
  5. `assets/autonomy.md` contains the exact updated line once.

**Trap**: In `src/closeout.rs`, compare polish's `prd.md` `authorized_tests` against the baseline snapshot (from `base` commit's `prd.md` or initial `run.state().authorized_tests`? Wait: if the owner earlier answered `"allow"` to a `BLOCK-n` during a checkpoint, `run.state().authorized_tests` has grown beyond `prd.md`'s original list!). Good catch — compare `lane_dir/prd.md`'s `authorized_tests` against `repo/prd.md`'s `authorized_tests` (the main tree before polish lands, since `prd.md` is protected from every non-polish worker!). That way polish can freely update `prd.md` Status prose even after an earlier `"allow"` answer, while any edit polish makes to `prd.md`'s `authorized_tests` frontmatter is caught and dropped!

**Acceptance:** `authorized_tests_snapshot_blocks_unlisted_test_edits_and_persists_allow_for_run` passes and fails if checkpoint skips `testguard::check`, if `"allow"` is not persisted onto `RunState.authorized_tests` (re-blocking on closeout), if polish is allowed to mutate `prd.md`'s `authorized_tests`, or if the `assets/autonomy.md:62` clause is removed.

### 4. Verify — medium — blocked by: 1, 2, 3
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 5. Prove — at the checkpoint — blocked by: 4
**Source**: §9 QA plan row FR-5:
- FR-5: `run authorized_tests_snapshot_blocks_unlisted_test_edits_and_persists_allow_for_run` (evidence: test transcript)

**Acceptance:** after the checkpoint, the test runs under `cargo test` with full gate green and transcript recorded in Progress.
