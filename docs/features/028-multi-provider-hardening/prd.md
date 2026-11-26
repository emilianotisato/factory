---
number: 028
slug: multi-provider-hardening
title: "Multi-provider hardening and security"
type: feature
created: 2026-11-26
author: Emiliano Tisato
authorized_tests: ["tests/review.rs::review_second_runs_all_five_providers_and_none_mode", "tests/worker.rs::a_worker_gets_no_github_and_no_session_but_keeps_the_account", "tests/worker.rs::a_worker_pushes_nowhere_through_the_git_push_off_env", "tests/setup.rs::setup_workers_configures_mixed_roles", "mods/cockpit/tests/cockpit.test.tsx"]
summary: |
  Closes the multi-provider footguns and security gaps left after PRD-026/027: clears stale model/effort from provider-less `[workers.<role>]` tables when `[workers] provider` switches and validates every resolved model/effort in preflight (plus fixes wizard default/reload, `role_sources`, and `setup --show` alignment); isolates all five adversarial reviewers (and probes) with env scrubbing, `--` prompt guards, Claude `--setting-sources user --strict-mcp-config`, OpenCode read-only permission env, and `factory review second --dry-run`; closes `dec-2` PATH-shim gaps (`GITHUB_PERSONAL_ACCESS_TOKEN`, `help.autocorrect=0`, full-URL `pushInsteadOf`, and explicit `remote.*.pushurl` `insteadOf` rewrites); fixes stale `collect.md` reuse across mid-run reviewer switches, `proceed` skip wording, preflight config warnings, PATH-order `detect_installed`, `X_OK` checks, and adds `--title-file` on `factory docs log add`.
---

# PRD-028 — Multi-provider hardening and security

> **Interviewed 2026-11-26** (absorbing `2026-11-25-provider-switches-leave-claude-slugs-on-provider-l.md`, `2026-11-25-isolate-the-adversarial-reviewer-from-repo-and-use.md`, `2026-11-25-close-the-dec-2-gaps-the-worker-path-shim-cannot-s.md`, `2026-11-25-reviewer-status-wording-and-collect-reuse-after-a.md`, and the `--dry-run` / `--title-file` halves of `2026-11-26-providers-live-codex-opencode-jsonl-recordings-rev.md`). Every `file:line` and live CLI flag below was verified against `main` at `970546b`. Revised 2026-11-26 after cross-vendor adversarial pass (`claude:opus`, log `20261126-213654`).

## 1. Why now

Now that coworkers are adopting `factory` across multiple providers (`claude`, `codex`, `cursor`, `agy`, `opencode`), five real multi-provider footguns and security gaps from PRD-026/027 polish need closing:

