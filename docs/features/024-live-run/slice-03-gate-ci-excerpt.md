# Slice 03 — Gate, CI and the failure excerpt

**Source**: [`prd.md`](prd.md) → §6 FR-4, §2 (`ci/<round>.log`, `gate_failure`). **Depends on**: 01 (the `src/live/gate.rs` stub, `tests/live/gate.rs`, and the `gate_started` / `fix_empty` / `ci_waiting` / `ci_no_checks` kinds it reads). Independent of 02, 04, 05, 06, 07. Slices 08 and 09 wait on this one: 08 reuses the extractor, 09 plants secrets in the gate and CI logs this slice reads.
**Created**: 2026-11-07

**Progress**:
- **Built**: `src/excerpt.rs` (the extractor with an ANSI/OSC/charset stripper, BOM and CR handling, `{text, hit}` lines) over five scrubbed fixtures in `tests/fixture/gate-logs/`; `gate_failure` in `project.rs` (array reader, `failure_patterns` skips a bad regex) and its `check.rs` arm; the `/gate` skill line with the 0.13.1 bump, the three mod pins and a relocked `skills/bundle.lock`; `on_red` writes `ci/<round>.log` cleaned; `live` fills `gate` and `ci` (`src/live/gate.rs`, with `read_log` and `excerpt_json` for slice 05).
- **Inner loop**: 2 full runs, 0 red (1.7 and 1.4 min). Targeted runs before that: 1 compile error (a closure borrow in the budget loop) and 1 red unit test. The red test caught a wrong test, not wrong code: the project-pattern test's generic match sat right before `returned with error code`, so the always-add rule kept it.
- **Deviations**: AD-5 (`excerpt` is `[{text, hit}]`, decide), AD-6 (extractor readings: non-zero vulnerabilities, header as context), AD-7 (`ci` null only on an old layout, excerpt only while red), AD-8 (`ci_red_writes_the_whole_failure_log` is its own test so the Prove row can run it by name).
- **Prove**: left for the checkpoint: the `excerpt_finds_*`, `excerpt_ignores_zero_failed` and `excerpt_uses_project_gate_failure_first` tests, `ci_red_writes_the_whole_failure_log`, `docs_check_rejects_a_bad_gate_failure_regex`.
- **Proved (checkpoint 1):** FR-4, the four `excerpt_finds_*` tests, `ci_red_writes_the_whole_failure_log`, `docs_check_rejects_a_bad_gate_failure_regex`, 6 passed — `cp-1/fr-4-excerpt-ci-log.txt`

**Slice**: After this slice, one stack-neutral `excerpt` function finds the failing lines anywhere in a gate or CI log, trying a project's own `gate_failure` regexes first. `factory live` fills `gate` (the running or last gate, its 40-line tail and, when red, its excerpt) and `ci` (state, round, head, checks, excerpt). A red CI round's whole failure log is saved as `runs/<id>/ci/<round>.log`. `docs check` rejects a `gate_failure` regex that does not compile, and the `/gate` skill names the key. Emitting `gate_started` is slice 04's: this slice reads it from fixture events.

## Current code / environment

