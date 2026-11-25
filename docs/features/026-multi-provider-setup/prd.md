---
number: 026
slug: multi-provider-setup
title: "Multi-provider setup, portable cockpit skills, and adversarial review"
type: feature
created: 2026-11-25
author: Emiliano Tisato
summary: |
  Introduces a unified five-provider registry (`claude`, `codex`, `cursor`, `agy`, `opencode`), moves `[workers]` config to user-only `~/.config/factory/config.toml` alongside `[cockpit]`, `[review]`, and `[ci]`, adds an interactive/scriptable `factory setup` wizard, makes `factory review second` and preflight work across all five providers (plus `none`), and makes the bundled cockpit skills portable across every harness. Verified by a cross-vendor adversarial pass (`grok-4.7-high`) 2026-11-25 with all findings corrected inline; companion to PRD-027.
---

# PRD-026 — Multi-provider setup, portable cockpit skills, and adversarial review

> **Interviewed 2026-11-25.** Every `file:line` below was verified against `main` at `e61273d`, then **re-verified by a cross-vendor adversarial pass (`grok-4.7-high`, 860 s) on 2026-11-25 which caught nine real breaks**. Each correction is inline and dated. Companion to **PRD-027** (`multi-provider-workers`), which builds directly on the provider registry, config schema, and setup wizard defined here.

## 1. Why now

Coworkers adopting `factory` do not all share the same coding-agent subscriptions or interactive harnesses — across the team people use **Claude Code (`claude`)**, **Codex CLI (`codex`)**, **Cursor (`cursor-agent` / `agent`)**, **Antigravity (`agy`)**, and **OpenCode (`opencode`)**. Today the repo blocks non-Claude / non-Grok users in four concrete places:

- **Adversarial review is hardcoded to `cursor-agent` and Grok.** Although `src/config.rs:60-67, :97-106` exposes `[review] agent` (defaulting to `"cursor-agent"`) and `model` (defaulting to `"grok-4.7-high"`), `src/review.rs:18-25` hardcodes Cursor-specific `FLAGS` (`--trust --mode ask --output-format stream-json --stream-partial-output`), `src/review.rs:573-616` probes with `<agent> --list-models` and checks `"Authentication required"`, `src/review.rs:643-671` parses only Cursor/Claude stream-json, and `src/docs/preflight.rs:374-377` interpolates `run \`{} login\`` from `cfg.agent` (assuming every CLI has a `login` subcommand) while hardcoding `"one Claude agent"` as the fallback (*corrected 2026-11-25, adversarial pass: an earlier draft said `:374-377` hardcoded `cursor-agent login`; `cfg.agent` is interpolated, and `cursor-agent` is the default at `src/config.rs:100`*). A coworker who has Codex or Antigravity (or no second vendor) cannot configure it cleanly.
- **Cockpit skills assume Claude Code in layout and wording.** `src/skills.rs:380-393` symlinks the 10 `NAMES` into `.agents/skills/` but omits `_shared/`, breaking relative `../_shared/models.md` and `../_shared/stale-tooling.md` links in harnesses that do not canonicalize symlinks before resolving relative paths; `factory`'s own source checkout has `.claude/skills/` links (`tests/skills_bundle.rs:58-71`) but no `.agents/skills/` links; `skills/_shared/models.md:28-32` and `skills/polish/SKILL.md:63` hardcode *"Cursor CLI is the cross-vendor path, unconditionally"* and fall back to *"a fresh Claude agent"*; and `skills/deliver/SKILL.md` names Claude-only tool names (`AskUserQuestion`, `PushNotification`, `Skill tool`, `/clear`).
- **`docs/project.toml` can override `[workers]` models for everyone.** `src/config.rs:164, :238-255` lets a committed `docs/project.toml` override `[workers.<role>] model` and `effort`. Once teammates use different providers (PRD-027), a repo committing `model = "opus"` breaks any teammate running Codex, Cursor, Antigravity, or OpenCode.
- **No `factory setup` helper exists.** `src/cli/mod.rs:1-18` has no command to detect installed CLIs, configure `~/.config/factory/config.toml`, or quickly change providers/models/efforts when a new model drops and you want to test it for a few PRDs.

**What this leaves alone.** Running unattended build/fix/merge/checkpoint/polish workers on non-Claude providers and universal worker `dec-2` guardrails live in **PRD-027**. Gemini CLI is out of scope for now (easy to add later via `src/provider.rs`). Named config profiles are out of scope (decided 2026-11-25: simple modular wizard only).

## 2. Blast radius

