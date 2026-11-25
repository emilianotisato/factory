# Slice 01 — CI mode config, atomic writer, and --ci flag

**Source**: [`prd.md`](prd.md) → §3 (FR-1). **Depends on**: — _(independent spine: slices 02 and 03 build on `CiMode`, `resolve_ci_mode`, `write_ci_mode`, `RunState.ci_mode`, and the test-harness default)_
**Created**: 2026-11-25

**Progress**:
- Checkpoint 1, FR-1 (`ci_mode_reads_user_config_flag_and_env_never_project_toml`, `write_ci_mode_preserves_comments_and_updates_idempotently`, `run_flag_ci_persists_across_daemon_and_resume` green): cp-1/FR-1/tests.txt

**Slice**: Adds user-only CI mode (`[ci] mode = "local" | "remote"` in `~/.config/factory/config.toml`), a pure `resolve_ci_mode` helper (`--ci` → user `config.toml` → `FACTORY_CI_MODE` → unset), an atomic comment-preserving `write_ci_mode` writer, `--ci <local|remote>` on `factory run` persisted to `state.json` and `runs.knobs`, and `FACTORY_CI_MODE=remote` in `tests/harness/mod.rs` so all existing tests stay untouched. Preflight asking (slice 02) and local closeout behavior (slice 03) follow; until they land, behavior for existing runs is unchanged.

## Current code / environment

- **`src/config.rs:16-21`**: `Config` holds `budget_weighted_tokens`, `workers`, `source`, `review`. `TOP_KEYS` (`:125`) lists `["budget_weighted_tokens", "workers", "source", "review"]`. `PROJECT_TOP_KEYS` (`:126`) lists `["docs", "lanes", "leaks", "run", "workers"]`. `unknown_keys` (`:322-360`) checks subtables `run`, `source`, `review`, `workers`.
- **`src/config.rs:106-109`**: `Sources` holds `budget: Source` and `workers: RoleSources`. `knobs()` (`:218-241`) builds the JSON saved to `runs.knobs`.
- **Keep `config::load` / `load_with_project` pure file-only**: `src/config.rs:418` asserts `assert_eq!(l.config, Config::default())` inside unit tests. Reading `FACTORY_CI_MODE` inside `load` would break unit tests whenever the outer process sets that env var. Put env resolution in a separate helper (`resolve_ci_mode`).
- **No `toml_edit` in `Cargo.toml`**: only `toml = "1.1"` (`Cargo.toml:26`), which drops comments on re-serialization. For `write_ci_mode`, patch lines inside the `[ci]` table (or append `\n[ci]\nmode = "<mode>"\n`), parse-validate the whole result with `toml::from_str::<toml::Table>`, then write via temp file in `config_dir` + rename (`docs/decisions.md#dec-3`).
- **`src/cli/run.rs:16-29`**: `RunArgs` has `feature`, `budget`, `foreground`, `hypothesis`. `hypothesis_ignored` (`:193-198`) prints a stderr warning when `--hypothesis` is passed on a live or resumed run. `record_start` (`:200-216`) writes `config::knobs` into `runs.knobs`.
- **`src/run.rs:94-126`**: `RunState` uses `#[serde(default, skip_serializing_if = "Option::is_none")]` for optional fields added after v1 (`paused_until`, `pr_url`, `pid_start`). Add `ci_mode: Option<CiMode>` in the same style plus `Run::set_ci_mode(&mut self, mode: Option<CiMode>) -> Result<()>`.
- **`tests/harness/mod.rs:580-629`**: `build_env` creates an empty `FACTORY_CONFIG_DIR` for every fixture. Adding `("FACTORY_CI_MODE".to_string(), "remote".to_string())` here keeps `tests/run.rs:607`, `tests/closeout.rs:2287`, `tests/preflight.rs:51,125`, and `tests/docs_lifecycle.rs:184` untouched and green (`assets/autonomy.md:62`).
- **Deliberate non-changes**:
  - *prd §3 & §10*: do **not** add `ci` to `PROJECT_TOP_KEYS` (`docs/project.toml` must warn on `[ci]` and never override user config).
  - *prd §2 & §3*: do **not** change `knobs_text` (`src/cli/report.rs:732-768`), which `tests/knobs.rs:141` pins via `ends_with("lanes 1 · factory ... · claude 2.1.296")`.
  - *prd §3 & §10*: passing `--ci` when resuming an existing run warns on stderr (`--ci ignored: run <id> already exists`) and leaves `state.ci_mode` untouched — never update `ci_mode` on resume.

## Tasks

### 1. Add `CiMode`, `[ci]`, `resolve_ci_mode`, `knobs.ci`, and `write_ci_mode` — high — blocked by: —
**Source**: §3 FR-1: user-only `[ci] mode` in `config.toml`, `FACTORY_CI_MODE` env fallback outside `load`, `ci` in `knobs` JSON, atomic comment-preserving `write_ci_mode`.

