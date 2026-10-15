<!-- head: d9b7e3cd8577176f21376e03dcc867bc98934516 -->

## Plan

changed: 2 files, 700 lines against main...HEAD
- a (400 lines)
- b (300 lines)
size: local review and second opinion
second opinion: reviewed, 2 of 2 passes counted

## Leaks

verdict: clean

comment share: 0.0% (0 of 0 added lines)
clean

## Second opinion

exit: 0
log: /tmp/cp2-007/fr3.mJix/data/reviews/20261015-005408

### Pass 1 — a (400 lines, 0 s, grok-4.7-high) — counted

I'll read the observatory PRD in full, then check every file it cites against the claims.The PRD cites a lot of line ranges. I'll check those against the source and the 001/002 docs it points at.The draft’s acceptance numbers and several “today” citations do not match the code or the 001 write-ups they point at.

## Findings

**§3 / §9 — `factory report 024-20261012-e894` and `018-20261011-6490` print `weighted_tokens` within 1% of 19,439,039 and 8,750,823.** Checked `src/cli/report.rs:443-444` and `src/quota.rs:109-120` (`weighted_tokens` is the sum of worker rows; `cockpit_tokens` is a separate key), `docs/features/001-driver/ab/runs.md:133` and `:233`, `docs/features/001-driver/retro.md:105` and `:220-221`. Those two targets are the cockpit-inclusive totals. Arca workers are 16,170,027; adding the missed 2,294,981 gives 18,465,008, and 19,439,039 − 18,465,008 = 974,031, the cockpit, 5.0% under the target. Icg workers are 6,078,688; adding 2,052,906 gives 8,131,594, and the gap is the cockpit 619,229, 7.1% under 8,750,823. A recount that adds subagent tokens onto worker rows cannot land `weighted_tokens` inside 1% of either cited total.

**§3 — both of those figures are “the transcript weighing recorded in `001-driver/ab/runs.md`”.** Checked `docs/features/001-driver/ab/runs.md:239` and `docs/features/001-driver/retro.md:220`. 19,439,039 is in `runs.md`. 8,750,823 is only in `retro.md`. `runs.md` records the icg correction as “54.2%” and never states 8,750,823.

**§3 — `report` prints lookup shares “for the worker’s own calls, as the A/B research measured them.”** Checked `src/stream.rs:292-301` (every assistant `usage` updates `peak_context` / `start_context`; `parent_tool_use_id` is never read) and the same section’s accepted-cost note. That note, and the “Today” paragraph above it, say the A/B baselines mixed the worker’s calls with its subagents’. Own-only shares are a different measurement from the one the A/B recorded.

**§3 — subagent usage comes from the existing `measure()` reader at `<CLAUDE_CONFIG_DIR, else ~/.claude>/projects/<slug>/<session_id>/subagents/*.jsonl`, as four separate columns.** Checked `src/measure.rs:169-211` and `src/cli/report.rs:524-528`. `measure()` takes a caller-supplied `*.jsonl`, sums that file and `<path-without-.jsonl>/subagents/*.jsonl` into one `Measure`, and returns no own/sub split. An unreadable `subagents/` directory is skipped (`if let Ok`), so the result is zeros. The only caller passes `--cockpit` paths. Nothing builds a projects-dir slug, and `CLAUDE_CONFIG_DIR` is only a kept env name at `src/worker.rs:67`.

**§3 — a missing `subagents/` directory is the zero case, and a directory that cannot be read emits one `SubagentsUnread` event; `--recount` never touches `events`.** Checked `src/measure.rs:197-198`. The same section says the zero case keys on a missing directory, then §13 says that layout is unconfirmed and the zero case may have to key on an empty directory. The unread path emits an event, and the recount bullet forbids writing events. A recount of an unreadable directory cannot do both.

