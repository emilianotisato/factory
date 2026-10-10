# Agent log — slice 07

## Traps

- **The closing checkpoint is a stub call (2026-10-10)**: with `checkpoint_every: 10` the fixture still runs one checkpoint after the last slice, and its worker is a recorded `calls/n.json` — a `calls().len()` expectation is builds + 1. Cost one red run in `tests/quota.rs`.

## Blockers

_(none)_

## Decisions

### AD-701 — What does `run_paused.resets_at` carry when the stream gave no `resetsAt`? · slice-07 · 2026-10-10

**Chose:** `resets_at` is always the epoch second spawning may resume (u64, the same clock as `paused_until`): the stream's `resetsAt` when it is still ahead, else now + 900. A second limited worker that was already in flight when the pause began extends `paused_until` silently; a worker limited after the pause lifted starts a new pause with its own `run_paused`.
**Over:** `resets_at: null` for the no-reset case, with the 15-minute retry implied. A consumer (slice 10's cockpit) would then have to know the retry constant to show a countdown.
**Because:** FR-9 names the payload `{literal, resets_at}` and says "no resetsAt → retry every 15 min" without saying what the event carries then; "one `run_paused` per pause" settles the in-flight case.
**Undo:** one line in `quota::pause` and the test's `(880..=900)` window.
**Review:** no

### AD-702 — Which worker streams count as a usage limit? · slice-07 · 2026-10-10

**Chose:** a `rate_limit_event` whose `status` does not start with `allowed` (so `allowed_warning` is not a limit), carrying its `resetsAt`; else a `result` with `is_error` and `api_error_status` 429, or `is_error` and text matching `usage limit|rate limit` (case-insensitive) — these two pause with no reset, so the 15-minute retry applies. The board's word wins where it has one: a slice `done` or `blocked`, a `CP-n`, a new block settle as such even if the stream also showed a limit; a fix worker has no board signal, so its stream decides before the dead/done split.
**Over:** the slice's literal "anything other than `allowed`" (would pause on every `allowed_warning`, which the recording shows at 21 % utilisation on a healthy session); and pausing on any limit regardless of outcome (would stall a run on a worker that nonetheless finished).
**Because:** FR-9 lists the three triggers but not the `allowed_warning` literal nor the finished-anyway case; §4 FR-2 bullet 5 only says the fields are recorded.
**Undo:** `quota::Limit::from` and the three call sites (`scheduler::route_exit`, `settle_orphans`, `settle_checkpoint_orphan`; `checkpoint::fix_once` and the checkpoint-worker loop).
**Review:** decide

### AD-703 — Does a budget park keep the driver up like a block park? · slice-07 · 2026-10-10

**Chose:** no — `quota::park_budget` emits `run_parked {reason: "budget", weighted, budget}`, sets `waiting-owner`, and the driver returns, so the process exits once nothing is in flight (inside a checkpoint the step reports `Outcome::Budget` up to the scheduler, which parks the same way; the checkpoint runs again on resume).
**Over:** staying up on the slow tick like a block park. `factory run NNN --budget <n>` is the resume FR-9 names, and `cli::run` refuses a live run ("stop it first") before it reads `--budget`; keeping the driver up would make the owner `factory stop` first, which FR-9 does not say.
**Because:** FR-9 says "parks" and "`factory run NNN --budget <n>` resumes" without saying how those meet a live driver.
**Undo:** replace the two `return quota::park_budget(..)` with `enter_park` + a budget re-check on the tick, and teach `cli::run` to apply `--budget` to a live run.
**Review:** fyi

### AD-704 — How do the tests wait out a 15-minute pause in seconds? · slice-07 · 2026-10-10

**Chose:** a scaled clock in `clock::unix_now()`: with `FACTORY_CLOCK_SCALE=S` (integer > 1) and `FACTORY_CLOCK_ANCHOR=<wall epoch>` it reports `anchor + (wall − anchor) × S`, read once per process. The driver, a resumed driver and the stub worker inherit the two variables, so `resetsAt`, `paused_until` and every event `ts` sit on one clock; the pause waits in 250 ms wall slices until `unix_now() >= paused_until`. Unset, the clock is the wall clock and the binary behaves as before.
**Over:** a clock trait threaded through `Run`/scheduler (touches every `now` call and the stub still needs the same scale by env); or shortening `RETRY_SECS` under `cfg(test)` (integration tests run the real binary, so `cfg(test)` does not reach it).
**Because:** FR-9's acceptance ("no spawn until the reset time", the 15-minute retry) needs real elapsed time the tests cannot afford; PRD §16's "wall-clock timing" fixture note settles nothing about pauses.
**Undo:** delete `clock::scale`/`wall_now`, make `unix_now` the wall clock, and give `tests/quota.rs` another way to pass time (slower tests or a trait).
**Review:** decide
