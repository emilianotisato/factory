---
slice: 01
status: pending
depends_on: []
---

# Slice 01 — Harness: crate, fixture repo, stub claude and gh (FR-10)

**Source**: [`prd.md`](prd.md) → §12 FR-10 (all three bullets), §16 QA plan "Harness: none — FR-10 builds it as the first slice". **Depends on**: — (first slice; everything else depends on this one)
**Created**: 2026-10-09 · **Status**: pending

**Progress**: _(written as it lands — what was built, the gate numbers, what a review caught)_

**Slice**: After this slice the repo is a Rust crate that the declared gate can run, and `cargo test` can build a throwaway git repo on the feature-docs v2 framework, put a stub `claude` and a stub `gh` on PATH, and script the fixture's gate outcome. The stub `claude` applies the side effects a real worker would (slice frontmatter, log fragment, a touched code file) and then replays a recorded stream, so the loop the later slices build has something that advances the board. Only the `done` scenario ships here; each later slice adds the scenario it consumes (§17). No driver code: `factory` has subcommands that print "not built yet" and exit 2.

**The highest-risk work in this slice is task 5, the stub `claude`.** If it emits a stream without flipping the board, every later test hangs on a slice that never becomes `done`. Write its side effects first and the stream second.

## Current code / environment

- **There is no crate.** The repo holds `docs/` and `.git` only; `docs slice` warned `no Cargo.toml for cargo` on every slice it created. The gate in `docs/index.md:3-4` is `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`, inner `cargo test`; this slice creates the crate it runs against. Toolchain on the owner's machine: cargo 1.98.1, node v20.20.2.
- **No CI workflow exists** (no `.github/`). §16 FR-10 names "CI run" as the evidence and §8 FR-6 watches `gh pr checks` on this repo's own PR, so the harness slice adds the workflow that runs the gate.
- **`docs.mjs` is Node, zero dependencies, Node 18+, and reads templates from `../templates/` relative to itself** (`docs.mjs:8`). `vendor(root, dest='.docs')` (`docs.mjs:1913`, command `:2093`) copies `bin/docs.mjs` and `templates/` under `.docs/`; `:1926-1927` prefers a vendored `.docs/bin/docs.mjs` when one exists. A vendored copy committed under `tests/fixture/.docs/` is what lets a clean checkout run the tests with `node` alone.
- **Preflight asks and refusals the fixture must avoid**: `docs.mjs:1703` refuses on any open BLOCK; `:1726-1732` refuses on a dirty tree; `:1708` asks when no inner loop is declared; `:1713` asks about lanes; `:1717` asks when `gate_parallel` is false, `:1718` judges when it is unmeasured. So the fixture index declares `gate`, `inner`, `lanes: 2`, `gate_parallel: true` (read at `docs.mjs:317`), and the builder commits everything before the first `factory` call. `:1707` judges on a missing `docs/ux-rubric.md`; a judge is routed, not blocking, and the fixture has no UI, so leave it.
- **Lanes may be counted from files, not from the index key.** `docs.mjs:1470-1490` reads `docs/reference/lanes/lane-N.md` (frontmatter `lane`, `env`, `setup`), and §20 notes "no project under `~/Projects` has `docs/reference/lanes`". Check at slice entry whether `lanes: 2` in the index is enough or two lane files are needed; if files, the fixture writes `lane-1.md` and `lane-2.md` with `env: []`.
- **`status --json` shape** (`docs.mjs:1372-1416`): `slices[]` with `number, file, title, status, lane, depends_on`; `ready[]` as `{number,file}`; `building[]` as `{number,file,lane}`; `spawnable` is 0 while a checkpoint is due (`:1414`); `checkpointEvery` default 2 (`:318`). Slice statuses are `pending|building|blocked|done` (`:22`).
- **The stream the stub replays is recorded**: [`recordings/claude-p-oneshot-2.1.280.jsonl`](recordings/claude-p-oneshot-2.1.280.jsonl), 18 events: 11 `system` (`hook_started`, `hook_response`, `init`), 4 `assistant`, 1 `user` (a `tool_result`), 1 `rate_limit_event` (`rate_limit_info.status: "allowed"`, `resetsAt`, `unifiedWindows`), 1 `result` with `usage{input_tokens,cache_creation_input_tokens,cache_read_input_tokens,output_tokens,output_tokens_details.thinking_tokens}`, `modelUsage`, `num_turns`, `duration_ms`, `terminal_reason`, `permission_denials`, `total_cost_usd`, `session_id`. [`claude-p-json-haiku-2.1.280.json`](recordings/claude-p-json-haiku-2.1.280.json) is the `--output-format json` single result. Strip the owner's hook names and paths from what the stub replays (§19: nothing owner-specific outside `config.toml`).
- **The flags the stub must accept** are FR-2's: `-p <prompt> --output-format stream-json --verbose --permission-mode bypassPermissions --append-system-prompt-file <pack> --session-id <uuid> --model <m> --effort <e>`, plus `--settings <json>` from slice 13 and the inline `--append-system-prompt <text>` fallback (§20: the `-file` form is only mentioned in the `--bare` help text).
- **The global git hooks apply inside the fixture.** `git config --global core.hooksPath` is `~/.config/git/hooks` (the `commit-msg` and `pre-push` hooks, active under `CLAUDECODE=1`). They are harmless to the fixture's commits but a test must not depend on them: the fixture sets `GIT_CONFIG_GLOBAL` to an empty file and passes `user.name`/`user.email` itself.
- **Deliberate non-change**: *(prd §19 "Not handled: macOS or Windows". Linux only; use `/usr/bin/env` shebangs and POSIX paths without a compatibility layer.)* *(prd §19 "Not handled: two runs on the same repo at once". A fixture per test in its own temp dir makes the question moot here.)*

