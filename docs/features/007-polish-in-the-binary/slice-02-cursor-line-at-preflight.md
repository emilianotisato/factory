# Slice 02 — Cursor line at preflight

**Source**: [`prd.md`](prd.md) → §3 FR-1; §11 Live proof ("`factory run` prints the Cursor line as it starts"). **Depends on**: — (independent: no file in common with slices 01, 03, 04 or 05)
**Created**: 2026-10-15

**Progress**: `preflight::second_opinion(cfg, pf)` (`src/docs/preflight.rs`) probes into a fresh review log dir and adds an ok line ("second opinion: {agent} with {model} ready") or a judge line (the reason, "run `{agent} login`", the fallback per `pass_lines`-line pass). `preflight::cli` calls it before printing or serializing; `factory run` calls it after the refuse check with its loaded config, so the judge line prints as `  · …` and lands as `judge_noted`. `preflight::run` stays probe-free. Tests: `tests/preflight.rs` (logged in/out, text and `--json`, equal exit codes; `pass_lines` from config; a logged-out `factory run --foreground` notes the line). Inner loop: 2 runs, 1 red (fmt on the new test file), ~2.5 min. Deviation: AD-1 (probe after the refuse check; a broken config is a judge line).

Task 2's transcript (`cargo test --test preflight preflight_names -- --nocapture`):

```
$ factory docs preflight 001
  ✓ second opinion: cursor-agent with grok-4.7-high ready
exit=Some(0)
$ factory docs preflight 001 --json
    "second opinion: cursor-agent with grok-4.7-high ready"
exit=Some(0)
$ factory docs preflight 001          (stub list_models = "auth")
  ? second opinion not ready: cursor-agent --list-models exited 1: Error: Authentication required. Run 'agent login' first, or set CURSOR_API_KEY. — run `cursor-agent login`; polish falls back to one Claude agent per 600-line pass
exit=Some(0)
$ factory docs preflight 001 --json   (stub list_models = "auth")
    "second opinion not ready: cursor-agent --list-models exited 1: Error: Authentication required. Run 'agent login' first, or set CURSOR_API_KEY. — run `cursor-agent login`; polish falls back to one Claude agent per 600-line pass"
exit=Some(0)
```

Checkpoint 1 (FR-1): logged in → ok line with the model, logged out → judge line with `cursor-agent login`, exit 0 both ways, text and `--json`: `docs/features/007-polish-in-the-binary/evidence/cp-1-preflight-second-opinion.txt`.

**Slice**: After this slice, `factory docs preflight` and `factory run` probe the second opinion with the user's `[review]` config. Ready → an ok line naming the agent and the model. Not ready → a judge line with the reason, the login command, and that polish will fall back to one Claude agent per pass of `pass_lines` lines. The line never refuses a run and never changes an exit code.

## Current code / environment

