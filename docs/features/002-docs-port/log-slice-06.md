## Blockers

## Decisions

### AD-601 — How does `docs retro` exit 1 inside slice 01's dispatch? · slice-06 · 2026-10-13
**Chose:** `native` runs `Verb::Retro` itself and returns its exit code (0 with a row, 1 with none); `dispatch` keeps `Result<()>` and its `Retro` arm is `unreachable!`.
**Over:** changing `dispatch` to return `Result<i32>`.
**Because:** `dispatch`'s arms (slice 02's `log` arms among them) `return print(…)` as `Result<()>`; changing its type would rewrite another slice's code while it is being built.
**Undo:** make `dispatch` return `Result<i32>` and move the arm into it.
**Review:** no

### AD-602 — The guarded-column list as a const · slice-06 · 2026-10-13
**Chose:** move the `ALTER TABLE` list out of `migrate` into `const COLUMNS`, same entries, same order; `migrate` loops over it.
**Over:** leaving it inline, as "nothing else changes in `migrate`" reads.
**Because:** task 1's v2-store test must add "every guarded column", and a copy of a 29-entry list in the test would drift from the real one.
**Undo:** inline the const back into `migrate` and copy the list into the test.
**Review:** no

### AD-603 — How task 4's test makes its v2 store · slice-06 · 2026-10-13
**Chose:** run the metrics fixture's finished run, then `DROP TABLE retros` and reset `schema_version` to rows 1 and 2; "before" is `factory report` on a `cp -a` copy of the data dir taken while the store was at version 2, checked by a raw `rusqlite` read.
**Over:** building the store from the `SCHEMA` text, which is private to the library.
**Because:** a downgraded store has exactly PRD-003's shape (every column, no `retros`, versions 1-2) and keeps a real run with workers, gates and tool calls to report on.
**Undo:** expose a v2 schema helper from `db.rs` and build the store from it.
**Review:** no

### AD-604 — How `docs retro` prints the minutes · slice-06 · 2026-10-13
**Chose:** Rust's shortest form of the stored `REAL`: `16`, `2.23`.
**Over:** `factory report`'s `{:.2}` (`16.00`).
**Because:** the slice says only "`key: value`" with empty for null; the row prints what the store holds, and the closeout writer (slice 10) can round before it writes.
**Undo:** format the three minute columns with `{:.2}` in `src/docs/retro.rs` `lines`.
**Review:** fyi

## Traps

- The store enforces foreign keys: a `retros` row whose `run_id` names no `runs` row fails with `FOREIGN KEY constraint failed`. Insert the run first in tests; imported rows (slice 07) must pass `run_id: None`.
- The base `SCHEMA` already declares some columns the guarded list also adds (`workers.commit_sha`, …): a hand-built old store must check `pragma_table_info` before each `ALTER`, as `migrate` does.
- Another lane edits `src/cli/docs.rs` at the same time: make small `Edit`s anchored on lines you own, and re-read before each.
