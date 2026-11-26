---
number: 027
slug: multi-provider-workers
title: "Multi-provider workers with per-role mixing and universal guardrails"
type: feature
created: 2026-11-25
author: Emiliano Tisato
summary: |
  Unlocks running unattended `factory` workers (`build`, `fix`, `merge`, `checkpoint`, `polish`) on any of the five providers from PRD-026 (`claude`, `codex`, `cursor`, `agy`, `opencode`) with per-role provider/model/effort mixing, one-off `factory run --worker` overrides, preflight ask-if-missing, universal `dec-2` `PATH` shims blocking `gh` and `git push`, per-provider JSONL stream normalizers, and provider-aware metrics in `factory.db` and `factory report`. Verified by a cross-vendor adversarial pass (`grok-4.7-high`) 2026-11-25 with all findings corrected inline; depends on PRD-026 shipping first.
from_idea: "2026-10-09-codex-or-cursor-as-build-workers"
---
> **History.** The code, `docs/reference` and `docs/decisions.md` win over this document.

# PRD-027 — Multi-provider workers with per-role mixing and universal guardrails

> **Interviewed 2026-11-25.** Every `file:line` below was verified against `main` at `e61273d`, then **re-verified by a cross-vendor adversarial pass (`grok-4.7-high`, 1137 s) on 2026-11-25 which caught ten real breaks**. Each correction is inline and dated. **Prerequisite: PRD-026 (`multi-provider-setup`) must ship first** — this PRD builds directly on PRD-026's `src/provider.rs`, user-only `[workers]` schema, `config::patch`, and `factory setup` wizard.

## From idea 2026-10-09-codex-or-cursor-as-build-workers

Graduated 2026-11-25. PRD-001 (`docs/archive/001-driver/prd.md:58`) rejected Codex/Cursor workers because their flags and streams differed and there was no evidence they were cheaper; the `docs/ideas/2026-10-09-codex-or-cursor-as-build-workers.md` note left them open until the metrics store could compare per-worker cost (*corrected 2026-11-25, adversarial pass: an earlier draft attributed the metrics-store deferral to PRD-001 directly*). Today `factory.db` (`runs.knobs`, `workers`, `factory report --per-worker`) records per-run knobs, hypotheses, and per-worker metrics, and coworkers adopting `factory` need to run unattended loops on their own agent subscriptions and mix providers per role.

## 1. Why now

Even after PRD-026 makes the cockpit skills, `factory setup`, and adversarial review provider-agnostic, every unattended worker spawned by the driver is still hardcoded to Claude Code:

- **Hardcoded `claude` invocation.** `src/worker.rs:221-240` spawns `Command::new("claude")` with Claude-only flags (`--output-format stream-json --verbose`, `--permission-mode bypassPermissions`, `--append-system-prompt-file`, `--session-id`, `--strict-mcp-config`, `--tools`, `--settings`). A teammate with Codex, Cursor, Antigravity, or OpenCode subscriptions cannot run `factory run`.
- **No per-role provider mixing or per-provider effort mapping.** Teams want to mix providers across roles (e.g., `codex` with `xhigh` reasoning effort on `build`/`fix`, `agy` or `claude` on `checkpoint`/`polish`) and test newly released models for a few PRDs without editing code.
- **Claude-only stream parsing, live activity, and subagent telemetry.** `src/stream.rs:8-35` parses only Claude NDJSON, `src/live/workers.rs:124-187` (`read_tail`) and `:189-265` (`wanted`, `assistant`, `label`, `outcome`) read only Claude stream lines, `src/metrics.rs:449-480` looks up `~/.claude/projects/` transcripts for every worker, and `src/cli/run.rs:225, :262-285` runs `claude --version` unconditionally at run start.
- **`dec-2` GitHub/push enforcement relies partially on Claude hooks.** Env scrubbing (`GH_TOKEN_VARS`, `GH_CONFIG_DIR` pointing to the empty `run_dir/gh-config` dir) and `push_off_env` (`src/worker.rs:23-85, :204-205, :250-262`) are portable, but blocking `gh` binaries, `git -c ... push`, and remotes with explicit `pushurl` currently relies on Claude's `--settings` deny rules and `PreToolUse` `factory hook` (`src/worker.rs:21, :97-112`), which non-Claude CLIs do not run.

**What this leaves alone.** Soft `PreToolUse` hygiene nudges (`factory hook` bare-grep and pack-read hints in `src/cli/hook.rs:18-85`) stay Claude-only (decided 2026-11-25). Named config profiles and Gemini CLI remain out of scope (see PRD-026 §11).

## 2. Blast radius

