---
slice: 04
status: pending
depends_on: []
---

# Slice 04 — Knobs (FR-23)

**Source**: [`prd.md`](prd.md) → §5 FR-23, four decisions ("A project scope: `docs/project.toml`", "The run row records what it resolved", "`factory run NNN --hypothesis`", "`workers.model_id`"); §11 Sequencing risk (PRD-002 owns `docs/project.toml` later). **Depends on**: — (independent of 01, 02, 03 and 05; it appends tuples to the shared migration list and one column to the per-worker line slice 02 extends, a few-line merge either way).
**Created**: 2026-10-13 · **Status**: pending

**Progress**: _(written as it lands — what was built, the gate numbers, what a review caught)_

**Slice**: After this slice a project can set model, effort and budget per role in `docs/project.toml`, every run row says which scope each knob came from and which model actually answered, and a run can carry the one-line hypothesis it tests. It turns no knob itself (prd §1) and leaves the retro's knobs block to slice 06.

## Current code / environment

- **One config file, user scope**: `src/config.rs:69-101` `load(config_dir)` reads `config.toml` (NotFound → defaults), `Config { budget_weighted_tokens, workers: Workers { build, fix, checkpoint, polish: Worker { model, effort } } }`, `Loaded { config, path, warnings }`; defaults `DEFAULT_BUDGET 30_000_000`, `DEFAULT_MODEL "opus"`, `DEFAULT_EFFORT "high"` (`:11-13`); `unknown_keys` (`:103-125`) with `TOP_KEYS`, `ROLE_KEYS`, `WORKER_KEYS` warns, never fails. The dir is `paths::config_dir()` (`src/paths.rs:76-78`: `FACTORY_CONFIG_DIR`, `XDG_CONFIG_HOME`, `.config`).
- **Two load sites**: `src/cli/run.rs:50` (`run()`, prints `warnings` to stderr, `budget = args.budget.unwrap_or(loaded.config.budget_weighted_tokens)`) and `:162` (`daemon()`). Both must read the project file, or the daemon's workers run on the user config while `factory run` reported the project's.
- **Where roles read `cfg`**: build `src/scheduler.rs:733` (`&cfg.workers.build`), checkpoint `src/checkpoint.rs:320-326`, fix `:640-647`, polish `src/closeout.rs:261-267`, CI fix `:1577-1583` (reuses `fix`). `Worker::spawn` passes `--model &cfg.model`, `--effort &cfg.effort` (`src/worker.rs:282-297`); `record()` stores the configured strings as `workers.model` and `effort` (`:396-405`).
- **`RunArgs { feature, budget: Option<u64>, foreground }`** (`src/cli/run.rs:27-37`); `Run::create(data_dir, repo, feature, branch, budget)` (`src/run.rs:275`) calls `db.insert_run(&state)` (`:323`) from `RunState` (`:109+`); `RunRow` (`src/db.rs:111-122`) has `id, repo, feature, branch, started_at, finished_at, phase, budget_weighted_tokens, pr_url`. The `runs` table stores nothing else about the configuration (prd §5).
- **The model that answered**: `message.model` is parsed into `Message` (`src/stream.rs:45-50`) and dropped. Verified 2026-10-13 on run 024's `ddbb0927` stream: the first assistant line with `parent_tool_use_id` null has `message.model = "claude-opus-5-5"`; the `system/init` line also carries `model` and `claude_code_version` (`2.1.296`). Subagent lines carry their own `message.model` (prd §12: not stored).
- **Lanes** come from the docs index: `Status.lanes` (`src/docs.rs`), `docs.rs:197`.
- **The stub records flags**: `tests/stubs/claude:100-111` writes `model`, `effort`, `settings` from argv; `init.model` is overridden by `--model` (`:142`). The stub treats every invocation as a worker: a `claude --version` call has no `-p` and would fail its argument parsing.
- **The skill that passes flags**: `skills/factory-deliver/SKILL.md:12`: "`factory run NNN` (add `--budget <weighted tokens>` only when the owner gave one)".
- **Report conventions**: append new keys at the end of `EXTRA_KEYS` (`src/cli/report.rs:80-89`); a unit test asserts order and `lines()[16]`, `[17]`, `[21]`. `PerWorker.text()` (`:205-216`); slice 02 appends `own/sub/subagents` after `repeat_rate` and leaves a comment where `model_id` goes.
- **Deliberate non-changes** — *(prd §5: "No variant files, no experiment ledger: an experiment is a settings change in the right scope, and the row says which scope."; §12: "Not handled: variant files or an experiment ledger"; "Not handled: subagent `model_id`. A subagent's model varies per call and the skills choose it; FR-23 records the worker's own."; §5 rejected a `.factory/config.toml`. §11: PRD-002 adds its own keys to `docs/project.toml` later, so unknown top-level tables there warn as today and never fail. This PRD creates no `docs/project.toml`.)*

