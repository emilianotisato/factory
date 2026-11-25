# Slice 04 — Multi-provider adversarial review and preflight ask

**Source**: [`prd.md`](prd.md) → §2, §6 (FR-4), §9, §11. **Depends on**: 01 (needs `Provider`, `ReviewProvider`, `resolve_binary`, `config::resolve_review`, `config::patch`, and `FACTORY_REVIEW_PROVIDER=cursor` in `tests/harness/mod.rs`)
**Created**: 2026-11-25

**Progress**:
- **Built:** `src/review.rs` dispatches on `ResolvedReview`. `probe_resolved` runs Cursor `--list-models` (unchanged), Claude/Codex `--version`, and agy/OpenCode `models` + `lists_model`. `run_pass_resolved` passes each provider's exact flags: Claude with `worker::scrubbed` vars removed, Codex reading `review-n.prompt` as a file on stdin (`run_logged` takes `stdin: Option<&Path>`). `parse_stream_for` handles stream-json, Codex JSONL (the last `agent_message` becomes the result on `turn.completed`) and OpenCode JSONL (the last `text` on `step_finish` `stop`). `pass_header` shows `provider:model` for providers other than Cursor. `probe`/`run_pass`/`run_passes`/`collect::collect` still take a legacy `&Review` through `From<&Review> for ResolvedReview`. `ReviewProvider::None`: `review second` prints `disabled: …` and exits 2 with no log dir; `collect` writes `skipped (disabled in config)` (reusable). Unset: `review second` prints `unreviewed: adversarial reviewer not set …` and exits 2. Closeout re-reads `config.toml` and honours `RunState.review_override`. Preflight adds `REVIEW_ASK_PREFIX` (an ask listing the installed CLIs plus `none`/`proceed`), `disabled (user|env)`, and a harness-neutral judge line. `factory answer` handles `proceed` / `none` / `<provider>[:<model>[:<effort>]]` through `config::patch`; anything else exits 1 and leaves the ask open.
- **Inner loop:** 1 run, 0 red, ~2 min. Before it, one targeted `--test preflight` red: the old `one Claude agent` needle against the new judge text (AD-7).
- **Seen red once:** `review_second_runs_all_five_providers_and_none_mode` (Codex stdin left null: panicked "codex: " on an empty prompt) · `preflight_asks_when_review_unset_and_answer_persists_or_proceeds` (reviewer-answer dispatch removed: `bogus` answered with exit 0). Each green after restore.
- **Deviations:** AD-7 (decide): preflight needle repinned. AD-8 (fyi): with no reviewer chosen, collect keeps the legacy Cursor defaults. `Patch` values are built inline (config.rs has no `Patch::set`/`remove` helpers, and it belongs to slice 01).
- **Prove (CP-2):**
  - FR-1: cp-2/FR-1/provider_registry_roundtrips_infers_and_probes_without_executing_shims.txt, cp-2/FR-1/discover_models_parses_live_outputs_and_kills_hung_probe.txt (pass)
  - FR-2: cp-2/FR-2/config_loads_multi_provider_schema_maps_legacy_agent_and_ignores_project_workers.txt, cp-2/FR-2/config_patch_updates_multiple_sections_atomically_preserving_comments.txt (pass)
  - FR-3: cp-2/FR-3/setup_wizard_configures_all_sections_and_keeps_defaults.txt, cp-2/FR-3/setup_non_interactive_flags_and_show_json.txt, cp-2/FR-3/setup_non_tty_without_flags_exits_one.txt (pass)
  - FR-4: cp-2/FR-4/review_second_runs_all_five_providers_and_none_mode.txt, cp-2/FR-4/preflight_asks_when_review_unset_and_answer_persists_or_proceeds.txt (pass; mutation cp-2/FR-4/mutation-red.txt, cp-2/FR-4/mutation-restored.txt)
  - FR-5: cp-2/FR-5/skills_render_links_shared_into_agents_skills.txt, cp-2/FR-5/this_repos_claude_skills_link_into_the_source.txt, cp-2/FR-5/every_file_under_skills_is_in_the_bundle_byte_for_byte.txt, cp-2/FR-5/bundle_lock_matches.txt, cp-2/FR-5/skills_are_harness_neutral_and_distinguish_disabled_review.txt, cp-2/FR-5/the_cockpit_skill_is_under_sixty_lines_and_names_the_five_commands.txt (pass)

