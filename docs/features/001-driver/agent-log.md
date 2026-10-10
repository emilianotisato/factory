# Agent log — PRD-001

What an unattended run (`/deliver`) decided on its own, and where it stopped. **The owner reviews this in the pull request.** What earns an entry is defined in the feature-docs skill's `autonomy.md`.

**Rules**
- `AD-n` (decisions), `BLOCK-n` (blockers) and `CP-n` (checkpoints) are numbered per feature, monotonic, never reused. `docs check` rejects a duplicate.
- Only what the PRD, the slice or a `DEC-n` did not already settle.
- Never delete an entry. When the owner reviews a decision (`/deliver` § 8), append `**Owner ({date}):** kept` or `**Owner ({date}):** changed — {to what}` under it, and leave the entry as it was.
- **Headings are machine-read — keep their shape.** `docs status` and `docs preflight` resume a run from them.

## Traps

What this repo does that cost a builder a run and will cost the next one too: a tool that takes no paths, a test that needs hidden setup, a formatter that rewrites files outside the slice. One line each, with what works instead. Every builder reads this first and adds what it learned.

- `tests/harness.rs` next to `tests/harness/mod.rs` is a rustc E0761 ambiguity; every test file includes the shared module with `#[path = "harness/mod.rs"] mod harness;` instead of `mod harness;`.
- `docs.mjs` resolves a `./gate.sh` gate by checking the file exists, so the fixture writes `gate.sh` before `docs new`; the other way round the gate reads as missing and preflight refuses.
- `docs preflight` refuses a repo with no `origin` remote (`docs.mjs:1731`); a fixture needs the bare `origin.git` the builder adds, or every preflight test fails on the wrong thing.
- `cargo test` swallows the timing line `tests/harness.rs` prints; `cargo test --test harness -- --nocapture` shows it.

- `docs preflight` never returns an empty `judge[]`: `docs.mjs:1706` pushes the harness line unconditionally, and the fixture gets four (unstruck OPEN-1, harness, no rubric, `on main — create feat/fixture`). A test that needs an unparked `factory run` on the fixture cannot go through the binary; call `cli::run::ensure_branch` and `begin` with a synthetic `docs::Branch` instead.
- `docs log merge 001` on the fixture exits 2 (`no agent-log.md in 001-fixture — nothing to merge into`); a test that wants a 0 has to write an `agent-log.md` first.

- `docs status --json` says `checkpointDue` whenever every slice is done and unverified (`allDone`), whatever `checkpoint_every` says: a fixture run ends at phase `checkpoint`, never `finished`, until slice 06's checkpoint exists — assert `checkpoint`.
- Numbering seeds are numeric, not zero-padded: slice 01 is `AD-101` and `BLOCK-102` (`Number("0101")` in the stub); `format!("{:02}01")` gives `AD-0101` and the pack test does not find `AD-101`.
- A recorded `claude -p` stream opens with `system/hook_started` lines; `init` is not line 1, match it with `any`, not `[0]`.
- `factory run` now detaches a driver that starts building at once: a test that reads `state.json` or runs a second `factory run` right after the first returns races it — pass `--foreground` with a stub plan, or poll the phase.

