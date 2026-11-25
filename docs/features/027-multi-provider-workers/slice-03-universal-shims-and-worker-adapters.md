# Slice 03 — Universal `dec-2` `PATH` shims, multi-provider worker spawn adapters, and live tail

**Source**: [`prd.md`](prd.md) → §4 (FR-2), §5 (FR-3), §6 (FR-4), §9, §11. **Depends on**: 01, 02 (needs `ResolvedWorker`, `ResolvedWorkers`, `RunState.worker_config`, `WorkerRef.provider` from 01; `stream::parse_for` and `metrics::record_subagents_for` from 02)
**Created**: 2026-11-25

**Progress**:
- Built: `worker::ensure_shims` writes `run_dir/bin/gh` (always exits 126 with `NEVER_HINT`) and `run_dir/bin/git`, which skips git's global options and refuses `push`/`send-pack` with 126, else execs the real git. Real git is looked up on the driver's `PATH` with `run_dir/bin` skipped. Both scripts are mode `0o700`, written by temp file and rename, and rewritten only when their text changes. `Worker::spawn` takes `&ResolvedWorker` and resolves the binary with `provider::resolve_binary`. It puts `run_dir/bin` first on the child's `PATH`, after the lane env, for every provider. Claude keeps its argv and `--settings` (the latter only for Claude). For any other provider it writes `workers/<sid>.prompt.md` (`0o600`, the pack then `\n---\n\n<prompt>\n`) and builds the exact codex, cursor, agy or opencode argv; codex reads the file on stdin. `Worker.provider` and `WorkerRef.provider` are set. Exits, `summary_from_disk` (and its `cli/status.rs` caller) and the three `settle`s use the worker's provider, the settles through `record_subagents_for` via `worker::provider_of`. `worker::effective_worker` gives every spawn site its role from `RunState.worker_config`; a run without one falls back to Claude with `cfg.workers`. `factory live --json` adds `"provider"` to each worker. `read_tail(.., provider)` maps Codex, Cursor and OpenCode lines onto the Claude shape (a final message with a `STATUS:` line counts as that stream's result) and reads Agy as Claude. The provider stubs live in `tests/stubs/workers/` (`codex`, `agy`, `opencode`, plus a shared `provider-worker.js`), and `tests/stubs/cursor-agent` runs it only with `--force`. They run the claude stub on the split pack and rewrite its stream into each provider's own stream shape. They are left off the fixture `PATH` because the preflight tests require `codex` to be missing from it.
- Inner loop: 1 run, 0 red, 2 min.
- Deliberate reds: `universal_path_shims_…` failed on the worker `PATH` not starting with `run_dir/bin` when the prepend was removed, and on `bare push` exiting 128, not 126, when the git shim's refusal case was disabled. `all_five_worker_providers_…` failed on the cursor argv once `--trust` was dropped. `mixed_provider_run_…` failed on a build worker recorded as `claude` once `effective_worker` ignored `worker_config`. `live_json_reads_non_claude_worker_streams` failed on a codex worker's `doing: null` once non-Claude lines were read as Claude.
- Deviation: AD-7 (fyi). The git shim does not resolve aliases.
- Prove (CP-2): FR-2, FR-3 and the FR-4 live row.
- CP-2 FR-2: cp-2/FR-2/universal_path_shims_block_gh_and_git_push_including_pushurl_while_passing_normal_git.txt, cp-2/FR-2/mutation-red.txt, cp-2/FR-2/mutation-restored.txt
- CP-2 FR-3: cp-2/FR-3/all_five_worker_providers_spawn_with_expected_flags_and_large_packs.txt, cp-2/FR-3/mixed_provider_run_completes_slices_checkpoint_and_polish.txt
- CP-2 FR-4: cp-2/FR-4/stream_normalizers_parse_status_findings_usage_and_rate_limits_for_all_five_providers.txt, cp-2/FR-4/non_claude_worker_skips_claude_subagent_lookup_without_warning.txt, cp-2/FR-4/live_json_reads_non_claude_worker_streams.txt

**Slice**: **Highest-risk slice.** Enforces `docs/decisions.md#dec-2` universally for every worker provider via `run_dir/bin/` `PATH` shims (`gh` + `git` push blocker), switches `Worker::spawn` and all driver call sites (`scheduler`, `checkpoint`, `closeout`) to dispatch across all five providers (`claude`, `codex`, `cursor`, `agy`, `opencode`) using `RunState.worker_config` (with Claude fallback for pre-upgrade `worker_config: None` runs), updates `live::workers` to parse non-Claude streams and emit `"provider"`, and adds multi-provider worker stubs and end-to-end mixed-provider integration tests. Leaves DB `workers.provider`, `runs.knobs`, `probe_provider_versions`, and `factory report` to slice 04.

## Current code / environment

- **Worker spawn**: `src/worker.rs:188-287` takes `cfg: &config::Worker`, writes `<session_id>.settings.json`, spawns `Command::new("claude")`, sets `GH_CONFIG_DIR = run_dir/gh-config` (mode `0o700`, pinned by `tests/worker.rs:125-130`), removes `GH_TOKEN_VARS`, injects `push_off_env`, and calls `stream::parse` in `summary_at` / `summary_from_disk` (`:418-427`).
- **All five spawn call sites & subagent call sites**:
  - `src/scheduler.rs:760-768` (`Build` and `Merge`), `:1862` (`record_subagents`)
  - `src/checkpoint.rs:275-283` (`Checkpoint`), `:707-718` (`Fix`), `:1177` (`record_subagents`)
  - `src/closeout.rs:268-276` (`Polish`), `:2050-2060` (`CiFix`), `:2449` (`record_subagents`)
  - Crash-recovery `summary_from_disk` callers across `src/scheduler.rs`, `src/checkpoint.rs`, `src/closeout.rs` (search `summary_from_disk` to pass `w.provider.unwrap_or(Provider::Claude)` everywhere!).
- **Live tail**: `src/live/workers.rs:23-60` emits `workers[]` JSON; `read_tail` (`:124-187`) and `wanted`/`assistant`/`label`/`outcome` (`:189-265`, reading `STATUS:` and `BLOCK:` per `assets/contract-slice.md:29`) currently parse only Claude JSONL.
- **Verified exact provider worker argv (PRD-027 §5 + PRD-026 AD-11)**:
  - **`claude`**: unchanged (`-p <pack.prompt>`, `--output-format stream-json --verbose`, `--permission-mode bypassPermissions`, `--append-system-prompt-file <pack_path>`, `--session-id <session_id>`, `--model <model>`, `--effort <effort>` when `Some`, narrowed `--strict-mcp-config --disable-slash-commands --tools Bash,Read,Edit,Write` for `build|fix|merge`, `--settings <settings_file>`).
  - **Non-Claude prompt file**: write `workers/<session_id>.prompt.md` (mode `0o600`, pack file contents + `\n---\n\n<pack.prompt>\n`) so large packs (> 150 KB) never hit `ARG_MAX`:
    - **`codex`**: `codex exec --json -m <model> [-c model_reasoning_effort=<effort>] --dangerously-bypass-approvals-and-sandbox -` with `stdin` connected to `workers/<session_id>.prompt.md` (`Stdio::from(File)`).
    - **`cursor`**: `<bin> -p --force --trust --output-format stream-json --stream-partial-output --model <model> --add-dir <workers_dir> "Read and follow all instructions in <abs_prompt_md>."` (never passes an effort flag).
    - **`agy`**: `<bin> -p --output-format stream-json --dangerously-skip-permissions --model <model> [--effort <effort>] [--disable-slash-commands when narrowed(role)] --add-dir <workers_dir> "Read and follow all instructions in <abs_prompt_md>."`.
    - **`opencode`**: `<bin> run --standalone --format json -m <model>[#<effort>] -f <abs_prompt_md> "Read and follow all instructions in the attached prompt file."`.
- **Deliberate non-changes (PRD-027 §11)**:
  - Soft `PreToolUse` nudges (`bare grep` → `factory ctx`, pack-read hints) stay Claude-only.
  - No tool allowlist narrowing (`--tools`) on Codex/Cursor/OpenCode (`agy` gets `--disable-slash-commands`).
  - No automatic provider failover on rate-limit pause.

## Tasks

### 1. Add universal `run_dir/bin/` `dec-2` `PATH` shims (`gh` + `git` push blocker) — high — blocked by: —
**Source**: §4 FR-2: "Add `worker::ensure_shims(run_dir: &Path) -> Result<PathBuf>` creating `run_dir/bin/` (mode `0o700`) with two executable scripts (`gh` and `git`)... prepend `run_dir/bin` to the worker child's `PATH` for all providers".

`src/worker.rs`, `tests/worker.rs`.
- Add `pub fn shims_dir(run_dir: &Path) -> PathBuf { run_dir.join("bin") }` and `pub fn ensure_shims(run_dir: &Path) -> Result<PathBuf>`:
  - Creates `run_dir/bin/` with `paths::private_dir_all` (mode `0o700`).
  - Resolves real `git` from the outer `PATH`, explicitly skipping `run_dir/bin` so nested invocations never recurse.
  - Writes `run_dir/bin/gh` (mode `0o700`, atomic temp + rename per `docs/decisions.md#dec-3`): `#!/bin/sh` printing `hook::NEVER_HINT` to stderr and exiting `126`.
  - Writes `run_dir/bin/git` (mode `0o700`, atomic temp + rename): `#!/bin/sh` scanning argv past global git options (`-C <d>`, `-c <kv>`, `--git-dir[=]<p>`, `--work-tree[=]<p>`, `--namespace[=]<n>`, `--exec-path[=]<p>`, `--super-prefix[=]<p>`, `--config-env[=]<v>`, `--bare`, `--no-pager`, `-p`, `--paginate`, `--no-replace-objects`, `--literal-pathspecs`, `--glob-pathspecs`, `--noglob-pathspecs`, `--icase-pathspecs`, `--no-optional-locks`, stopping option scan at `--`); if the first positional subcommand is `push` or `send-pack`, prints `hook::NEVER_HINT` to stderr and exits `126`; otherwise `exec '<real_git>' "$@"`.
- In `Worker::spawn`, prepend `run_dir/bin` to the child's `PATH` (reading `PATH` from lane `env` if present, else `std::env::var_os("PATH")`), placed **after** lane env is applied (right alongside `GH_TOKEN_VARS` removal, `GH_CONFIG_DIR = run_dir/gh-config`, and `push_off_env`).

**The trap**: Do NOT break `tests/worker.rs:125-130` (`GH_CONFIG_DIR` must remain `run_dir/gh-config`, mode `0o700`, empty dir) or `tests/stubs/claude` (which runs `git` and `$FACTORY_BIN` inside the worker's `PATH`). Make sure `git status`, `git add`, `git commit`, `git diff`, `git -C <dir> log -1`, and `git -c user.name=... commit` all pass straight through the `git` shim with exit code and stdout/stderr preserved! Also avoid shell-quoting bugs if the real `git` path contains spaces or quotes (use `paths::shell_word`).

**Acceptance:** Integration test `universal_path_shims_block_gh_and_git_push_including_pushurl_while_passing_normal_git` in `tests/worker.rs` proves under a worker's env (tested for both `claude` and `codex`):
- `gh pr list` and `gh api repos/x/y` exit `126` with `NEVER_HINT` on stderr.
- `git push`, `git -c url.https://example.invalid/.pushInsteadOf=x push`, `git -C <repo> push`, and `git push` on a remote with an explicit `pushurl` all exit `126` with `NEVER_HINT` on stderr.
- `git status`, `git add`, `git commit`, `git diff`, and `git -C <repo> log -1` succeed normally.
- `run_dir/bin` and `run_dir/gh-config` both have mode `0o700`.
- Fails if either `gh` or `git` shim is omitted from `PATH`.

### 2. Dispatch `Worker::spawn` and all driver call sites across all five providers, and update `live::workers` — high — blocked by: 1
**Source**: §5 FR-3 & §6 FR-4: "Change `Worker::spawn` to take `&config::ResolvedWorker`... build the exact child `Command` per provider... pass `provider` to `Worker::exit` / `summary_from_disk` / `metrics::record_subagents_for` / `live::workers::read_tail`".

`src/worker.rs`, `src/scheduler.rs`, `src/checkpoint.rs`, `src/closeout.rs`, `src/live/workers.rs`, `tests/worker.rs`, `tests/live.rs`, `tests/stubs/`.
- In `src/worker.rs`:
  - Change `Worker::spawn` to accept `cfg: &config::ResolvedWorker` (storing `pub provider: Provider` on `Worker` and `provider: Some(cfg.provider)` on `WorkerRef`).
  - Resolve binary via `provider::resolve_binary(cfg.provider)` (falling back to `cfg.provider.binaries()[0]` so OS error names the binary cleanly).
  - Build exact provider argv per the verified contract above:
    - `Provider::Claude`: keep existing flags, `--settings` (`settings_file`), `scrubbed` env removal, and `narrowed(role)` tools unchanged.
    - Non-Claude providers: write `workers/<session_id>.prompt.md` (mode `0o600`, reading `pack.path` + `\n---\n\n` + `pack.prompt` + `\n`) and build exact argv for `Codex` (stdin from `prompt.md`), `Cursor` (`--add-dir <wdir>`), `Agy` (`--add-dir <wdir>`, `--effort` when `Some`, `--disable-slash-commands` when `narrowed(role)`), `Opencode` (`run --standalone --format json -m <model>[#<effort>] -f <prompt.md>`).
    - Update `spawn` error message to name the actual binary (`cfg.provider.as_str()` / binary) instead of `"claude"`.
  - Update `Worker::exit`, `summary_at(path, provider)`, and `summary_from_disk(run_dir, session_id, provider)` to call `stream::parse_for(provider, &text)`.
- Add helper `effective_worker(run: &Run, cfg: &config::Config, role: &str) -> config::ResolvedWorker`:
  - When `run.state().worker_config` is `Some(ref wc)` → `wc.for_role(role).clone()`.
  - When `None` (pre-PRD-027 resumed runs) → Claude fallback using `cfg.workers` concrete `model`/`effort` (preserving pre-upgrade resume behavior).
- Update all 5 `Worker::spawn` sites (`src/scheduler.rs:760`, `src/checkpoint.rs:275, :707`, `src/closeout.rs:268, :2050`), all `summary_from_disk` callers, and all 3 `metrics::record_subagents` sites (`src/scheduler.rs:1862`, `src/checkpoint.rs:1177`, `src/closeout.rs:2449`) to pass the worker's `Provider` (`record_subagents_for`).
- In `src/live/workers.rs`:
  - Add `"provider": provider.as_str()` to each object in `workers()` (`:32-58`), using `w.provider.unwrap_or(Provider::Claude)`.
  - Update `read_tail(path, alive, paths, provider)` to extract `doing`, `narration`, and `outcome` (`STATUS:` and `BLOCK:` per `src/live/workers.rs:250-265`) for all 5 provider JSONL shapes (leaving `doing`/`narration`/`running` `None` when omitted by a stream, never panicking).
- Add stub scripts in `tests/stubs/` (`codex`, `agy`, `opencode`, and extend `cursor-agent` to handle worker mode `-p --force` without breaking existing review-mode tests!) so tests can run real multi-provider & mixed-provider `factory run` flows in `tests/worker.rs` and `tests/live.rs`.

**The trap**: `tests/stubs/cursor-agent` is ALREADY used by `tests/review.rs` and `tests/preflight.rs`! Keep `cursor-agent --list-models` and `cursor-agent -p ... --mode ask` 100% backward-compatible — only enter worker-stub behavior when `--force` is passed! Also remember `tests/harness/mod.rs` puts `tests/stubs/` first on `PATH`, so adding `codex`, `agy`, `opencode` stubs in `tests/stubs/` makes `detect_installed()` in `Fixture` see them — check `tests/preflight.rs` and `tests/setup.rs` to ensure none of their assertions assume only `claude` + `cursor-agent` exist in `tests/stubs/` (if any does, keep worker stubs in a dedicated helper dir or adjust the test cleanly!).

**Acceptance:** Integration tests `all_five_worker_providers_spawn_with_expected_flags_and_large_packs`, `mixed_provider_run_completes_slices_checkpoint_and_polish` (in `tests/worker.rs`), and `live_json_reads_non_claude_worker_streams` (in `tests/live.rs`) pass:
- Spawns each of the 5 providers and asserts exact argv, effort flag omission for `cursor` / `None`, `0o600` stream/stderr/prompt files, and a > 150 KB pack succeeding with zero `E2BIG` errors (fails if any provider adapter or prompt-file delivery is broken).
- Runs a multi-slice feature with `codex` on `build`, `agy` on `checkpoint`, and `claude` on `polish` through `scheduler::drive` to `Phase::Finished`, asserting each worker's `provider` is recorded on `state.json` and zero `SubagentsUnread` events fire for non-Claude workers (fails if per-role dispatch or non-Claude subagent skipping breaks).
- `factory live <id> --json` emits `"provider"` on every worker and extracts `outcome` (`STATUS:`/`BLOCK:`) and `narration`/`doing` from `codex`/`agy`/`cursor`/`opencode` streams (fails if `live/workers.rs` only parses Claude JSONL).

### 3. Verify — medium — blocked by: 1, 2
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q`

**Acceptance:** inner loop green, one commit.

### 4. Prove — at the checkpoint — blocked by: 3
Drive these rows from [`prd.md`](prd.md) §9 QA plan at CP-2 (after slice 04 lands):
- **FR-2**: run `cargo test universal_path_shims_block_gh_and_git_push_including_pushurl_while_passing_normal_git`.
- **FR-3**: run `cargo test all_five_worker_providers_spawn_with_expected_flags_and_large_packs` and `cargo test mixed_provider_run_completes_slices_checkpoint_and_polish`.
- **FR-4 (live)**: run `cargo test live_json_reads_non_claude_worker_streams`.

**Acceptance:** all three rows have passing test transcripts captured in checkpoint evidence and recorded in **Progress**.