## Tasks

### 1. The project scope: `docs/project.toml` — high — blocked by: —
**Source**: §5 FR-23: "A project scope: `docs/project.toml`, with `[workers.<role>] model`, `effort` and `[run] budget`, overriding the user file key by key, read from the repo the run targets. Unknown keys warn as today (`src/config.rs:103-125`). The file is read when present; this PRD creates none."

`src/config.rs`, `src/cli/run.rs:50` and `:162`. Add `load_with_project(config_dir, repo) -> Result<Loaded>`: read the user file as today, then `repo/docs/project.toml` when present (NotFound → nothing), with `[run] budget` and `[workers.<role>] model | effort` overriding key by key. `Loaded` gains `sources: Sources { budget: Source, workers: per role { model: Source, effort: Source } }`, `enum Source { Default, User, Project }` serialised lower case. Project-file unknown keys go through `unknown_keys` with the file name in the warning. Both load sites call the new function with the run's repo (`paths::repo_root()` resolves it in `run()`; the daemon has it in `RunState.repo`). `--budget` on the command line still wins over every file (`src/cli/run.rs` `unwrap_or`), recorded as source `flag` in task 2.

**Acceptance:** `config.rs` unit tests: a user file with `[workers.polish] model = "opus"` and a project file with `[workers.polish] model = "sonnet"` resolve `polish.model == "sonnet"` with source `project`, `polish.effort` from the user file or default with its own source, `build.model` default with source `default`; a project file with `[docs] foo = 1` yields one warning naming `docs/project.toml` and no error; no project file → identical to today's `load`.

### 2. The run row records what it resolved — high — blocked by: 1
**Source**: §5 FR-23: "`runs.knobs`, JSON, written at run start: per role `{model, effort, source}` with `source` one of `default`, `user`, `project`; `budget` with its source; `lanes`; the `factory` version; the `claude --version` string."

`src/db.rs` (append `("runs", "knobs", "TEXT")` to the migration list; `set_run_knobs(run_id, json)`; `RunRow.knobs`), `src/cli/run.rs` right after `Run::create` (`:98`, `:121`), with a small `knobs::resolve(&loaded, lanes, claude_version) -> serde_json::Value` in `src/config.rs`. Shape: `{"workers":{"build":{"model":"opus","effort":"high","source":"default"},…},"budget":{"value":30000000,"source":"user"},"lanes":1,"factory_version":"0.1.0","claude_version":"2.1.296 (Claude Code)"}`. `factory_version` is `env!("CARGO_PKG_VERSION")`; `claude_version` is the trimmed stdout of `claude --version` run once at start, or `null` with one stderr line if the command fails. When a role's `model` and `effort` come from different scopes, `source` is the model's scope and `effort_source` is written beside it only then. A `--budget` flag records `"source":"flag"`.

The trap: the stub `claude` treats every call as a worker (`tests/stubs/claude`, argument parsing before the role check). Give it a `--version` branch that prints a fixed string and exits 0 before anything else, or every harness run logs a failed version probe.

**Acceptance:** harness test with a fixture `docs/project.toml` holding `[workers.polish] model = "sonnet"`: the stub call log shows `model == "sonnet"` for the polish call and `"opus"` for every other call (`tests/stubs/claude:109-111`); the run row's `knobs` parses with `workers.polish.source == "project"`, `workers.build.source == "default"`, `lanes == 1`, `factory_version` non-empty and `claude_version` equal to the stub's fixed string.

