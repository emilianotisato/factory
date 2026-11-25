# Slice 03 — Interactive and scriptable `factory setup` wizard

**Source**: [`prd.md`](prd.md) → §5 (FR-3), §9, §11. **Depends on**: 01 (needs `Provider`, `ReviewProvider`, `detect_installed`, `discover_models`, extended `Config` schema, `resolve_review`, and `config::patch`)
**Created**: 2026-11-25

**Progress**:
- **Built:** `src/cli/setup.rs` (`SetupArgs`/`SetupSection`/`WorkerRole`, `--show [--json]`, flag mode, non-TTY usage guard, `run_wizard` over an injected `BufRead`) wired as `Command::Setup` in `src/main.rs` and `src/cli/mod.rs`. `tests/setup.rs` has the three FR-3 tests: the wizard in-process with stub CLIs on a synthetic PATH, and flags, `--show --json` and the non-TTY guard through the binary.
- **Inner loop:** 1 run, 0 red, ~2 min. Caught nothing.
- **Seen red once:** `setup_wizard_configures_all_sections_and_keeps_defaults` failed when Enter cleared saved keys: the second all-Enter run rewrote `config.toml`. `setup_non_interactive_flags_and_show_json` failed with flag handling disabled: `setup review --provider codex …` exited 1 with the usage text. `setup_non_tty_without_flags_exits_one` failed with the TTY guard removed: "factory [\"setup\"] blocked on a piped stdin" after 10 s. All three were green again once restored.
- **Deviations:** the spec gave `run_with(args, input, is_tty, config_dir, repo, out, err)`. It takes an `Env` struct instead (`config_dir`, `repo`, `path`, `is_tty`) because the PATH searched for CLIs is an input too, and eight positional parameters trip clippy. `run_wizard` takes the same `Env`. AD-9 covers what switching the review provider does to the legacy `agent` and the old model/effort. AD-10 covers Enter on an unset provider prompt saving the suggested CLI.
- **Prove (CP-2):** the FR-3 row is left for the checkpoint.

**Slice**: Adds the `factory setup` subcommand (`src/cli/setup.rs`) supporting interactive TTY wizard mode (`factory setup` and single-section `factory setup cockpit|workers|review|ci`), `--show [--json]` inspection without touching disk, and non-interactive scriptable flags for cockpit agents and automation. Leaves multi-provider `review.rs` & preflight ask to slice 04 and non-Claude worker provider selection to PRD-027.

## Current code / environment

- **CLI dispatch**: `src/main.rs:18-108` and `src/cli/mod.rs:1-18` register one module per top-level subcommand, each taking `out: &mut dyn Write` (and `err` / `stdin` where needed) and returning `Result<i32>` so tests drive it in-process.
- **Cockpit plugin installer**: `src/cli/cockpit.rs:34-55` exports `install(dir: &Path, out: &mut dyn Write) -> Result<i32>` for installing Claude Code's cockpit mod.
- **Skills verdict**: `src/skills.rs:448` exports `skills::verdict(root)` for reporting a repo's skills status when invoked inside a git repo.
- **Provider & config helpers (from slice 01)**: `provider::detect_installed()`, `provider::discover_models()`, `config::load()`, `config::resolve_review()`, `config::resolve_ci_mode()`, and `config::patch(config_dir, &edits)`.
- **Deliberate non-changes (PRD-026 §11)**:
  - No named config profiles (`[profiles.<name>]`, `factory setup use <name>`).
  - No cross-vendor same-family warning (cut 2026-11-25, §5 cut).
  - No `dialoguer` / `ratatui` crate dependency — plain numbered prompts over `BufRead` / `Write`.
  - In PRD-026, `factory setup workers` configures Claude `model` and `effort` (plus optional per-role overrides for `build|fix|checkpoint|polish`); non-Claude worker provider choice activates in PRD-027 so no dead worker provider option is written yet.

## Tasks

### 1. Implement `src/cli/setup.rs` and wire `Command::Setup` in `src/cli/mod.rs` and `src/main.rs` — high — blocked by: —
**Source**: §5 FR-3: "Add `src/cli/setup.rs` and top-level `Command::Setup(cli::setup::SetupArgs)` in `src/main.rs`... `factory setup [cockpit | workers | review | ci]`, `--show [--json]`, interactive TTY wizard, non-interactive flags".

