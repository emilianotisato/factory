---
number: 022
slug: guards-a-branch-cant-loosen
title: "Guards a branch can't loosen"
type: feature
created: 2026-11-05
author: Emiliano Tisato
summary: |
  Closes five ways a feature branch under build can loosen or break the rules it is judged by: protects `docs/project.toml` from all workers (including polish), reads `[leaks] allow` from the base ref with a one-line notice if changed on the branch, unifies base-ref resolution across every caller (handling `origin/<default>` when no local default branch exists), catches hyphen-less unambiguous plan IDs (`fr123-7`, `prd025`, `oq4`, `block101`) in strings and comments, and enforces a run-start `authorized_tests` snapshot from `prd.md` frontmatter on pre-existing test diffs at checkpoint and closeout. Verified by a cross-vendor adversarial pass (`claude:opus`, 221 s) 2026-11-26 with all 12 findings corrected inline.
---

# PRD-022 — Guards a branch can't loosen

> **Interviewed 2026-11-26** (upgraded from the 2026-11-05 triage draft). Every `file:line` below was verified against `main` at `004a6a3`, then **re-verified by a cross-vendor adversarial pass (`claude:opus`, 221 s) on 2026-11-26 which caught twelve real breaks**. Each correction is inline and dated.

## 1. Why now

Unattended runs judge a feature branch by rules stored inside the same repository (`docs/project.toml`, `assets/autonomy.md`, `factory leaks`). Today five concrete gaps let a branch under build either weaken those guards or break them on common repo layouts:

- **`docs/project.toml` is unprotected from workers.** `commit::protected_files` (`src/commit.rs:11-13`) still names only `prd.md` and `docs/index.md` (which held the gate in v2). In v3, the verify gate, inner loop, lane count, and `[leaks] allow` all live in `docs/project.toml`. `land::commit_lane` (`src/land.rs:47-89`) stages lanes with `git add -A` and drops only `board` + `protected`, and `Closer::Polish::protected` (`src/closeout.rs:558-563`) protects only `docs/index.md` — so any build, fix, checkpoint, polish, or CI-fix worker can land edits to `docs/project.toml`. (Confirmed: v3 has no preflight worker and `assets/autonomy.md` grants no `inner` write exception, so protecting `docs/project.toml` for every worker needs no carve-out.)
- **`factory leaks --base` reads `[leaks] allow` from the scanned worktree.** `leaks::scan` (`src/leaks.rs:787-788`) calls `allowlist(repo)` on the working tree even when `Input::Base(base)` is given. A branch can add `[leaks] allow` entries in `docs/project.toml` and silence its own hits.
- **Base-ref resolution is split and breaks without a local default branch.** `preflight::default_branch` (`src/docs/preflight.rs:30-50`) strips `origin/` from `refs/remotes/origin/HEAD` (returning `"main"` even when no local `refs/heads/main` exists) and `closeout::default_branch` (`src/closeout.rs:2480-2490`) falls back to literal `"main"`. Callers (`src/checkpoint.rs:115-116`, `src/closeout.rs:186, :242, :257, :1034-1043`, `src/retro.rs:39`, `src/pack.rs:533-561`, `src/leaks.rs:795`, `src/review.rs:74-78, :380`, `src/collect.rs:102-108`) pass that string directly into `git diff <base>...HEAD` / `git log <base>..HEAD`, which fails in clones where the default branch exists only as `origin/<name>`.
- **`factory leaks` misses hyphen-less plan IDs inside strings and comments.** `ids!()` (`src/leaks.rs:41-45`) requires a hyphen after `PRD|FR|AD|CP|OPEN|OQ|DEC|BLOCK|ASK`, so real strings like `fr123-7` (hit on `icg-cloud`) or `prd025` slip past `id-string` and `id-comment` even though `name_hit` (`:198-226`) already catches them in identifiers.
- **No tamper-proof way to authorize legitimate changes to pre-existing tests.** `assets/autonomy.md:62` forbids modifying or deleting tests the run did not write with no structured escape hatch, yet deliberate spec changes routinely need specific existing tests updated (e.g. PRD-026 and PRD-027 each updated pinned tests) — while meanwhile nothing in the driver programmatically blocks an unauthorized worker from weakening an unlisted pre-existing test.

**What this leaves alone.** Building the merge-base tree to execute modified tests red-on-base (tracked in `docs/ideas/2026-11-23-let-a-prd-authorize-a-test-change-provably-without.md` as a future hardening pass), multi-line leak splitters / `--all` tree caching / tuning candidates (cut 2026-11-05), and pre-push guards for the driver's own git calls (cut 2026-11-05; worker `pushurl` is already covered by PRD-027's `run_dir/bin/git` shim).