**Slice**: Replaces hardcoded Cursor-only invocation, probing, and stream parsing in `src/review.rs` with provider-dispatched adapters for all five providers (`claude`, `codex`, `cursor`, `agy`, `opencode`) plus `ReviewProvider::None` (`disabled:` / `skipped (disabled in config)`), adds `review_override` on `RunState` for `proceed` runs, and wires the one-time preflight ask-if-missing + persistent `factory answer` flow in `src/docs/preflight.rs`, `src/cli/run.rs`, and `src/cli/answer.rs`. Leaves `factory setup` to slice 03 and skills to slice 02.

## Current code / environment

- **Review engine**: `src/review.rs:18-25` defines Cursor-only `FLAGS`; `probe()` (`:573-616`) runs `<agent> --list-models`; `parse_stream()` (`:643-671`) parses Cursor/Claude NDJSON; `run_pass()` (`:682-714`) runs `<agent> -p <FLAGS> --model <model> <prompt>`; `pass_header()` (`:837-850`) formats `Pass n — ... (lines, s, model)`.
- **CLI & collect callers**: `src/cli/review.rs:79-85` loads `config::Review` via `config::load`; `src/collect.rs:109-137, :198-273` calls `review::probe` and `review::run_passes`, and `collect::reusable` (`:31-36`) rejects `fallback owed`, `did not run`, and `STOPPED`. `src/closeout.rs:242, :382` calls `collect::collect` during polish.
- **Preflight & answer pattern (PRD-025)**:
  - `src/docs/preflight.rs:360-409` has `second_opinion(&Review, &mut Preflight)` (interpolating `cfg.agent` into `run \`{} login\`` and hardcoding `"one Claude agent"`) and `ci_mode(&config::Loaded, Option<CiMode>, &mut Preflight)` (warns on invalid env, pushes `pf.ask` when `None`, pushes `pf.ok` when resolved).
  - `src/cli/run.rs:110-113` calls `preflight::second_opinion` and `preflight::ci_mode`; `src/docs/preflight.rs:413-430` calls both in `preflight::cli` and recomputes exit code (`1` refuse, `2` ask, `0` ok).
  - `src/cli/answer.rs:300-304, :366-396` dispatches on `AskOpened.text.starts_with(preflight::CI_MODE_ASK_PREFIX)`, saves to `config.toml`, updates `RunState`, and rejects invalid answers with exit `1` leaving the ask open.
  - `src/run.rs:94-130` holds `RunState` (`ci_mode: Option<CiMode>` at `:129`).
- **Existing tests**: `tests/preflight.rs:55` asserts exact line `"second opinion: cursor-agent with grok-4.7-high ready"` (kept green via `FACTORY_REVIEW_PROVIDER=cursor` from slice 01). `tests/review.rs` drives stub `cursor-agent` and `tests/review_stop.rs` / `tests/review_progress.rs` construct `config::Review`.
- **Deliberate non-changes (PRD-026 §11)**:
  - No cross-vendor same-family warning (cut 2026-11-25, §5 cut).
  - No live API inference ping during `claude`/`codex` probes (verify binary + `--version` within `PROBE_SECS`).

## Tasks

### 1. Dispatch `src/review.rs`, `src/cli/review.rs`, and `src/collect.rs` across all five providers plus `none` — high — blocked by: —
**Source**: §6 FR-4: "Provider-dispatched `src/review.rs`... Explicit opt-out (`ReviewProvider::None`)... For each of the 5 providers (`Provider`), dispatch `probe`, `run_pass`, and `parse_stream_for(provider, text)`".