- **The gate log**: `gate_once` (`src/checkpoint.rs:439-…`) creates `runs/<id>/gates/<n>-<attempt>.log` (`:495-501`) and the gate writes it live; `gate_run {n, attempt, red, minutes, log}` comes only at the end (`:547`), `gate_cached {n, attempt, red, log}` on a cache hit (`:474`, `log` is the earlier run's file). `log` is relative to the run dir (`src/events.rs:128`).
- **The CI failure log**: assembled in `on_red` (`src/closeout.rs:1681-1708`) from `ci::log_failed` per failed run (`src/ci.rs:391-392`) plus unlinked rows, and returned as `Next::Fix { round, check, log }` (`:1709`). Only the pack sees it, cut to 200 lines (`src/pack.rs:79`, `:620`). Nothing saves it whole today.
- **Which round**: `rounds_done` (`src/closeout.rs:1506`) is the count since the last answer and is what `ci_red.round` stores (`:1508-1512`); `round` (`:1507`, `ever.last() + 1`) is the pack's, used for `packs/ci-fix-<round>-*.md`. prd §6: name the file by `round`, not `rounds_done`.
- **CI states**: `ci_green {head}` (`src/events.rs:198`), `ci_red {check, round, head}` (`:202`), and, from slice 01, `ci_waiting {checks}` and `ci_no_checks {minutes}`. A run with no CI event is `none`.
- **`docs/project.toml` readers**: `project::load` (`src/docs/project.rs:38-78`) has int, float, string and bool readers (`:52-62`, `gate_parallel` at `:69-73`) and no array reader. `KEYS` (`:10-19`) lists the known keys and `migrate.rs:709` drops unknown v2 index keys by it: add `gate_failure`. `Project` is also built literally at `src/docs/check.rs:94`. The `check.rs` `[docs]` arm (`:218-229`) lets unknown keys pass (`_ => true`); `kind_of` (`:239-246`) names the expected type.
- **The `/gate` skill** is `skills/gate/SKILL.md` (`.claude/skills/gate` links to it). §6 "Declare and document" (`:155-161`) is where `[docs]` keys are told; add one line for `gate_failure` there.
- **The trap of a skill edit**: `bundle_lock_matches` (`tests/skills_bundle.rs:203`) fails on any change under `skills/` until the version is bumped and the lock rewritten; `relock` refuses a changed bundle under the same version (`src/skills.rs:165-180`). So: bump `Cargo.toml` `0.13.0` → `0.13.1`, and the three pins `the_mod_carries_the_crates_version` (`tests/cockpit_mod.rs:19-40`) checks: `mods/cockpit/.claude-plugin/plugin.json:3`, `mods/cockpit/hooks/register.tsx:5` `MOD_VERSION`, `mods/cockpit/tests/cockpit.test.tsx:5` `VERSION`; then `FACTORY_BUNDLE_WRITE=1 cargo test` rewrites `skills/bundle.lock`, and `Cargo.lock` follows. `f31db8b` did the same for a skill edit (without the mod pins, which came later).
- **No ANSI stripper exists** in `src/`. Write one in the extractor's module (CSI `ESC [ … final byte`, plus OSC), and strip a leading BOM.
- **Real logs to cut fixtures from** (read once at build time, scrub, commit under `tests/fixture/gate-logs/`; tests never read the machine's runs folder): a Pest JSON `failures[]` line in `~/.local/share/factory/runs/016-20261020-0bba/gates/1-1.log`; `error TS` in `022-20261029-f9ea/gates/2-1.log`; a CRAP row in `002-20261030-f2d1/gates/1-2.log`; a rustfmt `Diff in` at the top of `003-20261013-4139/gates/1-1.log` or `002-20261030-f2d1/gates/3-1.log`. Cut to the lines that matter plus some noise either side, home paths to `/home/user`, no token shapes.
- **The CI stub**: `tests/stubs/gh:221-225` prints, per failed check, `FAIL <check>: expected 2 got 3` and `##[error]Process completed with exit code 1.` for `run view <id> --log-failed`. `a_red_check_gets_one_fix_round_then_green` (`tests/closeout.rs:1042`) drives one red round end to end.
- **Deliberate non-change**: prd §14 "Not handled: a CI block's title". CI blocks open before `on_red` fetches the log; do not touch them. The pack's 200-line cut stays.

## Tasks

### 1. The excerpt extractor — high — blocked by: —
**Source**: §6 FR-4 "The excerpt extractor, one stack-neutral function that searches the whole log", steps 1 to 5 (red SGR 31/91 lines as candidates before stripping ANSI and CR; JSON `failures[].{test,file,line,message}` from objects split on `}{`, `message` capped at 300; the generic set; the line before the last `returned with error code` or `make: *** … Error` and the last `=== N.` tier header; at most 20 lines with 2 of context, preferring the last matches), and "`gate_failure` … tried before the generic set."

`src/excerpt.rs` (new, top-level: slice 08 calls it from `checkpoint.rs`), `src/lib.rs`. Input is the bytes read (the last 1 MiB, per §3 Budgets); output the excerpt lines in log order, empty when nothing matches, each marked as a match or a context line: slice 08 titles a block from the first match, never from a context line above it. Project patterns come in as already-compiled regexes.

**Acceptance:** unit tests over the committed fixtures, named by behaviour: `excerpt_finds_a_pest_json_failure` (the test name and message, not the JSON blob), `excerpt_finds_a_tsc_error`, `excerpt_finds_a_red_crap_row` (found through its SGR-red marking), `excerpt_finds_a_rustfmt_diff_at_the_top` (a log whose last 200 lines are green noise), `excerpt_ignores_zero_failed` (`… 0 failed …` alone gives nothing), `excerpt_uses_project_gate_failure_first` (a project regex wins over a generic match elsewhere in the log).

### 2. `gate_failure` in `docs/project.toml` — high — blocked by: —
**Source**: §6 FR-4: "`gate_failure`: an optional array of regexes under `[docs]` in `docs/project.toml` … It needs an array reader in `project.rs`, a `check.rs` arm …, and a line in the `/gate` skill. A bad regex is a `docs check` error, not a crash."

`src/docs/project.rs` (field `gate_failure: Vec<String>`, `KEYS`), `src/docs/check.rs` (the arm: an array of strings that each compile, else `E-PROJECT-INVALID` naming the key and the regex error; `kind_of`; the literal at `:94`), `skills/gate/SKILL.md`, and the version bump in Current code. Callers that compile the patterns skip one that fails, so a bad regex never panics a run.

**Acceptance:** `docs_check_rejects_a_bad_gate_failure_regex` (in `tests/docs_check.rs`: `gate_failure = ["("]` is an `E-PROJECT-INVALID` error, `["FAIL:"]` passes, `gate_failure = "x"` is a type error); `project::load` reads the array; `bundle_lock_matches` and `the_mod_carries_the_crates_version` green.

### 3. Save the whole CI failure log — high — blocked by: —
**Source**: §6 FR-4: "`excerpt` comes from `ci/<round>.log`. `on_red` writes it with the BOM and ANSI stripped, numbered by the pack round, `round` at `closeout.rs:1507` (`ever.last() + 1`) … Not `rounds_done` at `:1506`"; §2 "A new run file, `ci/<round>.log`."

`src/closeout.rs` `on_red`, just before `Ok(Next::Fix { … })` (`:1709`). Write with `scheduler::write_atomic`; use the stripper from task 1.

**Acceptance:** `ci_red_writes_the_whole_failure_log`: extend `a_red_check_gets_one_fix_round_then_green` (`tests/closeout.rs:1042`) to assert `runs/<id>/ci/1.log` holds both stub lines for the failed check and no ESC byte.

### 4. The `gate` and `ci` sections — high — blocked by: 1, 3
**Source**: §6 FR-4: "`gate`: `{ n, attempt, cmd, running, startedAt, red, minutes, cached, log, bytes, tail, excerpt, fixEmpty }`. `tail` is the last 40 lines × 300 chars, ANSI stripped. `excerpt` is set when red. `running` and `startedAt` come from FR-5's `gate_started`. `fixEmpty` comes from `fix_empty`", "`ci`: `{ state: none|waiting|red|green|noChecks, round, head, checks, excerpt }`"; §3 Budgets "the last 1 MiB of the current gate log and of the newest `ci/<round>.log` (the excerpt and the tail come from that read)".

`src/live/gate.rs`, reading `docs/project.toml` of `state.repo` for `gate_failure` (missing or unreadable → none). The gate is the newest of `gate_started`, `gate_run`, `gate_cached`: `running` when a `gate_started` has no later `gate_run`/`gate_cached` for its `(n, attempt)`; `cmd` from `gate_started`; `log` the run-relative path; `bytes` its size. `fixEmpty` is true when a `fix_empty` with this `n` sits after the previous gate result. `ci.state` is the newest CI event's (`ci_green` → `green`, `ci_red` → `red`, `ci_waiting` → `waiting`, `ci_no_checks` → `noChecks`, none → `none`); `round` from the newest `ci/<round>.log` name; `head` from the newest `ci_red`/`ci_green`; `checks` from the newest `ci_waiting`, else `[]`. A missing log is null fields, never an error.

**Acceptance:** `live_shows_a_running_gate_tail` (in `tests/live/gate.rs`: a fixture run with `gate_started` and a 200-line log with no `gate_run` → `running: true`, `tail` is the last 40 lines and no line over 300 chars, `excerpt` null; adding a red `gate_run` → `running: false`, `red: true`, `excerpt` holding the planted failure line); a fixture with `ci_red` and `ci/2.log` gives `ci.state == "red"`, `round == 2` and the excerpt.

### 5. Verify — medium — blocked by: 2, 4
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 6. Prove — at the checkpoint — blocked by: 5
prd §12 row **FR-4**: run the `excerpt_finds_*` tests, `ci_red_writes_the_whole_failure_log` and `docs_check_rejects_a_bad_gate_failure_regex`, and keep the transcript.

**Acceptance:** after the checkpoint, this slice's Progress names that transcript.
