## Traps

- `docs preflight` never returns an empty `judge[]`: `docs.mjs:1706` pushes the harness line unconditionally, and the fixture gets four (unstruck OPEN-1, harness, no rubric, `on main — create feat/fixture`). A test that needs an unparked `factory run` on the fixture cannot go through the binary; call `cli::run::ensure_branch` and `begin` with a synthetic `docs::Branch` instead.
- `docs log merge 001` on the fixture exits 2 (`no agent-log.md in 001-fixture — nothing to merge into`); a test that wants a 0 has to write an `agent-log.md` first.

## Blockers

_(none)_

## Decisions

### AD-201 — How does the driver stamp dates without a date crate? · slice-02 · 2026-10-09

**Chose:** `src/clock.rs`: seconds since the epoch to RFC 3339 and `yyyymmdd` with Howard Hinnant's twelve-line `civil_from_days`, unit-tested on five known instants
**Over:** `chrono` or `time`, a runtime dependency for two format strings
**Because:** the PRD names the run id (`<NNN>-<yyyymmdd>-<4 hex>`) and `ts` on every event but no library (§3, §19 lists the dependencies it minds); the arithmetic is public domain and never changes
**Undo:** replace two functions with `chrono::Utc::now().format(…)` and add the dependency
**Review:** fyi

### AD-202 — How does `find_live` know a pid is alive? · slice-02 · 2026-10-09

**Chose:** read `/proc/<pid>/stat` and treat a missing file, `Z` or `X` as gone
**Over:** `kill(pid, 0)` through `libc` (a dependency for one syscall) or `std::process` (no liveness query)
**Because:** §19 makes the driver Linux-only, and a zombie run process must count as gone or `factory run` would refuse forever
**Undo:** one function, `src/run.rs::pid_alive`
**Review:** no

### AD-203 — Which existing run does `factory run` resume? · slice-02 · 2026-10-09

**Chose:** the newest run for this repo and feature whose phase is not `finished` or `failed`; a terminal run is left alone and a new run is created beside it
**Over:** resuming any existing run dir (a finished run would be reopened and its board re-read with nothing to do), or always creating a new run (the resume of §3 bullet 1 would never happen)
**Because:** task 3 says "an existing run dir → skip preflight, `run_resumed`" without saying which; a terminal run has nothing to resume
**Undo:** `src/run.rs::find_resumable`
**Review:** fyi

### AD-204 — What does an `ask_opened` event carry? · slice-02 · 2026-10-09

**Chose:** `{id: "ASK-n", source: "ask" | "judge", text}` — the tool's line verbatim and where it came from
**Over:** `{id, text}` only, which loses whether `/deliver`'s "proceed as-is" shortcut applies, or one event holding every question, which slice 04 would have to split to answer one
**Because:** §3 says one question each but not the payload; slices 04 and 10 read it, and BLOCK-201 may turn on `source`
**Undo:** one field in `src/events.rs::EventKind::AskOpened` and its readers
**Review:** fyi
