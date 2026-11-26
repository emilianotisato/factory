# Slice 02 — Adversarial reviewer isolation and --dry-run

**Source**: [`prd.md`](prd.md) → §4 (FR-2). **Depends on**: Slice 01 _(uses expanded `crate::worker::GH_TOKEN_VARS` including `GITHUB_PERSONAL_ACCESS_TOKEN`)_
**Created**: 2026-11-26

**Progress**: `scrub_reviewer_env`, `build_pass_command`/`PassCommand`, `factory review second --dry-run`, `allow_hyphen_values` on `--prompt`; tests `review_second_isolates_all_five_providers_and_supports_dry_run` + updated authorized run. Inner loop: 3 runs, 1 red, ~2 min — Codex argv order and OpenCode `-f`/`--` test expectation. Deliberate red: stripped Claude `--setting-sources` from `args` → isolation test failed on dry-run output; restored → green.

**Slice**: Isolates all five adversarial reviewer providers (and their probes) from GitHub tokens, Claude session vars, branch hooks/MCPs (Claude `--setting-sources user --strict-mcp-config`), OpenCode branch config overrides (`OPENCODE_CONFIG_CONTENT` denying top-level + `agent.plan` `edit`/`bash`/`webfetch`), and `-`-leading prompt flag injection (`--` before prompt + clap `allow_hyphen_values = true`), and adds file-free `factory review second --dry-run` sharing one `build_pass_command` builder with `run_pass_resolved`.

## Current code / environment

- **`probe_resolved`**: `src/review.rs:685-696` — builds `Command::new(&bin)`, currently calls `scrub_claude_session(&mut cmd)` ONLY when `p == Provider::Claude` and never removes `GH_TOKEN_VARS`.
- **`run_pass_resolved`**: `src/review.rs:901-1005` — builds provider argv inline:
  - `Cursor` (`:920-925`): `-p`, `FLAGS`, `--model`, `model`, `prompt` (no `--`, no env scrub).
  - `Claude` (`:926-946`): `-p --output-format stream-json --verbose --permission-mode plan --tools Read,Grep,Glob --model <m> [--effort <e>] --add-dir <dir> -- <prompt>` + `scrub_claude_session`. Missing `--setting-sources user --strict-mcp-config` and `GH_TOKEN_VARS` scrub.
  - `Codex` (`:947-964`): `exec --json --sandbox read-only --ephemeral -m <m> [-c model_reasoning_effort=<e>] -`, writes `dir/review-{n}.prompt` for stdin. Missing env scrub.
  - `Agy` (`:965-979`): `-p --mode plan --output-format stream-json --model <m> [--effort <e>] --add-dir <dir> <prompt>`. Missing `--` before `prompt` and env scrub.
  - `Opencode` (`:980-1001`): `run --standalone --format json --agent plan -m <m>[#<e>] [-f <dir>/review-{n}.diff] <prompt>`. Missing `--` before `prompt`, `OPENCODE_CONFIG_CONTENT`, and env scrub.
- **`SecondArgs` & `second()`**: `src/cli/review.rs:25-45, :118-222` — `SecondArgs.prompt` (`:34-35`) lacks `allow_hyphen_values = true`, so clap exits 2 on `--prompt "-check this"`. `second()` calls `review::new_log_dir` (`:184`) and `review::probe_resolved` (`:185`) unconditionally.
- **Authorized test in `tests/review.rs`**: `tests/review.rs::review_second_runs_all_five_providers_and_none_mode` (`:1246-1495`) — asserts exact argv arrays (`:1285-1365`) AND at `:1427` asserts non-Claude calls (both probe and pass) keep `CLAUDECODE`. Declared in `prd.md` `authorized_tests`.
- **`testguard` rule on `tests/review.rs`**: Only `review_second_runs_all_five_providers_and_none_mode` is authorized for modification. Add any new `use` imports on separate lines; do not touch other existing test functions.
- **Deliberate non-changes** — *(prd §4 Accepted cost & §10 Non-goals)*:
  - Do not try to disable Codex `~/.codex` MCP, Cursor `.cursor/mcp.json`, or OpenCode plugins/local MCP via unsupported flags.

## Tasks

