# Slice 03 — Gate memory cap and orphan gate reaping

**Source**: [`prd.md`](prd.md) → §3 FR-1, §4 FR-2, §12 QA plan. **Depends on**: 02 (edits `src/docs/project.rs`, `src/docs/check.rs`, `src/lanes.rs`, `src/run.rs` after slice 02).
**Created**: 2026-11-26

**Progress**: `src/memcap.rs` + `gate_memory_max` validation; wired into `gate_once`, `setup_capped`, preflight; `ActiveGate` + `reap_active_gate` on resume/stop; `FACTORY_GATE_MEMCAP=off` in harness. Inner loop: 2 runs, 1 red (~clippy len_zero), ~3 min green. Deliberate reds: `gate_and_setup_wrap` without `oom_cap` → BLOCK title lost cap text; `killed_driver_reaps` with noop reap → sleep child survived. cp-2: FR-1 `cp-2/FR-1/transcript.txt`; FR-2 `cp-2/FR-2/transcript.txt`.

**Slice**: Adds `src/memcap.rs` (`systemd-run --user --scope` wrapping with strict limit normalization, `OOMPolicy=kill`, live availability + memory-controller delegation checks, and OOM detection), wires it into `checkpoint::gate_once`, `Lane::setup_capped` / `run_setup_capped`, and `preflight::run` (with `FACTORY_GATE_MEMCAP=off` in `tests/harness/mod.rs`), and records `ActiveGate` (pid/pgid/scope) in `RunState` with safe `process_group(0)` + signal forwarding and process-group/scope reaping on resume and dead-driver stop. Leaves `land_lane` unification and polish/CI-fix respawn lane retention to slice 04.

## Current code / environment

- **`src/docs/project.rs:10-35` (`KEYS`, `Project`)** and **`src/docs/check.rs:99-109, :228-279` (`read_project`, `kind_of`)**: `Project` needs `pub gate_memory_max: Option<String>`, `"gate_memory_max"` added to `KEYS`, and `src/docs/check.rs:99-109`'s default literal updated.
- **`src/checkpoint.rs:45` (`run`), `:125-167` (`RED_CAP` block title), `:537-542` (`GateResult`), `:657-715` (`gate_once`), `:1438` (`failure_line`)**: `gate_once` currently spawns a bare `Command::new("sh").arg("-c")` synchronously with `.status()`. `failure_line` scans the log for the first `^error` match (`src/excerpt.rs:25`), which would mask a memory-cap message appended at the end of cargo output — store `oom_cap: Option<String>` on `GateResult` and use `"gate hit its memory cap ({cap})"` directly for the `BLOCK-n` line/title when present!
- **`src/review.rs:496-550`**: already demonstrates the exact safe `CommandExt::process_group(0)` + async-signal-safe `SIGINT`/`SIGTERM`/`SIGHUP` `forward_signals` pattern (`libc` is already in `Cargo.toml:27`). Reuse that pattern for `gate_once` instead of unsafe `pre_exec`!
- **`src/lanes.rs:98-130` (`Lane::setup`, `Lane::run_setup`)**: called by `scheduler::prepare_lane` (`src/scheduler.rs:801`) and `cli::lanes::up_lanes` (`src/cli/lanes.rs`). Add `setup_capped(&self, dir: &Path, limit: Option<&str>)` and `run_setup_capped(&self, dir: &Path, limit: Option<&str>)` (with existing `setup` / `run_setup` delegating with `None` so external callers keep compiling) and wire `project::load(...).ok().flatten().and_then(|p| p.gate_memory_max)` into both callers.
- **`src/docs/preflight.rs:85-239` (`preflight::run`)** and **`tests/harness/mod.rs:609-634`**: `tests/run.rs:347` and `tests/loop.rs:565` assert exact 4 `judge_noted` events on headless CI (where `systemd-run --user` has no bus). Adding `("FACTORY_GATE_MEMCAP".into(), "off".into())` to `tests/harness/mod.rs` (purely additive!) and having `preflight::run` call `memcap::resolve_limit(...)` keeps all existing tests green on every runner.
- **`src/run.rs:95-138` (`RunState`), `src/scheduler.rs` (`drive`), `src/cli/stop.rs:36-45`**: add `ActiveGate` and reap leftover gate process groups / scopes on resume and dead-driver stop.
- **Leaks trap**: `src/memcap.rs` is a NEW file NOT in `docs/project.toml` `[leaks] allow`. Keep it 100% free of plan-id strings (`PRD-`, `FR-`, `BLOCK-`, `slice-`)!

## Tasks

