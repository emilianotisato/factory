# Slice 02 — Standalone filtered grep hook and denial cap

**Source**: [`prd.md`](prd.md) → §3 FR-1, §4 FR-2 (hook denial cap + combined FR-2 proof), §9 QA plan. **Depends on**: 01 (uses `src/ctx/print.rs` caps from slice 01 so the combined FR-2 proof verifies all three caps end-to-end).
**Created**: 2026-11-26

**Progress**: _(written as it lands — what was built, the gate numbers, what a review caught)_

**Slice**: Updates the PreToolUse grep hook so it denies a bare-name `grep`/`rg` only when no pipe or following command exists, no file/type filter flags are passed, target directories are outside `VENDOR_DIRS` and repo-root `ROOT_DIRS`, and `ctx` actually finds at least one ref or def — capping hook denial reasons at 8 KB (`MAX_DENY_BYTES`) and adding string-literal quoting guidance to `grep_hint()`. Leaves Bash `guard` `.push`/interpreter fixes, widened `process_call`, the regression corpus, and skill updates to slice 03.

## Current code / environment

- **`src/shell.rs:59-62, :71-74, :101, :156-192`**: `tokens()` emits `Tok::Op(";")` for `;` and `\n`, and `Tok::Op("&")` for `&`. `first_command_in(cmd)` returns `(words, end, cds)` after stripping leading `cd`/`pushd`, stopping at the first `Tok::Op`. It does NOT currently tell the caller whether any command/pipe follows `end` (so `grep -rn Foo src;` and `grep -rn Foo src\n` have `end = Some(";")` with nothing after, whereas `grep -rn Foo src | head` and `cd repo && grep -rn Foo src && sed ...` have a pipe or following command).
- **`src/cli/hook.rs:18-23, :111-152, :639-872`**:
  - `grep_hint()` (`:18-23`) currently ends with `"a bare name goes to ctx, grep anything else"`.
  - `bash()` (`:132-139`) calls `bare_name(search, &here, repo)` regardless of pipes or chained commands.
  - `grep_search` (`:715-776`) parses `--include`, `--exclude`, `--exclude-dir`, `--exclude-from` into `WITH_ARG` and discards them.
  - `rg_search` (`:809-872`) parses `-g`, `--glob`, `--iglob`, `-t`, `--type`, `-T`, `--type-not` and discards them.
  - `bare_name` (`:670-683`) checks `VENDOR_DIRS` only (`:698-706`), not repo-root `ROOT_DIRS` (`dist`, `build`, `storage`).
  - `answered()` (`:639-667`) searches `here` (never scoping to a single concrete target dir like `docs` or `src/Http`), returns `hint` alone when `r.total == 0 && d.hits.is_empty()` (`:658-660`), and has no total byte cap.
- **Unit tests in `src/cli/hook.rs:998-1222`**: five unit tests (`recursive_grep_over_the_repo_is_denied_with_the_verb`, `a_denied_grep_carries_what_ctx_finds`, `only_a_bare_name_outside_vendor_code_goes_to_ctx`, `grep_operands_are_directories_only_when_they_are`, `a_leading_cd_moves_where_paths_resolve`) use `repo()` which writes `"x\n"` into every file. Seed real `foo` and `Recon` code into `src/one.php`, `src/Http/Ctrl.php`, and `app/App.php` so all valid bare-name deny assertions (`-eRecon`, `--regexp=`, `\bRecon\b`, `cd src && rg foo Http`, `grep -r foo *`) stay denied while newly-allowed cases (`--include`, `| head`, `storage`, `nothing_here`) move to allow assertions.
- **`tests/cli.rs:100-115`**: `hook_denies_repo_greps_and_whole_reads_of_held_files` pins the old `grep_hint()` text and expects a deny on an empty `src/`. It is explicitly authorized in `prd.md` frontmatter (`tests/cli.rs::hook_denies_repo_greps_and_whole_reads_of_held_files`) so updating its fixture (`src/one.php` containing `foo`) and expected hint passes `testguard`.
- **Deliberate non-changes (`prd.md` §2, §11)**: do NOT edit `assets/lookup.md` (`tests/loop.rs:398` pins `"grep anything else"`), do NOT touch `guard()` / `process_call()` / `mentions()` / `tests/hook.rs` (slice 03), and keep `src/shell.rs` free of plan-id strings.

## Tasks