### 1. Extract shared `build_pass_command` and isolate all five reviewers + `probe_resolved` — high — blocked by: —
**Source**: §4 FR-2: "Build pass commands in one shared helper `pub fn build_pass_command(...)`", "All five providers (in BOTH `build_pass_command` and `probe_resolved`)", "`Provider::Claude`: Add `--setting-sources`, `user` and `--strict-mcp-config`", "`Provider::Opencode`: Set `OPENCODE_CONFIG_CONTENT`".

`src/review.rs`.
1. Add a helper `scrub_reviewer_env(cmd: &mut Command)` that removes every var in `crate::worker::GH_TOKEN_VARS` AND calls `scrub_claude_session(cmd)`. Call `scrub_reviewer_env(&mut cmd)` in `probe_resolved` (`:692-696`) for ALL five providers (replacing the `if p == Provider::Claude` guard) and in `build_pass_command` for ALL five providers.
2. Add a public struct `PassCommand` (holding `pub cmd: Command`, `pub bin: String`, `pub args: Vec<String>`, `pub stdin: Option<PathBuf>`, `pub env_set: Vec<(String, String)>`, `pub env_remove: Vec<String>`, `pub stdin_note: Option<String>`) and `pub fn build_pass_command(cfg: &ResolvedReview, cwd: &Path, dir: &Path, n: usize, prompt: &str, dry_run: bool) -> Result<PassCommand>`:
   - **`Claude`**: add `"--setting-sources", "user", "--strict-mcp-config"` alongside existing flags, keeping `--add-dir <dir> -- <prompt>`.
   - **`Cursor`**: add `"--"` before `prompt`.
   - **`Agy`**: add `"--"` before `prompt`.
   - **`Opencode`**: add `"--"` before `prompt`; when `dry_run` is `true` and the prompt references a diff file (or `dir.join(format!("review-{n}.diff")).is_file()` when `dry_run` is `false`), include `-f <diff>` appropriately without creating any file on disk; set env `OPENCODE_CONFIG_CONTENT` to `r#"{"permission":{"edit":"deny","bash":"deny","webfetch":"deny"},"agent":{"plan":{"permission":{"edit":"deny","bash":"deny","webfetch":"deny"}}}}"#`.
   - **`Codex`**: keep `-` stdin arg; when `dry_run` is `false`, write `dir.join(format!("review-{n}.prompt"))` and set `stdin = Some(file)`; when `dry_run` is `true`, skip writing any file and note stdin prompt.
   - Apply `scrub_reviewer_env(&mut cmd)` and `cmd.current_dir(cwd)`.
3. Refactor `run_pass_resolved` to call `build_pass_command(cfg, cwd, dir, n, prompt, false)?` and pass `built.cmd` and `built.stdin.as_deref()` to `run_logged`.

**The trap**: Keep `build_pass_command` strictly file-free when `dry_run = true` — do NOT write `review-1.prompt` or create `dir` on disk in dry-run mode. Also make sure `env_remove` lists every `GH_TOKEN_VARS` entry plus `CLAUDECODE`/`CLAUDE_*` summary so `--dry-run` can print what is scrubbed.

**Acceptance:** Unit tests in `src/review.rs` (or `tests/review.rs`) verify `build_pass_command(..., dry_run = true)` creates zero files on disk, emits `--setting-sources user --strict-mcp-config` for Claude, `--` before prompt for Claude/Cursor/Agy/OpenCode, and `OPENCODE_CONFIG_CONTENT` for OpenCode (fails if any flag/env is removed or if a file is created under `dir`).

### 2. Add `allow_hyphen_values = true` on `SecondArgs.prompt` and implement `factory review second --dry-run` — high — blocked by: 1
**Source**: §4 FR-2: "Add `allow_hyphen_values = true` on `SecondArgs.prompt`", "`factory review second --dry-run`".

`src/cli/review.rs`.
1. On `SecondArgs` (`:25-45`):
   - Update `prompt`: `#[arg(long, value_name = "TEXT", allow_hyphen_values = true)] pub prompt: Option<String>`.
   - Add `#[arg(long)] pub dry_run: bool` (update `USAGE` string at `:63` to mention `[--dry-run]`).
