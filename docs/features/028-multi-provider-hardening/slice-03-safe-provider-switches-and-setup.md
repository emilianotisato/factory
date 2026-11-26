# Slice 03 — Safe provider switches and setup/preflight fixes

**Source**: [`prd.md`](prd.md) → §3 (FR-1). **Depends on**: — _(independent of Slices 01–02; lands before Slice 04 because both edit `src/docs/preflight.rs`)_
**Created**: 2026-11-26

**Progress**: cp-1/FR-1/transcript.txt

**Slice**: Clears stale `model`/`effort` on provider-less `[workers.<role>]` tables whenever `[workers] provider` switches (via `factory setup workers`, the setup wizard, or `factory answer ASK-n`), validates resolved worker model/effort in `preflight::worker_refusals` (and warns on bad `[review]` in `preflight::second_opinion`), and fixes `workers_step` default bracketing + repeated role edits, `config::role_sources` under `FACTORY_WORKER_PROVIDER` with `--worker`, and `setup --show` role spacing (`  {role:<12}`).

## Current code / environment

- **`resolve_role` & `role_sources`**: `src/config.rs:520-565` and `:821-851`.
  - In `role_sources` (`:540`), `let own = spec.is_none() || worker.provider.or(w.provider) == Some(r.provider);` ignores `FACTORY_WORKER_PROVIDER`, so when a `--worker` spec is present (`spec.is_some()`), a role whose provider came from `FACTORY_WORKER_PROVIDER` gets `own = false` and mislabels its model as `default` instead of `user`.
  - `ROLE_KEYS` (`["build", "fix", "checkpoint", "polish"]`) already exists in `src/config.rs`.
- **`setup::worker_edits`, `worker_flags`, `workers_step`, `show`**: `src/cli/setup.rs:380-455`, `:645-675`, `:875-966`.
  - `worker_edits` (`:399-434`) only clears `model`/`effort` on `now.table` (`"workers"`), leaving `[workers.<role>]` tables untouched.
  - `show` (`:655`) formats `  {role:<10}{}` (`"  checkpoint"` has 0 spaces before its provider). Top-level labels (`"review      "`, `:646-671`) are width 12 — keep top-level labels untouched so `tests/setup.rs:251` stays green!
  - `workers_step` (`:883-885`) computes `suggested = now.own_provider.or_else(|| detected.first().map(|d| d.provider))` (ignoring `FACTORY_WORKER_PROVIDER` and legacy Claude model/effort), and in its role-edit loop (`:927-964`) reads `table_now(loaded, role)` from the pre-wizard `loaded` struct on every iteration, so editing the same role twice loses the first edit.
- **`cli::answer::worker_answer`**: `src/cli/answer.rs:474-570`.
  - Lines `517-531` mutate `planned.config.workers` in memory and call `config::resolve_workers(&planned, &[])` BEFORE patching `config.toml` (`:551-558`). Must clear stale provider-less role `model`/`effort` on BOTH `planned.config.workers` in-memory AND in the `config::patch` list!
- **`preflight::worker_refusals` & `second_opinion`**: `src/docs/preflight.rs:487-507` (and `second_opinion` in the same file).
  - `worker_refusals` checks `resolve_binary(p)` and `w.model.is_empty()`, but does NOT check `provider::is_model_name(&w.model)` or `w.provider.effort_levels()`.
- **Authorized test in `tests/setup.rs`**: `tests/setup.rs::setup_workers_configures_mixed_roles` (`:462`) asserts `"  fix       cursor grok-4.7-high ("`; declared in `prd.md` `authorized_tests`. `tests/setup.rs:251` asserts `"review      codex gpt-5.4 · high (user)"` — do NOT widen top-level labels.
- **`testguard` rule on `tests/setup.rs`**: Only `setup_workers_configures_mixed_roles` is authorized for modification. Add any new `use` imports on separate lines.

## Tasks

### 1. Add `config::stale_role_clears` and fix `config::role_sources` — high — blocked by: —
**Source**: §3 FR-1: "Add a shared helper `config::stale_role_clears(loaded: &Loaded, env_provider: Option<Provider>, new_provider: Provider) -> Vec<Patch>`", "In `config::role_sources`: account for `FACTORY_WORKER_PROVIDER`".

