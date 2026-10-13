# Slice 07 — Quota: rate-limit pause and weighted budget (FR-9)

**Source**: [`prd.md`](prd.md) → §11 FR-9 (all bullets and acceptance); §4 FR-2 bullet 5 (the `rate_limit_event` and the `result` error fields are in the record). **Depends on**: 06 (pause must hold across every spawn site: slice workers, fix workers and checkpoint workers; the last two exist only after 06).
**Created**: 2026-10-09 · **Status**: pending

**Progress**: built — `src/quota.rs` (`WEIGHTS` const citing `~/.claude/skills/feature-docs/MAINTAINING.md:298`, `RETRY_SECS = 900`, `Limit::from`, `weigh` / `weighted` over the `workers` rows, `Hold` / `hold` / `can_spawn`, `pause` / `resume` / `pause_tick`, `park_budget`), `stream::Summary.rate_limit` from the last `rate_limit_event`, events `run_paused {literal, resets_at}` / `run_resumed_from_pause` / `run_parked {reason, weighted, budget}`, `RunState.paused_until` and `run::WorkerOutcome` (`counts_as_death` exhaustive: `rate_limited` is no death at `scheduler::settle_dead` and the checkpoint-worker cap; `// quota: rate_limited is not a red` at the gate loop), the scheduler asks `quota::hold` before every spawn (phase `paused` while it holds; a limited build worker settles `rate_limited`, its slice goes back to `pending` with `lane` removed by `remove_frontmatter_keys`; over budget → `park_budget`, `waiting-owner`, and the driver exits), `checkpoint::await_quota` before fix and checkpoint workers, `fix_once -> bool` (a limited fix worker retries the same gate attempt with no gate run and no red between) and `Outcome::Budget`; `clock::unix_now` scaled by `FACTORY_CLOCK_SCALE` / `FACTORY_CLOCK_ANCHOR` plus `from_rfc3339`; stub outcomes `rate_limited` / `rate_limited_no_reset` for every role and a `rate_limit_event` template in `stream.json`; `tests/quota.rs` (five scenarios: pause until the reset with no spawn inside it, a no-reset limit retried after 15 minutes, two pauses on one slice under the cap, a limited fix worker is not a red, `--budget 1` parks with `weighted: 11609` and `--budget 100000000` resumes) and five unit tests in `quota` · inner loop: 7 runs, 4 red, ~2 min — red caught: a temporary borrowed past its statement in the no-cap test (compile), the closing checkpoint's worker is a stub call (`calls().len()` is builds + 1, two assertions), and two slice 02 tests in `tests/run.rs` whose budgets of 7 and 9 FR-9 now parks on (one stub worker weighs 11609) — expectation shift only: `run_on_the_clean_fixture_starts_notes_the_judge_lines_then_resumes` resumes with `--budget 90000000`, `budget_comes_from_the_flag_then_the_config_then_the_default` starts with `--budget 70000000`, the runs that spawn nothing keep 5 and 9; fmt reflowed 2 files, clippy caught nothing · deviations: AD-702 (`allowed_warning` is not a limit; the board's word wins over the stream), AD-703 (a budget park exits the driver so `factory run --budget` can resume), AD-704 (the scaled clock is an environment knob in the binary) — all in `agent-log.md`, merged at CP-4 · **CP-4 (2026-10-10)** drove the FR-9 row on two kept fixtures (`FACTORY_KEEP_FIXTURE=1`, the named test with `--exact --nocapture`; each file holds the test transcript, the whole `events.jsonl`, `state.json`, the `workers` rows, the stub call count and `driver.log`): [`evidence/cp-4-fr9-rate-limit-pause.txt`](evidence/cp-4-fr9-rate-limit-pause.txt) — `a_rate_limited_worker_pauses_the_run_until_the_reset` at clock ×20: exactly one `run_paused` (seq 7, `literal: rejected`, `resets_at` 1791631965 = 120 s after its own `ts` 1791631845), no event at all until `run_resumed_from_pause` (seq 8, `ts` = `resets_at` exactly) while the stub call count stayed at 1, then three `slice_spawned` (seq 9, 12, 15), workers `rate_limited, done, done, done` + checkpoint `done`, no `worker_dead`, `slice_blocked` or `run_parked`, slice 01 `done` with `lane` restored; and [`evidence/cp-4-fr9-budget-park.txt`](evidence/cp-4-fr9-budget-park.txt) — `a_spent_budget_parks_the_run_and_a_higher_budget_resumes_it`: `run_parked {reason: budget, weighted: 11609, budget: 1}` at seq 9 after exactly one `slice_spawned`, the one `workers` row giving 18 + 6514 + 0.1 × 41674 + 5 × 182 = 11609.4, the driver exited (AD-703), then `factory run 001 --budget 100000000` → `run_resumed` seq 10 on the same run dir, two more builds, a green checkpoint, `run_finished`.

