# Slice 01 — Base-ref resolver and docs/project.toml guard

**Source**: [`prd.md`](prd.md) → §3 FR-1, §5 FR-3, §9 QA plan. **Depends on**: — (spine: introduces `commit::resolve_base_ref` and lands `src/commit.rs` / `src/closeout.rs` / `assets/autonomy.md` base changes that slices 02 and 03 build on).
**Created**: 2026-11-26

**Progress**: _(written as it lands — what was built, the gate numbers, what a review caught)_

**Slice**: Protects `docs/project.toml` from every unattended worker (including polish) and adds one shared `commit::resolve_base_ref` wired into every git diff/log/show caller (`leaks`, `review`, `collect`, `retro`, `pack`, `checkpoint`, `closeout`) plus `origin/{main,master}` fallback in `preflight::default_branch`. Leaves base-ref `[leaks] allow` loading and hyphen-less plan-id rules to slice 02, and run-start `authorized_tests` enforcement to slice 03.

## Current code / environment

- **`commit::protected_files` (`src/commit.rs:11-13`)**: returns `[String; 2]` (`[format!("{}/prd.md", board.dir), "docs/index.md".to_string()]`). Callers (`src/scheduler.rs:1044`, `src/checkpoint.rs:895`, `src/closeout.rs:561`) borrow it as `&[String]` or `.to_vec()`. Changing the return type to `[String; 3]` compiles cleanly everywhere.
- **`Closer::Polish::protected` (`src/closeout.rs:558-563`)**: returns `vec!["docs/index.md".to_string()]` so polish may update `prd.md` Status prose. It must add `"docs/project.toml".to_string()` while continuing not to list `prd.md`.
- **Lane commit & protected drops (`src/land.rs:48-89`)**: `land::commit_lane` stages with `git add -A`, drops `board` + `protected`, returns dropped protected paths in `LaneCommit.dropped`. `src/scheduler.rs:1061` emits `EventKind::WorkerWroteProtectedFile`; `src/checkpoint.rs:909` and `src/closeout.rs:685` log dropped paths. Existing test `src/land.rs:440` (`board_and_protected_files_stay_out_and_only_protected_ones_are_named`) passes its own constant slice, not `commit::protected_files`.
- **`assets/autonomy.md:67`**: lets workers extend gate configuration to cover new code without stating that `docs/project.toml` itself stays protected.
- **`preflight::default_branch` (`src/docs/preflight.rs:30-52`)**: checks `refs/remotes/origin/HEAD` (stripping `"origin/"`), then local `refs/heads/{main,master}` only. It misses `refs/remotes/origin/{main,master}` when `origin/HEAD` is unset and no local `main`/`master` branch exists.
- **Plain branch name vs. git diff ref**: `gh pr create --base` (`src/closeout.rs:1212-1217`, `src/ci.rs:405-427`), `commit::branch_refusal` (`src/commit.rs:86`), and `ci::branch_problem` (`src/closeout.rs:1325`) MUST continue receiving the plain branch name (e.g. `"main"`), never `"origin/main"`.
- **Git diff/log callers needing `resolve_base_ref`**:
  - `src/leaks.rs:787-795` (`scan`, `Input::Base`)
  - `src/review.rs:241-253` (`numstat`) and `:380-398` (`diff`) when `Range::Base(b)` is passed; option-injection guards in `src/cli/review.rs:94, :121` stay intact
  - `src/collect.rs:98-112` (`collect` calling `review::numstat` and `leaks_part` with `opts.base`)
  - `src/retro.rs:38-47` (`changed_lines`, called from `src/closeout.rs:1034`)
  - `src/pack.rs:529-562` (`Pack::for_polish` takes one `default_branch: &str` plain-branch param, prints ``Default branch: `{default_branch}` `` at `:560`, and runs `git log --oneline` at `:544`, falling back silently to `"(no commits listed)"` on git error at `:547-551`)
- **Deliberate non-changes (`prd.md` §1, §11)**: do not read `[leaks] allow` from `<base>` here (slice 02), do not touch `ids!()` in `src/leaks.rs` (slice 02), and do not add `authorized_tests` checks (slice 03).

## Tasks

### 1. Protect `docs/project.toml` across all workers and clarify `assets/autonomy.md` — high — blocked by: —
**Source**: §3 FR-1: "Change `commit::protected_files(board: &Status) -> [String; 3]` ... to return `[prd.md, docs/index.md, docs/project.toml]`. Change `Closer::Polish::protected` ... to return `vec!["docs/index.md", "docs/project.toml"]` ... Update `assets/autonomy.md:67` to clarify that extending tool config ... is allowed, while `docs/project.toml` itself stays protected from all workers".