- A new `tests/*.rs` file that says `mod harness;` fails with E0761: `tests/harness.rs` (the harness's own test) and `tests/harness/mod.rs` both exist — write `#[path = "harness/mod.rs"] mod harness;` as `loop.rs` and `run.rs` do.
- Every pack carries `assets/autonomy.md`, whose § Resuming quotes `**Resolution ({date}):**` — a stub that looks for that string to tell an answered respawn from a first spawn sees it every time and builds the slice to `done` instead of blocking it. The driver's `# Block and answer` heading is the only reliable mark.
- A fixture lives in a `TempDir` that a panic unwinds away, so a detached driver's `driver.log`, `events.jsonl` and `state.json` are gone before anyone can read them — `tests/lanes.rs::wait_until` dumps all three on timeout; without that the first `waiting-owner` timeout was a 60 s blank.

- **Inner-loop `cargo test` runs neither fmt nor clippy.** Slices 03/04 arrived with fmt red on 11 files and clippy red on 7 sites; run `cargo fmt` and `cargo clippy --all-targets -- -D warnings` before the commit, not after.
- **`lanes: 1` in the fixture index opens a preflight `ASK-1` and parks the run** before a worker spawns (`tests/run.rs` `run-ask` relies on it). A one-at-a-time build is `fx.chain()` on the default two-lane index.
- **`docs slice NNN a` titles the slice `A`**, so the driver's subjects are `feat: A`, not `feat: a`; read `slices[].title` from the board, never the slug.

- **`driver.log` holds the detached driver's lines only.** A `--foreground` run says the same lines on its stdout and appends nothing to `runs/<id>/driver.log`; a test that starts in the foreground and resumes detached finds only the resumer's "— resumed" line there (`tests/checkpoints.rs` captures the foreground stdout into `scenario/driver.out`).
- **Every spawn eats a plan step, respawns included.** A `dead` build worker's respawn takes the next step, so a plan that ends in `checkpoint` is one step short and the checkpoint step lands on a build worker, which the stub refuses (exit 65 → dead → respawn, and the count is off by one more). Count spawns, not slices: `[dead, done, done, done, done, checkpoint]` for three slices with one death; a kill-and-resume under a checkpoint worker spawns two checkpoint workers for one `CP-n`.
- **The stub records its call file after the driver has emitted the spawn event.** Waiting on `kinds().last() == "checkpoint_spawned"` and then reading `fx.calls()` races; wait on the call count too.
- **The agent-log Trap "a fixture run ends at phase `checkpoint`, never `finished`" is over as of this slice**: the end of the run is the last checkpoint, so a run with every slice done finishes (`finished (every slice done)`), and `factory run NNN` on a delivered feature finishes at once without a spawn.
- **`checkpoint_every: 10` in a fixture index now means "one checkpoint at the end", not "no checkpoint"**: every loop/lanes/commits plan needs a trailing `checkpoint` step and every exact event tail ends `checkpoint_started, gate_run, log_merged, checkpoint_spawned, checkpoint_done, run_finished`.

- **The closing checkpoint is a stub call (2026-10-10)**: with `checkpoint_every: 10` the fixture still runs one checkpoint after the last slice, and its worker is a recorded `calls/n.json` — a `calls().len()` expectation is builds + 1. Cost one red run in `tests/quota.rs`.

- **`Fixture::calls()` can read a call file mid-write (2026-10-10)**: `a_run_killed_during_the_checkpoint_worker_resumes_at_the_checkpoint` kills the driver while a stub worker runs; once in ten runs `calls/3.json` was empty when the harness parsed it (`EOF while parsing a value`) and the test passed on three reruns. A flake of the harness, not of the slice under test; rerun before chasing it.

## Blockers

### BLOCK-201 — FR-1's "`judge[]` parks the run" rule parks every run, including the fixture's · slice-02 · resolved

**Happened:** PRD §3 FR-1 says `ask[]` and `judge[]` "go to the cockpit as one question each; a run never starts on an unanswered one", and slice 02 task 3, slice 04 and slice 10 repeat it. But `docs.mjs` has something to judge on every repo: `docs.mjs:1706` pushes the harness line unconditionally, `:1707` the rubric line whenever `docs/ux-rubric.md` is missing, `:1743` the branch line whenever the run starts on the default branch. The slice 01 fixture returns four judge lines (`tests/run.rs::run_on_the_clean_fixture_parks_on_the_judge_lines_then_resumes` lists them); this repo returns three. Built as written, `factory run 001` on a clean fixture creates the run, emits `ask_opened` ASK-1..4 and exits 0 in `waiting-owner` — which contradicts task 3's acceptance ("clean fixture → … `events.jsonl` has `run_started`, stdout lists slices 01 and 02, the branch is `feat/fixture`"), task 5's (`phase = 'starting'`) and slice 03's (`factory run 001` on the fixture builds three slices). Those are proven at lib level with a synthetic `Branch` for now; the binary path cannot reach them without an answer path (slice 04) or a different rule. `/deliver` itself (`SKILL.md:34-35`) judges the `judge[]` lines on its own and stops only for `ask[]`.
**Options:**
1. Only `ask[]` parks. `judge[]` items become informational events (`judge_noted`, or `ask_opened` with `source: judge` that does not block) the cockpit shows but the run does not wait on — the `/deliver` behaviour, with the driver as the judge. Cost: FR-1's sentence changes to "`ask[]` goes to the cockpit as one question each; `judge[]` is logged", one `if` in `src/cli/run.rs`, the fixture test flips to the pass path; slices 03–10 can test against the fixture unchanged.
2. Keep the rule as written. Every run, on every repo, starts with an owner ritual of three or four `factory answer`s; slice 03 cannot test its loop on the fixture until slice 04 lands the answer path, so slices 03 and 04 swap order or 03 tests through `begin` only. Cost: the acceptances of slice 02 task 3, task 5 and slice 03 are rewritten, and the unattended run is no longer unattended at its start.
3. Make `docs.mjs` emit judge lines only when there is something to judge (a harness that disagrees with the QA plan, a rubric the PRD asked for) — outside this repo (feature-docs skill), a PRD-002 of its own; this slice would wait on it.
**Recommended:** 1, because `judge` is already defined by `/deliver` as "the orchestrator decides, the owner is not asked", and the driver is the orchestrator; it keeps slices 03–10 testable on the fixture and the run unattended, and nothing of what was built here changes shape — only the routing of one list.
**Resolution (2026-10-09):** owner: "Deliver should be unattended" → option 1. Only `ask[]` parks; `judge[]` lines are logged as one `judge_noted` event each, shown by the cockpit, never waited on. FR-1 and FR-7's `wait` line corrected inline in `prd.md`; slice 02 task 3, slice 04 and slice 10 updated to match.

_(none)_

_(none)_

## Decisions

### AD-101 — Does the fixture repo get a remote? · slice-01 · 2026-10-09

**Chose:** a bare `origin.git` beside the repo in the same temp dir, added as `origin`
**Over:** no remote, which `docs preflight` refuses (`docs.mjs:1731`), or a fake URL, which later slices' pushes could not reach
**Because:** FR-10 asks for a fixture preflight has nothing to raise on, and §12 names only the index keys; a local bare remote also gives slice 09 somewhere to push without network
**Undo:** two lines in `tests/harness/mod.rs`
**Review:** no

### AD-102 — Where do the gate scenario and its log live so a run never dirties the tree? · slice-01 · 2026-10-09

**Chose:** `scenario/` at the repo root, listed in the fixture's `.gitignore`
**Over:** a directory outside the repo passed by env, which `gate.sh` would have to read on every call
**Because:** FR-10 says the gate "reads a scenario file" and preflight refuses a dirty tree (`docs.mjs:1726`); ignoring the directory keeps both true
**Undo:** the `.gitignore` line and the path in `gate.sh`
**Review:** no

### AD-103 — Does the slice write a CI workflow when autonomy.md forbids it? · slice-01 · 2026-10-09

**Chose:** write `.github/workflows/ci.yml` as task 7 orders: ubuntu, Node 20, stable Rust with rustfmt and clippy, the gate string verbatim from `docs/index.md`
**Over:** leaving CI to the owner, which would leave §16 FR-10 without its named evidence (a CI run) and FR-6 with nothing to watch
**Because:** autonomy.md lists CI workflow files under Never, and says the PRD outranks that list for a file it names, with the entry marked `decide`
**Undo:** delete the file
**Review:** decide

### AD-104 — How do the test binaries share the fixture code? · slice-01 · 2026-10-09

**Chose:** `tests/harness/mod.rs` included by `#[path]` from each `tests/*.rs`, with `#![allow(dead_code)]` since no single test uses every helper
**Over:** a `[[test]]` with `harness = false`, or moving the fixture into `src/` behind a feature flag, which would ship test code in the binary
**Because:** the slice names `tests/harness/mod.rs` and `tests/harness.rs` without saying how they coexist (E0761)
**Undo:** rename the smoke test file
**Review:** no

### AD-105 — What does the stub claude replay? · slice-01 · 2026-10-09

**Chose:** `tests/stubs/stream.json`, four events cut from `recordings/claude-p-oneshot-2.1.280.jsonl` (init, one assistant, one user, result) with `{{PLACEHOLDERS}}` the stub fills per spawn; owner paths, plugins, skills, memory paths, MCP servers and hook events removed, usage numbers kept
**Over:** reading the recording at run time, which carries the owner's machine into every stream and would need the same stripping on every spawn
**Because:** FR-10 wants streams "cut from real recordings" and §19 wants nothing owner-specific outside `config.toml`
**Undo:** regenerate from the recording; the cut is a 20-line node script
**Review:** no

### AD-106 — What status does the fixture PRD carry? · slice-01 · 2026-10-09

**Chose:** `status: ready`, the prose Status line to match, `**Harness**: none`, and one `run` row per slice replacing the scaffold's `FR-1` placeholder
**Over:** leaving the scaffold's `draft`, which preflight accepts but which the driver's own preflight (slice 02) may not
**Because:** the slice asks for a QA plan "so `W-NO-QA-PLAN` does not fire", and that warning only checks ready or in-progress PRDs
**Undo:** three branches in `Fixture::patch_prd`
**Review:** no

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

### AD-205 — Does `ask_opened` keep its `source` field once `judge[]` no longer parks? · slice-02 · 2026-10-09

**Chose:** drop it — `ask_opened` is `{id: "ASK-n", text}`; a `judge[]` line is its own kind, `judge_noted {text}`
**Over:** keeping `source: "ask"` on every `ask_opened`, a constant that AD-204 added only so the cockpit could tell the two apart
**Because:** BLOCK-201's resolution routes the two lists to two event kinds, so the kind carries what `source` did; slices 04 and 10 read `ask_opened` and now see one shape
**Undo:** one field in `src/events.rs::EventKind::AskOpened` and its readers
**Review:** no

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

### AD-401 — Who keeps a blocked slice's lane out of `freeLanes`? · slice-04 · 2026-10-10

**Chose:** the driver: `scheduler::free_lanes` subtracts the `lane:` of every slice in the board's `blocked[]` (and of the worker table) from the board's `freeLanes`.
**Over:** trusting the board, as task 2 reads it ("the board keeps it out of `freeLanes`, which is the parking"), or changing `docs.mjs`.
**Because:** `docs.mjs status` computes `heldLanes` from `building[]` only, so a blocked slice's lane is free again on the next read and the third slice would have taken lane 1 beside the parked block. The tool is out of this repo's hands; the parking is the driver's rule to keep, and the acceptance (02 and 03 on lane 2) holds.
**Undo:** drop the `blocked` filter in `free_lanes` once the tool holds the lane itself.
**Review:** fyi

### AD-402 — Same second, or one tick, for the two-spawn acceptance? · slice-04 · 2026-10-10

**Chose:** `tests/lanes.rs` asserts the two `slice_spawned` timestamps are at most 1 s apart (and the two workers alive together, both `building` on lanes 1 and 2 before either exits).
**Over:** the literal "two `slice_spawned` with the same second" of task 1's acceptance.
**Because:** both spawns happen in one tick, but a tick that starts at `.98` crosses a second boundary between them; a `docs status` read sits in between. The same-second form fails on timing the code does not control. What the criterion measures — one tick, not one-after-the-other — is what the gap and the two-alive assertions pin.
**Undo:** `gap == 0`.
**Review:** fyi

### AD-403 — Checkpoint or park when both are due? · slice-04 · 2026-10-10

**Chose:** the checkpoint wins: with the table empty, `checkpoint_due` ends the loop at phase `checkpoint` before open blocks park it.
**Over:** parking first, or spawning across a due checkpoint.
**Because:** slice 03 settled that a due checkpoint stops spawns (AD-302) and slice 06 owns what happens next; a parked run with verified-nothing would hide the checkpoint behind the owner's answer. The cost: task 2's acceptance (`blocked, done, done` → `waiting-owner`) only holds when the checkpoint is not due at that point — with the fixture's default `checkpoint_every: 2`, two `done` slices make it due — so the lanes tests set `checkpoint_every: 10`.
**Undo:** swap the two branches at the bottom of `drive_inner`.
**Review:** fyi

### AD-404 — Who starts the driver of an ask-parked run, and from which preflight? · slice-04 · 2026-10-10

**Chose:** `factory answer` launches the daemon (`daemon::spawn`) when the run's `pid` is dead and its phase is `waiting-owner`; the driver then takes the branch from `runs/<id>/preflight.json`, which `factory run` writes at ask time, and runs `begin` (branch, `judge_noted`, `run_started`, board) exactly as the `pass` path does. A run parked on asks that still has a live driver is picked up on its slow tick.
**Over:** telling the owner to run `factory run NNN` after the last answer (two commands for one act), or re-running preflight in the driver (a second `docs preflight` whose asks may differ from the ones answered).
**Because:** task 4 says "the driver (slow tick) proceeds", but an ask-parked run has no driver — `factory run` returns after printing the asks. The answer is the one owner command the slice promises. The saved preflight makes the run proceed on the questions the owner actually answered.
**Undo:** remove the `daemon::spawn` branch of `answer::wake` and `load_preflight`.
**Review:** fyi

### AD-405 — How do the slice-03 tests survive two lanes? · slice-04 · 2026-10-10

**Chose:** the serial-order tests chain the fixture (`Fixture::chain()`: slice 02 `depends_on: [1]`, rendered and committed) so the plan still runs 01, 02, 03 in order; the stub takes its call number — hence its plan line — from the pack name's spawn suffix (`slice-NN-<spawn>.md`), not from the race to claim `calls/N.json`; `two_dead_workers_block_the_slice_and_park_the_run` runs detached and ends with `factory stop`, since a run with an open block now parks alive instead of finishing.
**Over:** rewriting the slice-03 assertions around either order, or giving the stub a lock.
**Because:** with `lanes: 2` in the index, 01 and 02 spawn together and the stub's "first free number" let whichever started first eat the other's plan line — a `dead` meant for 01 killed 02. The spawn suffix is the driver's order, which is the plan's.
**Undo:** `chain()` is one call per test; the stub falls back to the first free number when the pack name carries no suffix.
**Review:** fyi

### AD-406 — What does an answer file carry? · slice-04 · 2026-10-10

**Chose:** `runs/<id>/answers/<ID>.json` is `{id, slice, text, at}` — the slice an answered `BLOCK-n` belongs to, `null` for an `ASK-n`.
**Over:** the `{text, at}` of task 3.
**Because:** the driver finds the answer for a `building` slice nobody runs by that `slice` field, which is what tells an answered slice from a dead worker's (task 3's trap) without parsing the fragment again.
**Undo:** drop the two fields and look the block up by id.
**Review:** no