**§4 — a slice pack is the sections Slice, Block and answer, PRD sections, Outline, Lookup, Contract, Autonomy, Inner loop, and `pack_parts` sums to the pack file’s size minus its header line.** Checked `src/pack.rs:145-178` and `:385-388`. `Block and answer` is written only when `block` is `Some` (`:148-154`). The file starts with `<!-- factory-pack files: … -->\n\n`, so one header line plus a blank line. The polish pack writes two comment lines (`files` and `run dir`) and then the blank line (`:385-388`). The decision stores `chars`; the acceptance compares that sum to file size. Those are different quantities, and subtracting one header line leaves the blank line (and, for polish, the second comment) outside every section.

**§5 vs §12 — this PRD does not change models or effort, and the only skill edit is `/polish`.** Checked §5’s acceptance and §12. The acceptance requires the polish spawn to be recorded as `--model sonnet` from `docs/project.toml`, and §5 says `/factory-deliver` passes `--hypothesis`. §12 says models and effort are unchanged and the only `~/.claude/skills` edit is the `FINDINGS` line.

**§6 — `worker_gate_red` is a new key.** Checked `src/cli/report.rs:80-82`, `:402-422`, `:463-464`, `:505`. `gate_red` is driver rows only. `worker_gate_red` is already an `EXTRA_KEYS` entry, already computed from `source == "worker"`, and already printed.

**§6 — the “failed-then-fixed gate” scenario plus “red CI then green” reports `fix_commits = 1`, `ci_red_rounds = 1`, `ci_reruns = 0`.** Checked `tests/checkpoints.rs:602-605` and `:696-697`, `tests/closeout.rs:723-726` and `:799-810`, `src/checkpoint.rs:763-768`, `src/closeout.rs:1718-1718`. No test scripts a gate as `red, green`. The red-gate scenario is `red, red, green`, which emits two `fix_committed` events. `red:test, green` emits one `ci_red`, one `ci_fix_committed`, and no `ci_rerun`. `fix_commits` is defined as `FixCommitted`, and the CI commit is `CiFixCommitted`. Neither scenario, and not both together, yields `fix_commits = 1` with those CI counts.

**§6 / §13 — the polish return block is `assets/contract-polish.md:29-35`, and the worker leaves the frontmatter alone at `:21`.** Checked `assets/contract-polish.md`. The file ends at line 26. The return block is lines 20–26. Line 21 is `STATUS: done | blocked`. The frontmatter instruction is line 13. `src/pack.rs:394-402` does tell the worker the driver fills the twelve keys.

**§6 / §13 — `message.model`, `model_usage`, `permission_denials`, `api_error_status`, and rate-limit info are parsed and dropped (`src/stream.rs:184-210`).** Checked `src/stream.rs:360-373`, `src/quota.rs:56-78`, `src/cli/status.rs:97` and `:259-265`. `api_error_status` and `rate_limit` are stored on `Summary` and read by the quota pause and `factory status`. `model_usage` and `permission_denials` are copied onto `Summary` at `:360` and `:366`. `message.model` (`src/stream.rs:47`) is the one field the collector never reads.

**§7 — `PRD-001 §10` retires `measure.py` because FR-11 is over.** Checked `docs/features/001-driver/prd.md:141`, `docs/index.md:34`, `docs/features/001-driver/retro.md:214-223`. Retirement is conditional on `factory report` matching `measure.py` within 2%. The correction says the report missed the polish subagents. The index still lists FR-11 human QA as pending.

**§7 vs §11 — FR-25’s `## Numbers` section is PRD-002’s `## Numbers` table.** Checked `docs/features/002-docs-port/prd.md:132-136`. FR-19’s table is the twelve `RETRO_KEYS`. §7’s section is a per-role own/subagent/weighted/minutes/turns table, knobs, hypothesis, the quality block, pack composition, and a three-run comparison, between `<!-- factory:numbers -->` markers. §11 says those are the same section and that 002 must not write a second table.

**§7 — comparison stays inside the repo and never prints a cross-repo ratio; `factory retro 024-20261012-e894` shows a comparison row for `018-20261011-6490`.** Checked `docs/features/001-driver/retro.md:71` and `:165`, `docs/features/001-driver/ab/runs.md:12`. `018-20261011-6490` is icg-cloud. `024-20261012-e894` is arca-api. The same sentence’s “54% against 86%” is that cross-repo pair (`retro.md:220-221`: 54.2% and 86.4%).

