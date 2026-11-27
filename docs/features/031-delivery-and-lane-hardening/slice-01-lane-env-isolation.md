# Slice 01 — Lane env isolation and shell clobber check

**Source**: [`prd.md`](prd.md) → §3 (FR-1). **Depends on**: — (independent; writes no files touched by Slice 02)
**Created**: 2026-11-27

**Progress**: _(written as it lands — what was built, the gate numbers, what a review caught)_

**Slice**: Protects parallel lanes against interactive/login shells that overwrite declared lane variables like `CARGO_TARGET_DIR`: resolves relative `CARGO_TARGET_DIR` to an absolute path inside the lane, surfaces `# Lane env` and `export K=V; <cmd>` prefixes across all worker packs, and adds a timeout-bounded `$SHELL -lic` clobber warning to `factory lanes up --prove`. Leaves checkpoint Prove deferral (Slice 03) and CLI/docs changes (Slices 02, 04) to their own slices.

**Highest-risk**: `$SHELL -lic` probing inside `factory lanes up --prove` — must set `stdin(Stdio::null())`, bound execution to a 3-second timeout so interactive zsh frameworks cannot hang `cargo test`, and never break existing `tests/lanes.rs` tests.

## Current code / environment

- **`src/lanes.rs:93-98`**: `Lane::env(&self)` returns `FACTORY_LANE=<n>` plus raw `(k, v)` pairs from `docs/reference/lanes/lane-N.md` (where `CARGO_TARGET_DIR=target/lane-N` is relative).
- **`src/cli/lanes.rs:144-214`**: `prove()` spawns `sh -c <inner>` with `.envs(lane.env())` (`:174`) and never checks whether the user's interactive/login shell clobbers declared lane vars.
- **All `lane_def.env()` call sites**: `src/scheduler.rs:769`, `src/checkpoint.rs:119` (`gate_once`), `src/checkpoint.rs:329` (checkpoint worker), `src/checkpoint.rs:895` (fix worker), `src/closeout.rs:275` (polish worker), `src/closeout.rs:2169` (ci-fix worker), and `src/cli/lanes.rs:174`.
- **`src/pack.rs:211-310, 437, 470-565, 682`**: `Pack::for_slice`/`for_slice_with`, `for_merge`, `for_ci_fix` render `# Inner loop` with bare `inner.trim()`; `for_fix` and `for_checkpoint` carry `# Gate transcript` and no `# Lane env` section. Existing unit tests in `src/pack.rs` call `Pack::for_slice`, `for_slice_with`, `for_fix`, `for_checkpoint` with current signatures — keep those signatures as thin wrappers passing `&[]` so zero existing tests break.
- **Deliberate non-change**: Do NOT edit the user's shell dotfiles (`~/.bashrc`) or add a `cargo` PATH shim *(prd §10 Non-goals decided against mutating dotfiles; pack exports + `--prove` warning handle it universally.)*

## Tasks

### 1. Resolve relative `CARGO_TARGET_DIR` in `Lane::env_for` and thread it to every spawn — high — blocked by: —
**Source**: §3 FR-1: "Add `pub fn env_for(&self, lane_dir: &Path) -> Vec<(String, String)>`: same as `self.env()`, except when a key is `CARGO_TARGET_DIR` and its value is a relative path, resolve it to `std::path::absolute(lane_dir.join(val))` (falling back to `lane_dir.join(val)`)... Use `lane_def.env_for(&lane_dir)` everywhere a lane env is passed to a gate or worker."

`src/lanes.rs`, `src/scheduler.rs`, `src/checkpoint.rs`, `src/closeout.rs`, `src/cli/lanes.rs`.
- Add `pub fn env_for(&self, lane_dir: &Path) -> Vec<(String, String)>` on `Lane` in `src/lanes.rs`:
  - Call `self.env()`, and for any `(k, v)` where `k == "CARGO_TARGET_DIR"` and `!Path::new(&v).is_absolute()`, replace `v` with `std::path::absolute(lane_dir.join(&v)).unwrap_or_else(|_| lane_dir.join(&v)).to_string_lossy().into_owned()`.