Evidence for the FR-9 Prove row, from the two `tests/quota.rs` runs kept with `FACTORY_KEEP_FIXTURE=1` (pids dropped, session ids cut; the clock runs at ×20 in the first):

```jsonl
{"kind": "slice_spawned", "seq": 6, "session_id": "cd4954a7…", "slice": 1, "ts": "2026-10-10T10:58:44Z"}
{"kind": "run_paused", "literal": "rejected", "resets_at": 1791630044, "seq": 7, "ts": "2026-10-10T10:58:44Z"}
{"kind": "run_resumed_from_pause", "seq": 8, "ts": "2026-10-10T11:00:44Z"}
{"kind": "slice_spawned", "seq": 9, "session_id": "63237b06…", "slice": 1, "ts": "2026-10-10T11:00:44Z"}
{"kind": "slice_done", "seq": 10, "slice": 1, "ts": "2026-10-10T11:00:44Z"}
{"kind": "slice_spawned", "seq": 12, "session_id": "7bbabfb3…", "slice": 2, "ts": "2026-10-10T11:00:44Z"}
{"kind": "slice_spawned", "seq": 15, "session_id": "7ad9fab5…", "slice": 3, "ts": "2026-10-10T11:00:44Z"}
{"kind": "run_finished", "seq": 23, "ts": "2026-10-10T11:01:04Z"}
```

```jsonl
{"kind": "slice_spawned", "seq": 6, "session_id": "eceeb63c…", "slice": 1, "ts": "2026-10-10T10:58:32Z"}
{"kind": "slice_done", "seq": 7, "slice": 1, "ts": "2026-10-10T10:58:32Z"}
{"budget": 1, "kind": "run_parked", "reason": "budget", "seq": 9, "ts": "2026-10-10T10:58:32Z", "weighted": 11609}
{"kind": "run_resumed", "seq": 10, "ts": "2026-10-10T10:58:32Z"}
{"kind": "slice_spawned", "seq": 11, "session_id": "cae74780…", "slice": 2, "ts": "2026-10-10T10:58:32Z"}
{"kind": "slice_spawned", "seq": 14, "session_id": "1085ff3d…", "slice": 3, "ts": "2026-10-10T10:58:33Z"}
{"kind": "run_finished", "seq": 22, "ts": "2026-10-10T10:58:33Z"}
```

**Code-review fixes (2026-10-10).**
- `quota::pause` kept the later deadline, so a 15-minute guess (a limit with no `resetsAt`) held spawning past an earlier reported reset (Xq1). A reported `resetsAt` now replaces a guess, earlier or later, a guess never pushes past a report, and one pause announces once.
- The budget was compared after rounding, so a spend of 1.4 passed a budget of 1 (Xq2). `quota::hold` now parks when the unrounded weighted sum is above the budget, still `>` as §11's "Past the budget" says (AD-20014), and slice 10's `status` line rounds up to match (AD-50004).
- The tick checked `quota::hold` before it settled orphans and then went on, so an orphan whose limit or tokens should have paused or parked the run was followed by a spawn in the same tick, and a settled checkpoint orphan ran the gate during a pause (Xc1, Xd6). The hold is now checked again after any orphan settles.

