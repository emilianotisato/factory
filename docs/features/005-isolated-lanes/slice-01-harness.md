# Slice 01 — Harness for lanes

**Source**: [`prd.md`](prd.md) → §14 QA plan (Harness: "It needs a harness slice, first in the order"), serving the acceptance rows of §5 FR-3, §6 FR-4 and §7 FR-5. **Depends on**: nothing. It runs beside 02, 03 and 04; slice 07 is the first to spawn a worker in a lane, and it needs every piece here.
**Created**: 2026-10-14

**Progress**: built 2026-10-14. `gate.sh` finds `scenario/` next to the git common dir (falls back to its own dir outside a repo) and logs `<epoch> <kind> <exit> <pwd -P> <head|->`; `GateRun` gains `pwd` and `head`, parsed with the head split off the right so a pwd may hold a space. Both lane setups append through the common dir. The stub takes `sh` on slice, fix, polish and merge (one Bash call, exit 65 with stderr on failure), and a `merge` role (first heading `# Conflict`, AD-2) that never writes a slice status; the slice-file lookup became `findSlice()`, shared by slice and merge. New tests: `gate_sh_reads_the_main_trees_scenario_from_a_worktree` (tests/harness.rs), `stub_claude_sh_edits_through_one_bash_call_and_no_named_tool`, `stub_claude_a_failing_sh_step_exits_65`, `stub_claude_merge_role_resolves_or_blocks_and_leaves_the_status` (tests/stubs.rs).
Inner loop: 3 runs, 2 red, ~4 min. Run 1 caught a closure typed for one return in `gate_log`, and the lane setups failing because `src/lanes.rs` trims a trailing `"` off `setup:` (note in the log; the path now ends unquoted). Run 3: clippy and all 33 test binaries green; `cargo fmt --check` red only on `src/lanes.rs` and `src/docs/preflight.rs`, which slices running beside this one were editing; this slice's files pass `rustfmt --check`.
Deviation: an early plain `cargo fmt` here may have reformatted sibling slices' files mid-edit (formatting only).
Prove (CP-1), harness tests: the worktree gate test and the three stub tests (`sh`, failing `sh`, merge role) green: `evidence/cp-1-slice-01-harness.txt`.
Prove (CP-1), gate from a worktree: a scripted red run from `lane-1` exits 1, the lane has no `scenario/`, and the main tree's `gate.log` records the lane's `pwd` and HEAD: `evidence/cp-1-slice-01-gate-from-worktree.txt`.

**Slice**: After this slice the CLI test harness works from a git worktree of its fixture, not only from the fixture itself. `gate.sh` and the lane setups find the gitignored `scenario/` through the git common dir, so a scripted red gate stays red in a lane. Every `gate.log` line also records the directory the gate ran in and the HEAD it saw, which FR-5's acceptance reads. The stub `claude` gains a `merge` role (a pack whose first section is `# Conflict`) and an `sh` step key that edits files through one Bash command and no tool call naming them, which FR-3's Bash-only edits and FR-4's conflict and resolution need. Nothing in the driver changes: today's runs still spawn every worker in the fixture's main tree, and every test stays green unchanged except the ones that parse `gate.log`.

## Current code / environment

