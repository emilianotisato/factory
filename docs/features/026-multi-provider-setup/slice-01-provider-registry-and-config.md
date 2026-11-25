# Slice 01 — Provider registry, user-only config, and atomic patcher

**Source**: [`prd.md`](prd.md) → §2, §3 (FR-1), §4 (FR-2), §9, §11. **Depends on**: — (spine; slices 03 and 04 build on `Provider`, the extended `Config` schema, `resolve_review`, and `config::patch`)
**Created**: 2026-11-25

**Progress**: _(written as it lands — what was built, the gate numbers, what a review caught)_

**Slice**: Adds `src/provider.rs` with the five-provider registry (`claude`, `codex`, `cursor`, `agy`, `opencode`), `ReviewProvider`, and timeout-safe PATH/model probing; extends `src/config.rs` with `[cockpit]`, `[workers]` top-level defaults + provider-scoped role resolution, `[review]` `provider`/`effort` + legacy `agent` inference + `FACTORY_REVIEW_PROVIDER`, removes `[workers]` overrides from `docs/project.toml`, generalizes `write_ci_mode` into `config::patch`, and sets `FACTORY_REVIEW_PROVIDER=cursor` in `tests/harness/mod.rs`. Leaves the `factory setup` CLI to slice 03, multi-provider `review.rs` & preflight ask to slice 04, and portable skills to slice 02.

## Current code / environment

- **Module layout**: `src/lib.rs:1-41` has no `provider` module yet.
- **Config schema & callers**: `src/config.rs:16-115` defines `Config`, `Review` (`agent: String`, `model: String`, `timeout_secs: u64`, `pass_lines: u64`), `Workers` (`build`, `fix`, `checkpoint`, `polish`), and `Worker` (`model: String`, `effort: String`). Callers outside `config.rs` (`tests/run.rs:225-238`, `src/worker.rs:192`, `src/scheduler.rs:764`, `src/checkpoint.rs:279, :714`, `src/closeout.rs:272, :2054, :2695, :2737`, `tests/review_stop.rs:9`, `tests/review_progress.rs:10`) read concrete `String` fields on `Worker` and `Review::default()`. Keep `Worker.model`/`Worker.effort` and `Review.agent`/`Review.model` as `String` (defaulted) while adding `Option<Provider>` / `Option<ReviewProvider>` / `Option<String>` and tracking explicit presence in `Sources` so slice 01 does not touch `worker.rs`, `scheduler.rs`, `checkpoint.rs`, `closeout.rs`, or `review.rs`!
- **Three tests pin `[workers]` in `docs/project.toml`**: `src/config.rs:575-605` (`the_project_file_overrides_the_user_file_key_by_key`), `src/docs/project.rs:249-261` (`the_driver_tables_and_docs_share_the_file_without_warnings`), and `tests/knobs.rs:40-159` (`a_project_file_sets_the_polish_model_and_the_row_says_so`). PRD-026 §2 & §4 deliberately authorize updating all three to test user-level `[workers]` and the project warning.
- **Atomic config writer**: `src/config.rs:328-433` implements `write_ci_mode` using `fs::canonicalize` (preserving symlinks & permissions), line-by-line patching, `toml::from_str` verification, and `write_synced` + `fs::rename` (`docs/decisions.md#dec-3`). No `toml_edit` crate in `Cargo.toml`.
- **Fixture harness**: `tests/harness/mod.rs:609-630` sets empty `FACTORY_CONFIG_DIR` and `FACTORY_CI_MODE=remote`.
- **Deliberate non-changes (PRD-026 §11)**:
  - No Gemini CLI provider.
  - No cross-vendor `same_vendor` / vendor-family check (cut 2026-11-25, §5 cut).
  - No named config profiles (`[profiles.<name>]`).
  - Do not execute binaries on `PATH` during `detect_installed()` (lazy shims like `mise` trigger network downloads on `--version`).

## Tasks

### 1. Implement `src/provider.rs` and export from `src/lib.rs` — high — blocked by: —
**Source**: §3 FR-1: "Add `src/provider.rs` (exported from `src/lib.rs`) as the single source of truth for all five supported agent CLIs... `Provider` enum (`Claude`, `Codex`, `Cursor`, `Agy`, `Opencode`)... `ReviewProvider` enum... `infer_from_binary`... `resolve_binary`... `detect_installed`... `discover_models`".

