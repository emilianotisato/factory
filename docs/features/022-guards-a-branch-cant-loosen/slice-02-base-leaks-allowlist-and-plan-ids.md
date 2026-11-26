# Slice 02 — Base-ref leaks allowlist and hyphen-less plan IDs

**Source**: [`prd.md`](prd.md) → §4 FR-2, §6 FR-4, §9 QA plan. **Depends on**: 01 (uses `commit::resolve_base_ref` introduced in slice 01 and writes `src/leaks.rs` after slice 01 wired `resolve_base_ref` into `leaks::scan`). Independent of slice 03 (zero shared files).
**Created**: 2026-11-26

**Progress**: `parse_allowlist` + `allowlist_at_ref`; `--base` scans with base allowlist and `allowlist_note` above `comment share:`; `ids!()` / `SEMVER` per FR-4; tests `leaks_base_uses_base_allowlist_and_notes_branch_edits` and `leaks_catches_hyphenless_unambiguous_plan_ids_without_ambiguous_false_positives`. Inner loop: 2 runs, 1 red (clippy `ci::Output.code`), ~3.5 min green. Deliberate reds: base stub used worktree allow → exit 0 not 1; old `ids!()` → missed `"fr123-7"`. Fixture allow entries use `scope = "all"` so comment hits suppress on `--all`.

**Slice**: Makes `factory leaks --base <ref>` read `[leaks] allow` from `<base_ref>:docs/project.toml` with a one-line notice when the branch modified `[leaks] allow`, and extends `ids!()` in `src/leaks.rs` so strings and comments containing hyphen-less unambiguous plan IDs (`fr123-7`, `prd025`, `oq12`, `block101`) are caught while ambiguous short prefixes (`cp1252`, `dec2026`, `ad1`, `open2`, `ask1`) and semver (`block1.2.3`) stay quiet.

## Current code / environment

- **`allowlist` (`src/leaks.rs:444-498`)**: reads `docs/project.toml` from `root` on disk and parses `[leaks] allow` into `Vec<Allow>`. `scan(repo, input)` (`src/leaks.rs:787-833`) calls `allowlist(repo)?` unconditionally for both `Input::All` and `Input::Base(base)`.
- **`Report` and `Report::render` (`src/leaks.rs:517-583`)**: `Report` has `hits`, `comment_lines`, `added_lines`. `Report::render` prints hit groups, then `comment share: ...\n`, then `clean\n` / `1 fix hit\n` / `{n} fix hits\n`. Adding `pub allowlist_note: Option<String>` (with `#[derive(Debug, Default)]`) keeps existing struct initialization with `..Default::default()` or field updates clean.
- **Git helper in `src/leaks.rs`**: `src/leaks.rs` already has private `git(repo, args)` and `git_bytes` helpers; check how non-zero exit codes are handled when calling `git show <base_ref>:docs/project.toml` (a repo whose base commit predates `docs/project.toml` must return `Ok(Vec::new())`, NOT an error — whereas an invalid `<base_ref>` must return `Err`).
- **`ids!()` and `SEMVER` (`src/leaks.rs:41-45, :159`)**: `ids!()` is currently `r"\b(?:PRD|FR|AD|CP|OPEN|OQ|DEC|BLOCK|ASK)-\d+|\bslice[- ]?\d+"`, and `SEMVER` is at `:159`. `name_hit` (`:198-226`) already catches hyphen-less forms in identifiers; `id-string` and `id-comment` use `ids!()`.
- **Existing tests (`src/leaks.rs` unit tests and `tests/leaks.rs`)**: all must stay green. Note that `src/leaks.rs` and `tests/leaks.rs` are already listed in `docs/project.toml` `[leaks] allow`.
- **Deliberate non-changes (`prd.md` §11)**: do not add a multi-line leak splitter, do not cache `--all` scans, and do not touch `C99`/`Dec-2026` tuning.

## Tasks

### 1. Read `[leaks] allow` from `<base_ref>:docs/project.toml` on `Input::Base` with change notice — high — blocked by: —
**Source**: §4 FR-2: "Extract `parse_allowlist(label: &str, text: &str) -> Result<Vec<Allow>>` ... Add `pub fn allowlist_at_ref(repo: &Path, git_ref: &str) -> Result<Vec<Allow>>` ... In `leaks::scan(repo, input)`: For `Input::All`: keep `allow = allowlist(repo)?` (no notice). For `Input::Base(base)`: Resolve `base_ref = commit::resolve_base_ref(repo, Some(base))?`, Read `allow = allowlist_at_ref(repo, &base_ref)?` ... Compare `allow` against the worktree's `allowlist(repo)` ... set `report.allowlist_note = Some(format!("note: [leaks] allow changed on this branch; scanned with {base_ref}'s allowlist"))`. In `Report::render` ... print `{note}\n` right above `comment share:`."

- `src/leaks.rs:444-498`: extract `fn parse_allowlist(label: &str, text: &str) -> Result<Vec<Allow>>` from `allowlist(root)` so filesystem and git-blob paths produce identical validation and error messages (using `label` in error formatting).
- `src/leaks.rs`: add `pub fn allowlist_at_ref(repo: &Path, git_ref: &str) -> Result<Vec<Allow>>`:
  - First verify `git_ref` resolves as a commit (`commit::git_code(repo, &["rev-parse", "--verify", "--quiet", &format!("{git_ref}^{{commit}}")])? == 0`), returning `Err(Error::Msg(...))` if not.
  - Check whether `docs/project.toml` exists in `git_ref`'s tree (`git cat-file -e {git_ref}:docs/project.toml` or checking `git ls-tree` / `git show` exit when absent): if absent in that commit, return `Ok(Vec::new())`.
  - Otherwise read blob text via `git show {git_ref}:docs/project.toml` and pass to `parse_allowlist(&format!("{git_ref}:docs/project.toml"), &text)`.