- **`scenario/` is gitignored and found next to `gate.sh`**: `tests/harness/mod.rs:218-219` writes `/scenario/` into the fixture's `.gitignore`. `tests/fixture/gate.sh:8` sets `here` to the script's own dir and reads `$here/scenario/$kind` (`:11`), then appends to `$here/scenario/gate.log` (`:22-23`). A lane worktree holds a committed `gate.sh` but no `scenario/`, so there `gate.sh` sees no script, goes green, and logs into the lane. Seeding `scenario/` does not help: FR-1 copies a seed path only when the lane lacks it (prd §3).
- **The lane setups write relative to cwd**: `tests/lanes.rs:210` and `:1136` declare `setup: echo … >> scenario/lane-setup.log`; the tests read it back from the main tree (`:291`, `:1168`, `:1181`). Slice 03 (`factory lanes up`) and slice 07 (a run) move setup into the lane, where this path is the lane's.
- **`gate.log` is parsed by one helper**: `GateRun { epoch, kind, exit }` at `tests/harness/mod.rs:57-62`, filled by `Fixture::gate_log` at `:459-483`, which panics on a line it cannot read. `script_gate` (`:411`), `script_inner` (`:416`) and `script_ci` (`:422`) write `scenario/` in the main tree, which stays the one place a test scripts outcomes.
- **The common dir is the main tree's**: in a linked worktree, `git rev-parse --path-format=absolute --git-common-dir` prints `<main tree>/.git` (git 2.55.0 here). Its parent is the fixture root from the main tree and from every lane. No env var is needed, and none would survive: slice 07 builds the worker's env from scratch (`src/worker.rs:298-322`).
- **The stub's role detection**: `tests/stubs/claude:136-145` reads the pack's sections in order: `# Gate failure` or `# CI failure` is `fix`, `# Gate transcript` is `checkpoint`, `# Branch` is `polish`, anything else is `slice`. The allowed outcomes per role are at `:146-154`; the global outcome list at `:155` refuses anything it does not know with exit 65. A merge pack also names a slice file (FR-4: "the pack holding the slice file"), so `# Conflict` must be tested before the slice fallback.
- **The stub works from its cwd**: it records `cwd` (`:114`) and puts it in the stream's `init` line (`:158`); it resolves the slice file from cwd (`:375-383`) and writes every `touch`, `stray`, `heredoc` and `commit` path relative to cwd (`:430-454`). Its board writes go through `$FACTORY_BIN docs` with cwd (`:543-561`). So it needs no change to run in a lane; only the driver's spawn does (slice 07).
- **Existing step keys that edit without naming**: `stray` (`:449-455`) appends to files with no tool call, and `git_add` (`:446-448`) runs one `git add`. Neither can `git mv`, write a symlink, or rewrite line 3 of a file, which FR-3's and FR-4's rows need.
- **The stub's own tests**: `tests/stubs.rs` drives the stub directly through `claude(fx, pack)` (`:16`) and reads its stream with `tool_use_paths` (`:38`) and `bash_commands` (`:55`). `tests/harness.rs:13` checks the fixture itself.
- **The gh stub needs nothing**: it reads `scenario/` from its own cwd (`tests/stubs/gh:63`), and the driver runs `gh` in the main tree, which FR-5 does not move.
- **Deliberate non-change**: the fixture's committed files stay as they are. `shared.txt` for FR-4's conflict is written by the tests that need it, not by `build_v3` (`tests/harness/mod.rs:188-225`), which slice 03 edits at the same time to add `[lanes]`.

## Tasks

### 1. `gate.sh` reads `scenario/` from the main tree and logs where it ran — high — blocked by: —
**Where**: `tests/fixture/gate.sh`, `tests/harness/mod.rs` (`GateRun`, `gate_log`).

**Source**: §14: "The harness slice reaches `scenario/` by an absolute path … and makes `gate.sh` record `pwd` and HEAD." §7 FR-5 acceptance: "The fixture's `gate.sh` records `pwd` and `git rev-parse HEAD`: at every checkpoint they are a lane path and the feature tip."

Resolve the scenario dir as `$(dirname "$(git rev-parse --path-format=absolute --git-common-dir)")/scenario`, falling back to today's `$here/scenario` when `git` fails (a test may run `gate.sh` outside a repo). Every line of `gate.log` becomes `<epoch> <kind> <exit> <pwd> <head>`, with `pwd -P` and `git rev-parse HEAD` (or `-` when HEAD is unborn). Update the header comment.

`GateRun` gains `pwd: PathBuf` and `head: String`; `gate_log` parses the two new fields. Callers that build a `GateRun` literal or compare whole structs change with it.

**The trap**: the fixture root under a tempdir may sit behind a symlink (macOS `/var` → `/private/var`). Record `pwd -P`, and compare against canonical paths in tests.

**Acceptance:** a test in `tests/harness.rs`: `git worktree add --detach <tmp>/wt HEAD` from the fixture, `fx.script_gate(&["red"])`, then `./gate.sh` with cwd `<tmp>/wt` exits 1. `fx.gate_log()` (read from the main tree) holds one line with `kind == "gate"`, `exit == 1`, `pwd` the canonical worktree path and `head` equal to `git rev-parse HEAD`. The same call from the main tree records the fixture's canonical root.

### 2. The lane setups write to the main tree's `scenario/` — high — blocked by: 1
**Where**: `tests/lanes.rs:210`, `:1136`.

**Source**: §14: "a scripted red gate goes green and a lane setup fails" in a lane worktree.

Both setups write through the same common-dir route as `gate.sh`: `>> "$(dirname "$(git rev-parse --path-format=absolute --git-common-dir)")/scenario/lane-setup.log"`. Today setup still runs in the main tree, so the assertions at `:291`, `:1168` and `:1181` stay exactly as they are and pass before and after.