- **Users upgrading `factory` with no `[review]` configured.** A user whose `~/.config/factory/config.toml` has neither `[review] provider` nor legacy `[review] agent` (and no valid `FACTORY_REVIEW_PROVIDER` env) hits one preflight `ASK-n` on their next **new** `factory run` (or exit `2` on `factory docs preflight`) to pick an adversarial reviewer (`claude` | `codex` | `cursor` | `agy` | `opencode` | `none` | `proceed`). Answering a provider or `none` persists `[review]` to `config.toml` once and never asks again; answering `proceed` skips the second opinion for **that run only** (recorded on `RunState`) and leaves `config.toml` unset so the next new run asks again (*corrected 2026-11-25, adversarial pass: an earlier draft implied `proceed` also persisted to `config.toml`*). Existing users who already have `[review] agent = "cursor-agent"` map transparently to `provider = "cursor"` without being asked. Resumed runs (`Run::find_resumable`, `src/cli/run.rs:76-95`) are unaffected.
- **Repos with `[workers]` in `docs/project.toml`.** `config::load_with_project` stops reading `[workers]` from `docs/project.toml` and warns `unknown key \`workers\` ignored` (matching `[ci]` in PRD-025). **Three** existing tests explicitly pin repo-level `[workers]` overrides — `src/config.rs:575` (`the_project_file_overrides_the_user_file_key_by_key`), `src/docs/project.rs:249-261` (`the_driver_tables_and_docs_share_the_file_without_warnings`), and `tests/knobs.rs:59` (`a_project_file_sets_the_polish_model_and_the_row_says_so`) — and are **deliberately updated** by this PRD to test user-level `[workers]` config and the project warning instead (*corrected 2026-11-25, adversarial pass: an earlier draft missed `src/docs/project.rs:252-258`*).
- **Existing fixture tests (`tests/harness/mod.rs:580-631`).** `build_env` runs every fixture with an empty `FACTORY_CONFIG_DIR`. Adding `FACTORY_REVIEW_PROVIDER=cursor` alongside `FACTORY_CI_MODE=remote` (`tests/harness/mod.rs:620`) keeps every existing test in `tests/preflight.rs`, `tests/run.rs`, `tests/closeout.rs`, and `tests/review.rs` untouched and green; new tests for the unset-review ask remove `FACTORY_REVIEW_PROVIDER`.
- **Skills bundle, `Cargo.toml`, and `tests/cockpit.rs`.** Editing `skills/` changes `bundle_hash()` (`src/skills.rs:104-119`). Because `v0.15.0` is already tagged locally (`e61273d`), `tag_state` returns `Tagged::Yes` and `relock` refuses at `0.15.0`; `Cargo.toml` must bump to `0.16.0` before `FACTORY_BUNDLE_WRITE=1 cargo test` relocks `skills/bundle.lock`. `tests/cockpit.rs:767-820` pins exact passages in `skills/deliver/SKILL.md` including Claude-only names (`AskUserQuestion`, `PushNotification`) — this PRD **deliberately updates** those passages in `tests/cockpit.rs` to pin the harness-neutral phrasing while preserving the `< 60` lines cap.
- **External contracts.** `factory review second` exit codes (`0` counted, `2` unreviewed, disabled, or clap-refused flag per `src/cli/review.rs:1`, `1` bad flag combination per `tests/review.rs:360` — *corrected 2026-11-25, adversarial pass*), `factory review collect` output shape, `factory docs doctor` lines, `factory status --json`, `factory live --json`, and `factory wait` stay backward-compatible.

## 3. FR-1 — Unified `Provider` registry and timeout-safe CLI probing

**Today.** No `src/provider.rs` exists. `src/worker.rs:221` hardcodes `"claude"`, `src/config.rs:97-105` hardcodes `agent: "cursor-agent"` / `model: "grok-4.7-high"`, and `src/review.rs:573-616` runs `<agent> --list-models` directly with a 60 s timeout and Cursor-only text matching.

**Decided 2026-11-25.** Add `src/provider.rs` (exported from `src/lib.rs`) as the single source of truth for all five supported agent CLIs:

- `Provider` enum (`Claude`, `Codex`, `Cursor`, `Agy`, `Opencode`), `#[serde(rename_all = "lowercase")]`, implementing `Display`, `FromStr`, `clap::ValueEnum`, and `ALL: [Provider; 5]`:
  - **`claude`** — binary candidates `["claude"]`; effort levels `&["low", "medium", "high", "max"]`; default model `"opus"`, default effort `Some("high")`; curated models `["opus", "sonnet", "haiku"]`; login hint `"claude login"`.
  - **`codex`** — binary candidates `["codex"]`; effort levels `&["minimal", "low", "medium", "high", "xhigh"]`; default model `"gpt-5.4"`, default effort `Some("high")`; curated models `["gpt-5.4", "o3", "o4-mini"]`; login hint `"codex login"`.
  - **`cursor`** — binary candidates `["cursor-agent", "agent"]`; effort levels `&[]` (effort is not a separate CLI flag; model slugs carry parameterized overrides); default model `"grok-4.7-high"`, default effort `None`; live model discovery via `<bin> --list-models`; login hint `"cursor-agent login"`.
  - **`agy`** — binary candidates `["agy"]`; effort levels `&["low", "medium", "high", "xhigh", "max"]`; default model `"gemini-4-neon-high"`, default effort `Some("high")`; live model discovery via `<bin> models`; login hint `"agy login"`.
  - **`opencode`** — binary candidates `["opencode"]`; effort levels `&["low", "medium", "high", "max"]` (mapped to `--variant`); default model `""` (user picks from `opencode models` or enters `<provider>/<model>`); default effort `None`; live model discovery via `<bin> models`; login hint `"opencode auth login"`.
- `ReviewProvider` enum: either `Provider(Provider)` or `None` (`"none"`), serialized flat as `"claude" | "codex" | "cursor" | "agy" | "opencode" | "none"`.
- `Provider::infer_from_binary(name: &str) -> Option<Provider>` maps basename `"claude"` → `Claude`, `"codex"` → `Codex`, `"cursor-agent"` or `"agent"` → `Cursor`, `"agy"` → `Agy`, `"opencode"` → `Opencode` (so legacy `[review] agent = "cursor-agent"` or `"/custom/path/cursor-agent"` resolves seamlessly).
- **Timeout-safe probing:**
  - `resolve_binary(provider, path_env) -> Option<PathBuf>` checks each candidate in `PATH` for an executable regular file (or symlink to one) without executing it — so lazy shims (e.g. `mise` shims for uninstalled tools) never trigger downloads during detection.
  - `detect_installed() -> Vec<Detected>` checks `PATH` without spawning subprocesses for presence, and runs a version/model probe **only when requested** (e.g. for the provider a user selected in `factory setup` or `review::probe`), always in its own process group (`process_group(0)`) with a hard timeout (`DETECT_TIMEOUT = 5s` for `setup` model listing, `PROBE_SECS` for `review::probe`) and `SIGKILL` to `-pid` on expiry.
  - `discover_models(provider, bin, timeout) -> Vec<String>` parses `cursor-agent --list-models`, `agy models` (first tab-delimited column, skipping status lines like `"Fetching available models..."`), and `opencode models` (non-empty model ids), falling back cleanly to curated suggestions if the probe times out or fails.

**Accepted cost.** Curated model lists for `claude` and `codex` can lag new vendor model releases because those CLIs expose no fast `--list-models` subcommand; `factory setup` always allows free-text model entry alongside suggestions, and unknown model/effort strings warn rather than refuse so a newly released model works immediately without waiting for a `factory` release. Spawning every binary on `PATH` during `detect_installed()` lost because `mise` shims trigger multi-second network downloads on `--version`.

