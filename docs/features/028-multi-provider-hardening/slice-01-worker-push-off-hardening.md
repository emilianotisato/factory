# Slice 01 — Worker env and git push-off hardening

**Source**: [`prd.md`](prd.md) → §5 (FR-3). **Depends on**: — _(independent; lands first so Slice 02 reuses the expanded `GH_TOKEN_VARS`)_
**Created**: 2026-11-26

**Progress**: cp-1/FR-3/transcript.txt

**Slice**: Closes all three `dec-2` PATH-shim bypasses in worker spawn env: strips `GITHUB_PERSONAL_ACCESS_TOKEN` alongside the existing GitHub token vars, injects `help.autocorrect=0` into `push_off_env`, and introduces `push_off_env_for_repo(&cwd, ...)` wired into `Worker::spawn` to emit full-URL `pushInsteadOf` rules for every `remote.*.url` (beating shorter user `pushInsteadOf` prefixes by git's longest-match rule) plus `insteadOf` rules for every explicit `remote.*.pushurl`. Leaves reviewer isolation to Slice 02.

**Highest-risk**: `Worker::spawn` must read remotes from `cwd` (the canonicalized lane worktree, `src/worker.rs:462`), NOT `wdir` (`run_dir/workers`, a log directory with no git remotes).

## Current code / environment

- **`GH_TOKEN_VARS`**: `src/worker.rs:26-31` — currently `[&str; 4]` (`GH_TOKEN`, `GITHUB_TOKEN`, `GH_ENTERPRISE_TOKEN`, `GITHUB_ENTERPRISE_TOKEN`); omits `GITHUB_PERSONAL_ACCESS_TOKEN`.
- **`push_off_env`**: `src/worker.rs:72-86` — currently emits `GIT_CONFIG_COUNT = n + 5` with 5 `url.factory-no-push://.pushInsteadOf` scheme prefixes (`git@`, `ssh://`, `https://`, `http://`, `git://`). Does not set `help.autocorrect=0`.
- **`Worker::spawn`**: `src/worker.rs:449-462` — `wdir = dir(&run_dir)` is the workers log dir (`run_dir/workers`), while `cwd` (`:462`) is the canonicalized lane worktree. `push_off_env` is called around line 560 with `std::env::var(GIT_CONFIG_COUNT).ok().as_deref()`.
- **Existing tests that assert exact `GIT_CONFIG_COUNT` / `git_config` map**:
  - `src/worker.rs:824` (`the_push_off_env_goes_after_the_inherited_git_config`) — unit test in `src/`, asserts 5 entries; update to 6 (including `help.autocorrect = "0"`).
  - `tests/worker.rs:134` (`a_worker_gets_no_github_and_no_session_but_keeps_the_account`) — **authorized in `prd.md`**; asserts `git["GIT_CONFIG_COUNT"]`; expect `"7"` (6 base + 1 harness `remote.*.url` pushInsteadOf rule).
  - `tests/worker.rs:202-218` (`a_worker_pushes_nowhere_through_the_git_push_off_env`) — **authorized in `prd.md`**; asserts `GIT_CONFIG_COUNT` and exact `want` map; expect `"8"` (1 inherited + 6 base + 1 harness remote URL rule) including `GIT_CONFIG_KEY_6: "help.autocorrect"`, `GIT_CONFIG_VALUE_6: "0"`.
- **`testguard` rule on `tests/worker.rs`**: Only the two functions named above are authorized for modification in `tests/worker.rs`. Add any new `use` imports on their own lines — never merge or edit existing top-level `use` lines.
- **Deliberate non-changes** — *(prd §5 Accepted cost & §10 Non-goals)*:
  - Do not attempt to reorder interactive `.bashrc` PATH inside non-Claude shells.
  - A `remote.*.pushurl` added by a worker *after* spawn is outside the spawn-time env snapshot (covered by `run_dir/bin/git`, `tests/worker.rs:592-609`).

## Tasks

### 1. Expand `GH_TOKEN_VARS` and add `help.autocorrect=0` + `push_off_env_for_repo` — high — blocked by: —
**Source**: §5 FR-3: "Add `GITHUB_PERSONAL_ACCESS_TOKEN` to `GH_TOKEN_VARS`", "Set `help.autocorrect=0` in `push_off_env`", "Block explicit `remote.*.pushurl` and longest-match user `pushInsteadOf` in `push_off_env_for_repo`".

`src/worker.rs`.
1. Expand `GH_TOKEN_VARS` to `[&str; 5]` adding `"GITHUB_PERSONAL_ACCESS_TOKEN"`.
2. In `push_off_env(inherited: Option<&str>) -> Vec<(String, String)>`, append `("help.autocorrect", "0")` after the 5 `PUSH_OFF_PREFIXES` entries (6 entries total, updating `GIT_CONFIG_COUNT` to `n + PUSH_OFF_PREFIXES.len() + 1`).
3. Add `pub fn push_off_env_for_repo(repo: &Path, inherited: Option<&str>) -> Vec<(String, String)>`:
   - Start from `push_off_env(inherited)` (extracting base count `next = n + 6`).
   - Run `git config --get-regexp '^remote\..*\.url$'` in `repo` (ignoring non-zero exit / non-repo errors cleanly). Extract each value (`line.split_once(' ').map(|(_, v)| v.trim())`), deduplicate preserving order, and append `url.factory-no-push://.pushInsteadOf = <url>`.
   - Run `git config --get-regexp '^remote\..*\.pushurl$'` in `repo` (ignoring non-zero exit / non-repo errors cleanly). Deduplicate values and append `url.factory-no-push://.insteadOf = <pushurl>`.
   - Update `env[0].1` (`GIT_CONFIG_COUNT`) to the final count.
4. In `Worker::spawn`, replace `push_off_env(inherited.as_deref())` with `push_off_env_for_repo(&cwd, inherited.as_deref())`.
5. Update `src/worker.rs:824` (`the_push_off_env_goes_after_the_inherited_git_config`) for the 6th `help.autocorrect = "0"` entry.

**The trap**: `git config --get-regexp` exits `1` when no remotes match (common in bare test fixtures). Treat exit `1` or spawn failure as empty, never as a worker spawn error. Also pass `&cwd` (lane worktree), NOT `&wdir` (`run_dir/workers`).

**Acceptance:** `src/worker.rs` unit test verifies `push_off_env` emits 6 entries ending in `help.autocorrect = 0`, and `push_off_env_for_repo` appends both `pushInsteadOf` for `remote.*.url` and `insteadOf` for `remote.*.pushurl` (fails if either query or entry is omitted).

### 2. Update authorized worker tests and add end-to-end hardening test — high — blocked by: 1
**Source**: §2 Blast radius & §5 FR-3 Acceptance: "Integration test `worker_env_strips_pat_disables_autocorrect_and_blocks_explicit_pushurl` in `tests/worker.rs` proves via a real `factory run --foreground` spawn (`Worker::spawn`)".

`tests/worker.rs`.
1. Update `a_worker_gets_no_github_and_no_session_but_keeps_the_account` (`:134`) from `"5"` to `"6"`.
2. Update `a_worker_pushes_nowhere_through_the_git_push_off_env` (`:202-218`) to expect `"7"` with `GIT_CONFIG_KEY_6: "help.autocorrect"`, `GIT_CONFIG_VALUE_6: "0"`.
3. Add `worker_env_strips_pat_disables_autocorrect_and_blocks_explicit_pushurl` in `tests/worker.rs`:
   - Configure a bare remote, add `origin` with explicit `git remote set-url --push origin <bare-remote>`, and add a second remote `upstream` with `https://github.com/org/repo.git` plus repo config `url.git@github.com:.pushInsteadOf = https://github.com/`.
   - Run `factory run 001 --foreground` with `GITHUB_PERSONAL_ACCESS_TOKEN=ghp_secret` in env.
   - Assert the recorded worker call does NOT have `GITHUB_PERSONAL_ACCESS_TOKEN` in its env.
   - Using the real `git` binary (resolved from `PATH` outside `run_dir/bin`, never hardcoded `/usr/bin/git`) with the spawned worker's recorded `git_config` env:
     - `git config --get help.autocorrect` outputs `"0"`.
     - `git remote get-url --push origin` starts with `"factory-no-push://"` and `git push origin HEAD` fails.
     - `git remote get-url --push upstream` starts with `"factory-no-push://"` (beating the user's `url.git@github.com:.pushInsteadOf` rule).

**The trap**: Do not modify any existing top-level `use` line in `tests/worker.rs` (add new `use` lines separately so `testguard` sees zero removed lines outside authorized functions). Note also that `fx.command(BIN)` runs in the fixture repo whose lane worktree shares the repo's `.git/config`, so remotes set on `fx.path()` are visible in the lane worktree `cwd`.

**Acceptance:** `cargo test --test worker worker_env_strips_pat_disables_autocorrect_and_blocks_explicit_pushurl` passes; removing `GITHUB_PERSONAL_ACCESS_TOKEN`, `help.autocorrect=0`, `remote.*.url`, or `remote.*.pushurl` rewriting each causes the corresponding assertion to fail.

### 3. Verify — medium — blocked by: 2
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 4. Prove — at the checkpoint — blocked by: 3
**Source**: §8 QA plan FR-3 row.
- **FR-3**: run `cargo test --test worker worker_env_strips_pat_disables_autocorrect_and_blocks_explicit_pushurl -- --nocapture` and keep the transcript. Expected: passes proving PAT stripping, `help.autocorrect=0`, explicit `pushurl` rewrite to `factory-no-push://`, and longest-match `pushInsteadOf` dominance via `Worker::spawn`.

**Acceptance:** after the checkpoint, the FR-3 row has transcript evidence in this slice's **Progress**.

**Code-review fixes (2026-11-26).**
- `push_off_env_for_repo` skipped empty pushurls and pushurls that overlap fetch URLs so `insteadOf` does not rewrite remotes the worker still needs to read; worker integration test now uses fixture git isolation and asserts `remote-factory-no-push` on stderr.
- `git_config_values` splits keys on tab as well as space; blast-radius docs corrected for harness `remote.*.url` GIT_CONFIG_COUNT expectations.