`src/config.rs`.
- Define `#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)] #[serde(rename_all = "lowercase")] pub enum CiMode { Local, Remote }` (with `Display` / `as_str()` returning `"local"` / `"remote"`) and `#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)] #[serde(default)] pub struct Ci { pub mode: Option<CiMode> }`.
- Add `pub ci: Ci` to `Config`, `pub ci: Source` to `Sources`, `"ci"` to `TOP_KEYS`, `CI_KEYS: [&str; 1] = ["mode"]` checked in `unknown_keys`. Set `loaded.sources.ci = scope(&table, &["ci", "mode"], Source::User)` in `load`.
- Add `pub fn resolve_ci_mode(loaded: &Loaded, flag: Option<CiMode>) -> (Option<CiMode>, Source, Option<String>)`: `flag` → `(Some(m), Source::Flag, None)`; else `loaded.config.ci.mode` → `(Some(m), Source::User, None)`; else inspect `std::env::var("FACTORY_CI_MODE")`: `"local"` / `"remote"` (trimmed, ASCII case-insensitive) → `(Some(m), Source::Default, None)`; non-empty invalid value → `(None, Source::Default, Some(warning))`; unset/empty → `(None, Source::Default, None)`.
- Include `"ci": {"value": loaded.config.ci.mode, "source": loaded.sources.ci}` in `knobs()` JSON (leave `knobs_text` untouched).
- Implement `pub fn write_ci_mode(config_dir: &Path, mode: CiMode) -> Result<PathBuf>`: create `config_dir` if absent, read existing `config.toml` if present, update `mode = "..."` inside an existing `[ci]` section (or insert it if `[ci]` has no `mode` line, or append `\n[ci]\nmode = "<mode>"\n` if no `[ci]` section exists), verify with `toml::from_str::<toml::Table>`, then write to a unique temp file in `config_dir` and `fs::rename` over `config.toml`.

**The trap**: A `[ci]` section followed by another `[table]` header must stop its replacement scan at the next `[` header so a `mode = ...` key in another section is never touched; also handle files without a trailing newline cleanly.

**Acceptance:**
- `ci_mode_reads_user_config_flag_and_env_never_project_toml` in `src/config.rs` (or `tests/run.rs`): verifies `local`/`remote` load from user `config.toml`, unknown `[ci]` key warns `unknown key \`ci.<k>\` ignored`, invalid `mode` returns `Err`, `[ci]` in `docs/project.toml` warns and leaves user mode intact, and `resolve_ci_mode` respects `flag > user > FACTORY_CI_MODE > None`. **Fails if removed**: removing `"ci"` from `TOP_KEYS` or adding it to `PROJECT_TOP_KEYS` breaks the warning/override assertions.
- `write_ci_mode_preserves_comments_and_updates_idempotently` in `src/config.rs`: creates missing `config.toml`, appends `[ci]` to an existing file keeping leading comments and `[source]`/`[review]`/`[workers]` byte-for-byte, updates `local` → `remote` idempotently without duplicating `[ci]`, and leaves no leftover temp file in `config_dir`. **Fails if removed**: re-serializing via `toml::to_string` strips the comments and fails the comment-preservation assertion.

### 2. Wire `--ci`, `RunState.ci_mode`, and harness env default — high — blocked by: 1
**Source**: §3 FR-1: `--ci <local|remote>` on `RunArgs`, `ci_mode` persisted on `RunState` and `runs.knobs`, resume warns and ignores `--ci`; §2 Blast radius: `FACTORY_CI_MODE=remote` in `tests/harness/mod.rs`.

`src/run.rs`, `src/cli/run.rs`, `tests/harness/mod.rs`.
- Add `#[serde(default, skip_serializing_if = "Option::is_none")] pub ci_mode: Option<CiMode>` to `RunState` (`src/run.rs:94-126`) and `Run::set_ci_mode(&mut self, mode: Option<CiMode>) -> Result<()>`.
- Add `#[arg(long, value_enum)] pub ci: Option<CiMode>` to `RunArgs` (`src/cli/run.rs:16-29`).
- On `Run::find_live` (`:61`) and `Run::find_resumable` (`:72`): if `args.ci.is_some()`, print `--ci ignored: run {id} already exists` on stderr (next to `hypothesis_ignored`) without changing `state.ci_mode`.
- On new runs in `cli::run::run`: resolve via `config::resolve_ci_mode(&loaded, args.ci)` (printing any env warning to stderr), store `loaded.config.ci.mode = resolved; loaded.sources.ci = source;` so `record_start` records the resolved value/source in `runs.knobs`, and persist `ci_mode` on the new `Run` (`run.set_ci_mode(resolved)?`).
- In `tests/harness/mod.rs:609-628` (`build_env`), add `("FACTORY_CI_MODE".to_string(), "remote".to_string())`.

**The trap**: `daemon()` (`src/cli/run.rs:167-191`) reloads `config.toml` from disk; it must not clobber `run.state().ci_mode` set by `--ci` on the new run. Persisting `ci_mode` in `state.json` guarantees the detached daemon sees `--ci`.

**Acceptance:** `run_flag_ci_persists_across_daemon_and_resume` in `tests/run.rs` — `factory run 001 --ci local --foreground` writes `ci_mode: "local"` into `state.json` and `knobs.ci == {"value": "local", "source": "flag"}` in `factory.db`; a second run parked on an ask (`lanes = 1`, unchained) started with `--ci local` keeps `ci_mode: "local"` when resumed with `factory run 001 --ci remote`, and stderr contains `--ci ignored: run <id> already exists`. **Fails if removed**: without persisting `ci_mode` on `RunState`, `state.json` lacks `ci_mode` and the assertion fails.

### 3. Verify — medium — blocked by: 2
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q`

**Acceptance:** inner loop green, one commit.

### 4. Prove — at the checkpoint — blocked by: 3
`prd.md` §8, the **FR-1** row: `ci_mode_reads_user_config_flag_and_env_never_project_toml`, `write_ci_mode_preserves_comments_and_updates_idempotently`, `run_flag_ci_persists_across_daemon_and_resume`. All three are `run` rows, proven below the UI: the checkpoint runs them and keeps the transcript.

**Acceptance:** after the checkpoint, the transcript shows all three tests green.
