---
slice: 02
status: done
depends_on: [1]
lane: 1
---

# Slice 02 — Recount and per-worker tokens (FR-21, rest)

**Source**: [`prd.md`](prd.md) → §3 FR-21, decisions 3 and 4 ("`factory report <run> --recount`", "`report --per-worker` prints tokens") and the FR-21 acceptance targets. **Depends on**: slice 01 — it rewrites the `sub_*` columns and reuses the split collector and the transcript reader; without them there is nothing to recount.
**Created**: 2026-10-13 · **Status**: pending

**Progress**:
- Built: `factory report <run> --recount` (`src/cli/report.rs` `recount`): refuses a run whose phase is not over (exit 1, names the phase), reads every ended worker's stream first (one missing → nothing rewritten, AD-202), deletes the run's `tool_calls`, then per worker in `started_at` order rewrites the usage columns only (`db::update_worker_usage`), re-inserts its calls through `metrics::insert_calls` (factored out of `record_exit`, shared `seen_run`), and rewrites `sub_*` from the transcript dir: transcript gone → nulls and `transcript missing, own tokens only`; unreadable dir → nulls and `subagents unreadable: <error>`. One `recount:` line per worker (stderr under `--json`, AD-205), then the normal report. `events`, `gates` and docs are never written. `--per-worker` gains `weighted_own`, `weighted_sub` (null when `sub_*` are), `subagents`, in text and JSON. The stub now writes every session's `<session>.jsonl` transcript (AD-206).
- Tests: `tests/metrics.rs` `recount_rewrites_the_tokens_and_tool_calls_from_the_streams` (mutated polish row, half its calls deleted → row, every `tool_calls` row, `gates`, `weighted_tokens` and `events.jsonl` back to the pre-mutation values; per-worker JSON figures), `recount_without_the_transcript_keeps_own_tokens_only` (the note, `sub_*` null, `—` in the per-worker text, events byte-identical, a `building` run refused with exit 1).
- Inner loop (`cargo test`): 7 runs (two `--test metrics` only), 5 red or cut short, ~14 min. Run 1 caught slice 04's `model_id` appended to the per-worker line, which broke my exact asserts (fixed with prefix matches). The other red runs failed only on sibling slices' in-flight work: the `EXTRA_KEYS` list slices 03, 04 and 05 are appending to, and slice 04's `tests/knobs.rs` hanging past 10 min. The full run with `--skip` on slice 04's two in-progress `knobs` tests was green (20 targets, 64 s). The last `--test metrics` run after a format fix: all slice-02 tests green, `report_prints_the_retro_keys_then_the_run_keys` red on the siblings' new keys `knobs`, `hypothesis`, `pack_by_role`.
- Deviations: tests run on slice 01's subagent scenario, not `finished_run()` (AD-201). Prove (task 4) is left for CP-1.
- **Prove (task 4), driven at CP-1 (2026-10-12)**: the FR-21 row, second half, green on the real store (backed up as `factory.db.pre-003`, recount kept). `024-20261012-e894` reads `weighted_tokens` 18,465,008 against the target 18,465,008; `recount: polish ddbb0927 own 1948573 sub 2294981 subagents 4`; `cockpit_tokens` 974,031 before and after. `018-20261011-6490` reads 8,131,594 against 8,131,594, with `cockpit_tokens` 619,229 unchanged. Evidence: `evidence/cp-1-fr21-recount.txt` (every report output). Found: the arca "cockpit" transcript the A/B used (`f4063ce4`) is slice 01's build worker, so 974,031 is not the cockpit's cost. Also, `--recount` leaves `model_id` null on these pre-003 rows, so qa-guide step 4's `claude-opus-…` check reads `—`.

**Slice**: After this slice the two A/B runs stored on this machine can be corrected in the store with one command, and `report --per-worker` says what each worker cost. It leaves `model_id` to slice 04 (which appends it to the same per-worker line) and the retro to slice 06.

