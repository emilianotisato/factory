# Slice 04 — Reviewed head accuracy and docs log add clarity

**Source**: [`prd.md`](prd.md) → §5 (FR-3). **Depends on**: Slices 02 and 03 (same files: `src/cli/docs.rs`, `src/closeout.rs`, `tests/delivery_hardening.rs`)
**Created**: 2026-11-27

**Progress**: Cleared stale `reviewed_head` on lone `reviewed=` in `state::set`, widened `is_sha` to 7..=64 and `last_reviewed` git grep to `^reviewed(_head)? =`, resolved `reviewed_head=HEAD` via `ci::head(cwd)`, `closeout::ship` reports set errors, log-add stderr + slice/polish contract sentences; unit tests in `state.rs`/`settled.rs`/`closeout.rs`, integration `docs_set_clears_stale_reviewed_head_resolves_head_and_log_add_explains_main_checkout`. Inner loop 2 runs (1 clippy `collapsible_if` on ship error branch), ~3 min green. Deliberate red: integration test — `reviewed_head` line kept after lone `reviewed=` set (stale-clear revert); `docs_set_clears_*` assert `!state.contains("reviewed_head")` failed. Deliberate red: `last_reviewed_advances_when_only_reviewed_head_is_cleared` — reverted `^reviewed(_head)? =` grep; fell back to older `^reviewed =` commit, wrong SHA. · cp-2/FR-3/transcript.txt

**Slice**: Fixes attended polish `reviewed_head` drift (clearing stale `reviewed_head` on `docs set reviewed=<date>`, matching `^reviewed(_head)? =` in `settled::last_reviewed`, widening `is_sha` to 7..=64 hex chars, resolving `reviewed_head=HEAD` against the cwd repo rather than `FACTORY_BOARD`, and reporting `closeout::ship` write errors instead of `let _ =`) and clarifies `docs log add` main-checkout stderr output and slice/polish contracts.

**Highest-risk**: Keeping `tests/review.rs:1152-1164` (`last_reviewed_commit_is_the_one_that_set_reviewed`) and `tests/review_claims_and_since.rs:363` (`reviewed_head` count == 1 in `skills/polish/SKILL.md`) 100% untouched and green, and resolving `reviewed_head=HEAD` against `cwd`'s repo instead of the re-rooted `FACTORY_BOARD` checkout.

## Current code / environment

- **`src/docs/state.rs:172-200`**: `apply("reviewed", value)` clears `s.reviewed_head` only when `value` is empty; `is_sha` allows only 7..=40 hex chars.
- **`src/settled.rs:32-50`**: `last_reviewed` returns `st.reviewed_head` if set, else runs `git log -1 --format=%H -G '^reviewed =' -- <state.toml>` (`:42`).
- **`src/cli/docs.rs:226-227, 254-270, 737-752`**: `route` re-roots `root` via `board_root(&root, args)` to `FACTORY_BOARD` (the main checkout) for `docs set` and `docs log add`. `set()` passes raw pairs straight to `state::set`.
- **`src/cli/docs.rs:430-439`**: `LogVerb::Add` prints `{path} updated` to `err` when `outside_cwd(&dir.join(log::FILE))` returns `Some(path)`.
- **`src/closeout.rs:1073-1081`**: `let _ = docs.set(&feature, &[("reviewed_head", head)]);` silently swallows errors.
- **`assets/contract-slice.md:9` and `assets/contract-polish.md:9`**: Say `state.toml` and `log.jsonl` are written through the tool only, without explaining that they live in the main checkout and leave the lane's `git status` clean.
- **`tests/review.rs:1152-1164` & `tests/review_claims_and_since.rs:363`**: Unauthorized — must remain green untouched! Do NOT edit `skills/polish/SKILL.md`.

## Tasks

### 1. Clear stale `reviewed_head`, widen `is_sha` & `settled::last_reviewed`, and resolve `reviewed_head=HEAD` against cwd — high — blocked by: —
**Source**: §5 FR-3: "In `src/docs/state.rs::set`: when the call sets a non-empty `reviewed=<date>` AND does NOT also include a `reviewed_head=...` pair in the same `set()` call, clear any pre-existing `s.reviewed_head = None`. Also widen `is_sha` to accept 7..=64 ASCII hex chars... In `src/settled.rs:42`: widen the fallback regex from `^reviewed =` to `^reviewed(_head)? =`... In `src/cli/docs.rs::set`: if a pair is `reviewed_head=HEAD` (exact `\"HEAD\"`), resolve it via `ci::head(&cwd_repo)?` where `cwd_repo = paths::repo_root(Path::new(\".\"))` (NOT the re-rooted `FACTORY_BOARD` `root`!)."

`src/docs/state.rs`, `src/settled.rs`, `src/cli/docs.rs`.
- In `src/docs/state.rs`:
  - Widen `is_sha(s: &str)` to `(7..=64).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_hexdigit())`.
  - In `set(feature_dir, pairs)`: check whether `pairs` contains any `("reviewed_head", _)` entry. If a pair sets non-empty `reviewed=<date>` and no `reviewed_head` pair was passed in `pairs`, clear `s.reviewed_head = None` (so `reviewed=2026-11-27 reviewed_head=<sha>` still works regardless of argument order!).
- In `src/settled.rs:42`: change `"-G", "^reviewed ="` to `"-G", "^reviewed(_head)? ="` so a same-day commit whose only state diff is removing a stale `reviewed_head = "..."` line advances `last_reviewed` to that commit!
- In `src/cli/docs.rs::set`:
  - Before calling `state::set`, resolve any `("reviewed_head", "HEAD")` pair by calling `let cwd_repo = paths::repo_root(Path::new(".")).unwrap_or_else(|_| PathBuf::from(".")); crate::ci::head(&cwd_repo)?` so running from a lane with `FACTORY_BOARD` pointing at another checkout resolves the **lane's** `HEAD`, not the main checkout's!
