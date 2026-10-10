## Traps

- `docs status --json` says `checkpointDue` whenever every slice is done and unverified (`allDone`), whatever `checkpoint_every` says: a fixture run ends at phase `checkpoint`, never `finished`, until slice 06's checkpoint exists — assert `checkpoint`.
- Numbering seeds are numeric, not zero-padded: slice 01 is `AD-101` and `BLOCK-102` (`Number("0101")` in the stub); `format!("{:02}01")` gives `AD-0101` and the pack test does not find `AD-101`.
- A recorded `claude -p` stream opens with `system/hook_started` lines; `init` is not line 1, match it with `any`, not `[0]`.
- `factory run` now detaches a driver that starts building at once: a test that reads `state.json` or runs a second `factory run` right after the first returns races it — pass `--foreground` with a stub plan, or poll the phase.

## Blockers

_(none)_

## Decisions

### AD-301 — Does the driver take `libc` as a direct dependency? · slice-03 · 2026-10-10

**Chose:** `libc = "0.2"` in `[dependencies]`, for `setsid()` in the daemon's `pre_exec` and `kill(-pgid, SIGKILL)` in the resume test.
**Over:** `nix` (a larger surface for two calls) or shelling out to `setsid(1)`/`kill(1)` (another process on every spawn, and `setsid(1)` is util-linux, not POSIX).
**Because:** §3 FR-1 wants a detached process and §19 names Linux as the target; the PRD does not list the crate. It is already a transitive dependency of `rusqlite` and `tempfile`, so the lockfile gains nothing new.
**Undo:** one `use`, one `pre_exec` closure, one line of the test.
**Review:** decide

### AD-302 — What does the loop do when every slice is done but none is verified? · slice-03 · 2026-10-10

**Chose:** follow the tool: `checkpoint_due` on the board → emit `checkpoint_due{unverified}`, set phase `checkpoint`, return. `finished` is reached only when nothing is ready, nothing is building and no checkpoint is due. On the three-slice fixture with `checkpoint_every: 10` the run therefore ends at `checkpoint`, not at the `finished` task 5's acceptance names.
**Over:** treating `allDone` as `finished` here and letting slice 06 change it back — a phase that lies for three slices, and a `run_finished` event on a run with unverified slices.
**Because:** `docs.mjs` `checkpointDueFor` returns true on `allDone` regardless of the cadence; §3 FR-1 says "an unverified checkpoint runs first", so the final checkpoint is part of the run, not after it. The PRD outranks the slice's acceptance wording.
**Undo:** one branch in `scheduler::drive_inner` and the `checkpoint` assertions in `tests/loop.rs`, `tests/run.rs`.
**Review:** decide

### AD-303 — Does the contract inline the inner-loop command? · slice-03 · 2026-10-10

**Chose:** no `{inner}` placeholder: the contract's Verify bullet says "run the inner-loop command given under `# Inner loop` below", and the pack carries the command once, in that section.
**Over:** filling `{inner}` in the contract as well, as task 2 lists it.
**Because:** task 2's acceptance says a different inner command changes exactly one line of the pack; an inlined copy makes it two. The slice contradicted itself and the acceptance is the testable half.
**Undo:** one sentence in `assets/contract-slice.md` and one `replace` in `pack.rs`.
**Review:** fyi

### AD-304 — Double fork or re-exec for the detached driver? · slice-03 · 2026-10-10

**Chose:** `factory run` spawns `current_exe() daemon <id>` with `setsid()` in `pre_exec`, stdin null, stdout/stderr appended to `runs/<id>/driver.log`, and waits up to 5 s for `state.json` to carry the child's pid.
**Over:** the double fork task 5 names.
**Because:** the parent holds an open SQLite connection and may have threads; a bare `fork()` in Rust duplicates both unsafely, and a re-exec gives the daemon a clean process with the same binary and environment. The observable contract (detached, survives the parent, logs to the run dir) is the same.
**Undo:** `src/daemon.rs` and the hidden `Daemon` subcommand in `main.rs`.
**Review:** no

### AD-305 — Does a worker inherit `CLAUDECODE` and `CLAUDE_*` from the driver? · slice-03 · 2026-10-10

**Chose:** stripped from the worker's environment at spawn.
**Over:** passing the environment through untouched.
**Because:** a driver started from inside a Claude Code session (the dogfood case, §1) hands its workers the nested-session guard otherwise; the PRD does not mention the variables.
**Undo:** the `env_remove` loop in `worker.rs`.
**Review:** fyi

### AD-306 — What outcome does a worker get whose stream had a `result` but whose slice is neither `done` nor `blocked`? · slice-03 · 2026-10-10

**Chose:** `failed` in `workers.outcome` and `state.json`; `dead` is reserved for no `result` line. Both count toward the once-then-block cap, both write the Traps line and the `worker_dead` event.
**Over:** one `dead` outcome for both.
**Because:** §3 FR-1 defines a dead worker as "process gone, no DONE/BLOCKED line"; a worker that returned and still left the board untouched is a different failure the owner will want to tell apart in `factory status`.
**Undo:** one string in `scheduler::settle_dead`.
**Review:** no

### AD-307 — What happens to slice 02's `factory run` tests now that `run` drives the loop? · slice-03 · 2026-10-10

**Chose:** reshaped onto the loop: the start/resume test polls the detached driver to `checkpoint` (default `checkpoint_every: 2`) and resumes from there; the budget and stop tests run `--foreground` with a three-step plan; a new test stops a run mid-slice and checks the worker finishes first.
**Over:** keeping them as written — they asserted the pre-loop stop (`starting`, no calls) and raced the daemon.
**Because:** the behaviour they pinned no longer exists; the judge-line, refusal and resume assertions survive unchanged.
**Undo:** `git show 2861352:tests/run.rs`.
**Review:** fyi

### AD-308 — How many entry probes, and which ones? · slice-03 · 2026-10-10

**Chose:** three Haiku calls, not two: the slice's literal deny probe (`git push --force --dry-run`) was inconclusive because the model asked for confirmation instead of calling the tool, so a third call denied `Bash(git status:*)` — a command the owner's user settings allow — under a worker-style system prompt, and got the denial.
**Over:** reporting the literal probe as "unverified" and leaving the deny question to slice 13.
**Because:** slice 13's hook design rests on deny rules holding under `bypassPermissions`; the question was answerable for one more $0.01 call.
**Undo:** nothing; the recordings are evidence only.
**Review:** no
