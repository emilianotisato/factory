# Slice 01 — ctx output caps and Go, PHP and Pest indexing

**Source**: [`prd.md`](prd.md) → §4 FR-2 (`ctx` print caps), §5 FR-3, §9 QA plan. **Depends on**: — (touches only `src/ctx/*` and `tests/ctx.rs`).
**Created**: 2026-11-26

**Progress**: `MAX_LINE_CHARS`/`MAX_DEFS`, Go `const_spec`, PHP `const_element`, `--kind constant`, Pest `test`/`it` outline extraction; inner loop green. cp-1: `cp-1/FR-3/transcript.txt`

**Slice**: Adds UTF-8-safe 200-char text-field truncation (`MAX_LINE_CHARS`) and a 20-definition cap (`MAX_DEFS`) in `src/ctx/print.rs`, indexes Go `const_spec` and PHP class constants (`const_element`) with `--kind constant` support, and extracts PHP Pest `test()`/`it()` calls into `ctx outline` via a safe Rust AST walk. Leaves the 8 KB hook denial cap and grep hook changes to slice 02, and Bash guard/corpus/skill changes to slice 03.

## Current code / environment

- **`src/ctx/print.rs:1-189`**: `defs()` prints all `found.hits` without a cap; `block()` (`:147-152`), `indented()` (`:154-160`), and `outline()` (`:109-145`) print raw text without width truncation.
- **`src/ctx/lang.rs:22-52, :92-93, :120, :157-174`**:
  - `PHP_EXTRA` (`:22-25`) tags methods and enums only; `GO_EXTRA` (`:39-42`) tags structs/interfaces and type aliases only.
  - `TagsConfiguration::new(...).expect(...)` (`:92-93`): `tree-sitter-tags` ONLY permits captures named `name`, `doc`, `ignore`, `local.*`, `definition.*`, `reference.*`. Any custom capture like `@_fn` panics with `InvalidCapture`! Pest `test()`/`it()` MUST be extracted in Rust using `Unit::parse()`, NOT inside `PHP_EXTRA`.
  - `tree-sitter-tags` caps `line_range` at 180 bytes (`:120`), whereas Rust-extracted Pest definitions carry their full trimmed call line (so >200-char Pest lines exercise `outline()` truncation too).
- **`src/ctx/def.rs:43-107, :318-327`**: `Table::Php::patterns` and `Table::Go::patterns` lack `const` patterns; `vocabulary` (`:318-327`) maps `"const"` → `"constant"` only for `Table::Rust`.
- **`src/ctx/mod.rs:36`**: `DefArgs.kind` uses `value_parser = ["class", "fn", "method", "type"]` (missing `"constant"`).
- **`src/ctx/outline.rs:154-164`**: `outlined()` returns `Some(false)` for `"fn" | "method" | "type"`, `None` for `"constant"` (constants stay out of `outline`).
- **`tests/ctx.rs:42-100`**: asserts 20 golden snapshots over `tests/fixture/ctx` (`searched 48 files`). **Do NOT add or modify any file inside `tests/fixture/ctx`** — write new tests in `tests/ctx.rs` (purely additive, zero lines removed from existing tests so `testguard` stays green) using `tempfile::tempdir()`.
- **Deliberate non-changes (`prd.md` §11)**: do NOT add extensionless shebang sniffing to `src/ctx/walk.rs`, do NOT change `tests/fixture/ctx`, and keep `src/ctx/*.rs` free of plan-id strings (`factory leaks`).

## Tasks

### 1. Cap printed text width (200 chars) and `def` locations (20) in `src/ctx/print.rs` — high — blocked by: —
**Source**: §4 FR-2: "define `pub const MAX_LINE_CHARS: usize = 200;` and a UTF-8 char-safe `truncate_line(s: &str, max: usize) -> Cow<'_, str>` ... Apply it to the `text` field in `block()`, each `line` in `indented()`, and each entry's `text` in `outline()` ... define `pub const MAX_DEFS: usize = 20;` ... When `found.hits.len() > MAX_DEFS`, emit a line after the 20th definition: `"{more} more definitions: narrow with --kind or --in"` while keeping the accurate total count in the trailer".