### AD-9001 — How does a checkpoint keep a test's fixture for its evidence? · checkpoint · 2026-10-10

**Chose:** `FACTORY_KEEP_FIXTURE=1` makes `Fixture::with_index` call `TempDir::disable_cleanup(true)` and print `fixture <name> kept at <path>` on stderr; nothing else in the harness changes
**Over:** rebuilding each Prove scenario by hand against a `Fixture` (the builder is sixty lines of git, `docs.mjs` and PRD patching), or a second copy of the scenario outside the test suite
**Because:** the QA plan's FR-1, FR-2 and FR-3 proofs name `events.jsonl`, the pack file and a `workers` row, which live in the fixture's `TempDir` and are gone when the test returns; no slice says how a checkpoint reaches them
**Undo:** seven lines in `tests/harness/mod.rs`
**Review:** no

### AD-501 — What id does the driver give a merge-refusal block? · slice-05 · 2026-10-10

**Chose:** `BLOCK-n` with the lowest `n ≥ 1` no `BLOCK-` entry in the feature's logs uses (`BLOCK-1` in the fixture), phase `checkpoint`, written to `agent-log.md`; a second refusal after an answer takes the next free number and parks the run anew.
**Over:** the slice's `BLOCK-<NNN>-merge`. Both `answers::block_heading` and `docs.mjs` `parseAgentLog` require `BLOCK-<digits>`: `factory answer BLOCK-001-merge` would find nothing to resolve and `docs status` would not list the block, so the owner could never clear it.
**Because:** FR-5 says the driver blocks; the id shape is the tool's, not the PRD's. Builders use `NN01`, `NN02`, … so the driver's own blocks stay below 100 without colliding.
**Undo:** one format string in `scheduler::merge_or_block` and the test's expected heading.
**Review:** decide

