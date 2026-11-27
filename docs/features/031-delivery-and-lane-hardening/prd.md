---
number: 031
slug: delivery-and-lane-hardening
title: "Delivery and lane hardening"
type: feature
created: 2026-11-27
author: Emiliano Tisato
authorized_tests: ["mods/cockpit/tests/cockpit.test.tsx"]
summary: |
  Hardens unattended runs and daily CLI ergonomics: resolves and surfaces lane env (including absolute `CARGO_TARGET_DIR`) in all worker packs (`export K=V;`) and warns in `factory lanes up --prove` when `$SHELL -lic` clobbers declared lane vars; defers checkpoint Prove rows for FRs with pending/building slices (scanning Prove + pre-Depends-on Source), lets clean-tree `run` Prove rows cite the full green gate log, and adds the two UI Prove rules to `skills/slice/SKILL.md`; clears stale `reviewed_head` on `docs set reviewed=<date>` (matching `^reviewed(_head)? =` in `settled::last_reviewed`, resolving `reviewed_head=HEAD` against cwd's repo, and reporting `closeout::ship` write errors), clarifies `docs log add` main-checkout output and worker contracts; adds `factory docs check [FEATURE]`, shared `ctx::def` linguist-generated filtering, `-c core.attributesFile=` isolation in `review::check_attr_src`, a token-free squash-merge skip-CI reminder in `closeout` and `skills/deliver/SKILL.md`, and bumps `0.19.0` → `0.20.0`.
---

# PRD-031 — Delivery and lane hardening

> **Interviewed 2026-11-27** (verified against `main` at `41e4517`). Revised 2026-11-27 after cross-vendor adversarial pass (`claude:opus`, 406 s, log `20261127-211132`) which surfaced 14 concrete catches, all folded in below.

## 1. Why now

Four real delivery frictions hit repeatedly across PRD-029, PRD-030, and PRD-016:

- **Interactive/login shells clobber lane `CARGO_TARGET_DIR` (hit in PRD-029 slice-03 and PRD-016 slice-02).** `docs/reference/lanes/lane-1..4.md` already declare `env: [CARGO_TARGET_DIR=target/lane-N]` and `src/worker.rs:586-588` passes them to the spawned worker, *but* workers like `cursor-agent` execute shell commands through interactive/login `bash` whose profile unconditionally exports a host-level `CARGO_TARGET_DIR`, causing parallel lanes to overwrite each other's test binaries.
- **Checkpoints block on partially-built FRs and waste turns rerunning tests the gate just ran (hit in PRD-029 `BLOCK-1`, PRD-015, PRD-024).** `pack::handed_qa` (`src/pack.rs:749-771`) demands evidence for any `FR-n` cited by a covered slice even when a `pending` or `building` slice still owns the rest of that FR, and `assets/contract-checkpoint.md:15` forces every `run <test>` row to rerun tests that the checkpoint's green gate transcript already ran. Meanwhile, two proven UI Prove rules (empty submit + 375 px narrow-screen form drives; option-distinguishing seeds) are still only in an open idea file rather than `skills/slice/SKILL.md`.
- **Attended polish stays pinned to a stale `reviewed_head`, and lane workers are repeatedly confused by `docs log add` (hit in PRD-029 polish and seven delivered features' retros).** Running `factory docs set <NNN> reviewed=<date>` leaves an earlier run's `reviewed_head` intact in `state.toml` (`src/docs/state.rs:184-189`), and `src/settled.rs:42` only matches `^reviewed =` (missing same-day `reviewed_head` removal); `src/closeout.rs:1080` silently discards `docs.set(..., "reviewed_head")` errors with `let _ =`; and `factory docs log add` from a lane prints only `<path> updated` to stderr (`src/cli/docs.rs:432-434`) without explaining that the driver owns and commits the log in the main checkout.
- **CLI, code-lookup, and squash-merge papercuts.** `factory docs check` (`src/cli/docs.rs:65`) cannot scope to a single feature; `ctx::def::find` (`src/ctx/def.rs:152`) still searches `linguist-vendored`/`linguist-generated` files that `ctx::refs::find` (`src/ctx/refs.rs:61-77`) skips; `review::check_attr_src` (`src/review.rs:382-394`) does not isolate `core.attributesFile` / `~/.config/git/attributes` during `leaks` scans; and on PR #31 (`ci = local`), GitHub's default squash-merge body copied `* docs(016): ship/owner review [skip ci]` onto `main`, suppressing both `ci` and `Edge` workflows.

**What this leaves alone.** Editing the user's personal shell profile (`~/.bashrc`), adding plan-title/phase-label heuristics to `factory leaks` (item 3 of `2026-11-20-three-small-cli-papercuts`, too false-positive prone), and changing `commit::cut` (item 1 of that same idea — already word-boundary-safe at `src/commit.rs:241-266`).

## 2. Blast radius

- **`mods/cockpit/tests/cockpit.test.tsx` (authorized) and `0.19.0` → `0.20.0` version bump.** Editing `skills/slice/SKILL.md` (FR-2) and `skills/deliver/SKILL.md` (FR-4) changes the embedded skill bundle hash. An earlier draft proposed relocking at `0.19.0`, but the 2026-11-27 adversarial pass caught that same-version relocking runs `git ls-remote --tags origin` (`src/skills.rs:199`) which fails offline or if `v0.19.0` gets tagged before the run. Bumping `0.19.0` → `0.20.0` across `Cargo.toml`, `Cargo.lock`, `skills/bundle.lock` (via `FACTORY_BUNDLE_WRITE=1 cargo test --test skills_bundle`), `mods/cockpit/.claude-plugin/plugin.json`, `mods/cockpit/hooks/register.tsx`, and `mods/cockpit/tests/cockpit.test.tsx` makes relocking offline-safe (`order == Greater` in `src/skills.rs:224`). Both skill edits and the bundle relock land in a single slice (or chained slices) so `skills/bundle.lock` cannot conflict at merge.
- **`tests/review.rs:1152-1164` (`last_reviewed_commit_is_the_one_that_set_reviewed`) and `tests/review_claims_and_since.rs:363`.** Must stay untouched and green: `docs set reviewed=<date>` must NOT stamp `reviewed_head = HEAD` implicitly when `reviewed_head` was unset (that would point at the pre-commit parent instead of the review commit), and `skills/polish/SKILL.md` must keep `reviewed_head` occurring exactly once. Clearing a *stale* `reviewed_head` on `reviewed=<date>` (unless `reviewed_head=` is also passed) + matching `^reviewed(_head)? =` in `src/settled.rs:42` + supporting explicit `reviewed_head=HEAD` (resolved against cwd's repo) keeps every existing test green untouched.
- **`review::check_attr_src` (`src/review.rs:364-415`) is shared across `leaks`, `ctx refs`, `ctx def`, and `review collect`.** Passing `-c core.attributesFile=` (WITHOUT `GIT_CONFIG_GLOBAL=/dev/null`, which would drop CI `safe.directory`!) disables both explicit `core.attributesFile` and git's default `~/.config/git/attributes` across all four callers while preserving `safe.directory` and committed `.gitattributes`.

## 3. FR-1 — Lane env isolation against login shells (`CARGO_TARGET_DIR` & `lanes up --prove`)

**Today.**
- `src/lanes.rs:94-98`: `Lane::env(&self)` returns `FACTORY_LANE=<n>` plus raw `(k, v)` pairs from `docs/reference/lanes/lane-N.md` (e.g., `CARGO_TARGET_DIR=target/lane-2` as a relative path).
- `src/worker.rs:586-588`: sets `lane.env()` on the spawned worker process, but when the worker invokes an interactive/login shell (`bash -ic` / `bash -lic`), user startup files overwrite `CARGO_TARGET_DIR` back to a global host value.
- `src/pack.rs:302, 437, 520-545, 682`: slice, merge, and ci-fix packs print `# Inner loop` with the bare command (`cargo fmt && cargo clippy ...`); fix and checkpoint packs carry `# Gate transcript` and no `# Lane env` section; none of them surface the lane's resolved env.
- `src/cli/lanes.rs:144-214`: `factory lanes up --prove` runs the inner loop via non-interactive `sh -c`, so it never detects that an interactive/login user shell clobbers a declared lane env var.

**Decided 2026-11-27.**

- **Resolve relative `CARGO_TARGET_DIR` to an absolute path inside the lane (`src/lanes.rs`):**
  - Add `pub fn env_for(&self, lane_dir: &Path) -> Vec<(String, String)>`: same as `self.env()`, except when a key is `CARGO_TARGET_DIR` and its value is a relative path, resolve it to `std::path::absolute(lane_dir.join(val))` (falling back to `lane_dir.join(val)`) so `cd` into a subdirectory or nested command still builds inside the lane's own target directory.
  - Use `lane_def.env_for(&lane_dir)` everywhere a lane env is passed to a gate or worker: `src/scheduler.rs:769`, `src/checkpoint.rs:119` (`gate_once`), `src/checkpoint.rs:329` (checkpoint worker), `src/checkpoint.rs:895` (fix worker), `src/closeout.rs` (polish/ci-fix workers), and `src/cli/lanes.rs:174` (`prove`).
- **Prefix and surface lane env in worker packs (`src/pack.rs`, `src/scheduler.rs`, `src/checkpoint.rs`, `src/closeout.rs`):**
  - Add pure helper `pub fn env_prefixed(env: &[(String, String)], cmd: &str) -> String` in `src/pack.rs`: when `env` contains lane-declared vars (excluding `FACTORY_LANE`), ALWAYS emit `export K1=<quoted> K2=<quoted>; <cmd>` (shell-quoting each value via `paths::shell_word`) so every command in chained pipelines (`&&`, `||`, `;`, `|`, subshells) inherits the lane vars without brittle substring heuristics. When `env` has no custom vars beyond `FACTORY_LANE`, return `cmd.to_string()` unchanged.
  - Thread `lane_env: &[(String, String)]` into slice, fix, merge, checkpoint, and ci-fix packs (keeping existing `Pack::for_slice` / `for_slice_with` / `for_fix` / `for_merge` / `for_checkpoint` / `for_ci_fix` public signatures working as thin wrappers passing `&[]` so existing callers and unit tests stay green):
    - Everywhere a pack renders `# Inner loop` (`for_slice`, `for_merge`, `for_ci_fix`), emit `env_prefixed(lane_env, inner.trim())`.
    - In every worker pack (including `for_fix` and `for_checkpoint`, which have no `# Inner loop` section), add a `# Lane env` section (omitted when `lane_env` has no custom vars beyond `FACTORY_LANE`) listing the exact `export K=<quoted>` command and noting that workers must keep those exports active when running build/test commands in interactive shells.
- **Detect shell-profile clobbering in `factory lanes up --prove` (`src/cli/lanes.rs`):**
  - Add helper `pub fn detect_shell_clobber(shell: &Path, env: &[(String, String)]) -> Vec<(String, String, String)>`:
    - Only runs for declared lane vars beyond `FACTORY_LANE`, and only when `shell` exists and its basename is a Bourne/POSIX interactive shell (`sh`, `bash`, `zsh`, `dash`, `ash`, `ksh`) or an explicit test stub path set via `FACTORY_PROVE_SHELL`.
    - Spawns `<shell> -lic 'printf ...'` with `.stdin(Stdio::null())`, `.stdout(Stdio::piped())`, `.stderr(Stdio::null())`, passing `(k, v)` in the child env, and enforces a **3-second timeout** (killing and reaping the child if a slow/interactive shell framework stalls — returning empty on timeout/spawn error instead of hanging).
    - Compares each printed value inside the shell against `v` and returns `(key, want, got)` for every clobbered variable.
  - In `factory lanes up --prove`: unless `FACTORY_PROVE_SHELL` is `"off"`, resolve shell from `FACTORY_PROVE_SHELL` (when set and non-empty) else `$SHELL` (skipping cleanly when unset), run `detect_shell_clobber` once against lane 1's resolved env (`lane.env_for(&dir)`), and for each clobbered var print `! shell <shell> -lic overwrites <K> (<want> → <got>) — guard it in your shell profile with export <K>="${<K>:-<got>}"` (warning only, exit code unchanged).

**Accepted cost.** Prefixing `# Inner loop` with `export K=V;` and adding `# Lane env` adds ~100 bytes to worker packs. Running `$SHELL -lic` with `stdin=null` and a 3 s timeout adds a fraction of a second (up to 3 s worst case on heavy zsh frameworks) to `factory lanes up --prove`; clobber detection is a warning rather than a hard failure so owners who cannot immediately edit a corporate profile can still run lanes (since the pack exports protect workers).

**Acceptance.** Unit & integration tests prove: (1) `Lane::env_for` resolves relative `CARGO_TARGET_DIR=target/lane-2` to `<lane_dir>/target/lane-2` while preserving absolute paths and other env vars; (2) `pack::env_prefixed` always emits `export CARGO_TARGET_DIR=<quoted-abs>; <cmd>`, slice/merge/ci-fix packs prefix `# Inner loop`, and slice/fix/checkpoint packs include `# Lane env` when custom lane env is supplied; (3) `detect_shell_clobber` catches a stub shell that overwrites `CARGO_TARGET_DIR`, returns empty when preserved, and terminates cleanly on a hanging stub without deadlocking; (4) `factory lanes up --prove` with `FACTORY_PROVE_SHELL=<clobber-stub>` prints the warning line while exiting `0`.

## 4. FR-2 — Checkpoint Prove deferral, gate-transcript reuse, and UI Prove rules

**Today.**
- `src/pack.rs:749-771`: `handed_qa(repo, board, covers, prd_text)` collects every `FR-n` named in the covered slices' `Prove` sections without checking whether any slice whose status is not `"done"` (`pending`, `building`, `blocked`) also owns that `FR-n`.
- `assets/contract-checkpoint.md:15`: requires driving every `run` row from scratch and placing files in `$FACTORY_EVIDENCE/cp-{n}/<key>/`, even when step 1 made no edits and the checkpoint's own green gate log file already ran and passed that exact test command. (Note: `# Gate transcript` in the pack only carries the last 200 lines at `src/pack.rs:80, :525`, while its header comment names the full gate log path on disk.)
- `skills/slice/SKILL.md:58`: does not yet capture the two UI Prove rules from `docs/ideas/2026-11-20-slice-deliver-three-rules-for-prove-rows-and-the-l.md`.

**Decided 2026-11-27.**

- **Defer FR rows still owned by incomplete slices (`src/pack.rs:749-771`):**
  - In `handed_qa`, when `!board.all_done`: scan every slice in `board.slices` whose `status.as_deref() != Some("done")`, collecting every `FR-n` cited in:
    1. its `Prove` section (`prove_section(&slice_text)`), AND
    2. the `**Source**:` line **strictly before** `**Depends on**` (caught in 2026-11-27 adversarial pass: scanning the whole line would pick up dependency rationale like `Depends on: Slice 02 (so FR-4 ...)` and falsely defer an already-finished FR!).
  - Exclude those incomplete `FR-n` ids from `named`, and append a one-line note to the rendered `# QA rows` text naming any deferred FRs (e.g., `(deferred until remaining slice(s) finish: FR-3)`).
  - Once `board.all_done` is true (the final checkpoint), every row in the QA table is handed as before, so nothing can ever be deferred forever. (Note: checkpoints run exclusively with zero concurrent builders in flight, so `board.slices` statuses do not change between checkpoint spawn and `orphan_record_holds` resume at `src/checkpoint.rs:1423`.)
- **Let clean-tree `run` Prove rows cite the full green gate log (`assets/contract-checkpoint.md:15-17`):**
  - Update step 2 (`**Prove**`) of `assets/contract-checkpoint.md`: when **step 1 (Leaks) made zero code edits** AND a `run` row names a test that the checkpoint's green gate already executed and passed (verified by checking the **full gate log file** at the path named in `# Gate transcript`'s header comment, not only the 200-line tail!), the checkpoint worker may cite the gate log instead of re-running that test — writing a short citation file (e.g., `gate-transcript.txt` naming the gate log path and the passing test line(s)) into `$FACTORY_EVIDENCE/cp-{n}/<key>/` so the driver's non-empty directory check (`unproven_rows`) and audit trail still hold. If step 1 edited any file, or if the row drives anything outside the gate (a harness, a live CLI binary invocation, a fixture check), the row MUST still be executed fresh; the checkpoint's single mutation check MUST still run as before.
- **Add the two UI Prove rules to `skills/slice/SKILL.md`:**
  - Under §2 (`Design the slices`, right under the Prove bullet at `:58`), add:
    1. Each form a slice adds gets one empty submit and one narrow-screen drive (375 px).
    2. A Prove row that picks between options seeds a state where the options give different results.

**Accepted cost.** Scanning `Prove` and pre-`Depends on` `**Source**:` of incomplete slices reads a few small markdown files (<1 ms). Citing the full gate log on clean-tree checkpoints saves redundant test re-runs while preserving real proof via the gate log check + non-gate drives + the mandatory mutation check.

**Acceptance.** Unit & bundle tests prove: (1) `handed_qa` defers `FR-3` when covered Slice 02 cites `FR-3` and pending Slice 04 also cites `FR-3` in its `Prove` or pre-`Depends on` `**Source**:` line, does NOT defer `FR-4` when pending Slice 03 mentions `FR-4` only after `**Depends on**:`, and hands `FR-3` once `board.all_done` is true; (2) `assets/contract-checkpoint.md` carries the clean-tree full-gate-log citation rule (asserted via `assert_unique_passage` in `tests/skills_bundle.rs`); (3) `skills/slice/SKILL.md` carries both UI Prove rules (asserted via `assert_unique_passage` in `tests/skills_bundle.rs`).

## 5. FR-3 — Attended polish `reviewed_head` accuracy and `docs log add` lane clarity

**Today.**
- `src/docs/state.rs:184-198`: `apply("reviewed", value)` clears `s.reviewed_head` ONLY when `value` is empty; setting `reviewed=2026-11-27` on a feature with an older `reviewed_head` leaves the stale SHA intact; `is_sha` caps length at 40 hex chars (rejecting SHA-256 64-char SHAs).
- `src/settled.rs:42`: fallback git lookup runs `git log -1 --format=%H -G '^reviewed =' -- state.toml`, which does NOT match a same-day commit whose only state change was removing a stale `reviewed_head =` line.
- `src/cli/docs.rs:226-227, 254-270, 737-752`: `route` re-roots `root` to `FACTORY_BOARD` (the main checkout) for `docs set`, and `set` rejects `reviewed_head=HEAD`.
- `src/closeout.rs:1080`: `let _ = docs.set(&feature, &[("reviewed_head", head)]);` silently swallows errors.
- `src/cli/docs.rs:432-434`: `docs log add` from a lane prints only `<abs-path> updated` to stderr, and `assets/contract-slice.md:9` / `assets/contract-polish.md:9` do not explain that `docs log add` / `docs set` write to the main checkout while the lane's `git status` stays clean.

**Decided 2026-11-27.**

- **Advance attended polish `reviewed_head` accurately (`src/docs/state.rs`, `src/settled.rs`, `src/cli/docs.rs`):**
  - In `src/docs/state.rs::set`: when the call sets a non-empty `reviewed=<date>` AND does NOT also include a `reviewed_head=...` pair in the same `set()` call, clear any pre-existing `s.reviewed_head = None`. Also widen `is_sha` to accept 7..=64 ASCII hex chars (supporting SHA-256 repos as well as SHA-1).
  - In `src/settled.rs:42`: widen the fallback regex from `^reviewed =` to `^reviewed(_head)? =` so a same-day attended polish commit that clears a stale `reviewed_head = "..."` line (with an unchanged `reviewed = "YYYY-MM-DD"` date) is matched by `settled::last_reviewed`, advancing the review watermark cleanly to that commit! (When `reviewed` is cleared via `reviewed=`, line `:35` already returns `None`.)
  - In `src/cli/docs.rs::set`: if a pair is `reviewed_head=HEAD` (exact `"HEAD"`), resolve it via `ci::head(&cwd_repo)?` where `cwd_repo = paths::repo_root(Path::new("."))` (NOT the re-rooted `FACTORY_BOARD` `root`, caught in 2026-11-27 adversarial pass!). Note: `reviewed_head=HEAD` pins the commit *already* at `HEAD` (useful after committing fixes, including same-day Round 3+ attended polish).
- **Report `reviewed_head` write errors in `closeout::ship` (`src/closeout.rs:1073-1081`):**
  - Replace `let _ = docs.set(&feature, &[("reviewed_head", head)]);` with `if let Err(e) = docs.set(&feature, &[("reviewed_head", head)]) { say(out, &format!("ship: could not record reviewed_head — {e}"))?; }`, and add a test proving `closeout::ship` writes `reviewed_head` matching `collect.md`'s head into `state.toml`.
- **Clarify `docs log add` from a lane (`src/cli/docs.rs:432-434`, `assets/contract-slice.md`, `assets/contract-polish.md`):**
  - When `outside_cwd(&dir.join(log::FILE))` is `Some(path)`, print `{path} updated (main checkout; the driver commits the log, so this lane's git status stays clean)` to `err`.
  - Add one concise sentence in `assets/contract-slice.md:9` and `assets/contract-polish.md:9` stating that `docs log add` and `docs set` write to the main checkout (the driver commits them, so the lane's `git status` stays clean).

**Accepted cost.** Clearing a pre-existing `reviewed_head` when `reviewed=<date>` is set alone means callers who want to pin a specific commit pass both pairs (`reviewed=<date> reviewed_head=<sha|HEAD>`). On an unattended run, `closeout::ship` already writes `reviewed_head` explicitly from `collect.md` after polish returns.

**Acceptance.** Unit & integration tests prove: (1) setting `reviewed=2026-11-27` alone on a `state.toml` with an old `reviewed_head` clears it, committing that change causes `settled::last_reviewed` to return the new commit even on the same date (`^reviewed(_head)? =`), setting `reviewed=2026-11-27 reviewed_head=<sha>` keeps `<sha>`, and `docs set <NNN> reviewed_head=HEAD` from a lane with `FACTORY_BOARD` resolves the **lane's** `HEAD` (not the main checkout's!); (2) `closeout::ship` writes `reviewed_head` matching `collect.md`'s head into `state.toml`; (3) `factory docs log add` run from a lane with `FACTORY_BOARD` prints `main checkout; the driver commits the log, so this lane's git status stays clean` on stderr while leaving the lane's `git status` clean, and `assets/contract-slice.md` + `assets/contract-polish.md` carry the matching sentence.

## 6. FR-4 — CLI & code-analysis papercuts (`docs check [FEATURE]`, `ctx def` linguist skip, `leaks` `attributesFile`, squash-merge skip-CI note, `0.20.0` bump)

**Today.**
- `src/cli/docs.rs:65, 297-305`: `Verb::Check` takes no arguments and always validates the entire board. `src/docs/check.rs:808, :883` (`home_paths`, `links`) scan all tracked `docs/` with no path prefix filter.
- `src/ctx/def.rs:152-153`: walks `walk::walk(root, opts)` without skipping `linguist-vendored`/`linguist-generated` files, unlike `src/ctx/refs.rs:61-77`.
- `src/review.rs:364-415`: `check_attr_src` runs `git check-attr [--source <rev>] -z --stdin ...` without passing `-c core.attributesFile=`, so an uncommitted user/local `core.attributesFile` (or `~/.config/git/attributes`) can mark real source files `linguist-generated` and hide hits in `factory leaks` / `review collect` / `ctx`.
- `src/closeout.rs:147-153, 1241-1251` and `skills/deliver/SKILL.md:45-46`: when `ci = local`, `closeout` appends `[skip ci]` to `docs(NNN): ship` (`:147-153`) and `/deliver` §3 appends it to `docs(NNN): owner review` (`:45`), but neither the PR body nor `/deliver` §3 warns that GitHub's default squash-merge message copies those lines onto `main` and suppresses post-merge `main` workflows.

**Decided 2026-11-27.**

- **`factory docs check [FEATURE]` (`src/cli/docs.rs`, `src/docs/check.rs`):**
  - Change `Verb::Check` to `Check { feature: Option<String> }`.
  - When `feature` is `None`: unchanged full-board `check::run_on(root, board)`.
  - When `feature` is `Some(f)`: add `pub fn run_feature_on(root: &Path, board: Option<&Path>, feature: &str) -> Result<Report>` scanning live + archived folders (`board::scan_folder_on`) and resolving `f` (error if unknown):
    - If `f` is **archived**: run only the `E-STATUS-PLACE` check for that archived folder (and duplicate-number check for `f.number`).
    - If `f` is **live**: run `read_project`, duplicate-number check for `f.number`, `feature(root, &board_dir, &limits, target, &mut r)`, plus `home_paths` and `links` scoped via an optional `only_under: Option<&str>` prefix (`target.dir`, e.g. `docs/features/001-x/`), skipping unrelated features, `docs/ideas/`, and other folders.
- **Skip `linguist-vendored` / `linguist-generated` in `ctx::def::find` (`src/ctx/def.rs`, `src/ctx/refs.rs`, `src/ctx/walk.rs`):**
  - Extract the gitattributes filtering block from `src/ctx/refs.rs:50-77` into a shared `pub(super) fn drop_generated(root: &Path, files: &mut Vec<(PathBuf, Lang)>, opts: &WalkOpts)` helper in `src/ctx/walk.rs`, and call it from BOTH `refs::find` and `def::find` when `!opts.vendor`.
- **Neutralize `core.attributesFile` / `~/.config/git/attributes` in `review::check_attr_src` (`src/review.rs:364-415`):**
  - Pass `["-c", "core.attributesFile="]` before `"check-attr"` in `check_attr_src` (do NOT set `GIT_CONFIG_GLOBAL=/dev/null`, so CI container `safe.directory` keeps working!). This stops both explicit `core.attributesFile` and git's default `~/.config/git/attributes` while keeping committed `.gitattributes` active.
- **Token-free squash-merge skip-CI warning in `closeout` PR body and `skills/deliver/SKILL.md` (`src/closeout.rs`, `skills/deliver/SKILL.md`):**
  - **Crucial invariant (caught in 2026-11-27 adversarial pass):** NEVER put the literal `[skip ci]` token inside `pr-body.md`! On repos configured to squash-merge with "Pull request title and description", a literal token in the PR body would itself skip `main` workflows!
  - In `src/closeout.rs::open_pr`: when `local == true`, ensure `pr-body.md` carries a token-free note (idempotently, same pattern as `flag_body`): `> Local CI mode: remove any skip-CI tags from the squash-merge commit message on merge so \`main\` workflows still run.` (never containing the literal bracketed token!).
  - In `skills/deliver/SKILL.md:46`: extend the `ci = "local"` closing note with: `Also strip any skip-CI lines from GitHub's default squash-merge commit message so \`main\` workflows still run on merge.`
- **Version bump `0.19.0` → `0.20.0` & bundle relock:**
  - Bump `0.19.0` → `0.20.0` across `Cargo.toml`, `Cargo.lock`, `mods/cockpit/.claude-plugin/plugin.json`, `mods/cockpit/hooks/register.tsx`, and `mods/cockpit/tests/cockpit.test.tsx`, and run `FACTORY_BUNDLE_WRITE=1 cargo test --test skills_bundle` to relock `skills/bundle.lock` offline-safely.

**Accepted cost.** `-c core.attributesFile=` applies to every caller of `review::check_attr` (`leaks`, `review collect`, `ctx refs`, `ctx def`), meaning personal global attributes files no longer hide files (repo-wide generated rules belong in committed `.gitattributes`). Git's per-repo `.git/info/attributes` is still read by `git check-attr`; neutralizing `core.attributesFile` / `~/.config/git/attributes` closes the local/global config leak without breaking `safe.directory`.

**Acceptance.** Unit & integration tests prove: (1) `factory docs check 001` reports errors/warnings only for feature `001` (ignoring a broken sibling feature `002` and broken ideas), checks `E-STATUS-PLACE` on an archived feature, and `factory docs check` with no arg still catches everything; (2) `ctx::def::find` skips a `.rs`/`.ts` file marked `linguist-generated` in `.gitattributes` (and includes it with `--vendor`); (3) setting local `git config core.attributesFile <file>` marking `src/leak.rs linguist-generated` does NOT hide a `PRD-1` hit from `factory leaks --base main` or `factory leaks --all`, while a `.gitattributes` committed on `main` still skips it; (4) `closeout` in `local` CI mode adds the token-free squash-merge note idempotently to `pr-body.md` (omitting it in `remote` mode and never writing literal `[skip ci]` into `pr-body.md`); (5) `tests/skills_bundle.rs` asserts the unique squash-merge passage in `skills/deliver/SKILL.md` and `claude plugin test mods/cockpit` passes at `0.20.0`.

## 7. Decisions log

No `OPEN-n`. Every fork was settled on 2026-11-27:

- Resolve relative `CARGO_TARGET_DIR` to an absolute path in `Lane::env_for`, prefix `# Inner loop` with `export K=V;` and add `# Lane env` across all worker packs, and warn in `factory lanes up --prove` when `$SHELL -lic` (with `stdin=null` + 3 s timeout) clobbers a declared lane var (§3).
- Defer FR rows in `handed_qa` when any non-done slice cites that FR in `Prove` or pre-`Depends on` `**Source**:`, allow clean-tree checkpoint `run` Prove rows to cite the full green gate log file into `$FACTORY_EVIDENCE/cp-{n}/<key>/`, and add the two UI Prove rules to `skills/slice/SKILL.md` (§4).
- Clear stale `reviewed_head` on `docs set reviewed=<date>` (unless `reviewed_head=` is also passed), widen `settled::last_reviewed` fallback to `^reviewed(_head)? =` and `is_sha` to 7..=64 hex chars, resolve `reviewed_head=HEAD` against cwd's repo (not `FACTORY_BOARD`), report `closeout::ship` `reviewed_head` errors instead of `let _ =`, and clarify `docs log add` main-checkout output + slice/polish contracts (§5).
- Add `factory docs check [FEATURE]`, share `linguist-generated` filtering between `ctx::refs` and `ctx::def`, pass `-c core.attributesFile=` (keeping `safe.directory`) in `review::check_attr_src`, add a token-free squash-merge skip-CI reminder to `closeout` PR bodies (`ci = local`) and `skills/deliver/SKILL.md`, and bump `0.19.0` → `0.20.0` for offline-safe `skills/bundle.lock` relock (§2, §6).

## 8. QA plan

**Gate**: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test && claude plugin validate mods/cockpit && claude plugin test mods/cockpit` *(declared in `docs/project.toml`)*
**Surface**: `factory lanes up --prove`, worker packs (`Pack`), checkpoint `handed_qa` & contracts, `factory docs set` / `log add` / `check [FEATURE]`, `factory ctx def`, `factory leaks`, `closeout` PR body, and `skills/{slice,deliver}/SKILL.md`. CLI-only: every FR is proven below the UI.
**Harness**: unit tests in `src/lanes.rs`, `src/pack.rs`, `src/docs/state.rs`, `src/docs/check.rs`, `src/ctx/def.rs`, `src/closeout.rs`; integration tests in `tests/lanes.rs`, `tests/docs_v3.rs`, `tests/leaks.rs`, `tests/skills_bundle.rs`, `mods/cockpit/tests/cockpit.test.tsx`, and new `tests/delivery_hardening.rs`.
**Seed state**: fixture git repos with multi-slice boards, lane definitions + stub interactive shell clobbering `CARGO_TARGET_DIR`, multi-feature boards with one broken sibling feature, `.gitattributes` committed on `main` vs local `core.attributesFile`, and a `FACTORY_BOARD` lane worktree.

| FR | Proof | Evidence | Human? |
|---|---|---|---|
| FR-1 | run `lane_env_resolves_target_dir_prefixes_pack_and_warns_on_shell_clobber` | test transcript | no |
| FR-2 | run `checkpoint_defers_pending_slice_frs_and_bundle_carries_prove_rules` and `cargo test --test skills_bundle` | test transcript | no |
| FR-3 | run `docs_set_clears_stale_reviewed_head_resolves_head_and_log_add_explains_main_checkout` | test transcript | no |
| FR-4 | run `docs_check_feature_ctx_def_generated_leaks_attributes_and_local_pr_note` and `claude plugin test mods/cockpit` | test transcript | no |

## 9. Slices

_(Not sliced yet. Run `/slice`.)_

## 10. Non-goals

- **Editing the user's personal shell profile (`~/.bashrc`).** Not handled (2026-11-27): `factory` should never mutate user dotfiles; pack env exports + `lanes up --prove` warning fix the problem for every user.
- **Plan-title / phase-label heuristics in `factory leaks`.** Not handled (2026-11-27): high false-positive risk on ordinary domain words; reopen only if unnumbered plan labels recur in polish reviews.
- **Word-boundary builder commit subject truncation.** Already built in `src/commit.rs:241-266` (`cut()`).

## 11. Verification notes

Checked against `main` at `41e4517`, 2026-11-27; revised 2026-11-27 after cross-vendor adversarial pass (`claude:opus`, 406 s, log `20261127-211132`). **Re-verify at slice entry.**

- `docs/reference/lanes/lane-1..4.md` (`env: [CARGO_TARGET_DIR=target/lane-N]`), verified `CARGO_TARGET_DIR=target/lane-2 bash -ic 'echo $CARGO_TARGET_DIR'` returns host `/mnt/devcache/...` on this machine.
- `src/lanes.rs:94-98` (`Lane::env`), `src/cli/lanes.rs:144-214` (`prove`), `src/worker.rs:586-615`, `src/scheduler.rs:769`, `src/checkpoint.rs:119, 329, 895`.
- `src/pack.rs:80, 211-310, 437, 470-565, 682, 749-835` (`for_slice`, `for_merge`, `for_checkpoint`, `for_ci_fix`, `handed_qa`, `qa_rows`).
- `assets/contract-checkpoint.md:14-17`, `assets/contract-slice.md:9`, `assets/contract-polish.md:9`, `skills/slice/SKILL.md:58`, `skills/deliver/SKILL.md:45-46`.
- `src/docs/state.rs:172-200`, `src/settled.rs:32-50`, `src/cli/docs.rs:65, 226-227, 254-270, 297-305, 430-439, 737-758`, `src/docs/check.rs:87-170, 808, 883`.
- `src/closeout.rs:147-153` (`skip_ci`), `:1005-1027` (`flag_body`), `:1073-1081` (`reviewed_head`), `:1212-1260` (`open_pr`).
- `src/ctx/def.rs:132-180` (missing `check_attr`), `src/ctx/refs.rs:50-77` (existing `check_attr` filter).
- `src/review.rs:364-415` (`check_attr_src`), `src/leaks.rs:845-921` (`scan`), `tests/leaks.rs:104`.
- `src/skills.rs:185-237` (`tag_state`, `relock` — bumping `0.19.0` → `0.20.0` ensures offline-safe relock).
- `tests/review.rs:1152-1176`, `tests/review_claims_and_since.rs:363`, `mods/cockpit/tests/cockpit.test.tsx:5`.
