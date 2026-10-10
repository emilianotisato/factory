---
slice: 07
status: pending
depends_on: [6]
---

# Slice 07 — Quota: rate-limit pause and weighted budget (FR-9)

**Source**: [`prd.md`](prd.md) → §11 FR-9 (all bullets and acceptance); §4 FR-2 bullet 5 (the `rate_limit_event` and the `result` error fields are in the record). **Depends on**: 06 (pause must hold across every spawn site: slice workers, fix workers and checkpoint workers; the last two exist only after 06).
**Created**: 2026-10-09 · **Status**: pending

**Progress**: _(written as it lands)_

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