### AD-502 — Which events report the commit path? · slice-05 · 2026-10-10

**Chose:** `slice_committed {slice, sha, files}` and `untracked_leftovers {slice, files}` as the slice names them; `worker_wrote_protected_file {slice, path}` and `merge_refused {block_id, detail}` with these payloads; and a `log_merged {sha, fragments}` the slice does not name, so the cockpit sees the merge commit the way it sees a slice commit. The park on a refusal is the existing `run_waiting` with `blocked: []`, `asks: []` — the `merge_refused` right before it carries the id.
**Over:** folding the merge sha into `checkpoint_due`, which slice 06 owns; or a new `run_waiting` field for a driver-level block, which would change an event slice 04 already ships.
**Because:** `events.jsonl` is the cockpit's record (§9 FR-7) and FR-5 leaves the payloads open.
**Undo:** five variants in `events.rs` and the assertions in `tests/commits.rs`.
**Review:** decide

### AD-503 — Does the driver create `agent-log.md` when the feature has none? · slice-05 · 2026-10-10

**Chose:** yes — `# Agent log — PRD-{NNN}\n` written before `docs log merge`, which dies with `no agent-log.md in <dir> — nothing to merge into` otherwise (`docs new` does not create the file; the fixture has none).
**Over:** treating the missing file as a refusal, which would park every fresh feature on its first merge.
**Because:** `answer_ask` (slice 04) already creates it the same way; two writers of the header is one convention.
**Undo:** drop the `fs::write` in `commit::merge_logs`.
**Review:** fyi