**§7 — that command reproduces the corrected per-role run B table in `001-driver/ab/runs.md`.** Checked `docs/features/001-driver/ab/runs.md:223-239`. The table there is the uncorrected report (run B 17,144,058, polish 1.95M, cockpit as its own row). The correction is one missed total and one corrected run total, not a per-role own/subagent table.

**§7 — the driver writes the section at ship, and running `factory retro` again leaves `retro.md` byte-identical.** Checked `src/closeout.rs:575-590` and `src/scheduler.rs:1688-1701`. `fill_retro` runs in `ship`, before CI and before `RunFinished`. The section compares “the same repo’s last three finished runs.” At ship this run is not finished. After `run_finished` it is. A later `factory retro` is comparing a different cohort, so the file written at ship does not stay byte-identical.

**§7 — per-role weighted figures in the section equal `report --json`.** Checked `src/cli/report.rs:39-40`, `:192-216`, `:530-537`. `--json` serializes `Report`. Per-worker rows are attached only with `--per-worker`, and `PerWorker` has lookups, `ctx_share`, and `repeat_rate`, no token fields. §3 puts `weighted_own` / `weighted_sub` on `--per-worker`, and no section adds them to the JSON object.

**§9 — the two-run replay is a manual step in `qa-guide.md`.** Checked `docs/features/001-driver/qa-guide.md` (the only `qa-guide.md` in the repo) and `docs/features/003-observatory/`. Neither file contains a recount or an observatory replay step.

**§9 — the stub writes `subagents/*.jsonl` under a `FACTORY_CONFIG_DIR`-controlled `CLAUDE_CONFIG_DIR`.** Checked `tests/harness/mod.rs:449-456`. The fixture env sets `FACTORY_CONFIG_DIR` to an empty config dir and does not set `CLAUDE_CONFIG_DIR`.

**§11 — `index.md` build order says 003 ships first, so 002’s `retros` table moves to schema v3.** Checked `docs/index.md:35`, `:38`, `:40`. Build order is still the placeholder. The 002 row still says the `retros` table is schema v2. Next free FR is FR-22, while this PRD’s frontmatter owns FR-21 through FR-25.

**§13 — CI produces `CiFixSpawned` at `src/closeout.rs:1534`.** Checked `src/closeout.rs:1530-1534` and `src/worker.rs:444-449`. Line 1534 matches events already stored, to count sessions. The emit is `worker.rs` when the CI-fix job is recorded.

**§13 — hook denials are not recorded, and a denied call is still a `tool_calls` row, at `src/cli/hook.rs:874-877`.** Checked `src/cli/hook.rs:874-877`. Those lines write the hook’s stdout verdict and return `Ok(0)`. They say nothing about the events table or `tool_calls`.

**§13 — the retro stand-in is `src/closeout.rs:2134-2150`.** Checked `src/closeout.rs:2132-2150` and `:365-372`. `:2134` is `retro_template()`. The stand-in write and `stood_in` call are `:365-372`.

**§2 — token cost is zero.** The next sentence in that bullet says the `FINDINGS` line is a few output tokens per polish.

**§1 / §6 — nothing reads `LogSummary`.** Checked `src/docs.rs:270-276` and `tests/docs.rs:34`. No driver path reads `log.decisions`, `decide`, or `blocks`. `tests/docs.rs:34` asserts `s.log.decisions`.

## Acceptance criteria and where they would move