## Current code / environment

- **`report` today**: `src/cli/report.rs:34-47` `ReportArgs { run, json, cockpit: Vec<PathBuf>, per_worker }`; `PerWorker { id, role, slice, session_id, lookups, ctx_calls, ctx_share, repeat_rate }` (`:192-201`) and its `text()` (`:205-216`); `per_worker()` (`:231-249`) builds the rows from `workers` and `tool_calls`; `report()` (`:522-550`) opens the store, computes, prints text or JSON. No token field on `PerWorker` (prd §3).
- **The stored streams**: `runs/<id>/workers/<session>.jsonl` (`src/worker.rs:244-246` `stream_path`), one per worker, plus `<session>.stderr` and `.settings.json`. `stream::parse(text)` (`src/stream.rs:430`) and `Collector::push` rebuild a `Summary` from the file; after slice 01 the collector splits own and subagent lines.
- **How exits are written**: `finish_worker` (`src/db.rs:434-467`) writes the token and context columns together with `outcome`, `terminal_reason` and `ended_at`; a recount must rewrite the first group only. `insert_tool_calls` (`:598-627`) appends; there is no delete per worker today. `metrics::record_exit` (`src/metrics.rs:453+`) seeds `seen_run` from `db.run_targets(run_id)`, so `repeat_in_run` depends on insertion order: a recount must clear the run's `tool_calls` first and replay the workers in `started_at` order.
- **The transcript may be gone**: prd §3, accepted cost: "a stored stream for a worker that Claude Code's transcript dir has since lost gets its own tokens only, and the report says so per worker". Three cases to tell apart with slice 01's reader: the `<session>.jsonl` transcript itself is missing (own tokens only, a per-worker note), the transcript exists and has no `<session>/` dir (zeros, `subagents = 0`), the dir exists and cannot be read (nulls plus a warning line; "the recount writes no events").
- **Where the repo path comes from**: `runs.repo` (`RunRow.repo`, `src/db.rs:111-122`, `run_row` `:345`). For run 024 it is `/home/emiliano/Projects/Prokey/wt-b`, and the transcripts are under `~/.claude/projects/-home-emiliano-Projects-Prokey-wt-b/` (verified 2026-10-13, 17 session files, `ddbb0927…/subagents/` with four `agent-*.jsonl`).
- **The real targets**: `001-driver/ab/runs.md:233`, `:239`: arca workers 16,170,027 + polish subagents 2,294,981 = **18,465,008**; `001-driver/retro.md:105`, `:220`: icg workers 6,078,688 + 2,052,906 = **8,131,594**. `cockpit_tokens` (974,031 on arca) is computed from `--cockpit` transcripts, not from worker rows, and must not move.
- **The report unit test** asserts the KEYS+EXTRA_KEYS order and `lines()[16]`, `[17]`, `[21]`: append, never insert.
- **Deliberate non-changes** — *(prd §3 decision 3: `--recount` "never touches `events`, `gates` or the run's docs"; prd §12: no stream fallback for subagent usage; prd §3 decision 4 names `model_id` on the per-worker line, which is FR-23's column and lands in slice 04.)*

## Tasks

### 1. `factory report <run> --recount` — high — blocked by: —
**Source**: §3 FR-21: "`factory report <run> --recount` re-reads the stored streams under `runs/<id>/workers/` and the transcript dirs, and rewrites the token, context and `tool_calls` rows of that run. It exists so the two A/B rounds on this machine can be corrected in the store, and so a layout fix can be applied to past runs. It never touches `events`, `gates` or the run's docs."

`src/cli/report.rs` (`ReportArgs.recount: bool`, a `recount(db, run_dir, run_id, out)` step before `compute`), `src/db.rs` (new `update_worker_usage(id, &Summary)` writing only `input, cache_write, cache_read, output, thinking, cost_usd, num_turns, peak_context, start_context`, and `delete_tool_calls(run_id)`), `src/metrics.rs` (factor the `ToolCallInsert` building out of `record_exit` into a function the recount calls with its own `seen_run`). For each worker row of the run in `started_at` order, skipping rows with `ended_at` null (still running, no result yet): parse the stream file through the collector, write the usage, re-insert its `tool_calls` (with `pack_doc` from `worker_pack_files` as today), then slice 01's `subagents` reader on the transcript path built from `runs.repo` and `paths::claude_config_dir()`. Print one line per worker: `recount: <role> <session> own <weighted> sub <weighted|—> subagents <n|—>`, with `transcript missing, own tokens only` or `subagents unreadable: <error>` appended in those cases. Then fall through to the normal report of the rewritten store.

The trap: `quota::weighted` and the quota pause read the same rows; a recount of a live run would move its budget under the driver. Rows with `ended_at` null are skipped, and the whole run is refused with exit 1 when its phase is not `finished`, `stopped` or `failed`, naming the phase.

**Acceptance:** harness test: after `finished_run()`, zero the polish row's `output` and null its `sub_*` columns through `rusqlite`, delete half its `tool_calls`, run `factory report <id> --recount`, and assert the row's `output`, `sub_*` and the `tool_calls` count equal their values before the mutation and `weighted_tokens` is back to its pre-mutation figure; a second test removes the fixture `<CLAUDE_CONFIG_DIR>/projects/<slug>/<session>.jsonl` for the polish worker and asserts the `transcript missing, own tokens only` line and `sub_*` null, `events.jsonl` unchanged byte for byte.

### 2. `--per-worker` prints tokens — high — blocked by: —
**Source**: §3 FR-21: "`report --per-worker` prints tokens: `weighted_own`, `weighted_sub`, `subagents`, `model_id` (FR-23), next to the lookup columns it prints today, and `--json --per-worker` carries the same fields (today `PerWorker` has no token field, `src/cli/report.rs:192-216`)."

`src/cli/report.rs`. `PerWorker` gains `weighted_own: u64` (`quota::weigh` of the row's own columns, rounded), `weighted_sub: Option<u64>` (null when `sub_*` are null), `subagents: Option<i64>`; `text()` appends them after `repeat_rate` as `own <n> sub <n|—> subagents <n|—>`; `--json --per-worker` carries the three fields. `model_id` is slice 04's: it appends after `subagents` when it lands; leave a comment naming that, nothing else.

**Acceptance:** `tests/metrics.rs`: after the slice 01 scenario, `factory report <id> --json --per-worker` has, on the polish row, `weighted_own` equal to `weigh` of its own columns, `weighted_sub` equal to `weigh` of the fixture sums and `subagents == 2`, and on a build row `weighted_sub == 0`, `subagents == 0`; the text form of the same run prints the three columns on every row and `—` for a row whose `sub_*` are null.

### 3. Verify — medium — blocked by: 1, 2
`cargo test` (the inner loop; the full gate `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` runs at this slice's checkpoint).

**Acceptance:** inner loop green, one commit.

### 4. Prove — at the checkpoint — blocked by: 3
Rows from [`prd.md`](prd.md) §9 this slice must prove: the **FR-21 row, second half**: on this machine, `cp ~/.local/share/factory/factory.db` aside, then `factory report 024-20261012-e894 --recount` and `factory report 024-20261012-e894` → `weighted_tokens` within 1% of **18,465,008** with `cockpit_tokens` untouched (974,031 with the cockpit transcript the A/B used); `factory report 018-20261011-6490 --recount` → within 1% of **8,131,594**. The step is written out in this feature's [`qa-guide.md`](qa-guide.md). Driven at CP-1 together with the full gate; both report outputs go in this slice's **Progress**. Below the UI: `run` commands, no browser.

**Acceptance:** both report outputs pasted in Progress, each within 1% of its target, and the `recount:` line of `ddbb0927` showing the polish split (own about 1.95M, subagents about 2.29M).