- **The trap**: Never stamp `reviewed_head` automatically when `reviewed=<date>` is passed alone — that would break `tests/review.rs::last_reviewed_commit_is_the_one_that_set_reviewed` (`:1152-1164`). Only clear a *stale* `reviewed_head` and resolve explicit `reviewed_head=HEAD`.

**Acceptance:** Unit tests in `src/docs/state.rs` and `src/settled.rs` prove: (a) setting `reviewed=2026-11-27` alone clears a stale `reviewed_head`, while setting both `reviewed=2026-11-27` and `reviewed_head=<sha>` (in either order) keeps `<sha>`; (b) `is_sha` accepts 64-char SHA-256 hex strings; (c) a same-day commit that only clears `reviewed_head` advances `settled::last_reviewed` to that commit (fails if `state::set` clearing or `^reviewed(_head)? =` is reverted).

### 2. Report `closeout::ship` `reviewed_head` errors and clarify `docs log add` + worker contracts — high — blocked by: 1
**Source**: §5 FR-3: "Replace `let _ = docs.set(&feature, &[(\"reviewed_head\", head)]);` with `if let Err(e) = docs.set(&feature, &[(\"reviewed_head\", head)]) { say(out, &format!(\"ship: could not record reviewed_head — {e}\"))?; }`, and add a test proving `closeout::ship` writes `reviewed_head` matching `collect.md`'s head into `state.toml`... When `outside_cwd(&dir.join(log::FILE))` is `Some(path)`, print `{path} updated (main checkout; the driver commits the log, so this lane's git status stays clean)` to `err`. Add one concise sentence in `assets/contract-slice.md:9` and `assets/contract-polish.md:9` stating that `docs log add` and `docs set` write to the main checkout (the driver commits them, so the lane's `git status` stays clean)."

`src/closeout.rs`, `src/cli/docs.rs`, `assets/contract-slice.md`, `assets/contract-polish.md`, `tests/delivery_hardening.rs`.
- In `src/closeout.rs:1073-1081`: replace `let _ = docs.set(&feature, &[("reviewed_head", head)]);` with explicit `if let Err(e)` reporting `ship: could not record reviewed_head — {e}` via `say(out, ...)?`.
- Add a unit test in `src/closeout.rs` (or integration test in `tests/delivery_hardening.rs`) proving `closeout::ship` writes `reviewed_head` matching `collect.md`'s head into `state.toml`.
- In `src/cli/docs.rs:432-434`: update the `outside_cwd` stderr line to `{path} updated (main checkout; the driver commits the log, so this lane's git status stays clean)`.
- In `assets/contract-slice.md:9` and `assets/contract-polish.md:9`: append one clear sentence stating that `docs log add` and `docs set` write to the main checkout (the driver commits them, so the lane's `git status` stays clean).
- Add integration test `docs_set_clears_stale_reviewed_head_resolves_head_and_log_add_explains_main_checkout` in `tests/delivery_hardening.rs` proving:
  1. Setting `reviewed=2026-11-27` alone clears a stale `reviewed_head`, and after committing on the same date, `settled::last_reviewed` returns the new commit.
  2. Running `factory docs set 001 reviewed_head=HEAD` from a worktree/lane with `FACTORY_BOARD` pointing at a main repo on a *different* commit records the **lane's** `HEAD` SHA, not the main checkout's.
  3. Running `factory docs log add` from a lane with `FACTORY_BOARD` prints `main checkout; the driver commits the log, so this lane's git status stays clean` on stderr while the lane's `git status --porcelain` stays empty.
  4. `assets/contract-slice.md` and `assets/contract-polish.md` contain the main-checkout note once each.

**Acceptance:** `cargo test --test delivery_hardening docs_set_clears_stale_reviewed_head_resolves_head_and_log_add_explains_main_checkout` passes and existing `tests/review.rs` + `tests/review_claims_and_since.rs` stay green untouched (fails if stale clearing, cwd `HEAD` resolution, `log add` stderr clarity, or contract text is removed).

### 3. Verify — medium — blocked by: 1, 2
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 4. Prove — at the checkpoint — blocked by: 3
**Source**: §8 QA plan FR-3 row: "run `docs_set_clears_stale_reviewed_head_resolves_head_and_log_add_explains_main_checkout`".

At Checkpoint 2, run `cargo test --test delivery_hardening docs_set_clears_stale_reviewed_head_resolves_head_and_log_add_explains_main_checkout` alongside the full gate and record the transcript in **Progress**.

**Acceptance:** Checkpoint 2 transcript in **Progress** confirms `docs_set_clears_stale_reviewed_head_resolves_head_and_log_add_explains_main_checkout` passes (would fail if `reviewed_head` clearing/resolution, `closeout::ship` write, or `docs log add` lane message were broken).

**Code-review fixes (2026-11-28).**

- Second opinion: `detect_shell_clobber` runs the login shell in its own process group, SIGKILLs the group on timeout, and caps stdout reads inside the same 3 s window so a background job cannot hang `lanes up --prove` or steal Ctrl-C from `factory`.
- Second opinion: `Pack::for_polish` and `Pack::for_fix_env` carry `# Lane env` and a prefixed `# Inner loop`; `closeout` passes `lane_def.env_for` into polish.
- Second opinion: `Lane::run_setup_capped` uses `env_for(dir)`; empty `CARGO_TARGET_DIR=` is dropped so cargo does not litter the worktree root.
- Second opinion: `delivery_hardening` pins cockpit/bundle versions to `CARGO_PKG_VERSION` instead of a literal semver.
- Second opinion: `settled` unit git fixture isolates global git config and checks commit exit codes.