| Criterion | Surface |
|---|---|
| Recount then `weighted_tokens` within 1% of 19,439,039 and of 8,750,823 | `factory report` stdout, key `weighted_tokens` (`src/cli/report.rs:496`). The check cannot pass against those two numbers; see the first finding. |
| “polish with subagents”: four `sub_*` columns, `subagents = 2`, `in_subagent = 1`, main-only `peak_context` | `workers` row and `tool_calls` rows (`src/db.rs:507`, `:630`). `in_subagent` is not a report column. |
| Build worker with no `subagents/` dir has zeros | That worker’s `sub_*` columns. §13 says the missing-directory predicate is unverified, so a real build worker may not be this case. |
| `pack_parts` has one entry per section the pack kind writes, summing to file size minus the header | `workers.pack_parts` and the file under `runs/<id>/packs/`. The equality does not match `src/pack.rs:145` and `:385-388`. |
| `report` prints per-role section means; the stub’s three roles differ | `factory report` stdout. Build, checkpoint, and polish packs differ (`src/pack.rs:146-177`, `:315-332`, `:390-412`). |
| Polish spawn is `--model sonnet`, every other call `opus` | Stub call JSON field `model` (`tests/stubs/claude:109`). |
| `knobs` has `polish.source = project`, `build.source = default`, `lanes`, both versions; `report` prints the knobs block | `runs` row and `factory report` stdout. |
| `--hypothesis` is in the row and in the retro | `runs.hypothesis` and the `## Numbers` section of `retro.md`. |
| `FINDINGS` fills five columns and `report` prints them | `runs.findings_*` and the report quality block. |
| No `FINDINGS` line: nulls, one event, `findings: —` | Those columns, the `events` row, and report stdout. |
| `fix_commits = 1`, `ci_red_rounds = 1`, `ci_reruns = 0` | Report quality block. The named scenarios do not produce that tuple. |
| `decisions` equals what `docs status --json` prints | Report `decisions` against `log.decisions` (`src/docs.rs:272`). |
| After ship, the section’s per-role weighted figures equal `report --json` | `retro.md` and `factory report --json`. No JSON field is specified for per-role weighted tokens. |
| A second `factory retro` leaves the file byte-identical | Bytes of `retro.md`. The ship-time write and a post-`run_finished` rewrite compare different finished-run sets. |
| `factory retro 024-20261012-e894` reproduces the corrected per-role table and a comparison row for `018-20261011-6490` normalised per slice | The generated section. There is no corrected per-role table in `runs.md`. There is no formula and no expected quotient for “normalised per slice” or “per changed line” in the PRD or in the A/B docs, so that figure has no oracle. Showing `018` on the `024` retro is the cross-repo row §7 forbids. |
| §9 “does the section read well beside the polish prose” | A person reading `retro.md`. |

(replayed for pass 1)

### Pass 2 — b (300 lines, 0 s, grok-4.7-high) — counted

I'll read the observatory PRD in full, then check every file it cites against the claims.The PRD cites a lot of line ranges. I'll check those against the source and the 001/002 docs it points at.The draft’s acceptance numbers and several “today” citations do not match the code or the 001 write-ups they point at.

## Findings

**§3 / §9 — `factory report 024-20261012-e894` and `018-20261011-6490` print `weighted_tokens` within 1% of 19,439,039 and 8,750,823.** Checked `src/cli/report.rs:443-444` and `src/quota.rs:109-120` (`weighted_tokens` is the sum of worker rows; `cockpit_tokens` is a separate key), `docs/features/001-driver/ab/runs.md:133` and `:233`, `docs/features/001-driver/retro.md:105` and `:220-221`. Those two targets are the cockpit-inclusive totals. Arca workers are 16,170,027; adding the missed 2,294,981 gives 18,465,008, and 19,439,039 − 18,465,008 = 974,031, the cockpit, 5.0% under the target. Icg workers are 6,078,688; adding 2,052,906 gives 8,131,594, and the gap is the cockpit 619,229, 7.1% under 8,750,823. A recount that adds subagent tokens onto worker rows cannot land `weighted_tokens` inside 1% of either cited total.

**§3 — both of those figures are “the transcript weighing recorded in `001-driver/ab/runs.md`”.** Checked `docs/features/001-driver/ab/runs.md:239` and `docs/features/001-driver/retro.md:220`. 19,439,039 is in `runs.md`. 8,750,823 is only in `retro.md`. `runs.md` records the icg correction as “54.2%” and never states 8,750,823.

