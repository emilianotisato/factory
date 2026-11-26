---
title: "testguard: ignore purely additive top-level use and import merges"
state: "open"
added: "2026-11-26"
origin: "harvest 2026-11-26"
---

In PRD-020 (`BLOCK-1` on `tests/lanes.rs` and `BLOCK-2` on `tests/closeout.rs`), adding new test functions at the bottom of an existing test file triggered `testguard` solely because `use foo::a;` was grouped into `use foo::{a, b};` at the top of the file (zero existing test lines modified).

**Shape:** in `src/testguard.rs`, allow a modified hunk in a pre-existing test file when every removed `-` line is a top-level import/use statement whose imported symbol(s) all still appear in the hunk's added `+` lines (or normalize Rust `use` / JS `import` groups). Keep deletions of imports or changes inside test functions strictly guarded.
