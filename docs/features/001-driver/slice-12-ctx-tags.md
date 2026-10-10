---
slice: 12
status: done
lane: 1
depends_on: [11]
---

# Slice 12 — ctx tags: tree-sitter, outline, kind filter, Vue shim (FR-12 slice 2)

**Source**: [`prd.md`](prd.md) → §14 FR-12 decision 2 (tree-sitter half: definition-node confirmation, body as node range, comments and strings dropped, `outline` from `tags.scm`, the Vue shim), decision 5 ("(2) tree-sitter tags for the four grammars, `outline`, the kind filter, the Vue script shim"), and its acceptance. **Depends on**: 11 (same files: the regex hits are the candidates tree-sitter confirms; the printer and fixtures are reused).
**Created**: 2026-10-09 · **Status**: pending

**Progress** (2026-10-10, lane 1):
- **Built.** `src/ctx/lang.rs`: `Grammar` (PHP, TypeScript, TSX, Go, Rust; AD-1204) with `language()` and `tags_config()`, the latter built once per process in a `OnceLock` from each crate's `TAGS_QUERY` with our patterns in front (AD-1203); the comment and string node sets (AD-1208); `Unit` (a whole file, or a Vue script block at its offset) with `definitions()` and `in_comment_or_string()`; the `--kind` mapping and `kind_matches` (AD-1201). `src/ctx/vue.rs`: `script_block`. `def.rs`: each candidate file is tagged once; a regex hit is kept when a definition tag's name sits on its line, kind from the tag, filtered by `--kind`, body = the node's range printed whole; an unconfirmed hit is classed (comment or string, or no definition node) with one parse and counted in the trailer (AD-1205); a file whose tree holds an error keeps slice 11's output with a trailer note (AD-1206). `refs.rs`: each file with hits (test files only under `--tests`) is parsed once, and a line is dropped before the caps when every match on it lies in a comment or a string, counted as `N dropped (comments/strings)`. `outline.rs` and `print::outline`: tags only, one parse per file, a `== path ==` header each, members indented under their container, `--depth` default 2 as task 4 says (the §14 synopsis shows the flag as `[--depth 1]`), a directory outlined non-recursively (AD-1202, AD-1207). Slice 11's outline stub and its "applied from slice 12" `--kind` note are gone.
- **Crates and the version check.** `tree-sitter` 0.27.1, `tree-sitter-tags` 0.27.1, `tree-sitter-php` 0.25.1, `tree-sitter-go` 0.25.0, `tree-sitter-rust` 0.24.2 and `tree-sitter-typescript` 0.23.2 compiled together on the first build: all four grammars share `tree-sitter-language` 0.1.9, whose `LanguageFn` the 0.27 core takes, so no pin was needed. Nothing else added.
- **Tests.** Un-ignored slice 11's `def_drops_the_docblock_mention` and `refs_drops_the_comment_and_string_hits`; both pass (298 real hits = 300 - 2, trailer `2 dropped (comments/strings)`). New in `tests/ctx.rs`: `def_kind_filters_and_fn_takes_methods`; `def_body_is_the_node_range` (UserService ends at its brace, Config is whole, a 40-line body is not cut and its last line stops where the node does); `outline_prints_definitions_and_nothing_else` (the PHP fixture, `--depth 1`, and a temp class whose `use`, constant and property print nothing); `outline_covers_go_rust_vue_and_empty_files` (Vue lines 6 and 10; an empty `.rs` and a template-only `.vue` print their header and exit 0; an unreadable path exits 2); `a_file_that_does_not_parse_keeps_its_textual_matches`. Eight new snapshots, each read line by line: `def-handle-kind-method`, `def-handle-kind-class`, `def-new-kind-fn`, `outline-php`, `outline-php-depth-1`, `outline-go`, `outline-rs`, `outline-vue`. Unit tests: `lang.rs` (task 1: one fixture per language parses to `program`, `program`, `source_file`, `source_file`, and the Vue block to `program`, each with a definition tag; the kind mapping; comments and strings in four grammars, with `${…}` as code), `vue.rs`, `outline.rs` (nesting, locals, bodyless modules), `def.rs` (the fallback's keyword vocabulary).
- **Inner loop:** 5 runs of `cargo test`, 1 red, about 2 min (5 s, 20 s, 2 s, 36 s, 37 s). Run 1 was `--lib ctx::`, the version check. The red run was the snapshot test, by design: seven slice 11 snapshots moved and eight new ones were absent. They were blessed with `FACTORY_BLESS=1`, then read. The other 15 `ctx` tests passed on their first run. The final run was green: 141 passed, 0 ignored. `cargo fmt` reflowed long lines in `def.rs`, `refs.rs`, `lang.rs` and `print.rs`; `cargo clippy --all-targets -- -D warnings` caught nothing.
- **Earlier-slice expectation shifts** (`tests/ctx.rs`):
  - `def_prints_location_kind_signature_and_body_on_every_stack`: the kinds `function`→`method`, `func`→`fn`, `struct`→`class`, `const`→`fn`.
  - `def_still_prints_the_docblock_mention` → `def_prints_only_the_definition_node`: one hit, the drop counted.
  - `refs_still_prints_the_comment_and_string_hits` → `refs_starts_at_the_first_real_hit`: the first hit is `caller01.ts:5`.
  - `refs_caps_at_10_with_a_trailer`: 290 → 288 more hits.
  - `a_hit_in_a_test_file_shows_only_with_tests`: 300 → 298 and 301 → 299 hits, plus `; 2 dropped (comments/strings)`.
  - `outline_says_it_needs_tree_sitter_and_exits_2`: replaced by the outline tests above.
  - `a_bad_flag_or_an_unreadable_root_exits_2`: `--kind class` now prints `rs/config.rs:4  class`.
  - `SNAPSHOTS`: 12 → 20.

  Snapshots re-blessed:
  - `def-Config`, `def-NewClient`, `def-OtherService`, `def-handle`, `def-useThing`: the kinds, and the docblock hit is gone.
  - `refs-planted`, `refs-planted-max-20-per-file-1`: the comment and string hits are gone, and the trailers changed.

  `def-UserService` and its `--no-body` twin did not move: the node range ends where the heuristic did.