- **Prerequisite sequencing.** PRD-026 modifies `src/config.rs`, `src/cli/setup.rs`, `src/docs/preflight.rs`, `src/cli/answer.rs`, `src/lib.rs`, and `tests/harness/mod.rs`. **PRD-026 ships first**; PRD-027 starts from `main` after PRD-026 merges so there is zero merge conflict on `src/config.rs` and `src/cli/setup.rs`.
- **Users upgrading after PRD-027 without `[workers] provider` set.** A user whose `~/.config/factory/config.toml` has no `[workers] provider` (and no `FACTORY_WORKER_PROVIDER` env or `--worker` flag) hits one preflight `ASK-n` on their next **new** `factory run` (or exit `2` on `factory docs preflight`) to choose a default worker provider from detected CLIs; answering a provider writes `[workers]` to `config.toml` once and never asks again, while answering `proceed` records the first detected CLI on `RunState.worker_config` for **that run only** and leaves `config.toml` unset. Resumed runs (`Run::find_resumable`, `src/cli/run.rs:76-95`) read `RunState.worker_config` (or default to `claude` when `RunState.worker_config` is `None` on pre-upgrade runs), so an in-progress pre-upgrade run finishes on `claude` without asking even if `[workers] provider` is later set in `config.toml` (*clarified 2026-11-25, adversarial pass; renamed `RunState.worker_config` 2026-11-25 at slice entry because `RunState.workers` at `src/run.rs:118` is already `Vec<WorkerRef>`*).
- **Existing fixture tests (`tests/harness/mod.rs:580-631`).** `build_env` sets `FACTORY_WORKER_PROVIDER=claude` (alongside `FACTORY_CI_MODE=remote` and PRD-026's `FACTORY_REVIEW_PROVIDER=cursor`) so every existing test in `tests/worker.rs`, `tests/loop.rs`, `tests/checkpoints.rs`, `tests/closeout.rs`, `tests/merge.rs`, `tests/quota.rs`, and `tests/live.rs` runs untouched and green; new tests for unset-worker asks and non-Claude providers clear or override `FACTORY_WORKER_PROVIDER`.
- **SQLite migration (`src/db.rs:115-148`).** Adds one nullable column `("workers", "provider", "TEXT")` via `db::COLUMNS` (additive `ALTER TABLE ADD COLUMN` guarded by live column check). Pre-upgrade DB rows read `provider = None` and display cleanly in `factory report` without migration breakage.
- **External contracts.**
  - `runs.knobs` JSON gains `"provider"` on each `workers.<role>` object and `"provider_versions"` map (keeping `"claude_version"` populated when `claude` is among the run's configured providers for backward compatibility).
  - `factory report` (`knobs_text` at `src/cli/report.rs:732-768` and `--per-worker` at `:205-215`) includes each worker's provider (`<provider>:<model>/<effort>` when `provider` is present; legacy rows without `provider` render identically to today). `tests/knobs.rs:131-153` is **deliberately updated** to assert the provider-qualified format.
  - `factory live --json` (`src/live/workers.rs:32-58`) adds `"provider": "<name>"` to each worker entry; all existing fields remain unchanged (`null` when a provider stream omits them).

## 3. FR-1 — Per-role worker provider resolution, `factory run --worker`, preflight ask, and `factory setup workers`

**Today.** `src/config.rs:69-83` (extended by PRD-026's schema in `config.rs`) holds `[workers]` and `[workers.build|fix|checkpoint|polish]`, but `src/worker.rs:221` ignores any provider field and spawns `"claude"`. `RunArgs` (`src/cli/run.rs:16-33`) has no `--worker` flag, `preflight` has no worker-provider ask or binary check, and PRD-026's `factory setup workers` only prompts for Claude `model`/`effort`.

**Decided 2026-11-25.**

- **Provider-scoped resolution per role (`build`, `fix`, `checkpoint`, `polish`; `Job::Merge` uses `build` as at `src/scheduler.rs:764`)** (*clarified 2026-11-25, adversarial pass — resolves model/effort inheritance across mixed providers and Cursor effort normalization consistently with PRD-026 FR-2*):
  1. Resolve `provider`: `--worker` flag (`Source::Flag`) → `[workers.<role>].provider` (`Source::User`) → `[workers].provider` (`Source::User`) → `FACTORY_WORKER_PROVIDER` env (`Source::Default`, resolved outside `config::load`) → unset (triggers preflight ask).
  2. Resolve `model` and `effort` using the exact rule defined in PRD-026 FR-2:
     - `--worker` flag's `model`/`effort` (when given) → `[workers.<role>].model`/`effort` (when `Some`) → `[workers].model`/`effort` **only when the role's resolved provider equals `[workers]`'s effective provider** (so switching a role to `codex`, `agy`, or `cursor` never inherits a Claude `"opus"`/`"high"` default from `[workers]`) → `resolved_provider.default_model()` / `resolved_provider.default_effort()`.
     - When `resolved_provider.effort_levels().is_empty()` (`Provider::Cursor`), `effort` is always normalized to `None` (never passed as a CLI flag).
- **Persist resolved run workers on `RunState` (`src/run.rs:95-135`, `src/cli/run.rs:16-33, :76-95, :181-205`)** (*corrected 2026-11-25, adversarial pass; field named `worker_config` at slice entry because `RunState.workers` at `src/run.rs:118` holds `Vec<WorkerRef>`*):
  - Add repeatable `--worker <SPEC>` on `RunArgs`, accepting either `<provider>[:<model>[:<effort>]]` (all roles) or `<role>=<provider>[:<model>[:<effort>]]` (`role` in `build|fix|checkpoint|polish`). Refuse an OpenCode spec with no model (since `Provider::Opencode.default_model()` is empty, matching PRD-026 AD-12/AD-14), or a model failing `provider::is_model_name`.
  - Add `#[serde(default, skip_serializing_if = "Option::is_none")] pub worker_config: Option<ResolvedWorkers>` on `RunState` holding the concrete `{provider, model, effort}` resolved for each of the four roles (`build`, `fix`, `checkpoint`, `polish`).
  - Written **once** when a new run starts (or when its worker-provider `ASK-n` is answered — whether via a saved provider or `"proceed"`).
  - `scheduler::drive`, `checkpoint`, and `closeout` resolve every worker spawn from `run.state().worker_config` when `Some`; when `None` (strictly pre-PRD-027 in-progress runs), they fall back to `Provider::Claude` + loaded `config.workers` (model/effort), so pre-upgrade runs finish cleanly on `claude` regardless of later `config.toml` edits.
  - Passing `--worker` when resuming an existing run warns `--worker ignored: run <id> already exists` on stderr and leaves `RunState.worker_config` unchanged (matching `--ci` and `--hypothesis` at `src/cli/run.rs:207-215`).
- **Preflight ask-if-missing and binary verification (`src/docs/preflight.rs`, `src/cli/run.rs`, `src/cli/answer.rs`):**
  - Add `pub const WORKER_ASK_PREFIX: &str = "worker provider not set in config.toml"` and `preflight::workers(loaded, flag_overrides, pf)` called from `preflight::cli` and `cli::run::run`.
  - When any role has no resolved provider: push `pf.ask` starting with `WORKER_ASK_PREFIX`, listing detected installed CLIs (formatting `opencode:<model>` since it has no default model, matching PRD-026 AD-13), plus `proceed` (which uses the first detected CLI with a default model for this run only without editing `config.toml`).
  - In `src/cli/answer.rs` (`answer_ask`): answering `ASK-n` with `<provider>[:<model>[:<effort>]]` persists `[workers]` (`provider`, `model`, optional `effort`) via `config::patch`, writes the resolved `ResolvedWorkers` onto `RunState.worker_config` (and updates `runs.knobs`), and wakes the run; `"proceed"` resolves `ResolvedWorkers` using the first detected CLI that has a default model, writes it onto `RunState.worker_config` (surviving daemon launch and resume) without modifying `config.toml` (refusing cleanly with exit `1` if none is installed); invalid answers (including bare `opencode` with no model) exit `1` leaving the ask open.
  - When all roles have a resolved provider: verify each distinct provider's binary resolves on `PATH` via `provider::resolve_binary` and its resolved model is non-empty. If any configured worker provider binary is missing or has an empty model, push to `pf.refuse` (`"worker provider <p> (<bin>) not found on PATH — install it or run `factory setup workers`"`) so a run fails at preflight rather than crashing mid-slice; otherwise push `pf.ok` summarizing the resolved worker config (compact when all roles match, per-role when mixed).
- **Activate provider & effort mixing in `factory setup workers` (PRD-026 FR-3):**
  - Step 1: pick default `[workers]` provider (from detected CLIs), model (live-discovered/curated + free text), and effort (provider-specific effort choices from `src/provider.rs`, skipped when the provider has none).
  - Step 2: *"Override any role (`build`, `fix`, `checkpoint`, `polish`) with a different provider/model/effort? [y/N]"* — loops over selected roles to set or clear per-role overrides cleanly, allowing full per-role mixing.
  - Non-interactive flags gain `--provider <p>` on `factory setup workers [--role <role>] --provider <p> [--model <m>] [--effort <e>]`.

**Accepted cost.** A user who only ever wants one provider still sees one `[y/N]` question in `factory setup workers` (pressing Enter skips role overrides). Persisting concrete `ResolvedWorkers` on `RunState.worker_config` means editing `config.toml` mid-run does not hot-swap providers for an already-open run; stop and start a new run to switch mid-feature.

**Acceptance.** (1) `config` resolves mixed roles including `[workers]` = `claude:opus:high` + `[workers.build]` = `codex` (no model/effort set) → `codex:gpt-5.4/high` and `[workers.fix]` = `cursor` → `cursor:grok-4.7-high` with `effort: None` (never leaking `"opus"` or `"high"` onto Cursor); (2) `factory run 001 --worker build=codex:gpt-5.4:xhigh` persists on `state.json` (`worker_config.build`) across daemon launch and resume, `--worker` on resume warns and is ignored, and a pre-upgrade `state.json` with `worker_config: None` resumes on `claude` even when `config.toml` sets `[workers] provider = "codex"`; (3) with `FACTORY_WORKER_PROVIDER` unset and no `[workers] provider`, `factory docs preflight 001` exits `2`, `factory run 001` parks on `ASK-n`, `factory answer ASK-n "codex:gpt-5.4:xhigh"` writes `[workers]` to `config.toml` and wakes the run, `factory answer ASK-n "proceed"` writes `RunState.worker_config` without modifying `config.toml` and survives daemon spawn, and a missing configured worker binary refuses preflight with exit `1`; (4) `factory setup workers` configures both default and mixed per-role providers/efforts interactively and via flags.

## 4. FR-2 — Universal `dec-2` GitHub and `git push` guardrails

**Today.** `src/worker.rs:20-85, :204-205, :250-262` scrubs `GH_TOKEN_VARS`, points `GH_CONFIG_DIR` to the private empty directory `run_dir/gh-config` (mode `0o700`, pinned by `tests/worker.rs:125-130` — *corrected 2026-11-25, adversarial pass: an earlier draft ambiguously called `GH_CONFIG_DIR` "empty"*), and injects `GIT_CONFIG_COUNT` + `url.factory-no-push://.pushInsteadOf`, then adds Claude-only `--settings` (`DENY_RULES` + `PreToolUse` `factory hook` at `:97-112`) to catch `gh`, `git -c ... push`, and remotes with an explicit `pushurl`. Non-Claude workers do not read Claude's `--settings`, leaving explicit `pushurl` and `gh` binaries reachable on `PATH`.

**Decided 2026-11-25.** Enforce `docs/decisions.md#dec-2` at the OS/process level for **every** worker provider via `run_dir/bin/` `PATH` shims:

- Add `worker::ensure_shims(run_dir: &Path) -> Result<PathBuf>` creating `run_dir/bin/` (mode `0o700`, `paths::private_dir_all`) with two executable scripts (mode `0o700`, written atomically per `docs/decisions.md#dec-3`):
  1. **`gh`**: prints `hook::NEVER_HINT` to stderr and exits `126`.
  2. **`git`**: wraps the real `git` binary resolved by the driver from the outer `PATH` (skipping `run_dir/bin` to prevent recursion). Scans argv skipping global git options (`-C <dir>`, `-c <k=v>`, `--git-dir[=]<p>`, `--work-tree[=]<p>`, `--namespace[=]<n>`, `--exec-path[=]<p>`, `--super-prefix[=]<p>`, `--config-env[=]<v>`, `--bare`, `--no-pager`, `-p`, `--paginate`, `--no-replace-objects`, `--literal-pathspecs`, `--glob-pathspecs`, `--noglob-pathspecs`, `--icase-pathspecs`, `--no-optional-locks`); if the git subcommand is `push` or `send-pack`, prints `hook::NEVER_HINT` to stderr and exits `126`; otherwise `exec "$REAL_GIT" "$@"` preserving exit code, stdin, stdout, and stderr.
- In `Worker::spawn`, prepend `run_dir/bin` to the worker child's `PATH` (after applying any lane env, matching `GH_TOKEN_VARS` / `push_off_env` ordering at `src/worker.rs:250-262`) for **all** providers.
- Keep existing env scrubbing (`GH_TOKEN_VARS`, `GH_CONFIG_DIR = run_dir/gh-config` mode `0o700`, `push_off_env`) unchanged for all providers, and keep `--settings` (`DENY_RULES` + `factory hook`) on `Provider::Claude` so Claude workers retain soft `PreToolUse` hygiene nudges (`bare grep` → `factory ctx`, pack-read hints).
- Keep `worker::scrubbed` (`CLAUDECODE` / `CLAUDE_*`) for Claude workers and avoid leaking outer cockpit session env vars into child workers.

**Accepted cost.** The `git` shim adds one shell `exec` (~1 ms) per `git` invocation inside workers, and a worker invoking `/usr/bin/git push` by absolute path bypasses the `PATH` shim (still caught by `push_off_env` for every normal remote and by `GH_TOKEN_VARS` removal + empty `run_dir/gh-config` dir for HTTPS/GitHub auth). Soft `PreToolUse` nudges (`factory hook` bare-grep / pack-read) are not ported to non-Claude workers (decided 2026-11-25).

**Acceptance.** Integration tests prove that under a spawned worker's environment (for both `claude` and a non-Claude provider): (1) `gh pr list` and `gh api ...` exit `126` with `NEVER_HINT`; (2) `git push`, `git -c url.https://example.invalid/.pushInsteadOf=x push`, and `git push` against a remote with an explicit `pushurl` all exit `126` with `NEVER_HINT`; (3) normal `git status`, `git add`, `git commit`, `git diff`, and `git -C <dir> log -1` succeed transparently through the shim; (4) `run_dir/bin` and `run_dir/gh-config` keep mode `0o700` (with `tests/worker.rs:125-130` unchanged and green).

## 5. FR-3 — Worker spawn adapters for all five providers

**Today.** `Worker::spawn` (`src/worker.rs:188-287`) takes `&config::Worker` (`model`, `effort`), always writes `<session_id>.settings.json`, always invokes `Command::new("claude")`, and passes `--append-system-prompt-file <pack_path>`. All five call sites (`src/scheduler.rs:760`, `src/checkpoint.rs:275, :707`, `src/closeout.rs:268, :2050`) pass `&cfg.workers.<role>`.

**Decided 2026-11-25.**

- Change `Worker::spawn` to take `&config::ResolvedWorker` (`provider: Provider`, `model: String`, `effort: Option<String>`), resolve the provider binary via `provider::resolve_binary`, and build the exact child `Command` per provider (*fixed 2026-11-25, adversarial pass — all five argv and prompt-delivery contracts are settled with no open forks*):
  - **Prompt & pack delivery:**
    - **`claude`**: unchanged (`-p <pack.prompt>`, `--output-format stream-json --verbose`, `--permission-mode bypassPermissions`, `--append-system-prompt-file <pack_path>`, `--session-id <session_id>`, `--model <model>`, `--effort <effort>` when `Some`, narrowed `--strict-mcp-config --disable-slash-commands --tools Bash,Read,Edit,Write` for `build|fix|merge`, `--settings <settings_file>`).
    - **Non-Claude providers:** always write `workers/<session_id>.prompt.md` (mode `0o600`, containing the pack body followed by `\n---\n\n<pack.prompt>\n`) so large packs (`prd_max_kb = 120` + slice + code excerpts) never hit Linux `ARG_MAX` limits:
      - **`codex`**: `codex exec --json -m <model> [-c model_reasoning_effort=<effort>] --dangerously-bypass-approvals-and-sandbox -` with `stdin` connected to `workers/<session_id>.prompt.md` (`Stdio::from(File)`).
      - **`cursor`**: `<cursor-agent|agent> -p --force --trust --output-format stream-json --stream-partial-output --model <model> --add-dir <workers_dir> "Read and follow all instructions in <abs_prompt_md>."` (no effort flag: `effort` is always `None` for Cursor).
      - **`agy`**: `agy -p --output-format stream-json --dangerously-skip-permissions --model <model> [--effort <effort>] [--disable-slash-commands when narrowed(role)] --add-dir <workers_dir> "Read and follow all instructions in <abs_prompt_md>."`.
      - **`opencode`**: `opencode run --standalone --format json -m <model>[#<effort>] -f <abs_prompt_md> "Read and follow all instructions in the attached prompt file."` (*updated 2026-11-25 at slice entry to match PRD-026 AD-11 / `src/review.rs:969-985`, verified against live `opencode v2.4.1`: effort is `#<effort>` on `-m`, `--standalone` ensures timeout/kill reaps the worker cleanly, and `-f` attaches the prompt file outside the repo*).
  - Every provider keeps: `current_dir(&cwd)`, `stdout` → `workers/<session_id>.jsonl` (mode `0o600`), `stderr` → `workers/<session_id>.stderr` (mode `0o600`), `FACTORY_PACK`, `FACTORY_REPO`, `FACTORY_BOARD`, `FACTORY_EVIDENCE`, lane env, `GH_TOKEN_VARS` removal, `GH_CONFIG_DIR = run_dir/gh-config`, `push_off_env`, and prepended `run_dir/bin` `PATH`.
- Store `pub provider: Provider` on `Worker`, add `#[serde(default, skip_serializing_if = "Option::is_none")] pub provider: Option<Provider>` on `WorkerRef` (`src/run.rs:143-157`), and pass `provider` to `Worker::exit` / `summary_from_disk(run_dir, session_id, provider)` so exits and crash-recovery reads use the right stream parser.
- Update `Worker::spawn` error message (`src/worker.rs:269-272`) to name the actual provider binary instead of hardcoding `"claude"`.

**Accepted cost.** `cursor`, `agy`, and `opencode` spend one initial Read tool turn to read `workers/<session_id>.prompt.md` instead of receiving the entire pack on argv; that eliminates `ARG_MAX` failures and CLI stdin quirks uniformly across those three harnesses. Non-Claude workers in `build|fix|merge` roles do not receive Claude's `--tools Bash,Read,Edit,Write --strict-mcp-config` narrowing (`agy` receives `--disable-slash-commands`); they still run in isolated lane worktrees with universal `dec-2` shims and the same pack instructions.

**Acceptance.** Integration tests with stub CLIs in `tests/worker.rs` prove: (1) each of the 5 providers spawns with its exact argv above, model, effort flag when the provider supports effort and `Some` (omitted when `None`, and never emitted for `cursor`), cwd, `FACTORY_*` env, and `0o600` stream/stderr/prompt files; (2) a single multi-slice run with mixed roles (e.g., `codex` on `build`, `agy` on `checkpoint`, `claude` on `polish`) completes end-to-end through `scheduler::drive` and records each worker's provider on `state.json` and `factory.db`; (3) large packs (> 150 KB) never fail with `E2BIG` on any non-Claude provider.

## 6. FR-4 — Per-provider stream normalizers, rate-limit detection, and graceful telemetry

**Today.** `src/stream.rs:8-35` parses only Claude NDJSON into `Summary` (keeping unknown line kinds as `StreamEvent::Unknown` at `:33-34` and unknown fields as `Value` at `:1`). `src/worker.rs:418-427` calls `stream::parse(&text)`, `src/live/workers.rs:124-187` (`read_tail`) and `:189-265` (`wanted`, `assistant`, `label`, `outcome` reading `BLOCK:` at `:257-260`) parse Claude JSONL directly, and `src/metrics.rs:449-480` reads `~/.claude/projects/` subagent transcripts for every worker (called from `src/scheduler.rs:1862`, `src/checkpoint.rs:1177`, `src/closeout.rs:2449`, and `src/cli/report.rs:835`).

**Decided 2026-11-25.**

- **Provider-dispatched stream parsing (`src/stream.rs`):**
  - Add `pub fn parse_for(provider: Provider, text: &str) -> (Vec<StreamEvent>, Summary)` and keep `pub fn parse(text: &str)` delegating to `parse_for(Provider::Claude, text)`.
  - Each normalizer populates `Summary` (`has_result`, `result_text`, `status_hint`, `is_error`, `usage`, `cost_usd`, `num_turns`, `duration_ms`, `model_id`, `tool_uses`, `rate_limit`, `events`, `bad_lines`):
    - **`claude`**: existing parser unchanged.
    - **`codex`**: parses `codex exec --json` JSONL — extracts final `item.completed` `agent_message` text into `result_text` (driving `StatusHint::from_result` and `Findings::from_result`) and sets `has_result = true`, extracts `turn.completed.usage` (`input_tokens`, `cached_input_tokens` → `cache_read_input_tokens`, `output_tokens`), maps `command_execution` / `file_change` items into `tool_uses`, and maps `error` / `turn.failed` into `is_error = true` and `has_result = true`.
    - **`cursor`**: parses `cursor-agent --output-format stream-json` — extracts `assistant` text/tool calls, `tool_call` events, `result` (`result`, `is_error`, `duration_ms`, `usage` when present).
    - **`agy`**: parses `agy --output-format stream-json` — extracts `system`/`assistant`/`tool_use`/`tool_result`/`result` events, `usage`, `model_id`, `duration_ms`.
    - **`opencode`**: parses `opencode run --format json` — extracts `text` (`part.text`) into `result_text`, `tool_use` (`part.tool`/`part.state`) into `tool_uses`, `step_finish` (`part.tokens`/`part.cost`) into `usage`/`cost_usd`, and `error` (`error.message`/`data.message`) into `is_error` (*harmonized 2026-11-25 with PRD-026 FR-4*).
  - **Rate-limit contract for `quota::Limit::from` (`src/quota.rs:38-67`)** (*corrected 2026-11-25, adversarial pass*): whenever any non-Claude provider stream reports HTTP 429, `"rate limit"`, or `"usage limit"` (case-insensitive) in an error/failure event, the normalizer sets `summary.rate_limit = Some(RateLimit { status: Some("rejected".into()), resets_at, utilization_pct: None })` (plus `summary.has_result = true`, `summary.is_error = true`, and `summary.result_text = Some(err_msg)` if no assistant message was emitted), guaranteeing `quota::Limit::from(&summary)` (`src/quota.rs:39-49`) detects the limit and triggers `RunPaused`.
  - Unrecognized event types and fields never panic or fail parsing (matching `StreamEvent::Unknown` at `src/stream.rs:33-34` — *corrected 2026-11-25, adversarial pass*).
- **Graceful degradation contract:**
  - **Budgets (`src/quota.rs:89-134`):** when a provider stream omits token usage, `Summary.usage` is `None`, stored as `NULL` in `workers`; `quota::weigh_own` already maps `None → 0` (`src/quota.rs:97-103`), so runs never crash or false-park on unmeasured tokens.
  - **Subagents (`src/metrics.rs:449-480` and `src/cli/report.rs:835-845`):** `metrics::record_subagents` takes `provider: Provider`; for `Provider::Claude` it reads `~/.claude/projects/` transcripts as today; for non-Claude providers it writes `None` (unmeasured) to the subagent columns and **never** looks up `~/.claude/projects/` or emits a spurious `EventKind::SubagentsUnread` cockpit warning.
  - **Live tail (`src/live/workers.rs:23-60, :124-265`):** passes `provider = w.provider.unwrap_or(Provider::Claude)` into `read_tail`, adds `"provider": provider.as_str()` to each `workers[]` JSON object, and extracts `doing`, `narration`, and `outcome` (`STATUS:` and `BLOCK:` per `src/live/workers.rs:250-265` and `assets/contract-slice.md:29` — *corrected 2026-11-25, adversarial pass*) for each provider's JSONL shape (leaving `doing`/`narration`/`running` as `null` when a stream omits them).

**Accepted cost.** Non-Claude workers whose CLI does not report per-subagent token breakdowns record `sub` / `subagents` as `—` (unmeasured) in `factory report`, and budget caps (`budget_weighted_tokens`) count only the tokens a provider's stream actually reports. Building provider-specific private-transcript scrapers (equivalent to `~/.claude/projects/`) lost because other CLIs do not share a stable on-disk subagent transcript format.

**Acceptance.** (1) Unit tests in `src/stream.rs` against fixture JSONL streams for all 5 providers verify extraction of `STATUS: done|blocked|failed`, `FINDINGS:`, token usage, `tool_uses`, and 429/rate-limit detection through `quota::Limit::from(&summary)` returning `Some(Limit { .. })`; (2) a non-Claude worker run with no `~/.claude` dir completes with zero `SubagentsUnread` events and `subagents = None` in `factory.db`; (3) `factory live <id> --json` returns `"provider"` on every worker entry and extracts `outcome` (`STATUS:` / `BLOCK:`) and `narration`/`doing` from non-Claude streams without erroring on omitted fields.

## 7. FR-5 — Provider observability in `factory.db`, `runs.knobs`, and `factory report`

**Today.** `src/db.rs:28-51, :115-148, :554-580` has no `provider` column on `workers`. `src/cli/run.rs:225-226, :262-285` unconditionally runs `claude --version` and writes `"claude_version"` into `runs.knobs` (emitting a stderr warning on machines without `claude`). `src/config.rs:259-283` records only `model`, `effort`, and `source` per role in `knobs`, and `src/cli/report.rs:205-215, :732-768` prints no provider name.

**Decided 2026-11-25.**

- **SQLite schema (`src/db.rs`):**
  - Append `("workers", "provider", "TEXT")` to `db::COLUMNS` (`src/db.rs:115-148`), add `pub provider: Option<String>` to `WorkerRow`, and persist `provider.as_str()` in `insert_worker` (`:554-580`).
- **Run knobs & version probing (`src/config.rs:259-283`, `src/cli/run.rs:217-285`):**
  - In `config::knobs`, include `"provider"` (and `"provider_source"` when distinct from `model`'s source) on each `workers.<role>` entry, include `"review": {"provider": ..., "model": ..., "source": ...}`, and accept `provider_versions: &BTreeMap<Provider, String>` written under `"provider_versions"`. Keep `"claude_version"` populated from `provider_versions.get(&Provider::Claude)` (or `null` when `claude` is not used) for backward compatibility.
  - Replace unconditional `claude_version()` (`src/cli/run.rs:262-285`) with `probe_provider_versions(used_providers, err)` that runs `<bin> --version` (with a 5 s timeout) **only for the distinct providers actually configured on this run** — so a coworker running Codex-only on a machine without `claude` never sees a spurious `claude --version` warning.
- **Report formatting (`src/cli/report.rs:205-215, :732-768`):**
  - `knobs_text`: when `w["provider"]` is present, format each role as `<role> <provider>:<model>/<effort> <src>` (omitting `/<effort>` when effort is empty/null, e.g. Cursor); when `w["provider"]` is absent (pre-PRD-027 DB rows), keep the legacy `<role> <model>/<effort> <src>` format unchanged. Render versions from `provider_versions` (e.g. `codex 0.172.0 · claude 2.1.296`) when present, falling back to `claude <ver>` on legacy rows.
  - `--per-worker` line (`src/cli/report.rs:205-215`): include `provider` when `Some` (e.g. `worker 1 build codex slice 01 ...`).
  - Update `tests/knobs.rs:131-153` to assert the updated `knobs_text` and `--per-worker` format while adding a unit test in `src/cli/report.rs` proving legacy rows without `provider` / `provider_versions` still render identically.

**Accepted cost.** Existing scripts grepping `knobs:` text from `factory report` see `<provider>:<model>/<effort>` on new runs instead of `<model>/<effort>`; JSON consumers (`factory report --json`) remain additive (`provider` and `provider_versions` added alongside existing keys).

**Acceptance.** (1) `Db::open` on a pre-PRD-027 SQLite file adds `workers.provider` without error and reads legacy rows with `provider: None`; (2) a Codex-only run on a `PATH` with **no** `claude` binary produces zero `claude --version` warnings on stderr and records `provider_versions.codex` in `runs.knobs`; (3) a mixed-provider run records each worker's `provider` in `workers`, `runs.knobs`, `factory report`, and `factory report --per-worker --json`; (4) `knobs_text` renders both new multi-provider rows and legacy pre-PRD-027 rows cleanly.

## 8. Decisions log

No `OPEN-n`. Every fork was settled on 2026-11-25 and lives in its FR:

- Per-role provider/model/effort mixing is first-class in `~/.config/factory/config.toml` and `factory setup workers`, plus repeatable `factory run --worker` overrides for one-off experiments (§3).
- Model/effort inheritance is scoped to matching providers so switching a role to another provider never leaks Claude defaults (`opus`/`high`) onto non-Claude roles, and `cursor` effort always normalizes to `None` (§3).
- Concrete resolved `RunState.workers` is persisted per run so `"proceed"` survives detached daemons and pre-upgrade runs (`workers: None`) resume on `claude` regardless of later `config.toml` edits (§3).
- Non-Claude `dec-2` enforcement uses `run_dir/bin/` `PATH` shims (`gh` + `git` push blocker) alongside env scrubbing, `GH_CONFIG_DIR = run_dir/gh-config`, and `push_off_env`; soft `PreToolUse` nudges stay Claude-only (§4).
- Non-Claude prompt delivery uses stdin for `codex` (`codex exec … -`) and `workers/<session_id>.prompt.md` + `--add-dir` pointer for `cursor`, `agy`, and `opencode` (§5).
- Non-Claude subagent telemetry degrades to `None` (unmeasured) with zero spurious `SubagentsUnread` warnings (§6).
- Version probing at run start probes only the providers actually used by that run, never assuming `claude` is installed (§7).

## 9. QA plan

**Gate**: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test && claude plugin validate mods/cockpit && claude plugin test mods/cockpit` *(declared in `docs/project.toml`)*
**Surface**: `factory` CLI (`run`, `docs preflight`, `answer`, `setup workers`, `live --json`, `report`), `~/.config/factory/config.toml`, `factory.db`, and worker subprocess sandboxes. No GUI: every FR is proven below it.
**Harness**: `tests/harness/mod.rs` (`Fixture`), `tests/stubs/` (adding multi-provider worker stubs & recorded JSONL fixtures for `codex`, `cursor-agent`, `agy`, `opencode`), `tests/worker.rs`, `tests/preflight.rs`, `tests/knobs.rs`, `tests/live.rs`, `src/stream.rs`, `src/db.rs`, `src/cli/report.rs`.
**Seed state**: fixture repo with stub binaries for all 5 providers; fixture `PATH` without `claude` (for Codex-only run); fixture repo with `FACTORY_WORKER_PROVIDER` unset (for preflight ask); fixture repo with remote carrying explicit `pushurl` (for `git` shim test).

| FR | Proof | Evidence | Human? |
|---|---|---|---|
| FR-1 | run `worker_config_mixes_roles_without_leaking_claude_defaults`, `run_flag_worker_persists_across_daemon_and_resume`, `preflight_asks_when_worker_provider_unset_and_refuses_missing_binary`, and `setup_workers_configures_mixed_roles` | test transcript | no |
| FR-2 | run `universal_path_shims_block_gh_and_git_push_including_pushurl_while_passing_normal_git` | test transcript | no |
| FR-3 | run `all_five_worker_providers_spawn_with_expected_flags_and_large_packs` and `mixed_provider_run_completes_slices_checkpoint_and_polish` | test transcript | no |
| FR-4 | run `stream_normalizers_parse_status_findings_usage_and_rate_limits_for_all_five_providers`, `non_claude_worker_skips_claude_subagent_lookup_without_warning`, and `live_json_reads_non_claude_worker_streams` | test transcript | no |
| FR-5 | run `codex_only_run_without_claude_on_path_records_knobs_and_reports_cleanly` and `report_knobs_renders_multi_provider_and_legacy_rows` | test transcript | no |

No row needs a human, so no `qa-guide.md` is generated.

## 10. Slices

Four slices on one branch, `feat/multi-provider-workers`, one PR, one commit per slice, Prove driven at the checkpoints.

- **01 — Worker provider resolution, `RunState.worker_config`, preflight ask, and `setup workers`** (FR-1, spine). `src/config.rs`, `src/run.rs`, `src/docs/preflight.rs`, `src/cli/run.rs`, `src/cli/answer.rs`, `src/cli/setup.rs`, `tests/harness/mod.rs` (`FACTORY_WORKER_PROVIDER=claude`), `tests/preflight.rs`, and `tests/setup.rs`. Independent and the prerequisite for slices 03 and 04.
- **02 — Per-provider stream normalizers, rate limits, and subagent telemetry** (FR-4 core). `src/stream.rs` (`parse_for(Provider, &str)` + 429/rate-limit normalization into `quota::Limit::from`) and `src/metrics.rs` (`record_subagents_for(..., provider: Provider)`). Independent of 01 (zero file overlap).
- **03 — Universal `dec-2` `PATH` shims, multi-provider worker spawn adapters, and live tail** (FR-2, FR-3, FR-4 wiring, highest-risk). `src/worker.rs`, `src/scheduler.rs`, `src/checkpoint.rs`, `src/closeout.rs`, `src/live/workers.rs`, `tests/worker.rs`, `tests/live.rs`, and `tests/stubs/` (worker stubs & JSONL recordings for `codex`, `cursor-agent`, `agy`, `opencode`). **Depends on 01 and 02**.
- **04 — Provider observability in `factory.db`, `runs.knobs`, and `factory report`** (FR-5). `src/db.rs` (`workers.provider` column), `src/worker.rs` (passing `provider` into `insert_worker`), `src/config.rs` (`knobs` with `provider` & `provider_versions`), `src/cli/run.rs` (`probe_provider_versions`), `src/cli/report.rs` (`knobs_text`, `--per-worker`, `recount`), and `tests/knobs.rs`. **Depends on 03** (exercises a real Codex-only run with no `claude` binary on `PATH`).

**Order and checkpoints.** Wave 1 runs **01** and **02** in parallel; **CP-1** lands right after both (`checkpoint_every = 2`), proving FR-1 and the FR-4 stream/rate-limit normalizers before anything builds on them. Wave 2 runs **03** alone (highest-risk worker runtime & universal shims), followed by **04** (observability & full Codex-only / mixed-provider report proof); **CP-2** (final checkpoint) proves FR-2, FR-3, FR-4 live/subagents, FR-5, and re-proves all rows.

**No `qa-guide.md`:** no row in §9 QA plan is marked `Human? = yes`, so nothing is generated for human QA.

## 11. Non-goals

- **Soft `PreToolUse` hygiene nudges (`bare grep` → `factory ctx`, pack-read hints) on non-Claude workers.** Not handled (2026-11-25): decided with owner; `dec-2` hard safety is enforced universally via `PATH` shims + env + `push_off_env`, while soft tool-call nudges stay Claude-only where `PreToolUse` hooks exist. Revisit if retros show non-Claude workers wasting significant tokens on bare greps or pack re-reads and those CLIs stabilize a hook contract.
- **Cross-provider private subagent transcript scraping.** Not handled (2026-11-25): only Claude writes `~/.claude/projects/<slug>/<session>/subagents/`; non-Claude subagent columns record `None` cleanly. Revisit if Codex/Cursor/Antigravity/OpenCode expose a stable subagent usage file or emit subagent usage in their JSONL stream.
- **Tool allowlist narrowing (`--tools Bash,Read,Edit,Write --strict-mcp-config`) on non-Claude build/fix/merge workers.** Not handled (2026-11-25): no uniform flag exists across Codex/Cursor/OpenCode (`agy` gets `--disable-slash-commands`). Revisit if a provider's MCP tools cause unwanted side effects during slice builds.
- **Changing `--worker` overrides on an already-created run via resume.** Not handled (2026-11-25): matches `--ci` and `--hypothesis` (`src/cli/run.rs:207-215`); stop and start a new run to switch providers mid-feature. Revisit if a rate-limited run needs to fail over to another provider mid-flight without stopping.
- **Automatic provider failover on rate-limit pause.** Not handled (2026-11-25): a rate-limited worker pauses until `resets_at` (`src/quota.rs:140-174`) rather than silently switching to another vendor mid-slice. Revisit if teams want automatic fallback providers on quota exhaustion.

## 12. Verification notes

Checked against `main` at `e61273d`, 2026-11-25, then re-verified the same day by a cross-vendor adversarial pass (`factory review second --doc`, `grok-4.7-high`, 1137 s, log `~/.local/share/factory/reviews/20261125-203417`). Ten findings were caught and corrected inline above. **Re-verify at slice entry (and again after PRD-026 merges, since PRD-026 moves line numbers in `src/config.rs`, `src/docs/preflight.rs`, `src/cli/answer.rs`, and `tests/harness/mod.rs`).**

- `src/worker.rs:20-85` (`DENY_RULES`, `GH_TOKEN_VARS`, `KEEP_CLAUDE_VARS`, `scrubbed`, `push_off_env`, `gh_config_dir`), `:92-116` (`narrowed`, `settings`), `:188-287` (`Worker::spawn` hardcoded to `Command::new("claude")`, `GH_CONFIG_DIR = run_dir/gh-config` at `:204-205, :254`), `:418-427` (`summary_at`, `summary_from_disk`).
- `src/scheduler.rs:760-768` (`Worker::spawn` for `Build` and `Merge` using `&cfg.workers.build`), `:1862` (`metrics::record_subagents`).
- `src/checkpoint.rs:275-283` (`Worker::spawn` for `Checkpoint`), `:707` (`Worker::spawn` for `Fix`), `:1177` (`metrics::record_subagents`).
- `src/closeout.rs:268-276` (`Worker::spawn` for `Polish`), `:2050` (`Worker::spawn` for `CiFix`), `:2449` (`metrics::record_subagents`).
- `src/stream.rs:1, :8-35` (`StreamEvent` including `Unknown` at `:33-34`), `:117-185` (`StatusHint`, `Findings`), `:215-240` (`Summary`), `:242-255` (`RateLimit`).
- `src/quota.rs:37-68` (`Limit::from`: requires non-`allowed*` `rate_limit.status`, or `has_result && is_error` with 429 / `"usage limit"` / `"rate limit"`), `:78-112` (`weigh`, `weigh_own`, `weigh_sub` mapping `None → 0`).
- `src/metrics.rs:427-480` (`read_subagents`, `record_subagents` reading `paths::claude_config_dir()`).
- `src/live/workers.rs:23-60` (`workers` JSON), `:124-187` (`read_tail`), `:189-248` (`wanted`, `assistant`, `label`), `:250-265` (`outcome` reading `STATUS:` and `BLOCK:`); `assets/contract-slice.md:29` (`BLOCK:` marker).
- `src/run.rs:94-157` (`RunState`, `WorkerRef`).
- `src/cli/run.rs:16-33` (`RunArgs`), `:76-95` (`find_resumable`), `:181-205` (`daemon` reloading `config.toml`), `:207-285` (`flags_ignored`, `record_start`, `claude_version`).
- `src/db.rs:28-51` (`workers` table), `:115-148` (`COLUMNS`, 32 entries), `:554-616` (`insert_worker`, `finish_worker`).
- `src/cli/report.rs:188-215` (`--per-worker` line), `:732-768` (`knobs_text`), `:835-845` (`recount` subagents).
- `tests/harness/mod.rs:580-631` (`build_env`), `tests/stubs/claude` (stub worker script), `tests/worker.rs:58-135` (`GH_CONFIG_DIR` mode `0o700` empty dir), `tests/knobs.rs:59-159`.
- `docs/archive/001-driver/prd.md:58` (PRD-001 rejected Codex/Cursor due to different flags/stream and no cost evidence).
- Verified real worker CLI flags on host 2026-11-25: `claude 2.1.332`, `codex-cli 0.172.0` (`codex exec --json -m <model> -c model_reasoning_effort=<effort> --dangerously-bypass-approvals-and-sandbox -`), `cursor-agent`/`agent` `2026.11.22-e9042b2` (`-p --force --trust --output-format stream-json --stream-partial-output --model <model> --add-dir <dir>`), `agy 1.5.17` (`-p --output-format stream-json --dangerously-skip-permissions --model <model> --effort <low|medium|high|xhigh|max> [--disable-slash-commands] --add-dir <dir>`). Note: `opencode` was not locally installed on this host (unpopulated `mise` shim); confirm `opencode run --format json` flags and JSONL schema against a live `opencode` binary at slice entry.