- Switch all 7 call sites from `.env()` to `.env_for(&lane_dir)`: `src/scheduler.rs:769`, `src/checkpoint.rs:119, 329, 895`, `src/closeout.rs:275, 2169`, `src/cli/lanes.rs:174`.
- Add unit tests in `src/lanes.rs` verifying relative `CARGO_TARGET_DIR` resolves under `lane_dir`, absolute `CARGO_TARGET_DIR` stays unchanged, and other env vars pass through verbatim.

**Acceptance:** Unit tests in `src/lanes.rs` prove `Lane::env_for` resolves relative `CARGO_TARGET_DIR=target/lane-2` to `<abs-lane-dir>/target/lane-2` and leaves absolute paths and non-cargo vars untouched (fails if `env_for` returns raw `self.env()`).

### 2. Prefix `# Inner loop` with `export K=V;` and add `# Lane env` across worker packs — high — blocked by: 1
**Source**: §3 FR-1: "Add pure helper `pub fn env_prefixed(env: &[(String, String)], cmd: &str) -> String` in `src/pack.rs`: when `env` contains lane-declared vars (excluding `FACTORY_LANE`), ALWAYS emit `export K1=<quoted> K2=<quoted>; <cmd>` (shell-quoting each value via `paths::shell_word`)... Thread `lane_env: &[(String, String)]` into slice, fix, merge, checkpoint, and ci-fix packs... add a `# Lane env` section (omitted when `lane_env` has no custom vars beyond `FACTORY_LANE`)."

`src/pack.rs`, `src/scheduler.rs`, `src/checkpoint.rs`, `src/closeout.rs`.
- Add `pub fn env_prefixed(env: &[(String, String)], cmd: &str) -> String` and `fn lane_env_section(env: &[(String, String)]) -> Option<String>` in `src/pack.rs`:
  - Filter `env` to entries where `k != lanes::ENV_LANE`.
  - If empty: `env_prefixed` returns `cmd.to_string()`, `lane_env_section` returns `None`.
  - Otherwise: format assignments as `{k}={paths::shell_word(v)}` joined by spaces; `env_prefixed` returns `format!("export {assignments}; {cmd}")`; `lane_env_section` returns `# Lane env` containing `export {assignments}` and one line noting that workers must keep those exports active when running build/test commands in interactive shells.
- Keep existing `Pack::for_slice`, `for_slice_with`, `for_fix`, `for_merge`, `for_checkpoint`, `for_ci_fix` public signatures unchanged (forwarding `&[]` for `lane_env`), and add `_in_lane` (or `_with_env`) constructors used by `src/scheduler.rs`, `src/checkpoint.rs`, and `src/closeout.rs` passing `&lane_def.env_for(&lane_dir)`.
- Prefix `# Inner loop` via `env_prefixed(lane_env, inner.trim())` in slice, merge, and ci-fix packs, and include `# Lane env` when `Some` in all five pack kinds (slice, merge, fix, checkpoint, ci-fix).
- **The trap**: Do NOT break existing `src/pack.rs` unit tests that assert exact section order or `# Inner loop` text when no lane env is passed (`lane_env == &[]` must keep existing output byte-identical!).

**Acceptance:** Unit tests in `src/pack.rs` prove `env_prefixed` always emits `export CARGO_TARGET_DIR=<quoted>; <cmd>` for chained commands, omits prefix when only `FACTORY_LANE` is present, and slice/merge/fix/checkpoint/ci-fix packs include `# Lane env` and prefixed `# Inner loop` when custom lane env is supplied (fails if `env_prefixed` or `# Lane env` rendering is removed).

### 3. Detect interactive/login shell clobbering in `factory lanes up --prove` — high — blocked by: 1, 2
**Source**: §3 FR-1: "Add helper `pub fn detect_shell_clobber(shell: &Path, env: &[(String, String)]) -> Vec<(String, String, String)>`: Spawns `<shell> -lic 'printf ...'` with `.stdin(Stdio::null())`, `.stdout(Stdio::piped())`, `.stderr(Stdio::null())`, passing `(k, v)` in the child env, and enforces a 3-second timeout... In `factory lanes up --prove`: unless `FACTORY_PROVE_SHELL` is `\"off\"`, resolve shell from `FACTORY_PROVE_SHELL` else `$SHELL`... print `! shell <shell> -lic overwrites <K> (<want> → <got>) — guard it in your shell profile with export <K>=\"${<K>:-<got>}\"` (warning only, exit code unchanged)."

