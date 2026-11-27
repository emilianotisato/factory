# Slice 02 — Honor .gitattributes generated/vendored in leaks and ctx refs

**Source**: [`prd.md`](prd.md) → §4 (FR-2). **Depends on**: — _(independent of Slices 01 and 03–05)_
**Created**: 2026-11-27

**Progress**: checkpoint prove: `cp-1/FR-2/leaks.txt`, `cp-1/FR-2/ctx.txt`.

**Slice**: Shares `review::check_attr` (plus a new `check_attr_at(root, source, paths)` using `git check-attr --source <tree-ish>`) across `factory leaks` and `factory ctx refs`, so `.gitattributes` `linguist-generated` and `linguist-vendored` marks are honored once in all three verbs — with `leaks --base` reading attributes strictly from `<base_ref>` so a branch cannot silence its own leaks scan.

## Current code / environment

- **`review::check_attr`**: `src/review.rs:351-386` — currently private `fn check_attr(root: &Path, paths: &[&str]) -> Result<Vec<(String, &'static str)>>` running `git check-attr -z --stdin linguist-vendored linguist-generated`. Live git (`2.56.0`) supports `git check-attr --source <tree-ish> -z --stdin linguist-vendored linguist-generated`.
- **`leaks::scan` & `Report`**: `src/leaks.rs:541-613, :837-896`.
  - `scan()` resolves `base_ref` and reads `allowlist_at_ref(repo, &base_ref)` (`:846`) for `Input::Base(base)` so a branch cannot weaken its own scan.
  - Parses `lines = parse_diff(...)` and `paths = parse_name_status(...)`, then calls `check(&allow, &lines, &paths)`.
- **`ctx::walk` & `ctx::refs`**: `src/ctx/walk.rs:48-91` and `src/ctx/refs.rs:49-105`.
  - `walk::walk(root, opts)` walks files respecting `opts.vendor` (`--vendor`). `ROOT_DIRS` (`dist`, `build`, `storage`) only skips depth 1, so `public/build/` is walked.
- **`[leaks] allow` list & `testguard`**:
  - `tests/leaks.rs` and `tests/ctx.rs` are ALREADY in `docs/project.toml` `[leaks] allow`, so adding new test functions there avoids false leaks hits from fixture strings!
  - Neither file is in `authorized_tests` — add ONLY new test functions at the bottom and add any new `use` imports on their own lines (never edit existing functions or merge existing top-level `use` lines!).
- **Deliberate non-changes** — *(prd §11 Non-goals)*:
  - Do NOT add a minified-line length heuristic; `.gitattributes` is the single source of truth.

## Tasks

### 1. Expose `review::check_attr` and `review::check_attr_at` — high — blocked by: —
**Source**: §4 FR-2: "Expose `pub fn check_attr(...)` AND `pub fn check_attr_at(root: &Path, source: &str, paths: &[&str]) -> Result<Vec<(String, &'static str)>>` running `git check-attr --source <source> -z --stdin linguist-vendored linguist-generated`".

`src/review.rs`.
1. Factor the body of `check_attr` (`:351-386`) into a shared helper taking `source: Option<&str>`: when `Some(rev)`, pass `["check-attr", "--source", rev, "-z", "--stdin", "linguist-vendored", "linguist-generated"]`; when `None`, keep existing `["check-attr", "-z", "--stdin", "linguist-vendored", "linguist-generated"]`.
2. Export `pub fn check_attr(root: &Path, paths: &[&str]) -> Result<Vec<(String, &'static str)>>` and `pub fn check_attr_at(root: &Path, source: &str, paths: &[&str]) -> Result<Vec<(String, &'static str)>>`.
3. Add a unit test in `src/review.rs` proving `check_attr_at` reads `.gitattributes` from the given tree-ish rather than uncommitted/branch edits.

**The trap**: Keep `paths.is_empty()` returning `Ok(Vec::new())` early without spawning `git`, and keep NUL-delimited stdin writing identical to today's `check_attr`.

**Acceptance:** `check_attr_at(repo, base_sha, &["src/a.js"])` ignores a `.gitattributes` rule added after `base_sha` (fails if `--source` is omitted).

### 2. Skip base-marked generated/vendored paths in `leaks::scan` and report them on `Report` — high — blocked by: 1
**Source**: §4 FR-2: "Skip base-marked paths in `factory leaks` (`src/leaks.rs:541-613, :837-896`)".

`src/leaks.rs`.
1. Add `pub skipped: Vec<(String, &'static str)>` (default empty) to `Report` (`:542-547`).
2. In `Report::render` (`:580-612`): when `!self.skipped.is_empty()`, emit:
   ```text
   skipped:
     <path> (<reason>)
   ```
   right before `comment share:` (preserving exact byte-for-byte existing output when `self.skipped.is_empty()`!).
3. In `leaks::scan` (`:837-896`):
   - Keep `base_ref` resolved for `Input::Base(base)` (avoid resolving twice!).
   - Collect sorted, deduplicated repo-relative paths present across `lines` and `paths`.
   - Call `let marked = crate::review::check_attr_at(repo, source, &distinct)?;` where `source` is `&base_ref` for `Input::Base` and `"HEAD"` for `Input::All` (if `distinct.is_empty()`, skip the call).
   - Build a `HashSet<&str>` of marked paths; filter `lines` and `paths` to exclude marked paths before calling `check(&allow, &lines, &paths)`.
   - Set `report.skipped = marked;` (sorted by path).