- **Deviations:** AD-1201 (`--kind fn` takes methods, which task 2's acceptance needs) and AD-1202 (`outline` prints Rust `impl` lines and trait declarations) are both `decide`. AD-1203 (`fyi`) puts our tag patterns in front of the upstream ones.
- **Prove (task 7):** left for the checkpoint. The FR-12 rows "`def` returns exactly the definition with kind and signature and drops the docblock mention" and "`outline` prints every top-level definition with its signature and nothing else" are covered by `cargo test --test ctx`. The warm `outline` timing rows (`time factory ctx outline app/Services` on icg-cloud, `internal/` on arca-api) go into slice 11's table.

**Slice**: After this slice `def` returns exactly the definition: each regex candidate is parsed with tree-sitter and kept only when the match sits inside a definition node of the right kind and outside comments and strings; the body is the node's exact range; `--kind class|fn|method|type` filters. `refs` drops hits inside comments and strings. `outline` prints every top-level definition with its signature from the grammar's `tags.scm`, and Vue files are outlined by parsing the `<script>` block with the TypeScript grammar at offset. The docblock mention from slice 11's fixture now disappears, which is the acceptance.

## Current code / environment

- **Crates** (cargo search 2026-10-09): `tree-sitter` 0.27.1, `tree-sitter-tags` 0.27.1, `tree-sitter-php` 0.25.1, `tree-sitter-go` 0.25.0, `tree-sitter-rust` 0.24.2, `tree-sitter-typescript` 0.23.2. The TypeScript grammar's version skew against the 0.27 core is the trap: check at slice entry that `tree-sitter-typescript` 0.23.2 compiles against `tree-sitter` 0.27.1 (its `LANGUAGE` constant must be a `tree_sitter_language::LanguageFn` the 0.27 core accepts); if not, pin the core to the newest version all four grammars share and note it in Progress. `tree-sitter-vue-sqry` 33.0.1 exists but its `tags.scm` is unknown (§14 "Vue has none"), so the shim is the plan, not the Vue grammar.
- **`tags.scm`**: each grammar crate exposes `TAGS_QUERY` (and `LOCALS_QUERY` where present); `tree_sitter_tags::TagsConfiguration::new(language, tags_query, locals_query)` and `TagsContext::generate_tags` yield `Tag{range, name_range, span, syntax_type_id, is_definition, docs}`; `syntax_type_name()` gives `class|function|method|interface|module|type|…`. Map to the §14 `--kind` vocabulary: `class` ← class, interface, trait, enum, struct; `fn` ← function; `method` ← method; `type` ← type, type alias.
- **Definition confirmation**: a slice 11 regex hit at `(path, line)` is confirmed when a tag with `is_definition` has `name_range` covering the hit's `NAME` column, and no `comment` or `string` node (by `kind()` in the grammar: PHP `comment`, TS `comment|template_string|string`, Go `comment|interpreted_string_literal|raw_string_literal`, Rust `line_comment|block_comment|string_literal|raw_string_literal`) contains the hit. Body = the tag's `range` (the whole definition node), printed in full; §14 "the body, printed by default … is the node's exact range, replacing the '`sed` 30 lines' guess".
- **`refs` filtering**: for each hit line, parse the file once (cache per file within one call only; §19 no tags cache across calls), walk to the node at the hit column, drop the hit if it or an ancestor is a comment or string node. The `refs/` fixture's planted comment and string hits are what the test now expects to vanish.
- **The Vue shim**: find `<script` … `</script>` (the first block; `setup` or not; `lang="ts"` or absent) with a byte offset; parse the inner text with the TypeScript grammar (`LANGUAGE_TYPESCRIPT` for `lang="ts"`, `LANGUAGE_TSX` never: Vue templates are not JSX); add the offset back to every range so `path:line` is right for the whole file. The `<template>` is not outlined (§14: "`outline` locates the `<script>` block and parses it").
- **Performance figures to hold**: §14 "whole-tree tagging 0.47 s for 263 Rust files, 7 ms per file re-parse". `def` parses only candidate files, `refs` only files with hits, `outline` only the named paths, so the sub-second target is structural.
- **Deliberate non-changes**: *(prd §14 "Accepted cost: … a Rust function made by a macro is found by regex only"; `def` on it stays empty, no macro expansion.)* *(prd §19 "Not handled: a tags cache"; parse per call.)* *(prd §14 "Rejected: an LSP proxy".)*

## Tasks

### 1. Grammar table and the version check — high — blocked by: —
**Source**: §14 FR-12: "Grammars: PHP 0.25.1, TypeScript, Go 0.25.0, Rust 0.24.2, all MIT with an upstream `tags.scm`".

`Cargo.toml` adds the five crates; `src/ctx/lang.rs`: `Lang::language()`, `Lang::tags_config()` (built once per process with `OnceLock`), the comment/string node-kind sets per language. Build on the slice-entry version check; record the outcome (versions that compiled together) in Progress.

**Acceptance:** `cargo build` succeeds with all four grammars; a test parses one fixture file per language and asserts the root node kind (`program`, `program`, `source_file`, `source_file`) and at least one `is_definition` tag each.

### 2. `def` confirmed by the tree, body as node range, `--kind` — high — blocked by: 1
**Source**: §14 FR-12: "then parses each candidate file with tree-sitter and keeps the hits inside a definition node of the right kind, outside comments and strings; the body, printed by default (`--no-body` for the location alone), is the node's exact range". Acceptance: "`def` returns exactly the definition with kind and signature and drops the docblock mention".

`src/ctx/def.rs`: after the regex pass, for each candidate file generate tags once, confirm each hit as described, replace the heuristic body with the tag's `range` text, set `kind` from the mapped `syntax_type_name()`, and apply `--kind` as a filter. The signature line is the first line of the range. Unconfirmed hits are dropped and counted in the trailer (`N textual matches dropped: inside comments or strings`). A file the grammar fails to parse (syntax error at the root) falls back to the slice 11 output for that file with a trailer note, so a broken file never hides a definition.

**Acceptance:** un-ignore slice 11's pinned assertion: `def OtherService` on the PHP fixture no longer prints the docblock line in `UserService.php`; `def handle --kind method` prints the method and `--kind class` prints nothing with the trailer; the body of `def UserService` ends at the class's closing brace exactly (snapshot); on the Rust fixture `def Config` prints the struct's full range and `def new --kind fn` finds the `impl` method as kind `method`, not `fn`.

### 3. `refs` without comments and strings — high — blocked by: 1
**Source**: §14 FR-12: "`refs` is a word-boundary search with comments and strings dropped, collapsed per file."

`src/ctx/refs.rs`: parse each file with hits once, drop hits inside comment or string nodes before the per-file collapse and the caps, count the drops in the trailer. The filter runs before `--max`, so the printed 10 are real references.

**Acceptance:** un-ignore the slice 11 assertion: `refs planted` no longer prints the comment hit or the string hit; the trailer says `2 dropped (comments/strings)`; the count of real hits is `300 - 2`.

### 4. `outline` from `tags.scm`, `--depth` — high — blocked by: 1
**Source**: §14 FR-12: "`outline` prints the grammar's `tags.scm` definitions with their signatures, the Agentless skeleton." Acceptance: "`outline` prints every top-level definition with its signature and nothing else."

`src/ctx/outline.rs`: for each path, generate tags, keep `is_definition`, print `line  kind  signature` with indentation by nesting depth (methods under their class), `--depth 1` keeps top level only (default: 2, so class members show); several paths print under a `== path ==` header each. A directory argument outlines every source file in it (non-recursive) so the pack builder (slice 13) can outline a recon folder in one call. Replaces slice 11's stub.

**Acceptance:** snapshot per language: the PHP fixture's class, its methods indented, the trait and the interface, no properties and no `use` lines; `--depth 1` drops the methods; the Go fixture lists the struct, the method and the function; the Rust fixture lists the struct, the trait and the two `impl` methods; `outline` on an empty file prints the header and nothing else, exit 0.

### 5. Vue `<script>` shim — medium — blocked by: 4
**Source**: §14 FR-12: "Vue has none, so `outline` locates the `<script>` block and parses it with the TypeScript grammar at offset."

`src/ctx/vue.rs`: `script_block(src) -> Option<(offset, lang, text)>`; `def`, `refs` and `outline` route `.vue` files through it, offsetting every range; no `<script>` → treated as a file with no definitions. `lang="ts"` or `setup lang="ts"` → TypeScript grammar; no `lang` → TypeScript grammar too (a superset for our purpose).

**Acceptance:** `outline` on the Vue fixture prints the `function` and the `const` arrow function with their real line numbers in the `.vue` file (not the offsets inside the script block); `def useThing` on it confirms through the tree; a `.vue` file with only a `<template>` prints the header and exits 0.

### 6. Verify — medium — blocked by: 1–5
`cargo test`. The full gate runs at this slice's checkpoint.

**Acceptance:** inner loop green, one commit.

### 7. Prove — at the checkpoint — blocked by: 6
Rows from [`prd.md`](prd.md) § QA plan: the rest of the pinned-output part of **FR-12** ("`def` returns exactly the definition with kind and signature and drops the docblock mention"; "`outline` prints every top-level definition with its signature and nothing else"; "`outline` under 1 s warm on icg-cloud and arca-api"). At the checkpoint: the snapshots and an `outline` timing row added to slice 11's table (`time factory ctx outline app/Services` on icg-cloud, `internal/` on arca-api, warm), pasted into this slice's **Progress**.

**Acceptance:** the timing table has `outline` rows under 1 s warm; the two un-ignored assertions from slice 11 pass.
