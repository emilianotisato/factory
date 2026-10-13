## Decisions

### AD-8301 — Why did rounds 1 and 2 not reach the commit, and how does this round land the fix? · ci · 2026-10-13

**Chose:** ship the same change as AD-8101: delete `src/docs.rs` and move `tests/fixture/.docs/` to `tests/fixture/v2/`. This round names every path in a `git add --` call that has no `cd`, then leaves the 10 deletions unstaged (` D`) and the 19 new files staged (`A `).
**Over:** another `git add -A -- src tests/fixture`, or a code change. CI still fails on the same pair, `src/docs.rs` next to `src/docs/mod.rs`.
**Because:** the driver commits only the paths in a worker's stream (`src/commit.rs` `Pathspec::from_stream`). A `git add` of a folder only adds the folder's new files that were not changed at spawn (`src/scheduler.rs` `unnamed`). These files were staged before the round began, so the swept list held none of them. A Bash call that contains a `cd` names nothing at all (`rooted_scripts`). `commit::commit` also drops any path that `ls-files --cached` does not list, and a staged deletion is not listed. Leaving the deletions unstaged lets the driver's own `git add` stage them. On a scratch copy of the index, that add gives 1 D, 9 R and 10 A against HEAD. The full gate (fmt, clippy `-D warnings`, test) is green on this tree in 53 s.
**Undo:** `git revert` the CI-fix commit.
**Review:** fyi