### 3. `--hypothesis` — high — blocked by: 2
**Source**: §5 FR-23: "`factory run NNN --hypothesis "<one line>"` stores `runs.hypothesis`. The `/factory-deliver` skill passes it when the owner gives one. Optional; most runs have none."

`src/cli/run.rs` `RunArgs.hypothesis: Option<String>`; `src/db.rs` `("runs", "hypothesis", "TEXT")`, `set_run_hypothesis`, `RunRow.hypothesis`; written once at run creation. On a resume of a live or parked run with `--hypothesis` given, print one stderr line `hypothesis ignored: run <id> already exists` and keep the stored one. `skills/factory-deliver/SKILL.md:12`: extend the parenthesis to "add `--budget <weighted tokens>` or `--hypothesis "<one line>"` only when the owner gave one".

**Acceptance:** harness test: `factory run 001 --hypothesis "sonnet polish costs less"` stores that string on the row and `factory report <id> --json` carries it (task 5); a second `factory run 001 --hypothesis other` on the same live run leaves the stored string unchanged and prints the stderr line.

### 4. `workers.model_id` — high — blocked by: —
**Source**: §5 FR-23: "`workers.model_id` is the model string of the worker's first own assistant message, so a row says what answered, not only what was asked for."

`src/stream.rs` `Summary.model_id: Option<String>`, set in `Collector::push` from the first assistant line whose `parent_tool_use_id` is null (slice 01 reads that field; if this slice lands first, read it here the same way: a non-null string in `rest`); `src/db.rs` `("workers", "model_id", "TEXT")`, written by `finish_worker` (`:434-467`), `WorkerRow.model_id`; `src/cli/report.rs` `PerWorker.model_id`, appended after slice 02's `subagents` column (or after `repeat_rate` if 02 has not landed), in text and JSON.

**Acceptance:** `stream.rs` unit test: two assistant lines, the first with `parent_tool_use_id: "toolu_x"` and model `claude-sonnet-x`, the second own with `claude-opus-x` → `model_id == Some("claude-opus-x")`; harness: every worker row's `model_id` equals the stub stream's `init.model` after the `--model` override (`tests/stubs/claude:142`), so the polish row of task 2's fixture reads `sonnet`-derived and the others `opus`-derived.

### 5. `report` prints the knobs block — medium — blocked by: 2, 3
**Source**: §5 FR-23 Acceptance: "`report` prints the knobs block; `--hypothesis` text is in the row and in the retro (FR-25)."

`src/cli/report.rs`. Two `EXTRA_KEYS` entries appended last: `knobs` (text: `build opus/high default · fix … · polish sonnet/high project · budget 30000000 user · lanes 1 · factory 0.1.0 · claude 2.1.296`; JSON: the stored object) and `hypothesis` (the string, or `—`). `Report` reads them from the `runs` row; an old run with null `knobs` prints `—`.

**Acceptance:** the report unit test's order assertion gains `knobs`, `hypothesis` last with `lines()[16]`, `[17]`, `[21]` unchanged; after task 2's fixture run, `factory report <id>` prints `polish sonnet/high project` and `factory report <id> --json` has `hypothesis` equal to the stored string.

### 6. Verify — medium — blocked by: 1-5
`cargo test` (the inner loop; the full gate `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` runs at this slice's checkpoint).

**Acceptance:** inner loop green, one commit.

### 7. Prove — at the checkpoint — blocked by: 6
Rows from [`prd.md`](prd.md) §9 this slice must prove, below the UI: the **FR-23 row**: `run` the fixture with `docs/project.toml` `[workers.polish] model = "sonnet"` → stub log shows `--model sonnet` for polish only; `knobs` JSON shows the sources; `--hypothesis` lands in the row. Driven at CP-2 together with the full gate; the test transcript and the `report` output go in this slice's **Progress**.

**Acceptance:** the FR-23 row has its test transcript and one `report` output attached in Progress, the knobs line showing `project` on polish and `default` on build.