### 1. Distinguish standalone commands (allowing trailing `;`/`\n`/`&`) from piped/chained commands — high — blocked by: —
**Source**: §3 FR-1: "add a small helper (e.g. `shell::first_command_standalone` or return `has_more: bool` alongside `first_command_in`) that reports whether any pipe (`|`, `|&`), logical operator (`&&`, `||`), or subsequent command word follows the first command after stripping leading `cd`/`pushd`".

- `src/shell.rs:153-192`: add a helper (e.g. `pub fn first_command_standalone(cmd: &str) -> (FirstCommand, bool)`, keeping `first_command` and `first_command_in` signatures intact for all existing callers) that inspects the remaining tokens after the first non-`cd` command:
  - Standalone (`true`) when `end` is `None`, OR `end` is `Some(";" | "&")` AND the remaining slice `&toks[i..]` contains NO `Tok::Word` and NO pipe/logical operator (only optional trailing `;`/`&` ops).
  - Not standalone (`false`) when `end` is `Some("|" | "|&" | "&&" | "||")` OR any `Tok::Word` follows a `;`/`&` terminator.
- Add unit tests in `src/shell.rs` proving `grep -rn foo src`, `grep -rn foo src;`, `grep -rn foo src\n`, and `cd repo && grep -rn foo src` are standalone (`true`), while `grep -rn foo src | head`, `grep -rn foo src && echo ok`, `cd repo && grep -rn foo src && sed -n 1,20p x`, and `grep -rn foo src; ls` are NOT standalone (`false`).

**Trap**: `grep -rn foo src\n` ends with `Tok::Op(";")` because `tokens()` maps `\n` to `Tok::Op(";")` (`src/shell.rs:59-62`). Checking `end.is_none()` alone would wrongly treat newline-terminated commands as non-standalone!

**Acceptance:** Unit tests in `src/shell.rs` pass and fail if `end.is_none()` is used instead (which breaks `grep -rn foo src\n` and `grep -rn foo src;`) or if chained commands return `true`.

### 2. Restrict grep denials to standalone, unfiltered, searchable repo code with ctx hits, and cap denial reasons at 8 KB — high — blocked by: 1
**Source**: §3 FR-1 & §4 FR-2: "only evaluate the bare-name `ctx` redirect when no pipe or following command exists ... `grep_search` returns `None` if `--include`/`--exclude`/`--exclude-dir`/`--exclude-from` ... `rg_search` returns `None` if `-g`/`--glob`/`--iglob`/`-t`/`--type`/`-T`/`--type-not` ... reject any path under `VENDOR_DIRS` OR directly under repo-root `ROOT_DIRS` ... scope single concrete dir target to `target` ... when `r.total == 0 && d.hits.is_empty()` or ctx errors, return `None` (`Verdict::Allow`) ... update `grep_hint()` ... `pub const MAX_DENY_BYTES: usize = 8 * 1024;`".