### AD-504 — What does the merge commit stage? · slice-05 · 2026-10-10

**Chose:** `agent-log.md`, `prd.md` and `docs/index.md` as the slice says (only those `render --write` changed land — on the fixture the index is already current), plus the fragments `docs log merge` removed, so their deletions are in the commit.
**Over:** the three files alone, which leaves ` D log-slice-NN.md` in `git status` and a dirty tree for the next preflight — the slice's own acceptance wants the fragments gone and the tree clean.
**Because:** the fragments were committed with their slices (they are in every slice's pathspec); the merge is what removes them.
**Undo:** drop the fragments from the path list in `commit::merge_logs`.
**Review:** fyi

### AD-505 — How are a slice's leftovers told apart from another lane's half-written work? · slice-05 · 2026-10-10

**Chose:** leftovers = untracked files now − untracked at this worker's spawn (a snapshot kept on the running-table entry) − the slice's pathspec − every other slice's file and fragment − the paths the still-running workers' streams name so far. A worker settled after a driver death has no snapshot, so its baseline is empty.
**Over:** the slice's `git status --porcelain --untracked-files=all` minus the pathspec, which in a two-lane run lists lane 2's new files in lane 1's fragment and names a leftover twice when a later slice runs after it.
**Because:** FR-5 wants the list to be the worker's own unnamed files; the files themselves are never committed either way. Residual: a file lane 2 wrote whose stream line is not yet flushed can still be listed — cosmetic.
**Undo:** `Running.untracked_before` and `scheduler::leftovers`.
**Review:** fyi

### AD-506 — Does `commit_sha` bump `schema_version`? · slice-05 · 2026-10-10

**Chose:** no. The column is in `CREATE TABLE workers` for a new db and a guarded `ALTER TABLE workers ADD COLUMN commit_sha TEXT` (checked through `pragma_table_info`) for an existing one; `SCHEMA_VERSION` stays 1.
**Over:** version 2 with a migration table, which the slice's "idempotent migration" does not ask for and `tests/run.rs` (version 1) would have to follow.
**Because:** SQLite's `ALTER TABLE` has no `IF NOT EXISTS`; the guard is the idempotence.
**Undo:** the `has_column` check in `Db::migrate`.
**Review:** no

### AD-601 — What id do the driver's checkpoint blocks carry? · slice-06 · 2026-10-10

**Chose:** `BLOCK-n` with the lowest free `n` across the feature's logs (`BLOCK-1` in the fixture), phase `checkpoint`, in `agent-log.md` — for the three-reds block and the two-dead-checkpoint-workers block alike. The gate block is titled `gate red three times at checkpoint {n}: {last non-empty gate line}` with the 40-line tail of the last red log quoted under **Happened**.
**Over:** the slice's `BLOCK-<NNN>-cp<n>`, for AD-501's reason: `factory answer` and `docs.mjs` only know `BLOCK-<digits>`.
**Because:** the same answer as AD-501; whichever way that review goes, this follows it.
**Undo:** `checkpoint::lowest_free_block` and the ids in `tests/checkpoints.rs`.
**Review:** fyi

### AD-602 — When does the driver trust a cached red gate? · slice-06 · 2026-10-10

**Chose:** only right after a fix worker in the same checkpoint pass (`trust_red = reds > 0`): a fix that changed nothing is a cache hit, red again, and counts. On entering the checkpoint — fresh, resumed, or after `factory answer` on a gate block — a cached red is ignored and the gate reruns even when the tree is unchanged. A cached green is always reused.
**Over:** trusting every hit, which on a resume or an answer would replay three cached reds and re-block the run instantly with no fix worker between them; or never caching reds, which loses the "the fix touched nothing" signal.
**Because:** FR-4's cache exists to skip a *rerun* when nothing changed; a red the owner was just asked about is the one result worth rerunning, and the slice's own acceptance has the answer rerun the gate.
**Undo:** drop the `trust_red` filter in `checkpoint::gate_once`.
**Review:** fyi

### AD-603 — Which events report the checkpoint, and what became of `checkpoint_due` and `log_merged.sha`? · slice-06 · 2026-10-10

**Chose:** `checkpoint_started {n, covers}` / `checkpoint_resumed {n, covers}`, `gate_run {n, attempt, red, minutes, log}`, `gate_cached {n, attempt, red, log}`, `fix_spawned {n, attempt, session_id, pid}`, `fix_committed {n, attempt, sha, files}`, `checkpoint_spawned {n, attempt, session_id, pid}`, `checkpoint_blocked {n, block_id, question}`, `checkpoint_done {n, sha}` (`sha` null when render and the worker left nothing new to commit). Slice 03's `checkpoint_due` stub is gone; `log_merged {fragments}` drops AD-502's `sha` because the merge no longer has a commit of its own — it lands in `docs: checkpoint n`, the `checkpoint_done` sha.
**Over:** keeping `checkpoint_due` as a marker before `checkpoint_started` (two events for one fact), or a `log_merged.sha` that would always be the later `checkpoint_done.sha`.
**Because:** FR-4 names `checkpoint_started`, `gate_run`, `fix_spawned`, `checkpoint_done` and leaves the payloads open; `events.jsonl` is the cockpit's record (§9).
**Undo:** the variants in `events.rs` and the exact tails in `tests/loop.rs`, `tests/commits.rs`, `tests/lanes.rs`, `tests/run.rs`, `tests/checkpoints.rs`.
**Review:** fyi

### AD-604 — On a resume, where does the killed checkpoint worker's death land? · slice-06 · 2026-10-10

**Chose:** `worker_dead {slice: null, session_id, exit_code}` right after `run_resumed`, before `checkpoint_resumed` and `gate_cached` — the scheduler settles every orphan by role before it reads the board, the way slices 03/04 settle a build worker's, and writes the `Dead checkpoint worker` Traps line. The death counts toward the two-deaths cap (AD-609).
**Over:** the slice's "the first events after `run_resumed` are `checkpoint_resumed` and `gate_cached`", which would mean deferring the checkpoint orphan into `checkpoint::run` — two places that settle orphans.
**Because:** one orphan path; and the cockpit sees the death before the retry, the order it happened in.
**Undo:** move `scheduler::settle_checkpoint_orphan` behind `checkpoint::run`'s resume branch; the `k[16..]` tail in `tests/run.rs`.
**Review:** decide

### AD-605 — What does the checkpoint commit stage? · slice-06 · 2026-10-10

**Chose:** the feature folder, `docs/index.md`, and whatever else the checkpoint worker's stream named (`Pathspec::from_stream` → `commit::expand`), as one `docs: checkpoint n` after `docs log merge` and `render --write`. The merged fragments' deletions, the worker's `CP-n` and pasted evidence, and the rendered PRD table land together.
**Over:** the slice's `docs/features/NNN-*/ docs/index.md` alone, which drops any evidence the worker wrote outside the folder; or per-file staging, which misses the fragments' deletions (AD-504).
**Because:** the folder is the common case; the stream is the record (§4) when the worker steps outside it.
**Undo:** drop `spec.paths` from `want` in `checkpoint::commit_checkpoint`.
**Review:** no

### AD-606 — Where does a fix worker write its decisions? · slice-06 · 2026-10-10

**Chose:** its own `log-fix-{n}.md` (`n` the checkpoint), AD numbers from `AD-9{n}01`, blocks from `BLOCK-9{n}01`; `docs log merge` already folds `log-fix-N.md` after the slice fragments, so the file goes with the checkpoint commit and never races a slice worker (the lanes are drained).
**Over:** letting a fix worker write `agent-log.md` (the file the slice contract forbids) or a slice's fragment (whose numbers are the slice's).
**Because:** the seed keeps a fix worker's numbers out of every slice's `NN01` range below slice 91.
**Undo:** the `{seed}` string in `Pack::for_fix` and `assets/contract-fix.md`.
**Review:** no