- **Switching `[workers] provider` leaves stale Claude slugs on provider-less role tables.** Today `resolve_role` (`src/config.rs:821-851`) inherits `[workers] provider` for a role table like `[workers.build] model = "sonnet"` (which has no `provider`), so switching `[workers] provider = "codex"` via `factory setup workers` or `factory answer ASK-n codex` spawns `codex -m sonnet` and crashes. Neither `config::load` nor `preflight::worker_refusals` (`src/docs/preflight.rs:487-507`) validates model ids (`is_model_name`) or provider effort levels; `workers_step` (`src/cli/setup.rs:875-966`) brackets the first CLI on PATH rather than the effective provider and rereads the pre-wizard config on repeated role edits; `config::role_sources` (`src/config.rs:520-565`) omits `FACTORY_WORKER_PROVIDER` when a `--worker` spec is present; and `setup::show` (`src/cli/setup.rs:655`) runs `  checkpointcursor` together (`{role:<10}` leaves zero spaces).
- **Adversarial reviewers are unevenly isolated from the branch under review.** In `src/review.rs:692-696` and `:901-1005`, only `Provider::Claude` gets `scrub_claude_session` and `--` before `prompt`. None of the five reviewers (or probes) scrubs `GH_TOKEN_VARS`; Claude still loads project `.claude/settings.json` hooks and user MCP servers (`--tools Read,Grep,Glob` does not skip MCPs); OpenCode lets a branch's `opencode.json` or `.opencode/agent/plan.md` redefine the `plan` agent's permissions; `--doc --prompt "-foo"` fails clap parsing or parses as a flag on Cursor/Agy/OpenCode; and there is no `factory review second --dry-run` to inspect exact argv without invoking the CLI.
- **`dec-2` PATH-shim backstops have three reachable bypasses.** `worker::GH_TOKEN_VARS` (`src/worker.rs:26-31`) omits `GITHUB_PERSONAL_ACCESS_TOKEN` (used by GitHub MCP servers); `worker::push_off_env` (`:72-86`) does not set `help.autocorrect=0`, its short scheme `pushInsteadOf` prefixes lose git's longest-match rule to a user's `url."git@github.com:".pushInsteadOf = "https://github.com/"`, and `pushInsteadOf` is ignored by git for remotes with an explicit `remote.*.pushurl` (so git's own `/usr/lib/git-core` children like `git subtree push`, `git rebase -x`, or `git lfs push` bypass `run_dir/bin/git`).
- **Stale `collect.md` reuse, wrong `proceed` wording, hidden preflight warnings, and detection papercuts.** `collect::reusable` (`src/collect.rs:34-39`, called from `src/closeout.rs:411`) checks only `HEAD`, so switching `[review]` mid-run reuses the previous vendor's collect file. A `proceed` skip prints `"skipped (disabled in config)"` (`src/collect.rs:24, :122`). `preflight::cli` (`src/docs/preflight.rs:557-570`) drops `loaded.warnings`. `provider::detect_installed_in` (`src/provider.rs:252-259`) orders by `Provider::ALL` instead of `PATH` order, and `is_executable` (`:248-250`) checks any `0o111` bit rather than caller execute permission (`X_OK`).
- **`factory docs log add` has no `--title-file`.** Titles containing backtick examples were mangled by shell substitution during PRD-027 (`AD-15` → `AD-16`).

**What this leaves alone.** Capturing live `codex exec --json` and `opencode run --format json` JSONL stream recordings (requires interactive logged-in CLIs; stays in `docs/ideas/2026-11-26-providers-live-codex-opencode-jsonl-recordings-rev.md`), interactive `.bashrc` PATH reordering (outside `factory`'s spawn env), and `config::patch` TOML edge cases (dropped during harvest).

## 2. Blast radius

- **`tests/review.rs::review_second_runs_all_five_providers_and_none_mode` (authorized).** FR-2 adds `--setting-sources user --strict-mcp-config` to Claude's reviewer argv, `--` before `prompt` on Cursor, Agy, and OpenCode, and scrubs `CLAUDECODE` and `GH_TOKEN_VARS` across all five providers (both `run_pass_resolved` and `probe_resolved`). This test asserts exact argv arrays (`tests/review.rs:1285-1365`) and that non-Claude calls keep `CLAUDECODE` (`:1427`, covering both probe and pass calls); both assertions are updated.
- **`tests/worker.rs::a_worker_gets_no_github_and_no_session_but_keeps_the_account` and `tests/worker.rs::a_worker_pushes_nowhere_through_the_git_push_off_env` (authorized), plus `src/worker.rs:824`.** FR-3 adds `help.autocorrect=0` to `push_off_env`, increasing its base entry count from 5 to 6 (so `tests/worker.rs:134` expects `"6"`, `tests/worker.rs:202-218` expects `"7"` with `GIT_CONFIG_KEY_6 = "help.autocorrect"` / `VALUE_6 = "0"`, and unit test `src/worker.rs:824` `the_push_off_env_goes_after_the_inherited_git_config` expects 6 entries).
- **`tests/setup.rs::setup_workers_configures_mixed_roles` (authorized).** FR-1 widens only role lines in `setup::show` (`src/cli/setup.rs:655`) from `  {role:<10}` to `  {role:<12}` so `checkpoint` has two spaces before its provider. Top-level labels (`review      `, width 12) remain unchanged so `tests/setup.rs:251` stays green, while `:462` updates from `"  fix       cursor grok-4.7-high ("` to `"  fix         cursor grok-4.7-high ("`.
- **`collect::reusable` backwards compatibility and `src/closeout.rs:2819`.** `tests/review.rs:1477` and `src/collect.rs:308-328` call `collect::reusable(&text, head)` (2 args). Keep `pub fn reusable(text: &str, head: &str) -> bool` intact and add `pub fn reusable_for(text: &str, head: &str, review: &ResolvedReview) -> bool` for `src/closeout.rs:411`. Update unit test `src/closeout.rs:2819` (`a_file_at_the_same_head_is_reused_unless_it_owes_a_rerun`) to include line 2 `<!-- reviewer: <tag> -->` in its reusable fixture file.
- **Worker `push_off_env` signature.** Keep `worker::push_off_env(inherited: Option<&str>)` (`src/worker.rs:72`) with its existing signature (now 6 entries with `help.autocorrect=0`) and add `push_off_env_for_repo(repo: &Path, inherited: Option<&str>)` wired to `cwd` in `Worker::spawn` (`src/worker.rs:462`).
- **Skills & version bump (FR-4 `proceed` wording, authorized).** `skills/polish/SKILL.md:65` currently lists only `skipped (disabled in config)` as "no fallback is owed". Updating `skills/polish/SKILL.md:65` to also recognize `skipped (proceed for this run)` bumps the crate/bundle/cockpit version from `0.17.0` → `0.18.0` across `Cargo.toml`, `Cargo.lock`, `skills/bundle.lock`, `mods/cockpit/.claude-plugin/plugin.json`, `mods/cockpit/hooks/register.tsx`, and `mods/cockpit/tests/cockpit.test.tsx` (declared in `authorized_tests`).
- **External contracts.** `factory setup --show` adds two spaces on role lines (`{role:<12}`); `--json` output schema is unchanged. `factory review second --dry-run` and `factory docs log add --title-file` are purely additive flags.

## 3. FR-1 — Safe provider switches, config validation, and `factory setup` fixes

**Today.** `cli::setup::worker_edits` (`src/cli/setup.rs:399-434`) and `cli::answer::worker_answer` (`src/cli/answer.rs:474-570`) only clear `model`/`effort` on the `[workers]` table itself when `[workers] provider` changes, leaving provider-less `[workers.<role>]` tables (e.g. `[workers.build] model = "sonnet"`) carrying the old provider's slugs. `preflight::worker_refusals` (`src/docs/preflight.rs:487-507`) checks binary presence and empty model, but does not check `provider::is_model_name(&w.model)` or `provider.effort_levels()` for invalid efforts loaded from `config.toml`. `workers_step` (`src/cli/setup.rs:875-966`) brackets `detected.first()` over the effective provider (`FACTORY_WORKER_PROVIDER` or legacy Claude) and re-reads the pre-wizard `loaded` struct if the user edits the same role twice in one wizard session. `config::role_sources` (`src/config.rs:520-565`) omits `FACTORY_WORKER_PROVIDER` when computing `own` under a `--worker` spec (`:540`), mislabeling a role's model source as `default` instead of `user`. And `setup::show` (`src/cli/setup.rs:655`) formats `  {role:<10}` with zero spaces after `"checkpoint"` (`  checkpointcursor`).

**Decided 2026-11-26.**

- **Clear stale model/effort on provider-less role tables on `[workers] provider` switch (`src/config.rs`, `src/cli/setup.rs`, `src/cli/answer.rs`):**
  - Add a shared helper `config::stale_role_clears(loaded: &Loaded, env_provider: Option<Provider>, new_provider: Provider) -> Vec<Patch>`: compute the previous effective `[workers]` provider as `w.provider.or(env_provider).unwrap_or(Provider::Claude)`. When `new_provider != prev_provider`, inspect every role in `ROLE_KEYS` (`build`, `fix`, `checkpoint`, `polish`). For any role table whose own `provider` is `None` (`worker.provider.is_none()`), emit `Patch { table: "workers.<role>", key: "model" | "effort", value: None }` for each explicit `model` or `effort` it holds (so `save()` / `answer` prints every cleared key transparently).
  - Call `stale_role_clears` from BOTH `cli::setup::worker_flags` / `workers_step` (when switching default `[workers]` provider) AND `cli::answer::worker_answer` (`src/cli/answer.rs:517-558`). In `worker_answer`, also clear those same provider-less role `model`/`effort` fields on `planned.config.workers` in-memory **before** `config::resolve_workers(&planned, &[])` so the current run resolves cleanly to the new provider's defaults instead of spawning `codex -m sonnet`, and include the returned clear patches in `config::patch(...)`.
- **Validate resolved model & effort in `preflight::worker_refusals` (`src/docs/preflight.rs:487-507`) and warn on bad `[review]` (`src/docs/preflight.rs`):**
  - In `preflight::worker_refusals`: for every `(role, w)` in `resolved.roles()`:
    - If `!w.model.is_empty()` and `!provider::is_model_name(&w.model)`, push a clear refusal (`"{role} worker model {:?} is not a valid model id — set one with `factory setup workers --role {role} --model <m>`"`).
    - If `let Some(e) = &w.effort`: if `w.provider.effort_levels().is_empty()` or `!w.provider.effort_levels().contains(&e.as_str())`, push a clear refusal (`"{role} worker effort {e:?} is not valid for {}"`).
  - In `preflight::second_opinion` (for `ReviewProvider::Provider(p)`): if `!r.model.is_empty()` and `!provider::is_model_name(&r.model)`, or if `let Some(e) = &r.effort` is not in `p.effort_levels()`, push a warning on `pf.judge` (never a refusal, so a hand-edited `[review]` degrades to local-review fallback instead of blocking a run).
- **Fix `workers_step`, `role_sources`, and `setup --show` spacing (`src/cli/setup.rs`, `src/config.rs`):**
  - In `workers_step` (`src/cli/setup.rs:875-966`): bracket the effective default provider (`now.own_provider.or(env_provider).or_else(|| (now.model.is_some() || now.effort.is_some()).then_some(Provider::Claude)).or_else(|| detected.first().map(|d| d.provider))`), and when editing roles inside the wizard loop, maintain a working copy of `Loaded` (or apply pending patches onto it / replace prior patches for the same `(table, key)`) so editing the same role twice in one session never overwrites the first pass with stale pre-wizard state.
  - In `config::role_sources` (`src/config.rs:520-565`): pass/read `env_provider` (matching `resolve_role` at `:829-831`) so `let own = spec.is_none() || worker.provider.or(w.provider).or(env_provider) == Some(r.provider);` keeps a role's model labeled `user` when `FACTORY_WORKER_PROVIDER` matches the role's provider even alongside a `--worker` spec.
  - In `setup::show` (`src/cli/setup.rs:655`): widen role line formatting from `  {role:<10}{}` to `  {role:<12}{}` while leaving top-level labels (`review      `, width 12) untouched so `checkpoint` always has two spaces before its provider without breaking top-level alignment.

**Accepted cost.** Clearing model/effort on provider-less `[workers.<role>]` tables when `[workers] provider` switches discards old role overrides automatically rather than leaving them behind; `factory setup` prints every removed key so the user sees it immediately and can set a new per-role override if wanted.

**Acceptance.** Integration test `provider_switch_clears_stale_role_slugs_and_preflight_refuses_bad_config` (plus a unit test in `src/config.rs` for `role_sources`) proves: (1) with `[workers.build] model = "sonnet"` (no `provider`), running `factory setup workers --provider codex` clears `[workers.build] model`, prints the removal, and resolves `build` to `codex`'s default model — and `factory answer ASK-1 codex` clears it both on disk and in the run's resolved `worker_config` (while setting `FACTORY_WORKER_PROVIDER=codex` with `[workers.build] model = "gpt-5.4"` and running `factory setup workers --provider codex` preserves `gpt-5.4`); (2) a hand-edited `config.toml` with `[workers.build] model = "--bad"` or `[workers.build] provider = "codex"\neffort = "max"` is refused by `preflight::worker_refusals`, and a hand-edited `[review] model = "--bad"` emits a `pf.judge` warning without refusing; (3) `factory setup --show` separates `checkpoint` from its provider by two spaces (`  checkpoint  cursor ...`), and `role_sources` (verified via `runs.knobs` or `src/config.rs`) labels a role's model `user` when `FACTORY_WORKER_PROVIDER=claude` matches a Claude role table under `--worker claude`.

## 4. FR-2 — Isolate all five adversarial reviewers and add `factory review second --dry-run`

**Today.** `review::probe_resolved` (`src/review.rs:692-696`) and `review::run_pass_resolved` (`src/review.rs:901-1005`) only run `scrub_claude_session(&mut cmd)` on `Provider::Claude`, and only pass `--` before `prompt` on `Provider::Claude`. Claude still loads project `.claude/settings.json` and user MCP servers; OpenCode can load a branch-committed `opencode.json` or `.opencode/agent/plan.md` overriding `plan` agent permissions; none of the five reviewers or probes strips `GH_TOKEN_VARS`; `SecondArgs.prompt` (`src/cli/review.rs:34-35`) rejects `-`-led prompts at clap parse time; and `factory review second` has no `--dry-run` flag to print the exact argv/env without running the CLI.

**Decided 2026-11-26.**

- **Build pass commands in one shared helper `pub fn build_pass_command(cfg: &ResolvedReview, cwd: &Path, dir: &Path, n: usize, prompt: &str, dry_run: bool) -> Result<PassCommand>` (`src/review.rs`):**
  - Used by BOTH `run_pass_resolved` (`dry_run = false`) and `--dry-run` (`dry_run = true`) so `--dry-run` always reflects the exact live argv and env while **writing no files on disk** when `dry_run = true` (skipping `review-n.prompt` file creation for Codex and emitting `-f <diff>` for OpenCode diff passes without requiring `review-n.diff` to exist on disk).
  - **All five providers (in BOTH `build_pass_command` and `probe_resolved`):**
    - Remove every var in `crate::worker::GH_TOKEN_VARS` (which includes `GITHUB_PERSONAL_ACCESS_TOKEN` under FR-3) AND scrub Claude session vars (`scrub_claude_session(&mut cmd)`).
  - **Prompt option-end guard (`build_pass_command`):**
    - Pass `--` before `prompt` on `Claude`, `Cursor`, `Agy`, and `Opencode` (`Codex` already reads the prompt via stdin `-`).
    - Add `allow_hyphen_values = true` on `SecondArgs.prompt` (`src/cli/review.rs:34-35`) so `factory review second --doc <file> --prompt "-check this"` passes through clap cleanly.
  - **`Provider::Claude` (verified live on `claude --help`):**
    - Add `"--setting-sources", "user"` (ignores project `.claude/settings.json` hooks in the branch under review) and `"--strict-mcp-config"` (with no `--mcp-config`, skips all user/project MCP servers).
  - **`Provider::Opencode`:**
    - Set `OPENCODE_CONFIG_CONTENT` env var to `r#"{"permission":{"edit":"deny","bash":"deny","webfetch":"deny"},"agent":{"plan":{"permission":{"edit":"deny","bash":"deny","webfetch":"deny"}}}}"#` so neither a branch's `opencode.json` nor `.opencode/agent/plan.md` can grant edit, bash, or webfetch permission to the read-only reviewer.
- **`factory review second --dry-run` (`src/cli/review.rs:25-45, :118-210`):**
  - Add `#[arg(long)] pub dry_run: bool` to `SecondArgs`.
  - When `--dry-run` is set (valid with either `--diff <base>` or `--doc <file> --prompt <text>`):
    - Skip `probe_resolved`, log-dir file creation, and `run_passes` (never executes the provider CLI, writes review files, or touches the network!).
    - Print the resolved provider/model/effort, binary, pass count, exact shell-quoted command line (`argv`), and explicit reviewer env overrides/removals for pass 1 to stdout, and exit `0`.

**Accepted cost.** Passing `--setting-sources user --strict-mcp-config` on Claude reviewers also drops project `CLAUDE.md`, project skills/agents, project `env`, and `apiKeyHelper`, and disables all MCP servers during adversarial review (intentional — reviewers only need `Read,Grep,Glob` on the checked-out diff and doc). Cursor `--trust` (`.cursor/mcp.json`), OpenCode plugins/local MCP, and Codex `~/.codex` user MCP config have no CLI flag to disable today and remain noted as accepted boundaries of those CLIs.

**Acceptance.** Integration test `review_second_isolates_all_five_providers_and_supports_dry_run` (plus updated `tests/review.rs::review_second_runs_all_five_providers_and_none_mode`) proves: (1) Claude reviewer argv includes `--setting-sources user --strict-mcp-config` and `--` before prompt; (2) Cursor, Agy, and OpenCode pass `--` before a `-`-leading prompt (and `factory review second --doc <file> --prompt "-check this"` parses through clap and reaches the prompt positional arg, not a flag); (3) OpenCode receives `OPENCODE_CONFIG_CONTENT` denying `edit`, `bash`, and `webfetch` both at top level and under `agent.plan`; (4) all 5 providers have `GH_TOKEN_VARS` (including `GITHUB_PERSONAL_ACCESS_TOKEN`) and `CLAUDECODE` scrubbed from their environment in both `probe_resolved` and `run_pass_resolved`; (5) `factory review second --doc <file> --prompt "hi" --dry-run` exits `0`, prints the exact provider argv and env rules, creates no review log files, and never executes the provider stub binary.

## 5. FR-3 — Close `dec-2` PATH-shim gaps (`GITHUB_PERSONAL_ACCESS_TOKEN`, `help.autocorrect=0`, full-URL `pushInsteadOf`, and explicit `remote.*.pushurl`)

**Today.** `worker::GH_TOKEN_VARS` (`src/worker.rs:26-31`) lists 4 env vars (`GH_TOKEN`, `GITHUB_TOKEN`, `GH_ENTERPRISE_TOKEN`, `GITHUB_ENTERPRISE_TOKEN`) and omits `GITHUB_PERSONAL_ACCESS_TOKEN`. `worker::push_off_env` (`:72-86`) injects 5 short scheme `url.factory-no-push://.pushInsteadOf` entries, which (a) can be overridden by a longer user `url."git@github.com:".pushInsteadOf = "https://github.com/"` rule under git's longest-match precedence, (b) are ignored by git for remotes with an explicit `remote.*.pushurl` (reachable via git's own `/usr/lib/git-core/git` helpers like `git subtree push` or `git rebase -x`), and (c) do not set `help.autocorrect=0`.

**Decided 2026-11-26.**

- **Add `GITHUB_PERSONAL_ACCESS_TOKEN` to `GH_TOKEN_VARS` (`src/worker.rs:26-31`):** expand `GH_TOKEN_VARS` from 4 to 5 entries.
- **Set `help.autocorrect=0` in `push_off_env` (`src/worker.rs:72-86`):** append `("help.autocorrect", "0")` alongside the 5 `pushInsteadOf` entries (6 entries total) so a repo/user `help.autocorrect=immediate` cannot turn a typo into a git push.
- **Block explicit `remote.*.pushurl` and longest-match user `pushInsteadOf` in `push_off_env_for_repo` (`src/worker.rs`):**
  - Add `pub fn push_off_env_for_repo(repo: &Path, inherited: Option<&str>) -> Vec<(String, String)>`:
    - Starts from `push_off_env(inherited)` (the 5 scheme `pushInsteadOf` prefixes + `help.autocorrect=0`).
    - Reads configured `remote.*.url` values in `repo` via `git config --get-regexp '^remote\..*\.url$'` (ignoring exit 1 when none exist), deduplicating values, and appends `url.factory-no-push://.pushInsteadOf = <url>` for each distinct remote URL (so exact-length match beats any shorter user `pushInsteadOf` prefix!).
    - Reads configured `remote.*.pushurl` values in `repo` via `git config --get-regexp '^remote\..*\.pushurl$'` (ignoring exit 1 when none exist), deduplicating values, and appends `url.factory-no-push://.insteadOf = <pushurl>` for each distinct pushurl.
    - Updates `GIT_CONFIG_COUNT` accordingly.
  - Wire `Worker::spawn` (`src/worker.rs:462`) to call `push_off_env_for_repo(&cwd, inherited.as_deref())` (reading the lane worktree `cwd`, NOT `wdir`!) instead of `push_off_env(...)`.

**Accepted cost.** (1) If a repo configures an explicit `remote.origin.pushurl` equal to its fetch `url`, rewriting it with `insteadOf` also blocks `git fetch` inside worker lanes — acceptable because lanes are local detached worktrees prepared by the driver and workers never need remote network fetches. (2) A `remote.*.pushurl` added by a worker *after* spawn is not in the spawn-time env snapshot; that path remains covered by the `run_dir/bin/git` PATH shim (`tests/worker.rs:592-609`).

**Acceptance.** Integration test `worker_env_strips_pat_disables_autocorrect_and_blocks_explicit_pushurl` in `tests/worker.rs` proves via a real `factory run --foreground` spawn (`Worker::spawn`): (1) the spawned worker has `GITHUB_PERSONAL_ACCESS_TOKEN` stripped from its env; (2) `git config --get help.autocorrect` under the worker's `git_config` env returns `"0"`; (3) in a fixture repo with `git remote set-url --push origin <bare-remote>` (and a user `pushInsteadOf` prefix rule on a second remote), invoking the real `git` binary directly (resolved outside shims) with the spawned worker's `git_config` env fails to push because `git remote get-url --push` rewrites to `factory-no-push://…`.

## 6. FR-4 — Fix stale `collect` reuse, `proceed` skip wording, preflight warnings, PATH-order detection, `X_OK`, and `docs log add --title-file`

**Today.**
- `collect::collect` (`src/collect.rs:28-39, :178-181`) writes `<!-- head: <sha> -->` with no reviewer stamp, so `closeout::collected` (`src/closeout.rs:411`) reuses a collect file at the same `HEAD` even after `[review]` changed mid-run.
- `collect.rs:24, :122` prints `second opinion: skipped (disabled in config)` even when `ReviewProvider::None` came from a per-run `proceed` answer (`RunState.review_override`) rather than `config.toml`.
- `preflight::cli` (`src/docs/preflight.rs:557-570`) ignores `loaded.warnings` (hiding typos like `[review] proivder = "claude"`).
- `provider::detect_installed_in` (`src/provider.rs:252-259`) iterates `Provider::ALL` rather than `PATH` directory order, and `is_executable` (`:248-250`) checks `mode & 0o111 != 0` instead of caller execute permission (`libc::access(..., libc::X_OK) == 0`).
- `cli::docs::LogVerb::Add` (`src/cli/docs.rs:160-191, :401-435`) takes `--title <TEXT>` only, with no `--title-file <PATH>` (`-` or file path).

**Decided 2026-11-26.**

- **Reviewer-aware `collect` reuse (`src/collect.rs`, `src/closeout.rs`):**
  - Add `pub fn reviewer_tag(cfg: &ResolvedReview) -> String`: returns `"none"` for `ReviewProvider::None`, else `"{provider}:{model}"` (or `"{provider}:{model}:{effort}"` when `effort` is `Some`).
  - Include `<!-- reviewer: <tag> -->` as line 2 of `collect.md` right under `<!-- head: <sha> -->` (preserving `<!-- head: <sha> -->` as line 1 so existing 2-arg `collect::reusable(text, head)` keeps working unchanged).
  - Add `pub fn reusable_for(text: &str, head: &str, review: &ResolvedReview) -> bool`: requires `reusable(text, head)` AND line 2 equals `<!-- reviewer: {reviewer_tag(review)} -->`.
  - Update `src/closeout.rs:411` (`collected`) to call `collect::reusable_for(&text, &head, review)` so changing `[review]` provider/model/effort mid-run forces a fresh collect at the same `HEAD`.
- **Accurate `proceed` skip wording (`src/collect.rs`, `src/closeout.rs`, `skills/polish/SKILL.md`):**
  - Add `pub skipped_by_proceed: bool` (default `false` in `Options::new`) on `collect::Options`: when `cfg.provider == ReviewProvider::None` and `opts.skipped_by_proceed` is `true` (set in `closeout.rs` when `run.state().review_override == Some(ReviewProvider::None)` and `config.toml` did NOT set `provider = "none"`), emit `second opinion: skipped (proceed for this run)` / `skipped: second opinion skipped for this run (proceed)` instead of `disabled in config`. Keep `skipped (disabled in config)` unchanged when `config.toml` sets `provider = "none"`.
  - Update `skills/polish/SKILL.md:65` to recognize both `skipped (disabled in config)` and `skipped (proceed for this run)` as "no fallback is owed", and bump the version from `0.17.0` → `0.18.0` (`Cargo.toml`, `Cargo.lock`, `skills/bundle.lock` via `factory skills lock` / bundle regen, `mods/cockpit/.claude-plugin/plugin.json`, `mods/cockpit/hooks/register.tsx`, and `mods/cockpit/tests/cockpit.test.tsx`).
- **Surface config warnings in `preflight::cli` (`src/docs/preflight.rs:557-570`):**
  - Push each entry of `loaded.warnings` onto `pf.judge` (e.g. `format!("config.toml: {w}")`) so `factory docs preflight` surfaces typos like `[review] proivder`.
- **PATH-order `detect_installed_in` and `X_OK` check (`src/provider.rs:228-260`):**
  - Change `is_executable(path: &Path) -> bool` on `#[cfg(unix)]` to verify `fs::metadata(path).is_ok_and(|m| m.is_file())` AND `libc::access(cstr, libc::X_OK) == 0` (so a binary with only other-user execute bits is not treated as executable by the caller).
  - Change `detect_installed_in(path_env: &OsStr) -> Vec<Detected>` to scan `PATH` directories in order (recording each provider at the earliest `PATH` directory index where one of its binaries resolves, breaking ties within the same directory by `Provider::ALL` order), returning providers sorted by `(dir_index, provider_rank)` so `.first()` truly returns the first installed CLI on `PATH`.
- **`--title-file` on `factory docs log add` (`src/cli/docs.rs:160-191, :401-435`):**
  - Make `title: Option<String>` with `conflicts_with = "title_file"` and add `#[arg(long, conflicts_with = "title")] title_file: Option<String>` (requiring one of `--title` or `--title-file`, reading trimmed single-line text from file path or `-` when `--body-file` is not also `-`, and refusing if both `--title-file -` and `--body-file -` try to read stdin).

**Accepted cost.** (1) Legacy `collect.md` files written before this change lack line 2 (`<!-- reviewer: ... -->`), so `reusable_for` will treat a pre-upgrade `collect.md` as stale once and regenerate it cleanly. (2) Sorting `detect_installed_in` by `PATH` order changes which CLI `proceed` picks in `src/cli/answer.rs:477` when multiple providers are installed — now honoring the user's `PATH` preference rather than fixed `Provider::ALL` order.

**Acceptance.** Integration tests prove (`collect_reuse_preflight_warnings_path_order_and_log_title_file`): (1) `collect::reusable_for` returns `true` for matching `(head, reviewer)` and `false` when provider, model, or effort changes at the same `HEAD`; (2) `collect` with `skipped_by_proceed = true` prints `skipped (proceed for this run)` rather than `disabled in config`; (3) `factory docs preflight` prints a `?` line when `config.toml` has an unknown key typo; (4) `detect_installed_in` returns a provider found in an earlier `PATH` directory ahead of one found only in a later directory, and rejects a caller-owned file with mode `0o001` (which passes `mode & 0o111 != 0` but fails `access(X_OK)` for non-root; skipped if `geteuid() == 0`); (5) `factory docs log add 001 --kind decision --title-file <path>` preserves backticks verbatim and refuses `--title-file - --body-file -`.

## 7. Decisions log

No `OPEN-n`. Every fork was settled on 2026-11-26 and lives in its FR:

- Clear stale model/effort on provider-less `[workers.<role>]` tables when `[workers] provider` switches (respecting `FACTORY_WORKER_PROVIDER`, updating both disk and `worker_answer` in-memory `planned`), and validate resolved worker model/effort as refusals and `[review]` as warnings in preflight (§3).
- Full isolation for all five adversarial reviewers and probes (env scrub, `--` prompt guard + clap `allow_hyphen_values`, Claude `--setting-sources user --strict-mcp-config`, OpenCode `OPENCODE_CONFIG_CONTENT` covering both top-level and `agent.plan`) plus file-free `factory review second --dry-run` (§4).
- Close all three `dec-2` PATH-shim gaps: `GITHUB_PERSONAL_ACCESS_TOKEN`, `help.autocorrect=0`, full-URL `pushInsteadOf`, and `remote.*.pushurl` `insteadOf` rewrites read from lane `cwd` (§5).
- Add reviewer-stamped `collect::reusable_for`, accurate `proceed` wording (+ `skills/polish/SKILL.md:65` alignment), preflight config warnings, PATH-order `detect_installed_in`, `X_OK` check, `setup --show` alignment, and `docs log add --title-file`; defer live `codex`/`opencode` JSONL recordings (§6, §10).

## 8. QA plan

**Gate**: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test && claude plugin validate mods/cockpit && claude plugin test mods/cockpit` *(declared in `docs/project.toml`)*
**Surface**: `factory setup`, `factory docs preflight`, `factory review second [--dry-run]`, `factory review collect`, `factory docs log add --title-file`, and worker spawn env (`src/worker.rs`). No GUI: every FR is proven below it.
**Harness**: `tests/setup.rs`, `tests/review.rs`, `tests/worker.rs`, `tests/docs_log.rs`, `src/config.rs`, `src/provider.rs`, `src/collect.rs`, `src/closeout.rs`.
**Seed state**: fixture repos with `config.toml` role overrides, stub provider CLIs on multi-dir `PATH`, git repo with explicit `remote.origin.pushurl` and user `pushInsteadOf`, and `collect.md` across reviewer switches.

| FR | Proof | Evidence | Human? |
|---|---|---|---|
| FR-1 | run `provider_switch_clears_stale_role_slugs_and_preflight_refuses_bad_config` | test transcript | no |
| FR-2 | run `review_second_isolates_all_five_providers_and_supports_dry_run` | test transcript | no |
| FR-3 | run `worker_env_strips_pat_disables_autocorrect_and_blocks_explicit_pushurl` | test transcript | no |
| FR-4 | run `collect_reuse_preflight_warnings_path_order_and_log_title_file` | test transcript | no |

No row needs a human, so no `qa-guide.md` is generated.

## 9. Slices

Four vertical slices in two conflict-free parallel branches (no two concurrent slices write the same file):

- **Slice 01 — Worker env and git push-off hardening (`FR-3`, deps: —):** expands `GH_TOKEN_VARS` with `GITHUB_PERSONAL_ACCESS_TOKEN`, adds `help.autocorrect=0` to `push_off_env`, and wires `push_off_env_for_repo(&cwd, ...)` into `Worker::spawn` for full-URL `pushInsteadOf` and explicit `remote.*.pushurl` `insteadOf` rewrites (`src/worker.rs`, `tests/worker.rs`). Lands first on branch A so Slice 02 consumes the expanded `GH_TOKEN_VARS`.
- **Slice 02 — Adversarial reviewer isolation and `--dry-run` (`FR-2`, deps: 01):** extracts `build_pass_command`, scrubs `GH_TOKEN_VARS` + Claude session vars across all five reviewers and `probe_resolved`, adds Claude `--setting-sources user --strict-mcp-config`, `--` prompt guards + clap `allow_hyphen_values = true`, OpenCode `OPENCODE_CONFIG_CONTENT` (top-level + `agent.plan`), and file-free `factory review second --dry-run` (`src/review.rs`, `src/cli/review.rs`, `tests/review.rs`).
- **Slice 03 — Safe provider switches and setup/preflight fixes (`FR-1`, deps: —):** adds `config::stale_role_clears` (respecting `FACTORY_WORKER_PROVIDER`) wired into `setup` and `worker_answer` (disk + in-memory), validates resolved worker model/effort as preflight refusals and bad `[review]` as warnings, and fixes `workers_step`, `role_sources`, and `setup --show` spacing (`src/config.rs`, `src/cli/setup.rs`, `src/cli/answer.rs`, `src/docs/preflight.rs`, `tests/setup.rs`). Lands first on branch B so Slice 04 layers cleanly onto `src/docs/preflight.rs`.
- **Slice 04 — Collect reuse, proceed wording, PATH order and `--title-file` (`FR-4`, deps: 03):** adds reviewer-stamped `collect::reusable_for`, `skipped_by_proceed` wording (+ `skills/polish/SKILL.md:65` and `0.17.0` → `0.18.0` crate/bundle/cockpit version bump), preflight config warnings, PATH-order `detect_installed_in` with `X_OK`, and `docs log add --title-file` (`src/collect.rs`, `src/closeout.rs`, `src/docs/preflight.rs`, `src/provider.rs`, `src/cli/docs.rs`, `skills/polish/SKILL.md`, `Cargo.toml`, `skills/bundle.lock`, `mods/cockpit/`, `tests/collect_and_provider.rs`).

Checkpoints fall every 2 done slices (`checkpoint_every = 2`: after 01+03 / 02+04, and once at the end). Workflow: one branch (`feat/multi-provider-hardening`), one PR, one commit per slice, Prove rows driven at checkpoints. No QA plan row needs a human, so no `qa-guide.md` is generated.

## 10. Non-goals

- **Live `codex exec --json` and `opencode run --format json` JSONL stream recordings.** Deferred 2026-11-26: requires logged-in interactive CLIs outside an unattended run; stays in `docs/ideas/2026-11-26-providers-live-codex-opencode-jsonl-recordings-rev.md`.
- **Interactive `.bashrc` PATH reordering inside non-Claude provider tool shells.** Not handled (2026-11-26): out of reach of spawn-time `PATH`, mitigated by FR-3's `GIT_CONFIG_*` `insteadOf`/`pushInsteadOf` + `GH_TOKEN_VARS` stripping.
- **Disabling Codex `~/.codex` user MCP, Cursor `.cursor/mcp.json`, or OpenCode plugins/local MCP.** Not handled (2026-11-26): no CLI flag isolates those surfaces today.

## 11. Verification notes

Checked against `main` at `970546b`, 2026-11-26, plus live CLI `--help` checks for `claude`, `cursor-agent`, and `agy`. **Re-verify at slice entry.**

- `src/config.rs:196-227` (`Workers::resolved`), `:520-565` (`role_sources`), `:650-686` (`resolve_review`), `:781-851` (`resolve_workers_from`, `resolve_role`).
- `src/cli/setup.rs:348-434` (`TableNow`, `table_now`, `worker_edits`), `:616-679` (`show`, `:655` `{role:<10}`), `:875-966` (`workers_step`).
- `src/cli/answer.rs:474-570` (`worker_answer`).
- `src/docs/preflight.rs:487-507` (`worker_refusals`), `:557-570` (`cli` drops `loaded.warnings`).
- `src/review.rs:692-696` (`probe_resolved`), `:901-1005` (`run_pass_resolved` for all 5 providers; Claude `--add-dir` + `--` + `scrub_claude_session`).
- `src/cli/review.rs:25-45, :118-210` (`SecondArgs`, `second`).
- `src/worker.rs:26-31` (`GH_TOKEN_VARS`), `:72-86` (`push_off_env`), `:137-156` (`ensure_shims`), `:445-470` (`Worker::spawn`, `cwd` vs `wdir`), `:824` (`the_push_off_env_goes_after_the_inherited_git_config`).
- `src/collect.rs:24-39, :98-182` (`DISABLED`, `head_line`, `reusable`, `collect`).
- `src/closeout.rs:380-428` (`review_for`, `collected`), `:2819` (`a_file_at_the_same_head_is_reused_unless_it_owes_a_rerun`).
- `src/provider.rs:228-260` (`resolve_binary_in`, `is_executable`, `detect_installed_in`).
- `src/cli/docs.rs:160-191, :401-435` (`LogVerb::Add`, `body_file`).
- `skills/polish/SKILL.md:65` (`second opinion:` plan line wording).
- `tests/review.rs:1246-1495` (`review_second_runs_all_five_providers_and_none_mode`), `tests/worker.rs:134, :202-218`, `tests/setup.rs:251, :462`, `tests/knobs.rs:104-109`.
- Live flags verified 2026-11-26: `claude --help` (`--setting-sources`, `--strict-mcp-config`, `--add-dir`), `cursor-agent --help` (`--mode`, `--model`, `--add-dir`), `agy --help` (`--mode`, `--model`, `--effort`, `--add-dir`, `--sandbox`).
