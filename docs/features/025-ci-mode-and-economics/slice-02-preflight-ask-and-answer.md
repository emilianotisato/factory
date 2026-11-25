# Slice 02 — Preflight CI ask and persistent answer

**Source**: [`prd.md`](prd.md) → §4 (FR-2). **Depends on**: 01 _(uses `CiMode`, `resolve_ci_mode`, `write_ci_mode`, `Run::set_ci_mode`, and edits `src/cli/run.rs`)_
**Created**: 2026-11-25

**Progress**:
- Checkpoint 1, FR-2 (`preflight_asks_when_ci_mode_is_unset_and_exits_two`, `answer_local_writes_config_and_clears_future_preflight`, `answer_proceed_keeps_config_unset_and_runs_remote`, `answer_invalid_ci_mode_refuses_and_keeps_ask_open` green): cp-1/FR-2/tests.txt

**Slice**: **Highest-risk slice** — any mistake in the preflight ask trigger or exit-code logic would park runs unexpectedly or break existing preflight tests. When CI mode is unset in `~/.config/factory/config.toml` (and no `--ci` flag or `FACTORY_CI_MODE` env var is present), `factory docs preflight` exits 2 with a clear `!` ask and `factory run` opens one `ASK-n` warning about `local` vs `remote` tradeoffs. Answering `local` or `remote` via `factory answer ASK-n` persists `[ci] mode` atomically to `config.toml`, updates `state.json` and `runs.knobs`, and wakes the run; `proceed` runs `remote` once without writing `config.toml`; any other text refuses cleanly with exit 1 leaving the ask open.

## Current code / environment

- **`src/docs/preflight.rs:88-242`**: `preflight::run` is pure repo-only (`Docs::preflight` calls it; `tests/docs.rs:33-35` asserts its output). Do **not** add user-config checks inside `preflight::run`.
- **`src/docs/preflight.rs:360-419`**: `second_opinion` is layered onto `pf` inside `preflight::cli` (`:381-386`) and `cli::run::run` (`src/cli/run.rs:101`). **Trap**: `preflight::cli` (`:382`) currently returns the `code` returned by `preflight::run` before any user-config lines are pushed! Adding an `ask` line inside `preflight::cli` requires recomputing `code` (`1` if `!pf.refuse.is_empty()`, `2` if `!pf.ask.is_empty()`, else `0`) before returning.
- **`src/cli/run.rs:101-129`**: Calls `preflight::second_opinion(&loaded.config.review, &mut pf)`, ensures lanes, then loops over `pf.ask` creating `ASK-1`, `ASK-2`, … in `Phase::WaitingOwner` and exiting 0. Note that `find_resumable` (`:72-90`) returns before `:101` — pre-upgrade in-progress runs are deliberately not asked retroactively (*prd §2 & §10*).
- **`src/cli/answer.rs:248-358`**: `answer_ask` finds the open `EventKind::AskOpened { id, text }`, logs `Kind::Ask` in `log.jsonl`, emits `EventKind::AskAnswered`, writes `answers/ASK-n.json`, and calls `wake(&run, out)` (`:344`) when `asks_pending` is empty.
- **`src/cli/run.rs:219-229`**: `budget_knob` shows the exact pattern for updating a field in `runs.knobs` JSON after run creation.
- **`tests/preflight.rs`**: Uses `Fixture::new`, which now sets `FACTORY_CI_MODE=remote` via slice 01 (`tests/harness/mod.rs`). New tests in `tests/preflight.rs` testing the unset-CI path call `.env_remove("FACTORY_CI_MODE")` on their command so existing tests in `tests/preflight.rs` remain completely untouched (`assets/autonomy.md:62`).
- **Deliberate non-changes**:
  - *prd §4*: Ask whenever CI mode is unresolved regardless of whether `.github/workflows/` exists.
  - *prd §4*: `skills/deliver/SKILL.md` edits live in slice 04 so no two slices touch `skills/` or `skills/bundle.lock`.

## Tasks

### 1. Add `preflight::ci_mode` and wire into `preflight::cli` and `cli::run::run` — high — blocked by: —
**Source**: §4 FR-2: `CI_MODE_ASK_PREFIX`, `preflight::ci_mode`, recomputed exit code in `preflight::cli`, wired beside `second_opinion` in `cli::run::run`.

`src/docs/preflight.rs`, `src/cli/run.rs`.
- Export `pub const CI_MODE_ASK_PREFIX: &str = "ci mode not set in config.toml";` and `pub fn ci_mode(loaded: &config::Loaded, flag: Option<CiMode>, pf: &mut Preflight)`.
- Inside `ci_mode`, call `config::resolve_ci_mode(loaded, flag)`:
  - When `Some(m)` → `pf.ok.push(format!("ci mode: {m} ({source})"))` (where `source` is `"flag"` / `"user"` / `"env"`).
  - When `None` → `pf.ask.push(...)` starting with `CI_MODE_ASK_PREFIX`, naming `local` (skips remote CI watch, stamps `[skip ci]`, relies on local gate, saves org Actions minutes, needs local CPU; locked default branches still need one CI run when marked ready to merge), `remote` (pushes and watches GitHub Actions on every PR, lighter on your machine, burns org Actions minutes), and `proceed` (`remote` for this run only without editing `config.toml`).