## 2. Blast radius

- **Existing tests on `commit::protected_files`.** `commit::protected_files(board)` (`src/commit.rs:11-13`) changes return type from `[String; 2]` to `[String; 3]` (adding `"docs/project.toml"`), and `Closer::Polish::protected` (`src/closeout.rs:560`) adds `"docs/project.toml"`. All existing callers (`src/scheduler.rs:1044`, `src/checkpoint.rs:895`, `src/closeout.rs:561`) borrow it as `&[String]` or `.to_vec()`. Verify `src/land.rs:440` and `tests/merge.rs` / `tests/closeout.rs` stay green (and add explicit `docs/project.toml` assertions).
- **Branches that legitimately modify `[leaks] allow`.** `factory leaks --base <ref>` now evaluates hits against `<base>:docs/project.toml`, ignoring branch edits to `[leaks] allow` and printing a one-line notice (`note: [leaks] allow changed on this branch; scanned with <base>'s allowlist`) so the owner knows why a branch allowlist edit did not suppress hits until merged. `factory leaks --all` continues reading the worktree's `docs/project.toml`.
- **PR creation and branch guards vs. git diff refs (`origin/<name>`).** `gh pr create --base <branch>` (`src/closeout.rs:1212-1217`, `src/ci.rs:405-427`), `commit::branch_refusal` (`src/commit.rs:86`), and `ci::branch_problem` (`src/closeout.rs:1325`) require a **plain branch name** (e.g. `"main"`), never `"origin/main"` (*corrected 2026-11-26, adversarial pass: an earlier draft mis-cited `closeout.rs:1034` as PR creation and omitted `ci::branch_problem` at `:1325`*). Only git diff/log/show operations resolve `"main"` → `"origin/main"` when no local `refs/heads/main` exists.
- **Unlisted edits to pre-existing tests during unattended runs.** Any run whose workers remove or modify lines in a pre-existing test file (existed at `<base>`) that is **not** listed in the PRD's snapshotted `authorized_tests` will open a `BLOCK-n` at checkpoint or closeout for the owner to approve (`allow`) or revert. Existing fixture tests in `tests/` (`tests/loop.rs`, `tests/checkpoints.rs`, `tests/closeout.rs`) only create/edit non-test files (`src/slice-NN.txt`, `src/lib.txt`) in their stubs (`tests/stubs/claude:30-32`), so no existing fixture test trips the guard.
- **External contracts.** `factory leaks` exit codes (`0` clean, `1` fix hits, `2` error) stay unchanged; `Report::render` keeps its existing `comment share:` and verdict footer lines; `state.json` gains an additive optional `authorized_tests` array (`skip_serializing_if = "Vec::is_empty"`).

## 3. FR-1 — Protect `docs/project.toml` from every worker

**Today.** `commit::protected_files(board)` (`src/commit.rs:11-13`) returns `[format!("{}/prd.md", board.dir), "docs/index.md".to_string()]`. `Closer::Polish::protected` (`src/closeout.rs:558-563`) returns `vec!["docs/index.md".to_string()]` so polish may edit `prd.md` Status prose. `land::commit_lane` (`src/land.rs:48-89`) drops every path in `protected` from the lane commit and returns dropped paths in `LaneCommit.dropped`; `scheduler.rs:1061` emits `EventKind::WorkerWroteProtectedFile`, while `checkpoint.rs:909` and `closeout.rs:685` log the dropped paths (*corrected 2026-11-26, adversarial pass: an earlier draft named a non-existent `EventKind::ProtectedDropped`*). `assets/autonomy.md:67` lets workers extend gate configuration to cover new code without noting that `docs/project.toml` itself is protected.

**Decided 2026-11-26.**

- Change `commit::protected_files(board: &Status) -> [String; 3]` in `src/commit.rs:11-13` to return:
  - `format!("{}/prd.md", board.dir)`
  - `"docs/index.md".to_string()`
  - `"docs/project.toml".to_string()`
- Change `Closer::Polish::protected` in `src/closeout.rs:560` to return `vec!["docs/index.md".to_string(), "docs/project.toml".to_string()]` so polish retains its narrow permission to update `prd.md` Status prose while `docs/project.toml` stays protected from **all** worker roles (build, merge, checkpoint, fix, polish, CI-fix).
- Update `assets/autonomy.md:67` (*added 2026-11-26, adversarial pass*) to clarify that extending tool config (e.g. `phpstan.neon`, `tsconfig.json`) is allowed, while `docs/project.toml` itself stays protected from all workers (propose any `docs/project.toml` edit in the PR body or block).

