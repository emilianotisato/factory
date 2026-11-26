---
number: 020
slug: runs-that-dont-hurt
title: "Runs that don't hurt the machine or lose work"
type: feature
created: 2026-11-05
author: Emiliano Tisato
authorized_tests: ["tests/commits.rs::a_dirty_path_in_the_main_tree_parks_the_landing_and_keeps_the_owners_edit", "tests/commits.rs::a_park_with_no_question_carries_its_reason", "tests/lanes.rs::two_lanes_build_two_slices_at_once_and_the_third_waits_for_its_dependency"]
summary: |
  Keeps unattended runs from taking down the host or losing worker edits: runs gates and lane setups under a `systemd-run --user --scope` memory cap (`MemoryMax` defaulting to 50% RAM, `OOMPolicy=kill`, overridable via `[docs] gate_memory_max`), records active gate pid/pgid/scope in `RunState` with safe `process_group(0)` + signal forwarding and reaps orphans on resume or dead-driver stop, adds `factory lanes clean [--dry]` plus optional `[lanes] clean_above_gb` run-start trimming, keeps polish and CI-fix lanes on respawn, stages seed copies atomically outside the lane worktree before renaming, refuses `lanes up`/`--prove` during an open run (resetting lanes before setup in `--prove`), resolves one `lanes::main_tree` across run/lanes/status, and unifies the three landing callers into one `land::land_lane` pipeline with conflict/kill driver tests and deterministic `wait_sh` barriers for multi-lane ordering tests. Verified by a cross-vendor adversarial pass (`claude:opus`, 541 s) 2026-11-26 with all 18 findings corrected inline.
---
> **History.** The code, `docs/reference` and `docs/decisions.md` win over this document.

# PRD-020 — Runs that don't hurt the machine or lose work

> **Interviewed 2026-11-26** (upgraded from the 2026-11-05 triage draft). Every `file:line` below was verified against `main` at `bb2707b`, then **re-verified by a cross-vendor adversarial pass (`claude:opus`, 541 s) on 2026-11-26 which caught six blockers and twelve real technical/citation breaks**. Each correction is inline and dated.

## 1. Why now

Four operational incidents and polish gaps have hurt live unattended runs:

- **Runaway gates kill the terminal and orphan processes on resume.** On 2026-11-01, a runaway mutation test inside `checkpoint::gate_once` (`src/checkpoint.rs:657`, a bare `sh -c`) grew to 80 GB RAM + 67 GB swap, causing `systemd-oomd` to kill the entire terminal scope (183 processes, including the cockpit session and driver). Lane setup (`src/lanes.rs:98-130`) also spawns a bare `sh -c`, and `gate_once` never records its child pid/pgid, so a killed driver leaves an orphaned gate running against the lane when the run resumes.
- **Lane build caches grow unbounded on disk.** On 2026-11-01, four lanes held 148 GB of `target/` plus 89 GB of stale `target/lane-*` directories in the main checkout. `src/cli/lanes.rs:12-26` has only `up` and `prune`, with no `clean` verb and no `[lanes]` size threshold.
- **Lanes lose edits or misbehave across worktrees and open runs.** `closeout::polish` (`src/closeout.rs:238`) and `closeout::fix_round` (`:2112`) always pass `fresh = true` to `scheduler::prepare_lane`, wiping a dead/rate-limited worker's edits on respawn (whereas build respawns pass `fresh = false`). `lanes::seed_one` (`src/lanes.rs:275-283`) copies directly to `to`, so a killed copy looks `kept` next time. `lanes up` and `lanes up --prove` (`src/cli/lanes.rs:47-151`) do not refuse during an open run, and `--prove` never resets lanes to `HEAD`. And `run::list_all` (`src/run.rs:583-588`) and `status::same_repo` (`src/cli/status.rs:178-180`) compare checkout paths instead of `lanes::main_tree`, so `factory status --here`, `factory lanes prune`, and `factory run` from a linked worktree miss runs on the main tree.
- **The landing pipeline is duplicated three times and multi-lane tests race on sleeps.** `scheduler::land_slice` (`src/scheduler.rs:974`), `checkpoint::land_worker` (`src/checkpoint.rs:877-966`), and `closeout::land_worker` (`src/closeout.rs:625`) each repeat commit-lane → undo-half-pick → `land::land` → already-landed recovery, lacking checkpoint/polish conflict and crash-recovery driver tests, while three multi-lane ordering tests in `tests/{commits,lanes}.rs` still rely on fixed `delay_ms` sleeps that raced on CI twice.

**What this leaves alone.** Narrow lane edge cases dropped at 2026-11-05 triage (unlisted lane re-add, `undo_half_pick` renames, hand-landed leftover refs, landing-wait phase label, pre-lanes workers, non-UTF-8 paths, low-confidence security notes), local CI with `act`, persisting CI rerun rows across restarts, capping workers' own inner loops or `lanes up --prove`, editing `skills/lanes/SKILL.md`, and converting single-worker kill/stop/status `delay_ms` tests (including `tests/merge.rs:139`).

## 2. Blast radius

- **`Project` struct and `docs/project.toml` validation (`src/docs/project.rs`, `src/docs/check.rs`).** Adding `pub gate_memory_max: Option<String>` to `Project` (`src/docs/project.rs:23-35`) and `"gate_memory_max"` to `KEYS` (`:10-20`) requires updating the `Project { ... }` default literal in `src/docs/check.rs:99-109` and adding validation in `read_project` / `kind_of` (`:228-279`), plus validating `[lanes] clean_above_gb` when present. `src/config.rs:431` already ignores `[docs]` and `[lanes]` without warnings.
- **Test harness isolation for `systemd-run --user` (`tests/harness/mod.rs:609-634`).** *Corrected 2026-11-26, adversarial pass:* `tests/run.rs::run_on_the_clean_fixture_starts_notes_the_judge_lines_then_resumes` (`:347-348`) and `tests/loop.rs::run_drives_three_slices_through_the_detached_driver` (`:565-566`) assert exact `judge_noted` event counts on headless CI (`ubuntu-latest` has no `systemd-run --user` bus). Adding `("FACTORY_GATE_MEMCAP".to_string(), "off".to_string())` to `tests/harness/mod.rs:609-634` (a pure addition, allowed by `testguard`) and having `preflight` consult `memcap::resolve_limit(...)` keeps all existing fixture tests green on every CI runner, while FR-1's dedicated tests explicitly override `FACTORY_GATE_MEMCAP` and `FACTORY_SYSTEMD_RUN`.
- **`RunState` (`state.json`).** Gains additive optional `#[serde(default, skip_serializing_if = "Option::is_none")] pub active_gate: Option<ActiveGate>` (`src/run.rs:95-138`) — fully backwards-compatible with existing `state.json` files.
- **CLI surface.** `factory lanes` gains `clean [--dry] [--days N]` (additive). `factory lanes up` and `factory lanes up --prove` now exit `1` when an open run holds the repo's lanes (consistent with `factory lanes prune`).
- **Three modified pre-existing test functions in `authorized_tests`.** *Corrected 2026-11-26, adversarial pass (dropped `tests/merge.rs:139` which is a single-worker keep-alive sleep, not lane ordering):* converting the three multi-lane ordering tests (`tests/commits.rs::a_dirty_path_in_the_main_tree_parks_the_landing_and_keeps_the_owners_edit`, `tests/commits.rs::a_park_with_no_question_carries_its_reason`, `tests/lanes.rs::two_lanes_build_two_slices_at_once_and_the_third_waits_for_its_dependency`) from `delay_ms` to `wait_sh` barriers is declared in this PRD's `authorized_tests` frontmatter.

