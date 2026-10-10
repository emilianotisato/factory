## Decisions

### AD-60001 — Where do the hook guard's wrapper, shell and doubt cases live? · polish · 2026-10-10

**Chose:** the tables in `tests/hook.rs`. The unit test `cli::hook::tests::the_guard_reads_wrappers_shells_and_doubt` is gone. Its 30 cases that were not already in a table (22 denied, 8 allowed) now sit in `every_segment_and_every_wrapper_is_read`, `doubt_is_denied_when_it_names_gh_or_push` and `the_rest_gets_no_decision`. Its five other cases ("gh --version", "env -u CLAUDECODE git push", "git stash push -m wip", "echo gh push", "cargo test --test hook") were already in a table word for word, so they were dropped.
**Over:** keeping both lists, which kept the same never-list in two places with no case unique to the unit test.
**Because:** each case now goes through `guard` and the full `hook_with` decision, where the unit test checked only `guard`. That is the same coverage plus the decision and its reason. The unit test's own doc comment already said the never-list cases belong in `tests/hook.rs`. `tests/` is outside the source scan in `the_driver_never_merges_or_readies_a_pr`, so the cases stay safe to name there.
**Undo:** restore the unit test from the parent of this commit with `git show`, and drop the added rows from the three tables.
**Review:** no

### AD-60002 — Is `allow_is_no_decision_and_deny_is_one_line` still needed? · polish · 2026-10-10

**Chose:** no, so it is removed from `src/cli/hook.rs`.
**Over:** keeping it next to the integration tests.
**Because:** Ln7 is covered twice over. `decide` in `tests/hook.rs` asserts empty stdout for every case in `the_rest_gets_no_decision`. For every denied case, it asserts exactly one JSON line with `permissionDecision` "deny" and reason `NEVER_HINT`, and "gh --version" is in `the_second_pass_cases_are_denied`. `tests/cli.rs::hook_denies_gh_and_git_push_and_fails_closed` checks the same through the binary.
**Undo:** restore the test from the parent of this commit.
**Review:** no

### AD-60003 — Is the Vue unit test in `ctx::lang` still needed? · polish · 2026-10-10

**Chose:** no, so `a_vue_file_has_one_unit_per_script_block` is removed from `src/ctx/lang.rs`.
**Over:** keeping it for C4 alongside the integration test.
**Because:** `tests/ctx.rs::every_vue_script_block_is_read` covers the same fix through the same code: `Unit::all` and `definitions` for `outline`, and `Parsed::of` for `def` and `refs`. It checks both script blocks at their file line numbers (2 and 13), a clean parse in the exact `def` output, a comment in the second block dropped by `refs`, and a call in the template between the blocks kept as code.
**Undo:** restore the test from the parent of this commit.
**Review:** no