`src/config.rs`.
1. Add `pub fn stale_role_clears(loaded: &Loaded, env_provider: Option<Provider>, new_provider: Provider) -> Vec<Patch>`:
   - Compute `prev = loaded.config.workers.provider.or(env_provider).unwrap_or(Provider::Claude)`.
   - If `new_provider == prev`, return `Vec::new()`.
   - For each `role` in `ROLE_KEYS`: inspect `loaded.config.workers.role(role)` and `loaded.sources.workers.role(role)`. When `worker.provider.is_none()`:
     - If `set.model != Source::Default` (explicitly set in `config.toml`), push `Patch { table: format!("workers.{role}"), key: "model".into(), value: None }`.
     - If `set.effort != Source::Default`, push `Patch { table: format!("workers.{role}"), key: "effort".into(), value: None }`.
2. Add a small helper (e.g. `pub fn clear_stale_roles_in_memory(loaded: &mut Loaded, env_provider: Option<Provider>, new_provider: Provider)`) or clear the corresponding role fields in `Workers` when `stale_role_clears` fires, so `worker_answer` can keep `planned` and disk patches in exact sync.
3. In `role_sources` (`src/config.rs:520-565`): read `env_provider` (passing it from `worker_knobs` / reading `env_worker_provider()`, matching `resolve_role` at `:829-831`) and change `:540` to `let own = spec.is_none() || worker.provider.or(w.provider).or(env_provider) == Some(r.provider);`.
4. Add unit tests in `src/config.rs` proving:
   - `stale_role_clears` clears `[workers.build] model`/`effort` when switching `claude → codex`, leaves roles with their own `provider = "claude"` intact, and returns empty when `env_provider == Some(Provider::Codex)` and `new_provider == Provider::Codex`.
   - `role_sources` labels a role's model `Source::User` when `FACTORY_WORKER_PROVIDER=claude` matches a provider-less `[workers.build] model = "sonnet"` alongside `--worker claude`.

**The trap**: Only clear `model`/`effort` keys that were actually set in `config.toml` (`set.model != Source::Default` / `set.effort != Source::Default`), NOT `Worker::default()` values (`"opus"`/`"sonnet"`), otherwise `stale_role_clears` would emit spurious removals for untouched role tables!

**Acceptance:** `src/config.rs` unit tests fail if `env_provider` is ignored in either `stale_role_clears` or `role_sources`, or if a role with its own `provider` is wrongly cleared.

### 2. Wire `stale_role_clears` into `setup` and `worker_answer`, fix `workers_step` and `setup --show` — high — blocked by: 1
**Source**: §3 FR-1: "Call `stale_role_clears` from BOTH `cli::setup::worker_flags` / `workers_step` AND `cli::answer::worker_answer`", "Fix `workers_step`, `role_sources`, and `setup --show` spacing".

`src/cli/setup.rs`, `src/cli/answer.rs`.
1. In `src/cli/setup.rs`:
   - In `worker_flags` (when `role.is_none()` and a new provider is chosen) and in `workers_step` (after computing default `[workers]` edits for chosen `p`): append `config::stale_role_clears(loaded, env_provider, p)` to `edits` (where `env_provider` reads `FACTORY_WORKER_PROVIDER`).
   - In `workers_step` (`:882-966`):
     - Compute `suggested = now.own_provider.or(env_provider).or_else(|| (now.model.is_some() || now.effort.is_some()).then_some(Provider::Claude)).or_else(|| detected.first().map(|d| d.provider))`.
     - In the per-role edit loop (`:927-964`), keep a mutable working copy of `Loaded` (updated after default `[workers]` edits and after each role edit, and deduplicating `edits` by `(table, key)` so a later edit to the same role replaces the earlier one) so editing the same role twice in one wizard session sees the first edit rather than stale pre-wizard state.
   - In `show` (`:655`): change `format!("  {role:<10}{}", ...)` to `format!("  {role:<12}{}", ...)` — do NOT change top-level label widths (`config      `, `review      `, etc.).
