## Traps

- Lanes 1-4 build at once in the same tree: `cargo test` stops at the first red test binary, and a sibling's half-written file breaks it (a missing struct field, a hanging `tests/knobs.rs`). Run `cargo test --no-fail-fast --lib --bins --test <each file but the sibling's>` under `timeout` to see your own targets, and say in Progress which red was not yours.
- `tests/metrics.rs` `report_prints_the_retro_keys_then_the_run_keys` checks the text report's keys exactly against the test's own `EXTRA_KEYS` copy: every slice that appends a key to `src/cli/report.rs` `EXTRA_KEYS` must append it to the copy in `tests/metrics.rs` too, in the same order.

## Decisions

### AD-501 — Which `LogSummary` does the ship store, when the `board` it holds predates polish? · slice-05 · 2026-10-13
**Chose:** `ship()` runs `docs status NNN --json` again right before `report::compute` and stores that `log`; if the tool fails it stores the held `board.log` rather than failing the ship.
**Over:** always the held `board` (misses the `AD` entries polish wrote), or failing the ship on a measurement read.
**Because:** the slice asks for the counts as they stand at ship; a count is never worth a stopped run.
**Undo:** replace the `docs.status(&feature)` read in `src/closeout.rs` `ship()` with `board.log`.
**Review:** no

### AD-502 — Where does the text report print the `quality:` line? · slice-05 · 2026-10-13
**Chose:** the seven new keys print as their own `key: value` lines at the end of `EXTRA_KEYS` (`findings: —` when null), and the one `quality:` line prints after all the keys and before the three threshold lines.
**Over:** the `quality:` line only, which would break the one-key-per-line order test.
**Because:** the slice names both the keys appended last and one `quality:` line; this keeps both without moving any existing line.
**Undo:** drop `.chain([r.quality_line()])` in `src/cli/report.rs` `report()`.
**Review:** no

### AD-503 — Is `polish_findings_missing` a cockpit line? · slice-05 · 2026-10-13
**Chose:** no: it is progress, seen in `events.jsonl` and `factory status`, never relayed by `factory wait`.
**Over:** relaying it like `subagents_unread`.
**Because:** a missing line costs only five blank numbers and nothing waits on it.
**Undo:** add `EventKind::PolishFindingsMissing` to `cockpit::is_cockpit` with a `line()` arm.
**Review:** no

## Untracked leftovers

- assets/contract-polish.md
- src/cli/run.rs
- src/closeout.rs
- src/config.rs
- src/events.rs
- src/stream.rs
- tests/checkpoints.rs
- tests/closeout.rs