## 3. FR-1 — Run gates and lane setups under a `systemd-run` memory cap

**Today.** `checkpoint::gate_once` (`src/checkpoint.rs:657`) and `Lane::setup` / `Lane::run_setup` (`src/lanes.rs:98-130`) spawn `Command::new("sh").arg("-c")...` directly inside the driver's / terminal's cgroup with no memory limit.

**Decided 2026-11-26.**

- **Configuration and strict systemd normalization (`src/docs/project.rs`, `src/docs/check.rs`, `src/memcap.rs`):**
  - Add `"gate_memory_max"` to `project::KEYS` (`src/docs/project.rs:10-20`) and `pub gate_memory_max: Option<String>` to `Project` (`:23-35`).
  - Add `pub fn normalize_limit(raw: &str) -> std::result::Result<Option<String>, String>` in `src/memcap.rs` (*corrected 2026-11-26, adversarial pass — systemd rejects lowercase `12g` or `GiB`/`GB`*):
    - `"off"` (case-insensitive, trimmed) → `Ok(None)`.
    - `<N>%` where `N` is an integer `1..=100` → `Ok(Some(format!("{n}%")))`.
    - `<N><unit>` where `N >= 1` and `<unit>` is `K|M|G|T` (case-insensitive, with optional `iB` or `B` suffix like `12GiB`, `12GB`, `512m`) → normalized strictly to systemd's uppercase form `Ok(Some(format!("{n}{U}")))` (e.g. `"12G"`, `"512M"`).
    - Anything else → `Err(...)`.
  - Validate `[docs] gate_memory_max` in `src/docs/check.rs` (`read_project`, `:228-279`) using `memcap::normalize_limit`: emit `E-PROJECT-INVALID` on non-string or `Err`.