`src/cli/lanes.rs`, `tests/delivery_hardening.rs` (new).
- Implement `pub fn detect_shell_clobber(shell: &Path, env: &[(String, String)]) -> Vec<(String, String, String)>` in `src/cli/lanes.rs`:
  - Filter `env` to vars with `k != lanes::ENV_LANE` and valid POSIX shell identifier names (`!k.is_empty()` and ASCII alphanumeric / `_`, not starting with digit). If empty or `!shell.is_file()`, return `Vec::new()`.
  - Unless `allow_any_basename` (i.e. explicit `FACTORY_PROVE_SHELL` override), only run when `shell.file_name()` is one of `sh`, `bash`, `zsh`, `dash`, `ash`, `ksh`.
  - Build a shell script printing `<K>=<value>\0` (or one marker-delimited line per key: `printf '__FACTORY_ENV_%s__=%s\n' K "$K"`) so any startup banners printed by `.bashrc`/`.zshrc` are ignored and only our marker lines are parsed!
  - Spawn `Command::new(shell).args(["-lic", &script]).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).envs(custom_vars).spawn()`.
  - Wait with a 3-second timeout (poll `try_wait` in short sleeps; on timeout `kill()` + `wait()` and return `Vec::new()`).
  - For each marker line matching `__FACTORY_ENV_{K}__={got}`, if `got != want`, push `(K, want, got)`.
- Call `detect_shell_clobber` once inside `prove()` (after `up_lanes` succeeds, using lane 1's `env_for(&dir)`) unless `FACTORY_PROVE_SHELL == "off"`. Print each clobbered var warning via `say(out, ...)` without altering exit code.
- **The trap**: Startup scripts often echo motd/banners on stdout in `-lic` mode — using unique `__FACTORY_ENV_{K}__=` markers and scanning lines from the end ensures startup noise never causes false positives.
- Create `tests/delivery_hardening.rs` with `lane_env_resolves_target_dir_prefixes_pack_and_warns_on_shell_clobber` testing:
  1. `Lane::env_for` resolving relative `CARGO_TARGET_DIR` to `<lane_dir>/target/lane-2`.
  2. `pack::env_prefixed` and slice/checkpoint packs emitting `export CARGO_TARGET_DIR=...` and `# Lane env`.
  3. `detect_shell_clobber` with: (a) a stub shell script that overwrites `CARGO_TARGET_DIR` → returns clobber tuple; (b) a stub shell script that preserves it → returns empty; (c) a stub shell script that sleeps 10 s → terminates within ~3.5 s returning empty without hanging.
  4. `factory lanes up --prove` on a fixture repo with `FACTORY_PROVE_SHELL=<clobber-stub>` prints `! shell ... overwrites CARGO_TARGET_DIR` and exits `0`.

**Acceptance:** `cargo test --test delivery_hardening lane_env_resolves_target_dir_prefixes_pack_and_warns_on_shell_clobber` passes (fails if `env_for`, `env_prefixed`, `# Lane env`, or `detect_shell_clobber` warning is removed).

### 4. Verify — medium — blocked by: 1, 2, 3
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 5. Prove — at the checkpoint — blocked by: 4
**Source**: §8 QA plan FR-1 row: "run `lane_env_resolves_target_dir_prefixes_pack_and_warns_on_shell_clobber`".

At Checkpoint 1, run `cargo test --test delivery_hardening lane_env_resolves_target_dir_prefixes_pack_and_warns_on_shell_clobber` alongside the full gate and record the transcript in **Progress**.

**Acceptance:** Checkpoint transcript in **Progress** confirms `lane_env_resolves_target_dir_prefixes_pack_and_warns_on_shell_clobber` passes (would fail if lane env resolution, pack export prefixing, or shell clobber warning were broken).