**§3 — `report` prints lookup shares “for the worker’s own calls, as the A/B research measured them.”** Checked `src/stream.rs:292-301` (every assistant `usage` updates `peak_context` / `start_context`; `parent_tool_use_id` is never read) and the same section’s accepted-cost note. That note, and the “Today” paragraph above it, say the A/B baselines mixed the worker’s calls with its subagents’. Own-only shares are a different measurement from the one the A/B recorded.

**§3 — subagent usage comes from the existing `measure()` reader at `<CLAUDE_CONFIG_DIR, else ~/.claude>/projects/<slug>/<session_id>/subagents/*.jsonl`, as four separate columns.** Checked `src/measure.rs:169-211` and `src/cli/report.rs:524-528`. `measure()` takes a caller-supplied `*.jsonl`, sums that file and `<path-without-.jsonl>/subagents/*.jsonl` into one `Measure`, and returns no own/sub split. An unreadable `subagents/` directory is skipped (`if let Ok`), so the result is zeros. The only caller passes `--cockpit` paths. Nothing builds a projects-dir slug, and `CLAUDE_CONFIG_DIR` is only a kept env name at `src/worker.rs:67`.

**§3 — a missing `subagents/` directory is the zero case, and a directory that cannot be read emits one `SubagentsUnread` event; `--recount` never touches `events`.** Checked `src/measure.rs:197-198`. The same section says the zero case keys on a missing directory, then §13 says that layout is unconfirmed and the zero case may have to key on an empty directory. The unread path emits an event, and the recount bullet forbids writing events. A recount of an unreadable directory cannot do both.

**§4 — a slice pack is the sections Slice, Block and answer, PRD sections, Outline, Lookup, Contract, Autonomy, Inner loop, and `pack_parts` sums to the pack file’s size minus its header line.** Checked `src/pack.rs:145-178` and `:385-388`. `Block and answer` is written only when `block` is `Some` (`:148-154`). The file starts with `<!-- factory-pack files: … -->\n\n`, so one header line plus a blank line. The polish pack writes two comment lines (`files` and `run dir`) and then the blank line (`:385-388`). The decision stores `chars`; the acceptance compares that sum to file size. Those are different quantities, and subtracting one header line leaves the blank line (and, for polish, the second comment) outside every section.

**§5 vs §12 — this PRD does not change models or effort, and the only skill edit is `/polish`.** Checked §5’s acceptance and §12. The acceptance requires the polish spawn to be recorded as `--model sonnet` from `docs/project.toml`, and §5 says `/factory-deliver` passes `--hypothesis`. §12 says models and effort are unchanged and the only `~/.claude/skills` edit is the `FINDINGS` line.

**§6 — `worker_gate_red` is a new key.** Checked `src/cli/report.rs:80-82`, `:402-422`, `:463-464`, `:505`. `gate_red` is driver rows only. `worker_gate_red` is already an `EXTRA_KEYS` entry, already computed from `source == "worker"`, and already printed.

**§6 — the “failed-then-fixed gate” scenario plus “red CI then green” reports `fix_commits = 1`, `ci_red_rounds = 1`, `ci_reruns = 0`.** Checked `tests/checkpoints.rs:602-605` and `:696-697`, `tests/closeout.rs:723-726` and `:799-810`, `src/checkpoint.rs:763-768`, `src/closeout.rs:1718-1718`. No test scripts a gate as `red, green`. The red-gate scenario is `red, red, green`, which emits two `fix_committed` events. `red:test, green` emits one `ci_red`, one `ci_fix_committed`, and no `ci_rerun`. `fix_commits` is defined as `FixCommitted`, and the CI commit is `CiFixCommitted`. Neither scenario, and not both together, yields `fix_commits = 1` with those CI counts.

**§6 / §13 — the polish return block is `assets/contract-polish.md:29-35`, and the worker leaves the frontmatter alone at `:21`.** Checked `assets/contract-polish.md`. The file ends at line 26. The return block is lines 20–26. Line 21 is `STATUS: done | blocked`. The frontmatter instruction is line 13. `src/pack.rs:394-402` does tell the worker the driver fills the twelve keys.