- `src/commit.rs:11-13`: change `protected_files(board: &Status) -> [String; 3]` to include `"docs/project.toml".to_string()`.
- `src/closeout.rs:557-563`: change `Closer::Polish::protected` to return `vec!["docs/index.md".to_string(), "docs/project.toml".to_string()]` and update the doc comment above it.
- `assets/autonomy.md:67`: add one clear clause noting that extending tool config (e.g. `phpstan.neon`, `tsconfig.json`) is allowed, while `docs/project.toml` itself stays protected from all workers (propose any `docs/project.toml` edit in the PR body or block).
- `tests/merge.rs` (or `tests/closeout.rs` / `src/commit.rs`): add `project_toml_is_dropped_from_slice_and_polish_lane_commits` verifying `commit::protected_files(board)` and polish protection, and exercising `land::commit_lane` with both protected lists when a lane edits `docs/project.toml` alongside a normal file — asserting `docs/project.toml` is absent from `LaneCommit.files`, present in `LaneCommit.dropped`, untouched in the main tree after `land::land`, and (for a slice worker in a fixture run or scheduler test) `EventKind::WorkerWroteProtectedFile` is emitted for `docs/project.toml`. Also assert the new `docs/project.toml` protection sentence occurs exactly once in `assets/autonomy.md`.

**Trap**: `Closer::Polish` must NOT add `prd.md` to its protected list — polish still updates `prd.md` Status prose (`src/closeout.rs:557-560`).

**Acceptance:** `project_toml_is_dropped_from_slice_and_polish_lane_commits` passes and fails if `"docs/project.toml"` is removed from either `commit::protected_files` or `Closer::Polish::protected` (causing `docs/project.toml` to appear in `LaneCommit.files` and vanish from `LaneCommit.dropped`), or if the exact protection sentence is removed from `assets/autonomy.md`.

### 2. Add `commit::resolve_base_ref` and `origin/{main,master}` fallback in `preflight::default_branch` — high — blocked by: 1
**Source**: §5 FR-3: "Add `pub fn resolve_base_ref(repo: &Path, preferred: Option<&str>) -> Result<String>` in `src/commit.rs` ... Reject any `preferred` starting with `'-'` ... When `preferred` is `Some(name)` (non-empty): candidates = `[name]` only ... When `preferred` is `None` (or empty): candidates = `[origin/HEAD stripped of "origin/" (if set), "main", "master"]` ... Update `preflight::default_branch` ... also checks `refs/remotes/origin/main` and `refs/remotes/origin/master`".

- `src/commit.rs`: add `pub fn resolve_base_ref(repo: &Path, preferred: Option<&str>) -> Result<String>`:
  1. If `preferred` is `Some(name)` and `name.starts_with('-')`, return `Err(Error::Msg(format!("base `{name}` starts with `-`, and git would read it as an option")))`.
  2. Build candidate names:
     - If `preferred` is `Some(name)` with `!name.is_empty()`: `vec![name.to_string()]` ONLY (never fall back to `origin/HEAD`, `"main"`, or `"master"` when an explicit base was given).
     - Otherwise (`None` or `Some("")`): if `refs/remotes/origin/HEAD` resolves via `git symbolic-ref --short refs/remotes/origin/HEAD`, start with that name stripped of `"origin/"`; then append `"main"` and `"master"` (deduplicated).
  3. For each candidate `name`:
     - Check `git rev-parse --verify --quiet {name}^{commit}` (`commit::git_code(repo, &["rev-parse", "--verify", "--quiet", &format!("{name}^{{commit}}")])? == 0`) → return `Ok(name)`.
     - Else if `!name.starts_with("origin/")`, check `git rev-parse --verify --quiet refs/remotes/origin/{name}^{commit}` → return `Ok(format!("origin/{name}"))`.
  4. If none resolves, return `Err(Error::Msg(...))` naming the requested/tried base ref(s).
- `src/docs/preflight.rs:30-52`: update `default_branch` so after `refs/remotes/origin/HEAD` and `refs/heads/{main,master}`, it also checks `refs/remotes/origin/{main,master}`, returning the plain branch name (`Some("main")` / `Some("master")`).

**Trap**: In the clone fixture test, switch to `feat/x` BEFORE running `git branch -D main`, and run `git remote set-head origin -d` so `origin/HEAD` is truly unset when testing `preflight::default_branch`'s `refs/remotes/origin/main` fallback.

**Acceptance:** Unit tests in `src/commit.rs` and `src/docs/preflight.rs` prove: (1) on a clone on `feat/x` with local `main` deleted and `origin/HEAD` unset, `resolve_base_ref(repo, Some("main"))` and `resolve_base_ref(repo, None)` both return `Ok("origin/main")` and `preflight::default_branch(repo)` returns `Some("main")`; (2) `resolve_base_ref(repo, Some("nonexistent"))` returns `Err` even when `origin/HEAD` points to `origin/main`; (3) `resolve_base_ref(repo, Some("-foo"))` returns `Err`. Removing the `refs/remotes/origin/` probe breaks (1); letting `Some("nonexistent")` fall through to `origin/HEAD` breaks (2).