**Accepted cost.** A feature that legitimately needs to edit `docs/project.toml` (for example changing `lanes`, `gate`, or `[leaks] allow`) must have the owner make that edit directly on the branch rather than an unattended worker landing it. Allowing a role exception (such as polish or checkpoint) lost because those are the exact workers under pressure to clear a red gate or leaks hit.

**Acceptance.** Unit and integration tests prove: (1) `commit::protected_files(board)` includes `"docs/project.toml"` alongside `prd.md` and `docs/index.md`; (2) `Closer::Polish::protected(board)` includes `"docs/project.toml"` and `"docs/index.md"` (and not `prd.md`); (3) a lane commit where a slice worker edits `docs/project.toml` alongside code drops `docs/project.toml` from the landed commit, leaves `docs/project.toml` untouched in the main tree, records `"docs/project.toml"` in `LaneCommit.dropped`, and emits `EventKind::WorkerWroteProtectedFile` — and a polish worker editing `docs/project.toml` alongside code likewise drops `"docs/project.toml"` in `LaneCommit.dropped`, leaves `docs/project.toml` untouched in the main tree, and logs the dropped path (*corrected 2026-11-26, adversarial pass*).

## 4. FR-2 — Read `[leaks] allow` from `<base>` on branch scans with change notice

**Today.** `leaks::allowlist(root: &Path)` (`src/leaks.rs:444-498`) reads `docs/project.toml` directly from the filesystem worktree, and `leaks::scan(repo, input)` (`src/leaks.rs:787-788`) calls `allowlist(repo)?` for both `Input::All` and `Input::Base(base)`.

**Decided 2026-11-26.**

- Extract `parse_allowlist(label: &str, text: &str) -> Result<Vec<Allow>>` from `allowlist()` (`src/leaks.rs:444-498`) so file-read and git-show paths share identical validation and error messages.
- Add `pub fn allowlist_at_ref(repo: &Path, git_ref: &str) -> Result<Vec<Allow>>`:
  - Verifies `<git_ref>` resolves as a commit via `git rev-parse --verify --quiet <git_ref>^{commit}` (returning `Err` if `<git_ref>` does not exist).
  - Runs `git show <git_ref>:docs/project.toml` in `repo`; if `docs/project.toml` does not exist in that commit's tree (e.g. a repo adopting v3 on the current branch), returns `Ok(Vec::new())`.
  - Otherwise parses the blob via `parse_allowlist(&format!("{git_ref}:docs/project.toml"), &text)`.
- In `leaks::scan(repo, input)` (`src/leaks.rs:787-833`):
  - For `Input::All`: keep `allow = allowlist(repo)?` (no notice).
  - For `Input::Base(base)`:
    - Resolve `base_ref = commit::resolve_base_ref(repo, Some(base))?` (FR-3).
    - Read `allow = allowlist_at_ref(repo, &base_ref)?` and scan `<base_ref>...HEAD` with `allow`.
    - Compare `allow` against the worktree's `allowlist(repo)` (if worktree parse succeeds and differs from `allow`, OR if worktree `docs/project.toml` changed `[leaks] allow` to invalid TOML): set `report.allowlist_note = Some(format!("note: [leaks] allow changed on this branch; scanned with {base_ref}'s allowlist"))`.
- Add `pub allowlist_note: Option<String>` on `leaks::Report` (`src/leaks.rs:517-521`). In `Report::render` (`:554-582`), when `allowlist_note` is `Some(note)`, print `{note}\n` right above `comment share:`. The notice never increments `fix_hits()` or changes `factory leaks`'s exit code.

**Accepted cost.** When a human intentionally adds a new `[leaks] allow` entry on a feature branch (e.g. for a new fixture recording), `factory leaks --base <base>` will still report hits in that fixture until the allowlist change is merged to `<base>`, printing the one-line notice to explain why. Failing closed on `<base>` over trusting the branch won because trusting the branch lets any branch disable leak checks for itself.

**Acceptance.** Integration tests in `tests/leaks.rs` prove: (1) adding a leak hit AND adding its path to `[leaks] allow` in `docs/project.toml` on the feature branch still reports the fix hit under `factory leaks --base main` (exit `1`) and prints `note: [leaks] allow changed on this branch; scanned with main's allowlist`; (2) an allowlist entry already present on `main` suppresses the hit cleanly with zero notice (exit `0`); (3) when `main` has no `docs/project.toml` at all, `factory leaks --base main` succeeds with an empty allowlist (not an error); (4) `factory leaks --all` still uses the worktree's `docs/project.toml` with no notice.

## 5. FR-3 — One shared base-ref resolver across pack, closeout, checkpoint, retro, leaks, review, and collect