**Acceptance:** `tests/lanes.rs` green unchanged apart from the two setup strings.

### 3. The stub's `sh` step — high — blocked by: —
**Where**: `tests/stubs/claude` (step keys, the slice, fix, polish and new merge branches), its header comment.

**Source**: §5 FR-3 acceptance: "A stub slice that edits only through Bash (`git mv`, `printf >`, an inline script rewriting a file), adds a directory, and adds a symlink to a directory". §6 FR-4 acceptance: "Slices 1 and 2 in two lanes both change line 3 of `shared.txt`."

`step.sh` is a string run as `sh -c <script>` in the stub's cwd, with the stub's env, after `touch` and before the board writes. The stream carries one `Bash` tool call whose `command` is the script, and no Read, Edit or Write naming any file the script changed. A nonzero exit fails the stub with exit 65 and the script's stderr, so a broken scenario is loud. Every role that takes `touch` takes `sh`: slice, fix, polish, and merge (task 4).

**Acceptance:** in `tests/stubs.rs`, a slice step with `"sh": "git mv src/lib.txt src/moved.txt && printf 'x\\n' > src/printed.txt && mkdir -p gen && printf 'a\\n' > gen/a.txt && ln -s gen gen-link"` leaves `src/moved.txt`, `src/printed.txt`, `gen/a.txt` and the symlink `gen-link` in the fixture; `bash_commands` holds the script once; `tool_use_paths` names none of those four paths. A step whose script exits 1 makes the stub exit 65.

### 4. The stub's `merge` role — high — blocked by: 3
**Where**: `tests/stubs/claude:136-155` (role detection and allowed outcomes), a new `// ── 3d. a merge worker` branch, its header comment.

**Source**: §6 FR-4: "It spawns a merge worker there: a new job kind and contract, the pack holding the slice file, the slice commit's message and diff stat, and the conflicted paths." "The merge worker does not touch the slice's status." "It is a BLOCK when a conflict marker remains in a file that was conflicted (the driver checks), when the worker logs a BLOCK …, or when it dies twice." Acceptance: "A stub call is recorded with role `merge` and cwd slice 2's lane. After it writes a resolution … The same, with the merge worker exiting with markers left".

A pack whose first section is `# Conflict` is a `merge` worker; test it before the slice fallback. Slice 08 builds the driver's merge pack with that heading first. Allowed outcomes: `done`, `dead`, `blocked`, `rate_limited`, `rate_limited_no_reset`.

- `done`: runs `step.sh` when given (the resolution, e.g. `printf …> shared.txt && git add shared.txt`), then `STATUS: done` and an `INNER:` line, as the fix branch does (`:212-240`). With no `sh` it changes nothing, so the conflict markers stay: that is the "markers left" case.
- `blocked`: one `block` entry with `--slice` taken from the pack's slice file (the slice branch's regex, `:372`) and no `--phase`, as a build worker's block is written (`:400-411`), and a `STATUS: blocked` result. It never calls `docs set`.
- `dead`: init, exit 137. Rate limits: `rateLimited(step)` as every role.

The stub must not set the slice's status on any outcome.

**The trap**: `docs log add --phase` refuses any phase outside `PHASES` (`src/docs/log.rs:21`, checked at `:188-192`), and `merge` is not one. A merge block is a block on its slice: pass `--slice`, no phase.

**Acceptance:** in `tests/stubs.rs`, through `claude(fx, pack)` with a pack starting `# Conflict` that names slice 02's file: a `done` step with `sh` runs the script, the result starts `STATUS: done`, and `state.toml` is unchanged. A `blocked` step adds one `block` entry with `slice: 2` and leaves `slices.2.status` as it was. An outcome `checkpoint` on a merge pack exits 65 naming the role.

### 5. Verify — medium — blocked by: 1, 2, 3, 4
`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 6. Prove — at the checkpoint — blocked by: 5
Rows from [`prd.md`](prd.md) §14 this slice proves, below the UI: none of its own. §14 names this slice as the harness the FR rows run on; its proof is that the rows of slices 07 to 10 can be driven. At CP-1, run the three tests from tasks 1, 3 and 4 and keep the transcript.

**Acceptance:** after the checkpoint, Progress holds the transcript of the worktree gate test (red from a worktree, `pwd` and `head` recorded), the `sh` stub test and the merge-role stub test.