**Acceptance.** Unit tests prove: (1) all five `Provider` and six `ReviewProvider` variants round-trip through serde, `Display`, and `FromStr`; (2) `infer_from_binary` maps all canonical basenames and paths including `"agent"` → `Cursor`; (3) `resolve_binary` finds executables on a synthetic `PATH` without executing them (verified by a canary shim that fails if executed); (4) `discover_models` parses recorded `agy models`, `cursor-agent --list-models`, and `opencode models` outputs and kills a hung process group within the timeout without leaving orphan children.

## 4. FR-2 — User-only multi-provider config schema and multi-section atomic writer

**Today.** `src/config.rs:16-22` holds `budget_weighted_tokens`, `workers`, `source`, `review`, and `ci`. `PROJECT_TOP_KEYS` (`:164`) includes `"workers"`, and `load_with_project` (`:238-255`) merges `docs/project.toml`'s `[workers.<role>]` over `~/.config/factory/config.toml`. Role fields today are concrete `Worker` structs (`src/config.rs:78-83`, filled by `Default` at `:108-115`), `[workers]` has no top-level default fields, `[cockpit]` does not exist, `[review]` has `agent`/`model`/`timeout_secs`/`pass_lines` with no `provider` or `effort`, and the only config writer is `write_ci_mode` (`:328-433`), which is hardcoded to `[ci] mode`.

**Decided 2026-11-25.**