`src/review.rs`, `src/cli/review.rs`, `src/collect.rs`, `src/closeout.rs`.
- Add `pub fn parse_stream_for(provider: Provider, text: &str) -> Parsed` (keeping `parse_stream(text)` calling `parse_stream_for(Provider::Cursor, text)` so existing unit tests stay green):
  - `Cursor` / `Claude` / `Agy`: parse `type == "result"` (`is_error`, `result`) and `type == "assistant"` text blocks into `Parsed`.
  - `Codex`: parse `codex exec --json` JSONL (`item.completed` with `item.type == "agent_message"` text into `result`/`partial`, `error` / `turn.failed` into `error`).
  - `Opencode`: parse `opencode run --format json` JSONL (`type == "text"` reading `part.text` into `partial`/`result`, `type == "error"` reading `error.message` or `data.message` into `error`).
- Add provider-aware `probe_resolved(cfg: &ResolvedReview, dir: &Path, cancel: Cancel) -> std::result::Result<(), String>` and `run_pass_resolved(cfg: &ResolvedReview, cwd: &Path, dir: &Path, n: usize, prompt: &str, cancel: Cancel) -> Result<Outcome>` (keeping `probe(&Review, ...)` and `run_pass(&Review, ...)` delegating through `ResolvedReview` so `tests/review_stop.rs`, `tests/review_progress.rs`, and `src/closeout.rs` stay compatible):
  - Resolve binary: explicit `cfg.binary` (legacy `[review] agent` override) → else `provider::resolve_binary(p)` → else `p.binaries()[0]`.
  - **`Cursor`**: `-p --trust --mode ask --output-format stream-json --stream-partial-output --model <model> <prompt>`; probe `--list-models` + `lists_model` + auth check (byte-for-byte existing behavior).
  - **`Claude`**: `-p --output-format stream-json --verbose --permission-mode plan --tools Read,Grep,Glob --model <model> [--effort <effort>] <prompt>`, scrubbing `worker::scrubbed` env vars so running from inside Claude Code never triggers the nested-session guard; probe `--version`.
  - **`Codex`**: `exec --json --sandbox read-only --ephemeral -m <model> [-c model_reasoning_effort=<effort>] -`, passing `prompt` via a temp file connected to `Stdio::from(File)` on stdin (avoiding `ARG_MAX` and pipe blocking); probe `--version`.
  - **`Agy`**: `-p --mode plan --output-format stream-json --model <model> [--effort <effort>] <prompt>`; probe `models` + `lists_model`.
  - **`Opencode`**: `run --format json --agent plan -m <model> [--variant <effort>] <prompt>`; probe `models` + `lists_model`.
- Handle `ReviewProvider::None` consistently:
  - In `src/cli/review.rs` (`second`): resolve via `config::resolve_review(&loaded)`; if `Some(r)` with `r.provider == ReviewProvider::None`, print `disabled: second opinion disabled in config.toml` to `out` and return `Ok(2)` without creating a log dir or spawning any process. If `None` (unset), print `unreviewed: adversarial reviewer not set in config.toml — run factory setup review` and return `Ok(2)`.
  - In `src/collect.rs` (`collect`): accept resolved review (or check `ReviewProvider::None`); when `ReviewProvider::None`, emit `Progress::Started { passes: 0, dir: None }` and record `second opinion: skipped (disabled in config)` with section text `skipped: second opinion disabled in config.toml\n` (which `collect::reusable` returns `true` on!).
  - In `src/closeout.rs`: pass effective review (honoring `run.state().review_override` when set, e.g. `ReviewProvider::None` from a `proceed` answer) into `collect::collect`.

**The trap**: `run_logged` (`src/review.rs:520-567`) currently sets `.stdin(Stdio::null())` unconditionally. For `Codex` (`codex exec ... -`), `run_logged` needs an optional `stdin_file: Option<&Path>` so Codex reads its prompt from `review-{n}.prompt` cleanly without ever deadlocking on a pipe! Also check `tests/review.rs` — any test that writes `[review]\ntimeout_secs = 20\n` without `provider` still resolves to `Cursor` via `FACTORY_REVIEW_PROVIDER=cursor` in `tests/harness/mod.rs`!

