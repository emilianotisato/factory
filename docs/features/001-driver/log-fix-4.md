## Traps

- 2026-10-10, polish fixer 4: tagging every file that names the symbol, so `def` would match `outline` by construction, took `def handle` on a 12,877-file Laravel checkout from 0.10 s to 0.59 s and `def id` to 0.77 s, near FR-12's 1 s. The regex prefilter stays and is widened instead (AD-40001); the same queries then answer in 0.08–0.10 s, as before.

## Blockers

_(none)_

## Decisions

### AD-40001 — How does `def` find the definition shapes its regexes skipped? · polish · 2026-10-10

**Chose:** keep the regex prefilter PRD §14 names and widen each table (`src/ctx/def.rs` `Table::patterns`): PHP takes `abstract`, `final`, `readonly` and the visibility keywords in any order; TypeScript takes `export`, `default`, `declare` and `async` before `function`, `get`/`set`/`override`/`abstract` and a generic `<T>` before a member, and a member whose parameters run onto the next line (`NAME(` with no `)` on the line); Rust takes `const`, `async`, `unsafe`, `default` and `extern "C"` before the keyword, plus `union` and `static mut`; Go adds a third regex, a name then a type at the line start, for a type inside a `type ( … )` group. The tree still decides: a struct field that matches the Go regex is counted as "no definition node", and an `impl` block never confirms a hit. Each shape the finding lists is pinned in `tests/ctx.rs` `def_finds_every_shape_outline_lists`, which also checks `outline` lists it at the same line.
**Over:** tagging every file that names the symbol and taking each definition tag of that name, which agrees with `outline` by construction but costs 5–8× on a large checkout (Traps); and widening only the shapes listed, which leaves `final public` and `public final` to differ.
**Because:** the prefilter is the engine PRD §14 describes and it keeps `def` at the measured 0.1 s; a missed shape is a missing alternative in one regex, cheap to add when found.
**Undo:** `Table::patterns` and the Go default kind in `Table::keywords` (`type`, for the group line, was `func`).
**Review:** no

### AD-40002 — Which `<script>` blocks of a Vue file are read? · polish · 2026-10-10

**Chose:** every complete `<script` block, in file order (`vue::script_blocks`). `Unit::all` returns one unit per block, `lang::definitions` merges their tags in source order, and `lang::Parsed` checks a byte against the block that holds it, so `outline`, `def` and the `refs` comment check read a component with both `<script>` and `<script setup>` at its real line numbers. A block that never closes ends the list. This supersedes AD-1208's clause "the Vue block is the first `<script` followed by whitespace or `>`"; the rest of AD-1208 (the quoted `>` rule, no `lang`, template hits kept) stands.
**Over:** the first block only (AD-1208), which hid the `<script setup>` half of a component that also has a plain `<script>`.
**Because:** Vue allows both blocks in one file and puts the composition code in the second.
**Undo:** `src/ctx/vue.rs`, `Unit::all`, `lang::definitions` and `lang::Parsed`.
**Review:** no

### AD-40003 — What is a test path, segment by segment? · polish · 2026-10-10

**Chose:** refining AD-1106 (still the path relative to the search root): a directory segment equal to `test`, `tests`, `spec`, `specs` or `__tests__`, or a file name ending `_test.go` or holding `.test.` or `.spec.` (`refs::is_test`). `app/Services/Inspector.php`, `src/respect.rs`, `docs/specification/…`, `latest/…` and `contest/…` are no longer test files.
**Over:** AD-1106's plain substring checks, which matched `spec` and `test/` inside other words.
**Because:** the slice's pattern names directories and file-name shapes; a substring inside a word is not one.
**Undo:** one function in `src/ctx/refs.rs`.
**Review:** no

### AD-40004 — Are `build/`, `dist/` and `storage/` skipped at any depth? · polish · 2026-10-10

**Chose:** only directly under the search root (`walk::ROOT_DIRS`); `vendor/`, `node_modules/` and `target/` stay skipped at any depth (`walk::VENDOR_DIRS`). `--vendor` lifts both. The walk test now plants `build/`, `storage/` and a nested `php/vendor/` and `php/node_modules/` as skipped, and `php/build/`, `app/storage/` and `web/dist/` as walked.
**Over:** slice 11's list at any depth, whose test planted `php/build/` as skipped.
**Because:** a nested `build/` or `storage/` is as often source as output (`src/build/`, `app/storage/`), and a `.gitignore` already hides the real output trees; a dependency tree is never source.
**Undo:** fold `ROOT_DIRS` back into `VENDOR_DIRS` and restore the planted `php/build/Gen.php`.
**Review:** decide — deviates from slice 11's walk test, which planted `php/build/` as skipped