**Today.** `preflight::default_branch(root)` (`src/docs/preflight.rs:30-50`) strips `origin/` from `refs/remotes/origin/HEAD` or checks local `refs/heads/{main,master}`. `closeout::default_branch(run, docs)` (`src/closeout.rs:2480-2490`) reads `preflight.json`'s `branch.default` or falls back to `"main"`. Callers (`src/checkpoint.rs:115-116`, `src/closeout.rs:186, :242, :257, :1034-1043`, `src/retro.rs:39`, `src/pack.rs:533-561`, `src/leaks.rs:795`, `src/review.rs:74-78, :241-252, :380`, `src/collect.rs:102-108`) pass that branch name straight to `git diff`/`git log`, failing whenever the repo has `refs/remotes/origin/main` but no local `refs/heads/main`.

**Decided 2026-11-26.**

- Add `pub fn resolve_base_ref(repo: &Path, preferred: Option<&str>) -> Result<String>` in `src/commit.rs` (keeping `preflight::default_branch` / `closeout::default_branch` for the **plain branch name** needed by `gh pr create --base`, `commit::branch_refusal`, and `ci::branch_problem`, while adding `resolve_base_ref` for every git diff/log/show ref):
  - Reject any `preferred` starting with `'-'` (preserving the option-injection guards in `src/leaks.rs:790-794` and `src/cli/review.rs:94, :121` — *corrected 2026-11-26, adversarial pass*).
  - Build candidate list (*corrected 2026-11-26, adversarial pass — `origin/HEAD` applies ONLY when `preferred` is `None`*):
    - When `preferred` is `Some(name)` (non-empty): candidates = `[name]` only (tries `name`, then `origin/<name>`, and errors if neither exists — never silently falls back to `origin/HEAD` or `main`).
    - When `preferred` is `None` (or empty): candidates = `[origin/HEAD stripped of "origin/" (if set), "main", "master"]`.
  - For each candidate `name`:
    - If `git rev-parse --verify --quiet <name>^{commit}` succeeds → return `Ok(name)`.
    - Else if `!name.starts_with("origin/")` and `git rev-parse --verify --quiet refs/remotes/origin/<name>^{commit}` succeeds → return `Ok(format!("origin/{name}"))`.
  - If none resolves → return `Err(Error::Msg(...))`.
- Update `preflight::default_branch` (`src/docs/preflight.rs:30-50`) so when `origin/HEAD` is unset and no local `main`/`master` exists, it also checks `refs/remotes/origin/main` and `refs/remotes/origin/master` (returning the plain branch name `"main"`/`"master"`).
- Wire every git diff/log/show caller through `commit::resolve_base_ref`:
  - `src/leaks.rs` (`scan` for `Input::Base`)
  - `src/review.rs` (`Range::Base` in `numstat` at `:241` and `diff` at `:380`)
  - `src/collect.rs` (`collect` resolving `opts.base` once for both `leaks` and `review`)
  - `src/retro.rs` (`changed_lines` at `:39`, called from `src/closeout.rs:1034` — *added 2026-11-26, adversarial pass*)
  - `src/pack.rs` (`Pack::for_polish` keeps its single `default_branch: &str` plain-branch parameter so `# Branch` still displays the plain branch name at `:560`, and resolves `base_ref = commit::resolve_base_ref(repo, Some(default_branch))` internally for `git log --oneline {base_ref}..HEAD` at `:544` — *clarified 2026-11-26, adversarial pass*)
  - `src/checkpoint.rs` (`leaks::scan` via `Input::Base(&base)`) and `src/closeout.rs` (passing plain branch name to `open_pr` at `:1212`, `ci::branch_problem` at `:1325`, `Pack::for_polish`, `collect::collect`, `retro::changed_lines`, and `leaks::scan`, each of which resolves via `resolve_base_ref` for git commands).

**Accepted cost.** When a caller passes an explicit `--base <name>` that exists only as `origin/<name>`, `resolve_base_ref` transparently falls back to `origin/<name>` rather than failing; if a user genuinely mistyped a local branch name that happens to exist on `origin/`, it diffs against `origin/<name>` (which is almost always what they wanted). Never falling back to `origin/HEAD` or `"main"` when an explicit `--base <typo>` was passed prevents silent wrong-base diffs.