- `src/leaks.rs:517-583`: add `pub allowlist_note: Option<String>` on `Report`; in `Report::render`, if `Some(note)`, emit `format!("{note}\n")` immediately before `comment share:`. Ensure `allowlist_note` never affects `fix_hits()` or `factory leaks` exit codes.
- `src/leaks.rs:787-833`: in `scan(repo, input)`:
  - `Input::All`: `allow = allowlist(repo)?`, `allowlist_note = None`.
  - `Input::Base(base)`: resolve `base_ref = crate::commit::resolve_base_ref(repo, Some(base))?`, read `allow = allowlist_at_ref(repo, &base_ref)?`, scan `<base_ref>...HEAD`, and compare against `allowlist(repo)`: if `allowlist(repo)` returns `Ok(worktree_allow)` with `worktree_allow != allow` OR returns `Err(_)` (branch edited `docs/project.toml` into invalid TOML/allowlist), set `report.allowlist_note = Some(format!("note: [leaks] allow changed on this branch; scanned with {base_ref}'s allowlist"))`.
- `tests/leaks.rs`: add `leaks_base_uses_base_allowlist_and_notes_branch_edits` covering all 4 acceptance cases:
  1. Branch adds a leak hit AND adds its path to `[leaks] allow` in `docs/project.toml` → `factory leaks --base main` exits `1`, reports the hit, and prints `note: [leaks] allow changed on this branch; scanned with main's allowlist` above `comment share:`.
  2. Allowlist entry already present on `main` → `factory leaks --base main` exits `0` with zero notice.
  3. `main` has no `docs/project.toml` at all → `factory leaks --base main` succeeds with an empty allowlist (not exit `2`).
  4. `factory leaks --all` uses the worktree's `docs/project.toml` (suppressing the allowlisted hit) with no notice.

**Trap**: Do NOT swallow invalid `git_ref` errors as "file missing in tree" — verify `{git_ref}^{commit}` first so typos still return exit `2`! Also make sure `Allow` derives `PartialEq, Eq` so `worktree_allow != allow` compares cleanly.

**Acceptance:** `leaks_base_uses_base_allowlist_and_notes_branch_edits` passes and fails if `scan` reads `allowlist(repo)` on `Input::Base` (case 1 would exit `0` and miss the notice) or if missing `docs/project.toml` on base returns `Err` (case 3 would exit `2`).

### 2. Catch hyphen-less unambiguous plan IDs in `ids!()` and update `SEMVER` — high — blocked by: —
**Source**: §6 FR-4: "Update `ids!()` (`src/leaks.rs:41-45`) to `r"\b(?:PRD|FR|OQ|BLOCK)[-_]?\d+|\b(?:AD|CP|OPEN|DEC|ASK)-\d+|\bslice[- ]?\d+"` ... Update `SEMVER` (`src/leaks.rs:159`) to `r"(?i)\b(?:PRD|FR|AD|CP|OPEN|OQ|DEC|BLOCK|ASK|slice)[-_ ]?\d+\.\d+\.\d+"`".

- `src/leaks.rs:41-45`: update `ids!()` to the exact regex from FR-4.
- `src/leaks.rs:159`: update `SEMVER` to the exact regex from FR-4 so semver tokens like `block1.2.3` or `slice-1.0.4` stay quiet.
- `src/leaks.rs` (`#[cfg(test)] mod tests`): add `leaks_catches_hyphenless_unambiguous_plan_ids_without_ambiguous_false_positives` testing both `id-string` and `id-comment` on:
  - Must hit: `"fr123-7"`, `"fr123"`, `"FR_4"`, `"prd025"`, `"oq12"`, `"block101"`
  - Must stay quiet: `"cp1252"`, `"dec2026"`, `"ad1"`, `"open2"`, `"ask1"`, `"block1.2.3"`

**Trap**: Keep `AD`, `CP`, `OPEN`, `DEC`, and `ASK` hyphen-required — relaxing them would break real code containing `cp1252`, `dec2026`, `ad1`, etc.

**Acceptance:** `leaks_catches_hyphenless_unambiguous_plan_ids_without_ambiguous_false_positives` passes and fails if `ids!()` requires `-` on `PRD|FR|OQ|BLOCK` (missing `"fr123-7"` / `"prd025"` / `"block101"`), or if `SEMVER` still requires `-` (flagging `"block1.2.3"`).

### 3. Verify — medium — blocked by: 1, 2
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 4. Prove — at the checkpoint — blocked by: 3
**Source**: §9 QA plan rows FR-2 and FR-4:
- FR-2: `run leaks_base_uses_base_allowlist_and_notes_branch_edits` (evidence: test transcript)
- FR-4: `run leaks_catches_hyphenless_unambiguous_plan_ids_without_ambiguous_false_positives` (evidence: test transcript)

**Acceptance:** after the checkpoint, both tests run under `cargo test` with full gate green and transcripts recorded in Progress.