`src/cli/setup.rs`, `src/cli/mod.rs`, `src/main.rs`.
- Export `pub mod setup;` in `src/cli/mod.rs` and wire `Command::Setup(cli::setup::SetupArgs)` in `src/main.rs` passing `&mut stdin().lock()`, `std::io::stdin().is_terminal()`, `&mut stdout()`, `&mut stderr()`.
- Define `SetupArgs`:
  - `#[arg(long)] pub show: bool`
  - `#[arg(long)] pub json: bool` (requires `--show` or implies `--show`)
  - `#[command(subcommand)] pub section: Option<SetupSection>` where `SetupSection` has:
    - `Cockpit { #[arg(long, value_enum)] provider: Option<Provider> }`
    - `Workers { #[arg(long, value_enum)] role: Option<WorkerRole>, #[arg(long)] model: Option<String>, #[arg(long)] effort: Option<String>, #[arg(long)] clear: bool }` (`WorkerRole`: `Default`, `Build`, `Fix`, `Checkpoint`, `Polish`)
    - `Review { #[arg(long)] provider: Option<ReviewProvider>, #[arg(long)] model: Option<String>, #[arg(long)] effort: Option<String> }`
    - `Ci { #[arg(long, value_enum)] mode: Option<CiMode> }`
- Implement `--show` / `--show --json`: reads `config::load(&paths::config_dir())`, resolves review & ci mode, calls `provider::detect_installed()`, and prints effective `cockpit`, `workers` (default + per-role), `review`, `ci`, their sources (`user` / `env` / `default`), and detected CLIs without writing anything.
- Implement non-interactive mode (when any section flag is supplied): validates flags, builds `Vec<config::Patch>`, calls `config::patch(&paths::config_dir(), &edits)` once, and prints what was saved.
- Implement non-TTY guard: when `!is_tty` and neither `--show` nor any section flag is set, print exact non-interactive flag usage to `err` and return `Ok(1)` without reading stdin.
- Implement interactive wizard (`run_wizard` taking `input: &mut dyn BufRead`, `out: &mut dyn Write`, `config_dir: &Path`, `repo: Option<&Path>`, `section: Option<&SetupSection>`):
  - Print detected CLIs once at the top.
  - Every prompt displays current value as `[default]`; empty line / Enter keeps current value.
  - **Cockpit**: numbered list of all 5 providers (marking installed ones). If `claude` chosen, prompt `Install/update Claude Code cockpit mod? [y/N]` and call `cli::cockpit::install` on `y`. If inside a `factory` repo, print `skills::verdict(repo)` line.
  - **Workers** (PRD-026 scope): prompt default `[workers]` `model` (curated Claude models + free text) and `effort` (`low|medium|high|max`), then `Override any role (build, fix, checkpoint, polish)? [y/N]` looping over chosen roles to set or clear `model`/`effort`.
  - **Review**: numbered list of the 5 providers + `none`. When a provider (not `none`) is chosen, run `provider::discover_models` (5 s timeout) if the binary is on `PATH`, show numbered suggestions + free-text entry for `model`, and prompt `effort` only when `!provider.effort_levels().is_empty()`. When `none` is chosen, clear `[review] model`/`effort`/`agent` or write `provider = "none"`.
  - **CI**: `local` vs `remote` with the one-line tradeoff from PRD-025.
  - Collect all `Patch` entries across selected section(s) and apply in **one** `config::patch(config_dir, &edits)` call, printing the config path written.

**The trap**: Keep `run_with(args, input, is_tty, config_dir, repo, out, err)` factored cleanly so tests never depend on a real PTY, real `HOME`, or real installed CLIs. Also ensure non-interactive `factory setup review --provider none` writes `provider = "none"` cleanly without requiring `--model`.

**Acceptance:** Integration tests in `tests/setup.rs` (`setup_wizard_configures_all_sections_and_keeps_defaults`, `setup_non_interactive_flags_and_show_json`, and `setup_non_tty_without_flags_exits_one`) pass:
- Scripted `BufRead` wizard configures `cockpit`, `workers` (default + role override), `review`, and `ci` in one atomic `config.toml` write, and a second wizard run with all blank lines (Enter) preserves every existing value unchanged (fails if Enter overwrites defaults or drops sections).
- `factory setup review --provider codex --model gpt-5.4 --effort high`, `factory setup ci --mode local`, `factory setup cockpit --provider agy`, and `factory setup workers --role polish --model sonnet --effort medium` update `config.toml` non-interactively and exit `0`, and `factory setup --show --json` reflects them without modifying `config.toml` mtime (fails if flags or `--show --json` are missing).
- Piping stdin (non-TTY) to `factory setup` with no flags exits `1` and prints usage mentioning `--provider` / `--mode` without hanging (fails if non-TTY guard is removed).

### 2. Verify — medium — blocked by: 1
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q`

**Acceptance:** inner loop green, one commit.

### 3. Prove — at the checkpoint — blocked by: 2
Drive this row from [`prd.md`](prd.md) §9 QA plan at CP-2:
- **FR-3**: run `cargo test setup_wizard_configures_all_sections_and_keeps_defaults`, `cargo test setup_non_interactive_flags_and_show_json`, and `cargo test setup_non_tty_without_flags_exits_one`.

**Acceptance:** FR-3 row has passing test transcripts captured in checkpoint evidence and recorded in **Progress**.
