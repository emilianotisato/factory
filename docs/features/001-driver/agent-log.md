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

- `docs ship NNN` refuses with exit **2** (`refusing: N slice(s) not done (02). Set them done first…`), not 1; a block body or a test that spells `(exit 1)` is wrong on the first red.
- A worker pack with no files has the header `<!-- factory-pack files: -->` (one space, from `files.join(",")` on an empty list); the CI fix pack is one, and a test spelling two spaces fails.
- `cargo test` stops at the first failing test *binary*: a change every finished run feels (here: the polish worker and two closeout commits at the end of each fixture run) surfaces one binary's failures per run. `cargo test --no-fail-fast` shows them all in one go — three runs here were spent discovering the next binary's shifts.
- The stub `claude` claims `calls/N.json` empty (`openSync … 'wx'`) and writes the JSON a moment later; `Fixture::calls()` polled from a `wait_until` closure panicked on the empty file (`EOF while parsing`) once in seven runs. `calls()` now skips empty files; do not read the calls dir any other way from a poll.
- A finished fixture run now ends with a `polish` worker and the commits `polish: review fixes` and `docs: ship PRD-001`: `fx.calls().len()`, `workers.len()`, `subjects()` and anything reading `HEAD` as the checkpoint commit must count them (`sha_of("docs: checkpoint 1")` instead of `HEAD`).

- **A test that waits on a phase and then reads the event tail races the driver (2026-10-10)**: `state.json` is saved before the event that explains it is appended. `scheduler::finish` used to run `report::compute` in that gap, and `tests/checkpoints.rs::two_dead_checkpoint_workers_block_the_run_and_the_answer_spawns_a_third` read a tail with no `run_finished` once in three runs. The gap is now one append; a new test should wait on the event (`kinds().last() == "run_finished"`), not on the phase.

- The cargo target dir here is `$CARGO_TARGET_DIR` (`/mnt/devcache/emiliano/cargo-target`), not `./target`: `target/debug/factory` does not exist. Run the built binary from `$CARGO_TARGET_DIR/debug/factory`, or `cargo run -q -- …`; tests use `env!("CARGO_BIN_EXE_factory")`.

- 2026-10-10, run 1: `tests/closeout.rs::the_driver_never_merges_or_readies_a_pr` scans `src/` for `pr merge`, and the injected deny rule `"Bash(gh pr merge*)"` in `src/worker.rs` tripped it. The scan now strips that one literal before it looks.

- `gh pr checks --json …` exits 1 when a row failed and 8 when one is pending, and prints the JSON on stdout either way. Parse stdout before reading the exit as an error. "no checks reported" comes on stderr with exit 1.
- `gh` puts `STARTUP_FAILURE` in its `pending` bucket. Classifying by `bucket` waits on a runner that never started, forever. `ci::class` reads `state` first.
- A `gh run rerun <id> --failed` keeps the run id and gives the jobs new ids. A check's job link (`…/runs/<run>/job/<job>`) tells the old attempt's row from the new one; the run id does not.
- `tests/closeout.rs::the_driver_never_merges_or_readies_a_pr` greps `src/` for the two `gh pr` subcommands the driver must never run. Do not write them in a comment or a message, even to say they are not used.
- The stub `gh` used to answer `pr checks` with one row and `run list` with every run on the branch. The wrong-run, wrong-workflow and `--limit` bugs passed every test until it printed real JSON, real job links and a run per workflow. Its header now documents `scenario/ci` (`<kind>:<check>[/<workflow>][@<sha>]`, `lag`, `stale`, `none`, `unreadable`) and `scenario/gh-fail`.

- 2026-10-10, polish fixer 4: tagging every file that names the symbol, so `def` would match `outline` by construction, took `def handle` on a 12,877-file Laravel checkout from 0.10 s to 0.59 s and `def id` to 0.77 s, near FR-12's 1 s. The regex prefilter stays and is widened instead (AD-40001); the same queries then answer in 0.08–0.10 s, as before.

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

_(none)_

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

### AD-901 — What id does a closeout block get? · slice-09 · 2026-10-10

**Chose:** `BLOCK-n` with the lowest free number across `agent-log.md` and the fragments (`checkpoint::lowest_free_block`, slice 06), the phase in the heading — `### BLOCK-n — {title} · polish|pr|ci · open` — and in the event (`closeout_blocked {phase, block_id, question}`). Task 4's "`BLOCK-<NNN>-ci`" is not used.
**Over:** a second id shape for CI (`BLOCK-001-ci`).
**Because:** `factory answer BLOCK-n` (slice 04) and `docs log merge` parse one id shape; a second would need a second parser and a second numbering, and the phase is already in the heading where the owner reads it.
**Undo:** format the id in `closeout::block`; `tests/closeout.rs::block_heading` pins the shape.
**Review:** decide

### AD-902 — When is a red check "flaky" (one rerun) rather than a fix round? · slice-09 · 2026-10-10

**Chose:** when `gh run list --branch <b> --json …` shows a `success` run at the red run's `headSha` (`ci::green_at`): the same tree passed once, so the red is the check's. One rerun per head (`ci_rerun {head}` events at that head counted against `CI_FLAKY_RERUNS = 1`); a second red at that head blocks (`flaky check <c> red again after its rerun · ci`). A refused `gh run rerun` is treated as a fix round. A failed run whose `headSha` is not this branch's head blocks at once with no fix worker (`autonomy.md:79`).
**Over:** the slice's "HEAD equals the HEAD recorded at the last green or at PR creation".
**Because:** that comparison never matches on the first red after a push — no green exists at the new head yet — so a flaky first check would always cost a fix round, and after any fix commit the heads always differ. The run list says whether the same sha passed, which is what "no diff since the last green" means.
**Undo:** replace `green_at` with a lookup of `ci_green {head}` events in `closeout::watch_ci`.
**Review:** decide

### AD-903 — What does a new `factory run NNN` do on a PRD already `shipped`? · slice-09 · 2026-10-10