- **The probe**: `review::probe(cfg, dir)` (`src/review.rs:243-279`) runs `<agent> --list-models` under `cfg.timeout_secs` and writes `list-models.txt` and `.err` into `dir`. It returns `Err(reason)`: `"{agent} --list-models exited {code}: {first line}"` on a nonzero exit, the first line itself when the text holds "login" or "Authentication required", `"model {m} not in --list-models"`, or a timeout line. `factory review second` calls it with `review::new_log_dir(&paths::data_dir())` (`src/cli/review.rs:91-95`).
- **The config**: `config::load(&paths::config_dir())` and `loaded.config.review` (`src/cli/review.rs:62-66`). Defaults: agent `cursor-agent`, model `grok-4.7-high`, `pass_lines` 600 (`src/config.rs:66-75`).
- **The login command** is not in the probe's output for the real agent: the stub's logged-out line reads "Error: Authentication required. Run 'agent login' first, or set CURSOR_API_KEY." (`tests/stubs/cursor-agent:41-44`). Name `{agent} login` from the config, so a renamed binary gets its own name.
- **Preflight today**: `preflight::run(root, reference)` (`src/docs/preflight.rs:87-241`) takes no config. Its ok/judge pattern is the skills line (`:111-114`) and the leaks line (`:115-118`). The exit code comes from `refuse` and `ask` only (`:220-226`), so a judge or ok line cannot change it.
- **Two user-facing paths**: `preflight::cli` (`:353-387`), reached from `factory docs preflight` (`src/cli/docs.rs:303`), serializes the `Preflight` for `--json` (`:356-358`) or prints `✓ ✗ ! ?` marks (`:363-372`). `factory run` calls `docs.preflight(&feature)` (`src/cli/run.rs:93`), saves it (`:130`), then prints each judge line as `  · {text}` and emits `JudgeNoted` (`:132-136`). It prints no ok line.
- **The trap: `preflight::run` has in-process callers.** `Docs::preflight` (`src/docs/mod.rs:72-74`) is also called by `src/scheduler.rs:620` (a fallback when no preflight was saved), `src/closeout.rs:2260` (`default_branch`'s fallback) and `tests/docs.rs:33,45`. Those tests run inside `cargo test` with the owner's own `PATH` and config, so a probe inside `run` would spawn the owner's real `cursor-agent`, and every closeout fallback would pay a probe. Add the probe in the two user-facing paths, after `run` returns.
- **Tests that read preflight output**: `tests/leaks.rs:215-246` matches lines with `contains` and asserts the exit code stays equal; `tests/harness.rs:44-46` asserts `pass` and an empty `ask`. `tests/run.rs:347-348` and `tests/loop.rs:572` assert exactly four `judge_noted` events, then `run_started`. The stub answers `--list-models` with the model by default (`tests/stubs/cursor-agent:40,45-48`), so the default probe is an ok line and those counts hold. The harness puts the stubs first on `PATH` (`tests/harness/mod.rs:502-550`), so a test that runs the binary reaches the stub.
- **The fixture feature**: `001` under `docs/features/001-fixture` (`tests/harness/mod.rs:16`); `Fixture::docs_output` runs `factory docs …` with the harness env (`:282-288`).
- **Deliberate non-change**: *(prd §13 "Not handled: refusing a run while Cursor is logged out. Accepted 2026-10-14: a machine without Cursor could not run, and the fallback found highs in 3 of 5 runs.")* The line is `judge`, never `refuse` or `ask`.

## Tasks

### 1. The probe line — high — blocked by: —
**Where**: `src/docs/preflight.rs` (beside `cli`), `src/cli/run.rs:93-136`.

**Source**: §3 FR-1: "Preflight runs the same probe with the user's `[review]` config. Ready → an ok line naming the agent and the model. Not ready → a **judge** line: the reason, the login command, and that polish will fall back to one Claude agent per 600-line pass. The run still starts if the owner starts it, and polish keeps the fallback." §11 Live proof: "`factory run` prints the Cursor line as it starts".

One function loads the config, probes into a fresh review log dir and pushes the line into the `Preflight`'s `ok` or `judge`. `preflight::cli` calls it before printing or serializing, so `--json` carries the same text. `factory run` calls it right after `docs.preflight` at `src/cli/run.rs:93`, so a judge line prints as `  · …` and lands as a `JudgeNoted` event through the existing loop. Take the pass size from the config's `pass_lines`, not a literal 600. Leave `preflight::run` without a probe (see the trap above).

**Acceptance:** task 2's tests.

### 2. Logged out, logged in — high — blocked by: 1
**Where**: a test file beside `tests/leaks.rs`'s preflight test, e.g. `tests/docs_lifecycle.rs`, or a new `tests/preflight.rs`.

**Source**: §3 FR-1 acceptance: "On the harness fixture (feature `001`), with the stub `cursor-agent` in its logged-out mode (`list_models = "auth"` …), `factory docs preflight 001` prints a `?` judge line holding the reason and the login command; with the stub listing the model, a `✓` line naming the agent and the model. `--json` carries the same text in `judge` or `ok`. The exit code is the same in both modes: the probe line never changes it."

Write the stub's scenario file (`cursor-agent.json` in the stub dir, as `tests/review.rs:35-37` does), run preflight both ways with and without `--json`, and compare the exit codes. A third case runs `factory run` against the logged-out stub and finds the Cursor line in a `judge_noted` event.

**Acceptance:** logged out: a `  ? ` line holding "Authentication required" and `cursor-agent login`, and the same text in `--json`'s `judge`. Logged in: a `  ✓ ` line naming `cursor-agent` and `grok-4.7-high`, and the same text in `ok`. Equal exit codes. The run's events hold a `judge_noted` with the line.

### 3. Verify — medium — blocked by: 1, 2
`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 4. Prove — at the checkpoint — blocked by: 3
Rows from [`prd.md`](prd.md) §11 this slice proves, below the UI:

- **FR-1**: "run `factory docs preflight 001` against the stub logged out, then logged in: a judge line with the login command, then an ok line with the model, same exit code" — task 2's test transcript, with both outputs and both exit codes.

**Acceptance:** after the checkpoint, Progress holds task 2's transcript.