### 3. Wire `commit::resolve_base_ref` into `leaks`, `review`, `collect`, `retro`, and `pack` — high — blocked by: 2
**Source**: §5 FR-3: "Wire every git diff/log/show caller through `commit::resolve_base_ref`: `src/leaks.rs`, `src/review.rs`, `src/collect.rs`, `src/retro.rs`, `src/pack.rs`, `src/checkpoint.rs`, and `src/closeout.rs`".

- `src/leaks.rs:787-804`: in `scan`, for `Input::Base(base)`, resolve `let base_ref = crate::commit::resolve_base_ref(repo, Some(base))?;` and use `format!("{base_ref}...HEAD")`.
- `src/review.rs:241-253` (`numstat`) and `:380-398` (`diff`): add a small helper `resolve_range(cwd: &Path, range: Range) -> Result<String>` that resolves `Range::Base(b)` via `crate::commit::resolve_base_ref(cwd, Some(b))?` to `"{base_ref}...HEAD"`, while leaving `Range::Since(c)` as `"{c}..HEAD"`. Use it in both `numstat` and `diff` (and check any other caller of `range.spec()` inside `src/review.rs` such as `git` invocations).
- `src/collect.rs:98-115`: resolve `base_ref = crate::commit::resolve_base_ref(root, Some(opts.base))?` once (when `opts.since.is_none()` or for `leaks_part`) so `review::numstat`, `review::second`, and `leaks_part` all receive the resolved ref.
- `src/retro.rs:38-47`: in `changed_lines(repo, base)`, resolve `let base_ref = crate::commit::resolve_base_ref(repo, Some(base))?;` and run `git diff --shortstat {base_ref}...HEAD`.
- `src/pack.rs:529-562`: keep `Pack::for_polish`'s single `default_branch: &str` plain-branch parameter so `# Branch` still displays ``Default branch: `{default_branch}` ``, and resolve `let base_ref = crate::commit::resolve_base_ref(repo, Some(default_branch)).unwrap_or_else(|_| default_branch.to_string());` for `git log --oneline {base_ref}..HEAD`.
- Confirm `src/checkpoint.rs:115-121` and `src/closeout.rs:186, :242, :257, :1034-1043, :1212-1217, :1325` pass the plain branch name to `open_pr`, `ci::branch_problem`, `Pack::for_polish`, `collect::collect`, `retro::changed_lines`, and `leaks::scan` (no `"origin/"` leaked into PR base or branch refusal checks).
- Add integration test `base_ref_resolver_handles_origin_only_default_branch_across_leaks_review_retro_and_pack` (e.g. in `tests/review.rs` or `tests/commits.rs`) building a clone on `feat/x` with local `main` deleted and `origin/HEAD` unset, committing a change on `feat/x`, and asserting: `leaks::scan(repo, Input::Base("main"))`, `review::numstat(repo, Range::Base("main"), &[])`, `review::diff(repo, Range::Base("main"), &files)`, `collect::collect` (with `ReviewProvider::None`), `retro::changed_lines(repo, "main")`, and `Pack::for_polish` all succeed — specifically asserting `Pack::for_polish` contains the feature commit's subject (NOT `"(no commits listed)"`) AND ``Default branch: `main` ``.

**Trap**: `Pack::for_polish` (`src/pack.rs:547-551`) silently turns any `git log` error into `"(no commits listed)"`. An assertion that only checks `Pack::for_polish(...).is_ok()` would pass even if `git log` failed — assert the actual commit subject string appears!

**Acceptance:** `base_ref_resolver_handles_origin_only_default_branch_across_leaks_review_retro_and_pack` passes and fails on any caller whose `resolve_base_ref` call is removed (git exits non-zero for unknown revision `main...HEAD`, or `Pack::for_polish` outputs `"(no commits listed)"` instead of the commit subject).

### 4. Verify — medium — blocked by: 1, 2, 3
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 5. Prove — at the checkpoint — blocked by: 4
**Source**: §9 QA plan rows FR-1 and FR-3:
- FR-1: `run project_toml_is_dropped_from_slice_and_polish_lane_commits` (evidence: test transcript)
- FR-3: `run base_ref_resolver_handles_origin_only_default_branch_across_leaks_review_retro_and_pack` (evidence: test transcript)

**Acceptance:** after the checkpoint, both tests run under `cargo test` with full gate green and transcripts recorded in Progress.
