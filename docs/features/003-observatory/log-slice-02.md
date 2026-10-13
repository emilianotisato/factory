## Traps

- Slices 03, 04 and 05 build in the same working tree at the same time and edit `src/cli/report.rs`, `src/db.rs`, `tests/metrics.rs` and `tests/stubs/claude`. A full `cargo test` can go red on a sibling's half-done edit (two slices appending keys after `weighted_by_role`). Run `cargo test --test <target>` for your own tests, rerun the full suite later, and edit shared files with exact-string edits, never a whole-file rewrite.
- Slice 04 appends `, model_id <m>` to the `--per-worker` line while slice 02 is still building, so an `assert_eq!` or `ends_with` on that line breaks. Assert with `starts_with` or `contains`.

## Decisions

### AD-201 — Which run do the recount tests mutate? · slice-02 · 2026-10-13
**Chose:** both recount tests run on slice 01's `subagent_run` scenario (the `finished_run` plan with a polish that spawned two subagents).
**Over:** `finished_run()`, which the acceptance names, where every `sub_*` is zero.
**Because:** with zeros, "`sub_*` back to their values" passes even when the recount never reads the transcript dir.
**Undo:** call `finished_run()` in `tests/metrics.rs` `recount_*`.
**Review:** no

### AD-202 — What does the recount do when a worker's stored stream is gone? · slice-02 · 2026-10-13
**Chose:** it reads every ended worker's stream before writing anything, and refuses the whole recount (exit 1, "nothing rewritten") when one is missing.
**Over:** skipping that worker, which would lose its `tool_calls` rows to the run-wide delete the slice asks for.
**Because:** a recount that leaves the store half-rewritten is worse than none; every worker the driver spawns gets a stream file at spawn.
**Undo:** in `src/cli/report.rs` `recount`, skip the worker and carry its old `tool_calls` rows over.
**Review:** fyi

### AD-203 — Where does the recount get the gate pair that marks inner-loop calls? · slice-02 · 2026-10-13
**Chose:** from `docs gate` and `docs gate --inner` in the run's repo at recount time, as the driver reads them. When the repo or the docs tool is gone, there is no pair and an inner-loop call is classified as a plain `bash` call.
**Over:** keeping the stored `kind` of each old row, which a run-wide rewrite cannot match back to a call.
**Because:** the `kind` of a gate run is not an FR-21 figure, and the lookup figures do not depend on it. `gates` rows are never touched.
**Undo:** pass `None` as the gate pair in `recount`.
**Review:** fyi

### AD-204 — Which phases count as "finished, stopped or failed"? · slice-02 · 2026-10-13
**Chose:** `Phase::is_over()`, so `finished` and `failed`. A stopped run is phase `finished` with `stopped: true` (`src/cli/stop.rs:3`), so there is no `stopped` phase to test.
**Over:** a separate check of the `stopped` flag.
**Because:** the flag never appears without phase `finished`.
**Undo:** none needed.
**Review:** no

### AD-205 — Where do the `recount:` lines go with `--json`? · slice-02 · 2026-10-13
**Chose:** to stderr, so stdout stays one JSON object. Without `--json` they go to stdout before the report. The session prints as its first 8 characters, as `--per-worker` does.
**Over:** stdout in both modes, which breaks `--json` parsing.
**Because:** `report --json` output is read by scripts and tests.
**Undo:** write to `out` in `report()` regardless of `args.json`.
**Review:** no

### AD-206 — The per-worker line test was exact, and two slices append to that line · slice-02 · 2026-10-13
**Chose:** `tests/metrics.rs` `ctx_calls_are_labelled_and_the_report_reads_the_reversal_rule` now checks the per-worker line with `starts_with` (it includes the new `own … sub … subagents …` columns) and `contains`, in place of `assert_eq!` and `ends_with`. The stub `tests/stubs/claude` now writes every session's `<session>.jsonl` transcript under `CLAUDE_CONFIG_DIR`, as Claude Code does, so the recount can tell a lost transcript apart.
**Over:** exact matches that slice 04's `model_id` breaks again.
**Because:** the slice appends columns to that line; the test's figures (lookups, `ctx_share`, `repeat_rate`) are all still checked.
**Undo:** put back the exact strings with the full column list once slice 04 lands.
**Review:** fyi
