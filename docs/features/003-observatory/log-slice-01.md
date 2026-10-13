## Traps

- `grep -rn` over the repo is refused by the hook: use `factory ctx refs <word>` or `git grep -n` (its `-A`/`-B` flags go before the pattern).
- An inline Bash heredoc whose code holds the word `push` (a Rust `v.push(p)`) is refused as a `git push`: write the script to a scratch file with Write and run `python3 -I <file>`.
- `tests/stubs/claude` runs its role branches at top level, so a top-level `const` declared below the polish branch is still uninitialised when the branch calls a function that reads it (a TDZ throw, the stub dies, the driver retries the polish with the default step). Keep fixtures inside the function that uses them.
- `tests/commits.rs` `a_file_git_added_by_name_is_committed_beside_a_live_sibling` failed once under the full suite's load (lane A's commit held lane C's files), then passed 5 of 5 alone and in the next two full runs. Rerun before chasing it.

## Decisions

### AD-101 — How does the cut polish stream show a subagent context larger than the worker's own? · slice-01 · 2026-10-12
**Chose:** `tests/stubs/polish-subagents.jsonl` keeps the real `ddbb0927` lines but raises each subagent line's `cache_read_input_tokens` by 200,000, so every subagent context (221k-237k) is larger than the own peak (89,526).
**Over:** an uncut recording, whose subagent contexts were smaller than the worker's own and so could not tell an own-only peak from a mixed one.
**Because:** task 6's acceptance asks for a fixture where the subagent lines are larger on purpose; without it the `peak_context` assertion passes for the wrong reason.
**Undo:** drop the +200,000 in the cut and the `peak_context` assertion in `tests/metrics.rs`.
**Review:** no

### AD-102 — What does the cockpit total do when one subagent file cannot be read? · slice-01 · 2026-10-12
**Chose:** `measure()` (the `--cockpit` path) now goes through `measure::subagents`; an `Unreadable` dir or file adds no subagent usage and no error, as an unreadable dir did before.
**Over:** keeping the old split behaviour, where an unreadable dir was skipped but an unreadable file inside it failed the whole cockpit measure.
**Because:** one reader for both paths, and the cockpit figure never failed on the subagents dir before.
**Undo:** match `Subagents::Unreadable` in `measure()` and return the error.
**Review:** fyi

### AD-103 — What counts as one own turn when a line with usage has no message id? · slice-01 · 2026-10-12
**Chose:** `num_turns` is the number of distinct own `message.id`s with usage, plus one per own usage line that carries no id.
**Over:** dropping id-less lines, which would undercount a stream that omits ids.
**Because:** real streams always carry the id, so this only matters for hand-written fixtures, and a count that never drops a turn is the safer floor.
**Undo:** remove `anonymous_turns` from `Collector` in `src/stream.rs`.
**Review:** no

### AD-104 — Does the per-role split add up to `weighted_tokens` exactly? · slice-01 · 2026-10-12
**Chose:** `weighted_by_role` rounds each role's own and subagent figures apart; `weighted_tokens` rounds the run's sum once. The two can differ by a token or so per role.
**Over:** distributing the rounding remainder so the parts add up.
**Because:** the cache_read weight is 0.1, so only fractions of a token are lost; matching the total exactly would need a remainder rule nobody reads.
**Undo:** compute the role figures as integers before weighing, in `src/cli/report.rs` `weighted_by_role`.
**Review:** no

### AD-105 — The schema bump breaks a test this run did not write · slice-01 · 2026-10-12
**Chose:** `tests/run.rs` `opening_the_store_twice_applies_the_schema_once` now compares with `db::SCHEMA_VERSION` instead of the literal 1; its "one row in `schema_version`" check stays.
**Over:** a literal 2, which the next schema bump breaks the same way.
**Because:** the slice moves the version to 2 (task 1); the test's point is "applied once", not "version 1".
**Undo:** put the literal back.
**Review:** fyi