`src/provider.rs`, `src/lib.rs`.
- Export `pub mod provider;` in `src/lib.rs`.
- Define `Provider` (`Claude`, `Codex`, `Cursor`, `Agy`, `Opencode`, serde lowercase, `Display`, `FromStr`, `clap::ValueEnum`, `ALL: [Provider; 5]`) with methods:
  - `as_str(self) -> &'static str`
  - `binaries(self) -> &'static [&'static str]` (`["claude"]`, `["codex"]`, `["cursor-agent", "agent"]`, `["agy"]`, `["opencode"]`)
  - `effort_levels(self) -> &'static [&'static str]` (`claude`: `["low", "medium", "high", "max"]`; `codex`: `["minimal", "low", "medium", "high", "xhigh"]`; `cursor`: `[]`; `agy`: `["low", "medium", "high", "xhigh", "max"]`; `opencode`: `["low", "medium", "high", "max"]`)
  - `default_model(self) -> &'static str` (`"opus"`, `"gpt-5.4"`, `"grok-4.7-high"`, `"gemini-4-neon-high"`, `""`)
  - `default_effort(self) -> Option<&'static str>` (`Some("high")`, `Some("high")`, `None`, `Some("high")`, `None`)
  - `curated_models(self) -> &'static [&'static str]` (`claude`: `["opus", "sonnet", "haiku"]`; `codex`: `["gpt-5.4", "o3", "o4-mini"]`; others: `&[]`)
  - `login_hint(self) -> &'static str` (`"claude login"`, `"codex login"`, `"cursor-agent login"`, `"agy login"`, `"opencode auth login"`)
  - `infer_from_binary(name: &str) -> Option<Provider>` (takes path or basename; maps `"claude"`, `"codex"`, `"cursor-agent"`, `"agent"`, `"agy"`, `"opencode"`).
- Define `ReviewProvider` (`Provider(Provider)` | `None`) with custom/untagged or clean `Serialize`/`Deserialize`, `Display`, `FromStr`, `clap::ValueEnum` (or `ValueParser`) accepting `"claude" | "codex" | "cursor" | "agy" | "opencode" | "none"`.
- Implement `resolve_binary_in(provider: Provider, path_env: &OsStr) -> Option<PathBuf>` and `resolve_binary(provider: Provider) -> Option<PathBuf>` checking executable regular files (following symlinks via `fs::metadata`) without spawning any child process.
- Implement `Detected { pub provider: Provider, pub binary: PathBuf }` and `detect_installed_in(path_env: &OsStr) -> Vec<Detected>` / `detect_installed() -> Vec<Detected>`.
- Implement `discover_models(provider: Provider, bin: &Path, timeout: Duration) -> Vec<String>`:
  - `Cursor`: `<bin> --list-models`; `Agy`: `<bin> models` (parse first tab-delimited column, skipping status lines like `"Fetching available models..."`); `Opencode`: `<bin> models` (non-empty model ids).
  - Run in a dedicated process group (`CommandExt::process_group(0)`) with stdout/stderr redirected to temp files (never undrained pipes, matching `src/review.rs:1`), poll `try_wait` until `timeout`, and `libc::kill(-(pid as i32), SIGKILL)` + `wait()` on expiry. Fall back to `provider.curated_models()` on failure/timeout or for `Claude`/`Codex`.

**The trap**: `fs::symlink_metadata` on a symlink to an executable returns `is_file() == false`; use `fs::metadata` so symlinks on `PATH` (such as `~/.local/bin/agent` or mise bins) resolve cleanly, and never execute the binary during `resolve_binary` because `mise` shims download tools on invocation.

**Acceptance:** Unit tests `provider_registry_roundtrips_infers_and_probes_without_executing_shims` and `discover_models_parses_live_outputs_and_kills_hung_probe` pass:
- Round-trips all 5 `Provider` and 6 `ReviewProvider` values through serde/Display/FromStr and maps `"agent"` and `"/opt/bin/cursor-agent"` → `Cursor`.
- Places a canary executable script on a synthetic `PATH` that creates a sentinel file if executed; asserts `detect_installed_in` finds it while the sentinel file does **not** exist (fails if `detect_installed_in` spawns binaries).
- Parses recorded `agy models`, `cursor-agent --list-models`, and `opencode models` outputs and spawns a stub that sleeps 30 s with a background child, asserting `discover_models` returns within 2 s and reaps the process group (fails if timeout or process-group kill is missing).

