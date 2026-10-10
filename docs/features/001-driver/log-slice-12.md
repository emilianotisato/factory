## Traps

## Blockers

## Decisions

### AD-1201 — Does `--kind fn` take methods? · slice-12 · 2026-10-10

**Chose:** yes, everywhere: `--kind fn` keeps tags mapped to `fn` and to `method`, and the hit still prints its own kind (`def new --kind fn` prints `rs/config.rs:10  method  pub fn new(…)`). `--kind method`, `class` and `type` match their own kind only (`lang::kind_matches`).
**Over:** `fn` ← function only, as the slice's mapping reads ("`fn` ← function; `method` ← method").
**Because:** task 2's acceptance asks `def new --kind fn` to find the `impl` method as kind `method`, which the literal mapping cannot do; one rule for all four grammars beats a Rust-only exception, and "a function" in an agent's question usually includes methods.
**Undo:** one line in `src/ctx/lang.rs` (`kind_matches`) and the `def-new-kind-fn` snapshot.
**Review:** decide — reconciles the slice's kind mapping with its own acceptance, a public flag's meaning

### AD-1202 — How does `outline` nest Rust methods, and what else does it list? · slice-12 · 2026-10-10

**Chose:** an `impl` block is a line of its own (`9  impl  impl Config {`) with its methods indented under it, as a class's are; a trait's method declarations are listed under the trait (`  24  method  fn load(&self) -> Config;`). So the Rust fixture prints the struct, the impl, its two methods, the trait and its one declaration.
**Over:** the acceptance's list read literally ("the struct, the trait and the two `impl` methods") with the methods at the top level, which `--depth 1` could not then hide, or the impl's methods indented under nothing.
**Because:** a Rust method has no class to sit under, and the impl line carries what a reader needs (which type, which trait); the trait's declarations are its interface, the same as a PHP interface's methods, which the slice wants listed.
**Undo:** the `impl_item` and `function_signature_item` patterns in `RUST_EXTRA` (`src/ctx/lang.rs`) and `"impl"` in `outline::outlined`; the `outline-rs` snapshot.
**Review:** decide — prints lines the acceptance's list does not name

### AD-1203 — Upstream `tags.scm` as shipped, or with patterns of our own? · slice-12 · 2026-10-10

**Chose:** each grammar's `TAGS_QUERY` with a few patterns put in front of it (`PHP_EXTRA`, `TS_EXTRA`, `GO_EXTRA`, `RUST_EXTRA` in `src/ctx/lang.rs`); tree-sitter-tags keeps one tag per name, the lowest pattern's, so ours win. PHP: `method_declaration` is a method (upstream: function), enums tagged. TypeScript: function, generator, class, method, `const`/`let`/`var` bound to an arrow or function expression (the whole declaration), type alias, enum. Go: a struct or interface `type_spec` is a class (upstream: type), `type_alias` tagged. Rust: `type` alias is a type (upstream: class), a fn in a `mod` body is a function (upstream: method), trait method declarations, `const`, `static`, `impl`. No locals query is passed.
**Over:** the shipped queries alone.
**Because:** tree-sitter-typescript's `tags.scm` leaves functions, classes and methods to tree-sitter-javascript's, so without these `def useThing` and `outline` on the Vue fixture find nothing; PHP methods come out as `fn`, which task 2's `def handle --kind method` rules out. The locals query only hides references, and only definitions are read.
**Undo:** delete the four constants' lines; the snapshots show what moves.
**Review:** fyi — the kinds come from our query text, not only the grammar crates'

### AD-1204 — Which grammar for `.tsx`, `.js`, `.jsx` and `.mjs`? · slice-12 · 2026-10-10

**Chose:** `LANGUAGE_TSX` for those four, `LANGUAGE_TYPESCRIPT` for `.ts` and Vue `<script>` blocks.
**Over:** TypeScript for all.
**Because:** JSX parses as an error under the TypeScript grammar, and an error sends the file to the textual fallback; the slice rules TSX out for Vue only.
**Undo:** `Grammar::of` in `src/ctx/lang.rs`.
**Review:** no