**Acceptance.** Unit and integration tests prove (*corrected 2026-11-26, adversarial pass*): (1) on a clone that switches to `feat/x` first, deletes local `main` (`git branch -D main`), and unsets `origin/HEAD` (`git remote set-head origin -d`), `commit::resolve_base_ref(repo, Some("main"))` and `commit::resolve_base_ref(repo, None)` both return `Ok("origin/main")`, and `preflight::default_branch(repo)` returns `Some("main")`; (2) on that remote-only-main repo, `factory leaks --base main`, `factory review second --diff main`, `factory review collect --base main`, `retro::changed_lines(repo, "main")`, and `Pack::for_polish` all succeed — and `Pack::for_polish`'s `# Branch` output lists the feature branch's commit subjects (not `(no commits listed)`) while showing ``Default branch: `main` ``; (3) `commit::resolve_base_ref(repo, Some("nonexistent"))` returns `Err` even when `origin/HEAD` points to `origin/main`, and rejects `-foo` as an option.

## 6. FR-4 — Leaks plan-id rule catches hyphen-less unambiguous prefixes (`fr123-7`, `prd025`)

**Today.** `ids!()` in `src/leaks.rs:41-45` is:
```rust
r"\b(?:PRD|FR|AD|CP|OPEN|OQ|DEC|BLOCK|ASK)-\d+|\bslice[- ]?\d+"
```
It requires a hyphen after `PRD`, `FR`, `OQ`, and `BLOCK`, so strings and comments containing `fr123-7`, `fr123`, `prd025`, `oq4`, or `block101` pass `id-string` and `id-comment` undetected.

**Decided 2026-11-26.**

- Update `ids!()` (`src/leaks.rs:41-45`) to allow an optional hyphen or underscore for the four unambiguous prefixes (`PRD`, `FR`, `OQ`, `BLOCK`) while keeping the hyphen required for the five short/ambiguous prefixes (`AD`, `CP`, `OPEN`, `DEC`, `ASK`) and keeping `\bslice[- ]?\d+` unchanged:
  ```rust
  r"\b(?:PRD|FR|OQ|BLOCK)[-_]?\d+|\b(?:AD|CP|OPEN|DEC|ASK)-\d+|\bslice[- ]?\d+"
  ```
- Update `SEMVER` (`src/leaks.rs:159`) to `r"(?i)\b(?:PRD|FR|AD|CP|OPEN|OQ|DEC|BLOCK|ASK|slice)[-_ ]?\d+\.\d+\.\d+"` so semver strings like `block1.2.3` or `slice-1.0.4` stay quiet.

**Accepted cost.** A string or comment containing an intentional non-plan token like `"fr24"` (e.g. a French locale+number code) or `"block1"` in prose would trigger `id-string` / `id-comment` and need a `[leaks] allow` entry in `docs/project.toml`. Keeping `AD`, `CP`, `OPEN`, `DEC`, and `ASK` hyphen-required prevents common false positives like `cp1252`, `dec2026`, `ad1`, and `open2`.

**Acceptance.** Unit tests in `src/leaks.rs` prove: (1) `id-string` and `id-comment` catch `"fr123-7"`, `"fr123"`, `"FR_4"`, `"prd025"`, `"oq12"`, and `"block101"`; (2) `id-string` and `id-comment` do **not** flag `"cp1252"`, `"dec2026"`, `"ad1"`, `"open2"`, `"ask1"`, or semver `"block1.2.3"`; (3) all existing `src/leaks.rs` and `tests/leaks.rs` tests stay green.

## 7. FR-5 — Run-start `authorized_tests` snapshot and pre-existing test diff guard

**Today.** `assets/autonomy.md:62` puts *"skip, delete or weaken a test this run did not write"* under **Never, to get green** with no escape hatch, and nothing in `src/checkpoint.rs` or `src/closeout.rs` programmatically detects removed or modified lines in pre-existing test files. `src/docs/frontmatter.rs:65-119` already parses YAML flow arrays (`key: [a, b]`). `Closer::Polish` (`src/closeout.rs:557-560`) allows polish to land edits to `prd.md` (intended for Status prose only).

**Decided 2026-11-26.**

- **Frontmatter declaration (`src/docs/check.rs`):**
  - Support optional `authorized_tests` key in `prd.md` frontmatter using the existing YAML flow-array syntax (`authorized_tests: ["tests/knobs.rs", "tests/docs_log.rs::a_log_without_covers_keeps_the_high_water_rule"]`), already parsed by `frontmatter::parse` (`src/docs/frontmatter.rs:65-119`).
  - Validate `authorized_tests` in `src/docs/check.rs` (`prd_keys`): if present, it must be a list of non-empty strings (no leading `/`, no `..` traversal, no globs `*`/`?`/`[`), else emit `E-FM-INVALID`.