**§6 / §13 — `message.model`, `model_usage`, `permission_denials`, `api_error_status`, and rate-limit info are parsed and dropped (`src/stream.rs:184-210`).** Checked `src/stream.rs:360-373`, `src/quota.rs:56-78`, `src/cli/status.rs:97` and `:259-265`. `api_error_status` and `rate_limit` are stored on `Summary` and read by the quota pause and `factory status`. `model_usage` and `permission_denials` are copied onto `Summary` at `:360` and `:366`. `message.model` (`src/stream.rs:47`) is the one field the collector never reads.

**§7 — `PRD-001 §10` retires `measure.py` because FR-11 is over.** Checked `docs/features/001-driver/prd.md:141`, `docs/index.md:34`, `docs/features/001-driver/retro.md:214-223`. Retirement is conditional on `factory report` matching `measure.py` within 2%. The correction says the report missed the polish subagents. The index still lists FR-11 human QA as pending.

**§7 vs §11 — FR-25’s `## Numbers` section is PRD-002’s `## Numbers` table.** Checked `docs/features/002-docs-port/prd.md:132-136`. FR-19’s table is the twelve `RETRO_KEYS`. §7’s section is a per-role own/subagent/weighted/minutes/turns table, knobs, hypothesis, the quality block, pack composition, and a three-run comparison, between `<!-- factory:numbers -->` markers. §11 says those are the same section and that 002 must not write a second table.

**§7 — comparison stays inside the repo and never prints a cross-repo ratio; `factory retro 024-20261012-e894` shows a comparison row for `018-20261011-6490`.** Checked `docs/features/001-driver/retro.md:71` and `:165`, `docs/features/001-driver/ab/runs.md:12`. `018-20261011-6490` is icg-cloud. `024-20261012-e894` is arca-api. The same sentence’s “54% against 86%” is that cross-repo pair (`retro.md:220-221`: 54.2% and 86.4%).

**§7 — that command reproduces the corrected per-role run B table in `001-driver/ab/runs.md`.** Checked `docs/features/001-driver/ab/runs.md:223-239`. The table there is the uncorrected report (run B 17,144,058, polish 1.95M, cockpit as its own row). The correction is one missed total and one corrected run total, not a per-role own/subagent table.

**§7 — the driver writes the section at ship, and running `factory retro` again leaves `retro.md` byte-identical.** Checked `src/closeout.rs:575-590` and `src/scheduler.rs:1688-1701`. `fill_retro` runs in `ship`, before CI and before `RunFinished`. The section compares “the same repo’s last three finished runs.” At ship this run is not finished. After `run_finished` it is. A later `factory retro` is comparing a different cohort, so the file written at ship does not stay byte-identical.

**§7 — per-role weighted figures in the section equal `report --json`.** Checked `src/cli/report.rs:39-40`, `:192-216`, `:530-537`. `--json` serializes `Report`. Per-worker rows are attached only with `--per-worker`, and `PerWorker` has lookups, `ctx_share`, and `repeat_rate`, no token fields. §3 puts `weighted_own` / `weighted_sub` on `--per-worker`, and no section adds them to the JSON object.

**§9 — the two-run replay is a manual step in `qa-guide.md`.** Checked `docs/features/001-driver/qa-guide.md` (the only `qa-guide.md` in the repo) and `docs/features/003-observatory/`. Neither file contains a recount or an observatory replay step.

**§9 — the stub writes `subagents/*.jsonl` under a `FACTORY_CONFIG_DIR`-controlled `CLAUDE_CONFIG_DIR`.** Checked `tests/harness/mod.rs:449-456`. The fixture env sets `FACTORY_CONFIG_DIR` to an empty config dir and does not set `CLAUDE_CONFIG_DIR`.

**§11 — `index.md` build order says 003 ships first, so 002’s `retros` table moves to schema v3.** Checked `docs/index.md:35`, `:38`, `:40`. Build order is still the placeholder. The 002 row still says the `retros` table is schema v2. Next free FR is FR-22, while this PRD’s frontmatter owns FR-21 through FR-25.