- `src/cli/hook.rs`:
  - Update `grep_hint()` (`:18-23`) to:
    `format!("use: {} ctx refs <word> (or ctx def <symbol>; --vendor for vendor code): a bare name goes to ctx; for string literals grep with quotes (e.g. grep '\"Word\"')", paths::factory_command())`.
  - Add `pub const MAX_DENY_BYTES: usize = 8 * 1024;` and `const TRUNC_SUFFIX: &str = "\n… (truncated; run ctx with --in or --max to narrow)";`.
  - In `grep_search` (`:715-776`): return `None` if any long flag is `"include" | "exclude" | "exclude-dir" | "exclude-from"`.
  - In `rg_search` (`:809-872`): return `None` if any long flag is `"glob" | "iglob" | "type" | "type-not"` or any short flag in a cluster is `'g' | 't' | 'T'`.
  - In `bare_name` (`:670-714`): import `ROOT_DIRS` alongside `VENDOR_DIRS` from `crate::ctx::walk`. Reject when any target path is under `VENDOR_DIRS` (any component relative to `repo`) OR directly under repo-root `ROOT_DIRS` (first component relative to `repo`, e.g. `storage`, `storage/logs`, `dist`, `build` — NOT `src/build`). Return both `name: &'a str` and `search_root: PathBuf`:
    - If `s.paths.len() == 1` and `!s.paths[0].contains(['*', '?', '['])` and `absolute(s.paths[0], here)` is an existing dir under `repo` → `search_root = PathBuf::from(full)`.
    - Otherwise (empty `s.paths`, globs like `*`, or multiple paths like `src app`) → `search_root = here` clamped to `repo` (if `here` is under `repo`, else `repo`).
  - In `bash()` (`:111-152`): call `shell::first_command_standalone(command)`; run the bare-name `ctx` check ONLY when `standalone` is `true`. If `answered(name, &search_root, repo)` returns `Some(reason)`, return `Verdict::Deny(reason)`, else `Verdict::Allow`.
  - In `answered(name: &str, search_root: &Path, repo: &Path) -> Option<String>` (`:639-667`):
    - Call `refs::find(repo, search_root, name, caps, &opts)` and `def::find(repo, search_root, name, None, &opts)` (note `base = repo` or `here` when `here` is under `repo`, matching existing relative-path formatting in `a_denied_grep_carries_what_ctx_finds`!).
    - On error or `r.total == 0 && d.hits.is_empty()` → return `None`.
    - Format the denial reason; if `reason.len() > MAX_DENY_BYTES`, truncate on a valid UTF-8 char/line boundary so `truncated.len() + TRUNC_SUFFIX.len() <= MAX_DENY_BYTES` and append `TRUNC_SUFFIX`.
  - Update the 5 existing unit tests in `src/cli/hook.rs:998-1222` (`repo()` seeded with real `foo`/`Recon` PHP code; `--include`, `| head`, `storage`, `nothing_here` moved to allow assertions) and update `tests/cli.rs:100-115` (`hook_denies_repo_greps_and_whole_reads_of_held_files`: seed `foo` in `src/one.php` and match the new hint prefix/text).
  - Add two dedicated proof tests (e.g. in `src/cli/hook.rs` or `tests/cli.rs` — purely additive in `tests/cli.rs` outside the authorized function):
    1. `grep_hook_denies_only_standalone_unfiltered_code_searches_with_ctx_hits` — proves standalone `grep -rn alpha src`, `grep -rn alpha src;`, `grep -rn alpha src\n`, `grep -r alpha *`, `grep -rn alpha src app`, `cd <repo> && rg alpha src` are **denied** with new hint + ctx output, while piped (`| head -20`), batched (`cd <repo> && grep -rn alpha src && echo done`), filtered (`--include=*.php`, `rg -t rs`, `rg -g '*.rs'`), repo-root `ROOT_DIRS` (`storage`, `storage/logs`), non-code dir (`docs`), and 0-hit / string-literal-only (`DirRecep` only in `"DirRecep"`) all return `Verdict::Allow`.
    2. `ctx_output_caps_line_width_def_count_and_hook_denial_size` — proves (a) 5,000-char minified `.js` line in `ctx refs` and `ctx def` prints text field `<= 200` chars ending in `…`, (b) 35 definitions of one symbol in `ctx def` prints 20 definitions + `"15 more definitions: narrow with --kind or --in"` + `"35 definitions"`, and (c) hook denial reason exceeding 8 KB is capped at `<= 8192` bytes and ends with the truncation notice.

**Trap**: In `tests/cli.rs:100-115`, `hook_denies_repo_greps_and_whole_reads_of_held_files` asserts exact equality `assert_eq!(why, ...)` on `grep -rn foo src` — once `src/one.php` contains `foo`, `why` carries `grep_hint()` PLUS the `$ factory ctx refs foo` / `ctx def foo --no-body` output! Assert `why.starts_with(...)` on the new hint (with `shell_word(BIN)`) AND that `why` contains the `src/one.php` hit. Also check `a_denied_grep_carries_what_ctx_finds` (`src/cli/hook.rs:1092-1133`) for relative path base (`src/lib.rs:1`, NOT `lib.rs:1` when searching `root/src` from `root`!).

**Acceptance:** Both `grep_hook_denies_only_standalone_unfiltered_code_searches_with_ctx_hits` and `ctx_output_caps_line_width_def_count_and_hook_denial_size` pass alongside all existing tests; reverting any filter check, standalone check, 0-hit allow, or 8 KB cap breaks them.

### 3. Verify — medium — blocked by: 1, 2
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 4. Prove — at the checkpoint — blocked by: 3
**Source**: §9 QA plan rows FR-1 and FR-2:
- FR-1: `run grep_hook_denies_only_standalone_unfiltered_code_searches_with_ctx_hits` (evidence: test transcript)
- FR-2: `run ctx_output_caps_line_width_def_count_and_hook_denial_size` (evidence: test transcript)

**Acceptance:** after the checkpoint, both tests run under `cargo test` with full gate green and transcripts recorded in Progress.