### 2. Extend `src/config.rs` schema, make `[workers]` user-only, add `config::patch`, and update harness/tests — high — blocked by: 1
**Source**: §2 Blast radius, §4 FR-2: "Extend `Config` in `src/config.rs`... Add `Cockpit`... Extend `Workers`... Add `Workers::resolved`... Extend `Review`... Add `REVIEW_PROVIDER_ENV` and `resolve_review`... Remove `"workers"` from `PROJECT_TOP_KEYS`... Replace `with_ci_mode` internals with `config::patch`".

`src/config.rs`, `src/docs/project.rs`, `tests/knobs.rs`, `tests/harness/mod.rs`.
- Add `Cockpit { pub provider: Option<Provider> }`, add `"cockpit"` to `TOP_KEYS`, validate `COCKPIT_KEYS = ["provider"]`.
- Extend `Workers` with top-level `pub provider: Option<Provider>`, `pub model: Option<String>`, `pub effort: Option<String>`, plus `build`, `fix`, `checkpoint`, `polish: Worker` where `Worker` adds `pub provider: Option<Provider>` alongside existing `pub model: String`, `pub effort: String`. Track explicit presence of `[workers]` defaults and per-role keys in `Sources` (add `provider: Source`, `cockpit: Source`, `review: Source`, plus explicit-set tracking for `[review]` `provider`/`agent`/`model`/`effort` and `[workers]` role overrides).
- Add `ResolvedWorker { pub provider: Provider, pub model: String, pub effort: Option<String> }` and `Workers::resolved(&self, sources: &RoleSources, role: &str, fallback_provider: Provider) -> ResolvedWorker` (or store explicit `Option`s alongside concrete fields) implementing the exact provider-scoped rule from §4 FR-2:
  1. `provider = role.provider.or(workers.provider).unwrap_or(fallback_provider)`.
  2. `model`: explicit `role.model` → else if `provider == workers.provider.unwrap_or(fallback_provider)` and `workers.model` is set, use `workers.model` → else `provider.default_model()`.
  3. `effort`: if `provider.effort_levels().is_empty()`, always `None`; else explicit `role.effort` → else if `provider == workers.provider.unwrap_or(fallback_provider)` and `workers.effort` is set, use `workers.effort` → else `provider.default_effort()`.
- Extend `Review` with `pub provider: Option<ReviewProvider>` and `pub effort: Option<String>` (keeping `agent: String`, `model: String`, `timeout_secs: u64`, `pass_lines: u64` with existing `Review::default()` so slice 01 keeps `review.rs`/`closeout.rs`/`tests/review_*.rs` compiling). Update `REVIEW_KEYS = ["provider", "agent", "model", "effort", "timeout_secs", "pass_lines"]`.
- Add `pub const REVIEW_PROVIDER_ENV: &str = "FACTORY_REVIEW_PROVIDER"`, `ResolvedReview { pub provider: ReviewProvider, pub binary: Option<String>, pub model: String, pub effort: Option<String>, pub timeout_secs: u64, pub pass_lines: u64 }`, and `pub fn resolve_review(loaded: &Loaded) -> (Option<ResolvedReview>, Source, Option<String>)`:
  - Checks explicit `[review] provider` → explicit legacy `[review] agent` (inferring `Provider::infer_from_binary(&agent).unwrap_or(Provider::Cursor)` with `Source::User`) → `FACTORY_REVIEW_PROVIDER` env (case-insensitive; invalid non-empty returns `(None, Source::Default, Some(warning))`) → `(None, Source::Default, None)`.
  - When `provider` is `ReviewProvider::Provider(p)`, resolves `model` from explicit `[review] model` or `p.default_model()`, `effort` normalized to `None` when `p.effort_levels().is_empty()`, and `binary` from explicit `[review] agent` when set.