2. In `src/cli/answer.rs` (`worker_answer`, `:516-565`):
   - Compute `env_provider` (`FACTORY_WORKER_PROVIDER`) and `let clears = config::stale_role_clears(&planned, env_provider, p);`.
   - Before `config::resolve_workers(&planned, &[])`, clear the stale provider-less role `model`/`effort` fields on `planned` in-memory so `resolved` (and the current run's `worker_config`) immediately gets `p`'s default model instead of the old Claude slug.
   - Append `clears` to the patch slice passed to `config::patch(&config_dir, &patches)?`, and report cleared role keys in output when any were cleared.

**The trap**: In `workers_step`, if the user switched default `[workers]` from `claude` to `codex` and then ALSO overrides `build` inside the same wizard session, the later explicit `build` edit must supersede the stale-clear patch for `workers.build` (deduplicating `edits` by `(table, key)` keeping the last patch handles this cleanly).

**Acceptance:** `setup workers --provider codex`, the setup wizard, and `answer ASK-1 codex` all clear stale provider-less role overrides on disk (and in memory for `answer`); editing `build` twice in `workers_step` retains the second pass cleanly; `setup --show` prints `  checkpoint  <provider>`.

### 3. Validate resolved model/effort in `preflight::worker_refusals` and warn on bad `[review]` — high — blocked by: 1
**Source**: §3 FR-1: "Validate resolved model & effort in `preflight::worker_refusals` (`src/docs/preflight.rs:487-507`) and warn on bad `[review]` (`src/docs/preflight.rs`)".

`src/docs/preflight.rs`.
1. In `worker_refusals(resolved: &config::ResolvedWorkers) -> Vec<String>` (`:487-507`):
   - Inside `for (role, w) in resolved.roles()`:
     - Keep the existing `w.model.is_empty()` check.
     - Else if `!provider::is_model_name(&w.model)`: push `format!("{role} worker model {:?} is not a valid model id — set one with `factory setup workers --role {role} --model <m>`", w.model)`.
     - If `let Some(e) = &w.effort`: if `w.provider.effort_levels().is_empty()` or `!w.provider.effort_levels().contains(&e.as_str())`, push `format!("{role} worker effort {e:?} is not valid for {} — set one with `factory setup workers --role {role}`", w.provider)`.
2. In `second_opinion(loaded: &config::Loaded, pf: &mut Preflight)` (for `ReviewProvider::Provider(p)`):
   - If `!r.model.is_empty()` and `!provider::is_model_name(&r.model)`: push a warning to `pf.judge` (NOT `pf.refuse`!).
   - If `let Some(e) = &r.effort` and (`p.effort_levels().is_empty()` || `!p.effort_levels().contains(&e.as_str())`): push a warning to `pf.judge` (NOT `pf.refuse`!).

**The trap**: Bad `[review]` config MUST go to `pf.judge` (warning) and must NOT set `pf.pass = false` or push to `pf.refuse`, because `[review]` degrades to local-review fallback instead of blocking a run!

**Acceptance:** `worker_refusals` refuses `[workers.build] model = "--bad"` and `[workers.build] provider = "codex"\neffort = "max"`; `second_opinion` emits a `pf.judge` warning (and leaves `pf.pass` true / `pf.refuse` empty) for `[review] model = "--bad"`.

### 4. Update authorized setup test and add integration test — high — blocked by: 2, 3
**Source**: §2 Blast radius & §3 FR-1 Acceptance.

`tests/setup.rs`.
1. Update authorized test `setup_workers_configures_mixed_roles` (`:462`) from `"  fix       cursor grok-4.7-high ("` to `"  fix         cursor grok-4.7-high ("`.
2. Add `provider_switch_clears_stale_role_slugs_and_preflight_refuses_bad_config` in `tests/setup.rs` proving:
   - With `[workers.build] model = "sonnet"` (no `provider`), running `factory setup workers --provider codex` clears `[workers.build] model`, prints the removal, and resolves `build` to `codex`'s default model.
   - With `FACTORY_WORKER_PROVIDER=codex` and `[workers.build] model = "gpt-5.4"`, running `factory setup workers --provider codex` preserves `gpt-5.4`.
   - In a run with an open worker ask and `[workers.build] model = "sonnet"`, running `factory answer ASK-1 codex` clears `[workers.build] model` both in `config.toml` AND in the run's saved `worker_config`.
   - A hand-edited `config.toml` with `[workers.build] model = "--bad"` or `[workers.build] provider = "codex"\neffort = "max"` is refused by `factory docs preflight` (exit 1), while `[review] model = "--bad"` emits a `?` judge warning without refusing.
   - `factory setup --show` outputs `"  checkpoint  "` (two spaces after `checkpoint`).

**Acceptance:** `cargo test --test setup provider_switch_clears_stale_role_slugs_and_preflight_refuses_bad_config` passes; removing any of the clears, validations, or spacing changes makes the test fail.

### 5. Verify — medium — blocked by: 4
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 6. Prove — at the checkpoint — blocked by: 5
**Source**: §8 QA plan FR-1 row.
- **FR-1**: run `cargo test --test setup provider_switch_clears_stale_role_slugs_and_preflight_refuses_bad_config -- --nocapture` and keep the transcript. Expected: passes proving stale role clears (setup + answer + env preservation), preflight worker refusals + review warning, and `setup --show` alignment.

**Acceptance:** after the checkpoint, the FR-1 row has transcript evidence in this slice's **Progress**.

**Code-review fixes (2026-11-26).**
- `factory setup workers --clear` on the default table now runs `stale_role_clears` so provider-less role slugs are dropped when the top-level `[workers]` provider is cleared, matching a normal provider switch.