- **Tamper-proof snapshot at run start (`src/run.rs`, `src/cli/run.rs`, `src/closeout.rs`):**
  - Add `#[serde(default, skip_serializing_if = "Vec::is_empty")] pub authorized_tests: Vec<String>` on `RunState` (`src/run.rs:95-135`), plus `Run::set_authorized_tests` / `Run::add_authorized_tests`.
  - When `factory run` creates a new run (`Run::create` at `src/run.rs:240`, called from **both** `src/cli/run.rs:141` on the ask path and `src/cli/run.rs:174` on the immediate-start path — *corrected 2026-11-26, adversarial pass*), read `authorized_tests` from `docs/features/NNN-slug/prd.md` frontmatter and persist it onto `RunState` in `~/.local/share/factory/runs/<id>/state.json` (outside the repo, before any worker spawns).
  - In `Closer::Polish` closeout (`src/closeout.rs`, before landing polish's lane commit — *added 2026-11-26, adversarial pass*): if the polish lane modified `prd.md` and its parsed `authorized_tests` frontmatter differs from `RunState.authorized_tests`, refuse to land that `prd.md` change (treat `prd.md` as dropped or open `CloseoutBlocked`) so a polish worker cannot rewrite `prd.md`'s `authorized_tests` for a subsequent `factory run`.
- **Pre-existing test diff checker (`src/testguard.rs`, exported from `src/lib.rs`):**
  - `pub fn check(repo: &Path, base_ref: &str, authorized: &[String]) -> Result<Vec<Violation>>`:
    - Runs `git diff -U0 --diff-filter=MD --no-renames <base_ref>...HEAD` (pre-existing files only: newly added test files `A` are always allowed).
    - Filters to test files via `is_test_path(path)`: path starts with `tests/`, `test/`, `__tests__/`, or `spec/`, or contains `/tests/` / `/__tests__/`, or filename ends with `_test.<ext>`, `.test.<ext>`, `.spec.<ext>`, `Test.<ext>`, or `Tests.<ext>`.
    - Parses `-U0` hunks:
      - A **deleted** (`D`) pre-existing test file is a `Violation` unless `authorized` contains the exact file path (`<path>`).
      - A **modified** (`M`) pre-existing test file with **0 removed (`-`) lines** (purely additive new tests) is always allowed.
      - For each hunk in a modified pre-existing test file that has **≥ 1 removed (`-`) line**:
        - Allowed if `authorized` contains the exact file path `<path>`.
        - Allowed if `authorized` contains `"<path>::<test_name>"` and either the hunk's git header context (`@@ ... @@ <ctx>`) contains `<test_name>` OR scanning upward in the `<base_ref>:<path>` file from the hunk's old start line finds `<test_name>` in the enclosing function declaration (`fn <name>`, `function <name>`, `def <name>`, `test("<name>"`, `it("<name>"`, `func <name>`).
        - Otherwise record a `Violation { path, test_name: Option<String>, deleted: bool }`.
- **Checkpoint and closeout enforcement (`src/checkpoint.rs`, `src/closeout.rs`, `src/cli/answer.rs`, `assets/autonomy.md`):**
  - At **checkpoint** (`src/checkpoint.rs`, after gate green and before closing/landing) and at **closeout** (`src/closeout.rs`, after polish lands and before `ship`): run `testguard::check(&lane_dir, &base_ref, &run.state().authorized_tests)`.
  - If violations are non-empty: open a `BLOCK-n` in `log.jsonl` (emitting `CheckpointBlocked` / `CloseoutBlocked`) whose title starts with `TESTGUARD_BLOCK_PREFIX` (`"unauthorized test change:"`) and whose body lists each offending `path` (and enclosing test name when found), with options: (1) `allow` — add these paths to `RunState.authorized_tests` for the rest of this run, (2) revert the unlisted test edits and answer to retry.
  - When the owner answers `"allow"` (case-insensitive) to a `TESTGUARD_BLOCK_PREFIX` block (`src/cli/answer.rs` or on resume before re-running `testguard::check` — *corrected 2026-11-26, adversarial pass*), the violated paths from that block are persisted onto `RunState.authorized_tests` so every later checkpoint and closeout in the same run honors the approval without re-blocking on those files, while any *new* unlisted test file edited later still blocks.
  - Update `assets/autonomy.md` (`Never, to get green`): `"skip, delete or weaken a test this run did not write — unless the PRD's frontmatter `authorized_tests` list (snapshotted at run start) names it"`.

**Accepted cost.** Because this is the lightweight guard (no base-tree build to run changed tests red-on-base), a worker modifying a test that *is* listed in `authorized_tests` is trusted not to weaken it beyond what the PRD ordered (still reviewed by local review, second opinion, and owner PR review). Inline `#[cfg(test)] mod tests` blocks inside non-test source files (like `src/config.rs`) are not matched by `is_test_path` unless their path matches the test-path patterns, keeping false positives on production code refactoring at zero.

**Acceptance.** Unit and integration tests prove: (1) `docs check` accepts valid flow-array `authorized_tests` (`["tests/a.rs", "tests/b.rs::fn_name"]`) and rejects invalid entries (`../x`, `/abs`, `tests/*.rs`, non-list values); (2) `factory run` snapshots `authorized_tests` from `prd.md` into `state.json` at run start on both immediate-start and ask-parked runs, and a polish worker cannot land an edit that changes `prd.md`'s `authorized_tests`; (3) `testguard::check` allows newly added test files and purely additive hunks (0 `-` lines) in pre-existing test files, allows removed lines when the file or `path::test_name` is in `authorized_tests`, and returns a `Violation` when a pre-existing test file has removed lines or is deleted without authorization; (4) a checkpoint run where a slice worker modifies an unlisted pre-existing test file opens a `BLOCK-n` (`CheckpointBlocked`), answering `"allow"` appends that path to `RunState.authorized_tests` so both the resumed checkpoint and closeout pass without re-blocking after subsequent commits — while listing that file/test in `prd.md`'s `authorized_tests` lets the checkpoint pass with zero blocks.

## 8. Decisions log

No `OPEN-n`. Every fork was settled on 2026-11-26 and lives in its FR:

- Protect `docs/project.toml` for all workers including `Closer::Polish` with no exception, and clarify `assets/autonomy.md:67` (§3).
- `[leaks] allow` read from `<base>` on `Input::Base` with a one-line notice when the branch modified `[leaks] allow` (§4).
- Unified `commit::resolve_base_ref` for git diff/log/show refs (including `retro::changed_lines` and `Pack::for_polish`), while keeping plain branch names for `gh pr create --base`, `commit::branch_refusal`, and `ci::branch_problem` (§5).
- Hyphen-less plan-id matching in strings/comments enabled for the four unambiguous prefixes (`PRD`, `FR`, `OQ`, `BLOCK`; `slice` was already hyphen-optional `[- ]?`), keeping hyphen required for `AD`, `CP`, `OPEN`, `DEC`, `ASK` (§6).
- Lightweight `authorized_tests` snapshotted from `prd.md` frontmatter into `RunState` at run start, protected from polish frontmatter edits, enforced via `-U0` diff check at checkpoint and closeout, and extended on owner `"allow"` for the rest of the run — leaving red-on-base base-tree builds as a future follow-up (§7).

## 9. QA plan

**Gate**: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test && claude plugin validate mods/cockpit && claude plugin test mods/cockpit` *(declared in `docs/project.toml`)*
**Surface**: `factory` CLI (`run`, `answer`, `leaks --base`, `leaks --all`, `review second`, `review collect`, `docs check`, `docs preflight`), lane landing (`land::commit_lane`), checkpoint & closeout drivers, and `state.json`. No GUI: every FR is proven below it.
**Harness**: `tests/harness/mod.rs` (`Fixture`), `src/commit.rs`, `src/land.rs`, `src/leaks.rs`, `src/testguard.rs`, `src/docs/frontmatter.rs`, `tests/leaks.rs`, `tests/review.rs`, `tests/checkpoints.rs`, `tests/closeout.rs`.
**Seed state**: fixture repo with `docs/project.toml`; fixture repo cloned onto `feat/x` with local `main` deleted and `origin/HEAD` unset; fixture repo with pre-existing `tests/existing_test.rs` and `prd.md` with and without `authorized_tests`.

| FR | Proof | Evidence | Human? |
|---|---|---|---|
| FR-1 | run `project_toml_is_dropped_from_slice_and_polish_lane_commits` | test transcript | no |
| FR-2 | run `leaks_base_uses_base_allowlist_and_notes_branch_edits` | test transcript | no |
| FR-3 | run `base_ref_resolver_handles_origin_only_default_branch_across_leaks_review_retro_and_pack` | test transcript | no |
| FR-4 | run `leaks_catches_hyphenless_unambiguous_plan_ids_without_ambiguous_false_positives` | test transcript | no |
| FR-5 | run `authorized_tests_snapshot_blocks_unlisted_test_edits_and_persists_allow_for_run` | test transcript | no |

No row needs a human, so no `qa-guide.md` is generated.

## 10. Slices

_(Not sliced yet. Run `/slice`.)_

## 11. Non-goals

- **YAML block-list syntax (`authorized_tests:\n  - ...`) in `frontmatter::parse`.** Cut 2026-11-26: existing flow-array syntax (`authorized_tests: ["tests/a.rs", "tests/b.rs::fn_name"]`) already parses cleanly with zero frontmatter parser changes. Revisit if long `authorized_tests` lists become awkward on one line.
- **Building the merge base to run modified tests red-on-base.** Tracked in `docs/ideas/2026-11-23-let-a-prd-authorize-a-test-change-provably-without.md` (decided 2026-11-26): lightweight snapshot + `-U0` diff guard ships here; red-on-base execution across stacks (Rust/PHP/JS) stays its own follow-up. Revisit if an agent weakens an authorized test while staying within the listed `authorized_tests`.
- **Detecting inline `#[cfg(test)]` edits inside production source files (`src/*.rs`).** Not handled (2026-11-26): `testguard` matches dedicated test paths/filenames (`tests/**`, `*_test.*`, `*Test.*`, etc.) to avoid false blocks during production refactors. Revisit if a worker weakens an inline unit test in `src/` without authorization.
- **Leaks multi-line splitter, `--all` scan cost/caching, and tuning candidates (`C99`, `Dec-2026`).** Dropped at 2026-11-05 triage: big work, low hit rate. Revisit if `--all` scan time or those false positives recur across multiple repos.
- **Pre-push guard for the driver's own git calls.** Dropped at 2026-11-05 triage (`docs/decisions.md#dec-1` and `commit::branch_refusal` guard driver commits; PRD-027's `run_dir/bin/git` shim already blocks worker `pushurl` pushes).

