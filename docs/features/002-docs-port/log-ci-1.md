## Decisions

### AD-8101 — Ship the uncommitted staged moves as the CI fix? · ci · 2026-10-13

**Chose:** commit the changes already staged in the index as they are: delete `src/docs.rs` (superseded by `src/docs/mod.rs`), move `tests/fixture/.docs/` to `tests/fixture/v2/.docs/`, and add the rest of `tests/fixture/v2/`. No code edits.
**Over:** deleting only `src/docs.rs`. HEAD code already reads `tests/fixture/v2/.docs/...` (`src/docs/migrate.rs:2030` `include_str!`, `src/docs/migrate_ideas.rs:514`, `tests/migrate.rs:595`), so the fixture move is needed too, or CI goes red on the next step.
**Because:** a slice ran `git mv` and staged the files, but its commits never took them. HEAD therefore has both `src/docs.rs` and `src/docs/mod.rs`, and `cargo fmt` refuses that pair. The full gate (fmt, clippy `-D warnings`, test) is green on this tree, and green again on a clean `git checkout-index` export of the index inside a new repo.
**Undo:** `git revert` the CI-fix commit, then restore `src/docs.rs` deleted and the fixture paths.
**Review:** fyi
