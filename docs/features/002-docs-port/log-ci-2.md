## Decisions

### AD-8201 — Why did round 1's fix not reach CI, and what ships now? · ci · 2026-10-13

**Chose:** ship the same change AD-8101 chose, and this time stage it by name inside the worker run: `git add -A -- src tests/fixture`. The change deletes `src/docs.rs` and moves `tests/fixture/.docs/` to `tests/fixture/v2/`. There are no code edits.
**Over:** a code change. CI fails the same way as in round 1 (`src/docs.rs` and `src/docs/mod.rs` both exist), so the cause has not changed.
**Because:** round 1 left the change staged from before its run and never ran `git add` itself. The driver committed only the files that worker wrote (`log-ci-1.md`, `retro.md`, commit 222795c), so HEAD still has `src/docs.rs`. On this tree the full gate (fmt, clippy `-D warnings`, test) is green in 52 s. `cargo fmt --check` also passes on a `git checkout-index` export of the index. No files are untracked and none are unstaged.
**Undo:** `git revert` the CI-fix commit.
**Review:** no