### 1. Implement `src/memcap.rs` and `[docs] gate_memory_max` config + validation — high — blocked by: —
**Source**: §3 FR-1: "Add `"gate_memory_max"` to `project::KEYS` and `pub gate_memory_max: Option<String>` to `Project` ... `pub fn normalize_limit(raw: &str) -> Result<Option<String>, String>` ... `pub fn parse_memtotal_kb(meminfo: &str) -> Option<u64>` ... `pub fn resolve_limit(configured: Option<&str>) -> Option<String>` ... `pub fn systemd_run_available() -> bool` (live probe + Linux `memory` cgroup controller check, no daemon-wide cache) ... `pub fn shell_command(cmd: &str, limit: Option<&str>, unit: Option<&str>) -> (Command, Option<String>)` with `-p MemoryMax=<cap> -p MemorySwapMax=0 -p OOMPolicy=kill` ... `pub fn detect_oom`".

- `src/lib.rs`: export `pub mod memcap;`.
- `src/memcap.rs`:
  - `pub fn normalize_limit(raw: &str) -> std::result::Result<Option<String>, String>`:
    - Trimmed `"off"` (case-insensitive) → `Ok(None)`.
    - `<N>%` (`1..=100`) → `Ok(Some(format!("{n}%")))`.
    - `<N><unit>` (`n >= 1`, unit case-insensitive `K|M|G|T` with optional `iB` or `B`, e.g. `12g`, `12GB`, `12GiB`, `512M`) → `Ok(Some(format!("{n}{U}")))` (strict uppercase systemd suffix).
    - Else `Err(...)`.
  - `pub fn parse_memtotal_kb(meminfo: &str) -> Option<u64>` and `pub fn default_memory_max() -> String` (50% of `MemTotal` in GiB, floored `>= 2G`, `"8G"` fallback if `/proc/meminfo` absent/unreadable).
  - `pub fn resolve_limit(configured: Option<&str>) -> Option<String>`: honors `FACTORY_GATE_MEMCAP=off` → `None`; normalizes `configured` (falling back to `Some(default_memory_max())` on invalid string); defaults to `Some(default_memory_max())`.
  - `pub fn systemd_run_available() -> bool`: uses `FACTORY_SYSTEMD_RUN` env var if non-empty, else `"systemd-run"`; on Linux when `FACTORY_SYSTEMD_RUN` is unset, verifies `"memory"` appears in the user's `cgroup.controllers` under `/sys/fs/cgroup`; probes `<bin> --user --scope --quiet -p MemoryMax=1G -- true` live (exit `0`).
  - `pub fn shell_command(cmd: &str, limit: Option<&str>, unit: Option<&str>) -> (Command, Option<String>)`: wraps with `<bin> --user --scope --quiet [--unit=<unit>] -p MemoryMax=<cap> -p MemorySwapMax=0 -p OOMPolicy=kill -- sh -c <cmd>` when `limit.is_some() && systemd_run_available()`, else `sh -c <cmd>`.
  - `pub fn detect_oom(status: &std::process::ExitStatus, unit: Option<&str>, log_tail: &str) -> bool`: true on Unix signal 9 / exit 137, `"oom-kill"` or `"MemoryMax"` in `log_tail`, or (when `unit` is `Some(u)` and `FACTORY_SYSTEMD_RUN` is unset) `systemctl --user show -p Result --value <u>` outputting `"oom-kill"`.
  - Unit tests in `src/memcap.rs` covering `parse_memtotal_kb`, `normalize_limit`, and `resolve_limit`.
- `src/docs/project.rs` & `src/docs/check.rs`: add `gate_memory_max` to `KEYS`, `Project`, `load`, default literal at `src/docs/check.rs:99-109`, and `read_project` / `kind_of` validation via `memcap::normalize_limit`.

**Trap**: Never cache `systemd_run_available()` across the daemon's lifetime (a mid-run logout/bus loss must fall back to uncapped rather than making every gate red), and use `.ok().flatten()` on `project::load` in `gate_once` / `prepare_lane` so a `project.toml` typo never kills the driver.

**Acceptance:** Unit tests in `src/memcap.rs` and `src/docs/check.rs` prove normalization (`"12g"`/`"12GiB"` → `"12G"`, `"50%"`, `"off"`), `parse_memtotal_kb` clamping (`>= 2G`), and `E-PROJECT-INVALID` on invalid `gate_memory_max`.

### 2. Wire `memcap` into `gate_once`, `Lane::setup_capped`, `preflight`, and `tests/harness/mod.rs` — high — blocked by: 1
**Source**: §3 FR-1: "wire `memcap::shell_command` into `checkpoint::gate_once` and `Lane::setup_capped` ... store `oom_cap: Option<String>` on `GateResult` and surface `"gate hit its memory cap (<cap>)"` in the log and `BLOCK-n` title ... emit preflight `judge` note when `resolve_limit(...).is_some()` and `!systemd_run_available()` ... add `FACTORY_GATE_MEMCAP=off` to `tests/harness/mod.rs:609-634`".