- Remove `"workers"` from `PROJECT_TOP_KEYS` (`src/config.rs:164`) and remove project `[workers]` merging in `load_with_project` (`:238-255`). Update the three pinned tests:
  - `src/config.rs:575` (`the_project_file_overrides_the_user_file_key_by_key`): test project `[run] budget` overriding user config while `[workers]` in `docs/project.toml` warns `unknown key \`workers\` ignored` and leaves user `[workers]` untouched.
  - `src/docs/project.rs:249-261` (`the_driver_tables_and_docs_share_the_file_without_warnings`): remove `[workers.polish]` from `SHARED` (or assert `[run]` shares cleanly without warnings while `[workers]` warns).
  - `tests/knobs.rs:40-159` (`a_project_file_sets_the_polish_model_and_the_row_says_so` → update to configure `[workers.polish] model = "sonnet"` in user `FACTORY_CONFIG_DIR/config.toml`, asserting `source: "user"` and `polish sonnet/high user/default`, plus asserting `[workers]` in `docs/project.toml` warns on stderr and does not override user config).
- Add `Patch { pub table: String, pub key: String, pub value: Option<toml::Value> }` and `pub fn patch(config_dir: &Path, edits: &[Patch]) -> Result<PathBuf>` supporting dotted table headers (`"cockpit"`, `"workers"`, `"workers.build"`, `"review"`, `"ci"`), preserving comments, symlinks, and permissions, validating every edit against `toml::from_str::<toml::Table>` before `write_synced` + atomic `rename`. Re-implement `write_ci_mode` via `patch`.
- Add `("FACTORY_REVIEW_PROVIDER".to_string(), "cursor".to_string())` in `tests/harness/mod.rs:620` right next to `FACTORY_CI_MODE`.

**The trap**: `src/config.rs:540-554` validates `[workers]` subkeys expecting every key under `[workers]` to be a role in `ROLE_KEYS`. Now `[workers]` accepts both scalar defaults (`provider`, `model`, `effort`) AND role subtables (`build`, `fix`, `checkpoint`, `polish`); `unknown_keys` must allow `WORKER_KEYS` at `[workers]` top level while still warning on unknown keys! Also watch `src/config.rs:490-495` (`scope`) when `[workers]` has top-level `model`/`effort` and `[workers.build]` does not.

**Acceptance:** Unit tests `config_loads_multi_provider_schema_maps_legacy_agent_and_ignores_project_workers` and `config_patch_updates_multiple_sections_atomically_preserving_comments` pass, plus the updated `src/docs/project.rs` and `tests/knobs.rs` tests pass:
- Switching `[workers.build]` to `provider = "codex"` with `[workers] provider = "claude", model = "opus", effort = "high"` resolves `build` to `codex` / `"gpt-5.4"` / `Some("high")` (not `"opus"`), and `[workers.fix] provider = "cursor"` resolves `effort` to `None` (fails if provider-scoped inheritance or Cursor normalization is removed).
- Legacy `[review] agent = "cursor-agent"` without `provider` resolves to `ReviewProvider::Provider(Provider::Cursor)` with `Source::User` (fails if legacy inference is removed).
- `[workers]` in `docs/project.toml` emits `unknown key \`workers\` ignored` and does not alter `config.workers` (fails if `PROJECT_TOP_KEYS` still merges `[workers]`).
- `config::patch` patches `[cockpit]`, `[workers]`, `[workers.build]`, `[review]`, and `[ci]` in one call, preserves comments and symlinks, removes keys set to `None`, refuses inline-table edits without mutating the file, and leaves zero `.tmp` files (fails if atomic patcher is broken).

### 3. Verify — medium — blocked by: 1, 2
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q`

**Acceptance:** inner loop green, one commit.

### 4. Prove — at the checkpoint — blocked by: 3
Drive these rows from [`prd.md`](prd.md) §9 QA plan at CP-1:
- **FR-1**: run `cargo test provider_registry_roundtrips_infers_and_probes_without_executing_shims` and `cargo test discover_models_parses_live_outputs_and_kills_hung_probe`.
- **FR-2**: run `cargo test config_loads_multi_provider_schema_maps_legacy_agent_and_ignores_project_workers` and `cargo test config_patch_updates_multiple_sections_atomically_preserving_comments`.

**Acceptance:** both FR rows have passing test transcripts captured in checkpoint evidence and recorded in **Progress**.