**Slice**: After this slice a run survives the five-hour window closing. The driver reads the `rate_limit_event` and the `result` error of every worker; a limit reached means pause: no new spawns, in-flight workers finish, one event, resume at the reset time the stream gave or on a 15-minute retry when it gave none. A pause never counts against any cap. The second half is spend: a per-run weighted token budget from `config.toml`, summed over every worker row, that parks the run when crossed and resumes with `--budget`.

## Current code / environment

- **The signal in the stream**: `recordings/claude-p-oneshot-2.1.280.jsonl` has one `rate_limit_event` with `rate_limit_info {status: "allowed", resetsAt: <epoch s>, rateLimitType: "five_hour", overageStatus: "rejected", overageDisabledReason: "org_level_disabled", isUsingOverage: false, unifiedWindows: {five_hour: {utilization: 0.21, resetsAt}}}`. §20: the value of `status` when the limit is reached is unverified (likely `"rejected"` or `"reached"`); treat anything other than `"allowed"` as a limit, and record the literal value in the event so the first real pause tells us. A worker refused at the API edge ends with a `result` whose `is_error` is true and `api_error_status` is 429 or whose `result` text names the usage limit; both count. `utilization` is reported as a status line (slice 10's `factory status`), not acted on.
- **The spawn sites** are three after slice 06: `scheduler.rs` (slice workers), `checkpoint.rs` (fix and checkpoint workers). Pause is a flag on the run (`state.json.paused_until`) checked by one `can_spawn()` that every site calls; nothing else gates spawns.
- **Caps that a pause must not touch**: dead-worker once (slice 03), fix workers three (slice 06), CI rounds three (slice 09). A worker killed by the limit has `outcome = rate_limited`, not `dead` or `red`, and the slice goes back to `pending` (frontmatter) with the lane freed, so the board sees it as never attempted.
- **Weights**: §11 "weighted (cache read 0.1, output 5, input and cache write 1; MAINTAINING.md:298)". Confirm the three numbers against `~/Dropbox/dev/claude-code-skills/MAINTAINING.md:298` at slice entry; the PRD quotes them, the file is the source. `weighted = input + cache_write + 0.1 × cache_read + 5 × output` summed over the `workers` rows of the run (thinking tokens are inside `output_tokens` in the recorded `usage`, not added again).
- **Budget source**: slice 02's `config.toml` (`budget_weighted_tokens`, default 30M) and `--budget` on `factory run`, stored in `state.json`. §11: "a run that crosses it parks (`waiting-owner`, reason `budget`) and resumes with `factory run NNN --budget <higher>`."
- **Testing time**: the real reset is hours away. Inject the clock: `FACTORY_CLOCK_SCALE` (a divisor applied to sleeps) or, cleaner, a `now()`/`sleep_until()` pair behind a trait with a test implementation that advances on demand. The stub needs two scenarios: `rate_limited` (emit `init`, a `rate_limit_event` with `status: "rejected"` and `resetsAt = now + 120`, a `result` with `is_error: true` naming the limit; apply nothing to the board) and `rate_limited_no_reset` (same without `resetsAt`).
- **Deliberate non-changes**: *(prd §11 "Rejected: switching to a cheaper model when the window closes. Mixed-model slices confound the A/B.")* *(prd §11: "parks … resumes with `--budget`"; the driver never raises its own budget.)* *(prd §19 "Not handled: per-slice budgets".)*

## Tasks

### 1. Detect the limit and pause — high — blocked by: —
**Source**: §11 FR-9: "The driver reads the `rate_limit_event` of each worker's stream and the `result` error. A limit reached means pause: no new spawns, in-flight workers finish, one event; resume at `resetsAt` when the stream gives it, else retry every 15 minutes."

`src/quota.rs`: `Limit::from(summary) -> Option<Limit{until: Option<Instant>, literal: String}>` from the last `rate_limit_event` with `status != "allowed"` or a `result` with `api_error_status == 429` or `is_error && result ~ /usage limit|rate limit/i`. On a limit: `state.json.paused_until = until`, phase `paused`, one `run_paused {literal, resets_at}` event; the limited worker's row gets `outcome = rate_limited`, its slice back to `pending` with `lane` cleared (frontmatter) so the board forgets the attempt. `can_spawn()` returns false until `now >= paused_until`; with no `resetsAt`, retry every 15 min: `paused_until = now + 15 min`, and each retry that is limited again emits `run_paused` again (one event per pause, as §11 says, not one per tick). On resume: `run_resumed_from_pause`, phase back to `building`.

**Acceptance:** test clock; plan `rate_limited, done, done, done`: after the first worker, `events.jsonl` has exactly one `run_paused` with `resets_at` 120 s ahead and `literal: "rejected"`, no stub call for 119 s of test time, three `slice_spawned` after the clock passes `resetsAt`, all three slices `done`, `workers` has one `rate_limited` and three `done` rows, and no `worker_dead` event. Plan `rate_limited_no_reset, done, …` → pause of 15 min then success.

### 2. A pause counts toward no cap — high — blocked by: 1
**Source**: §11 FR-9: "A pause does not count toward any cap."

Audit the three cap sites: dead-worker (slice 03 task 6), fix attempts (slice 06 task 3), CI rounds (slice 09, when it lands: leave a `// quota: rate_limited is not a red` note in the cap code there). Each counter increments only on `outcome ∈ {dead, red}`; `rate_limited` is excluded by an exhaustive `match`, so a new outcome variant forces the decision.

**Acceptance:** plan `rate_limited, rate_limited, done` on slice 01 → slice 01 is `done`, never `blocked`, and the dead-worker counter for slice 01 is 0; scripted gate `red` with plan `done, done, rate_limited, fix, checkpoint` → the rate-limited fix attempt does not count: the gate reruns after the real fix and `fix_attempts` in the `checkpoint_blocked` path never reaches 2.

### 3. Weighted budget per run — high — blocked by: —
**Source**: §11 FR-9: "`budget_weighted_tokens` in `~/.config/factory/config.toml`, overridable with `--budget`. Default 30M weighted (cache read 0.1, output 5, input and cache write 1; MAINTAINING.md:298). Summed over every worker row of the run; a run that crosses it parks (`waiting-owner`, reason `budget`) and resumes with `factory run NNN --budget <higher>`." Acceptance: "a budget set below one worker's spend parks the run after that worker with reason `budget`."

`quota.rs`: `weighted(db, run) -> u64` as the formula above over `workers` rows of the run; checked by `can_spawn()` after every worker completes. Over budget → phase `waiting-owner`, event `run_parked {reason: "budget", weighted, budget}`, no new spawns, in-flight finish. `factory run NNN --budget <n>` on that run updates `state.json` and resumes. The weights live in one `const` with the `MAINTAINING.md:298` citation and the slice-entry confirmation date.

**Acceptance:** `--budget 1` with plan `done, done, done`: one worker runs, `run_parked` with `reason: budget` follows it, no second spawn; `factory run 001 --budget 100000000` resumes and finishes; the `weighted` value in the event equals the hand-computed sum from the recorded `usage` (input 1, cache write 1, cache read 0.1, output 5).

### 4. Verify — medium — blocked by: 1–3
`cargo test`. The full gate runs at this slice's checkpoint.

**Acceptance:** inner loop green, one commit.

### 5. Prove — at the checkpoint — blocked by: 4
Rows from [`prd.md`](prd.md) § QA plan: **FR-9** ("stub emits a rate-limit event → one pause event, no spawn until the reset time, then resume; a budget below one worker's spend parks with reason `budget`", evidence: `events.jsonl` excerpt). At the checkpoint: the task 1 and task 3 runs on the fixture; attach the two `events.jsonl` excerpts to this slice's **Progress**.

**Acceptance:** the excerpts show one `run_paused` with the reset time and a `run_parked{reason: budget}` after exactly one worker.