- `tests/harness/mod.rs:609-634`: add `("FACTORY_GATE_MEMCAP".to_string(), "off".to_string())` (pure addition) so existing fixture tests (`tests/run.rs:347`, `tests/loop.rs:565`) keep exact `judge_noted` counts on headless CI.
- `src/docs/preflight.rs:85-239`: when `memcap::resolve_limit(project_gate_memory_max).is_some()` and `!memcap::systemd_run_available()`, push `"gate memory cap unavailable (no systemd-run --user); gates run uncapped"` to `pf.judge`.
- `src/lanes.rs:98-130`, `src/scheduler.rs:801`, `src/cli/lanes.rs`: add `setup_capped(&self, dir: &Path, limit: Option<&str>)` and `run_setup_capped(&self, dir: &Path, limit: Option<&str>)` using `memcap::shell_command(cmd, limit, None)` and wire `memcap::resolve_limit(project_mem.as_deref())` from `scheduler::prepare_lane` and `cli::lanes::up_lanes`.
- `src/checkpoint.rs:125-167, :537-542, :657-715`:
  - Add `oom_cap: Option<String>` on `GateResult`.
  - In `gate_once`, resolve `limit = memcap::resolve_limit(project::load(&run.state().repo).ok().flatten().and_then(|p| p.gate_memory_max).as_deref())`, build unique unit `format!("factory-gate-{}-{}-{}", run.id(), n, attempt)`, and call `memcap::shell_command(gate, limit.as_deref(), Some(&unit))`.
  - When `red`, if `let Some(cap) = &limit` and `memcap::detect_oom(&status, scope.as_deref(), &tail)`: append `\nfactory: gate hit its memory cap ({cap})\n` to `log` and set `oom_cap = Some(cap.clone())`.
  - At `reds >= RED_CAP` (`:125-137`), if `let Some(cap) = &result.oom_cap`, use `format!("gate hit its memory cap ({cap})")` directly as `line` (before falling back to `failure_line`) so cargo's earlier `^error` lines cannot hide it!
- `tests/checkpoints.rs` (purely additive new test): add `gate_and_setup_wrap_with_systemd_run_memcap_and_report_oom` proving: (1) with a stub `FACTORY_SYSTEMD_RUN` script (and `FACTORY_GATE_MEMCAP` removed), both `checkpoint::gate_once` (via a fixture run) and `Lane::setup_capped` pass `--user --scope --quiet --unit=... -p MemoryMax=... -p MemorySwapMax=0 -p OOMPolicy=kill -- sh -c ...`; (2) a gate printing `error: test failed` and exiting `137` records `"gate hit its memory cap (...)"` in the gate log AND in the `BLOCK-n` title; (3) with a failing `FACTORY_SYSTEMD_RUN` stub (and `FACTORY_GATE_MEMCAP` removed), `preflight::run` emits the uncapped `judge` note and gates fall back to `sh -c`.

**Trap**: `tests/harness/mod.rs` sets `FACTORY_GATE_MEMCAP=off` by default — in `gate_and_setup_wrap_with_systemd_run_memcap_and_report_oom`, explicitly override/remove `FACTORY_GATE_MEMCAP` (e.g. `cmd.env_remove("FACTORY_GATE_MEMCAP")`) and set `FACTORY_SYSTEMD_RUN` to the temp stub script!

**Acceptance:** `gate_and_setup_wrap_with_systemd_run_memcap_and_report_oom` passes alongside all existing `tests/run.rs` / `tests/loop.rs` tests; removing `OOMPolicy=kill`, `oom_cap` title override, or the preflight note breaks it.

### 3. Record `ActiveGate` in `RunState`, forward signals with safe `process_group(0)`, and reap on resume / stop — high — blocked by: 2
**Source**: §4 FR-2: "Define `ActiveGate { pid, pid_start, pgid, scope, checkpoint }` on `RunState` ... safe `CommandExt::process_group(0)` + `SIGINT`/`SIGTERM`/`SIGHUP` signal forwarder ... persist `active_gate` before waiting, clear on exit ... `pub fn reap_active_gate(run: &mut Run, out: &mut dyn Write) -> Result<bool>` reaping scope + full process group (`SIGTERM` -> 2s poll -> `SIGKILL`) on resume in `scheduler::drive`, `checkpoint::run`, and dead-driver `cli::stop::stop`".

