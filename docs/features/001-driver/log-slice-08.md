# Agent log — slice 08

## Traps

- **`Fixture::calls()` can read a call file mid-write (2026-10-10)**: `a_run_killed_during_the_checkpoint_worker_resumes_at_the_checkpoint` kills the driver while a stub worker runs; once in ten runs `calls/3.json` was empty when the harness parsed it (`EOF while parsing a value`) and the test passed on three reruns. A flake of the harness, not of the slice under test; rerun before chasing it.

## Blockers

_(none)_

## Decisions

### AD-801 — Where are `tool_calls` and the worker gate rows written, and where does the gate string come from? · slice-08 · 2026-10-10

**Chose:** `metrics::record_exit(run, worker_id, summary)` from both `settle`s (`scheduler::settle`, `checkpoint::settle`) right after `finish_worker`, so every closed row — build, fix, checkpoint, dead or limited — gets its calls in one transaction and its inner runs as `gates` rows. The gate pair comes from `docs gate` / `docs gate --inner` once per driver process at the top of `scheduler::drive_inner` and rides on `Run` as a non-persisted `gate_cmds` field (`None` in `Run::create`/`open`, so a `factory report` process never asks for it).
**Over:** classifying in `stream::Collector` (it would need the gate string at spawn, threaded through `Worker::spawn` and the pack); or re-running `docs gate` at every worker exit (a node process per exit).
**Because:** §10 says the rows exist and what they hold, not where the driver learns the gate command or when it writes; the slice only says "inserted when the worker's stream is parsed on exit".
**Undo:** move the call into `Db::finish_worker` and pass the gate pair as an argument.
**Review:** no

### AD-802 — Does the report stop at §10's twenty-one keys? · slice-08 · 2026-10-10

**Chose:** no — after `score` and the twenty keys, in that order, it prints `worker_gate_runs`, `worker_gate_red` and `cockpit_peak_context` (`0 (no --cockpit)` without the flag; `null` in `--json`). Task 3's text says worker runs "are reported separately as `worker_gate_runs`" and task 4 says `--cockpit` "sets `cockpit_tokens` and `cockpit_peak_context`", while task 3's acceptance says "exactly the 12 + 8 keys in that order". The first twenty-one lines satisfy the order; the extras follow so nothing the tasks named is lost. `tests/metrics.rs` pins both halves.
**Over:** exactly the 12 + 8 keys, dropping the three the task text names.
**Because:** the slice contradicts itself between its task text and its acceptance; §10 lists keys without saying "only these".
**Undo:** delete the three `format!` lines at the end of `Report::lines` and the three fields; `KEYS`/`EXTRA_KEYS` in `cli::report` name them.
**Review:** decide

### AD-803 — What do a worker gate row's `minutes` and `red` carry? · slice-08 · 2026-10-10

**Chose:** `minutes` is the gap from the `Bash` call's `timestamp` to its `tool_result`'s, else to the next call's, else `0.0` — the column is `REAL NOT NULL` since slice 02, so the slice's "else null" cannot land without a schema change; `at` is the call's timestamp (now, without one). `red` is the `tool_result`'s `is_error`, or a line starting `FAIL` or `Exit code <n≠0>` in the first 4096 characters of the result (`stream::ToolUse.result_head`, kept for every call; `result_chars` still counts the whole). `kind` is `slice`, `cmd` the command as issued.
**Over:** making `minutes` nullable (an `ALTER` on a column slice 06 reads as `f64` everywhere); keeping the whole result text in the summary (a worker's `cat` of a large file would sit in memory for nothing).
**Because:** §10 says the rows exist with `cmd, red, minutes`; neither it nor the slice says what a timestamp-less stream yields on a `NOT NULL` column, nor how much of a result to keep.
**Undo:** a nullable `minutes` is one guarded `ALTER` plus `GateInsert.minutes: Option<f64>`; `RESULT_HEAD` is one constant.
**Review:** decide

### AD-804 — Where does the report's `start_context` come from? · slice-08 · 2026-10-10

**Chose:** a `workers.start_context INTEGER` column (guarded `ALTER`, `SCHEMA_VERSION` unchanged) filled by `finish_worker` from `Summary.start_context`, the first assistant message's `ctx` as `measure.py` defines it. The report's `start_context` is the smallest across the run's workers; `peak_context` stays `MAX(peak_context)`.
**Over:** re-parsing the worker's stream at report time (§10 "written as events happen, not reconstructed"; the slice says the report is a `SELECT`); or using the result's usage (the last turn's context, not the first).
**Because:** §10 names `start_context` among the keys and the `workers` columns it lists stop at `peak_context`; nothing says where a per-worker first-message context lives.
**Undo:** drop the column from the `ALTER` list and compute 0; `tests/metrics.rs` asserts the column is set.
**Review:** decide

### AD-805 — How far does the `measure.py` port go? · slice-08 · 2026-10-10

**Chose:** `measure.rs` ports `assistant_msgs` (assistant rows with `usage` and `timestamp`, deduped by `message.id` with the last occurrence winning as a dict assignment does, sorted by time), `ctx`, `totals`, the `skills/deliver` start marker, the `<session>/subagents/*.jsonl` sum with peak as max, and `combine` (tokens add, start from the first path, peak the max). It also sums `input_tokens`, which `measure.py` does not, so `cockpit_tokens` can be weighted with slice 07's weights as task 4 asks. The build/polish minute markers (`skills/polish`, `docs.mjs ship`, `Agent` descriptions) stay out: the driver has its own clock for `build_minutes` and `polish_minutes`. The script lives at `~/Dropbox/dev/claude-code-skills/deliver/bin/measure.py`, not the slice's `~/.claude/skills/feature-docs/bin/measure.py`; the pinned command in `measure::tests` names the real path.
**Over:** a line-for-line port including the minute markers (dead code: the report never reads them).
**Because:** task 4 cites `measure.py:42-67, 87-150` for the token keys only; §10 asks for "the same way `measure.py` does" about `cockpit_tokens`.
**Undo:** add the marker scan to `measure::measure`; nothing else depends on its absence.
**Review:** no

### AD-806 — Kinds and repeat flags §10 leaves open · slice-08 · 2026-10-10

**Chose:** a tool §10 does not name (`Agent`, `Skill`, `WebFetch`, …) is kind `other` with target = tool name plus its input serialised, so it hashes and repeats like the rest; a `Bash` call is classified in the slice's order (lookup word first, then the gate strings, else `run`), so `cat ./gate.sh` is `bash-cat` and `./gate.sh --inner | tee out` is `run-gate`; the first word is read after `cd …` segments, `env`, `time` and `VAR=value` tokens, with any directory prefix dropped (`/usr/bin/rg` is `rg`). `repeat_in_run` means "the same target seen earlier in this run by any worker, this one included" (so it is never below `repeat_in_worker`); `repeat_rate` divides the lookup rows with `repeat_in_worker = 1` by the lookup rows, the in-worker literal re-hit rate `ctx-research.md` measured.
**Over:** refusing or dropping unknown tools (every call must have a row, §10); `repeat_in_run` as "seen by an earlier worker only" (reads cleaner but loses the superset the acceptance's second-worker case still satisfies).
**Because:** §10 lists six kinds and two flags without the unknown-tool case or the flags' overlap.
**Undo:** `metrics::classify`'s fallback arm and the `seen_run.insert` inside `call_rows`.
**Review:** fyi