- `src/ctx/print.rs`:
  - Update module doc comment (`:1`) since lines now have a width cap.
  - Add `pub const MAX_LINE_CHARS: usize = 200;` and `pub const MAX_DEFS: usize = 20;`.
  - Add `pub fn truncate_line(s: &str, max: usize) -> std::borrow::Cow<'_, str>`: if `s.chars().count() <= max`, return `Cow::Borrowed(s)`; otherwise take `max.saturating_sub(1)` chars + `'…'` and return `Cow::Owned(...)`.
  - Apply `truncate_line(text, MAX_LINE_CHARS)` inside `block()` (`:151`), `indented()` (`:158`), and `outline()` (`:118`, on `e.text`).
  - In `defs()` (`:9-62`): iterate `found.hits.iter().take(MAX_DEFS)`. If `found.hits.len() > MAX_DEFS`, emit `writeln!(out, "\n{} more definitions: narrow with --kind or --in", found.hits.len() - MAX_DEFS)` (keeping the trailer's total `n = found.hits.len()` accurate and keeping `withheld` behavior when `body && n > 1`).
  - Add unit tests in `src/ctx/print.rs` covering `truncate_line` on ASCII > 200 chars, multi-byte UTF-8 boundary truncation, exact-200 boundary (no `…`), and `defs()` capping 35 hits to 20 printed + `"15 more definitions: narrow with --kind or --in"` + `"35 definitions"`.

**Trap**: Truncate only the `text` field (not `path:line  kind  text` combined), and never slice a `&str` at a byte index without checking UTF-8 char boundaries (`char_indices`).

**Acceptance:** Unit tests in `src/ctx/print.rs` prove `truncate_line` caps multi-byte UTF-8 and 5,000-char lines to `<= 200` chars ending in `…`, and `defs()` caps 35 definitions at 20 with `"15 more definitions: narrow with --kind or --in"` and `"35 definitions"` in the trailer. Removing `truncate_line` or `.take(MAX_DEFS)` fails these tests.

### 2. Index Go `const_spec`, PHP class constants (`const_element`), `--kind constant`, and PHP Pest `test()`/`it()` in outline — high — blocked by: 1
**Source**: §5 FR-3: "Add `(const_spec name: (identifier) @name) @definition.constant` to `GO_EXTRA` ... in `Table::Go::patterns`, add `r"const\s+NAME\b"` and `r"NAME\s*(?://.*)?$"` ... Add `(const_declaration (const_element (name) @name)) @definition.constant` to `PHP_EXTRA` ... add `r"((public|protected|private|final)\s+)*const\s+([A-Za-z_\\|?]+\s+)?NAME\b"` to `Table::Php::patterns` ... add `"constant"` to `DefArgs.kind`'s `value_parser` ... extract Pest `test('...')` and `it('...')` entries directly in Rust using the parsed tree (`Unit::parse()` when `self.grammar == Grammar::Php`) ... add `"test"` to `outlined()` returning `Some(false)`".

- `src/ctx/lang.rs`:
  - `GO_EXTRA` (`:39-42`): add `(const_spec name: (identifier) @name) @definition.constant`.
  - `PHP_EXTRA` (`:22-25`): add `(const_declaration (const_element (name) @name)) @definition.constant` (verify against `tree-sitter-php` that both class constants `public const FOO = 1;` / `public const string MODE = 'a';` and top-level `const BAR = 2;` parse as `const_declaration` / `const_element`; if class constants use a different node name like `class_const_element` or `const_declaration`, include the exact verified node name!).
  - In `Unit::definitions()` (`:157-174`): when `self.grammar == Grammar::Php`, also call a Rust helper `pest_tests(&self)` using `self.parse()` that walks the tree (skipping subtrees inside `class_declaration`, `interface_declaration`, `trait_declaration`, `enum_declaration`, `function_definition`, `method_declaration`, while descending into top-level statements and `describe(...)` closures) and emits a `Definition` (`kind: "test"`, `range: call_node.byte_range()` shifted by `self.offset`, `name: first_arg_string.byte_range()` shifted, `line: trimmed_line_range` shifted) for every `function_call_expression` whose `function` child is a `(name)` equal to `"test"` or `"it"` and whose first argument is a `string` or `encapsed_string`. Merge with tags definitions and sort.
- `src/ctx/def.rs`:
  - `Table::Go::patterns` (`:55-59`): add `r"const\s+NAME\b"` and `r"NAME\s*(?://.*)?$"`; add `"const"` to `Table::Go::keywords` (`:90`).
  - `Table::Php::patterns` (`:45-48`): add `r"((public|protected|private|final)\s+)*const\s+([A-Za-z_\\|?]+\s+)?NAME\b"`; add `"const"` to `Table::Php::keywords` (`:74`).
  - `vocabulary` (`:318-327`): map `"const"` to `"constant"` for `Table::Rust | Table::Go | Table::Php` (keeping `Table::Ts` `"const"` mapped to `"fn"`).
- `src/ctx/mod.rs:36`: update `DefArgs.kind` `value_parser` to `["class", "fn", "method", "type", "constant"]`.
- `src/ctx/outline.rs:154-164`: add `"test"` to `outlined()` returning `Some(false)`.
- `tests/ctx.rs` (purely additive new test function at end of file, using `tempfile::tempdir()` — do NOT modify `tests/fixture/ctx`!): add `ctx_indexes_go_consts_php_class_consts_and_pest_outline_tests` proving:
  1. Go single `const Single = 1`, grouped `const (\n First = iota\n Second // trailing\n)` found by `ctx def` with kind `constant` and `--kind constant`, while a struct literal `First: 1,` in Go does NOT match.
  2. PHP `public const STATUS_OPEN = 'open';` and PHP 8.3 typed `public const string MODE = 'a';` found by `ctx def` with kind `constant` and `--kind constant`.
  3. Pest `.php` file with top-level `test('creates invoice', ...)`, `it('rejects empty', ...)`, `describe('billing', fn () => { it('charges once', ...); })`, a nested closure-local `function helper() {}`, and a class method calling `test('not a pest test')` outlines the 3 Pest tests as kind `test`, hides `helper`, and ignores the class method call.
  4. A >200-char Pest test title line is truncated to `<= 200` chars ending in `…` by `ctx outline`.

**Trap**: Verify the exact `tree-sitter-php` AST node names for class constants and typed constants (run a quick unit test) before finalizing `PHP_EXTRA`, and NEVER add a custom capture like `@_fn` inside `PHP_EXTRA` (it panics `tree-sitter-tags`!). Also NEVER add files to `tests/fixture/ctx` or all 20 snapshots in `tests/ctx.rs` will fail on `searched 48 files`.

**Acceptance:** `ctx_indexes_go_consts_php_class_consts_and_pest_outline_tests` passes and all 20 existing `tests/ctx.rs` snapshots remain green. Removing `const_spec`, PHP `const_element`, `--kind constant`, or `pest_tests` fails the corresponding assertion.

### 3. Verify — medium — blocked by: 1, 2
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 4. Prove — at the checkpoint — blocked by: 3
**Source**: §9 QA plan row FR-3:
- FR-3: `run ctx_indexes_go_consts_php_class_consts_and_pest_outline_tests` (evidence: test transcript)

**Acceptance:** after the checkpoint, the test runs under `cargo test` with full gate green and transcript recorded in Progress.