- Extend `Config` in `src/config.rs`:
  - Add `"cockpit"` to `TOP_KEYS` (`:157-163`) with `pub cockpit: Cockpit` (`Cockpit { pub provider: Option<Provider> }`, `COCKPIT_KEYS = ["provider"]`).
  - Extend `Workers` (`:69-76`) with top-level default fields (`pub provider: Option<Provider>`, `pub model: Option<String>`, `pub effort: Option<String>`) plus per-role tables `build`, `fix`, `checkpoint`, `polish` where each `WorkerOverride` holds `pub provider: Option<Provider>`, `pub model: Option<String>`, `pub effort: Option<String>`.
  - **Add** `Workers::resolved(&self, role: &str, fallback_provider: Provider) -> ResolvedWorker` (*corrected 2026-11-25, adversarial pass: an earlier draft said "Keep `resolved`"; this method and `ResolvedWorker` are new*):
    - Resolves `provider`: `role.provider.or(self.provider).unwrap_or(fallback_provider)`.
    - Resolves `model` and `effort` **without leaking another provider's defaults across providers**:
      1. `role.model` / `role.effort` (if `Some`) wins.
      2. Else, inherit `self.model` / `self.effort` **only when `resolved_provider == self.provider.unwrap_or(fallback_provider)`** (i.e., the role did not switch to a different provider than `[workers]`).
      3. Else, fall back to `resolved_provider.default_model()` / `resolved_provider.default_effort()`.
    - When `resolved_provider.effort_levels().is_empty()` (e.g. `cursor`), normalize `effort` to `None` so callers never pass an unsupported effort flag.
    - Unknown keys under `[workers]` or `[workers.<role>]` still warn cleanly (`WORKER_KEYS = ["provider", "model", "effort"]`).
  - Extend `Review` (`:60-67`) with:
    - `pub provider: Option<ReviewProvider>`
    - `pub agent: Option<String>` (optional binary override / legacy key; when `provider` is `None` and `agent` is `Some(a)`, `resolve_review()` infers `ReviewProvider::Provider(Provider::infer_from_binary(&a).unwrap_or(Provider::Cursor))` with `Source::User` so existing user configs never break or re-prompt)
    - `pub model: Option<String>` (defaults to the resolved provider's default model when `None`)
    - `pub effort: Option<String>` (optional; normalized to `None` when the provider has no effort knob)
    - `pub timeout_secs: u64` (default `1800`) and `pub pass_lines: u64` (default `600`).
    - Update `REVIEW_KEYS = ["provider", "agent", "model", "effort", "timeout_secs", "pass_lines"]`.
  - Add `pub const REVIEW_PROVIDER_ENV: &str = "FACTORY_REVIEW_PROVIDER"` and `config::resolve_review(loaded: &Loaded) -> (Option<ResolvedReview>, Source, Option<String>)` (pure env lookup kept outside `config::load`, matching `resolve_ci_mode` at `src/config.rs:285-324`): checks `[review] provider`, then legacy `[review] agent`, then `FACTORY_REVIEW_PROVIDER` (`claude|codex|cursor|agy|opencode|none`, case-insensitive; an invalid non-empty env value returns `(None, Source::Default, Some(warning))`, so preflight emits a `?` judge warning AND pushes the ask when `[review]` is also unset in `config.toml` — matching `ci_mode` at `src/docs/preflight.rs:387-400`, *clarified 2026-11-25, adversarial pass*). Set `FACTORY_REVIEW_PROVIDER=cursor` in `tests/harness/mod.rs:609-630`.
  - Remove `"workers"` from `PROJECT_TOP_KEYS` (`src/config.rs:164`) and remove `[workers]` merging from `load_with_project` (`:238-255`), so `[workers]` in `docs/project.toml` warns `unknown key \`workers\` ignored` and never overrides user config. Update `src/config.rs:575` (`the_project_file_overrides_the_user_file_key_by_key`), `src/docs/project.rs:249-261` (`the_driver_tables_and_docs_share_the_file_without_warnings`), and `tests/knobs.rs:59` (`a_project_file_sets_the_polish_model_and_the_row_says_so`) to assert user-config workers + project budget override and the `[workers]`-in-project warning.
  - Replace the single-purpose `with_ci_mode` internals (`src/config.rs:328-433`) with a general comment-preserving atomic patcher `pub fn patch(config_dir: &Path, edits: &[Patch]) -> Result<PathBuf>` where `Patch` updates or inserts (or removes when `None`) a scalar key under a dotted table header (`"cockpit"`, `"workers"`, `"workers.build"`, `"review"`, `"ci"`, etc.):
    - Preserves all existing comments, blank lines, and unrelated sections.
    - Preserves symlink targets and file permissions (matching `write_ci_mode:332-343`).
    - Parses the patched text with `toml::from_str::<toml::Table>` and verifies every edited key matches before writing to a sibling temp file, `sync_all`, and atomic `rename` (`docs/decisions.md#dec-3`).
    - Keeps `write_ci_mode(config_dir, mode)` as a thin wrapper over `patch` so existing callers stay clean.

**Accepted cost.** A repository can no longer force a specific worker model (e.g. `model = "sonnet"` for polish) via `docs/project.toml`; projects that still have `[workers]` in `docs/project.toml` see a non-fatal warning on stderr until they remove it. Keeping `[workers]` in `docs/project.toml` lost because model slugs are provider-specific and break teammates using other providers.

**Acceptance.** Unit and integration tests prove: (1) `[cockpit]`, `[workers]` + `[workers.<role>]` provider-scoped inheritance (verifying that switching a role to `codex` or `cursor` does not inherit `[workers]`'s Claude `opus`/`high` defaults), `[review]` (`provider`/`agent`/`model`/`effort`), and `[ci]` parse cleanly and warn on unknown keys; (2) legacy `[review] agent = "cursor-agent"` without `provider` resolves to `ReviewProvider::Provider(Provider::Cursor)` with `Source::User`; (3) `[workers]` in `docs/project.toml` produces an `unknown key \`workers\` ignored` warning and does not override user config; (4) `config::patch` creates a missing `config.toml`, patches multiple sections (`cockpit`, `workers`, `workers.build`, `review`, `ci`) in one atomic write, preserves comments and symlinks, removes keys when patched to `None`, rejects unpatchable inline tables without modifying the file, and leaves no `.tmp` files behind.

## 5. FR-3 — Interactive and scriptable `factory setup` wizard

**Today.** `src/main.rs:18-83` and `src/cli/mod.rs:1-18` have no setup or config command; users edit `~/.config/factory/config.toml` by hand.

**Decided 2026-11-25.** Add `src/cli/setup.rs` and top-level `Command::Setup(cli::setup::SetupArgs)` in `src/main.rs`:

- **Syntax:**
  - `factory setup` — runs the full interactive wizard in order: **Cockpit** → **Workers** → **Adversarial review** → **CI mode**.
  - `factory setup cockpit` / `workers` / `review` / `ci` — runs only that one section (so switching a model to test for a few PRDs takes seconds).
  - `factory setup --show [--json]` — prints the effective user configuration (`cockpit`, `workers` per role, `review`, `ci`), each value's source (`user` / `env` / `default`), and which of the 5 provider CLIs are detected on `PATH`, without modifying any file.
- **Interactive TTY behavior** (reads from an injected `BufRead` / writes to `Write` so unit tests drive every wizard prompt deterministically in-process):
  - Shows detected CLIs on `PATH` once at the top.
  - Every prompt displays the current value as `[default]`; pressing Enter keeps it unchanged.
  - **Cockpit step:** choose from the 5 providers (defaulting to current or first detected). If `claude` is chosen, asks whether to run `factory cockpit install` (`cli::cockpit::install`). If inside a `factory` repo, reports the repo's `skills::verdict` line.
  - **Workers step (PRD-026 scope):** configures default `[workers]` `model` and `effort` (and optional per-role overrides for `build`, `fix`, `checkpoint`, `polish`) for `claude` — showing `claude`'s effort options (`low|medium|high|max`) and curated models plus free-text entry. *(Non-Claude worker provider selection activates in this same step in PRD-027 so PRD-026 never writes a dead worker provider option.)*
  - **Review step:** choose from the 5 providers or `none` (defaulting to current or the first detected CLI), then pick a model (showing live discovered models for `cursor`/`agy`/`opencode` or curated models for `claude`/`codex`, plus free-text entry) and optional effort when the chosen provider supports effort.
  - **CI step:** choose `local` or `remote` with the one-line tradeoff from PRD-025.
  - Writes all confirmed changes in one atomic `config::patch` call and prints the path written.
- **Non-interactive / scriptable flags** (for cockpit agents and automation):
  - `factory setup cockpit --provider <p>`
  - `factory setup workers [--role <default|build|fix|checkpoint|polish>] [--model <m>] [--effort <e>] [--clear]`
  - `factory setup review --provider <p|none> [--model <m>] [--effort <e>]`
  - `factory setup ci --mode <local|remote>`
  - When stdin is not a TTY and neither `--show` nor section flags are given, exit `1` and print exact non-interactive flag usage rather than hanging on stdin.

**Accepted cost.** Plain numbered stdin/stdout prompts (no `dialoguer`/`ratatui` crate dependency) keep compile time, binary size, and testability tight at the cost of no arrow-key menus. Named config profiles were deliberately excluded (decided 2026-11-25) to keep `config.toml` simple; changing one section via `factory setup workers` or `factory setup review` covers testing a new model and switching back.

**Acceptance.** (1) `factory setup --show` and `--show --json` print effective config, sources, and detected CLIs without touching disk; (2) driving the wizard through a scripted `BufRead` configures all four sections in one atomic `config.toml` write and pressing Enter preserves existing values; (3) `factory setup review --provider codex --model gpt-5.4 --effort high` and `factory setup ci --mode local` work non-interactively and exit `0`; (4) non-TTY `factory setup` with no flags exits `1` with usage text and never blocks.

## 6. FR-4 — Multi-provider adversarial reviewer and preflight ask-if-missing

**Today.** `src/review.rs:18-25` defines Cursor-only `FLAGS`, `probe()` (`:573-616`) runs `<agent> --list-models`, `parse_stream()` (`:643-671`) parses Cursor/Claude NDJSON, `run_pass()` (`:682-714`) runs `<agent> -p <FLAGS> --model <model> <prompt>`, and `preflight::second_opinion` (`src/docs/preflight.rs:360-379`) emits a judge line naming `run \`{} login\`` (from `cfg.agent`) and hardcoding `"one Claude agent"`.

**Decided 2026-11-25.**

- **Provider-dispatched `src/review.rs`:**
  - **Explicit opt-out (`ReviewProvider::None`) — consistent across `review second`, `collect`, and skills** (*corrected 2026-11-25, adversarial pass: an earlier draft had `review second` emit `unreviewed:` exit `2`, which `polish/SKILL.md:70-72` and `prd/SKILL.md:99` treat as owing a local fallback, contradicting `collect` skipping the fallback*):
    - `factory review second` prints `disabled: second opinion disabled in config.toml` (not `unreviewed:`) and returns exit `2` without spawning any child process or creating a log dir.
    - `collect::collect` (`src/collect.rs:109-137`) records `second opinion: skipped (disabled in config)` (which `collect::reusable` treats as reusable, never `fallback owed`) instead of invoking `review::probe`.
    - `skills/polish/SKILL.md` (`:65, :70-72`) and `skills/prd/SKILL.md` (`:99`) explicitly distinguish **disabled** (`disabled:` / `skipped (disabled in config)` → owner opted out, no local fallback owed) from **unreviewed** (`unreviewed:` / `fallback owed` → auth/probe/pass failure, run the local fallback).
  - For each of the 5 providers (`Provider`), dispatch `probe`, `run_pass`, and `parse_stream_for(provider, text)` through `src/provider.rs` / `src/review.rs`:
    - **`cursor`** (`cursor-agent` / `agent`, or explicit `[review] agent` binary override): preserves existing invocation (`-p --trust --mode ask --output-format stream-json --stream-partial-output --model <model> <prompt>`), `--list-models` probe, and stream-json parser byte-for-byte.
    - **`claude`**: `claude -p --output-format stream-json --verbose --permission-mode plan --tools Read,Grep,Glob --model <model> [--effort <effort>] <prompt>` (scrubbing `CLAUDECODE` / session vars per `worker::scrubbed` so running from inside Claude Code never hits the nested-session guard). Probe checks binary availability (`claude --version` exit 0 within `PROBE_SECS`). Stream parses Claude `assistant` + `result` NDJSON.
    - **`codex`**: `codex exec --json --sandbox read-only --ephemeral -m <model> [-c model_reasoning_effort=<effort>] -` (prompt delivered on stdin so large prompts never hit `ARG_MAX`). Probe checks `codex --version` exit 0 within `PROBE_SECS`. Stream parses Codex JSONL (`item.completed` with `agent_message` text into `result`/`partial`, `error` / `turn.failed` into `error`).
    - **`agy`**: `agy -p --mode plan --output-format stream-json --model <model> [--effort <effort>] <prompt>`. Probe checks `agy models` exit 0 and `lists_model(&text, &model)` (matching Cursor's model verification). Stream parses `agy` stream-json (`assistant` text / `result` / `is_error`).
    - **`opencode`**: `opencode run --format json --agent plan -m <model> [--variant <effort>] <prompt>`. Probe checks `opencode models` exit 0 and `lists_model(&text, &model)`. Stream parses OpenCode JSONL (`text` with `part.text`, `error` with `error.message`/`data.message`).
  - `pass_header` (`src/review.rs:837-850`) preserves `Pass n — ... (lines, s, <model>)` (or `<provider>:<model>` for non-Cursor) compatible with `collect::passes_and_log` (`src/collect.rs:46-48`).
- **Preflight ask-if-missing (`src/docs/preflight.rs:360-379` and `src/cli/answer.rs:301-304`):**
  - Add `pub const REVIEW_ASK_PREFIX: &str = "adversarial reviewer not set in config.toml"`.
  - Update `preflight::second_opinion(loaded: &config::Loaded, pf: &mut Preflight)`:
    - Resolve via `config::resolve_review(loaded)`.
    - If `warning` is `Some(w)` (invalid `FACTORY_REVIEW_PROVIDER`), push `pf.judge.push(w)`.
    - If resolved is `None` (no `[review]` in `config.toml` and no valid `FACTORY_REVIEW_PROVIDER`): push `pf.ask` starting with `REVIEW_ASK_PREFIX`, listing detected installed CLIs, `none` (disable cross-vendor review), and `proceed` (skip for this run only without writing `config.toml`).
    - If `ReviewProvider::None`: push `pf.ok`: `"second opinion: disabled (user)"` (or `(env)`).
    - If `ReviewProvider::Provider(p)`: run `review::probe`; on `Ok(())`, push `pf.ok`: `"second opinion: <bin> with <model> ready"` (keeping `tests/preflight.rs:55` exact for Cursor); on `Err(reason)`, push `pf.judge`: `"second opinion not ready: <reason> — run `<login_hint>` or `factory setup review`; polish falls back to a fresh local subagent per <pass_lines>-line pass"`.
  - In `src/cli/answer.rs` (`answer_ask`):
    - When `AskOpened.text` starts with `preflight::REVIEW_ASK_PREFIX`:
      - `"proceed"` → leaves `config.toml` untouched, persists `review_override: Some(ReviewProvider::None)` on `RunState` so closeout skips second-opinion probing for this run only, logs the answer, and wakes the run.
      - `"none"` → writes `[review] provider = "none"` via `config::patch`, persists `review_override: Some(ReviewProvider::None)` on `RunState`, and wakes the run.
      - `"<provider>"` or `"<provider>:<model>"` or `"<provider>:<model>:<effort>"` where `<provider>` parses as `Provider` → writes `[review] provider`, `model` (given or provider default), and optional `effort` via `config::patch`, clears `review_override`, and wakes the run.
      - Anything else → prints valid choices, writes nothing, keeps the ask open, exits `1`.

**Accepted cost.** `claude` and `codex` probes verify binary + `--version` rather than a live API auth round-trip, so an expired Claude/Codex login surfaces on the first review pass (as `unreviewed`, triggering the polish fallback cleanly) rather than in preflight. Running a live inference ping during every `factory docs preflight` lost because it burns tokens and adds 3–10 s latency to every preflight.

**Acceptance.** (1) Stub tests in `tests/review.rs` prove `factory review second --diff main` and `--doc` succeed and parse findings for all five providers (`claude`, `codex`, `cursor`, `agy`, `opencode`) with each provider's exact flags and stream format; (2) `[review] provider = "none"` prints `disabled: second opinion disabled in config.toml` and exits `2` without spawning a subprocess, `collect` marks it `"skipped (disabled in config)"` with `reusable == true`, and preflight prints `✓ second opinion: disabled (user)`; (3) with `FACTORY_REVIEW_PROVIDER` unset (or set to an invalid value, which also emits a `?` judge warning) and no `[review]` in `config.toml`, `factory docs preflight 001` exits `2` with the review ask, `factory run 001` parks on `ASK-n`, `factory answer ASK-n "codex:gpt-5.4:high"` writes `[review]` to `config.toml` and wakes the run, `factory answer ASK-n "none"` saves `provider = "none"`, `factory answer ASK-n "proceed"` leaves `config.toml` untouched while setting `review_override = "none"` on `RunState`, and `factory answer ASK-n "bogus"` exits `1` leaving the ask open; (4) existing `tests/preflight.rs` and `tests/review.rs` pass unchanged with `FACTORY_REVIEW_PROVIDER=cursor` in `tests/harness/mod.rs`.

## 7. FR-5 — Portable cockpit skills across all five harnesses

**Today.** `src/skills.rs:379-393` symlinks only `NAMES` (10 folders) into `.agents/skills/`, omitting `_shared/`. `factory`'s own repo has `.claude/skills/` links (`tests/skills_bundle.rs:58-71`) but no `.agents/skills/` links. `skills/_shared/models.md:28-32` and `skills/polish/SKILL.md:63` hardcode Cursor CLI unconditionally and Claude fallback, and `skills/deliver/SKILL.md` names Claude-only tool names (`AskUserQuestion`, `PushNotification`, `Skill tool`, `/clear`) pinned by `tests/cockpit.rs:767-820`.

**Decided 2026-11-25.**

- **Layout fix (`src/skills.rs` and source checkout):**
  - Add `pub const LINK_DIRS: [&str; 11]` (all 10 `NAMES` plus `"_shared"`) and symlink `.agents/skills/_shared -> ../../.claude/skills/_shared` in `skills::render` (`src/skills.rs:379-393`) whenever `!shared`, recording `.agents/skills/_shared` in `factory.toml`'s `[links]` so `verdict_for` (`:493-501`) and `remove_dropped` (`:600-611`) check and maintain it cleanly.
  - Add `.agents/skills/<name>` and `.agents/skills/_shared` symlinks in `factory`'s own repo pointing to `../../skills/<name>` (and extend `tests/skills_bundle.rs:58-71` to verify both `.claude/skills/` and `.agents/skills/` in this checkout).
- **Harness-neutral skill wording:**
  - `skills/_shared/models.md`: replace *"Cursor CLI is the cross-vendor path, unconditionally"* (`:28`) with *"`factory review second` is the cross-vendor path, using `[review]` in `~/.config/factory/config.toml` (configured via `factory setup`)"*, and generalize the fallback (`:32`) from *"a fresh Claude agent"* to *"a fresh subagent on the `judgment` model with no inherited context"* (respecting `tests/skills_bundle.rs:107-120`, which rejects `` ```bash ``, `grok-`, `gpt-`, `gemini-`, `~/.local/bin`, `--trust`, and `cursor-agent -p` in `models.md` — *clarified 2026-11-25, adversarial pass*).
  - `skills/polish/SKILL.md` (*added 2026-11-25, adversarial pass — `:63` still hardcoded `"Cursor CLI, unconditionally"`*):
    - Replace `:63`'s *"Cursor CLI, unconditionally"* with the configured cross-vendor provider in `[review]` (`factory setup review`), noting that cross-vendor is the point.
    - Update `:65` and `:70-72` so `skipped (disabled in config)` / `disabled:` means the owner opted out (no fallback owed), while `fallback owed` / `unreviewed:` still triggers the per-pass fresh local subagent fallback, and remove the Cursor-specific `"checks --list-models"` phrase in `:70`.
  - `skills/prd/SKILL.md`: update `:99` so exit `2` with `disabled:` skips the fallback while `unreviewed:` still runs the fresh local subagent fallback; generalize bare `AskUserQuestion` wording to harness-neutral structured choices.
  - `skills/deliver/SKILL.md`: stay strictly `< 60` lines; generalize Claude-only tool names (`AskUserQuestion` → ask the owner with structured choices, `PushNotification` → notify the owner if your harness supports push notifications, `Skill tool` → invoke `/slice NNN from /deliver`, `/clear` → start a fresh session), add the one-line note for the adversarial-reviewer `ASK-n` (`<provider>`, `none`, or `proceed`), and update `tests/cockpit.rs:767-820` to pin the updated harness-neutral passages.
  - `skills/harvest/SKILL.md`, `skills/lanes/SKILL.md`, `skills/harness/SKILL.md`, `skills/_shared/stale-tooling.md`, `skills/README.md`: replace Claude-only phrasing (`Claude Code skills` → `Agent skills`, bare `AskUserQuestion` → harness-neutral structured question wording).
- **Version bump and relock:** bump `Cargo.toml` (and `Cargo.lock`) from `0.15.0` to `0.16.0` and run `FACTORY_BUNDLE_WRITE=1 cargo test` to update `skills/bundle.lock`.

**Accepted cost.** Repos initialized with `factory <= 0.15.0` will report `skills: behind` in `factory docs doctor` until the owner runs `factory skills update` once, which adds `.agents/skills/_shared` and refreshes the skills cleanly. Updating `tests/cockpit.rs:778-779` changes two pinned strings in `deliver/SKILL.md` deliberately because decoupling `/deliver` from Claude-only tool names is the requirement.

**Acceptance.** (1) `skills::render` creates `.agents/skills/_shared -> ../../.claude/skills/_shared` and verifies `../_shared/models.md` resolves from `.agents/skills/deliver/SKILL.md`; (2) `this_repos_claude_skills_link_into_the_source` verifies both `.claude/skills/` and `.agents/skills/` in this checkout; (3) `cargo test` passes `every_file_under_skills_is_in_the_bundle_byte_for_byte`, `bundle_lock_matches`, `models_md_holds_no_invocation_and_the_skills_call_review_second`, and the updated `tests/cockpit.rs` passages with `deliver/SKILL.md` `< 60` lines; (4) neither `skills/_shared/models.md` nor `skills/polish/SKILL.md` claims Cursor CLI is unconditional or names Claude as the sole fallback, and both `skills/polish/SKILL.md` and `skills/prd/SKILL.md` distinguish `disabled` from `unreviewed` (pinned by `assert_unique_passage` in `tests/skills_bundle.rs`).

## 8. Decisions log

No `OPEN-n`. Every fork was settled on 2026-11-25 and lives in its FR:

- Five first-class providers (`claude`, `codex`, `cursor`, `agy`, `opencode`), leaving Gemini CLI out for now (§3).
- `[workers]` config is strictly user-only in `~/.config/factory/config.toml` (removed from `docs/project.toml`), matching `[ci] mode` in PRD-025 (§4).
- Simple modular `factory setup [cockpit|workers|review|ci]` wizard over named profiles (§5).
- Explicit `[review] provider = "none"` prints `disabled:` (not `unreviewed:`) and skips local fallback consistently across `review second`, `collect`, `/polish`, and `/prd` (§6, §7).
- Two-PRD sequence: PRD-026 ships foundation + setup + portable cockpit skills + multi-provider adversarial review; PRD-027 unlocks multi-provider workers on top (§1, §5).
- `PATH` detection never executes binaries during discovery so lazy shims (`mise`) never trigger network installs; model discovery spawns with a 5 s process-group timeout only for selected/probed providers (§3).

## 9. QA plan

**Gate**: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test && claude plugin validate mods/cockpit && claude plugin test mods/cockpit` *(declared in `docs/project.toml`)*
**Surface**: `factory` CLI (`setup`, `review second`, `review collect`, `docs preflight`, `run`, `answer`, `init`, `skills update`, `docs doctor`), `~/.config/factory/config.toml`, and bundled skills. No GUI: every FR is proven below it.
**Harness**: `tests/harness/mod.rs` (`Fixture`), `src/provider.rs` unit tests, `src/config.rs` unit tests, `tests/setup.rs`, `tests/review.rs`, `tests/preflight.rs`, `tests/knobs.rs`, `tests/cockpit.rs`, `tests/skills_bundle.rs`.
**Seed state**: synthetic `PATH` with stub CLIs (`claude`, `codex`, `cursor-agent`, `agy`, `opencode`, plus a canary shim); fixture repo with empty `FACTORY_CONFIG_DIR` and `FACTORY_REVIEW_PROVIDER` unset; fixture `config.toml` with comments and legacy `[review] agent = "cursor-agent"`.

| FR | Proof | Evidence | Human? |
|---|---|---|---|
| FR-1 | run `provider_registry_roundtrips_infers_and_probes_without_executing_shims` and `discover_models_parses_live_outputs_and_kills_hung_probe` | test transcript | no |
| FR-2 | run `config_loads_multi_provider_schema_maps_legacy_agent_and_ignores_project_workers` and `config_patch_updates_multiple_sections_atomically_preserving_comments` | test transcript | no |
| FR-3 | run `setup_wizard_configures_all_sections_and_keeps_defaults`, `setup_non_interactive_flags_and_show_json`, and `setup_non_tty_without_flags_exits_one` | test transcript | no |
| FR-4 | run `review_second_runs_all_five_providers_and_none_mode` and `preflight_asks_when_review_unset_and_answer_persists_or_proceeds` | test transcript | no |
| FR-5 | run `skills_render_links_shared_into_agents_skills`, `this_repos_claude_skills_link_into_the_source`, `every_file_under_skills_is_in_the_bundle_byte_for_byte`, `bundle_lock_matches`, `skills_are_harness_neutral_and_distinguish_disabled_review`, and `the_cockpit_skill_is_under_sixty_lines_and_names_the_five_commands` | test transcript | no |

No row needs a human, so no `qa-guide.md` is generated.

## 10. Slices

Four slices on one branch, `feat/multi-provider-setup`, one PR, one commit per slice, Prove driven at the checkpoints.

- **01 — Provider registry, user-only config, and atomic patcher** (FR-1, FR-2, spine). `src/provider.rs`, `src/lib.rs`, `src/config.rs`, `src/docs/project.rs`, `tests/knobs.rs`, and `tests/harness/mod.rs` (`FACTORY_REVIEW_PROVIDER=cursor`). Independent and the prerequisite for slices 03 and 04.
- **02 — Portable cockpit skills and bundle relock** (FR-5). `src/skills.rs`, `.agents/skills/*`, `skills/**`, `Cargo.toml` (`0.16.0`), `Cargo.lock`, `skills/bundle.lock`, `tests/skills_bundle.rs`, and `tests/cockpit.rs`. Independent of 01, 03, and 04 (zero file overlap).
- **03 — Interactive and scriptable `factory setup` wizard** (FR-3). `src/cli/setup.rs`, `src/cli/mod.rs`, `src/main.rs`, and `tests/setup.rs`. **Depends on 01** for `Provider`, `ReviewProvider`, `detect_installed`, `discover_models`, the extended `Config` schema, and `config::patch`; independent of 02 and 04.
- **04 — Multi-provider adversarial review and preflight ask** (FR-4, highest-risk). `src/review.rs`, `src/cli/review.rs`, `src/collect.rs`, `src/closeout.rs`, `src/run.rs`, `src/docs/preflight.rs`, `src/cli/run.rs`, `src/cli/answer.rs`, `tests/review.rs`, and `tests/preflight.rs`. **Depends on 01** for `Provider`, `ReviewProvider`, `resolve_review`, `config::patch`, and `FACTORY_REVIEW_PROVIDER`; independent of 02 and 03.

**Order and checkpoints.** Wave 1 runs **01** and **02** in parallel; **CP-1** lands right after both (`checkpoint_every = 2`), proving FR-1, FR-2, and FR-5 before anything builds on 01. Wave 2 runs **03** and **04** in parallel; **CP-2** (final checkpoint) proves FR-3, FR-4, and re-proves all rows.

**No `qa-guide.md`:** no row in §9 QA plan is marked `Human? = yes`, so nothing is generated for human QA.

## 11. Non-goals

- **Non-Claude unattended worker execution (`build`, `fix`, `merge`, `checkpoint`, `polish`).** Lives in **PRD-027** (`docs/features/027-multi-provider-workers/prd.md`).
- **Gemini CLI (`gemini`) provider.** Not handled (2026-11-25): excluded by owner choice; the 5 team CLIs cover current usage. Revisit if a teammate adopts `gemini` CLI and adds one `Provider` variant.
- **Named config profiles (`[profiles.<name>]`, `factory setup use <name>`).** Not handled (2026-11-25): owner chose a simple modular `factory setup [section]` wizard to avoid profile state complexity. Revisit if switching multi-role combinations back and forth becomes frequent friction.
- **Not handled: live API auth ping during `claude`/`codex` review probes.** Accepted 2026-11-25: `claude` and `codex` expose no zero-token auth-check flag equivalent to `cursor-agent --list-models`; a logged-out pass degrades cleanly to `unreviewed` and triggers polish's local fallback. Revisit if either CLI adds a fast zero-cost `whoami` / `auth status` command.
- **Not handled: interactive TUI replacement for Claude's `mods/cockpit` plugin.** Accepted 2026-11-25: coworkers drive `/prd` and `/deliver` from their own agent harness using `factory` CLI + portable skills, and `factory live` already provides standalone status. Revisit if non-Claude users ask for a persistent terminal dashboard.
- **Not handled: cross-vendor same-family warning in `factory setup` and `preflight`.** Cut 2026-11-25 (§5 cut): owner chooses worker and reviewer providers explicitly without vendor-family warnings. Revisit if teammates accidentally configure same-vendor reviews and ask for a preflight nudge.
- **Not handled: user-global `~/.agents/skills/` installation outside repos.** Accepted 2026-11-25: `factory init` / `factory skills update` already version-pins `.claude/skills/` and `.agents/skills/` per repo via `factory.toml` so skills never drift from a repo's framework version. Revisit if teammates want skills available before running `factory init` in a repo.

## 12. Verification notes

Checked against `main` at `e61273d`, 2026-11-25, then re-verified the same day by a cross-vendor adversarial pass (`factory review second --doc`, `grok-4.7-high`, 860 s, log `~/.local/share/factory/reviews/20261125-203409`). Nine findings were caught and corrected inline above. **Re-verify at slice entry.**

- `src/lib.rs:1-41` — module list; no `provider` module exists.
- `src/main.rs:18-108` and `src/cli/mod.rs:1-18` — top-level commands; no `setup` subcommand exists.
- `src/config.rs:16-22` (`Config`), `:60-115` (`Review`, `Workers`, `Worker` defaults; `agent: "cursor-agent"` at `:100`), `:157-171` (`TOP_KEYS`, `PROJECT_TOP_KEYS` includes `"workers"`, `ROLE_KEYS`, `WORKER_KEYS`, `REVIEW_KEYS`), `:219-257` (`load_with_project` merges `project.workers`), `:285-324` (`resolve_ci_mode` env-outside-`load` pattern), `:328-433` (`write_ci_mode` atomic comment-preserving writer), `:575-605` (test pinning project `[workers]` override).
- `src/docs/project.rs:249-261` — `the_driver_tables_and_docs_share_the_file_without_warnings` also pins `docs/project.toml` `[workers.polish] model = "sonnet"` (added 2026-11-25, adversarial pass).
- `src/review.rs:18-25` (hardcoded Cursor `FLAGS`), `:570-623` (`probe` calling `--list-models` and `lists_model`), `:643-671` (`parse_stream`), `:682-714` (`run_pass`), `:837-850` (`pass_header`); `src/cli/review.rs:1` (exit `0` counted, `2` unreviewed/clap-refused, `1` bad combination).
- `src/collect.rs:31-52` (`reusable`, `passes_and_log`), `:109-137` (`second_opinion` dispatch), `:198-273` (`second_opinion`).
- `src/docs/preflight.rs:360-409` (`second_opinion` interpolating `cfg.agent` into `run \`{} login\`` and hardcoding `"one Claude agent"`, `ci_mode` warn+ask pattern), `:411-430` (`preflight::cli` exit code recomputation).
- `src/cli/answer.rs:249-396` (`answer_ask` and `ci_mode_answer` persistence pattern).
- `src/cli/run.rs:76-142` (`find_resumable` before preflight, `preflight::second_opinion`, `preflight::ci_mode`, `pf.ask` loop).
- `src/skills.rs:14-23` (`SKILLS_DIR = ".claude/skills"`, `LINKS_DIR = ".agents/skills"`, `NAMES` excludes `_shared`), `:360-404` (`render` links only `NAMES` into `.agents/skills/`), `:493-501` (`verdict_for` checks `m.links`), `:600-611` (`remove_dropped`).
- `skills/_shared/models.md:18, :28-32` (hardcodes Cursor CLI unconditionally and Claude fallback); `skills/polish/SKILL.md:63-72` (hardcodes `"Cursor CLI, unconditionally"` and exit `2` → fallback); `skills/prd/SKILL.md:99` (exit `2` → fallback); `skills/deliver/SKILL.md:1-50` (50 lines; Claude-only tool names).
- `tests/harness/mod.rs:580-631` (`build_env` sets empty `FACTORY_CONFIG_DIR` and `FACTORY_CI_MODE=remote`).
- `tests/preflight.rs:55` (asserts `"second opinion: cursor-agent with grok-4.7-high ready"`).
- `tests/knobs.rs:40-159` (asserts `docs/project.toml` `[workers.polish] model = "sonnet"` override).
- `tests/cockpit.rs:767-820` (pins `skills/deliver/SKILL.md` `< 60` lines and verbatim passages including `AskUserQuestion` and `PushNotification`).
- `tests/skills_bundle.rs:34-71, :107-137` (bundle completeness, `.claude/skills/` links in source checkout, `models.md` forbidden token list).
- `Cargo.toml:3` (`version = "0.15.0"`, already tagged locally as `v0.15.0` at `e61273d`).
- Verified real CLI flags on host 2026-11-25: `claude 2.1.332`, `codex-cli 0.172.0` (`codex exec --json --sandbox read-only --ephemeral -m <model> -c model_reasoning_effort=<effort> -`), `cursor-agent` / `agent` `2026.11.22-e9042b2` (`-p --trust --mode ask --output-format stream-json --stream-partial-output --model <model>`, `--list-models`), `agy 1.5.17` (`-p --mode plan --output-format stream-json --model <model> --effort <low|medium|high|xhigh|max>`, `agy models` outputs tab-delimited `<id>\t<label>` after `"Fetching available models..."`). Note: `opencode` was not locally installed on this host (unpopulated `mise` shim); confirm `opencode run --format json --agent plan` flags and JSONL schema against a live `opencode` binary at slice entry.