- `src/run.rs:95-138`: add `ActiveGate` struct and `#[serde(default, skip_serializing_if = "Option::is_none")] pub active_gate: Option<ActiveGate>` on `RunState`, plus `Run::set_active_gate(&mut self, gate: Option<ActiveGate>) -> Result<()>`.
- `src/checkpoint.rs`:
  - Add an atomic `ACTIVE_GATE_PGID: AtomicI32` slot + `forward_gate_signals()` (mirroring `src/review.rs:496-529` — async-signal-safe `SIGINT`/`SIGTERM`/`SIGHUP` handler killing `-pgid` and re-raising `SIG_DFL`).
  - In `gate_once`: call `cmd.process_group(0)` (`#[cfg(unix)]`), register `forward_gate_signals()`, spawn with `.spawn()`, store `pid as i32` in `ACTIVE_GATE_PGID`, persist `run.set_active_gate(Some(ActiveGate { pid, pid_start: crate::run::start_time(pid), pgid: Some(pid), scope: scope.clone(), checkpoint: n }))?`, wait on `child.wait()`, and always clear both `ACTIVE_GATE_PGID` (to `0`) and `run.set_active_gate(None)?` on every exit path!
  - Implement `pub fn reap_active_gate(run: &mut Run, out: &mut dyn Write) -> Result<bool>`:
    - Takes `run.state().active_gate.clone()`; if `None`, returns `Ok(false)`.
    - If `g.scope` is `Some(unit)`, runs best-effort `systemctl --user stop <unit>` (ignoring error).
    - Checks whether leader is alive (`run::alive(g.pid, g.pid_start)`) OR its process group (`pgid = g.pgid.unwrap_or(g.pid)`) still has live members (`libc::kill(-(pgid as i32), 0) == 0` while `run::start_time(g.pid)` is `None` or `== g.pid_start`, so a recycled pid is never killed!).
    - If alive: sends `SIGTERM` to `-(pgid as i32)`, polls up to 2 s (50 ms steps) until `libc::kill(-(pgid as i32), 0) != 0`, then sends `SIGKILL` to `-(pgid as i32)`, logs `"reaped leftover gate pid {} (checkpoint {})"`, marks `reaped = true`.
    - Clears `run.set_active_gate(None)?` and returns `Ok(reaped)`.
- Call `checkpoint::reap_active_gate(run, out)?` at the start of `scheduler::drive`, at the start of `checkpoint::run` (`:45`), and in `cli::stop::stop` (`src/cli/stop.rs:36-45`) before `scheduler::mark_stopped`.
- `tests/checkpoints.rs` (purely additive new test): add `killed_driver_reaps_leftover_gate_process_on_resume`:
  - Uses a gate script that on attempt 1 writes a marker file + child pid file and spawns a background child (`sleep 60 & echo $! > child.pid; wait`), and on attempt 2 (when marker exists) exits `0` immediately (< 1 s).
  - Waits until `state.json` records `active_gate.pid` and `child.pid` exists, `SIGKILL`s the driver pid, verifies both gate & child are still alive before resume, resumes with `factory run 001`, and asserts: (1) both the old gate pid and its `sleep 60` child are dead (`!run::alive`), (2) `active_gate` is `None` in `state.json`, and (3) the resumed run completes green cleanly!

**Trap**: Use a two-attempt marker gate script so attempt 2 exits `0` immediately on resume (completing the test in ~2 s instead of sleeping 60 s!), and test that BOTH the gate leader AND its background child process die on reap!

**Acceptance:** `killed_driver_reaps_leftover_gate_process_on_resume` passes; removing `set_active_gate` or `reap_active_gate` leaves the `sleep 60` child alive and fails the test.

### 4. Verify — medium — blocked by: 1, 2, 3
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 5. Prove — at the checkpoint — blocked by: 4
**Source**: §12 QA plan rows FR-1 and FR-2:
- FR-1: `run gate_and_setup_wrap_with_systemd_run_memcap_and_report_oom` (evidence: test transcript)
- FR-2: `run killed_driver_reaps_leftover_gate_process_on_resume` (evidence: test transcript)

**Acceptance:** after the checkpoint, both tests run under `cargo test` with full gate green and transcripts recorded in Progress.

**Code-review fixes (2026-11-26).**
- `default_memory_max` divided by MiB but formatted as GiB, so caps were ~1024× too large; fixed the arithmetic and tightened the unit test.
- `systemctl` and stored scope names now use the `.scope` suffix `systemd-run` creates; probe matches production `MemorySwapMax`/`OOMPolicy`.
- Gate reap treats a live process group whose leader exited as alive when `pid_start` was recorded; OOM labeling requires an actual capped scope.
- Orphan gate killed when `set_active_gate` fails; `gate_memory_max` validation no longer emits a duplicate `E-PROJECT-INVALID`.