**Acceptance:** Integration test `review_second_runs_all_five_providers_and_none_mode` in `tests/review.rs` drives stub binaries for all five providers (`claude`, `codex`, `cursor-agent`, `agy`, `opencode`) over both `--diff main` and `--doc`, asserting exact provider flags, stdin prompt delivery for Codex, `CLAUDECODE` scrubbing for Claude, parsed findings + exit `0`, and asserting `[review] provider = "none"` prints `disabled: second opinion disabled in config.toml`, exits `2` without creating a log dir, and makes `collect` write `second opinion: skipped (disabled in config)` with `reusable == true` (fails if any provider adapter or `none` short-circuit is missing).

### 2. Wire preflight review ask-if-missing, `RunState.review_override`, and `factory answer` persistence — high — blocked by: 1
**Source**: §2 Blast radius, §6 FR-4: "Preflight ask-if-missing (`src/docs/preflight.rs:360-379` and `src/cli/answer.rs:301-304`)... `REVIEW_ASK_PREFIX`... `RunState.review_override`".

`src/run.rs`, `src/docs/preflight.rs`, `src/cli/run.rs`, `src/cli/answer.rs`, `tests/preflight.rs`.
- In `src/run.rs`: add `#[serde(default, skip_serializing_if = "Option::is_none")] pub review_override: Option<ReviewProvider>` on `RunState` and `Run::set_review_override(&mut self, v: Option<ReviewProvider>) -> Result<()>`.
- In `src/docs/preflight.rs`:
  - Add `pub const REVIEW_ASK_PREFIX: &str = "adversarial reviewer not set in config.toml";`.
  - Update `second_opinion(loaded: &config::Loaded, pf: &mut Preflight)`:
    - Call `config::resolve_review(loaded)`. Push `warning` (if any, e.g. invalid `FACTORY_REVIEW_PROVIDER`) to `pf.judge`.
    - When `None`: call `provider::detect_installed()` and push `pf.ask` starting with `REVIEW_ASK_PREFIX`, listing detected installed CLIs (or all 5 names if none detected), `none` (disable cross-vendor review), and `proceed` (skip for this run only without writing `config.toml`).
    - When `ReviewProvider::None`: push `pf.ok` `"second opinion: disabled (<source>)"` (`user` or `env`).
    - When `ReviewProvider::Provider(p)`: run `review::probe_resolved`; on `Ok(())`, push `pf.ok` `"second opinion: <bin_name> with <model> ready"` (keeping `tests/preflight.rs:55` exact: `"second opinion: cursor-agent with grok-4.7-high ready"`!); on `Err(reason)`, push `pf.judge` `"second opinion not ready: <reason> — run `<login_hint>` or `factory setup review`; polish falls back to a fresh local subagent per <pass_lines>-line pass"`.
  - Update `preflight::cli` (`:415`) and `src/cli/run.rs` (`:110`) to pass `&loaded` into `preflight::second_opinion`.
- In `src/cli/answer.rs` (`answer_ask`, right next to `ci_mode_answer` at `:301`):
  - When `item.starts_with(preflight::REVIEW_ASK_PREFIX)` → call `review_answer(&mut run, id, text, out)?`:
    - `"proceed"` (case-insensitive) → do not edit `config.toml`, call `run.set_review_override(Some(ReviewProvider::None))?`, print that second opinion is skipped for this run only.
    - `"none"` (case-insensitive) → `config::patch(&paths::config_dir(), &[Patch::set("review", "provider", "none"), Patch::remove("review", "agent")])?`, call `run.set_review_override(Some(ReviewProvider::None))?`, print saved path.
    - `"<provider>"` | `"<provider>:<model>"` | `"<provider>:<model>:<effort>"` where `<provider>` parses as `Provider`: resolve `model` (explicit or `p.default_model()`), `effort` (explicit if provider supports effort, else `p.default_effort()`), write via `config::patch` (clearing legacy `agent`), call `run.set_review_override(None)?`, print saved path.
    - Anything else → print valid answers naming the 5 providers, `none`, and `proceed`, write nothing to `config.toml`/`log.jsonl`/`events.jsonl`/`answers/`, return `Ok(false)` (leaving the ask open and exiting `1`).

