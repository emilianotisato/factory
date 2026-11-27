# Slice 01 — Testguard additive imports and grep stdout redirects

**Source**: [`prd.md`](prd.md) → §3 (FR-1). **Depends on**: — _(independent of Slices 02–05)_
**Created**: 2026-11-27

**Progress**: checkpoint prove: `cp-1/FR-1/testguard.txt`, `cp-1/FR-1/grep_hook.txt`.

**Slice**: Eliminates two recurring false-positive guard blocks without loosening real protections: exempts purely additive top-level Rust `use` and JS/TS `import` merges in `src/testguard.rs` (keyed by full path + local binding name, strictly outside test bodies), and makes `shell::tokenize` / `first_command_standalone` recognize stdout-to-file redirects (`>`, `>>`, `1>`, `1>>`, `&>`, `&>>`) as non-standalone while keeping `2>/dev/null`, `< file`, and `>&2` denied.

**Highest-risk**: Both tasks loosen an existing guard. Every non-target bypass (`2>/dev/null`, `< file`, `>&2`, import deletions, alias renames, in-test `use` edits) MUST remain strictly denied and be replayed in the regression corpus.

## Current code / environment

- **`testguard::parse_diff` & `Hunk`**: `src/testguard.rs:111-162, :175-260`.
  - `Hunk` (`:175-179`) currently tracks only `old_start`, `removed`, `ctx`. Extend it to keep `removed_lines: Vec<String>` and `added_lines: Vec<String>` (stripping the leading `-`/`+` prefix).
  - `parse_diff` (`:136-159`) checks `hunk.removed == 0`, then `authorized`, then `enclosing_test_name(...)`.
- **`shell::tokenize` & `first_command_standalone`**: `src/shell.rs:89-135, :150-204`.
  - `Tok::Redirect` (`:113`) currently drops fd and arrow direction: at `:106-109` it clears numeric `word` before checking `>` vs `<`, and at `:96-100` `&>` emits bare `Tok::Redirect`.
  - `first_command_standalone` (`:150-193`) skips the redirect target word (`:171-174`) without recording whether stdout was redirected to a file, and `first_cmd_standalone` (`:195-204`) returns `true` for `end == None`.
- **Deliberate non-changes** — *(prd §3 Accepted cost)*:
  - Partial multi-line import hunks where `-U0` sees only an interior line without its `use` prefix or `from '...'` specifier MUST stay conservatively denied.
  - Put all new integration tests in a new `tests/guardrails_hardening.rs` file (never modify existing tests in `tests/hook.rs`).

## Tasks

### 1. Exempt purely additive top-level Rust `use` and JS/TS `import` merges in `src/testguard.rs` — high — blocked by: —
**Source**: §3 FR-1: "Allow purely additive top-level Rust `use` and JS/TS `import` merges in `src/testguard.rs`", "Every removed import item is keyed by full path + local binding name".

`src/testguard.rs`.
1. Store `removed_lines: Vec<String>` and `added_lines: Vec<String>` on `Hunk` in `hunks_in_chunk` (`:219-260`).
2. Add `fn is_additive_top_level_import(path: &str, hunk: &Hunk) -> bool`:
   - For `.rs` files:
     - Parse every non-blank line in `hunk.removed_lines` as a complete single-line or complete-hunk Rust `use` statement (`[pub[(...)] ]use <tree>;`). Reject immediately if ANY non-blank removed line is not a valid complete `use ...;` statement!
     - Expand nested brace trees recursively into normalized `(visibility, full_source_path, local_binding)` tuples:
       - `use foo::a;` → `("", "foo::a", "a")`
       - `use foo::{a, b as c, self};` → `("", "foo::a", "a")`, `("", "foo::b", "c")`, `("", "foo", "foo")` (preserving `self` local binding as the parent module name!)
       - `use foo::a as z;` → `("", "foo::a", "z")` (distinct from `("foo::a", "a")`!)
       - `use foo::*;` → `("", "foo::*", "*")`
     - Parse all complete `use ...;` statements from `hunk.added_lines` (ignoring blank/non-use added lines only? Wait: require every added line in the hunk to also be a `use` or blank line, or expand all `use` statements in `added_lines` — requiring all non-blank lines in `removed_lines` to be valid `use` statements and every removed tuple to be present in the added `use` tuples guarantees zero removed symbols/lines!).
   - For `.ts`/`.tsx`/`.js`/`.jsx`/`.mjs` files:
     - Parse every non-blank line in `hunk.removed_lines` as a complete `import [type] <clause> from '<spec>'` / `"<spec>"` statement (handling named `{ a, b as c }`, default `Foo`, namespace `* as ns`, and default+named `Foo, { a }`).
     - Normalize into `(is_type, module_specifier, imported_name, local_binding)` tuples (rejecting any non-import or unparseable removed line).
     - Return `true` iff the removed tuple set is non-empty AND is a subset of the added import tuples!
3. In `parse_diff` (`:143-159`), AFTER computing `enclosing = enclosing_test_name(...)`, if `enclosing.is_none() && is_additive_top_level_import(&path, &hunk)`, `continue` (exempt!). Never exempt deleted files or hunks with `enclosing.is_some()`.

**The trap**: Remember Rust `as` aliases (`use foo::a;` → `use foo::a as z;`) and JS `import type` (`import { A } from 'x'` → `import type { A } from 'x'`): both MUST stay denied because `local_binding` or `is_type` changed! Also handle trailing semicolons and optional quotes (`'` vs `"`) consistently.

**Acceptance:** Unit tests in `src/testguard.rs` and integration test in Task 3 prove every allowed merge passes and every regression-corpus case (deletion, subset removal, alias rename, source change, `import`→`import type`, in-test `use`, non-import removed line) stays denied.

