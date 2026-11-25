# Slice 01 — Worker provider resolution, `RunState.worker_config`, preflight ask, and `setup workers`

**Source**: [`prd.md`](prd.md) → §2, §3 (FR-1), §9, §11. **Depends on**: — (spine; slices 03 and 04 build on `ResolvedWorkers`, `RunState.worker_config`, `WorkerRef.provider`, `FACTORY_WORKER_PROVIDER`, and `resolve_workers`)
**Created**: 2026-11-25

**Progress**: _(written as it lands — what was built, the gate numbers, what a review caught)_

**Slice**: Adds provider-scoped per-role worker resolution (`ResolvedWorkers`, `FACTORY_WORKER_PROVIDER`, and `--worker` spec parsing) in `src/config.rs`, persists `worker_config: Option<ResolvedWorkers>` and `WorkerRef.provider: Option<Provider>` on `RunState` (`src/run.rs`), adds repeatable `--worker` on `factory run` (`src/cli/run.rs`), wires `WORKER_ASK_PREFIX` preflight ask + missing-binary refusal + persistent `factory answer` (`src/docs/preflight.rs`, `src/cli/answer.rs`), activates provider & per-role mixing in `factory setup workers` (`src/cli/setup.rs`), and sets `FACTORY_WORKER_PROVIDER=claude` in `tests/harness/mod.rs`. Leaves stream normalizers to slice 02, worker spawning & shims to slice 03, and DB/report observability to slice 04.

## Current code / environment

- **Provider & config (from PRD-026)**:
  - `src/provider.rs:15-111, :216-252` exports `Provider` (`Claude`, `Codex`, `Cursor`, `Agy`, `Opencode`), `is_model_name`, `resolve_binary`, `detect_installed`, `discover_models`. Note `Provider::Opencode.default_model()` is `""` (`:74`) and `Provider::Cursor.effort_levels()` is `&[]` (`:61`).
  - `src/config.rs:82-154` already defines `Workers`, `Worker`, `ResolvedWorker { pub provider: Provider, pub model: String, pub effort: Option<String> }` (needs `Serialize, Deserialize` to live on `RunState`), and `Workers::resolved(&self, sources, role, fallback_provider)` implementing provider-scoped inheritance (never leaking `"opus"`/`"high"` across providers, normalizing `Cursor` effort to `None`).
  - `src/config.rs:286-287` defines `CI_MODE_ENV` and `REVIEW_PROVIDER_ENV`; `resolve_ci_mode` (`:434-470`) and `resolve_review` keep env reads outside `config::load`.
- **RunState trap**: `src/run.rs:118` ALREADY has `pub workers: Vec<WorkerRef>`! Name the new run-level worker config field `pub worker_config: Option<ResolvedWorkers>` (with `#[serde(default, skip_serializing_if = "Option::is_none")]`) so it never collides with `RunState.workers`.
- **Preflight & answer pattern**:
  - `src/docs/preflight.rs:360-472` has `REVIEW_ASK_PREFIX`, `second_opinion`, `CI_MODE_ASK_PREFIX`, `ci_mode`, and `cli` recomputing exit code (`1` refuse, `2` ask, `0` ok). `review_ask` (`:404-429`) formats OpenCode as `` `opencode:<model>` `` because it has no default model.
  - `src/cli/answer.rs:301-308, :404-460` validates and saves answers via `config::patch`, leaving the ask open on invalid input (`review_choice` handles colons inside model ids like `opencode:ollama/llama3:8b`, refuses bare `opencode`, and refuses models failing `provider::is_model_name`).
- **Setup wizard**: `src/cli/setup.rs:47-58` defines `SetupSection::Workers` without `--provider`, and `workers_step` currently prompts only Claude `model`/`effort`.
- **Fixture harness**: `tests/harness/mod.rs:609-630` sets `FACTORY_CI_MODE=remote` and `FACTORY_REVIEW_PROVIDER=cursor`.
- **Deliberate non-changes (PRD-027 §11)**:
  - Passing `--worker` on resume warns and leaves `RunState.worker_config` unchanged.
  - No named config profiles, no Gemini CLI, no cross-vendor warning.