## Tasks

### 1. Crate skeleton the gate can run — high — blocked by: —
**Source**: §12 FR-10: "`cargo test` runs the whole loop. Start to draft PR, no network, under 60 seconds." §16: gate `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`.

`Cargo.toml` (package `factory`, edition 2024, `[[bin]] factory`, `[lib]`), `src/main.rs`, `src/lib.rs`, `rustfmt.toml` if a non-default is wanted (none expected), `.gitignore` with `target/`. clap 4.6 derive with the subcommands the PRD names (`run`, `wait`, `answer`, `status`, `stop`, `report`, `ctx def|refs|outline`, `hook`), each printing `not built yet (slice NN)` to stderr and exiting 2. No other dependencies yet. `cargo clippy -D warnings` must be green on the skeleton, so no `todo!()` left in reachable code.

**Acceptance:** `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` exits 0 on the skeleton; `factory --help` lists the eight subcommands; `factory run 001` exits 2.

### 2. Vendored docs tool for the fixture — high — blocked by: —
**Source**: §12 FR-10: "A git repo on the v2 framework, created in a temp dir per test: `docs.mjs` vendored".

`tests/fixture/.docs/bin/docs.mjs` and `tests/fixture/.docs/templates/*`, produced by `node ~/.claude/skills/feature-docs/bin/docs.mjs vendor` run in a scratch repo and copied in (`docs.mjs:1913`). Record the source version in `tests/fixture/.docs/VERSION` (the file's `sha256` and the date) so a refresh is a diff, not a guess. Tests call `node <fixture>/.docs/bin/docs.mjs`, never the owner's `~/.claude` copy.

**Acceptance:** in a fresh fixture, `node .docs/bin/docs.mjs doctor` exits 0 and `node .docs/bin/docs.mjs gate` prints `./gate.sh`.

### 3. Fixture builder — high — blocked by: 2
**Source**: §12 FR-10: "`docs/index.md` declaring `gate: ./gate.sh`, `inner: ./gate.sh --inner`, `lanes: 2` and `gate_parallel: true`, so preflight has no ask to raise (`docs.mjs:1708`, `:1713`); one PRD with three slices, two independent and one dependent."

`tests/harness/mod.rs`: `Fixture::new(name) -> Fixture` builds in a `tempfile` dir: `git init -b main`, `GIT_CONFIG_GLOBAL` pointed at an empty file, `user.name`/`user.email` set, `.docs/` copied from task 2, `docs/index.md` from the vendored template with frontmatter `framework: 2`, `gate: ./gate.sh`, `inner: ./gate.sh --inner`, `lanes: 2`, `gate_parallel: true` and any extra keys the caller passes (`Fixture::with_index(key, value)`, needed by slice 03 for `checkpoint_every`), `docs/ideas.md`; the feature through the vendored tool itself so the shape stays right: `docs new fixture --title "Fixture"`, then `docs slice 001 a`, `docs slice 001 b`, `docs slice 001 c --depends 2`; a `## QA plan` with one `run` row per slice in the PRD so `W-NO-QA-PLAN` does not fire; `src/lib.txt` as the code file the stubs touch; `gate.sh` (task 4) executable; `scenario/` directory; one commit of everything, so `preflight --json` sees a clean tree. If slice entry finds lanes are counted from files (see Current code), write `docs/reference/lanes/lane-1.md` and `lane-2.md` too. Expose `fixture.docs_json("status")` and `fixture.docs_json("preflight")` returning `serde_json::Value`, `fixture.path()`, `fixture.env()` (the PATH and env the driver must run with: `PATH=<stubs>:<dirs where node and git live>`, `FACTORY_STUB_DIR`, `FACTORY_DATA_DIR`, `FACTORY_CONFIG_DIR`, `GIT_CONFIG_GLOBAL`, `HOME` unchanged).

The trap: `docs new` and `docs slice` refuse or warn when run outside a framework repo and read the index frontmatter for the gate; create the index first, the feature second, and check the exit code of every tool call in the builder so a shape change in `docs.mjs` fails the fixture loudly instead of three tests later.

**Acceptance:** `Fixture::new` returns in under 3 s; `status --json` on it has `slices.len() == 3`, `ready.len() == 2`, `spawnable == 2`, `checkpointDue == false`, `lanes == 2`; `preflight --json` has `pass == true` and `ask == []`.

### 4. Scenario-scripted `gate.sh` — high — blocked by: 3
**Source**: §12 FR-10: "`gate.sh` reads a scenario file and exits as the test scripts it (for example red, red, green), so gate outcomes belong to the test, not to the stub agent."

`tests/fixture/gate.sh`, copied into every fixture. Without `--inner` it consumes the first line of `scenario/gate` (`green` or `red`); with `--inner` the first line of `scenario/inner`. An empty or missing file means green. Every invocation appends `<epoch> <inner|gate> <exit>` to `scenario/gate.log`, and a red run prints a fixed failure text (`FAIL tests/fake.rs: expected 2 got 3`) to stdout so fix workers have something to receive. Helpers `fixture.script_gate(&["red","red","green"])` and `fixture.gate_log() -> Vec<GateRun>`.

**Acceptance:** scripted `red, red, green`, four invocations exit 1, 1, 0, 0 and `gate.log` has four lines; `--inner` consumes `scenario/inner` and leaves `scenario/gate` untouched.

### 5. Stub `claude`: side effects, then the recorded stream — high — blocked by: 3
**Source**: §12 FR-10: "A stub `claude` and a stub `gh` on PATH. The stub replays canned stream-json for the scenario under test: done, blocked, failed-then-fixed gate, rate limit, dead worker, red CI then green. The canned streams are cut from real recordings, including a real `result` event with usage fields." §4 FR-2: "The DONE/BLOCKED/FAILED line at the end of `result` is a hint; the slice frontmatter after `docs status --json` is the truth."

`tests/stubs/claude`, a Node script (Node is already required for `docs.mjs`; no jq, no Python). Behaviour, in order:
1. Record the call: argv, cwd, env `FACTORY_*`, and the full text of the pack (`--append-system-prompt-file <path>` read, or `--append-system-prompt <text>`) to `$FACTORY_STUB_DIR/calls/<n>.json`, `n` from a counter file. Slice 03's FR-2 acceptance reads these.
2. Pick the step: `$FACTORY_STUB_DIR/plan.jsonl`, one JSON object per spawn, consumed in order; past the end, `{"outcome":"done"}`. Step keys this slice supports: `outcome: "done"`, `touch: ["src/lib.txt"]` (default that one file), `delay_ms` (sleep before the side effects, so a test can kill mid-slice).
3. Find the slice: the pack names exactly one `slice-NN-*.md` path (the pack layout is slice 03's; until then the test writes a pack that is just the slice file's path on its first line, and the stub accepts both).
4. Side effects for `done`: set `status: building` in the slice frontmatter, sleep `delay_ms`, append the touched files (content `touched by spawn <n>\n`), write `docs/features/001-fixture/log-slice-NN.md` with one `AD-NN01` entry in the shape of `templates/agent-log.md` from the vendored copy, append a `**Progress**` line to the slice, set `status: done`. No commit: the driver commits (§7 FR-5).
5. Emit the stream to stdout, one JSON per line: the recording's `system/init` (hook events removed, `cwd` and `session_id` replaced with the real ones), one `assistant` event with a `tool_use` named `Write` per touched file (`input.file_path` absolute) and one for the fragment and one `Edit` for the slice file, a `user` `tool_result` for each, one `assistant` text `STATUS: done`, then the recording's `result` with `session_id` set from `--session-id`, `result` text `STATUS: done\nCOMMIT: none\n…` in the slice contract's return shape. Exit 0.

Keep the recorded `result`'s usage numbers as they are; tests assert they are non-null, not their values.

**Acceptance:** `tests/stubs.rs`: calling the stub with FR-2's flags and a pack naming fixture slice 01 flips that slice to `done`, creates `log-slice-01.md` with `AD-101`, appends to `src/lib.txt`, writes `calls/1.json` holding the pack text, and prints a stream whose last line parses as `result` with all five usage fields present; with `delay_ms: 2000` the slice reads `building` 500 ms after spawn.

### 6. Stub `gh`: recorder with canned answers — medium — blocked by: 3
**Source**: §12 FR-10: "A stub `claude` and a stub `gh` on PATH." §8 FR-6 acceptance: "stubbed `gh` call log".

`tests/stubs/gh`, Node. Appends argv to `$FACTORY_STUB_DIR/gh.log`; `auth status` exits 0; `pr create` prints `https://example.invalid/pr/1`; `pr view --json` prints `{"number":1,"url":…}`; `pr edit` exits 0; everything else exits 0 with empty output. CI scenarios (`pr checks --watch` red or green, `run rerun`) are slice 09's.

**Acceptance:** `gh pr create --draft --body-file x` prints the URL and `gh.log` holds the argv line.

### 7. Harness smoke test and the CI workflow — high — blocked by: 1, 4, 5, 6
**Source**: §12 FR-10 acceptance: "`cargo test` green on a clean checkout with no `claude` and no `gh` installed, in under 60 seconds." §16 FR-10: evidence "CI run".

`tests/harness.rs`: builds a fixture, asserts `which claude` and `which gh` under `fixture.env()` resolve inside `tests/stubs/`, asserts the task 3 numbers, scripts and runs the gate once, and prints the elapsed time. `.github/workflows/ci.yml`: ubuntu, `actions/checkout`, Node 20, stable Rust with `rustfmt` and `clippy`, then the gate command verbatim from `docs/index.md`. No `claude` and no `gh` are installed on that runner, which is the acceptance.

**Acceptance:** `cargo test` on a clean clone of the branch passes in under 60 s locally; the workflow file runs the exact gate string.

### 8. Verify — medium — blocked by: 7
`cargo test`. The full gate runs at this slice's checkpoint.

**Acceptance:** inner loop green, one commit.

### 9. Prove — at the checkpoint — blocked by: 8
Rows from [`prd.md`](prd.md) § QA plan: **FR-10** ("`cargo test` on a clean checkout, no `claude`, no `gh` → green in under 60 s", evidence: CI run). At the checkpoint: clone the branch into a temp dir with `PATH` stripped of any real `claude` and `gh`, run `cargo test`, keep the timing line; the CI run on the PR is the second piece of evidence once slice 09 opens it. The evidence goes in this slice's **Progress**.

**Acceptance:** the test transcript with its elapsed time under 60 s is attached to Progress.