### AD-1205 — What does the `def` trailer say about what the tree dropped? · slice-12 · 2026-10-10

**Chose:** after `(searched K files)`: `; N textual match(es) dropped: inside comments or strings`, `; N textual match(es) dropped: no definition node` (a line that reads like a definition and is not one, or a Vue template line), `; N definition(s) of another kind (--kind)`, `; N file(s) not parsed cleanly: textual matches kept`. `def handle --kind class` prints `no definition of handle found (searched 48 files); 1 definition of another kind (--kind)`.
**Over:** one reason for every drop, as the slice's example has it.
**Because:** a hit can fail the tree without being in a comment; labelling it so would be false, and an empty answer under `--kind` should say a definition exists.
**Undo:** `print::defs`.
**Review:** no

### AD-1206 — What counts as "fails to parse", and what does it change? · slice-12 · 2026-10-10

**Chose:** a tree whose root `has_error()` (an ERROR or MISSING node anywhere), or tagging that fails. `def` then gives slice 11's output for that whole file (heuristic body, keyword kind mapped to the vocabulary: `class`, `fn`, `method`, `type`, `module`, `constant`, `macro`) and counts it in the trailer; under `--kind`, `fn` and `method` pass for each other there, since the keyword cannot tell them apart. `refs` keeps every hit of such a file unfiltered and says so in its trailer (`; N file(s) not parsed cleanly: comments and strings kept`); `outline` prints what was tagged and notes the file.
**Over:** keeping the confirmed hits of a file with a local error, or filtering a broken file's refs anyway.
**Because:** the slice's "syntax error at the root" names no node test; an unterminated string in a broken tree swallows the code after it, so filtering by it could drop real hits — "a broken file never hides a definition".
**Undo:** `File::confirmed` in `src/ctx/def.rs`, `code_lines` in `src/ctx/refs.rs`.
**Review:** no

### AD-1207 — What does `outline` list, how deep, and what does it say at the end? · slice-12 · 2026-10-10

**Chose:** kinds `class`, `fn`, `method`, `type`, `impl`, and `module` when it has a body; left out: `field` (properties), `constant`, `macro`, a bodyless `namespace X;`, and anything defined inside a function or method (locals). Depth = 1 + the containers around it (class, impl, module); `--depth N` keeps depth ≤ N. Each line is `{2 spaces per level}{line}  {kind}  {the name's line, trimmed}`, the line number being the name's. A directory stands for its source files, non-recursive, dot-files skipped, sorted. A path that cannot be read exits 2 (`factory ctx: cannot read P: …`); a named file of no known language is counted. The trailer is printed only when something was left out: `N nested definitions hidden (--depth)`, files not parsed cleanly, files in no outlined language, skipped binary or over-2 MB files. An empty file prints its header alone.
**Over:** a trailer on every outline, as `def` and `refs` have.
**Because:** the acceptance pins the empty file to "the header and nothing else"; the rest is where the slice was silent.
**Undo:** `src/ctx/outline.rs`, `print::outline`.
**Review:** no

### AD-1208 — Which nodes are comments and strings, and how is the Vue block found? · slice-12 · 2026-10-10

**Chose:** PHP `comment`, `string`, `encapsed_string`, `heredoc`, `nowdoc` (the slice lists `comment` only); TypeScript, Go and Rust as the slice lists them. A `${…}` inside a template string is code (`template_substitution` stops the climb). A `refs` line is kept when any whole-word match on it lies outside comments and strings. The Vue block is the first `<script` followed by whitespace or `>`; a `>` inside a quoted attribute (`generic="T extends Map<K, V>"`) does not close the tag; `script_block` returns `(offset, text)` without a `lang`, since every block parses as TypeScript. A `refs` hit in a Vue template is kept.
**Over:** the slice's PHP list read literally, which keeps `'planted'` in PHP strings.
**Because:** the slice's own heading drops "comments and strings"; PHP strings have four node kinds.
**Undo:** `Grammar::is_comment_or_string`, `Unit::in_comment_or_string` (`src/ctx/lang.rs`), `src/ctx/vue.rs`.
**Review:** no