### AD-607 — How is the gate's cache key built? · slice-06 · 2026-10-10

**Chose:** `git read-tree --empty` + `git add -A -- . ':(exclude)docs'` + `git write-tree` on a throwaway index at `runs/<id>/gate-index`, plus the trimmed gate command; stored in `runs/<id>/gate-cache.json`. Modified and untracked (not ignored) code counts; a committed `docs/` change does not.
**Over:** the slice's `read-tree HEAD` first, which keeps HEAD's `docs/` blobs in the index, so every checkpoint commit (docs only) would change the key and defeat the cache at the next checkpoint.
**Because:** PRD §6: "hash of the tree minus `docs/**`".
**Undo:** `gatecache::tree_id`.
**Review:** decide

### AD-608 — How do the checkpoint's workers fit the schema? · slice-06 · 2026-10-10

**Chose:** `WorkerRef.slice: Option<u32>` plus `checkpoint: Option<u32>` in `state.json`; `workers.checkpoint`, `gates.cached`, `gates.checkpoint`, `gates.attempt` added by guarded `ALTER TABLE` (slice 05's `has_column`), schema version unchanged; `workers.role` tells `build` / `fix` / `checkpoint` apart (`worker::Job`).
**Over:** a schema bump, or a second table for the checkpoint's workers.
**Because:** the rows are the same thing — a `claude -p` with a pack and an exit — and §10's report reads one table.
**Undo:** the four `ALTER`s in `db::open`.
**Review:** no

### AD-609 — How many checkpoint-worker deaths before a block, counted since when? · slice-06 · 2026-10-10

**Chose:** two (slice 04's `DEATH_CAP`), counted among this checkpoint's workers spawned after the latest `factory answer` on a checkpoint block, so an answer buys two fresh attempts; the block is `Two checkpoint workers died at checkpoint {n}`. A dead fix worker is settled and the gate reruns — it counts as a red, never as a death.
**Over:** counting across the whole run (an answered block would re-block on the next death) or resetting on every spawn (no cap).
**Because:** the slice: "treat it as slice 04 treats a slice worker: once more, then block"; a dead fix worker is FR-4's "counts as a red attempt".
**Undo:** `checkpoint::last_checkpoint_answer`.
**Review:** no

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

## Checkpoints

### CP-1 — after slice-02 · 2026-10-09

**Covers:** slices 01, 02
**Gate:** 2 runs, 0 red, <1 min (4.2 s wall clock on the timed run, warm `target/`; 20 tests) — caught: nothing
**Prove:** FR-10 (clean clone of the branch at 95c57a1, PATH with no `claude` and no `gh`, cold `target/`: `cargo test` green, 20 tests, 12.7 s wall clock, 4.2 s of tests) and the FR-1 refuse half (`tests/run.rs::run_refuses_a_dirty_tree_with_no_run_dir_and_no_spawn` through the binary: exit 1, the tool's refuse text, no `runs/`, empty `calls/`, HEAD still `main`) — evidence: `evidence/cp-1-gate.txt`, `evidence/cp-1-fr10-clean-checkout.txt`, `evidence/cp-1-fr1-refused-preflight.txt`. FR-10's "CI run" evidence is not reachable yet: `.github/workflows/ci.yml` triggers on push to `main` and on `pull_request` only, so the branch push started no run (`gh run list --branch feat/driver` → `[]`); it lands when slice 09 opens the PR. `docs log merge 001` folded `log-slice-02.md` (AD-205) in with no number clash.
**Fixed:** none

### CP-2 — after slice-04 · 2026-10-10

**Covers:** slices 03, 04
**Gate:** 6 runs, 4 red, <1 min (27 s in all; 12.1 s on the first green run and 10.4 s on the rerun over the final tree once the harness knob and the docs were in, warm target dir; 44 tests) — caught: `cargo fmt --check` red on 11 files, 108 hunks (slice-03 left `scheduler.rs`, `stream.rs`, `worker.rs`, `tests/loop.rs`, `tests/run.rs` unformatted; slice-04 its four new files plus `docs.rs` and `tests/cli.rs` — neither builder ran fmt); clippy red on the lib (a collapsible let-chain `if` and `.last()` on a double-ended iterator in `answers.rs`, `is_some()` + `unwrap()` in `pack.rs::cut_section`, `StreamEvent::Result` at 408 bytes against 176 → `Box<ResultEvent>`; the `.next_back()` fix drew a second lint, `rfind`, so two attempts on that one site) and on the test targets (`useless_format` at `tests/lanes.rs:399` and `tests/loop.rs:545`). No fix changed what a slice promises.
**Prove:** FR-1 (`kill -9` mid-slice-01, `factory run 001` resumes: `run_resumed`, `worker_dead`, one fresh spawn of 01, `slice_done` once per slice, `workers` dead/done/done/done; the refuse half was CP-1's), FR-2 (the stub's argv flag by flag; the pack it received for slice 01 with `# Slice`, `# PRD sections`, `# Contract`, `# Autonomy`, `# Inner loop`; three `workers` rows, 0 with a null usage field), FR-3 (`slice_blocked` 01 at seq 8, `slice_done` 02 and 03 at 9 and 11, `run_waiting` 12, `block_answered` 13, the respawn at 14 with `# Block and answer` in its pack, `slice_done` 01 at 15; the fragment's BLOCK-101 `resolved` with its Resolution line) — evidence: `evidence/cp-2-gate.txt`, `evidence/cp-2-fr1-kill-resume.txt`, `evidence/cp-2-fr2-pack-and-worker-row.txt`, `evidence/cp-2-fr2-pack-slice-01.txt`, `evidence/cp-2-fr2-pack-slice-01-cited-s1.txt`, `evidence/cp-2-fr3-blocked-answered-events.txt`. FR-10's "CI run" piece stays unreached until slice 09 opens the PR. `docs log merge 001` folded `log-slice-03.md` (AD-301–308, Traps) and `log-slice-04.md` (AD-401–406, Traps) with no number clash; the two `_(none)_` placeholders it appended under Blockers were dropped.
**Fixed:** formatting and the seven clippy sites above, in this checkpoint's commit (`verify(001): checkpoint after slice-04`); `FACTORY_KEEP_FIXTURE` added to the harness for the evidence runs (AD-9001)

### CP-3 — after slice-06 · 2026-10-10

**Covers:** slices 05, 06
**Gate:** 1 runs, 0 red, <1 min (15.0 s wall clock, warm target dir; 59 tests in 10 binaries) — caught: nothing (both builders ran fmt and clippy before committing, the slice-05 Trap; the tree under test was HEAD c8c8bb0, the merge being docs only)
**Prove:** FR-5 (`tests/commits.rs::two_lanes_commit_out_of_order_and_each_commit_takes_only_its_own_files` on the kept fixture, `lanes: 2`: `git log --stat --format='%h %s'` shows `feat: B`, `feat: C`, `feat: A` in that order — lane 2's two commits before lane 1's, whose worker was still writing — each exactly its own `src/<x>.txt`, slice file and fragment and never a sibling's, then `docs: checkpoint 1` with the three fragments' deletions; `%b` empty throughout, `git status --porcelain` empty, `git fsck --strict` exit 0 with one dangling tree that is the gate cache's `write-tree` (AD-607), the merged `agent-log.md` holding AD-101, AD-201, AD-301), FR-4 (`tests/checkpoints.rs::red_red_green_runs_two_fix_workers_and_three_gates_before_the_cp`: four `gates` rows all `source driver`, checkpoint 1 attempts 1–3 `red` 1, 1, 0 and checkpoint 2 attempt 1 green, none cached, one row per `./gate.sh` line in `gate.log`; two `fix` worker rows whose `commit_sha` are the two `fix: gate red at checkpoint 1` commits touching only `src/lib.txt`; `docs: checkpoint 1` carrying `CP-1 — after slice-02`; and the row's "red ×3" half through `three_reds_block_the_run_and_the_answer_reruns_the_gate`: three reds, two fix workers, `checkpoint_blocked` BLOCK-1 and `run_waiting` with no fourth gate until `factory answer`, then `checkpoint_resumed`, attempt 4 green and the run finishes) — evidence: `evidence/cp-3-gate.txt`, `evidence/cp-3-fr5-two-lanes-git-log.txt`, `evidence/cp-3-fr4-red-red-green-gates.txt`, `evidence/cp-3-fr4-three-reds-block.txt`. FR-10's "CI run" piece stays unreached until slice 09 opens the PR. `docs log merge 001` folded `log-slice-05.md` (AD-501–506, Traps) and `log-slice-06.md` (AD-601–609, Traps) with no number clash and nothing to drop; the Traps line "a fixture run ends at phase `checkpoint`, never `finished`" stays as written, superseded by slice 06's line beneath it.
**Fixed:** none

### CP-4 — after slice-08 · 2026-10-10

**Covers:** slices 07, 08
**Gate:** 1 runs, 0 red, <1 min (24.0 s wall clock, warm target dir; 86 tests in 12 binaries) — caught: nothing (both builders ran fmt and clippy before committing, the slice-05 Trap; the tree under test was HEAD b83a34a, the merge being docs only; the slice-08 Trap's `Fixture::calls()` flake did not show)
**Prove:** FR-9 (`tests/quota.rs::a_rate_limited_worker_pauses_the_run_until_the_reset` on a kept fixture at clock ×20: exactly one `run_paused` with `literal: rejected` and `resets_at` 120 s after its `ts`, not one event between it and `run_resumed_from_pause`, whose `ts` equals `resets_at`, the stub call count held at 1 across the pause, then three `slice_spawned`, workers `rate_limited, done, done, done` + a green checkpoint, no `worker_dead`; and `a_spent_budget_parks_the_run_and_a_higher_budget_resumes_it`: `run_parked {reason: budget, weighted: 11609, budget: 1}` after exactly one spawn, 11609 the hand sum over the one `workers` row, the driver gone, then `--budget 100000000` resumes the same run to `run_finished`), FR-8 (`tests/metrics.rs`, four tests, on the report test's kept fixture: `SELECT count(*)` 4 of 4 workers with non-null usage, 4 `gates` rows — three `source='worker'` inner runs with red 0, 1, 0 and the driver's one checkpoint gate, matching the fixture `gate.log` — and 19 `tool_calls` rows, every one with a kind, `edit` 10, `read` 6, `run-gate` 3; `factory report 001-20261010-6f12` prints the 21 keys in order then AD-802's three with `slices: 3`, `checkpoints: 1`, `workers: 4 (build 3, checkpoint 1)`, `weighted_tokens: 46438`, `--json` one object, `--cockpit` on the fixture transcript `63828` / `58351`) — evidence: `evidence/cp-4-gate.txt`, `evidence/cp-4-fr9-rate-limit-pause.txt`, `evidence/cp-4-fr9-budget-park.txt`, `evidence/cp-4-fr8-report-and-counts.txt`. FR-8's 2% `measure.py` comparison on a real cockpit transcript is the owner's `qa-guide.md` step 6 row, not driven here. FR-10's "CI run" piece stays unreached until slice 09 opens the PR. `docs log merge 001` folded `log-slice-07.md` (AD-701–704, Traps) and `log-slice-08.md` (AD-801–806, Traps) with no number clash; it first refused both for the `# Agent log — slice NN` title line each carried above `## Traps`, which no earlier fragment had — that one line was dropped from each and nothing in any entry touched; the two slices' Progress links to the deleted fragments now point at `agent-log.md`.
**Fixed:** none

---

Entry shapes. Copy them above; examples inside this fence are ignored by the tool.

```markdown
### BLOCK-1 — {what stopped the run} · slice-02 · open

**Happened:** {what, with the `file:line` or command output that proves it}
**Options:**
1. {option} — {its cost}
2. {option} — {its cost}
**Recommended:** {n}, because {why}
**Resolution ({date}):** {appended when the owner answers — then the heading ends in `resolved`}

### AD-1 — {the question, phrased as a question} · slice-02 · {YYYY-MM-DD}

**Chose:** {what}
**Over:** {the alternative, and why it lost}
**Because:** {the reason, citing the PRD section that left it open}
**Undo:** {what reverting costs — a file, a migration, a dependency}
**Review:** {decide | fyi | no}

### CP-1 — after slice-03 · {YYYY-MM-DD}

**Covers:** slices 02, 03
**Gate:** {runs} runs, {red} red, {minutes} min — caught: {what, or nothing}
**Prove:** {the QA-plan rows driven} — evidence: {paths}
**Fixed:** {what a red run or a failed row needed, and its commit} · or none
```

- The middle field is `slice-NN`, or the phase when no slice owns it: `preflight`, `slicing`, `polish`, `pr`, `ci`.
- **`Review: decide`** when the owner must answer before it ships (a deviation from the slice or an acceptance criterion, a schema or public interface the PRD did not specify, a new runtime dependency); **`fyi`** when a reviewer reading the diff would miss it; **`no`** otherwise, which is most entries. `decide` entries go to the top of the PR body.
- A `CP` heading names the last slice it covers; `docs status` computes what is unverified from it.
