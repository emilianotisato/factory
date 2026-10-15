# Slice 03 — Worker spawn per role

**Source**: [`prd.md`](prd.md) → §3 FR-1, FR-2. **Depends on**: — (independent of every other slice: only this one touches `src/worker.rs`. FR-1's Autonomy wording, "Several tool calls side by side", is slice 04's, which owns `assets/autonomy.md`)
**Created**: 2026-10-15

**Progress**: _(written as it lands — what was built, the gate numbers, what a review caught)_

**Slice**: Each worker's `claude` command and settings file follow its role. Build, fix and merge workers get four tools, no MCP servers and no slash commands; every role gets the default output style; build, fix and merge also run without the owner's auto memory. Checkpoint and polish keep today's tools and memory. This is the start-context cut, from about 26k to about 7k tokens in the probe.

## Current code / environment

- **One command for every role**: `Worker::spawn()` (`src/worker.rs:172`) takes `role = job.role()` (`:181`) and builds the command at `:206-221`: `-p` and the prompt first, then `--output-format stream-json --verbose`, `--permission-mode bypassPermissions`, `--append-system-prompt-file <pack>`, `--session-id`, `--model`, `--effort`, `--settings <file>`. Nothing limits tools, MCP or skills.
- **One settings shape for every role**: `settings(exe)` (`:92-102`) takes no role and returns `permissions.deny` and the `Bash|Read` PreToolUse hook with `"timeout": 5`. Its only caller writes it per session at `:196-199`, to `settings_path(run_dir, session_id)` (`:104`).
- **The roles**: `Job::role()` (`:119-127`) maps `Build` to `build`, `Merge` to `merge`, `Fix` and `CiFix` both to `fix`, `Checkpoint` to `checkpoint`, `Polish` to `polish`.
- **The probe's flags** (PRD §3 FR-1, FR-2, 2026-10-15): `--strict-mcp-config --disable-slash-commands --tools Bash,Read,Edit,Write` took a session from 25,944 to 9,300 tokens, and `"outputStyle": "default"` with `"autoMemoryEnabled": false` in the settings file took it to 6,762. The `--settings` hook still fired in both.
- **The stub's flag parser**: `parseFlags` (`tests/stubs/claude:589-599`) pairs any `-flag` with the next argv word that does not start with `-`. A boolean flag must be followed by another flag or come last, or it swallows the next word. `--tools` takes one comma-joined word.
- **What the stub records**: each call's JSON holds `argv` (`tests/stubs/claude:115`) and `settings`, the `--settings` path (`:128`). A pack's file name tells its role: `slice-NN-n.md`, `merge-NN-n.md`, `fix-N-n.md`, `checkpoint-N-n.md`, `polish-n.md`, `ci-fix-N-n.md` (the regex at `:96`).
- **Tests that read argv and settings today**: `check_worker_argv` (`tests/loop.rs:165`) for a build worker, and `check_settings` (`tests/loop.rs:146-163`), which reads only `permissions.deny` and the hook, so new keys do not break it. `tests/worker.rs` has a `flag()` helper (`:38`). `tests/metrics.rs:224-237` loops over every call checking `--settings`.
- **Where each role runs in a test**: build in `tests/loop.rs`; checkpoint and fix in `red_red_green_runs_two_fix_workers_and_three_gates_before_the_cp` (`tests/checkpoints.rs:883`); merge in `a_conflicting_landing_spawns_a_merge_worker_in_the_slices_lane` (`tests/merge.rs:139`); polish and CI fix in `a_red_check_gets_one_fix_round_then_green` (`tests/closeout.rs:1042`).
- **Deliberate non-change**: *prd §7: "Checkpoint and polish tool sets. They use Skill, Agent and an MCP server (§3 FR-1)."*
- **Deliberate non-change**: *prd §7: "Not handled: the `dontAsk` permission mode. Accepted 2026-10-15: it drops the bypass-mode reminder …, but a probe in a temp git repo showed it denies a Write into `.claude/skills/verify-x/` …"* Keep `--permission-mode bypassPermissions`.
- **Deliberate non-change**: *§3 FR-2 Rejected: "`--setting-sources project,local`: in the probe it dropped only the output style, while the global `~/.claude/CLAUDE.md` and MEMORY.md still loaded."*

## Tasks

### 1. Build, fix and merge workers get four tools — high — blocked by: —
**Source**: §3 FR-1: "The three flags go on the command when `Job::role()` (`src/worker.rs:119-127`) is `build`, `fix` (a gate fix and a CI fix) or `merge`. Checkpoint and polish keep today's command."

`src/worker.rs:206-221`: for `build`, `fix` and `merge`, add `--strict-mcp-config`, `--disable-slash-commands` and `--tools Bash,Read,Edit,Write`. Checkpoint and polish get none of the three.

**The trap:** the stub's `parseFlags` (`tests/stubs/claude:589-599`) pairs a boolean flag with the next word that does not start with `-`. Put each boolean before another flag, or the stub records a wrong argv and the tests read it wrong.

**Acceptance:** stub-run tests: the recorded argv of a build, a fix, a CI fix and a merge worker each holds `--tools Bash,Read,Edit,Write`, `--strict-mcp-config` and `--disable-slash-commands`; a checkpoint's and a polish worker's argv hold none of the three.

### 2. Each role gets its own settings — high — blocked by: —
**Source**: §3 FR-2: "Every role's settings file holds `"outputStyle": "default"`. Build, fix and merge also hold `"autoMemoryEnabled": false`; checkpoint and polish keep the owner's memory, because they judge what a project's gotchas mean."

`src/worker.rs:92-102`: `settings()` takes the role. Every role gets `"outputStyle": "default"`; `build`, `fix` and `merge` also get `"autoMemoryEnabled": false`. The deny rules and the hook stay as they are.

**Acceptance:** stub-run tests: the settings file each worker's argv names holds `outputStyle` `default` for all five roles, and `autoMemoryEnabled` `false` for build, fix and merge only; checkpoint and polish have no `autoMemoryEnabled` key. `check_settings` still passes on all five.

### 3. Verify — medium — blocked by: 1, 2
`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 4. Prove — at the checkpoint — blocked by: 3
`prd.md` § QA plan rows, driven below the UI:
- **FR-1**: run the stub runs that spawn a build, fix, merge, checkpoint and polish worker; read each recorded argv; keep the transcript.
- **FR-2**: the same runs; read the settings file each argv names; keep the transcript.

**Acceptance:** after the checkpoint, each row has its transcript in this slice's **Progress**.