2. In `second()` (`:118-222`):
   - After resolving `cfg`, `root`, `passes`, `skipped`, and `settled` — BEFORE `review::new_log_dir` and `review::probe_resolved`:
   - If `a.dry_run`:
     - Do NOT call `review::new_log_dir`, `review::probe_resolved`, or `review::run_passes`.
     - Build pass 1's prompt without writing any diff file on disk (for `Mode::Diff`, use a placeholder dir like `Path::new("<log-dir>")` and pass a placeholder prompt/diff path so OpenCode shows `-f <log-dir>/review-1.diff` cleanly; for `Mode::Doc`, build the doc prompt directly).
     - Call `review::build_pass_command(&cfg, &root, Path::new("<log-dir>"), 1, &sample_prompt, true)?`.
     - Print resolved provider/model/effort, binary, pass count (`passes.len()`), shell-quoted command line (`argv`), and explicit env overrides (`env_set`) / scrubbed vars (`env_remove`) to `out`, and return `Ok(0)`.

**The trap**: `prompt()` in `src/cli/review.rs` writes `review-{n}.diff` to `dir` in `Mode::Diff`! In `--dry-run` mode, do NOT call the file-writing `Mode::Diff` branch of `prompt()` (or pass a `dry_run` flag so it skips writing `review-1.diff`). Check that `paths::data_dir()` has zero new review directories created after `--dry-run`.

**Acceptance:** Running `factory review second --doc <file> --prompt "-check this" --dry-run` exits `0`, prints the provider/model, shell-quoted argv including `--` before `"-check this"`, creates no log dir, and never runs the stub binary (fails if clap rejects `-check this`, if `--dry-run` runs the probe/stub, or if a log dir is created).

### 3. Update authorized test and add full reviewer isolation + `--dry-run` test — high — blocked by: 2
**Source**: §2 Blast radius & §4 FR-2 Acceptance.

`tests/review.rs`.
1. Update authorized test `review_second_runs_all_five_providers_and_none_mode` (`:1246-1495`):
   - Update expected Claude argv to include `--setting-sources user --strict-mcp-config`.
   - Update expected Cursor, Agy, and OpenCode argv to include `--` before prompt.
   - Update `:1427` so non-Claude calls (both probe and pass) assert `CLAUDECODE` is scrubbed (`None`), not kept.
2. Add `review_second_isolates_all_five_providers_and_supports_dry_run` in `tests/review.rs`:
   - Exercise all 5 providers with `GH_TOKEN`, `GITHUB_TOKEN`, `GITHUB_PERSONAL_ACCESS_TOKEN`, and `CLAUDECODE` set in the parent env.
   - Verify BOTH probe and pass invocations across all 5 providers have all `GH_TOKEN_VARS` (including `GITHUB_PERSONAL_ACCESS_TOKEN`) and `CLAUDECODE` scrubbed.
   - Verify `--doc <file> --prompt "-check this"` passes clap and reaches Claude, Cursor, Agy, and OpenCode after `--`.
   - Verify Claude argv has `--setting-sources user --strict-mcp-config`.
   - Verify OpenCode receives `OPENCODE_CONFIG_CONTENT` whose parsed JSON denies `edit`, `bash`, and `webfetch` at both top-level `permission` and `agent.plan.permission`.
   - Verify `factory review second --doc <file> --prompt "hi" --dry-run` (and `--diff <base> --dry-run`) exits `0`, prints exact argv and env scrubbing/overrides, creates zero files under `data_dir/reviews`, and records zero stub invocations.

**Acceptance:** `cargo test --test review review_second_isolates_all_five_providers_and_supports_dry_run` and `review_second_runs_all_five_providers_and_none_mode` both pass; dropping any isolation flag, env scrub, `allow_hyphen_values`, or dry-run guard breaks the corresponding assertion.

### 4. Verify — medium — blocked by: 3
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 5. Prove — at the checkpoint — blocked by: 4
**Source**: §8 QA plan FR-2 row.
- **FR-2**: run `cargo test --test review review_second_isolates_all_five_providers_and_supports_dry_run -- --nocapture` and keep the transcript. Expected: passes proving all-5-provider env scrubbing (probe + pass), Claude `--setting-sources user --strict-mcp-config`, `--` prompt guard + `-`-led `--prompt`, OpenCode `OPENCODE_CONFIG_CONTENT`, and file-free `--dry-run`.

**Acceptance:** after the checkpoint, the FR-2 row has transcript evidence in this slice's **Progress**.