**§13 — CI produces `CiFixSpawned` at `src/closeout.rs:1534`.** Checked `src/closeout.rs:1530-1534` and `src/worker.rs:444-449`. Line 1534 matches events already stored, to count sessions. The emit is `worker.rs` when the CI-fix job is recorded.

**§13 — hook denials are not recorded, and a denied call is still a `tool_calls` row, at `src/cli/hook.rs:874-877`.** Checked `src/cli/hook.rs:874-877`. Those lines write the hook’s stdout verdict and return `Ok(0)`. They say nothing about the events table or `tool_calls`.

**§13 — the retro stand-in is `src/closeout.rs:2134-2150`.** Checked `src/closeout.rs:2132-2150` and `:365-372`. `:2134` is `retro_template()`. The stand-in write and `stood_in` call are `:365-372`.

**§2 — token cost is zero.** The next sentence in that bullet says the `FINDINGS` line is a few output tokens per polish.

**§1 / §6 — nothing reads `LogSummary`.** Checked `src/docs.rs:270-276` and `tests/docs.rs:34`. No driver path reads `log.decisions`, `decide`, or `blocks`. `tests/docs.rs:34` asserts `s.log.decisions`.

## Acceptance criteria and where they would move

| Criterion | Surface |
|---|---|
| Recount then `weighted_tokens` within 1% of 19,439,039 and of 8,750,823 | `factory report` stdout, key `weighted_tokens` (`src/cli/report.rs:496`). The check cannot pass against those two numbers; see the first finding. |
| “polish with subagents”: four `sub_*` columns, `subagents = 2`, `in_subagent = 1`, main-only `peak_context` | `workers` row and `tool_calls` rows (`src/db.rs:507`, `:630`). `in_subagent` is not a report column. |
| Build worker with no `subagents/` dir has zeros | That worker’s `sub_*` columns. §13 says the missing-directory predicate is unverified, so a real build worker may not be this case. |
| `pack_parts` has one entry per section the pack kind writes, summing to file size minus the header | `workers.pack_parts` and the file under `runs/<id>/packs/`. The equality does not match `src/pack.rs:145` and `:385-388`. |
| `report` prints per-role section means; the stub’s three roles differ | `factory report` stdout. Build, checkpoint, and polish packs differ (`src/pack.rs:146-177`, `:315-332`, `:390-412`). |
| Polish spawn is `--model sonnet`, every other call `opus` | Stub call JSON field `model` (`tests/stubs/claude:109`). |
| `knobs` has `polish.source = project`, `build.source = default`, `lanes`, both versions; `report` prints the knobs block | `runs` row and `factory report` stdout. |
| `--hypothesis` is in the row and in the retro | `runs.hypothesis` and the `## Numbers` section of `retro.md`. |
| `FINDINGS` fills five columns and `report` prints them | `runs.findings_*` and the report quality block. |
| No `FINDINGS` line: nulls, one event, `findings: —` | Those columns, the `events` row, and report stdout. |
| `fix_commits = 1`, `ci_red_rounds = 1`, `ci_reruns = 0` | Report quality block. The named scenarios do not produce that tuple. |
| `decisions` equals what `docs status --json` prints | Report `decisions` against `log.decisions` (`src/docs.rs:272`). |
| After ship, the section’s per-role weighted figures equal `report --json` | `retro.md` and `factory report --json`. No JSON field is specified for per-role weighted tokens. |
| A second `factory retro` leaves the file byte-identical | Bytes of `retro.md`. The ship-time write and a post-`run_finished` rewrite compare different finished-run sets. |
| `factory retro 024-20261012-e894` reproduces the corrected per-role table and a comparison row for `018-20261011-6490` normalised per slice | The generated section. There is no corrected per-role table in `runs.md`. There is no formula and no expected quotient for “normalised per slice” or “per changed line” in the PRD or in the A/B docs, so that figure has no oracle. Showing `018` on the `024` retro is the cross-repo row §7 forbids. |
| §9 “does the section read well beside the polish prose” | A person reading `retro.md`. |

(replayed for pass 2)