## Tasks

### 1. Add `FACTORY_WORKER_PROVIDER`, `ResolvedWorkers`, `--worker` spec parser, and `RunState.worker_config` — high — blocked by: —
**Source**: §3 FR-1: "Precedence per role... Add repeatable `--worker <SPEC>` on `RunArgs`... Add `pub worker_config: Option<ResolvedWorkers>` on `RunState`... `WorkerRef.provider: Option<Provider>`".

`src/config.rs`, `src/run.rs`, `src/cli/run.rs`, `tests/harness/mod.rs`.
- In `src/config.rs`:
  - Derive `Serialize, Deserialize` on `ResolvedWorker` and add `#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)] pub struct ResolvedWorkers { pub build: ResolvedWorker, pub fix: ResolvedWorker, pub checkpoint: ResolvedWorker, pub polish: ResolvedWorker }` with `for_role(&self, role: &str) -> &ResolvedWorker` (`"merge"` maps to `&self.build`) and `distinct_providers(&self) -> Vec<Provider>`.
  - Add `pub const WORKER_PROVIDER_ENV: &str = "FACTORY_WORKER_PROVIDER";`.
  - Add `WorkerSpec { pub role: Option<String>, pub provider: Provider, pub model: Option<String>, pub effort: Option<String> }` and `parse_worker_spec(raw: &str) -> std::result::Result<WorkerSpec, String>`:
    - Splits optional `<role>=` prefix (`build|fix|checkpoint|polish`; reject unknown role names).
    - Parses `<provider>[:<model>[:<effort>]]` reusing the colon-safe rule from PRD-026 AD-12 (trailing `:<level>` is effort only when `provider.effort_levels().contains(&level)` or when `level` is a known effort level for another provider — refuse wrong-provider effort cleanly; preserve colons inside model ids like `ollama/llama3:8b`; validate model via `provider::is_model_name`; refuse bare `opencode` with no model).
  - Add `pub fn resolve_workers(loaded: &Loaded, specs: &[WorkerSpec]) -> (Option<ResolvedWorkers>, Option<String>)` (pure env lookup via `resolve_workers_from(loaded, specs, env)` so unit tests pass `env` directly):
    - For each role (`build`, `fix`, `checkpoint`, `polish`), resolve `provider`: matching role `--worker` spec → all-role `--worker` spec → `[workers.<role>].provider` → `[workers].provider` → `FACTORY_WORKER_PROVIDER` env (invalid non-empty returns warning and ignores env) → unresolved (`None`).
    - If any role has no resolved provider, return `(None, warning)`.
    - Otherwise resolve `model` and `effort` via the provider-scoped rule (flag → role override → `[workers]` scalar ONLY when resolved provider matches `[workers]`'s effective provider → `provider.default_model()` / `provider.default_effort()`, and `None` effort when `provider.effort_levels().is_empty()`).
- In `src/run.rs`:
  - Add `#[serde(default, skip_serializing_if = "Option::is_none")] pub worker_config: Option<ResolvedWorkers>` on `RunState` and `Run::set_worker_config(&mut self, w: Option<ResolvedWorkers>) -> Result<()>`.
  - Add `#[serde(default, skip_serializing_if = "Option::is_none")] pub provider: Option<Provider>` on `WorkerRef`.
- In `src/cli/run.rs`:
  - Add `#[arg(long = "worker", value_name = "SPEC")] pub workers: Vec<String>` on `RunArgs`.
  - In `flags_ignored`, warn `--worker ignored: run {id} already exists` when `!args.workers.is_empty()` on resume/live.
  - On new runs, parse `args.workers` via `config::parse_worker_spec` (failing fast with exit 1 on invalid spec), resolve via `config::resolve_workers`, call `preflight::workers(&loaded, &specs, &mut pf)`, and persist `run.set_worker_config(resolved_workers)?` when creating the run (both in the `pf.ask` branch when resolved and in the normal start branch).
- In `tests/harness/mod.rs:620`: add `("FACTORY_WORKER_PROVIDER".to_string(), "claude".to_string())` right next to `FACTORY_CI_MODE` and `FACTORY_REVIEW_PROVIDER`.

**The trap**: Do NOT break existing `Worker::spawn` callers in slice 01 (keep `src/worker.rs` untouched for slice 03). When a new run starts with `RunState.worker_config = Some(...)`, slice 01 persists it on `state.json` cleanly and existing Claude tests stay green; slice 03 wires `scheduler`/`checkpoint`/`closeout` to pass `ResolvedWorker` into `Worker::spawn`.

**Acceptance:** Unit tests `worker_config_mixes_roles_without_leaking_claude_defaults` (in `src/config.rs`) and `run_flag_worker_persists_across_daemon_and_resume` (in `tests/run.rs`) pass:
- `[workers]` = `claude:opus:high` + `[workers.build]` = `codex` (no model/effort) resolves `build` to `codex:gpt-5.4` (`Some("high")`), `[workers.fix]` = `cursor` resolves `fix` to `cursor:grok-4.7-high` with `effort: None` (fails if `"opus"` or `"high"` leaks onto `codex`/`cursor`).
- `factory run 001 --worker build=codex:gpt-5.4:xhigh` persists `worker_config.build` on `state.json`, survives resume, and resuming with `--worker claude` warns `--worker ignored` on stderr while keeping `worker_config.build == codex` (fails if `RunState.worker_config` or `flags_ignored` is missing).

### 2. Wire `WORKER_ASK_PREFIX` preflight ask, missing-binary refusal, `factory answer` persistence, and `factory setup workers` mixing — high — blocked by: 1
**Source**: §3 FR-1: "Preflight ask-if-missing and binary verification (`src/docs/preflight.rs`, `src/cli/run.rs`, `src/cli/answer.rs`)... Activate provider & effort mixing in `factory setup workers`".

`src/docs/preflight.rs`, `src/cli/answer.rs`, `src/cli/setup.rs`, `tests/preflight.rs`, `tests/setup.rs`.
- In `src/docs/preflight.rs`:
  - Add `pub const WORKER_ASK_PREFIX: &str = "worker provider not set in config.toml";`.
  - Add `pub fn workers(loaded: &config::Loaded, specs: &[config::WorkerSpec], pf: &mut Preflight)`:
    - Call `config::resolve_workers(loaded, specs)`. Push any env warning to `pf.judge`.
    - When `None`: call `provider::detect_installed()` and push `pf.ask` starting with `WORKER_ASK_PREFIX`, listing detected CLIs (formatting `opencode:<model>` when default model is empty), plus `proceed` (uses the first detected CLI with a default model for this run only without writing `config.toml`).
    - When `Some(resolved)`: for each distinct provider in `resolved.distinct_providers()`, check `provider::resolve_binary(p)`: if `None`, push to `pf.refuse` (`"worker provider {p} ({bin}) not found on PATH — install it or run `factory setup workers`"`). Also check every role's resolved model is non-empty (if empty, e.g. `opencode` with no model, push to `pf.refuse`). If no refusals were added for workers, push `pf.ok` summarizing resolved workers (compact when all 4 roles match, per-role when mixed).
  - Call `workers(&loaded, &[], &mut pf)` in `preflight::cli` (`:464-467`).
- In `src/cli/answer.rs` (`answer_ask`):
  - When `item.starts_with(preflight::WORKER_ASK_PREFIX)` → call `worker_answer(&mut run, id, text, out)?`:
    - `"proceed"` (case-insensitive): find the first `Detected` CLI on `PATH` whose `default_model()` is non-empty; if none, print an error naming `factory setup workers` and return `Ok(false)` (exit 1, ask stays open). Otherwise build `ResolvedWorkers` for that provider, call `run.set_worker_config(Some(resolved))?`, leave `config.toml` untouched, and print what was chosen for this run only.
    - Otherwise parse `<provider>[:<model>[:<effort>]]` (reject role prefix here; refuse bare `opencode` with no model, invalid model name, or wrong-provider effort). Write `[workers] provider`, `model`, and `effort` (removing `effort` if `None`) via `config::patch`, compute `ResolvedWorkers` from reloaded config, call `run.set_worker_config(Some(resolved))?`, and print saved path.
    - Invalid input → print valid choices, write nothing, return `Ok(false)`.
- In `src/cli/setup.rs`:
  - Add `#[arg(long, value_enum)] provider: Option<Provider>` on `SetupSection::Workers` and update `USAGE` to show `--provider <claude|codex|cursor|agy|opencode>`.
  - Update non-interactive `flagged` for `Workers`: supports `--role <default|build|fix|checkpoint|polish>` with `--provider`, `--model`, `--effort`, `--clear` (clearing a role removes `provider`, `model`, and `effort`; switching a role or default to a new provider clears stale `model`/`effort` unless explicitly supplied, matching `review_edits`; refuses bare `--provider opencode` without `--model` and no saved opencode model; refuses `--effort` for `cursor` or invalid model names).
  - Update interactive `workers_step`:
    - Step 1: pick default `[workers]` `provider` (from the 5 providers, marking installed ones), `model` (live `discover_models` for `cursor`/`agy`/`opencode` or curated for `claude`/`codex`, plus free text), and `effort` (provider-specific `effort_levels()`, skipped when empty e.g. `cursor`).
    - Step 2: *"Override any role (`build`, `fix`, `checkpoint`, `polish`) with a different provider/model/effort? [y/N]"* — loops over selected roles to set or clear per-role `provider`, `model`, `effort`.
  - Update `show` / `show --json` to display each role's resolved `provider` alongside `model`/`effort` and source.

**The trap**: Check `tests/setup.rs` from PRD-026! Existing wizard tests in `tests/setup.rs` feed scripted line-by-line input to `run_wizard`. Adding a `provider` prompt to `workers_step` shifts the scripted input lines in `tests/setup.rs` — update the wizard test inputs in `tests/setup.rs` deliberately so all `tests/setup.rs` cases stay green alongside the new mixed-role test!

**Acceptance:** Integration tests `preflight_asks_when_worker_provider_unset_and_refuses_missing_binary` (in `tests/preflight.rs`) and `setup_workers_configures_mixed_roles` (in `tests/setup.rs`) pass:
- Removing `FACTORY_WORKER_PROVIDER` with no `[workers] provider` makes `factory docs preflight 001` exit `2` with `WORKER_ASK_PREFIX`; `factory run 001` parks on `ASK-n`; `factory answer ASK-n "codex:gpt-5.4:xhigh"` writes `[workers]` to `config.toml` and persists `worker_config` on `state.json`; `factory answer ASK-n "proceed"` writes `worker_config` on `state.json` without creating/editing `config.toml`; configuring `[workers] provider = "codex"` with no `codex` on `PATH` makes preflight exit `1` with refusal under `✗` (fails if preflight ask, answer persistence, or binary refusal is removed).
- `factory setup workers` configures default + mixed per-role providers/efforts both interactively and via flags, and `factory setup --show --json` reports them (fails if per-role provider mixing in `setup.rs` is missing).

### 3. Verify — medium — blocked by: 1, 2
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q`

**Acceptance:** inner loop green, one commit.

### 4. Prove — at the checkpoint — blocked by: 3
Drive this row from [`prd.md`](prd.md) §9 QA plan at CP-1:
- **FR-1**: run `cargo test worker_config_mixes_roles_without_leaking_claude_defaults`, `cargo test run_flag_worker_persists_across_daemon_and_resume`, `cargo test preflight_asks_when_worker_provider_unset_and_refuses_missing_binary`, and `cargo test setup_workers_configures_mixed_roles`.

**Acceptance:** FR-1 row has passing test transcripts captured in checkpoint evidence and recorded in **Progress**.