## 12. Verification notes

Checked against `main` at `004a6a3`, 2026-11-26, then re-verified the same day by a cross-vendor adversarial pass (`factory review second --doc`, `claude:opus` at `high` effort, 221 s, log `~/.local/share/factory/reviews/20261126-141136`). Twelve findings were caught and corrected inline above. **Re-verify at slice entry.**

- `src/commit.rs:11-13` (`protected_files` returns `[String; 2]`: `prd.md`, `docs/index.md`), `:80-102` (`head_branch`, `branch_refusal`), `:199-227` (`git`, `git_code`, `run_git`).
- `src/land.rs:31-95` (`commit_lane` stages with `git add -A`, drops `board` + `protected`, returns `LaneCommit.dropped`).
- `src/closeout.rs:186, :242, :257, :1034-1043` (`retro::changed_lines` and ship-time `leaks::scan`), `:557-563` (`Closer::Polish::protected` protects only `docs/index.md`; `Closer::CiFix` uses `commit::protected_files`), `:685` (logs dropped files), `:1212-1217` + `src/ci.rs:405-427` (`gh pr create --base`), `:1323-1325` (`ci::branch_problem`), `:2480-2490` (`default_branch` falls back to `"main"` without checking `origin/`).
- `src/retro.rs:39` (`changed_lines` runs `git diff --shortstat {base}...HEAD`).
- `src/docs/preflight.rs:30-50` (`default_branch` strips `origin/` from `refs/remotes/origin/HEAD`, checks only `refs/heads/{main,master}`).
- `src/pack.rs:533-561` (`Pack::for_polish` takes one `default_branch` param for `git log --oneline {default_branch}..HEAD` at `:544`, falling back to `(no commits listed)` at `:547-551`, and prints `# Branch` at `:560-561`).
- `src/checkpoint.rs:115-121` (calls `closeout::default_branch` then `leaks::scan(Input::Base(&base))`), `:895` (`commit::protected_files`), `:909` (logs dropped files).
- `src/scheduler.rs:1044` (`commit::protected_files`), `:1061` (`EventKind::WorkerWroteProtectedFile`).
- `src/leaks.rs:41-45` (`ids!()` macro requiring `-` after `PRD|FR|AD|CP|OPEN|OQ|DEC|BLOCK|ASK`), `:150-160` (`QUIET`, `SEMVER`), `:444-498` (`allowlist` reading worktree `docs/project.toml`), `:517-582` (`Report`, `Report::render`), `:787-833` (`scan` calling `allowlist(repo)` on `Input::Base`).
- `src/review.rs:67-88, :241-252, :380` (`Range::Base`, `numstat`, `diff`); `src/cli/review.rs:94, :121` (option-injection guards); `src/collect.rs:102-108` (`collect`).
- `src/docs/frontmatter.rs:65-155` (`parse` handles flow arrays `[a, b]` and `|` block scalars, not `- item` block lists).
- `src/docs/check.rs:384-435` (`prd_keys` validates required frontmatter keys, allows extra keys).
- `src/run.rs:95-135` (`RunState`), `:240` (`Run::create`); `src/cli/run.rs:141, :174` (both `Run::create` call sites).
- `assets/autonomy.md:60-69` (`Never, to get green`: `:62` test rule, `:67` gate configuration extension).