**The trap**: Check `tests/preflight.rs:55-160` carefully! Look at `logged_out` tests in `tests/preflight.rs` — they may assert substring `"cursor-agent login"` in the judge line! Notice `"run \`cursor-agent login\` or \`factory setup review\`"` STILL contains `"cursor-agent login"`! Verify every existing assertion in `tests/preflight.rs` stays green.

**Acceptance:** Integration test `preflight_asks_when_review_unset_and_answer_persists_or_proceeds` in `tests/preflight.rs` (with `FACTORY_REVIEW_PROVIDER` removed) proves:
- `factory docs preflight 001` exits `2` with `REVIEW_ASK_PREFIX` under `!`; an invalid `FACTORY_REVIEW_PROVIDER=bogus` emits a `?` warning AND exits `2` with the ask.
- `factory run 001` parks in `WaitingOwner` with `ASK-1`.
- `factory answer ASK-1 "bogus"` exits `1`, writes nothing to `config.toml` or `log.jsonl`, and leaves `ASK-1` open.
- `factory answer ASK-1 "codex:gpt-5.4:high"` writes `[review]` (`provider = "codex"`, `model = "gpt-5.4"`, `effort = "high"`) to `config.toml`, wakes the run, and a subsequent preflight exits `0` with `✓ second opinion: codex with gpt-5.4 ready`.
- `factory answer ASK-1 "none"` writes `provider = "none"` and subsequent preflight prints `✓ second opinion: disabled (user)`.
- `factory answer ASK-1 "proceed"` leaves `config.toml` untouched, writes `review_override = "none"` on `state.json`, and wakes the run.
- Fails if preflight ask, answer validation, `config::patch`, or `RunState.review_override` is removed.

### 3. Verify — medium — blocked by: 1, 2
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q`

**Acceptance:** inner loop green, one commit.

### 4. Prove — at the checkpoint — blocked by: 3
Drive this row from [`prd.md`](prd.md) §9 QA plan at CP-2 (and re-prove FR-1, FR-2, FR-3, FR-5 at the final checkpoint):
- **FR-4**: run `cargo test review_second_runs_all_five_providers_and_none_mode` and `cargo test preflight_asks_when_review_unset_and_answer_persists_or_proceeds`.

**Acceptance:** FR-4 row (and every prior row re-proved at final CP-2) has passing test transcripts captured in checkpoint evidence and recorded in **Progress**.

**Code-review fixes (2026-11-25).**

- The Claude, agy and OpenCode reviewers could not read the diff, which is written to the review log dir outside the repo. Claude and agy now get `--add-dir <log dir>`, and OpenCode gets the diff attached with `-f`. OpenCode's effort moved from `--variant` (absent in v2.4.1) to `-m <model>#<effort>`, plus `--standalone`, so a timeout kills the agent itself. The Claude `--version` probe is scrubbed of session variables like the pass is (AD-11; `review_second_runs_all_five_providers_and_none_mode` asserts the argv and the scrub on both calls).
- `factory answer ASK-n "codex"` saved `effort = "high"`, against AD-6. It now leaves effort unset, keeps `:` inside model ids, and refuses `-`-led ids (AD-12; `a_reviewer_answer_leaves_effort_unset_and_keeps_colons_in_the_model`).
- The ask told the owner to answer a bare `opencode`, which is always refused. It now lists `opencode:<model>`.
- `review second` exited 2 after every pass failed, with no `unreviewed:` line, so the skills could not tell that case from `disabled:`. It now prints `unreviewed: no pass returned a review` before `log:`.
- Closeout fell back to the driver's startup `[review]` when a fresh load chose nobody. It now uses the fresh file's legacy defaults, as `review collect` does (AD-8).
- The login hint ignored the configured or resolved binary (`run claude login` for `/opt/c/claude`, `cursor-agent login` when only `agent` is on PATH). It now names the binary the probe runs (`the_login_hint_names_the_binary_the_probe_runs`).