- **Memory cap wrapper (`src/memcap.rs`, exported from `src/lib.rs`):**
  - `pub fn parse_memtotal_kb(meminfo: &str) -> Option<u64>` and `pub fn default_memory_max() -> String`: parses `MemTotal:` from `/proc/meminfo` (unit-tested via `parse_memtotal_kb` without relying on host RAM), computes 50% in GiB floored to `>= 2G`, formatted `"{gib}G"` (falls back to `"8G"` if `/proc/meminfo` is absent/unreadable).
  - `pub fn resolve_limit(configured: Option<&str>) -> Option<String>`:
    - If `std::env::var("FACTORY_GATE_MEMCAP").ok().as_deref() == Some("off")` → `None`.
    - If `configured` is `Some(raw)` → `normalize_limit(raw)` (`Ok(opt)` returns `opt`; `Err` safely falls back to `Some(default_memory_max())` so a typo never bricks a gate).
    - Otherwise → `Some(default_memory_max())`.
  - `pub fn systemd_run_available() -> bool` (*corrected 2026-11-26, adversarial pass — no daemon-wide stale cache and checks memory controller delegation*):
    - Resolves the binary from `FACTORY_SYSTEMD_RUN` env var if set (non-empty), else `"systemd-run"`.
    - When `FACTORY_SYSTEMD_RUN` is unset on Linux, also checks that `"memory"` appears in the current user cgroup's `cgroup.controllers` (e.g. walking up from `/proc/self/cgroup` under `/sys/fs/cgroup`), returning `false` if the memory controller is not delegated.
    - Probes `<bin> --user --scope --quiet -p MemoryMax=1G -- true` live (exit `0` = available, ~15 ms — NOT cached across the daemon's lifetime so a mid-run logout/bus loss degrades cleanly to uncapped instead of turning every gate red).
  - `pub fn shell_command(cmd: &str, limit: Option<&str>, unit: Option<&str>) -> (Command, Option<String>)`:
    - When `limit` is `Some(cap)` and `systemd_run_available()` is `true`: builds `<bin> --user --scope --quiet [--unit=<unit>] -p MemoryMax=<cap> -p MemorySwapMax=0 -p OOMPolicy=kill -- sh -c <cmd>` (*corrected 2026-11-26, adversarial pass — `-p OOMPolicy=kill` kills the whole scope immediately on OOM instead of letting `cargo` exit 101 or sending `SIGTERM`*) and returns `(command, unit.map(str::to_string))`.
    - Otherwise builds `sh -c <cmd>` and returns `(command, None)`.
  - `pub fn detect_oom(status: &std::process::ExitStatus, unit: Option<&str>, log_tail: &str) -> bool` (*corrected 2026-11-26, adversarial pass*): true if `status` was killed by `SIGKILL` (signal `9` or exit `137`), OR `log_tail` contains `"oom-kill"` / `"MemoryMax"`, OR when `unit` is `Some(u)` and `FACTORY_SYSTEMD_RUN` is unset, `systemctl --user show -p Result --value <u>` prints `"oom-kill"`.
- **Wire into gate, lane setup, and preflight:**
  - `checkpoint::gate_once` (`src/checkpoint.rs:657`): read `configured = project::load(&run.state().repo).ok().flatten().and_then(|p| p.gate_memory_max)` (*corrected 2026-11-26, adversarial pass — `.ok().flatten()` so a `project.toml` typo never kills the driver*), compute `limit = memcap::resolve_limit(configured.as_deref())`, build unique unit `format!("factory-gate-{}-{}-{}", run.id(), n, attempt)`, and wrap via `memcap::shell_command`. Record `oom_cap: Option<String>` on `GateResult` when `red && limit.is_some() && memcap::detect_oom(&status, scope.as_deref(), &tail)` (*corrected 2026-11-26, adversarial pass — pass `oom_cap` straight into the `BLOCK-n` title `"gate hit its memory cap ({cap})"` rather than relying on `failure_line` scanning past earlier `^error` lines in cargo output*), and also append `factory: gate hit its memory cap ({cap})\n` to the gate log so fix workers see it immediately.
  - `Lane::setup` / `Lane::run_setup` (`src/lanes.rs:98-130`): wrap setup `sh -c` with `memcap::shell_command` using `project::load(main).ok().flatten().and_then(|p| p.gate_memory_max)` (passing `limit` into `run_setup_capped` / `setup_capped` from `scheduler::prepare_lane` and `cli::lanes::up`).
  - `preflight::run` (`src/docs/preflight.rs:85-239`): when `memcap::resolve_limit(project.gate_memory_max.as_deref()).is_some()` and `!memcap::systemd_run_available()`, push one `judge` note: `"gate memory cap unavailable (no systemd-run --user); gates run uncapped"`. Add `("FACTORY_GATE_MEMCAP".into(), "off".into())` to `tests/harness/mod.rs:609-634`.

**Accepted cost.** On macOS, containers without `systemd-run --user`, or Linux hosts without `memory` cgroup delegation, gates run uncapped with one preflight `?` notice rather than adding a fragile `ulimit -v` wrapper. Re-probing `systemd-run --user` before each gate adds ~15 ms to a multi-minute gate in exchange for resilience against mid-run user-bus loss.

**Acceptance.** Unit and integration tests prove (`gate_and_setup_wrap_with_systemd_run_memcap_and_report_oom`): (1) `memcap::parse_memtotal_kb` + `normalize_limit` normalize `"12g"` / `"12GiB"` → `"12G"`, accept `"50%"` and `"off"`, and reject invalid strings (which `docs check` flags with `E-PROJECT-INVALID`); (2) with a stub `FACTORY_SYSTEMD_RUN` script (and `FACTORY_GATE_MEMCAP` unset), `checkpoint::gate_once` and `Lane::setup_capped` invoke `--user --scope --quiet --unit=... -p MemoryMax=... -p MemorySwapMax=0 -p OOMPolicy=kill -- sh -c ...`, and a simulated exit-137 gate populates `GateResult.oom_cap` so the log and `BLOCK-n` title report `"gate hit its memory cap (<cap>)"`; (3) with `FACTORY_SYSTEMD_RUN` pointing to a failing stub (and `FACTORY_GATE_MEMCAP` unset), `preflight::run` emits the uncapped `judge` note and gates fall back to `sh -c`.

## 4. FR-2 — Record active gate pid/pgid/scope in `RunState`, forward signals, and reap on resume or stop

**Today.** `checkpoint::gate_once` (`src/checkpoint.rs:657`) calls `.status()` synchronously without recording the gate process in `RunState`. If the driver is killed mid-gate, the gate (and its children) stay alive after resume, racing with the new gate in the same lane.

**Decided 2026-11-26.**

- **`ActiveGate` on `RunState` (`src/run.rs:95-138`):**
  - Define:
    ```rust
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    pub struct ActiveGate {
        pub pid: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub pid_start: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub pgid: Option<u32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub scope: Option<String>,
        pub checkpoint: u32,
    }
    ```
  - Add `#[serde(default, skip_serializing_if = "Option::is_none")] pub active_gate: Option<ActiveGate>` to `RunState` and `pub fn set_active_gate(&mut self, gate: Option<ActiveGate>) -> Result<()>` on `Run`.
- **Safe `process_group(0)` + signal forwarding (`src/checkpoint.rs:657`):** *Corrected 2026-11-26, adversarial pass (an earlier draft used unsafe `pre_exec` and broke `stop.rs:1`'s process-group kill contract):*
  - Configure the gate `Command` with safe `std::os::unix::process::CommandExt::process_group(0)` (same pattern as `src/review.rs:549`) and register a `SIGINT`/`SIGTERM`/`SIGHUP` signal forwarder (same async-signal-safe atomic-slot pattern as `src/review.rs:496-529`, killing `-pgid` and re-raising `SIG_DFL`) so killing the driver's process group ALSO kills the gate's process group cleanly instead of creating an orphan!
  - Spawn with `.spawn()`, store `pgid` in the atomic signal-forward slot AND immediately persist `run.set_active_gate(Some(ActiveGate { pid, pid_start: run::start_time(pid), pgid: Some(pid), scope, checkpoint: n }))?` BEFORE waiting on `child.wait()`. Always clear both the atomic slot and `run.set_active_gate(None)?` when `wait()` returns (including on error).
- **Reap process group (not just leader pid) on resume and dead-driver stop (`src/checkpoint.rs`, `src/scheduler.rs`, `src/cli/stop.rs`):** *Corrected 2026-11-26, adversarial pass:*
  - Add `pub fn reap_active_gate(run: &mut Run, out: &mut dyn Write) -> Result<bool>` in `src/checkpoint.rs`:
    - If `run.state().active_gate` is `None`, return `Ok(false)`.
    - Otherwise take `g`; check whether the leader (`run::alive(g.pid, g.pid_start)`) OR any process in its process group (`libc::kill(-(pgid as i32), 0) == 0` when `g.pgid` is `Some(pgid)` and `pgid > 0` and `(g.pid_start.is_none() || run::alive(g.pid, g.pid_start) || group_matches_same_run)`) is still alive: specifically, when `g.scope` is `Some(unit)`, run best-effort `systemctl --user stop <unit>` (which reaps the entire cgroup even if the leader exited); when `run::alive(g.pid, g.pid_start)` is true OR `g.pgid` still has live members while `g.pid` hasn't been recycled (`start_time(g.pid)` is `None` or `== g.pid_start`), send `SIGTERM` to `-(pgid as i32)`, poll up to 2 s in 50 ms steps until `libc::kill(-(pgid as i32), 0) != 0` (ESRCH), then send `SIGKILL` to `-(pgid as i32)` and log `"reaped leftover gate pid {} (checkpoint {})"`.
    - Clear `run.set_active_gate(None)?` and return `Ok(reaped)`.
  - Call `checkpoint::reap_active_gate(run, out)?` on resume at the start of `scheduler::drive`, at the start of `checkpoint::run` (`src/checkpoint.rs:45`), and in `cli::stop::stop` (`src/cli/stop.rs:36-45`) before `scheduler::mark_stopped` when the driver is dead.

**Accepted cost.** Waiting up to 2 seconds on `SIGTERM` before `SIGKILL` adds at most 2 s to a resume after a `SIGKILL` mid-gate, which is negligible compared to preventing concurrent gate runs in the same lane.

**Acceptance.** Integration test `killed_driver_reaps_leftover_gate_process_on_resume` in `tests/checkpoints.rs` runs a gate script that on attempt 1 creates a marker file and spawns a background child (`sleep 60`), verifies `state.json` records `active_gate.pid` while running, `SIGKILL`s the driver mid-gate, resumes the run (where attempt 2 sees the marker and exits `0` immediately — completing in < 5 s, *corrected 2026-11-26, adversarial pass*), and proves both the leftover gate pid and its child are dead, `active_gate` is cleared from `state.json`, and the resumed checkpoint completes green.

## 5. FR-3 — `factory lanes clean [--dry]` and `[lanes] clean_above_gb` run-start trimming

**Today.** `src/cli/lanes.rs:12-26` exposes only `up` and `prune`. Lane worktrees accumulate tens of GB of build artifacts (`target/`, caches) plus stale `target/lane-*` directories in the main checkout, with no built-in way to inspect or trim them.

**Decided 2026-11-26.**

- **Config knob (`src/docs/project.rs`, `src/docs/check.rs`):**
  - Add `pub fn clean_above_gb(root: &Path) -> Result<Option<f64>>` in `src/docs/project.rs` reading `[lanes] clean_above_gb` (positive integer or float in GiB).
  - Validate `[lanes] clean_above_gb` in `src/docs/check.rs` (`read_project`): if present and not a positive number (`> 0.0`), emit `E-PROJECT-INVALID`.
- **Build-output discovery & deduplication (`src/lanes.rs`, `src/cli/lanes.rs`):**
  - Add `LanesVerb::Clean { #[arg(long)] dry: bool, #[arg(long, default_value_t = 7)] days: u32 }` in `src/cli/lanes.rs:12-26`.
  - `--dry` is read-only and allowed anytime; non-dry `clean` refuses (exit `1`, `"clean refused: ..."`) if the repo has an open run (*clarified 2026-11-26, adversarial pass*).
  - For each lane on disk (`lanes::on_disk(data, main)`): discover existing build-cache directories:
    - `$CARGO_TARGET_DIR` declared in `lane-N.md`'s `env` (resolved relative to `lane_dir` when relative), plus `<lane_dir>/target`, plus `<lane_dir>/node_modules/.cache`, `<lane_dir>/.next/cache`, `<lane_dir>/vendor/.cache`.
    - *Corrected 2026-11-26, adversarial pass (prevents double-counting/double-deleting nested `target/lane-N` inside `<lane>/target`):* canonicalize existing paths, sort shortest-first, and drop any path that is identical to or a descendant (`starts_with`) of an earlier kept path for that lane!
  - Also discover stale `target/lane-*` directories directly inside `main.join("target")` (matching `lane-<digits>`), **excluding** any path that canonicalizes to a `CARGO_TARGET_DIR` declared by a current `docs/reference/lanes/lane-*.md` file (*corrected 2026-11-26, adversarial pass*).
  - Compute recursive byte sizes via a safe directory walker that never follows symlinks.
  - **`--dry`:** print one line per lane with its build-cache size, any stale `main/target/lane-*` dirs, and a total line — deleting nothing.
  - **Without `--dry`:**
    - Remove each stale `main/target/lane-*` directory.
    - For Cargo target dirs: check `FACTORY_CARGO_SWEEP` env var (if set and non-empty, use that executable path directly; if set to `"off"`, skip sweep and remove directly), otherwise look for `cargo-sweep` on `PATH` (*corrected 2026-11-26, adversarial pass — invoke `cargo-sweep sweep --time <days>` directly, NOT `cargo sweep`, so test harnesses without `~/.cargo/bin` work cleanly*). When found, run `<cargo-sweep> sweep --time <days>` inside the lane dir with `CARGO_TARGET_DIR=<dir>`; if absent or non-zero exit, fall back to `fs::remove_dir_all`. Remove non-Cargo cache dirs directly. Print freed bytes per lane and total freed.
- **Run-start threshold (`src/cli/lanes.rs::ensure_all`):**
  - After ensuring lanes in `ensure_all` (called on fresh `factory run` start before workers spawn): if `project::clean_above_gb(main)?` is `Some(limit_gib)` AND `run::list_open(data, main)?.is_empty()` (*corrected 2026-11-26, adversarial pass — never trim lanes while another open run holds them, and log a warning instead of failing `ensure_all` if a cache dir cannot be removed*), trim any lane whose deduplicated build-cache bytes exceed `(limit_gib * 1024.0 * 1024.0 * 1024.0) as u64` (trying `cargo-sweep`, then removing if still over threshold) and print `"  lane-{n}  trimmed build cache ({before} > {limit_gib:.1} GiB)"`.

**Accepted cost.** When `cargo-sweep` is absent, trimming wipes `target/` cleanly, paying one cold recompile for guaranteed disk recovery. Resumed runs (`cli/run.rs:84-102`) intentionally skip run-start trimming so warm caches of an in-progress run stay intact.

**Acceptance.** Integration test `lanes_clean_reports_trims_and_enforces_clean_above_gb_at_run_start` in `tests/lanes.rs` proves: (1) `factory lanes clean --dry` reports exact deduplicated lane `target/` (even with nested `CARGO_TARGET_DIR=target/lane-1`) and stale `main/target/lane-9` sizes without deleting anything (and preserves a live `main/target/lane-1` declared in `lane-1.md`); (2) `factory lanes clean` removes stale `main/target/lane-9` and invokes a stub `FACTORY_CARGO_SWEEP` (and falls back to directory removal when `FACTORY_CARGO_SWEEP=off`); (3) non-dry `factory lanes clean` refuses with exit `1` during an open run while `--dry` succeeds; (4) `[lanes] clean_above_gb` trims an oversized lane during `ensure_all` while leaving lanes under the threshold untouched.

## 6. FR-4 — Polish and CI-fix respawns keep their lane

**Today.** `closeout::polish` (`src/closeout.rs:238`) and `closeout::fix_round` (`src/closeout.rs:2112`) both pass literal `true` (`fresh = true`) to `scheduler::prepare_lane`, resetting the lane and wiping any uncommitted edits left by a dead or rate-limited worker before spawning attempt 2. Build respawns (`src/scheduler.rs:699`) already pass `fresh = false`.

**Decided 2026-11-26.**

- In `closeout::polish` (`src/closeout.rs:238`): pass `attempt == 1` instead of `true` to `scheduler::prepare_lane(run, lane, attempt == 1, out)?`.
- In `closeout::fix_round` (`src/closeout.rs:2112`): pass `attempt == 1` instead of `true` to `scheduler::prepare_lane(run, lane, attempt == 1, out)?`.
- `scheduler::open_lane` (`src/scheduler.rs:815`) still resets when the lane was freshly created or has no `lane_base` yet.

**Accepted cost.** If a polish or CI-fix worker dies after writing partial edits, its single respawn inherits those edits in the lane (matching build respawns — the worker sees `git status` and finishes or reverts them rather than redoing review fixes from scratch).

**Acceptance.** Integration test `polish_and_ci_fix_respawns_keep_their_lane_edits` in `tests/closeout.rs` proves: (1) when polish attempt 1 writes `kept-polish.txt` and exits dead, polish attempt 2 sees `kept-polish.txt` still present in its lane and lands cleanly; (2) when CI-fix round 1 attempt 1 writes `kept-cifix.txt` and exits dead, round 1 attempt 2 sees `kept-cifix.txt` still present in its lane, whereas round 2 attempt 1 resets to a fresh lane without stale untracked files.

## 7. FR-5 — Atomic seed copy staged outside the lane worktree

**Today.** `lanes::seed_one` (`src/lanes.rs:249-289`) copies directly into `to = lane.join(relative)`. If killed mid-copy (`SIGKILL`), a half-copied `to` remains on disk and the next run returns `Seeded::Kept` (`:269-270`) on a broken seed.

**Decided 2026-11-26.**

- *Corrected 2026-11-26, adversarial pass (an earlier draft placed `.factory-seed-<pid>.tmp` inside the lane worktree, where `land::commit_lane`'s `git add -A` could stage and commit a killed partial seed because the temp name isn't gitignored):*
  - In `lanes::seed_one` (`src/lanes.rs:249-289`): keep the `to.symlink_metadata().is_ok()` → `Seeded::Kept` fast path.
  - Stage the temp copy **outside** the lane worktree, in `lane.parent()` (`data/lanes/<key>/`, on the exact same filesystem so `fs::rename(&tmp, &to)` stays atomic while never sitting inside any git working tree): `tmp = lane_parent.join(format!(".seed-tmp-lane-{n}-{}.tmp", std::process::id()))` (or unique per-call counter).
  - Sweep any stale `.seed-tmp-lane-{n}-*.tmp` entries in `lane_parent` inside `lanes::ensure` before seeding, and `remove(&tmp)?` before copying.
  - Attempt `cp(CLONE, &from, &tmp)` → `fs::rename(&tmp, &to)?` → `Ok(Seeded::Clone)`; else `remove(&tmp)?`, `cp(COPY, &from, &tmp)` → `fs::rename(&tmp, &to)?` → `Ok(Seeded::Copy)`.
  - On any failure (copy or rename), `remove(&tmp)?` and return `Err`.

**Accepted cost.** Requires space for one copy of the seed on the lane filesystem during the copy (same as today), and any `SIGKILL` mid-copy leaves only a hidden `.seed-tmp-lane-*` outside the worktree that the next `lanes::ensure` sweeps cleanly and git can never stage.

**Acceptance.** Unit test `seed_copy_uses_atomic_temp_sibling_and_cleans_up_on_failure` in `src/lanes.rs` proves: (1) a successful seed produces `to` with identical contents and leaves zero `.seed-tmp-*` files in either the lane or its parent; (2) a failed copy cleans up its temp staging dir and leaves `to` absent so the next run does not return `Seeded::Kept`; (3) a pre-existing stale `.seed-tmp-lane-*` in `lane.parent()` is swept by `lanes::ensure` and never enters the lane worktree.

## 8. FR-6 — `lanes up` and `--prove` refuse during an open run, and `--prove` resets lanes before setup

**Today.** `cli::lanes::prune` (`src/cli/lanes.rs:155-169`) refuses when `run::list_open(data, main)` is non-empty, but `cli::lanes::up` (`:47-82`) and `cli::lanes::prove` (`:84-151`) run unconditionally, and `prove` runs `inner` on whatever stale commit each lane happened to sit on.

**Decided 2026-11-26.**

- Extract `fn refuse_if_open(data: &Path, main: &Path, verb: &str, out: &mut dyn Write) -> Result<bool>` in `src/cli/lanes.rs`: checks `run::list_open(data, main)?` (using FR-7's `main_tree` matching); if non-empty, prints `"  ✗ {verb} refused: this repo has an unfinished run, which holds its lanes"` plus each open run line and returns `Ok(true)`.
- Call `refuse_if_open` at the top of `up` (`"up"`), `prove` (`"prove"` — checked BEFORE `up` so `--prove` always prints `"prove refused"`), `prune` (`"prune"`), and non-dry `clean` (`"clean"`), returning `Ok(1)` when refused.
- *Corrected 2026-11-26, adversarial pass (an earlier draft ran `up` — including setup — BEFORE resetting lanes, so `lanes::reset`'s `clean -fd` wiped non-ignored files that setup wrote):* extract a shared `up_lanes(data, main, reset_to_head: bool, out)` helper used by both `up` (`reset_to_head = false`) and `prove` (`reset_to_head = true`): when `reset_to_head` is `true`, reset each lane to `main`'s `HEAD` **immediately after `lanes::ensure` and BEFORE running `lane.setup_capped`**! Then `prove` spawns the parallel `inner` runs.

**Accepted cost.** Running `factory lanes up` or `factory lanes up --prove` while a run is parked (`waiting-owner` / `paused`) is refused until the run finishes or is stopped — matching `factory lanes prune`.

**Acceptance.** Integration test `lanes_up_and_prove_refuse_open_run_and_prove_resets_to_head` in `tests/lanes.rs` proves: (1) `factory lanes up` prints `up refused: this repo has an unfinished run` and `factory lanes up --prove` prints `prove refused: this repo has an unfinished run` (both exit `1`, running no setup); (2) with no open run, `factory lanes up --prove` resets a lane sitting on an older commit with stray untracked files to `HEAD` BEFORE running lane setup (so a non-ignored file created by setup survives into `inner` while seeded ignored state is preserved).

## 9. FR-7 — One main tree across `run`, `lanes`, and `status --here`

**Today.** `run::list_all` (`src/run.rs:583-588`) compares `s.repo == repo` literally, `status::same_repo` (`src/cli/status.rs:178-180`) compares `repo.canonicalize() == top` literally, and `cli::run::run` (`src/cli/run.rs:88`) checks `branch_refusal(&repo, ...)` against the caller's cwd checkout rather than `run.state().repo`.

**Decided 2026-11-26.**

- Add `pub fn same_repo(a: &Path, b: &Path) -> bool` in `src/lanes.rs`:
  - Resolve `key(p) = main_tree(p).ok().or_else(|| p.canonicalize().ok())` on both `a` and `b`.
  - Return `matches!((key(a), key(b)), (Some(x), Some(y)) if x == y)`.
- Wire `lanes::same_repo` into:
  - `run::list_all` (`src/run.rs:583-588`), which covers `run::list_open`, `Run::find_live`, `Run::find_resumable`, `cli::lanes::{up,prove,prune,clean}`, and `cli::run`.
  - `cli::status::same_repo` (`src/cli/status.rs:178-180`), so `factory status --here` (plain, `--line`, and `--json`) from any linked worktree or lane finds live runs started in the main checkout (and vice versa).
- *Corrected 2026-11-26, adversarial pass:* in `src/cli/run.rs:84-93` (resume path after `Run::find_resumable`), check `commit::branch_refusal(&run.state().repo, &branch, default.as_deref())?` against `run.state().repo` (the checkout the run actually commits in!) rather than the caller's cwd `&repo`, and re-open `Docs::open(&run.state().repo)?` for the resumed run so resuming from a different worktree validates and drives the run's own tree! Keep existing per-feature `find_live`/`find_resumable` semantics (now main-tree-aware via `list_all`).

**Accepted cost.** Calls `git rev-parse --git-common-dir` when matching repos in `list_all` / `status --here`; run lists are small (< 20 entries) and non-git/deleted paths fall back cleanly to `canonicalize()`.

**Acceptance.** Integration test `linked_worktree_shares_main_tree_across_status_lanes_and_run` in `tests/lanes.rs` proves: on a repo with a linked `git worktree`: (1) when an open run exists in the main tree, `factory status --here` and `factory status --here --json` from the linked worktree report that run, and `factory lanes prune` / `factory lanes up` from the linked worktree refuse; (2) when a resumable run exists in the linked worktree on its feature branch while `main` is checked out in the main tree, `Run::find_resumable` from the main tree finds it, `factory status --here` shows it, and resuming checks `run.state().repo`'s branch (not main's).

## 10. FR-8 — Unified `land::land_lane` pipeline, conflict/kill driver tests, and multi-lane `wait_sh` barriers

**Today.** `scheduler::land_slice` (`src/scheduler.rs:974-1096`), `checkpoint::land_worker` (`src/checkpoint.rs:877-966`), and `closeout::land_worker` (`src/closeout.rs:625-720`) duplicate the same landing state machine. There are no driver tests for a checkpoint worker conflict, a polish worker conflict, or a crash recovery with a dirty lane before worker recording, and three multi-lane ordering tests in `tests/{commits,lanes}.rs` still rely on fixed `delay_ms` sleeps.

**Decided 2026-11-26.**

- **Shared `land::land_lane` pipeline (`src/land.rs`):**
  - Move `undo_half_pick` (currently in `src/scheduler.rs`) into `src/land.rs` (`pub(crate)` / `pub`).
  - Add `LandSpec`, `LandOutcome`, and `pub fn land_lane(spec: &LandSpec, before_commit: Option<&mut dyn FnMut(&Path) -> Result<Vec<String>>>) -> Result<LandOutcome>` in `src/land.rs` (*corrected 2026-11-26, adversarial pass — `before_commit` runs ONLY on the fresh-commit path when no held ref is reused and the commit has not already landed, returning any extra protected paths to drop so polish's `stand_ins` and `prd.md` `authorized_tests` guard never fire on an already-landed resume!*):
    - `LandSpec`: `repo: &Path`, `lane_dir: &Path`, `base: &str`, `board_files: &[String]`, `protected: &[String]`, `message: &str`, `subject: &str`, `ref_name: &str`, `reuse_held: bool`, `require_parent_eq_base: bool`.
    - `LandOutcome`: `AlreadyLanded { sha }`, `Nothing { had_held: Option<String> }`, `Landed { sha, dropped }`, `Dirty { paths, dropped }`, `Conflict { sha, paths, dropped }`.
    - Executes once: (1) check `base..HEAD` for `subject` (cleanup `ref_name` → `AlreadyLanded`), (2) check held `ref_name` (reuse when `reuse_held` and parent matches `base` if `require_parent_eq_base`; else run `before_commit(lane_dir)` if provided, append any returned extra protected paths, and call `commit_lane` → if `None` return `Nothing { had_held }`), (3) `undo_half_pick(repo, &sha)`, (4) `land(repo, &sha, board_files)` → map to `Landed` / `Dirty` / `Conflict`.
  - Refactor all three callers (`scheduler::land_slice`, `checkpoint::land_worker`, `closeout::land_worker`) to call `land::land_lane`:
    - *Corrected 2026-11-26, adversarial pass:* `scheduler::land_slice` keeps its missing-lane / missing-base error check (`:1025-1032`) before calling `land_lane`, while `checkpoint::land_worker` and `closeout::land_worker` keep their `lane.is_none() || base.is_none()` fallback to `commit_board` / `nothing` before calling `land_lane`. Every caller preserves its own conflict/drop policy (scheduler's `WorkerWroteProtectedFile`, empty-merge block, `start_merge`; polish's `stand_ins` + `prd.md` `authorized_tests` guard via `before_commit`; checkpoint/closeout conflict blocks and dirty waits).
- **Missing driver tests (`tests/checkpoints.rs`, `tests/closeout.rs`, `tests/commits.rs`):**
  - Add `unified_land_lane_handles_checkpoint_polish_conflicts_and_claim_kill_recovery` (or three focused tests plus one umbrella/named proof test matching §12) proving:
    1. **Checkpoint conflict:** main tree advances a conflicting file while a checkpoint worker edits the same lines in its lane → driver parks with `CheckpointBlocked` naming the conflicting path and leaves main tree clean.
    2. **Polish conflict:** main tree advances a conflicting file while the polish worker edits the same lines in its lane → driver parks with `CloseoutBlocked` naming the conflicting path and leaves main tree clean.
    3. **Crash recovery between lane reset/claim and worker spawn** (*corrected 2026-11-26, adversarial pass — `spawn_on` writes `lane_base` at `:818` before `state.toml` flips to `building` at `:701-709`, so the real crash window is after `state.toml` is `building` + `lane = 1` + `lane_base` is set, BEFORE the worker is recorded in `state.json`*): simulate that exact state (slice `building` on lane 1 with `lane_base` recorded, no worker in `state.json`, and an existing marker file in lane 1), resume the run, and verify the driver respawns cleanly on lane 1 without clobbering or duplicating the slice and lands it green.
- **Replace the 3 multi-lane ordering `delay_ms` sleeps with `wait_sh` barriers (`tests/commits.rs`, `tests/lanes.rs`):**
  - Convert the three multi-lane ordering tests listed in `authorized_tests` (`tests/commits.rs::a_dirty_path_in_the_main_tree_parks_the_landing_and_keeps_the_owners_edit`, `tests/commits.rs::a_park_with_no_question_carries_its_reason`, `tests/lanes.rs::two_lanes_build_two_slices_at_once_and_the_third_waits_for_its_dependency`) from fixed `delay_ms` sleeps to deterministic `wait_sh` file barriers. Leave single-worker kill/stop/status `delay_ms` tests (including `tests/merge.rs:139`) untouched.

**Accepted cost.** Single-worker kill/stop/status tests keep `delay_ms` because they only hold one stub worker alive while the test kills/stops it.

**Acceptance.** (1) All three landing callers route through `land::land_lane` (zero duplicated `commit_lane` → `undo_half_pick` → `land::land` loops in `scheduler.rs`, `checkpoint.rs`, `closeout.rs`); (2) `unified_land_lane_handles_checkpoint_polish_conflicts_and_claim_kill_recovery` proves checkpoint conflict, polish conflict, and claim-to-spawn crash recovery; (3) the three converted multi-lane ordering tests contain zero `delay_ms` and pass reliably.

## 11. Decisions log

No `OPEN-n`. Every fork was settled on 2026-11-26 and lives in its FR:

- Gate & lane-setup memory cap via `systemd-run --user --scope` (`MemoryMax` defaulting to 50% host RAM, `MemorySwapMax=0`, `OOMPolicy=kill`, normalized strictly for systemd, overridable via `[docs] gate_memory_max` including `"off"`), falling back to uncapped + one preflight `judge` note when unavailable or lacking memory delegation; test harness sets `FACTORY_GATE_MEMCAP=off` (§3).
- Active gate pid/pgid/scope persisted in `RunState.active_gate` using safe `process_group(0)` + `SIGINT`/`SIGTERM`/`SIGHUP` signal forwarding, reaping the full process group/scope on resume and dead-driver stop (§4).
- Both `factory lanes clean [--dry] [--days N]` (deduplicating nested `CARGO_TARGET_DIR`, excluding live `main/target/lane-*`, invoking `cargo-sweep sweep`) AND `[lanes] clean_above_gb` run-start threshold (guarded by no open runs) (§5).
- Polish and CI-fix respawns keep their lane (`fresh = attempt == 1`) (§6).
- Atomic seed copy staged outside the lane worktree (`data/lanes/<key>/.seed-tmp-lane-*`) before `fs::rename` (§7).
- `lanes up` and `--prove` refuse during an open run (`up refused` / `prove refused`), and `--prove` resets lanes to `HEAD` before running lane setup (§8).
- Shared `lanes::same_repo` comparing `lanes::main_tree` across `run`, `lanes`, and `status --here`, with resume checking `branch_refusal` on `run.state().repo` (§9).
- Shared `land::land_lane` pipeline with `before_commit` hook + checkpoint/polish conflict and crash-recovery tests + converting the 3 multi-lane ordering `delay_ms` tests to `wait_sh` barriers (§10).

## 12. QA plan

**Gate**: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test && claude plugin validate mods/cockpit && claude plugin test mods/cockpit` *(declared in `docs/project.toml`)*
**Surface**: `factory` CLI (`run`, `stop`, `status --here`, `lanes up`, `lanes up --prove`, `lanes prune`, `lanes clean`, `docs check`, `docs preflight`), checkpoint & closeout drivers, `land::land_lane`, and `state.json`. No GUI: every FR is proven below it.
**Harness**: `src/memcap.rs`, `src/lanes.rs`, `src/land.rs`, `tests/checkpoints.rs`, `tests/lanes.rs`, `tests/closeout.rs`, `tests/commits.rs`, `tests/cockpit.rs`.
**Seed state**: fixture repos with stub `FACTORY_SYSTEMD_RUN`, stub `FACTORY_CARGO_SWEEP`, two-attempt marker gate scripts, populated lane `target/` + stale `main/target/lane-*`, linked `git worktree`, and multi-lane `wait_sh` barrier fixtures.

| FR | Proof | Evidence | Human? |
|---|---|---|---|
| FR-1 | run `gate_and_setup_wrap_with_systemd_run_memcap_and_report_oom` | test transcript | no |
| FR-2 | run `killed_driver_reaps_leftover_gate_process_on_resume` | test transcript | no |
| FR-3 | run `lanes_clean_reports_trims_and_enforces_clean_above_gb_at_run_start` | test transcript | no |
| FR-4 | run `polish_and_ci_fix_respawns_keep_their_lane_edits` | test transcript | no |
| FR-5 | run `seed_copy_uses_atomic_temp_sibling_and_cleans_up_on_failure` | test transcript | no |
| FR-6 | run `lanes_up_and_prove_refuse_open_run_and_prove_resets_to_head` | test transcript | no |
| FR-7 | run `linked_worktree_shares_main_tree_across_status_lanes_and_run` | test transcript | no |
| FR-8 | run `unified_land_lane_handles_checkpoint_polish_conflicts_and_claim_kill_recovery` | test transcript | no |

No row needs a human, so no `qa-guide.md` is generated.

## 13. Slices

Four vertical slices on one feature branch (`feat/runs-that-dont-hurt`), one PR, one commit per slice, Prove rows driven at checkpoints (`checkpoint_every = 2`). No QA row needs a human, so no `qa-guide.md` is generated.

- **Slice 01 — Main-tree identity, atomic seeds and open-run lane guards** (`FR-5`, `FR-6`, `FR-7`, `deps: —`): stages seed copies atomically outside the lane worktree (`data/lanes/<key>/.seed-tmp-lane-*`), unifies repo matching on `lanes::same_repo` (`lanes::main_tree`) across `run`, `lanes`, and `status --here` (fixing worktree resume `branch_refusal`), and makes `lanes up` / `--prove` refuse open runs with `--prove` resetting lanes before setup.
- **Slice 02 — Lanes clean and run-start build-cache trimming** (`FR-3`, `deps: 01`): adds `[lanes] clean_above_gb` parsing/validation, `factory lanes clean [--dry] [--days N]` (deduplicating nested `CARGO_TARGET_DIR`, excluding live declared `main/target/lane-*`, invoking `cargo-sweep sweep`), and run-start trimming in `ensure_all`.
- **Slice 03 — Gate memory cap and orphan gate reaping** (`FR-1`, `FR-2`, `deps: 02`): adds `src/memcap.rs` (`systemd-run --user --scope` with normalized `MemoryMax`, `OOMPolicy=kill`, live probe + cgroup delegation check, `OOM` detection, `FACTORY_GATE_MEMCAP=off` in test harness), wires it into `gate_once`, `Lane::setup_capped`, and `preflight::run`, and records `ActiveGate` in `RunState` with safe `process_group(0)` + signal forwarding and process-group/scope reaping on resume and dead-driver stop.
- **Slice 04 — Unified `land_lane`, respawn lane retention and `wait_sh` barriers** (`FR-4`, `FR-8`, `deps: 03`): keeps polish and CI-fix lanes on respawn (`attempt == 1`), unifies the three landing callers into `land::land_lane` with a `before_commit` hook, adds checkpoint-conflict, polish-conflict, and claim-to-spawn crash-recovery tests, and converts the three authorized multi-lane ordering tests from `delay_ms` to `wait_sh` barriers.

Checkpoint 1 runs after slices 01 and 02 (proving FR-3, FR-5, FR-6, FR-7); the final checkpoint runs after slices 03 and 04 (proving FR-1, FR-2, FR-4, FR-8 and re-proving all 8 FRs with the full gate).

## 14. Non-goals

- **Narrow lane edge cases dropped at 2026-11-05 triage:** unlisted lane re-add, `undo_half_pick` renames, refs left after hand landing, landing-wait phase label, pre-lanes workers (`lane: None`), non-UTF-8 paths, and low-confidence security notes (`prd.md` triage 2026-11-05).
- **Local CI with `act` and persisting CI rerun rows across restarts.** Dropped at 2026-11-05 triage.
- **Capping workers' own inner loops or `lanes up --prove` (`src/cli/lanes.rs:107`).** Not handled (2026-11-26): workers run in their own sessions; `lanes up --prove` is interactive. Revisit if either hits OOM.
- **Updating `skills/lanes/SKILL.md:97` for the new `lanes up` refusal.** Not handled (2026-11-26): avoids another skill bundle lock churn right after PRD-019's `0.17.0` bump; fold into the next skill pass or `/harvest`.
- **Converting single-worker kill/stop/status `delay_ms` sleeps to `wait_sh` (including `tests/merge.rs:139`).** Cut 2026-11-26: only the three multi-lane ordering tests in `tests/{commits,lanes}.rs` raced on CI.
- **Memory caps on macOS / non-systemd Linux via `ulimit -v`.** Not handled (2026-11-26): `ulimit -v` breaks virtual-address reservations in Rust/Go/JVM toolchains; non-systemd hosts run uncapped with one preflight `judge` note.

## 15. Verification notes

Checked against `main` at `bb2707b`, 2026-11-26, then re-verified the same day by a cross-vendor adversarial pass (`factory review second --doc`, `claude:opus` at `high` effort, 541 s, log `~/.local/share/factory/reviews/20261126-165007`). All 18 findings (6 blockers + 12 technical/citation breaks) were verified and corrected inline above. **Re-verify at slice entry.**

- `src/checkpoint.rs:45` (`checkpoint::run`), `:657-670` (`gate_once` bare `sh -c`), `:877-966` (`land_worker`), `:1438` (`failure_line`).
- `src/review.rs:496-550` (`forward_signals`, safe `process_group(0)` pattern).
- `src/lanes.rs:98-130` (`Lane::setup`, `Lane::run_setup` bare `sh -c`), `:139-150` (`main_tree`), `:249-289` (`seed_one` direct copy to `to`), `:322-328` (`reset`), `:330-348` (`on_disk`).
- `src/cli/lanes.rs:12-26` (`LanesVerb`: `Up`, `Prune` only), `:47-82` (`up` no open-run check), `:84-151` (`prove` no open-run check, no lane reset before setup/inner), `:155-184` (`prune` open-run check), `:187-199` (`ensure_all`).
- `src/closeout.rs:238` (`polish` `prepare_lane(run, lane, true, out)`), `:625-720` (`land_worker` with `stand_ins` at `:668-670` and `prd.md` `authorized_tests` guard), `:2112` (`fix_round` `prepare_lane(run, lane, true, out)`).
- `src/scheduler.rs:699-709` (`spawn_on` calls `prepare_lane` — writing `lane_base` at `:818` — before marking `building` at `:701-709`), `:791-821` (`prepare_lane` calling `lane_def.setup` at `:801`, `open_lane`), `:974-1096` (`land_slice`).
- `src/land.rs:9-145` (`LaneCommit`, `Landing`, `commit_lane` with `git add -A` at `:47`, `land`).
- `src/run.rs:95-138` (`RunState`), `:575-588` (`list_open`, `list_all` compares `s.repo == repo`), `:620-635` (`alive`, `start_time`, `pid_alive`).
- `src/cli/run.rs:84-93` (resume checks `branch_refusal` on `&repo` instead of `run.state().repo`), `:136-139` (`ensure_all`).
- `src/cli/status.rs:123-126, :178-180` (`--here`, `same_repo` compares `canonicalize() == top`).
- `src/cli/stop.rs:1, :36-45` (process-group kill contract; dead-driver `mark_stopped`).
- `src/docs/project.rs:10-35, :101-121` (`KEYS`, `Project`, `seed`).
- `src/docs/check.rs:99-109, :212-279` (`Project` literal, `read_project`, `kind_of`).
- `src/docs/preflight.rs:85-239` (`preflight::run`), `:550-603` (`cli`).
- `tests/harness/mod.rs:587-634` (harness PATH excludes `~/.cargo/bin`; env map for fixture runs).
- `tests/run.rs:347-348`, `tests/loop.rs:565-566` (assert exact 4 `judge_noted` events).
- `tests/commits.rs:595, :645`, `tests/lanes.rs:185` (the 3 multi-lane ordering `delay_ms` tests).