### 2. Carry `stdout_to_file` on `Tok::Redirect` and allow stdout-to-file grep redirects in `src/shell.rs` — high — blocked by: —
**Source**: §3 FR-1: "Change `Tok::Redirect` (`src/shell.rs`) to carry `stdout_to_file: bool`", "return `standalone = false` when `has_stdout_file_redirect` is `true`".

`src/shell.rs`.
1. Change `Tok::Redirect` to `Tok::Redirect { stdout_to_file: bool }`.
2. In `tokenize` (`:89-115`):
   - For `&>` / `&>>` (`:96-100`): when `!redirect(&mut chars)`, emit `Tok::Redirect { stdout_to_file: true }`.
   - For `>` / `<` (`:104-115`): BEFORE clearing `word`, capture `let fd = if in_word && word.chars().all(|d| d.is_ascii_digit()) { Some(word.as_str()) } else { None };` (owned or matched before clear). `let is_stdout = c == '>' && matches!(fd, None | Some("1"));` When `!redirect(&mut chars)` (meaning a file target follows, not `>&2` / `1>&2`!), emit `Tok::Redirect { stdout_to_file: is_stdout }`.
   - Verify `2>/dev/null` (`fd == Some("2")`) emits `stdout_to_file: false`, `< file` (`c == '<'`) emits `stdout_to_file: false`, and `>&2` / `1>&2` returns `true` from `redirect()` (so no `Tok::Redirect` emitted at all).
3. In `first_command_standalone` (`:150-193`):
   - Declare `let mut has_stdout_file_redirect = false;` inside the `loop` (so it resets after `cd`/`pushd`).
   - On `Tok::Redirect { stdout_to_file }`: set `skip_target = true;` and `if *stdout_to_file { has_stdout_file_redirect = true; }`.
   - Compute `let standalone = !has_stdout_file_redirect && first_cmd_standalone(end, &toks[i..]);`.

**The trap**: Do NOT let `2>/dev/null` or `grep foo < /tmp/in` or `grep foo >&2` set `has_stdout_file_redirect = true`! Only stdout-to-file (`>`, `>>`, `1>`, `1>>`, `&>`, `&>>` followed by a file target) makes a grep non-standalone.

**Acceptance:** Unit tests in `src/shell.rs` and integration test in Task 3 prove `grep foo src > /tmp/out`, `1>> /tmp/out`, `&> /tmp/out` are non-standalone (allowed by hook), while `grep foo src`, `grep -rn foo src 2>/dev/null`, `grep foo < /tmp/in`, `grep foo src >&2`, `rg foo src`, `cd sub && grep foo .`, and `grep foo src;` remain standalone (denied by hook).

### 3. Add regression-corpus integration tests in `tests/guardrails_hardening.rs` — high — blocked by: 1, 2
**Source**: §3 FR-1 Acceptance & §9 QA plan FR-1 row.

`tests/guardrails_hardening.rs` (new file — keep free of plan-id strings so `factory leaks` stays clean!).
1. Add `testguard_allows_additive_imports_and_denies_removals_and_in_test_edits`:
   - In a temp git repo with a pre-existing `tests/sample.rs` and `tests/sample.test.ts`, commit base versions and verify `factory::testguard::check` returns empty (`0` violations) for:
     - `use foo::a;` → `use foo::{a, b};`
     - `use a::b;` → `use a::{b, c};`
     - `import { a } from 'x'` → `import { a, b } from 'x'`
     - `import type { A } from 'x'` → `import type { A, B } from 'x'`
   - Replay the full deny corpus and assert `testguard::check` returns a violation for EVERY case:
     - deleting a `use` line
     - `use foo::{a, b};` → `use foo::a;` (symbol removal)
     - `use foo::a;` → `use foo::a as z;` (alias rename)
     - `import { a } from 'x'` → `import { a as z } from 'x'` (JS alias rename)
     - `import { a } from 'x'` → `import { a } from 'y'` (source change)
     - `import { A } from 'x'` → `import type { A } from 'x'` (type-only change)
     - editing `use foo::a;` inside `#[test] fn existing_test() { ... }`
     - modifying a non-import line in the same hunk as a `use` merge.
2. Add `grep_hook_allows_output_redirects_and_keeps_standalone_denied`:
   - Invoke `factory hook` (the Bash PreToolUse hook) with JSON tool input for allowed redirect commands (`grep foo src > /tmp/out`, `grep foo src 1>> /tmp/out`, `grep foo src &> /tmp/out`, `grep foo src > /tmp/out && wc -l /tmp/out`) and assert exit `0` / allow.
   - Replay the full deny corpus (`grep foo src`, `grep -rn foo src 2>/dev/null`, `grep foo < /tmp/in`, `grep foo src >&2`, `rg foo src`, `cd sub && grep foo .`, `grep foo src;`) and assert every single one is denied!

**Acceptance:** `cargo test --test guardrails_hardening` passes; removing the alias check or the `stdout_to_file` fd check makes the regression corpus fail.

### 4. Verify — medium — blocked by: 3
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 5. Prove — at the checkpoint — blocked by: 4
**Source**: §9 QA plan FR-1 row.
- **FR-1**: run `cargo test --test guardrails_hardening -- --nocapture` and keep the transcript. Expected: both `testguard_allows_additive_imports_and_denies_removals_and_in_test_edits` and `grep_hook_allows_output_redirects_and_keeps_standalone_denied` pass with full regression-corpus replay.

**Acceptance:** after the checkpoint, the FR-1 row has transcript evidence in this slice's **Progress**.