**Chose:** the preflight lets a shipped PRD through (`docs.mjs:1690`, "resumes at the PR, CI or owner review"), so `closeout::run` treats `board.status == shipped` as polished and shipped (no second polish worker, no second `docs ship`), and `open_pr` with no `pr_url` in state first asks `gh pr view <feature branch> --json url,state` (`ci::pr_view`): an open PR is reused (`set_pr_url`, `pr edit` refreshes the body, no `pr_opened`), none → `pr create`. The watch then runs and the run finishes. `tests/run.rs::budget_comes_from_the_flag_then_the_config_then_the_default` runs a finished fixture twice more: no worker spawned, one `pr create` and two `pr edit` in `gh.log`.
**Over:** polishing and shipping again (a second `polish:` commit and a second draft PR on the same branch); or refusing the run (that is `docs preflight`'s, outside this repo).
**Because:** nothing in §8 or the slice says what a run on a delivered feature does; the preflight's own line says "resumes at the PR".
**Undo:** drop `already_shipped` in `closeout::run` and the `pr_view` call in `open_pr`.
**Review:** decide

### AD-904 — What does the driver fill in when the polish worker leaves gaps? · slice-09 · 2026-10-10

**Chose:** the polish commit's pathspec is the stream's paths plus `{feature}/prd.md` and `{feature}/retro.md` always (the polish worker is the one worker allowed to write those two; `docs/index.md` stays the ship's); a missing `retro.md` is written from `templates/retro.md` beside `docs.mjs` (else a bare frontmatter with the twelve keys), a missing `runs/<id>/pr-body.md` from `assets/pr-body.md` (the PRD title and pointers to the log and the retro) — each said on stdout — so the ship and the PR never wait on a worker that skipped its last turn. The PR title is the PRD's h1 (`# PRD-001 — Fixture` → `PRD-001: Fixture`); the URL is `gh pr create`'s last non-empty stdout line.
**Over:** blocking when the worker wrote no retro or body.
**Because:** the slice says the worker writes them "in its last turn" and the acceptance checks they exist; it does not say what happens when they do not. A block would cost the owner a round-trip for a file the driver can write.
**Undo:** replace the two `fs::write` fallbacks in `commit_polish` with a `block(.., "polish", ..)`.
**Review:** decide

### AD-905 — What do `run_finished`, `pr_url` and `ci_cycles` carry? · slice-09 · 2026-10-10

**Chose:** `run_finished {pr_url, slices, checkpoints, ci_cycles, weighted_tokens, duration_minutes}` from `report::compute`, every field `#[serde(default)]` so a bare `run_finished` from before this slice still parses; `pr_url` in `state.json` and a `runs.pr_url TEXT` column by guarded `ALTER` (`SCHEMA_VERSION` unchanged); the report's `ci_cycles` is the count of `ci_fix_committed` events, and each `fix: CI <check>` commit bumps `ci_cycles` in `retro.md` in the same commit, so the retro the ship wrote stays in step with the report. `factory report` is unchanged otherwise.
**Over:** a second `runs` column for the cycles; a `run_finished` with only `pr_url`.
**Because:** task 5 names the fields and task 3 names the two homes of `pr_url`; the defaults, the `ALTER` and the retro bump are what a diff reviewer would miss.
**Undo:** the column is one line in `db.rs`'s `ALTER` list; the bump is `commit_ci_fix`.
**Review:** fyi

### AD-906 — How do the two new workers appear in the tables and the packs? · slice-09 · 2026-10-10

**Chose:** `Job::Polish {attempt}` → role `polish`, `Job::CiFix {round, attempt}` → role `fix` with `slice` and `checkpoint` NULL (slice 08's report counts both under `fix`, `build_minutes` includes them, `polish_minutes` is the polish worker's `duration_ms`); the polish worker's inner runs are `gates` rows with `source = worker`, `kind = slice` — slice 08's label for every worker run, left alone. The polish pack header carries `<!-- run dir: … -->` after the files line (the worker writes `pr-body.md` there), then `# Branch` (default and feature branch, `git log --oneline <default>..HEAD`), the PRD, `# Retro` (the template in a fenced block), `# PR body` (`<!-- write: <path> -->` and `assets/pr-body.md`'s shape), the contract with `AD-{seed}` = `{NNN}01`-style `AD-9901`, the autonomy text; the prompt is `/polish` (slice 03: `/polish` is among `init.slash_commands`). The CI fix pack is `<!-- factory-pack files: -->`, `# CI failure` with `<!-- round r, attempt a, check: name -->` and `gh run view <id> --log-failed`'s text, `assets/contract-ci-fix.md`, the autonomy text.
**Over:** a `role = ci-fix`; a `gates.kind = polish`.
**Because:** the slice names the roles (`polish`, fix) and the pack sections; the column values and the header line are the parts a reader of the tables would otherwise guess.
**Undo:** `Job::role` in `worker.rs`; `Pack::for_polish` / `for_ci_fix` in `pack.rs`.
**Review:** fyi

### AD-907 — What happens when `gh pr checks` reports no checks at all? · slice-09 · 2026-10-10

**Chose:** `Checks::None` ("no checks reported" in `gh`'s text) ends the watch as if green, without a `ci_green` event: the run finishes with the PR open — a repo with no workflow gets its draft PR and the owner reads it. A non-zero exit with no `fail` row and no such text (`Checks::Unreadable`) blocks in phase `ci` with `gh`'s text.
**Over:** blocking on "no checks" too.
**Because:** §8 says CI red is handled and green finishes; a repo without CI is neither, and the dogfood repo has a workflow so the case is only the fixture's and other repos'.
**Undo:** turn the `Checks::None` arm in `watch_ci` into a `block`.
**Review:** decide

### AD-908 — Where does the watch interval come from? · slice-09 · 2026-10-10

**Chose:** `FACTORY_CI_INTERVAL` (seconds, `> 0`) read at each watch, default `60` (`deliver/SKILL.md:193`); no config key.
**Over:** a `[ci] interval` key in `factory.toml`.
**Because:** task 4 says "interval injectable for tests" and nothing more; an env var is the smallest knob that a test harness and an owner can both turn, and the stub ignores it anyway.
**Undo:** read it from `Config` in `ci::interval`.
**Review:** fyi

### AD-909 — Where does the closeout read `branch.default` from on a fresh run? · slice-09 · 2026-10-10

**Chose:** `cli/run.rs` now calls `save_preflight` on the fresh-run path too (it already did on the resume path), so `closeout::default_branch` reads `preflight.json` from the run dir and falls back to `docs status`'s `branch.default`, then `main`.
**Over:** re-running `docs preflight` at the closeout.
**Because:** the slice says the default branch comes from the board's `branch.default` and that "the resume path has it in `state.json`"; the fresh path did not persist it anywhere.
**Undo:** delete the one `save_preflight` call; the fallback chain still works.
**Review:** fyi

### AD-910 — How is a dead, rate-limited or blocked polish worker handled? · slice-09 · 2026-10-10

**Chose:** slice 04's treatment, phase `polish`: a death is a `worker_dead {slice: null}` event, a Traps line `- **Dead polish worker (date)**: …` and one respawn (`polish_spawned {attempt: 2}`); a second death blocks (`two polish workers died · polish`), and an answer to that block spawns a third (deaths before the answer do not count). A usage limit waits for the reset (slice 07's `await_quota`) and respawns without counting as a death. `STATUS: blocked` parks on the worker's own `BLOCK-n · polish` entry.
**Over:** nothing new.
**Because:** task 1 says "the slice 04 treatment (once, then block with phase `polish`)".
**Undo:** —
**Review:** no

### AD-911 — How do the stubs script the closeout? · slice-09 · 2026-10-10

**Chose:** stub `gh`: `scenario/ci` in the repo root, one line per `pr checks` call — `green` (exit 0), `red:<check>[@<sha>]` (a `fail` row, exit 1; `run list` then shows that run as `failure` at `<sha>` or HEAD), `flaky:<check>` (as red plus a `success` run at the same head); a missing line is `green`; state in `$FACTORY_STUB_DIR/ci-state.json` (`pr`, `checks`, `reds`); `pr view … --json` answers only after `pr create` ran (`exit 1` before, as the real `gh` does for a branch with no PR); `run view <id> --log-failed` prints `FAIL <check>: expected 2 got 3`; `pr edit`, `run rerun`, `auth status` exit 0 silently; every call is one line in `gh.log`. Stub `claude`: the polish pack is recognised by `# Branch`, the CI fix pack by `# CI failure`; plan outcomes for polish are `polish|done` (writes `retro.md`, `reviewed:` in `prd.md`, `<run dir>/pr-body.md`, touches `touch ?? ["src/lib.txt"]`, forces slice statuses from `slices: {"02": "building"}`), `dead` (exit 137), `blocked` (its own `BLOCK-n · polish`), `limited`. `Fixture::script_ci` and `Fixture::gh_log` read and write them.
**Over:** a `gh` stub answering from a fixed JSON.
**Because:** task 4's acceptance scripts four CI shapes; a line-per-call file is the smallest thing that does.
**Undo:** both stubs document their formats at the top.
**Review:** fyi

### AD-1001 — How does `factory wait` tail `events.jsonl`? · slice-10 · 2026-10-10

**Chose:** a 2-second poll (`cli::wait::POLL`) that re-reads `state.json` and `events.jsonl`, napping no longer than the time left to `--timeout`.
**Over:** inotify through the `notify` crate, which the slice offers as the other option.
**Because:** the driver appends a few lines an hour; a 2 s lag on a question the owner answers in minutes costs nothing, and `notify` would be a new runtime dependency for it.
**Undo:** swap the `sleep` in `cli::wait::wait` for a watcher.
**Review:** no

### AD-1002 — How does a stopped run end, and does `wait` return on it? · slice-10 · 2026-10-10

**Chose:** `run_stopped {slices, weighted_tokens, duration_minutes}` is the last event of a run ended by `factory stop`, in place of `run_finished` (fields from `report::compute`, all `#[serde(default)]`), and it is cockpit-relevant: `wait` prints `stopped: N slice(s) done — factory run NNN resumes` and exits 0. `factory stop` on a run with no live driver writes the same state (`stopped: true`, phase `finished`) and the same event, where slice 02 set `finished` silently. The stop path's tests in `tests/loop.rs` and `tests/run.rs` now end on `run_stopped`.
**Over:** `run_stopped` as progress, which the slice's list of `wait` kinds implies ("everything else is progress"). A cockpit waiting on a stopped run would then never get an event; `wait` would fall through to "the run is over" with nothing to say why.
**Because:** task 3 names `run_stopped` but not its payload, and the list of `wait` kinds was written before it existed; the skill needs one line that says the run ended on a stop and how to resume it.
**Undo:** drop `RunStopped` from `cockpit::is_cockpit`; its payload is one variant in `events.rs`.
**Review:** decide

### AD-1003 — Where do `wait` and `status` learn which blocks and asks are open? · slice-10 · 2026-10-10

**Chose:** from the run's own events: every `slice_blocked`, `checkpoint_blocked`, `merge_refused`, `closeout_blocked` and `ask_opened` id, minus those a later `block_answered` or `ask_answered` closes (`cockpit::open_questions`).
**Over:** the board's `openBlocks` from `docs status --json`: a `node` call on every 2 s poll, and asks are not on the board.
**Because:** `factory answer` writes those answer events, so the event log is the same source the answer path uses; the slice does not say where `BLOCK-n still open` comes from.
**Undo:** one function in `cockpit.rs`.
**Review:** no

### AD-1004 — What does the cockpit do with a budget park, whose driver exits by design? · slice-10 · 2026-10-10

**Chose:** a run whose last `run_parked` is newer than its last `run_started`/`run_resumed`, in phase `waiting-owner`, is not a dead driver (`cockpit::parked_on`): `wait` relays `parked: budget X/Y` once, then exits 3 at the timeout with `still running: waiting-owner, parked: budget X/Y, no open question`, and `status` marks the budget `(parked)` instead of printing `driver dead`. The skill asks the owner for a new budget on `parked: …` and runs `factory run NNN --budget <n>` on an answer, then waits again either way.
**Over:** exit 4 `driver dead; run: factory run NNN` (the slice's rule for a dead pid), which would send the cockpit to restart a run that re-parks at once with a second `run_parked`; and the slice's "on `paused`/`parked` relay as information and keep waiting", which on a budget park waits forever, since nothing but `--budget` resumes it (AD-703).
**Because:** slice 07 made the budget park exit its driver (AD-703); the slice's dead-driver rule and its skill recipe were written as if every park kept one.
**Undo:** drop the `parked_on` term from `cockpit::driver_dead` and the `parked:` bullet of the skill.
**Review:** decide

### AD-1005 — How does `wait` avoid acting on a half-written ending? · slice-10 · 2026-10-10

**Chose:** the driver saves `state.json` before it appends the event that says why, and exits after both. So `wait` reads the state first and the events second, re-reads up to 20 times at 100 ms when the phase is over but no `run_finished`/`run_stopped`/`run_failed` is on disk yet, and re-reads once before it believes a dead pid. `scheduler::finish` and `mark_stopped` now compute the report *before* they save the phase, so the gap between the two writes is one file append instead of a database query.
**Over:** emitting the event before the phase: a reader that waits on `run_finished` and then asserts phase `finished` (the cockpit test, and the skill) would race the other way.
**Because:** without it a `wait` landing between the two writes prints an old block line as the run's end, or exits 4 on a driver that has just finished cleanly. The query in the gap also cost one red inner run: `tests/checkpoints.rs::two_dead_checkpoint_workers_…` waits on phase `finished`, then read an event tail with no `run_finished` yet.
**Undo:** the `rereads` counter in `cli::wait::wait`; the order of three statements in each of `finish` and `mark_stopped`.
**Review:** no

### AD-1006 — What do `wait` and `status` print on a run that is already over, or given no run? · slice-10 · 2026-10-10

**Chose:** `wait` on a finished, stopped or failed run with nothing past the cursor prints its ending line again and exits 0; it never blocks. `factory status` with no run id, or an unknown one, lists the live runs of every repo (`no live runs under …` when there are none) and exits 1. Line 6's utilisation is the `rateLimitType` window of the last worker's `rate_limit_event` (`unifiedWindows.<type>.utilization`, else the fullest window), now kept as `stream::RateLimit.utilization_pct`.
**Over:** blocking to the timeout on an ended run (a reattached cockpit would sleep 25 minutes on a finished run before it learnt the PR URL); `status` with no id exiting 0.
**Because:** the slice gives the exit code for a missing id but not the reattach case, nor where the utilisation sits in the stream.
**Undo:** the `is_over` branch of `cli::wait::wait`; the `Ok(1)` in `cli::status::status`.
**Review:** no

### AD-1007 — Long wait or 25-minute poll, with the slice-entry test not run? · slice-10 · 2026-10-10

**Chose:** the skill runs `factory wait <id> --timeout 1500` in a background Bash, the 25-minute poll §9 names as the fallback, and treats exit 3 as "wait again". The test (`--timeout 7200` in a background Bash, does the session wake?) is in `qa-guide.md` FR-7 step 2 for the owner, and the result is noted in `prd.md` §20 and this slice's Progress as "not tested".
**Over:** running the test at slice entry as the slice says. An unattended builder runs under `cargo test` and a subagent, with no interactive Claude Code session whose wake-up could be watched; `/deliver` told this builder not to try.
**Because:** the poll works whether or not a long background Bash wakes the session; the long wait only saves the session a wake every 25 minutes. Switching later is one number in the skill and one needle in `tests/cockpit.rs`.
**Undo:** `--timeout 1500` → `--timeout 7200` in `skills/factory-deliver/SKILL.md` and the test.
**Review:** decide

### AD-9002 — How does a checkpoint get the `wait`, `status` and `stop` transcripts? · checkpoint · 2026-10-10

**Chose:** the `factory()` helpers in `tests/cockpit.rs` and `tests/run.rs` echo each call to stderr — `$ factory <args>`, its stdout and stderr, `exit=<code>` — and return the `Output` unchanged; `cargo test` captures it, `-- --nocapture` prints it
**Over:** re-running the scenarios by hand against a kept fixture (the parked fixture, the SIGKILL, the answer and the stop timing are the test's own choreography), or a second copy of them outside the suite
**Because:** slice 10's Prove names the `wait`, `status` and `stop` transcripts, and the tests asserted on the output without ever printing it; no slice says how a checkpoint reaches them (as AD-9001 for the fixture)
**Undo:** delete the `eprintln!` and its comment from the two helpers; nothing asserts on the echo
**Review:** no

### AD-1101 — Where does a heuristic `def` body end? · slice-11 · 2026-10-10

**Chose:** the body goes on while a bracket the definition opened is still open, or the next non-blank line is indented deeper than the match, or that line opens with `{`; it stops at the first line whose indentation returns to the match's, keeping the closing brace, and at 25 lines (`def::body_end`). Trailing blank lines are dropped; a body cut at 25 lines is counted in the trailer (`1 body cut at 25 lines`).
**Over:** the slice's rule read literally ("up to the first line whose indentation returns to the match's"): on the PHP fixture `final class UserService` is followed by `{` at the same indentation, so the body would be empty or the lone `{`, and a PSR-12 signature spread over lines (`): User` then `{`) would stop at the `)`.
**Because:** task 2 gives the rule in one line and its own fixture breaks it; bracket depth plus indentation keeps the rule's intent on all four stacks. Slice 12 replaces the body with the tree-sitter node range.
**Undo:** one function in `src/ctx/def.rs` and the def snapshots.
**Review:** decide — deviates from the slice's literal body rule (orchestrator regrade, autonomy.md)

### AD-1102 — What does a `refs` hit print, and what does the trailer say when nothing was cut? · slice-11 · 2026-10-10

**Chose:** `path:line  ref  <hit line trimmed>`, then the next line indented four spaces (left out when it is blank or the file ends). Trailers: `N more hits in M files (--max, --per-file to widen)` when the caps cut (M = files with a hit not shown), `N hits in M files (searched K files)` when they did not, `no references to NAME found (searched K files)` on none; then `; N in test files (--tests)` when test files held hits, and `; skipped …` for binary or over-2 MB files. `def` ends `N definitions (searched K files)` or the slice's `no definition of NAME found (searched N files)`.
**Over:** a bare `path:line  text` line for refs (the printer task spells `path:line  kind  signature` for both verbs and a ref has no kind in this slice), and a trailer only when something was cut.
**Because:** the slice spells the cut trailer and the empty `def` line but not the others; a model reading a trailerless list cannot tell whether it is complete, and test files are cut silently otherwise.
**Undo:** `src/ctx/print.rs` and the snapshots.
**Review:** no

### AD-1103 — What does `--vendor` lift, and what counts as binary? · slice-11 · 2026-10-10

**Chose:** `--vendor` (on `def` and `refs`) lifts both the vendor directories and every gitignore rule (`standard_filters(false)`, hidden files still skipped). Binary = a NUL byte anywhere in the file, read whole since files over 2 MB are already skipped at the walk.
**Over:** `--vendor` lifting only the six directory overrides: `vendor/` and `node_modules/` are gitignored in every Laravel and Node checkout, so the flag would do nothing there.
**Because:** §14 "Vendor and gitignored paths are skipped unless `--vendor`"; the slice does not say how binary is detected.
**Undo:** `walk::walk` and `walk::scan`.
**Review:** no

### AD-1104 — How many `planted` hits does the `refs/` fixture hold? · slice-11 · 2026-10-10

**Chose:** 300 textual hits across 40 files, two of them the comment and the string in `refs/caller01.ts` (298 real), plus one in `refs/tests/planted.test.ts`.
**Over:** 300 real hits plus the comment and string (302 textual), which the Current code section's "300 planted hits … plus one hit inside a comment and one inside a string" reads as.
**Because:** task 3's acceptance says `290 more hits` with the comment hit still printed, and slice 12 task 3 expects "the count of real hits is `300 - 2`"; only 300 textual satisfies both.
**Undo:** regenerate `tests/fixture/ctx/refs/` and re-bless the refs snapshots.
**Review:** no

### AD-1105 — Does the `outline` stub exit 0 or 2? · slice-11 · 2026-10-10

**Chose:** exit 2 with `outline needs tree-sitter (slice 12)` on stderr, as task 5 and its acceptance say.
**Over:** exit 0, which the slice's summary paragraph says ("a stub that says so and exits 0").
**Because:** task 5 states the reason (no caller mistakes silence for an empty outline) and its acceptance is the check.
**Undo:** one line in `ctx::run`.
**Review:** no

### AD-1106 — What is a test path matched against? · slice-11 · 2026-10-10

**Chose:** the path relative to the search root (`--in`, else the current directory), with the slice's pattern `tests?/|spec|_test\.go|\.test\.|\.spec\.` written as plain substring checks (`refs::is_test`).
**Over:** the absolute path, which hides every file of a checkout that lives under a `tests/` directory (this fixture does), and the `regex` crate for one pattern, a new runtime dependency.
**Because:** the slice gives the pattern, not what it is matched against. A consequence: `refs X --in tests` searches the test tree without `--tests`.
**Undo:** one function in `src/ctx/refs.rs`.
**Review:** no

### AD-1107 — How is the printed kind found, and what is it for TypeScript's class-member regex? · slice-11 · 2026-10-10

**Chose:** the last keyword of the language's table that precedes the symbol on the matched line (`def::kind_of`); a TypeScript class member, whose regex has no keyword, prints `method`.
**Over:** a capture group per regex, which needs the `grep-matcher` trait as a direct dependency the slice does not name.
**Because:** the slice says "the kind printed here is the keyword matched" and gives the member regex without one.
**Undo:** one function in `src/ctx/def.rs`.
**Review:** no

### AD-1108 — How are the outputs pinned? · slice-11 · 2026-10-10

**Chose:** a plain string compare against `tests/snapshots/ctx/*.txt` (twelve commands, each run twice and compared byte for byte); `FACTORY_BLESS=1 cargo test --test ctx` rewrites them. The slice 12 states are `#[ignore]`d tests beside the slice 11 ones.
**Over:** the `insta` crate, which the slice offers as the other option.
**Because:** twelve files and one loop need no dev dependency; slice 12 re-blesses with the same variable.
**Undo:** `tests/ctx.rs` and the snapshot folder.
**Review:** no

### AD-1201 — Does `--kind fn` take methods? · slice-12 · 2026-10-10

**Chose:** yes, everywhere: `--kind fn` keeps tags mapped to `fn` and to `method`, and the hit still prints its own kind (`def new --kind fn` prints `rs/config.rs:10  method  pub fn new(…)`). `--kind method`, `class` and `type` match their own kind only (`lang::kind_matches`).
**Over:** `fn` ← function only, as the slice's mapping reads ("`fn` ← function; `method` ← method").
**Because:** task 2's acceptance asks `def new --kind fn` to find the `impl` method as kind `method`, which the literal mapping cannot do; one rule for all four grammars beats a Rust-only exception, and "a function" in an agent's question usually includes methods.
**Undo:** one line in `src/ctx/lang.rs` (`kind_matches`) and the `def-new-kind-fn` snapshot.
**Review:** decide — reconciles the slice's kind mapping with its own acceptance, a public flag's meaning

### AD-1202 — How does `outline` nest Rust methods, and what else does it list? · slice-12 · 2026-10-10

**Chose:** an `impl` block is a line of its own (`9  impl  impl Config {`) with its methods indented under it, as a class's are; a trait's method declarations are listed under the trait (`  24  method  fn load(&self) -> Config;`). So the Rust fixture prints the struct, the impl, its two methods, the trait and its one declaration.
**Over:** the acceptance's list read literally ("the struct, the trait and the two `impl` methods") with the methods at the top level, which `--depth 1` could not then hide, or the impl's methods indented under nothing.
**Because:** a Rust method has no class to sit under, and the impl line carries what a reader needs (which type, which trait); the trait's declarations are its interface, the same as a PHP interface's methods, which the slice wants listed.
**Undo:** the `impl_item` and `function_signature_item` patterns in `RUST_EXTRA` (`src/ctx/lang.rs`) and `"impl"` in `outline::outlined`; the `outline-rs` snapshot.
**Review:** decide — prints lines the acceptance's list does not name

### AD-1203 — Upstream `tags.scm` as shipped, or with patterns of our own? · slice-12 · 2026-10-10

**Chose:** each grammar's `TAGS_QUERY` with a few patterns put in front of it (`PHP_EXTRA`, `TS_EXTRA`, `GO_EXTRA`, `RUST_EXTRA` in `src/ctx/lang.rs`); tree-sitter-tags keeps one tag per name, the lowest pattern's, so ours win. PHP: `method_declaration` is a method (upstream: function), enums tagged. TypeScript: function, generator, class, method, `const`/`let`/`var` bound to an arrow or function expression (the whole declaration), type alias, enum. Go: a struct or interface `type_spec` is a class (upstream: type), `type_alias` tagged. Rust: `type` alias is a type (upstream: class), a fn in a `mod` body is a function (upstream: method), trait method declarations, `const`, `static`, `impl`. No locals query is passed.
**Over:** the shipped queries alone.
**Because:** tree-sitter-typescript's `tags.scm` leaves functions, classes and methods to tree-sitter-javascript's, so without these `def useThing` and `outline` on the Vue fixture find nothing; PHP methods come out as `fn`, which task 2's `def handle --kind method` rules out. The locals query only hides references, and only definitions are read.
**Undo:** delete the four constants' lines; the snapshots show what moves.
**Review:** fyi — the kinds come from our query text, not only the grammar crates'

### AD-1204 — Which grammar for `.tsx`, `.js`, `.jsx` and `.mjs`? · slice-12 · 2026-10-10

**Chose:** `LANGUAGE_TSX` for those four, `LANGUAGE_TYPESCRIPT` for `.ts` and Vue `<script>` blocks.
**Over:** TypeScript for all.
**Because:** JSX parses as an error under the TypeScript grammar, and an error sends the file to the textual fallback; the slice rules TSX out for Vue only.
**Undo:** `Grammar::of` in `src/ctx/lang.rs`.
**Review:** no

### AD-1205 — What does the `def` trailer say about what the tree dropped? · slice-12 · 2026-10-10

**Chose:** after `(searched K files)`: `; N textual match(es) dropped: inside comments or strings`, `; N textual match(es) dropped: no definition node` (a line that reads like a definition and is not one, or a Vue template line), `; N definition(s) of another kind (--kind)`, `; N file(s) not parsed cleanly: textual matches kept`. `def handle --kind class` prints `no definition of handle found (searched 48 files); 1 definition of another kind (--kind)`.
**Over:** one reason for every drop, as the slice's example has it.
**Because:** a hit can fail the tree without being in a comment; labelling it so would be false, and an empty answer under `--kind` should say a definition exists.
**Undo:** `print::defs`.
**Review:** no

### AD-1206 — What counts as "fails to parse", and what does it change? · slice-12 · 2026-10-10

**Chose:** a tree whose root `has_error()` (an ERROR or MISSING node anywhere), or tagging that fails. `def` then gives slice 11's output for that whole file (heuristic body, keyword kind mapped to the vocabulary: `class`, `fn`, `method`, `type`, `module`, `constant`, `macro`) and counts it in the trailer; under `--kind`, `fn` and `method` pass for each other there, since the keyword cannot tell them apart. `refs` keeps every hit of such a file unfiltered and says so in its trailer (`; N file(s) not parsed cleanly: comments and strings kept`); `outline` prints what was tagged and notes the file.
**Over:** keeping the confirmed hits of a file with a local error, or filtering a broken file's refs anyway.
**Because:** the slice's "syntax error at the root" names no node test; an unterminated string in a broken tree swallows the code after it, so filtering by it could drop real hits — "a broken file never hides a definition".
**Undo:** `File::confirmed` in `src/ctx/def.rs`, `code_lines` in `src/ctx/refs.rs`.
**Review:** no

### AD-1207 — What does `outline` list, how deep, and what does it say at the end? · slice-12 · 2026-10-10

**Chose:** kinds `class`, `fn`, `method`, `type`, `impl`, and `module` when it has a body; left out: `field` (properties), `constant`, `macro`, a bodyless `namespace X;`, and anything defined inside a function or method (locals). Depth = 1 + the containers around it (class, impl, module); `--depth N` keeps depth ≤ N. Each line is `{2 spaces per level}{line}  {kind}  {the name's line, trimmed}`, the line number being the name's. A directory stands for its source files, non-recursive, dot-files skipped, sorted. A path that cannot be read exits 2 (`factory ctx: cannot read P: …`); a named file of no known language is counted. The trailer is printed only when something was left out: `N nested definitions hidden (--depth)`, files not parsed cleanly, files in no outlined language, skipped binary or over-2 MB files. An empty file prints its header alone.
**Over:** a trailer on every outline, as `def` and `refs` have.
**Because:** the acceptance pins the empty file to "the header and nothing else"; the rest is where the slice was silent.
**Undo:** `src/ctx/outline.rs`, `print::outline`.
**Review:** no

### AD-1208 — Which nodes are comments and strings, and how is the Vue block found? · slice-12 · 2026-10-10

**Chose:** PHP `comment`, `string`, `encapsed_string`, `heredoc`, `nowdoc` (the slice lists `comment` only); TypeScript, Go and Rust as the slice lists them. A `${…}` inside a template string is code (`template_substitution` stops the climb). A `refs` line is kept when any whole-word match on it lies outside comments and strings. The Vue block is the first `<script` followed by whitespace or `>`; a `>` inside a quoted attribute (`generic="T extends Map<K, V>"`) does not close the tag; `script_block` returns `(offset, text)` without a `lang`, since every block parses as TypeScript. A `refs` hit in a Vue template is kept.
**Over:** the slice's PHP list read literally, which keeps `'planted'` in PHP strings.
**Because:** the slice's own heading drops "comments and strings"; PHP strings have four node kinds.
**Undo:** `Grammar::is_comment_or_string`, `Unit::in_comment_or_string` (`src/ctx/lang.rs`), `src/ctx/vue.rs`.
**Review:** no

### AD-9003 — Where does `def` put a definition whose node opens with attributes or a decorator? · checkpoint · 2026-10-10

**Chose:** the header is the line of the definition's name, its number and its text trimmed, as `outline` (AD-1207) and slice 11's regex hit already locate it; the body is the node's lines after it, and when the node opens above its name the body is the node's whole range, the signature line repeated in place, so the attributes still print (`def::node_def`). Found at CP-6 on icg-cloud: `def Company` said `app/Core/Models/Company.php:37  class  #[Fillable([...])]` where `outline` says `38  class  class Company extends Model implements …`; 46 attributed class or method definitions in that repo's `app/` and `tests/` printed an attribute as their signature. Pinned by `tests/ctx.rs::def_locates_an_attributed_definition_at_its_name` (PHP class and method attributes, a TypeScript class decorator); the 20 fixture snapshots did not move.
**Over:** slice 12 task 2 read literally ("The signature line is the first line of the range"), which names the attribute line as the location and the attribute as the signature; or the header at the name with the attribute lines dropped, which loses the node's exact range (§14) and, on a Laravel model, its `#[Fillable]` list.
**Because:** §14 promises `path:line, kind, signature` and "`def` returns exactly the definition with kind and signature"; the fixture has no attributed definition, so the slice's rule held there and failed on the owner's own repository. Rust attributes and TypeScript member decorators are sibling nodes and were never affected.
**Undo:** `node_def` in `src/ctx/def.rs` (three lines) and the one test.
**Review:** decide — changes slice 12 task 2's signature rule

### AD-1301 — `--settings` as inline JSON or as a file? · slice-13 · 2026-10-10

**Chose:** a file the driver writes per worker, `runs/<id>/workers/<session>.settings.json` (`worker::settings_path`), passed as `--settings <file>` right after `--effort`. It holds the three deny rules and the `PreToolUse` hook (`matcher: "Bash|Read"`, command `<current_exe> hook`, shell-quoted only when the path needs it, `timeout: 5`). `FACTORY_PACK=<absolute pack path>` goes in the worker env. Every role gets both.
**Over:** `--settings '<json>'` inline, as the slice's Current code and task 2 read.
**Because:** the run brief requires the settings to live in a file the driver owns under the run's directory. The file also leaves a record of what each worker ran under next to its stream, keeps argv short in `ps` and in `calls/N.json`, and `claude --help` 2.1.280 takes either form. Nothing is written under `~/.claude/` or a project's `.claude/`.
**Undo:** in `Worker::spawn`, pass `settings(&exe).to_string()` in place of the path and drop the `fs::write`. `check_worker_argv` in `tests/loop.rs` then reads the JSON from argv.
**Review:** decide — deviates from the slice's stated argv form

### AD-1302 — What is the shape of the new columns and report keys? · slice-13 · 2026-10-10

**Chose:**
- **Schema** (the PRD does not specify either column):
  - `workers.pack_files`: a JSON array of the pack header's files, set right after `insert_worker`.
  - `tool_calls.pack_doc INTEGER`: 1 on a `read`/`bash-cat` row whose target is held, 0 on the others.
- **Report text:**
  - `followup_reads: N of M ctx calls (P%)` and `pack_doc_read_chars: N` follow `repeat_rate` (`KEYS` 21 → 23).
  - `lookup_share_chars` (slice 08's byte share) joins `EXTRA_KEYS`.
  - The three status lines follow the keys.
  - `--per-worker` adds one line per worker: `worker <id> <role> slice NN (<sid8>): lookups N, ctx_share X.XXX, repeat_rate X.XXX`.
- **Report JSON:**
  - `followup_reads` is `{count, ctx_calls, share}`.
  - `thresholds` is `{followup, ctx_share, repeat_rate}`, each `ok|over`, `ok|under` or `lower|not lower`.
  - `per_worker` is an array, present only with `--per-worker`.
**Over:** recomputing the held list from the pack file at report time; a flat JSON shape.
**Because:** the pack file can be pruned with the run, while the row cannot. The flag is stored per row so the report needs no path matching. The text lines are the slice's own wording.
**Undo:** the two columns in `src/db.rs` (the schema and the add-if-missing list), and `Report`/`PerWorker`/`Thresholds` in `src/cli/report.rs`.
**Review:** decide — new schema columns and a public report and JSON surface that the PRD does not specify

### AD-1303 — Does the hook also deny `gh pr merge` and `git push --force`? · slice-13 · 2026-10-10

**Chose:** no. The deny rules ride in `permissions.deny` alone, and the hook answers an explicit `allow` for everything it does not refuse.
**Over:** the slice's fallback, where the hook denies those commands itself.
**Because:**
- The fallback applies only when slice 03 recorded that deny rules do not hold under `bypassPermissions`. Slice 03 recorded the opposite: Progress, "`--settings` deny holds under `bypassPermissions`", and AD-308's third Haiku probe denied `Bash(git status:*)` under bypass.
- Claude Code 2.1.280 applies a deny rule over a hook's `allow`, so the explicit `allow` lets nothing through that the rules refuse.

The checkpoint's real-worker probe confirms the hook half.
**Undo:** two match arms in `cli::hook::bash` returning `Verdict::Deny`.
**Review:** fyi — a fallback named in the task is deliberately absent

### AD-1304 — Which whole-file reads does the hook refuse? · slice-13 · 2026-10-10

**Chose:**
- Whole-file reads are refused only under a build worker's slice pack (pack file name `slice-*`). Fix, checkpoint and polish packs get the grep hint only.
- In a slice pack:
  - the slice file (header index 0) → `# Slice`;
  - every outlined file → `# Outline`;
  - `prd.md` always passes;
  - so do `agent-log.md` and a `log-*.md` named for an answered block.
- A `Read` with an `offset` or a `limit` passes.
- `cat <held>` is refused unless its first command pipes on (`cat f | head`).
**Over:** refusing every header file in every pack.
**Because:**
- A checkpoint pack lists each slice's file but carries only its Prove task.
- The block log is in the pack as one entry.
- The PRD is in the pack as cuts.

In all three cases, refusing would point the worker at text it does not have.
**Undo:** `Held::section` in `src/cli/hook.rs`.
**Review:** fyi — a worker must now `Read` its own slice by range before its first `Edit`. Watch for that at the checkpoint's real-worker run.

### AD-1305 — What counts as a grep "over the repo"? · slice-13 · 2026-10-10

**Chose:**
- **Lexical only, on the first simple command** (after `cd …`, `env`, `time` and `VAR=` prefixes):
  - `grep`/`egrep`/`fgrep` with `-r`/`-R`/`--recursive`/`--dereference-recursive` (not `-d recurse`), or any `rg` other than `--files`/`--type-list`;
  - with no path, or a path under the hook's `cwd` that reads as a directory: `.`, a trailing `/`, or a last segment with no extension.
- **No `stat`.**
- **These pass:**
  - a single file (`grep -n foo src/one.php`, `rg foo src/x.rs`);
  - a path outside `cwd`;
  - `~` or `$VAR` paths;
  - `git grep`;
  - a grep in a later pipeline stage (`cat f | grep foo`) or after `&&`.
**Over:** stat'ing each path, and scanning every command in the line.
**Because:** the hook must answer in well under 20 ms with no I/O beyond the pack's first line (task 1). It measured a 2.9 ms median and 4.1 ms max over 50 runs. The deny is a hint, not a guard.
**Undo:** `over_repo`/`repo_dir` in `src/cli/hook.rs`.
**Review:** fyi — `ls && grep -rn foo .` is not refused

### AD-1306 — Where do the pack's Outline paths and `def` symbols come from? · slice-13 · 2026-10-10

**Chose:** paths and symbols only from the slice's `## Current code / environment` section, as task 3 says:
- **Paths:**
  - a backticked repo-relative path with an extension, with an optional `:N` or `:N-M`;
  - no `/`, `~`, `$`, `..` or empty segment;
  - existing, in a language `ctx outline` knows.
- **Symbols:** a backticked `name()` or `A::b`, last segment.
- **`def`:**
  - runs in-process for each symbol;
  - a symbol with no hit is skipped;
  - it stops adding once 6,000 chars are written.
- **Outline:** covers the cited files plus every file a `def` hit landed in (depth 2).
- **Caps:**
  - each section is cut at its last whole line within the cap (8,000 and 6,000 chars);
  - a trailer follows the cut: `[cut at N chars: run \`factory ctx outline <path>\` for the rest]`;
  - the header's `files:` gains only the outlined files whose `== path ==` header survived the cut.
- **No match:** the section says so in an HTML comment.
**Over:** outlining only the cited paths; reading symbols from the whole slice, as the slice's Current code bullet reads.
**Because:**
- Task 3's acceptance wants `src/helper.rs` outlined and in `files:` when only `helper()` names it.
- Task text from the whole slice cites symbols the worker is meant to create.
- A cut mid-line would hand the hook a file the pack does not hold.
**Undo:** `Recon::of` in `src/pack.rs`.
**Review:** fyi — the outlined set is larger than the cited paths

### AD-1307 — What do `followup_reads` and `pack_doc_read_chars` count? · slice-13 · 2026-10-10

**Chose:**
- `followup_reads` prints a count and a share of `ctx` calls (`1 of 2 ctx calls (50%)`), with no chars.
- `pack_doc_read_chars` sums `result_chars` over every `read`/`bash-cat` of a held file: ranged reads and `prd.md` included.
- The `ctx-*` kinds are lookups, so `lookup_chars` and `repeat_rate` now include them.
**Over:**
- §10's "count and chars" for `followup_reads`;
- a `pack_doc_read_chars` that counts only the reads the hook refuses.
**Because:**
- The slice's task 5 gives the line format, and the 30% rule reads a share of calls.
- Task 4 defines `pack_doc` by the held list, not by the hook's refusal.
- `ctx_share` is ctx rows among lookup rows (§14 decision 4), which needs them counted as lookups.
**Undo:** `Followups` and the `pack_doc` sum in `src/cli/report.rs`; `LOOKUP_KINDS` in `src/metrics.rs`.
**Review:** fyi — the report's `followup_reads` follows the slice, not §10's wording

### AD-1308 — When is a call "expanded"? · slice-13 · 2026-10-10

**Chose:**
- `label_expanded` is set on `read` and `ctx-refs` rows. Task 4's acceptance puts it on a `read`, while §10 names `ctx-*` rows only.
- A `read` is expanded when a later `read` of the same file within five calls has a larger `limit` or none. The first read must have had a `limit`.
- A `ctx-refs` is expanded when a later one has the same word and `--in` and a larger `--max`, where the default is 10.
- Rows of other kinds stay null.
**Over:** setting the label on whole-file reads.
**Because:** a whole read cannot be widened.
**Undo:** `metrics::labels`.
**Review:** no

### AD-1309 — How much of a `ctx` answer is kept, and from which binary does the hook run? · slice-13 · 2026-10-10

**Chose:**
- A `ctx` call's `tool_result` head is kept up to 65,536 chars (`stream::CTX_RESULT_HEAD`), so the follow-up detector sees every named path. Other calls keep slice 08's 4,096 chars.
- The hook command is `std::env::current_exe()`, as task 2 says, which is the test binary in in-process tests.
- `ctx_kind` reads the first command through `shell::first_command`, so `cd x && factory ctx refs y` counts. The other kinds keep slice 08's whitespace reading.
**Over:** one head size for all calls.
**Because:** a `refs` answer can run to several thousand chars and the detector needs its paths.
**Undo:** `CTX_RESULT_HEAD` in `src/stream.rs`.
**Review:** no

### AD-10001 — How are workers kept off `gh` and `git push`? · polish · 2026-10-10

**Chose:** three layers, each enough for the review's cases on its own where it can see them:
- **Deny rules** `Bash(gh)`, `Bash(gh *)`, `Bash(git push*)` in every worker's `--settings` (`worker::DENY_RULES`): any `gh` and any `git push`, not just `gh pr merge*` and `git push --force*`.
- **The hook's guard** (`cli::hook::guard`, on `shell::script`) reads every simple command in the line at any depth: lists, pipes, groups, `if`/`for`/`case` bodies, `$(…)`, backticks, `<(…)`, heredoc bodies fed to a shell, `sh -c`/`bash -c` strings, `eval`, `env -S`, and wrappers (`env`, `command`, `exec`, `nohup`, `time`, `timeout`, `nice`, `stdbuf`, `sudo`, `xargs`, `find -exec`, `watch` and the rest of its table). It denies any `gh`, and any `git push` (also `send-pack`, `subtree push`, `lfs push`, an inline `-c alias.x=push`) after git's global options (`-c`, `-C`, `--git-dir`, `--no-pager` …). Where the reading is unsure — an unbalanced quote, an interpreter handed inline code or stdin, a shell reading a pipe, a program word that is an expansion — it denies when `gh` or `push` appears as a whole word anywhere in the line; hook input that does not parse is denied on the same test. One reason for every deny: `workers never run gh or git push: the driver pushes the branch and talks to GitHub; …`.
- **No credential:** `GH_TOKEN`, `GITHUB_TOKEN`, `GH_ENTERPRISE_TOKEN` and `GITHUB_ENTERPRISE_TOKEN` are removed from the worker's env after the lane's env is applied, and `GH_CONFIG_DIR` is an empty 0700 `runs/<id>/gh-config`, so a `gh` that gets past both finds no login. `SSH_AUTH_SOCK` is kept.

**Over:**
- Wider prefix rules alone: a rule cannot see `true && gh …` or `git -c k=v push`, which the review proved.
- Taking `gh` off the worker's `PATH`: `/usr/bin/gh` by absolute path walks round it, and the worker's `PATH` is the owner's.
- Scrubbing `SSH_AUTH_SOCK`: it breaks SSH commit signing and SSH dependency fetches. An HTTPS push with a credential helper would not need it anyway.

**Because:** the never-list is held against an agent under pressure, not a malicious one (the brief's threat model). Three layers mean an accidental `gh` or push fails even when one layer misses it. What still gets through: a script file the worker writes and then runs, a git alias from config, a `make` or `npm` target, a wrapper not in the table, and a `git push` through SSH or a credential helper, since only the GitHub tokens and `gh`'s config are withheld. The real Claude Code check refused all three probes (`evidence/polish-f1-deny-real.txt`).
**Undo:** `DENY_RULES`, `GH_TOKEN_VARS` and `gh_config_dir` in `src/worker.rs`; `guard` and its helpers in `src/cli/hook.rs`; `shell::script` in `src/shell.rs`.
**Review:** decide — the worker env and deny rules are the PRD's §4 FR-2 interface, widened; `SSH_AUTH_SOCK` stays.

### AD-10002 — How do the pack and the hook name the `factory` binary? · polish · 2026-10-10

**Chose:** `paths::factory_command()`: the running binary's absolute path (`current_exe`, with Linux's ` (deleted)` suffix stripped after a rebuild, single-quoted when it holds a shell character), when its file name is `factory`. Otherwise it is the bare `factory`: a test binary that builds a pack in-process is not the CLI. The `# Lookup` verbs (`pack::lookup`), the grep hint and the settings' hook command all use it.
**Over:** the bare name, with `qa-guide.md` requiring an installed `factory`. CP-7's real worker followed the hint and got exit 127.
**Because:** the worker should run the same build that wrote its pack.
**Undo:** `factory_command`/`self_exe` in `src/paths.rs`, `pack::lookup`, `hook::grep_hint`.
**Review:** no

### AD-10003 — What is "the repo" a held file is relative to? · polish · 2026-10-10

**Chose:** `FACTORY_REPO`, which the driver sets to the run's repo for every worker. Without it (the hook run by hand), it is the call's `cwd`. `Held::section` matches `<repo>/<held path>` exactly. Recursive greps are judged against the same root.
**Over:** `CLAUDE_PROJECT_DIR`, which belongs to whatever session started the driver and is scrubbed from workers (AD-10007).
**Because:** a worker that runs `cd src` is still in the repo, and another checkout with the same relative path is not the pack's file (C3).
**Undo:** `REPO_ENV` in `src/worker.rs`, `hook()`/`hook_with` in `src/cli/hook.rs`.
**Review:** no

### AD-10004 — How does `autonomy.md` square "read whole" with AD-1304? · polish · 2026-10-10

**Chose:** in `assets/autonomy.md`, line 67 now reads "whole when it fits … unless your pack already holds it: then read only the range you need, because a whole read of a file in your pack is refused". In the same pass, three lines now say what the hook enforces:
- line 58, in the Never list: a worker runs no `git push` and no `gh` at all;
- line 78: the driver does the flaky rerun;
- line 85: the driver commits and pushes a block, and a worker stops after step 2.

`assets/contract-ci-fix.md` lines 5 and 13 say the driver fetched the failed log and that a worker never runs `gh`. Every change is made in place, so the file keeps 100 lines and the `autonomy.md:NN` citations in `src/ci.rs`, `src/scheduler.rs` and the PRD still point at the same lines.
**Over:** loosening AD-1304 to allow whole reads, or adding lines.
**Because:** without this, the pack tells a worker to do what its own hook refuses (Cn2), and to run `gh run rerun` and `git push`.
**Undo:** the asset lines.
**Review:** fyi — `assets/autonomy.md` is no longer byte-identical to `~/.claude/skills/feature-docs/autonomy.md`. It is this repo's own copy (slice-03), and line 67 may be worth taking upstream.

### AD-10005 — When is a grep's operand a directory, and where is it resolved? · polish · 2026-10-10

**Chose:**
- **Cn4:** an operand is a directory when `Path::is_dir` says so and it is under the repo, or when it is a glob whose last part is `*` or `**`. A path that does not exist is not a directory. This replaces AD-1305's lexical rule, where a trailing `/` or a last segment with no extension meant a directory.
- **Cn3:** each leading `cd X &&` moves the directory later operands resolve against. A target the hook cannot follow (`cd`, `cd -`, `cd ~`, `cd "$D"`) means no decision.

**Over:** the lexical rule, which denied `grep -rn x Makefile` and passed `grep -rn x src.d`.
**Because:** one `stat` per operand costs microseconds against AD-1305's 20 ms budget, and the deny is a hint, so a missed one costs only tokens.
**Undo:** `repo_dir` and `bash` in `src/cli/hook.rs`, `shell::first_command_in`.
**Review:** no

### AD-10006 — How does a worker's stream reach its file? · polish · 2026-10-10

**Chose:** the worker's stdout is `runs/<id>/workers/<session>.jsonl` itself (`Stdio::from(File)`), with no tee thread. The driver parses the file when the process exits (`summary_at`).
**Over:** keeping the tee and ignoring `EPIPE`.
**Because:** the stream must outlive a driver that dies: a tee leaves the worker writing into a pipe nobody reads (L9). Also, a grandchild holding the pipe could stall the reader's join. `tests/worker.rs::the_stream_outlives_the_driver` kills only the driver and finds the worker's `result` line on disk.
**Undo:** `Worker::spawn`/`exit` in `src/worker.rs`.
**Review:** no

### AD-10007 — Which `CLAUDE_*` variables does a worker keep? · polish · 2026-10-10

**Chose:**
- **Kept:** `worker::KEEP_CLAUDE_VARS` and every `CLAUDE_CODE_USE_*` (Bedrock, Vertex, Foundry). The kept list is `CLAUDE_CONFIG_DIR`, `CLAUDE_CODE_OAUTH_TOKEN`, `CLAUDE_CODE_API_KEY_HELPER_TTL_MS`, `CLAUDE_CODE_SKIP_{BEDROCK,VERTEX,FOUNDRY}_AUTH` and `CLAUDE_CODE_CLIENT_{CERT,KEY,KEY_PASSPHRASE}`.
- **Scrubbed:** `CLAUDECODE` and every other `CLAUDE_*`.
- **Untouched:** `ANTHROPIC_*`, as before.

**Over:** scrubbing only a named list of session variables, which would leak any session variable a later Claude Code adds.
**Because:** the account, config dir and provider must reach the worker, or it logs in as someone else or not at all (L17). The session's variables must not reach it.
**Undo:** `KEEP_CLAUDE_VARS`/`scrubbed` in `src/worker.rs`.
**Review:** fyi — a provider or auth variable outside the list is dropped; add it there.

### AD-10008 — Which files and directories are private? · polish · 2026-10-10

**Chose:**
- The data dir is created 0700 only when it is missing. An existing one keeps its mode, since `FACTORY_DATA_DIR` may name a directory the owner shares on purpose.
- `workers/` and `gh-config/` are 0700.
- A worker's stream, stderr and settings files are 0600.

**Over:** a `chmod` of existing directories.
**Because:** streams hold the worker's whole transcript (S-n2).
**Undo:** `private_dir_all` in `src/paths.rs`, `private_file` in `src/worker.rs`.
**Review:** no — `runs/` and `runs/<id>/` are created by `src/run.rs`, outside this fix's files, and keep the umask's mode.

### AD-10009 — Where do paths become absolute? · polish · 2026-10-10

**Chose:** `paths::data_dir()` and `config_dir()` resolve against the driver's cwd. `Worker::spawn` makes the run dir and the pack path absolute once more before they go on the command line or into `FACTORY_PACK`.
**Over:** absolutizing only the worker's flags, which Ln6 named. A relative `FACTORY_DATA_DIR` also broke the checkpoint's `git read-tree` into `runs/<id>/gate-index`, run in the repo. `tests/worker.rs` caught this from a subdirectory.
**Because:** every consumer of the data dir runs somewhere else: the worker, `git`, the gate.
**Undo:** `dir` in `src/paths.rs`.
**Review:** no — the detached driver (`src/daemon.rs`) still inherits a relative `FACTORY_DATA_DIR` and runs in the repo; it should be handed the absolute one.

### AD-10010 — What does the no-merge source scan look for? · polish · 2026-10-10

**Chose:** `tests/closeout.rs::merges_or_readies` reads each `src/**/*.rs` file in two ways. With whitespace runs collapsed, it looks for `pr merge`, `pr ready` and `/merge` (the REST endpoint). With whitespace removed, so that rustfmt's one-argument-per-line arrays read as one, it looks for `"pr","merge"`, `"pr","ready"`, the GraphQL `mergePullRequest`, `markPullRequestReadyForReview` and `enablePullRequestAutoMerge`, and a `"pr","create"` array without `"--draft"`. The scan has no exemption: the deny rules name no merge, so the stale strip of `"Bash(gh pr merge*)"` is gone. The test also checks the scan itself against each form and against `ci::pr_create`'s shape (K3).
**Over:** keeping the substring scan, which missed the args-array form the driver actually uses for `gh`.
**Because:** this keeps the test's intent, that no code path in the driver merges a PR or marks one ready, and makes it see how the driver really calls `gh`.
**Undo:** the test.
**Review:** no

### AD-20001 — Which number does the driver's death block on a slice take? · polish · 2026-10-10

**Chose:** an open block already titled "Two workers died on slice NN" if a kill left one written. Otherwise the lowest `BLOCK-NNkk` with `kk` from 02 that no log file of the feature uses (`scheduler::dead_block_id`).
**Over:** the lowest free number below 100, as the checkpoint's blocks take (AD-501); or `kk` from 01.
**Because:** the slice's range keeps the block attributable to the slice when the merge folds it. Starting at 02 keeps the id the driver has always written whenever nothing clashes, and skipping used numbers ends the clash with a builder's own `BLOCK-NN02`.
**Undo:** in `dead_block_id`, start the range at 1, or call `checkpoint::lowest_free_block`.
**Review:** no

### AD-20002 — What orders "since the last answer" for the death cap? · polish · 2026-10-10

**Chose:** `events.jsonl` order. A slice worker counts toward the cap unless its `slice_spawned` comes before the latest `block_answered` naming the slice. A checkpoint's workers are spent by the latest `block_answered` for one of that checkpoint's own `checkpoint_blocked` ids. A worker with no spawn event counts.
**Over:** comparing `spawned_at` with the answer file's `at`, which the checkpoint used. Both are whole seconds, so a respawn in the answer's second compares wrongly.
**Because:** `seq` is a total order (the emit is serialised under a lock, AD-20006), and `factory answer` now logs `block_answered` before the slice flips, so the answer always precedes the respawn it causes.
**Undo:** filter on `spawned_at > answer.at` in `scheduler::deaths_since_answer` and `checkpoint::spawned_before_answer`.
**Review:** no

### AD-20003 — Which answers reset a checkpoint's death cap? · polish · 2026-10-10

**Chose:** only the answers to that checkpoint's own blocks: the three-reds block, the two-deaths block, or the worker's own block, all emitted as `checkpoint_blocked {n}`.
**Over:** any answer naming no slice. That covers a refused merge, a closeout block or another checkpoint's block.
**Because:** an answer to a refused merge says the fragments are fixed. It says nothing about why the checkpoint workers died, so the once-only respawn should not come back with it.
**Undo:** treat `merge_refused` block ids as the checkpoint's own in `checkpoint::since_answer`.
**Review:** no

### AD-20004 — What does a sanitized BLOCK title look like? · polish · 2026-10-10

**Chose:** `answers::block_title`. Whitespace runs, newlines included, collapse to one space, every `·` becomes `-`, and the title is cut at 160 chars ending in `…`. The checkpoint's three-reds and refused-merge titles go through it.
**Over:** escaping the separator, or dropping the gate line from the title.
**Because:** `entries` and `resolve` split the heading on ` · `. A `·` in a gate's last line shifted the phase and `open` fields, so the block no longer parsed as open. A 2000-char compiler line in a heading also helps nobody. The full output stays in the body and the transcript.
**Undo:** use the raw `format!` titles in `checkpoint::run` and `block_on_refusal`.
**Review:** no

### AD-20005 — What does the driver do when HEAD leaves the run's branch? · polish · 2026-10-10

**Chose:** two guards. Each tick, before the reap, the driver checks HEAD against the run's branch. Off the branch, it parks in `waiting-owner` with a line naming `git switch <branch>`. In-flight workers finish but are not routed, nothing spawns, and the run goes on by itself once HEAD is back. Every `commit::commit` in the driver process is also held to the run's branch (`commit::hold_branch`, a process-wide table keyed by repo), so a switch between the check and a commit errors instead of landing a commit. That error fails the run. `factory run NNN` refuses a resume off the branch before it adopts anything. A run whose branch is the preflight's default branch refuses at creation, at resume and at every commit.
**Over:** a BLOCK-n the owner answers after switching back; failing the run on the first tick off the branch; or checking only in the four driver commit sites of my files.
**Because:** switching back is the whole fix, so a block and an answer would add a step without adding information. A park that clears itself keeps the run. The process-wide hold also covers the polish, ship and CI-fix commits in `closeout.rs`, which this fixer may not edit.
**Undo:** drop the `branch_refusal` block at the top of the `drive_inner` loop, and the `held` check in `commit::commit`.
**Review:** fyi

### AD-20006 — How are emitters in several processes serialised? · polish · 2026-10-10

**Chose:** `flock` on `events.jsonl` itself, through std `File::lock`. It is taken before the last `seq` is read and held until the `events` row is inserted. The line and its newline go out in one `write_all`.
**Over:** a separate lock file; an in-process mutex, which `factory stop` and `factory answer` never see; or the `fs2`/`fd-lock` crates, a new runtime dependency.
**Because:** the file is the thing being appended, so locking it needs no second path to create or clean up. Holding the lock through the insert keeps the table's rows in `seq` order with the file. AD-20002's "since the last answer" depends on that order being total. rustc 1.98 has `File::lock` in std.
**Undo:** drop the `f.lock()` call in `events::emit`.
**Review:** no

### AD-20007 — Whose are the tracked files a worker changed without naming them? · polish · 2026-10-10

**Chose:** the worker's, when nothing else ran beside it. A file changed since the worker's spawn baseline that no tool call named goes in the worker's commit if it is tracked, not protected, and no other worker was live. That applies to a slice worker with no sibling, and to every fix and checkpoint worker, since the lanes drain before a checkpoint. With a sibling live, the file is listed (an `untracked_leftovers` event and the fragment's list for a slice, `checkpoint_leftovers` at a checkpoint) and left in the tree. A new, untracked file that no tool call named is always listed, never committed, as before. A file once listed is never swept later. Each checkpoint worker's baseline is saved under `runs/<id>/untracked-before/checkpoint-n-<role>.json`, as a slice's is, so a resumed driver judges an orphan the same way.
**Over:** listing every tracked change as well (it would leave a `sed -i` or a formatter's edit uncommitted for good); or sweeping with siblings live (whose change it is cannot be told).
**Because:** with one worker running, a tracked file that changed under it is its work. The commit message already names the worker, so the sweep loses nothing, and `git diff` of the commit shows it. An untracked file can be a scratch file, a build output that `.gitignore` misses, or a secret, so it stays a human call. The cost is that an owner editing a tracked file by hand during a lone worker's run sees the edit in that worker's commit.
**Undo:** pass `alone: false` to `scheduler::unnamed` everywhere, so every unnamed change is listed.
**Review:** fyi

### AD-20008 — Does a slice whose worker died wait for a due checkpoint? · polish · 2026-10-10

**Chose:** yes, on resume and in an unbroken run alike. With `checkpointDue` set, a `building` slice with no live worker (a death below the cap, or an answered block) is not respawned. Workers in flight finish, the checkpoint runs, and the respawn follows it. A death-cap block still happens at once, since it spawns nothing.
**Over:** respawning first and counting the respawn as part of draining the lanes.
**Because:** PRD §5 FR-4 says to drain the lanes on `checkpointDue`, and the board reports `spawnable: 0`. A respawn is a spawn. Respawning first also lets a checkpoint cover more than `checkpoint_every` slices. The cost is that the gate runs over the dead worker's half-written changes. The checkpoint commit leaves them out, because they are in its spawn baseline, so the respawned worker still owns them.
**Undo:** drop the `board.checkpoint_due` skip in the building-respawn loop of `drive_inner`.
**Review:** fyi

### AD-20009 — When `factory answer BLOCK-n` fails partway, how does a retry finish it? · polish · 2026-10-10

**Chose:** two changes. First, everything that can refuse runs before anything is written; a block naming a slice with no file is refused and nothing is written. Second, the steps run in this order: answer file, `block_answered`, then the heading and its `**Resolution**`, then the slice flip, then the wake. A retry that finds every `BLOCK-n` resolved finishes the answer when the feature's resumable run has it on file and is still stuck: either the slice is `blocked`, or the run is `waiting-owner` with no driver and is not parked on its budget. Otherwise it is refused as "already resolved", as before.
**Over:** writing the heading last, after the flip.
**Because:** a respawned worker's pack quotes the entry as the log holds it (`block_section`). If the flip happens before the heading, a live driver can respawn the slice in between, and the worker gets the open block without its resolution.
**Undo:** in `cli/answer.rs`, call `answers::resolve` first again and drop `answer_resolved`.
**Review:** no

### AD-20010 — Two open runs wait on the same `ASK-n`: which does `factory answer` take? · polish · 2026-10-10

**Chose:** neither. It refuses and names each run, with its feature and the ask's text. A new `--run <id>` option on `factory answer` picks one.
**Over:** the newest run (what it did, silently); refusing with no way to answer; or matching on the feature number.
**Because:** ask numbers start at 1 in every run, so two features parked at preflight in one repo both have an ASK-1. A refusal with no option would leave both runs unanswerable. The run id is what the refusal prints, so it is what the owner copies.
**Undo:** remove `AnswerArgs::run`, and take the first run that waits.
**Review:** fyi

### AD-20011 — Does a run parked on its budget get its own phase? · polish · 2026-10-10

**Chose:** no. It stays `waiting-owner`. `factory answer` does not start a driver for a run that `quota::hold` says is over budget. It prints the weighted tokens, the budget, and `factory run NNN --budget <higher>`.
**Over:** a `parked-budget` phase.
**Because:** the budget is read from the `workers` rows each time, so the hold check already tells the two parks apart. A new phase would touch the cockpit, `status`, the `runs` table and every match on `Phase`, and those files belong to other fixers.
**Undo:** drop the `Hold::Budget` arm in `cli/answer.rs::wake`.
**Review:** no

### AD-20012 — What does the driver do with an answer file that does not parse? · polish · 2026-10-10

**Chose:** skip it, and say so in the driver log once per file. An `ASK-n` with a torn file counts as unanswered, so the park line lists it and `factory answer ASK-n` writes it again. `answers::write` now writes a temporary file and renames it, so `factory answer` cannot leave a torn file.
**Over:** failing the run (what it did); deleting the file.
**Because:** one bad file in `answers/` stopped every slice of the run, though the other answers were fine. Deleting it would lose what the owner can still read by hand.
**Undo:** make `answers::scan` return the parse error again.
**Review:** no

### AD-20013 — Where does an unclosed fence in a log file end? · polish · 2026-10-10

**Chose:** fences pair from the top. With an odd count, the driver does not treat the last fence line as a fence, so the entries after it are still found. `answers::resolve` repairs the file: it adds the missing closing fence before the next heading after the opener, then writes the resolution.
**Over:** reading it as docs.mjs `parseAgentLog` does, where the fence runs to the end of the file; or closing the fence at the next heading even when the count is even.
**Because:** the old behaviour hid a worker's later `BLOCK-n` from `slice_blocked` and `factory answer`, and wrote the resolution as code. After the repair, the driver and docs.mjs read the same entries. Closing at a heading when the count is even would break the template's fenced examples, which hold BLOCK headings.
**Undo:** in `answers.rs`, make `dangling_fence` return `None`.
**Review:** fyi

### AD-20014 — Does a run that reaches its budget exactly park? · polish · 2026-10-10

**Chose:** no. `quota::hold` parks when the unrounded weighted sum is above the budget (`>`). The hold reports the rounded sum, as `status`, `report` and the `run_parked` event already do.
**Over:** `>=`; or comparing the rounded sum (what it did, which let 1.4 pass a budget of 1).
**Because:** PRD §11 FR-9 says "Past the budget the run … parks", and a budget is what the run may spend. Rounding first moved the line by up to half a token.
**Undo:** compare `weighted(..)` with the budget again in `quota::hold`.
**Review:** no

### AD-30001 — How long does "not registered yet" last after a push, a rerun or the watch's start? · polish · 2026-10-10

**Chose:** a grace window of `ci::GRACE_POLLS = 5` polls (`5 × FACTORY_CI_INTERVAL`, 5 min by default), counted from the watch's start, each fix push and each rerun. Inside it, each of these is a pending poll:
- a PR whose `headRefOid` is not the pushed head;
- an empty check table;
- a stale rerun row;
- a check name from the last red table that is missing from this one.

Past it:
- a PR head that is still behind blocks (`the PR's head X is not this branch's head Y · ci`);
- an empty table ends the watch as AD-907 says (`ci: no checks reported … nothing to watch`).

**Over:**
- judging the first poll after a push, which sees the previous head's suite or none at all;
- an unbounded wait for checks that never come.

**Because:** GitHub registers a pushed head's check suite seconds to minutes after the push. Five polls covers that without turning a repo with no workflow into a 6 h wait. The expected-names rule stops a red table that is still partly registered from reading as a smaller red.
**Undo:** set `GRACE_POLLS` to 0 in `src/ci.rs`; `judge` then decides on the first poll.
**Review:** fyi

### AD-30002 — When does a check that stays pending become a block? · polish · 2026-10-10

**Chose:**
- After `FACTORY_CI_TIMEOUT` seconds (default 6 h, the GitHub Actions job limit) since the watch's start, the last push or the last rerun, the watch blocks with `CI still pending after {span} · ci`. The block quotes the last table.
- Each poll is a single `gh pr checks --json`. Between polls the driver sleeps in one-second steps and checks `stop_requested` at each step and before each fix round's push. `factory stop` therefore ends a watch within about a second instead of at the next `--watch` exit.

**Over:** `gh pr checks --watch`, which never returns on a stuck check and cannot see a stop; a config key.
**Because:** a queued job with no runner and a check app that never reports are both real. Six hours is the longest a GitHub-hosted job can legitimately run, and the env var follows AD-908's knob.
**Undo:** raise `DEFAULT_TIMEOUT` in `src/ci.rs`.
**Review:** fyi

### AD-30003 — What do the CI caps count once the owner has answered a CI block? · polish · 2026-10-10

**Chose:** both caps count only events with a `seq` above the last answered `· ci ·` block's `closeout_blocked` event:
- fix rounds: distinct `ci_fix_spawned.round`;
- reruns: `ci_rerun` for the same check name at the same head.

An answer to the cap block therefore buys three fresh rounds, and the block text says so.

Round numbers keep counting from the run's highest round. The fourth round's pack is `ci-fix-4-*.md`, its log is `log-ci-4.md` and its AD seed is `8401`, so they never collide with round 1's. `ci_red.round` is the number of rounds since the answer.

**Over:** counting every round the run ever made. Under that rule, an answered cap block re-blocks on the next red and the owner's "fixed it by hand" buys nothing.
**Because:** `autonomy.md:77` caps the driver's own attempts between owner decisions. Once the owner has looked and answered, the driver gets another bounded try, never an unbounded one.
**Undo:** set `window` to 0 in `closeout::watch_ci`.
**Review:** fyi

### AD-30004 — What does each closeout block's `proceed` answer do? · polish · 2026-10-10

**Chose:** an answer is `proceed` when its first word is `proceed`, ignoring an option number (`2.`, `2)`), backticks and case. The driver reads it from the block entry's last `**Resolution (…):**` line. Each block that offers `proceed` gives it one effect:

| Block | Effect of `proceed` |
|---|---|
| `Two polish workers died · polish` | The driver ships what is committed, with no third polish worker. |
| `gh pr create refused · pr` | The PR is the owner's. The run finishes with no create and no CI watch. |
| `gh pr edit refused · pr` | The body is left as it is and CI is watched. |
| `gh pr checks unreadable`, `gh pr view unreadable`, `flaky check … red again after its rerun`, `CI did not run to a verdict`, `CI still pending after` (all `· ci ·`) | The checks are the owner's. The run finishes with no further watch and no `ci_green`. |

Any other answer, and any answer to a block with no `proceed` option, re-runs the step that blocked. For the CI cap block that means three fresh fix rounds (AD-30003).

**Over:**
- free-text intent parsing;
- keeping the texts' `proceed` option while no code read it, which is what the review found: every answer re-ran the step.

**Because:** each block now promises an effect the code delivers, and `tests/closeout.rs` answers four of them and checks what follows. A first-word rule is something the owner can type without guessing.
**Undo:** drop the `is_proceed` checks in `closeout::run`, `open_pr` and `watch_ci`, and the `proceed` options from the block texts.
**Review:** decide

### AD-30005 — What makes a red check flaky, and how often is it rerun? · polish · 2026-10-10

**Chose:**
- Each failed row's run is the run id in its job link, read with `gh run view <id> --json`.
- The red is flaky when every failed row has a run and, for each of those runs, `gh run list --commit <headSha> --workflow <workflowName> --limit 100` holds another run that concluded `success`: the same workflow passed at the same head.
- Each such run is rerun once with `--failed`, and a `ci_rerun` is emitted per check name.
- A check name red again at the same head after its rerun blocks (`flaky check … red again after its rerun`). A different check that turns flaky at that head still gets its own rerun.
- Several red workflows in one poll make one fix round. The check is the joined names (`test, lint`), and the log is one `--- run <id> (`<workflow>`), failed: …` section per run.

**Over:** AD-902's rule, which is any `success` in `gh run list --branch <b> --limit 20` at the red's head. It counted another workflow's green as this check passing and lost runs past 20. It also took the first `failure` in the list as the run to rerun or log, even when that run belonged to the other red workflow.
**Because:** "flaky" means the same check passed on the same tree. Only the same workflow at the same sha says that, and only the check's own link names its run.
**Undo:** `ci::passed_once` and `ci::runs_at` in `src/ci.rs`.
**Review:** fyi

### AD-30006 — What happens when `gh` refuses a call the red verdict needs? · polish · 2026-10-10

**Chose:** a refused `gh run rerun` blocks (`gh run rerun <id> refused: … · ci`), and so does a refused `gh run view` or `gh run list` (`gh run list refused: …`). None of them falls into a fix round, and no failure is swallowed as an empty list.
**Over:** AD-902's "a refused rerun is treated as a fix round", and the old `runs().unwrap_or_default()`.
**Because:** a fix worker given a flaky check, or a check whose run it cannot name, changes code that is not broken. A `gh` that refuses is a login or permission problem for the owner, as the other `gh` refusals already are.
**Undo:** map the `Err` arms in `closeout::on_red` back to `Next::Fix`.
**Review:** fyi

### AD-30007 — Which check states are infrastructure, and do they wait for the rest of the table? · polish · 2026-10-10

**Chose:**
- `TIMED_OUT`, `STARTUP_FAILURE`, `CANCELLED`, `ACTION_REQUIRED` and `STALE` (or the `cancel` bucket) are infrastructure. Any one of them blocks at once with `CI did not run to a verdict: <checks> · ci`, even while other checks are still running, and spawns no fix worker.
- The one exception is a run that is queued or in progress again (the owner reran it). The watch then waits for it.

**Over:**
- skipping these rows, as `failed_run` did by matching only `failure`;
- waiting for the whole table to settle first.

**Because:** `autonomy.md:79` says infrastructure blocks at once. A timeout or a runner that never started is not something a fix worker can see in a diff.
**Undo:** move the states to `Class::Fail` in `ci::class`.
**Review:** fyi

### AD-30008 — How does the watch tell a rerun's old rows from its new ones? · polish · 2026-10-10

**Chose:** the job links the driver just reran are kept in memory (`Watch::stale`). A row with one of those links reads as "the rerun of `x` has not registered". After a driver restart that memory is gone, and the run's own `status` (`queued` or `in_progress`, read with `gh run view`) makes the watch wait instead.
**Over:** a `links` field on `ci_rerun`, which would make the set survive a restart. `src/events.rs` is outside this fix's files, so that field is listed for the orchestrator.
**Because:** the in-flight status covers the restart case for everything except a rerun that has not yet left `completed`, a window of seconds.
**Undo:** drop `Watch::stale`; `ci::judge` then reads the old attempt's rows as a fresh red, and the rerun counter blocks.
**Review:** no

### AD-30009 — Where do ship and push check the branch? · polish · 2026-10-10

**Chose:** `closeout::branch_guard` runs before `docs ship` (phase `polish`) and before every `git push` (phase `pr`), including each fix round's push. It blocks with `{ship|push} stopped: {problem}` when any of these holds:
- the run names no feature branch;
- the run's branch is the default branch;
- HEAD is detached;
- HEAD is on another branch.

`gh pr create` gains `--head <branch>`. HEAD is read with `git symbolic-ref --quiet --short HEAD`.

**Over:** checking once at the start of the closeout, which misses a HEAD that moves while the run is parked on a block (the regression test does exactly that).
**Because:** the driver must never push the default branch or one the run does not own. A guard at each push is the only place the check cannot go stale.
**Undo:** remove the two `branch_guard` calls.
**Review:** fyi

### AD-30010 — When is the ship done on a re-entry? · polish · 2026-10-10

**Chose:** the ship is done when HEAD's `prd.md` says `status: shipped` (`git show HEAD:<dir>/prd.md`) or a `shipped` event exists. A board that says `shipped` while HEAD does not is a ship killed before its commit. The ship runs again: `docs ship` is a no-op on a shipped PRD, and the retro, render and `docs: ship PRD-NNN` commit follow.
**Over:** the board's `status` alone (`already_shipped`), which skipped the ship commit and left `prd.md` staged.
**Because:** the commit is the half that can be lost, so HEAD is the source of truth for it.
**Undo:** set `ship_committed` back to `board_shipped` in `closeout::run`.
**Review:** no

### AD-40001 — How does `def` find the definition shapes its regexes skipped? · polish · 2026-10-10

**Chose:** keep the regex prefilter PRD §14 names and widen each table (`src/ctx/def.rs` `Table::patterns`): PHP takes `abstract`, `final`, `readonly` and the visibility keywords in any order; TypeScript takes `export`, `default`, `declare` and `async` before `function`, `get`/`set`/`override`/`abstract` and a generic `<T>` before a member, and a member whose parameters run onto the next line (`NAME(` with no `)` on the line); Rust takes `const`, `async`, `unsafe`, `default` and `extern "C"` before the keyword, plus `union` and `static mut`; Go adds a third regex, a name then a type at the line start, for a type inside a `type ( … )` group. The tree still decides: a struct field that matches the Go regex is counted as "no definition node", and an `impl` block never confirms a hit. Each shape the finding lists is pinned in `tests/ctx.rs` `def_finds_every_shape_outline_lists`, which also checks `outline` lists it at the same line.
**Over:** tagging every file that names the symbol and taking each definition tag of that name, which agrees with `outline` by construction but costs 5–8× on a large checkout (Traps); and widening only the shapes listed, which leaves `final public` and `public final` to differ.
**Because:** the prefilter is the engine PRD §14 describes and it keeps `def` at the measured 0.1 s; a missed shape is a missing alternative in one regex, cheap to add when found.
**Undo:** `Table::patterns` and the Go default kind in `Table::keywords` (`type`, for the group line, was `func`).
**Review:** no

### AD-40002 — Which `<script>` blocks of a Vue file are read? · polish · 2026-10-10

**Chose:** every complete `<script` block, in file order (`vue::script_blocks`). `Unit::all` returns one unit per block, `lang::definitions` merges their tags in source order, and `lang::Parsed` checks a byte against the block that holds it, so `outline`, `def` and the `refs` comment check read a component with both `<script>` and `<script setup>` at its real line numbers. A block that never closes ends the list. This supersedes AD-1208's clause "the Vue block is the first `<script` followed by whitespace or `>`"; the rest of AD-1208 (the quoted `>` rule, no `lang`, template hits kept) stands.
**Over:** the first block only (AD-1208), which hid the `<script setup>` half of a component that also has a plain `<script>`.
**Because:** Vue allows both blocks in one file and puts the composition code in the second.
**Undo:** `src/ctx/vue.rs`, `Unit::all`, `lang::definitions` and `lang::Parsed`.
**Review:** no

### AD-40003 — What is a test path, segment by segment? · polish · 2026-10-10

**Chose:** refining AD-1106 (still the path relative to the search root): a directory segment equal to `test`, `tests`, `spec`, `specs` or `__tests__`, or a file name ending `_test.go` or holding `.test.` or `.spec.` (`refs::is_test`). `app/Services/Inspector.php`, `src/respect.rs`, `docs/specification/…`, `latest/…` and `contest/…` are no longer test files.
**Over:** AD-1106's plain substring checks, which matched `spec` and `test/` inside other words.
**Because:** the slice's pattern names directories and file-name shapes; a substring inside a word is not one.
**Undo:** one function in `src/ctx/refs.rs`.
**Review:** no

### AD-40004 — Are `build/`, `dist/` and `storage/` skipped at any depth? · polish · 2026-10-10

**Chose:** only directly under the search root (`walk::ROOT_DIRS`); `vendor/`, `node_modules/` and `target/` stay skipped at any depth (`walk::VENDOR_DIRS`). `--vendor` lifts both. The walk test now plants `build/`, `storage/` and a nested `php/vendor/` and `php/node_modules/` as skipped, and `php/build/`, `app/storage/` and `web/dist/` as walked.
**Over:** slice 11's list at any depth, whose test planted `php/build/` as skipped.
**Because:** a nested `build/` or `storage/` is as often source as output (`src/build/`, `app/storage/`), and a `.gitignore` already hides the real output trees; a dependency tree is never source.
**Undo:** fold `ROOT_DIRS` back into `VENDOR_DIRS` and restore the planted `php/build/Gen.php`.
**Review:** decide — deviates from slice 11's walk test, which planted `php/build/` as skipped

### AD-40005 — What does `wait` do with a question a dead cockpit was shown but never relayed? · polish · 2026-10-10

**Chose:** print it again. Every `wait` call first prints the event that opened each `BLOCK-n`/`ASK-n` still open and the `run_parked` the run still waits on (`cockpit::owed`), cursor or not, in `seq` order: before the fresh events when there are some, or alone and at once, exit 0, cursor untouched, when there are none. A dead driver still exits 4 first, and a run that is over owes nothing. The skill's BLOCK/ASK and `parked:` bullets end the session when the owner will not answer now, instead of waiting again into a loop, and §4 says `wait` repeats what is still open.
**Over:** the cursor alone (slice 10 task 1, and its acceptance "a second `wait --timeout 1` prints `still running: … BLOCK-101 open` and exits 3"), under which a cockpit that died between `wait` printing a block and the owner seeing it reattaches to `still running` forever; also over a cursor that advances only on `factory answer`, which leaves a park, never answered, unshown, and a separate `--open` flag the skill must remember.
**Because:** PRD §9 says a dead cockpit reattaches by calling `wait`; the cursor only knows what was printed, not what reached the owner, and an open question is still owed whoever saw it.
**Undo:** drop `cockpit::owed` from `src/cli/wait.rs` and restore the exit-3 step in `tests/cockpit.rs` and the skill's "Wait again either way."
**Review:** decide — deviates from slice 10's acceptance (second `wait` exits 3 on an open block) and amends AD-1004's "relays the park once"

### AD-40006 — Which shell reading picks the `grep`/`sed`/`cat` kinds? · polish · 2026-10-10

**Chose:** `metrics::first_word` is now `shell::first_command`'s first word with its directory dropped, the reading `ctx_kind` and the hook already use: quotes, glued operators (`cd /r&&sed …`) and quoted prefixes (`FOO="a b" grep …`) read as a shell would.
**Over:** AD-1309's "the other kinds keep slice 08's whitespace reading", which made `FOO="a b" grep x .` a `run` and kept two readings of one command line.
**Because:** one command line should get one first word; the research's counts compare on the commands both readings agree on, which are nearly all of them.
**Undo:** restore the whitespace loop and `is_assignment` in `src/metrics.rs`.
**Review:** fyi — supersedes AD-1309's whitespace clause; `src/shell.rs`'s module doc still describes the old split and is not in this fixer's files

### AD-40007 — What does a threshold line say with nothing to compare? · polish · 2026-10-10

**Chose:** `n/a`, in the text and as the JSON string `"n/a"`: the follow-up line when the run made no ctx call, the `ctx_share` and `repeat_rate` lines when it made no lookup. A run with lookups and no ctx call still reads `ctx_share … under`: that is a measurement.
**Over:** JSON `null` for the absent value, which would change the field's type for readers that switch on the three words.
**Because:** `ok` on a 0-of-0 share read as a pass for the reversal rule; the string keeps every reader's field a string.
**Undo:** `Thresholds::of` in `src/cli/report.rs` back to plain `f64`s.
**Review:** no

### AD-40008 — Does `/factory-deliver NNN` authorize its one push as `/deliver NNN` does? · polish · 2026-10-10

**Chose:** no change: the skill's §3 "Push once." stays, on the reading that the owner typing `/factory-deliver NNN` is the same authorization as `/deliver NNN` — the same feature, the same `feat/*` branch, the same review interview it copies — and the global `pre-push` guard still refuses a force-push or a push to the default branch.
**Over:** "ask before pushing" in §3, which would stop an unattended review at its last step; and naming `/factory-deliver` in the owner's global instructions, which this run cannot edit.
**Because:** the global rule names `/deliver NNN` only, so whether a push from this skill is asked for is the owner's call, not the skill's; leaving the text as it is keeps the behaviour the slice wrote until he says.
**Undo:** replace "Push once." with "Ask the owner, then push once." in `skills/factory-deliver/SKILL.md`.
**Review:** decide — the owner's push exception names `/deliver NNN`; extend it to `/factory-deliver NNN`, or make the skill ask?

### AD-50001 — Who handles a HEAD off the run's branch once the closeout is due? · polish · 2026-10-10

**Chose:** the closeout. The tick's branch check (AD-20005) parks the run as before while slices are left, a worker is in flight, or the board is not all done and checked. Once the closeout owns the run (`scheduler::closeout_owns`), the tick skips that check and the closeout's `branch_guard` runs at its entry, before polish, ship, push and the CI watch, and again before each CI fix round. Off the branch, it writes a `BLOCK-n` in the closeout's phase. Option 1 is "put HEAD back on the feature branch and answer this block". Both holds stay: `commit::hold_branch` keeps every commit on the branch, and `ci::branch_problem` keeps every push there.
**Over:** the tick's silent `waiting-owner` park for every phase. Under that park, `the_push_refuses_a_head_off_the_feature_branch_and_the_pr_names_its_head` waited for a BLOCK-2 that never came. The other options were dropping the closeout's guard, or making the tick write the block.
**Because:** in the closeout, the owner is already answering blocks, and a park with no block and no ask says nothing in `factory status` about what to do. Before the closeout, the self-clearing park keeps lanes of slices moving without an answer step. Each phase has one owner and one outcome. The cost: if HEAD moves while a polish or CI fix worker is running, `hold_branch` still makes that worker's commit error, and the run fails (AD-20005's cost, unchanged).
**Undo:** drop the `closeout_owns` condition on the tick's `branch_refusal` check in `scheduler::drive_inner`, and the entry `branch_guard` call in `closeout::run`.
**Review:** fyi

### AD-50002 — Whose are the files a closeout worker changed without naming them? · polish · 2026-10-10

**Chose:** the same rule as AD-20007, with `alone: true`, since a polish or CI fix worker always runs alone. Each worker's spawn baseline is saved under `runs/<id>/untracked-before/closeout-polish.json` or `closeout-ci-fix-<round>.json`. A tracked, unprotected file it changed unnamed goes in its commit. A new untracked file is listed in a new `closeout_leftovers {role, files}` event and in the driver log, and stays in the tree. A resumed driver settling an orphan loads the saved baseline. A file listed once is never swept later.
**Over:** leaving closeout commits to the named paths only, which strands a formatter's or a `sed -i` edit outside every commit; or reusing `untracked_leftovers`, whose `slice` field a closeout worker has no value for.
**Because:** the slice and checkpoint workers already work this way (a354d0d). The polish and CI fix workers were the two left out. Both run alone, so a tracked change under them is their work.
**Undo:** pass `alone: false` to `scheduler::unnamed` in `commit_polish` and `commit_ci_fix`, or drop the `unnamed` calls and `EventKind::CloseoutLeftovers`.
**Review:** fyi

### AD-50003 — How does the CI fix contract name the round? · polish · 2026-10-10

**Chose:** "fix round {round}", with no cap in the sentence. The asset's first line is edited in place, so it is still 22 lines. `a_red_check_gets_one_fix_round_then_green` now asserts "fix round 1, unattended".
**Over:** "fix round {round} of three".
**Because:** the cap is three rounds since the owner last answered (autonomy.md:77), so after an answer the round number passes three and the old line read "fix round 4 of three". The cap is the driver's to enforce, not the worker's to know.
**Undo:** restore the "of three" wording on line 1 of `assets/contract-ci-fix.md` and in the test.
**Review:** no

### AD-50004 — How does `factory status` round the budget use? · polish · 2026-10-10

**Chose:** up: `ceil` of the unrounded weighted sum (`quota::weighted_sum`, now `pub`).
**Over:** the rounded `quota::weighted`, which shows 1.4 against a budget of 1 as "1/1"; or a fractional formatter.
**Because:** the budget parks a run when the sum is greater than the budget. For a whole-number budget, `ceil(sum) > budget` holds exactly when that is true, so the line shows "over" exactly when the run is over. Above 1k, the change is invisible. `run_parked.weighted` and `report` keep the rounded value, and `STUB_WEIGHTED` in tests/quota.rs holds.
**Undo:** use `quota::weighted` again in `cli::status::board`.
**Review:** no

### AD-60001 — Where do the hook guard's wrapper, shell and doubt cases live? · polish · 2026-10-10

**Chose:** the tables in `tests/hook.rs`. The unit test `cli::hook::tests::the_guard_reads_wrappers_shells_and_doubt` is gone. Its 30 cases that were not already in a table (22 denied, 8 allowed) now sit in `every_segment_and_every_wrapper_is_read`, `doubt_is_denied_when_it_names_gh_or_push` and `the_rest_gets_no_decision`. Its five other cases ("gh --version", "env -u CLAUDECODE git push", "git stash push -m wip", "echo gh push", "cargo test --test hook") were already in a table word for word, so they were dropped.
**Over:** keeping both lists, which kept the same never-list in two places with no case unique to the unit test.
**Because:** each case now goes through `guard` and the full `hook_with` decision, where the unit test checked only `guard`. That is the same coverage plus the decision and its reason. The unit test's own doc comment already said the never-list cases belong in `tests/hook.rs`. `tests/` is outside the source scan in `the_driver_never_merges_or_readies_a_pr`, so the cases stay safe to name there.
**Undo:** restore the unit test from the parent of this commit with `git show`, and drop the added rows from the three tables.
**Review:** no

### AD-60002 — Is `allow_is_no_decision_and_deny_is_one_line` still needed? · polish · 2026-10-10

**Chose:** no, so it is removed from `src/cli/hook.rs`.
**Over:** keeping it next to the integration tests.
**Because:** Ln7 is covered twice over. `decide` in `tests/hook.rs` asserts empty stdout for every case in `the_rest_gets_no_decision`. For every denied case, it asserts exactly one JSON line with `permissionDecision` "deny" and reason `NEVER_HINT`, and "gh --version" is in `the_second_pass_cases_are_denied`. `tests/cli.rs::hook_denies_gh_and_git_push_and_fails_closed` checks the same through the binary.
**Undo:** restore the test from the parent of this commit.
**Review:** no

### AD-60003 — Is the Vue unit test in `ctx::lang` still needed? · polish · 2026-10-10

**Chose:** no, so `a_vue_file_has_one_unit_per_script_block` is removed from `src/ctx/lang.rs`.
**Over:** keeping it for C4 alongside the integration test.
**Because:** `tests/ctx.rs::every_vue_script_block_is_read` covers the same fix through the same code: `Unit::all` and `definitions` for `outline`, and `Parsed::of` for `def` and `refs`. It checks both script blocks at their file line numbers (2 and 13), a clean parse in the exact `def` output, a comment in the second block dropped by `refs`, and a call in the template between the blocks kept as code.
**Undo:** restore the test from the parent of this commit.
**Review:** no

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

### CP-5 — after slice-10 · 2026-10-10

**Covers:** slices 09, 10
**Gate:** 4 runs, 1 red, ~1.7 min (101.8 s in all: 32.9 s green on HEAD 9248211 plus the merge, 0.2 s red, 33.8 s and 34.9 s green over the final tree; warm target dir; 113 tests in 14 binaries; then the five live-driver binaries ten times, 370 test executions, 0 failures) — caught: nothing in the slice commits (69913e8, 9248211 green as committed); the red run was this checkpoint's own transcript echo failing `cargo fmt --check` in `tests/cockpit.rs` and `tests/run.rs`. No race tripped; the phase-then-event order of `scheduler::finish`/`mark_stopped` (one append wide since AD-1005) stays latent, see the gate file. `docs log merge 001` folded `log-slice-09.md` (AD-901–911, Traps) and `log-slice-10.md` (AD-1001–1007, Traps) in with no number clash.
**Prove:** FR-6 (`tests/closeout.rs::a_red_check_gets_one_fix_round_then_green`: stub `gh.log` `pr view` probe, one `pr create --draft --base main`, `pr checks` red, `run list`, `run view 1000 --log-failed`, `pr checks` green; one `fix` worker, `fix: CI test`, `ci_cycles: 1`; and `::a_flaky_check_is_rerun_once_then_green`: the same with `run rerun 1000 --failed` and no fix worker, `ci_cycles: 0`; both `status: shipped`, `docs: ship PRD-001`, 12 of 12 retro keys filled, `score:` empty), FR-7 machine half (`tests/cockpit.rs` wait/still-running/status/unknown id/dead driver/answer → `finished`/replay/finished status on the slice 04 parked fixture, and stop → `run_stopped` → a new run that builds 02 and 03; `tests/run.rs::stop_during_a_slice_lets_the_worker_finish_then_ends_the_run`) — evidence: `evidence/cp-5-gate.txt`, `evidence/cp-5-fr6-ci-red-one-fix-round.txt`, `evidence/cp-5-fr6-ci-flaky-one-rerun.txt`, `evidence/cp-5-fr7-wait-and-status.txt`, `evidence/cp-5-fr7-stop.txt`. Unreached: FR-7's human row (in `qa-guide.md` for the owner); FR-10's CI run on this repo's PR.
**Fixed:** `factory status` printed the gate minutes on its checkpoints line as a raw f64 (`0.00018016041666666667 min`) — now `{:.2}` like `factory report` (`src/cli/status.rs`), pinned by the needle in `tests/cockpit.rs`; the transcript echo (AD-9002) and its `cargo fmt`; all in this checkpoint's commit (`verify(001): checkpoint after slice-10`)

### CP-6 — after slice-12 · 2026-10-10

**Covers:** slices 11, 12
**Gate:** 2 runs, 0 red, ~1.2 min (73.8 s in all: 35.5 s green on HEAD 25abef4 plus the merge, 38.3 s green over the final tree with the Prove fix; warm target dir; 141 then 142 tests in 15 binaries, 0 ignored) — caught: nothing (both builders ran fmt and clippy; the slice commits 1845d9d and 25abef4 green as committed). `docs log merge 001` folded `log-slice-11.md` (AD-1101–1108, Traps; AD-1101's regraded `Review: decide` kept) and `log-slice-12.md` (AD-1201–1208) in with no number clash.
**Prove:** FR-12 pinned output (`cargo test --test ctx -- --nocapture`: 17 tests green, the 20 snapshot commands each run twice and byte-identical, the `def OtherService` docblock mention dropped, `refs planted` 10 lines + `288 more hits in 40 files …; 2 dropped (comments/strings)`, 298 = 300 − 2, `outline` definitions only; the same 17 commands driven by hand on the fixture, 34 of 34 comparisons byte-identical) and FR-12 "each verb under 1 s warm" (release build, one warm-up and three timed runs per row, on the owner's checkouts `~/Projects/ICG/icg-cloud` and `~/Projects/Prokey/arca-api`, the two repos at one level below the slices' paths, identified read-only: `def`/`refs Company` 0.050/0.041 s and `outline app/Core/Http/Controllers/Admin` 0.046 s on icg-cloud, `def`/`refs Code` 0.030/0.044 s and `outline internal/caea` 0.052 s on arca-api, medians, slowest run 0.060 s; the suggested `app/Services` is absent from icg-cloud and `internal/` holds no files directly, both timed as given and recorded) — evidence: `evidence/cp-6-gate.txt`, `evidence/cp-6-fr12-ctx-tests.txt`, `evidence/cp-6-fr12-pinned-output.txt`, `evidence/cp-6-fr12-timing.txt`. Unreached: FR-12's `ctx_share`/`repeat_rate` piece (slice 13, then the A/B run); FR-10's CI run on this repo's PR.
**Fixed:** `def` on icg-cloud located an attributed definition at its attribute line and printed the attribute as its signature (`Company.php:37  class  #[Fillable([...])]`, where `outline` says 38, `class Company extends Model …`): the header is now the name's line and an attributed node's body its whole range (AD-9003, `decide`), in `src/ctx/def.rs::node_def`, pinned by `tests/ctx.rs::def_locates_an_attributed_definition_at_its_name`; in this checkpoint's commit (`verify(001): checkpoint after slice-12`)

### CP-7 — after slice-13 · 2026-10-10

**Covers:** slice 13
**Gate:** 1 run, 0 red, ~0.6 min (35.5 s green on HEAD bf172e2 plus the merge; warm target dir; 155 tests in 16 binaries, 0 ignored, 0 warnings) — caught: nothing (the slice commit bf172e2 was green as committed, fmt and clippy included; the checkpoint changed no code, so there was no second run). `docs log merge 001` folded `log-slice-13.md` (AD-1301–1309, Traps) in with no number clash.
**Prove:** FR-12 tail and FR-8 tail (`factory report --per-worker` on a kept fixture run (`FACTORY_KEEP_FIXTURE=1`, `tests/metrics.rs::ctx_calls_are_labelled_and_the_report_reads_the_reversal_rule`): `followup_reads: 1 of 2 ctx calls (50%)`, `pack_doc_read_chars: 14124` (the sum of the seven held reads), `ctx_share` 0.200, per worker `ctx_share` and `repeat_rate` (worker 1 0.400/0.000, the rest 0.000/0.000), and the three lines `followup threshold 30%: over`, `ctx_share threshold 10%: ok`, `repeat_rate vs 26.7%: lower`; `--json` with `followup_reads`, `thresholds` and `per_worker`); task 1's hook transcript (12 PreToolUse inputs to `factory hook` against the kept slice pack: 4 repo greps and `rg` denied with the `factory ctx refs` hint, a whole `Read` and a `cat` of the held slice denied naming `# Slice`, and a ranged read, `prd.md`, a one-file grep, a piped grep, `cargo test` and malformed stdin allowed, all exit 0); the real one-turn worker (`claude -p "run: grep -rn foo ."`, 2.1.280, `--model haiku`, `--output-format stream-json --verbose --permission-mode bypassPermissions --session-id --settings <driver-written file> --max-budget-usd 0.25`, with no `--max-turns` because it is absent from `--help`, `CLAUDE*` unset and `FACTORY_PACK` set, in a deleted `mktemp -d` copy of `tests/fixture/`: `permission_denials` holds the grep and its retry, and the tool results carry the hook's reason; 4 turns, $0.0621). The run also showed that the hint is followed, since the model ran `factory ctx refs foo`, and that it then fails where `factory` is not installed: exit 127, because the hook resolves by absolute path but the hint names the bare `factory`. qa-guide.md already requires an installed `factory`, so this is recorded, not fixed. Evidence: `evidence/cp-7-gate.txt`, `evidence/cp-7-fr12-fr8-report-per-worker.txt`, `evidence/cp-7-fr2-fr12-hook-transcript.txt`, `evidence/cp-7-fr2-fr12-real-worker.txt`, `recordings/claude-p-hook-grep-deny-haiku-2.1.280.jsonl`. Unreached: AD-1304's ranged slice read before an `Edit` on a real worker, because the fixed prompt makes no `Edit` (now a check in `qa-guide.md` FR-11 step 6); the A/B figures for `ctx_share` and `repeat_rate` (the owner's rows); FR-7 and FR-11 (human rows); FR-10's CI run on this repo's PR.
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