**The trap**: For `Input::Base(base)`, MUST pass `&base_ref` to `check_attr_at` — NEVER read working-tree `.gitattributes`, or a branch can silence its own leaks scan by adding `src/** linguist-generated`! Also make sure `Report::render` emits zero extra blank lines when `self.skipped.is_empty()` so all existing `tests/leaks.rs` output assertions stay green!

**Acceptance:** `leaks::scan` skips base-marked `linguist-generated`/`vendored` paths, lists them under `skipped:`, and ignores branch-only `.gitattributes` additions (fails if `--source <base_ref>` is replaced with working-tree `check_attr`).

### 3. Skip `.gitattributes` generated/vendored paths in `ctx::refs` unless `--vendor` is set — high — blocked by: 1
**Source**: §4 FR-2: "Skip marked paths in `factory ctx refs` (`src/ctx/walk.rs:48-91`, `src/ctx/refs.rs:49-105`)".

`src/ctx/refs.rs` (and/or `src/ctx/walk.rs`).
1. In `ctx::refs::find` (`:49-105`, right after `let mut walked = walk::walk(root, opts);`):
   - When `!opts.vendor && !walked.files.is_empty()`:
     - If `let Ok(repo_root) = crate::paths::repo_root(root)`:
       - Canonicalize/strip `repo_root` cleanly to get repo-relative paths for files under `repo_root`.
       - Call `if let Ok(marked) = crate::review::check_attr(&repo_root, &rel_refs)` (gracefully ignoring non-git/error cases), collect marked relative paths into a `HashSet`, and `walked.files.retain(...)` to drop every marked file before `walk::scan`!
2. When `opts.vendor` (`--vendor`) is `true`, do NOT filter by `.gitattributes` — include marked files!

**The trap**: `root` passed to `ctx::refs::find` can be a subdirectory of `repo_root` OR a directory outside any git repo (in tests). Handle `repo_root(root).is_err()` by skipping the `check_attr` filter cleanly, and strip `repo_root` (matching canonical/lexical form of `walked.files`) so `git check-attr` receives repo-relative forward-slash paths!

**Acceptance:** `ctx refs <sym>` skips a file under `public/build/` marked `linguist-generated` in `.gitattributes`, while `ctx refs --vendor <sym>` finds it (fails if `check_attr` filtering is omitted or applied when `opts.vendor == true`).

### 4. Add integration tests in `tests/leaks.rs` and `tests/ctx.rs` — high — blocked by: 2, 3
**Source**: §4 FR-2 Acceptance & §9 QA plan FR-2 row.

`tests/leaks.rs`, `tests/ctx.rs` (both already in `[leaks] allow`; append new tests only — do NOT touch existing tests or merge existing top-level `use` lines!).
1. In `tests/leaks.rs`, add `leaks_honors_base_gitattributes_and_blocks_branch_self_silencing`:
   - Base commit on `main` has `.gitattributes` with `public/build/** linguist-generated`.
   - Case 1 (generated-only branch): commit `public/build/app.js` containing an `id-string` hit (`"PRD-999"`) → `factory leaks --base main` exits `0`, stdout contains `clean` and `skipped:` + `public/build/app.js (generated)`.
   - Case 2 (generated + real hit): also commit `src/app.js` containing `"PRD-999"` → `factory leaks --base main` exits `1`, reports `1 fix hit` citing `src/app.js`, still skips `public/build/app.js (generated)`.
   - Case 3 (anti-tampering): on the branch, append `src/** linguist-generated` to `.gitattributes` and commit → `factory leaks --base main` STILL exits `1` and reports the `src/app.js` hit!
2. In `tests/ctx.rs`, add `ctx_refs_honors_gitattributes_generated_unless_vendor_flag_set`:
   - Repo with `.gitattributes` marking `public/build/** linguist-generated` and committed `public/build/app.js` defining/referencing `uniqueBundleSym` alongside `src/live.js`.
   - `factory ctx refs uniqueBundleSym` returns 0 hits (skips `public/build/app.js`).
   - `factory ctx refs --vendor uniqueBundleSym` finds `public/build/app.js`.

**Acceptance:** `cargo test --test leaks leaks_honors_base_gitattributes_and_blocks_branch_self_silencing` and `cargo test --test ctx ctx_refs_honors_gitattributes_generated_unless_vendor_flag_set` pass; removing `--source <base_ref>` or the `ctx::refs` filter breaks them.

### 5. Verify — medium — blocked by: 4
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 6. Prove — at the checkpoint — blocked by: 5
**Source**: §9 QA plan FR-2 row.
- **FR-2**: run `cargo test --test leaks leaks_honors_base_gitattributes_and_blocks_branch_self_silencing -- --nocapture && cargo test --test ctx ctx_refs_honors_gitattributes_generated_unless_vendor_flag_set -- --nocapture` and keep the transcript. Expected: passes proving base `.gitattributes` skipping, branch anti-tampering, and `ctx refs` vs `--vendor`.

**Acceptance:** after the checkpoint, the FR-2 row has transcript evidence in this slice's **Progress**.