- Call `ci_mode(&loaded, None, &mut pf)` in `preflight::cli` (`src/docs/preflight.rs:383-386`) and recompute `code = if !pf.refuse.is_empty() { 1 } else if !pf.ask.is_empty() { 2 } else { 0 }`.
- Call `preflight::ci_mode(&loaded, args.ci, &mut pf)` in `cli::run::run` (`src/cli/run.rs:101`) right next to `preflight::second_opinion`.

**The trap**: `preflight::cli` calls `config::load(&paths::config_dir())` (`:383`) — if `config.toml` fails to parse, handle the `Err` gracefully like `second_opinion` does (`:385`), without panicking. Also ensure `refuse` still wins over `ask` (exit `1`, never `2`) when both are non-empty.

**Acceptance:** `preflight_asks_when_ci_mode_is_unset_and_exits_two` in `tests/preflight.rs` — with `.env_remove("FACTORY_CI_MODE")` and no `[ci]` in `config.toml`: `factory docs preflight 001` exits `2` and prints `! ci mode not set in config.toml…`; with a refusal (dirty tree) it exits `1`; `factory run 001` exits `0`, parks in `waiting-owner`, and opens `ASK-1` naming `local`, `remote`, and `proceed`; `factory run 001 --ci local` bypasses the ask without writing `config.toml`. **Fails if removed**: without recomputing `code` in `preflight::cli`, `factory docs preflight 001` exits `0` instead of `2`.

### 2. Persist CI-mode `ASK-n` answer in `factory answer` — high — blocked by: 1
**Source**: §4 FR-2: `answer_ask` persists `local`/`remote` to `config.toml`, keeps `config.toml` untouched on `proceed`, refuses invalid text leaving the ask open.

`src/cli/answer.rs`.
- In `answer_ask` (`:299-358`), before writing `log.jsonl`, `events.jsonl`, or `answers/ASK-n.json`, check if `item.starts_with(preflight::CI_MODE_ASK_PREFIX)`:
  - Match `text.trim().to_ascii_lowercase().as_str()`:
    - `"local"` → `config::write_ci_mode(&paths::config_dir(), CiMode::Local)?`, `run.set_ci_mode(Some(CiMode::Local))?`, update `runs.knobs` JSON with `{"value": "local", "source": "user"}`, note the saved config path on stdout.
    - `"remote"` → same for `CiMode::Remote`.
    - `"proceed"` → do **not** write `config.toml`; `run.set_ci_mode(Some(CiMode::Remote))?`, update `runs.knobs` JSON with `{"value": "remote", "source": "default"}`.
    - `_` → print an error naming the valid answers (`local`, `remote`, `proceed`), write nothing to `config.toml`, `log.jsonl`, `events.jsonl`, or `answers/`, and return `Ok(1)`.
- Proceed with the normal `log::add`, `AskAnswered`, `answers::write`, and `wake(&run, out)` only after validation and persistence succeed.

**The trap**: Validation and `write_ci_mode` MUST run **before** `log::add` (`:315`), `run.emit(AskAnswered)` (`:321`), and `answers::write` (`:326`), otherwise a rejected typo or write failure would partially close the ask.

**Acceptance:** Three tests in `tests/preflight.rs` (all with `.env_remove("FACTORY_CI_MODE")`):
- `answer_local_writes_config_and_clears_future_preflight`: `factory answer ASK-1 "local"` exits `0`, writes `[ci]\nmode = "local"` to `config.toml`, sets `ci_mode = "local"` on `state.json`, and a follow-up `factory docs preflight 001` exits `0` with `✓ ci mode: local (user)` and no CI ask. **Fails if removed**: `config.toml` stays absent and the follow-up preflight still exits `2`.
- `answer_proceed_keeps_config_unset_and_runs_remote`: `factory answer ASK-1 "proceed"` leaves `config.toml` absent, sets `ci_mode = "remote"` on `state.json`, and wakes the run. **Fails if removed**: `config.toml` gets written or `ci_mode` is not `"remote"`.
- `answer_invalid_ci_mode_refuses_and_keeps_ask_open`: `factory answer ASK-1 "bogus"` exits `1`, leaves `config.toml` absent, writes nothing to `log.jsonl`, emits no `ask_answered`, writes no `answers/ASK-1.json`, and `ASK-1` remains open. **Fails if removed**: `bogus` exits `0` and closes `ASK-1`.

### 3. Verify — medium — blocked by: 2
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q`

**Acceptance:** inner loop green, one commit.

### 4. Prove — at the checkpoint — blocked by: 3
`prd.md` §8, the **FR-2** row: `preflight_asks_when_ci_mode_is_unset_and_exits_two`, `answer_local_writes_config_and_clears_future_preflight`, `answer_proceed_keeps_config_unset_and_runs_remote`, `answer_invalid_ci_mode_refuses_and_keeps_ask_open`. All four are `run` rows, proven below the UI: the checkpoint runs them and keeps the transcript.

**Acceptance:** after the checkpoint, the transcript shows all four tests green.
