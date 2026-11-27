---
number: 030
slug: guardrails-and-live-hardening
title: "Guardrails, hooks, and live/status hardening"
type: feature
created: 2026-11-27
author: Emiliano Tisato
authorized_tests: ["mods/cockpit/tests/cockpit.test.tsx", "tests/cockpit.rs::status_json_carries_the_board_and_what_the_owner_owes"]
summary: |
  Eliminates recurring run-time friction and security/UI edge cases across 11 triaged ideas: loosens `testguard` solely for purely additive top-level Rust `use` and JS/TS `import` merges (keyed by local binding, guarded by a regression corpus); fixes `shell::first_cmd_standalone` so stdout-to-file redirects (`grep … > file`) are allowed while `2>/dev/null` and `< file` stay denied; honors `.gitattributes` `linguist-generated`/`linguist-vendored` in `leaks` (read from `<base>` via `--source`, preventing branch self-silencing) and `ctx refs`; fixes `mods/cockpit` `TS2345` warnings and `discover_models` `ETXTBSY` retry; adds `# Sent back` context and collision-free `cp-{n}.sent-back-{k}/` rotation on checkpoint send-back; adds external-platform/live-CLI/third-party-shim verification rules to `/prd`, `/slice`, `/polish`; enriches `status --json` questions with `body`/`phase` and symlink-safe reads in both `status` and `board`; fixes `factory live` post-fix CI `waiting` state, `--since` cap direction, post-cap `rev` with 16-byte placeholder, and canonical lane labels; hardens `live/redact` and fixes `excerpt`/`gate_failure` edge cases.
---

# PRD-030 — Guardrails, hooks, and live/status hardening

> **Interviewed 2026-11-27** (closing 11 ideas after owner keep/drop triage: `2026-11-26-testguard-ignore-purely-additive-top-level-use-and.md`, `2026-11-26-grep-hook-denies-standalone-greps-with-output-redi.md`, `2026-11-26-leaks-and-ctx-refs-ignore-gitattributes-generated.md`, `2026-11-26-gate-fix-mods-cockpit-ts2345-noise-plugin-test-com.md`, `2026-11-23-a-checkpoint-send-back-tells-the-respawn-why-and-d.md`, `2026-11-26-skills-verify-external-platform-rules-live-cli-fla.md`, `2026-11-06-status-json-for-the-herdr-cockpit-full-question-te.md`, `2026-11-07-factory-live-cap-rev-and-owner-section-edge-cases.md`, `2026-11-07-factory-live-say-ci-is-waiting-before-its-checks-r.md`, `2026-11-07-failure-excerpt-and-red-gate-title-edge-cases.md`, `2026-11-07-live-redactor-shapes-it-misses-cuts-short-or-over.md`). Every `file:line` and live CLI check below was verified against `main` at `00287c4`. Revised 2026-11-27 after cross-vendor adversarial pass (`claude:opus`, log `20261127-141852`).

## 1. Why now

Five clusters of real friction and security/correctness bugs recurred across PRD-019 through PRD-029 runs:

- **False-positive `testguard` blocks and broken `grep` redirects.** In PRD-020, adding a new test at the bottom of `tests/lanes.rs` and `tests/closeout.rs` triggered `BLOCK-1`/`BLOCK-2` solely because `use foo::a;` became `use foo::{a, b};` at the top of the file. Meanwhile `shell::first_command_standalone` (`src/shell.rs:150-204`) ignores `Tok::Redirect` (`:171`), causing the grep hook to deny `grep foo src > /tmp/out`.
- **`leaks` and `ctx refs` ignore `.gitattributes` `linguist-generated`/`vendored`.** `review::skip` (`src/review.rs:298-336`) honors `.gitattributes` via `check_attr` (`:351-386`), but `factory leaks` flagged the same minified third-party chunk across 13 consecutive features, and `factory ctx refs` walks nested committed build dirs (like `public/build/`) because `ROOT_DIRS` (`src/ctx/walk.rs:11`) only skips depth 1.
- **Gate noise, blind checkpoint send-backs, and missing skill guards.** Live `claude plugin validate mods/cockpit` prints 2 `TS2345` errors (`Property 'isCollapsed' is missing` in `mods/cockpit/tests/cockpit.test.tsx:154,168`); `provider::discover_models` (`src/provider.rs`) can hit a Linux `ETXTBSY` (`ExecutableFileBusy`) race when another thread forks while a temp script write fd is open; `checkpoint::run` (`src/checkpoint.rs:304-420`) respawns a sent-back checkpoint worker with the identical pack (never telling it which slices or rows were missing) and leaves stale `cp-{n}/` evidence behind after resetting the lane; and `/prd`, `/slice`, and `/polish` lack rules to verify external platform behavior, live CLI flags, and not invoke unreviewed third-party shims in security passes.
- **`status --json` and `factory live` UI/security gaps.** `status --json` returns only the 160-char block title in `questions[].text` without the options/body from `log.jsonl`, drops `CloseoutBlocked.phase`, and follows symlinks unboundedly in `board::scan_folder_on` (`src/docs/board.rs:186`) and `status::prd_title`/`feature_folder` (`src/cli/status.rs:243-275`). In `factory live`, `live/gate.rs:187-218` keeps `ci.state = "red"` after `CiFixCommitted`/`CiRerun`/`PrOpened` while checks register; `live/mod.rs:114-115` hashes `rev` *before* `cap` and `cut_timeline` (`:194-206`) drops oldest events even with `--since`; and `live/workers.rs:83-91` uses lexical lane paths that fail to strip when `data_dir`/`repo` sits behind a symlink.
- **Redactor misses & `excerpt` false hits.** `live/redact.rs:15-42` misses URL userinfo, `Authorization: Basic/Token`, `Cookie`/`Set-Cookie`/`PRIVATE-TOKEN`, PEM blocks, and common prefixes (`sk-proj-`, `glpat-`, `npm_`, `AIza`, `sk_live_`, `xapp-`), over-redacts `bypass=` and `compass=`, and can rewrite `evidence[].path` / `attention[].answer`. `excerpt.rs` matches JSON `"failed": 0` as a generic failure and colon-separated SGR `38:2::0:0:31` as red (`:208-222`), and `docs/check.rs:257-270` accepts empty-matching `gate_failure` patterns (`""` or `^`).

**What this leaves alone.** Verified live on `00287c4`: `claude plugin test mods/cockpit` already runs cleanly (`14 pass, 0 fail` in 0.38 s), so `docs/project.toml` gate spelling stays untouched. Low-ROI micro-items from the `#4` ideas were permanently dropped during owner triage (documented in §11 Non-goals).

## 2. Blast radius

- **`mods/cockpit/tests/cockpit.test.tsx` (authorized).** Adding `isCollapsed: false,` to `BAND.props` (`:14-21`) clears both `TS2345` errors from `claude plugin validate mods/cockpit` without changing any test behavior.
- **`tests/cockpit.rs::status_json_carries_the_board_and_what_the_owner_owes` (authorized) and `src/cli/status.rs:917`.** `tests/cockpit.rs:322-332` asserts exact equality on `run["questions"]` for an open `BLOCK-1` whose `log.jsonl` carries a body; adding optional `body` (with `#[serde(skip_serializing_if = "Option::is_none")]`) adds `"body"` on `BLOCK-1` while leaving `ASK-1` (`:401-410`, no log body) untouched. Adding `body`/`phase` to `cockpit::Question` also updates the `QuestionJson { .. }` struct literal in `src/cli/status.rs:917` (unit test in `src/`).
- **Guard loosening (`testguard` & `grep` hook).** FR-1 loosens two security/integrity guards; per `/prd` §2, every acceptance test replays a regression corpus proving all previously-denied non-target cases (including `2>/dev/null`, `< file`, import deletions, and import alias renames) remain strictly denied.
- **Test file placement vs `[leaks] allow` (caught 2026-11-27 in adversarial pass).** The `[leaks] allow` list is read from `main`, so new test files cannot carry plan-id-shaped fixture strings (`"PRD-999"`). FR-2's tests live in existing allowlisted `tests/leaks.rs` and `tests/ctx.rs` (adding new test functions only, never modifying existing tests or top-level `use` lines).
- **`Pack::for_checkpoint` signature (`src/pack.rs:427`).** Called in `src/checkpoint.rs:304` and unit tests in `src/pack.rs:1237, 1301, 1484, 1711`; adding `sent_back: Option<&SentBack>` updates all internal call sites cleanly.
- **`skills/{prd,slice,polish}/SKILL.md` & `skills/bundle.lock`.** Updating those three skill files requires regenerating `skills/bundle.lock` via `FACTORY_BUNDLE_WRITE=1 cargo test --test skills_bundle` (`v0.18.0` is untagged, so no version bump is needed). Preserve every existing passage asserted in `tests/skills_bundle.rs`.

## 3. FR-1 — `testguard` additive import merges and `grep` hook output redirects

**Today.**
- `testguard::parse_diff` (`src/testguard.rs:136-159`) flags any hunk in an unauthorized pre-existing test file whenever `hunk.removed > 0`, even when the only removed line is a top-level `use foo::a;` merged into `use foo::{a, b};`.
- `shell::tokenize` (`src/shell.rs:89-115`) discards the file descriptor and redirect direction (`>` vs `<`) before emitting `Tok::Redirect`, and `first_command_standalone` (`:150-204`) ignores `Tok::Redirect` altogether, causing the grep hook to deny `grep foo src > /tmp/out`.

**Decided 2026-11-27.**

- **Allow purely additive top-level Rust `use` and JS/TS `import` merges in `src/testguard.rs`:**
  - Inspect each hunk with `hunk.removed > 0` in a pre-existing test file (NOT deleted files).
  - The hunk is exempt ONLY when ALL of the following hold:
    1. `enclosing_test_name(...)` is `None` (strictly top-level, outside any test function/block).
    2. Every removed `-` line (ignoring blank/whitespace-only lines) parses as a complete top-level import statement: `use ...;` / `pub use ...;` / `pub(crate) use ...;` in `.rs` files, or `import ... from '...';` / `import type ... from '...';` in `.ts`/`.tsx`/`.js`/`.jsx`/`.mjs` files (any partial/unparseable/non-import `-` line fails immediately and stays **strictly denied**).
    3. Every removed import item is keyed by **full path + local binding name** (for Rust: `(visibility, full_source_path, local_binding)` so `use foo::a;` yields `("", "foo::a", "a")` while `use foo::a as z;` yields `("", "foo::a", "z")`; for JS/TS: `(is_type, module_specifier, imported_name, local_binding)` so `import { a } from 'x'` differs from `import { a as z } from 'x'` and `import { a } from 'y'`). The removed set must be non-empty, and **every** removed item must appear verbatim in the set of items parsed from the hunk's added `+` lines!
- **Treat stdout-to-file redirects (and ONLY stdout-to-file redirects) as non-standalone in `src/shell.rs:89-204`:**
  - Change `Tok::Redirect` (`src/shell.rs`) to carry `stdout_to_file: bool`:
    - `true` ONLY when redirecting stdout to a file: `>`, `>>`, `1>`, `1>>`, `&>`, `&>>` (and NOT fd duplication like `>&2` / `1>&2`, NOT stderr-only `2>/dev/null` / `2>>log`, and NOT stdin `< file` / `<<EOF` / `<<<str`).
    - Inspect the preceding numeric fd word (`word.is_empty()` or `word == "1"`) and arrow char (`>` vs `<`, or `&>`) BEFORE clearing `word` at `:106-114`!
  - In `first_command_standalone` (`:150-204`), track `let mut has_stdout_file_redirect = false;` (reset per command across leading `cd`/`pushd`), set it to `true` on `Tok::Redirect { stdout_to_file: true }`, and return `standalone = false` when `has_stdout_file_redirect` is `true`.

**Accepted cost.** Multi-line Rust `use` or JS `import` blocks where `-U0` sees only an interior line without its prefix/source stay conservatively denied rather than guessing across lines — zero risk of letting an import deletion slip through.

**Acceptance.** Regression-corpus tests (`testguard_allows_additive_imports_and_denies_removals_and_in_test_edits` and `grep_hook_allows_output_redirects_and_keeps_standalone_denied`) prove:
- **testguard allowed:** `use foo::a;` → `use foo::{a, b};`, `use a::b;` → `use a::{b, c};`, `import { a } from 'x'` → `import { a, b } from 'x'`, and `import type { A } from 'x'` → `import type { A, B } from 'x'` at top level.
- **testguard regression corpus (MUST stay denied):** deleting a `use` line, removing one symbol from `use foo::{a, b};` → `use foo::a;`, renaming/aliasing `use foo::a;` → `use foo::a as z;`, aliasing `import { a } from 'x'` → `import { a as z } from 'x'`, changing an import source `'x'` → `'y'`, changing `import` to `import type`, editing a `use` inside `#[test] fn t() { ... }`, or modifying any non-import line alongside a `use` merge.
- **grep hook allowed vs regression corpus:** `grep foo src > /tmp/out`, `grep foo src 1>> /tmp/out`, `grep foo src &> /tmp/out`, and `grep foo src > /tmp/out && wc -l /tmp/out` are allowed; standalone `grep foo src`, `grep -rn foo src 2>/dev/null`, `grep foo < /tmp/in`, `grep foo src >&2`, `rg foo src`, `cd sub && grep foo .`, and `grep foo src;` remain strictly denied.

## 4. FR-2 — Honor `.gitattributes` `linguist-generated` / `linguist-vendored` in `leaks` and `ctx refs`

**Today.** `review::skip` (`src/review.rs:298-336`) calls `check_attr` (`:351-386`, running `git check-attr -z --stdin linguist-vendored linguist-generated`) against the working tree, while `leaks::scan` (`src/leaks.rs:837-896`) scans every added line outside `docs/` and `ctx::walk::walk` (`src/ctx/walk.rs:48-91`) only skips `VENDOR_DIRS` (`vendor`, `node_modules`, `target`) and depth-1 `ROOT_DIRS` (`dist`, `build`, `storage`). Crucially, `leaks::scan` reads the allowlist from `&base_ref` (`src/leaks.rs:846-847`) so a branch cannot loosen its own leaks scan — `.gitattributes` for `leaks` MUST follow the same anti-tampering rule!

**Decided 2026-11-27.**

- **Share `check_attr` with optional `--source <tree-ish>` (`src/review.rs:351-386`):**
  - Expose `pub fn check_attr(root: &Path, paths: &[&str]) -> Result<Vec<(String, &'static str)>>` (working tree, for `review::skip` and `ctx refs`) AND `pub fn check_attr_at(root: &Path, source: &str, paths: &[&str]) -> Result<Vec<(String, &'static str)>>` running `git check-attr --source <source> -z --stdin linguist-vendored linguist-generated` (verified live on `git 2.56.0`).
- **Skip base-marked paths in `factory leaks` (`src/leaks.rs:541-613, :837-896`):**
  - In `leaks::scan`: collect distinct repo-relative paths present in `lines` and `paths`.
    - For `Input::Base(base)`: call `review::check_attr_at(repo, &base_ref, &distinct)` so `.gitattributes` is read strictly from `base_ref` — a branch that adds `src/** linguist-generated` to `.gitattributes` CANNOT silence its own leaks scan!
    - For `Input::All`: call `review::check_attr_at(repo, "HEAD", &distinct)` (matching `Input::All`'s scan of `HEAD`).
  - Filter out any `Line` or `Path` whose path is marked `linguist-generated` or `linguist-vendored` before running `check(&allow, &lines, &paths)`.
  - Record skipped paths on `Report` (`pub skipped: Vec<(String, &'static str)>`) and render a `skipped:` block in `Report::render` when `!self.skipped.is_empty()` (`skipped:\n  <path> (<reason>)\n`), leaving output byte-identical when nothing was marked generated/vendored.
- **Skip marked paths in `factory ctx refs` (`src/ctx/walk.rs:48-91`, `src/ctx/refs.rs:49-105`):**
  - When `!opts.vendor`, after `walk::walk(root, opts)` collects candidate files, if `paths::repo_root(root)` succeeds, pass the repo-relative paths through `review::check_attr(&repo_root, &rels)` (ignoring non-git directories gracefully) and drop any file marked `linguist-generated` or `linguist-vendored` before `walk::scan`.
  - When `opts.vendor` (`--vendor`) is `true`, include those marked files.

**Accepted cost.** Calling `git check-attr -z --stdin` once per `leaks::scan` and `ctx::refs::find` adds one local git process (~5–15 ms, O(1) spawns regardless of file count). Reading `--source <base_ref>` for `leaks --base` means a newly added `.gitattributes` rule takes effect after it lands on `main`, matching the `[leaks] allow` anti-tampering contract (`src/leaks.rs:846-847`).

**Acceptance.** Integration tests added to already-allowlisted `tests/leaks.rs` (`leaks_honors_base_gitattributes_and_blocks_branch_self_silencing`) and `tests/ctx.rs` (`ctx_refs_honors_gitattributes_generated_unless_vendor_flag_set`) prove:
1. With `.gitattributes` on `main` marking `public/build/** linguist-generated`, a branch adding ONLY `public/build/app.js` containing an `id-string` hit (`"PRD-999"`) makes `factory leaks --base main` exit `0` (`clean`) and list `public/build/app.js (generated)` under `skipped:`.
2. Adding a real `src/app.js` with `"PRD-999"` alongside `public/build/app.js` makes `factory leaks --base main` exit `1` (`1 fix hit` on `src/app.js` only, while `public/build/app.js` stays skipped).
3. A branch that adds `src/** linguist-generated` to `.gitattributes` on the branch (not on `main`) STILL reports the `src/app.js` hit (proving `--source <base_ref>` prevents self-silencing!).
4. `factory ctx refs uniqueBundleSym` skips `public/build/app.js` by default, while `factory ctx refs --vendor uniqueBundleSym` finds it.

## 5. FR-3 — Gate hygiene, checkpoint send-back context & stale evidence, and skills verification rules

**Today.**
- `claude plugin validate mods/cockpit` reports 2 `TS2345` warnings because `BAND.props` (`mods/cockpit/tests/cockpit.test.tsx:14-21`) omits `isCollapsed: false`.
- `provider::discover_models` (`src/provider.rs:266-315`) spawns a CLI binary once with no retry on Linux `ErrorKind::ExecutableFileBusy` (`ETXTBSY`), which races with concurrent test threads forking while another thread holds a script write fd.
- `checkpoint::run` (`src/checkpoint.rs:304-430`) computes `missing` slices and `unproven` rows on send-back (`:408-420`), then loops back to `Pack::for_checkpoint` with the same inputs (never telling the respawn *why* it was sent back) and resets the lane (`:319`) while leaving stale `$FACTORY_EVIDENCE/cp-{n}/` directories on disk for `unproven_rows` to accept on the next attempt.
- `skills/prd/SKILL.md` §4, `skills/slice/SKILL.md` §3, and `skills/polish/SKILL.md` §1 lack explicit rules for verifying external platform rules, live CLI flags, and not running unreviewed third-party CLIs/shims during security review.

**Decided 2026-11-27.**

- **Fix `mods/cockpit` `TS2345` warnings & `discover_models` `ETXTBSY` retry:**
  - Add `isCollapsed: false,` to `BAND.props` in `mods/cockpit/tests/cockpit.test.tsx:14-21` (authorized in `prd.md`).
  - Extract a small `spawn_retry_busy(cmd: &mut Command) -> std::io::Result<Child>` helper in `src/provider.rs` used by `discover_models`: retries `cmd.spawn()` on `ErrorKind::ExecutableFileBusy` up to 5 times with a 10 ms sleep (plus atomic temp-write + rename in test helper `script`). Unit-test `spawn_retry_busy` (or `discover_models` with a synthetic busy-retry / concurrent fork test) in `src/provider.rs`.
- **Checkpoint send-back context & collision-free stale evidence rotation (`src/checkpoint.rs`, `src/pack.rs`):**
  - Add `pub struct SentBack { pub attempt: u32, pub missing_slices: Vec<u32>, pub unproven_rows: Vec<String>, pub prior_record: Option<u32> }` and extend `Pack::for_checkpoint` (`src/pack.rs:427`) with `sent_back: Option<&SentBack>`:
    - When `Some(sb)`, append a `# Sent back` section to the checkpoint pack naming the prior record (`CP-{k}` if any), the missing slices (`pad_list`), and the previously unproven FR row keys, AND stating explicitly that **prior `cp-{n}/` evidence was moved aside because the lane was reset, so the respawn must re-drive and save fresh evidence for EVERY row of checkpoint `n`**.
  - In `src/checkpoint.rs` (`run`, `:304-465`): before continuing on a send-back, populate `sent_back = Some(SentBack { ... })` AND rotate `$FACTORY_EVIDENCE/cp-{n}/` (if it exists) to the **first unused** sibling path `$FACTORY_EVIDENCE/cp-{n}.sent-back-{k}/` (`k = 1, 2, ...` while target exists, so a later send-back after a block reset never collides with an earlier rotation!).
- **Skills verification rules (`skills/prd/SKILL.md`, `skills/slice/SKILL.md`, `skills/polish/SKILL.md`, `skills/bundle.lock`):**
  - `skills/prd/SKILL.md` §4: add one sentence containing the exact unique phrase `external platform behavior` instructing the adversarial pass to verify external platform behaviors an FR relies on (e.g. CI trigger rules, webhook/API guarantees), not only repo `file:line` claims.
  - `skills/slice/SKILL.md` §3: add one bullet containing the exact unique phrase `verify external CLI flags against the live binary` instructing recon to check external CLI flags against `--help` or official docs before specifying them in a task.
  - `skills/polish/SKILL.md` §1 (`security-review` brief): add one sentence containing the exact unique phrase `never invoke third-party CLIs or lazy shims not under review` forbidding the security subagent from running unreviewed CLIs/shims.
  - Regenerate `skills/bundle.lock` via `FACTORY_BUNDLE_WRITE=1 cargo test --test skills_bundle` (preserving all existing passages asserted by `tests/skills_bundle.rs`).

**Accepted cost.** Rotating `cp-{n}/` to `cp-{n}.sent-back-{k}/` requires the respawned checkpoint worker to re-drive all rows for checkpoint `n` on the clean lane (intentional — evidence from a reset/discarded tree is invalid).

**Acceptance.** Test `checkpoint_send_back_tells_respawn_why_and_rotates_stale_evidence` (plus `claude plugin validate mods/cockpit`, `src/provider.rs` busy-retry unit test, and `cargo test --test skills_bundle`) proves: (1) `claude plugin validate mods/cockpit` produces zero `TS2345` warnings; (2) when attempt 1 writes a short record (missing slice 02) and writes `cp-1/FR-1/proof.txt`, the send-back rotates `cp-1/` to `cp-1.sent-back-1/FR-1/proof.txt`, the respawn's pack contains `# Sent back` naming missing slice `02` and instructing fresh evidence for every row, and if the respawn writes a full record WITHOUT writing new `cp-1/FR-1/` evidence, checkpoint detects `FR-1` as unproven and blocks (proving stale `cp-1/FR-1/` did not carry over, with zero changes to `tests/stubs/claude`), and a second send-back rotates to `cp-1.sent-back-2/` without colliding; (3) the three exact skill phrases above each appear exactly once in their respective `SKILL.md` and `tests/skills_bundle` passes.

## 6. FR-4 — `status --json` question body & symlink safety, and `factory live` CI/cap/`rev`/label fixes

**Today.**
- `cockpit::Question` (`src/cockpit.rs:215-268`) exposes only `id`, `at`, `n`, `text`, `answer`; `status --json` drops `CloseoutBlocked.phase` and omits the block's `log.jsonl` body. Both `board::scan_folder_on` (`src/docs/board.rs:186`) — called first by `run_json` via `Docs::status` (`src/cli/status.rs:185`) — and `status::prd_title` (`src/cli/status.rs:272-275`) call plain `fs::read_to_string`, following symlinks/FIFOs with no size cap.
- `live::gate::ci` (`src/live/gate.rs:182-230`) ignores `CiFixCommitted`, `CiRerun`, and `PrOpened`, leaving `ci.state = "red"` and stale `ci.excerpt` while new checks register.
- `live::snapshot` (`src/live/mod.rs:113-116`) computes `rev(&v)` BEFORE `cap(&mut v)`, and `cut_timeline` (`:194-206`) always drains from the front (oldest) even when `opts.since.is_some()`.
- `live::workers::Paths::of` (`src/live/workers.rs:83-91`) stores lexical `lanes::dir(...)` and `ctx.state.repo`, missing canonicalized worker tool paths when `data_dir` or `repo` sits behind a symlink.

**Decided 2026-11-27.**

- **`status --json` & symlink-safe bounded reads (`src/cockpit.rs`, `src/cli/status.rs`, `src/docs/board.rs`):**
  - Add `#[serde(skip_serializing_if = "Option::is_none")] pub phase: Option<String>` (populated from `CloseoutBlocked { phase, .. }`) and `#[serde(skip_serializing_if = "Option::is_none")] pub body: Option<String>` to `cockpit::Question` (updating `src/cli/status.rs:917` literal and authorized `tests/cockpit.rs::status_json_carries_the_board_and_what_the_owner_owes`).
  - In `cli::status::questions`, when a question is a `BLOCK-n` and the feature's `log.jsonl` exists as a regular file, populate `q.body` from the matching block entry's trimmed non-empty body.
  - Add a shared helper `read_regular_capped(path: &Path, max_bytes: u64) -> Option<String>` (e.g., in `src/paths.rs` or `src/docs/board.rs`) checking `fs::symlink_metadata(path).ok()?.file_type().is_file()` (refusing symlinks, FIFOs, and device nodes!) and reading at most `max_bytes` (`256 KiB` for `prd.md`); use it in BOTH `src/docs/board.rs:186` (`scan_folder_on` reading `prd.md`) AND `src/cli/status.rs:272-275` (`prd_title`), and check `e.file_type().is_ok_and(|t| t.is_dir())` (non-following) in `cli::status::feature_folder` (`:243-270`).
- **`factory live` CI `waiting`, `--since` cap, post-cap `rev`, and canonical lane paths (`src/live/gate.rs`, `src/live/mod.rs`, `src/live/workers.rs`):**
  - In `live::gate::ci` (`src/live/gate.rs:187-229`): inspect events in reverse; if `EventKind::PrOpened { .. }`, `EventKind::CiFixCommitted { .. }`, or `EventKind::CiRerun { .. }` is newer than the last `CiGreen`/`CiRed`/`CiWaiting`/`CiNoChecks`/`CiLocal` while `!ctx.state.phase.is_over()` and `ctx.state.phase == Phase::Closeout`, report `state = "waiting"`, `checks = []`, and `excerpt = Value::Null`.
  - In `live::snapshot` (`src/live/mod.rs:101, :113-116`): initialize `"rev": "0000000000000000"` (exact 16-byte placeholder matching the final 16-hex-char SHA256 prefix!) BEFORE calling `cap(&mut v, opts.since.is_some())`, THEN compute `v["rev"] = Value::String(rev(&v))`! That guarantees (a) changes truncated away by `CAP` never move `rev`, and (b) replacing the 16-byte placeholder with the 16-byte hash adds 0 bytes so `render(&v).len() <= CAP` holds strictly!
  - In `live::cut_timeline(v: &mut Value, over: usize, drop_newest: bool)` (`src/live/mod.rs:194-206`): when `drop_newest` is `true` (`opts.since.is_some()`), pop from the back (newest) so the caller's oldest unread events stay first and a client paging with `since = timeline.last().seq` continues without gaps; when `false`, drain from the front (oldest) as today.
  - In `live::workers::Paths::of` (`src/live/workers.rs:83-91`): for each lexical root (lane dir and `ctx.state.repo`), also push `std::fs::canonicalize(&r).ok()` (deduplicated) so canonicalized worker file paths always resolve to lane-relative labels even behind symlinks.

**Accepted cost.** `board::scan_folder_on` and `status::prd_title` refuse a symlinked `prd.md` (returning `prd: None` / `title: null`) to prevent FIFO/device DoS from untrusted cloned repos. Keeping `at: "closeout"` and putting `phase` in its own optional field preserves `schema: 1` compatibility.

**Acceptance.** Integration & unit test `status_json_and_live_fixes_body_symlink_ci_waiting_cap_rev_and_lane_labels` proves: (1) `status --json` includes `body` on an open `BLOCK-n` and `phase: "ci"` on `CloseoutBlocked { phase: "ci", .. }`, while a feature whose `prd.md` is a symlink to a FIFO (`mkfifo`) completes immediately with `title: null` without blocking; (2) `live::gate::ci` returns `state: "waiting"` and `excerpt: null` after `CiRed` → `CiFixCommitted` before new checks register; (3) `live::snapshot` with `--since` over `CAP` keeps `render(&v).len() <= CAP`, preserves the oldest unread timeline events (dropping newest), and `rev` does not change when only truncated evidence changes; (4) `Paths::relative` strips a canonicalized path through a symlinked `data_dir`.

## 7. FR-5 — `live/redact` hardening and `excerpt` / `gate_failure` fixes

**Today.**
- `live/redact.rs:15-42` (`RULES`) misses URL userinfo (`postgres://user:pass@host`), `Authorization: Basic/Token`, `Cookie`/`Set-Cookie`/`PRIVATE-TOKEN`, PEM private key blocks, and prefixes `sk-proj-`, `glpat-`, `npm_`, `AIza`, `sk_live_`, `xapp-`; its env rule (`:34`) `\w*(?:key|secret|token|password|pass)=` over-redacts `bypass=0` and `compass=north`; and `walk` (`:57-68`) rewrites `evidence[].path` and `attention[].answer`.
- `excerpt.rs:41-42, :145-147` (`ZERO_FAILED`) excludes `0 failed` but still matches JSON `"failed": 0` (`"failed":0`) as a generic failure; `has_red` (`:208-222`) splits on `[';', ':']` and skips 5 params for `38:2`, so 6-field colon-SGR `38:2:<cs>:r:g:b` (`\x1b[38:2::0:0:31m`) reads `31` as red; and `docs/check.rs:257-270` (`docs/project.rs:41` `Project::failure_patterns`) accepts empty-matching `gate_failure` patterns like `""` or `^`.

**Decided 2026-11-27.**

- **Harden `src/live/redact.rs`:**
  - Add high-signal rules to `RULES`:
    - URL userinfo: `(r"(?i)([a-z][a-z0-9+.-]*://[^/\s:@]+:)[^@\s/]+(@)", "${1}[redacted]${2}")`
    - Auth/cookie headers: `Authorization: (Basic|Token) <val>`, `PRIVATE-TOKEN: <val>`, `(?:Set-)?Cookie: <val>`
    - PEM private key header/body lines: `-----BEGIN [A-Z0-9 ]*PRIVATE KEY-----[\s\S]*?(?:-----END [A-Z0-9 ]*PRIVATE KEY-----|$)`
    - Additional vendor prefixes: `\bsk-proj-[A-Za-z0-9_-]+`, `\bglpat-[A-Za-z0-9_-]+`, `\bnpm_[A-Za-z0-9]+`, `\bAIza[0-9A-Za-z_-]{20,}`, `\bsk_(?:live|test)_[0-9A-Za-z]+`, `\bxapp-[0-9A-Za-z-]+`
  - Fix the env rule (`:34`): require a word-start or underscore boundary before `key|secret|token|password|pass` (`\b(?:[A-Za-z0-9]+_)*(?:key|secret|token|password|pass)=`, matching `DB_PASS=`, `API_KEY=`, `password=`, `pass=`, while NOT matching `bypass=0` or `compass=north`).
  - Exempt functional fields from `tokens()` rewriting during `redact::all` (`walk`): skip `tokens()` on `evidence.checkpoints[*].files[*].path` and `attention[*].answer` so UI file paths and answer argv are never corrupted by token patterns.
- **Fix `src/excerpt.rs` and `src/docs/check.rs` (`src/docs/project.rs`):**
  - In `src/excerpt.rs`:
    - Extend `ZERO_FAILED` (or add a companion regex) to also strip `"failed"\s*:\s*0\b` before checking `FAILED.is_match(...)`, so passing JSON summaries like `{"result":"passed","failed":0}` are not treated as generic failures.
    - Fix `has_red` (`:207-223`): split SGR parameters on `;` first; when a `;`-delimited item starts with `38:` or `48:`, skip that entire colon-separated sub-group in one step (whether 5 or 6 fields like `38:2::0:0:31`), while keeping semicolon-form `38;5;N` (skip 3) and `38;2;r;g;b` (skip 5) intact.
  - In `src/docs/check.rs:257-270` and `src/docs/project.rs:41` (`Project::failure_patterns`): reject any `gate_failure` regex where `re.is_match("")` is `true` (`docs check` reports an error `"docs/project.toml [docs] gate_failure {item:?} matches the empty string"`, and `Project::failure_patterns` skips empty-matching patterns too).

**Accepted cost.** Requiring underscore or word-start before `pass|key|token|secret` in env assignments stops matching unseparated compound words like `dbpass=x` (rare) in exchange for never corrupting ordinary flags like `bypass=0` or `compass=north`.

**Acceptance.** Unit & integration test `redact_hardening_and_excerpt_gate_failure_edge_cases` proves: (1) `redact::tokens` redacts URL userinfo, `Authorization: Basic`, `PRIVATE-TOKEN:`, `Cookie:`, PEM key headers, and `sk-proj-`/`glpat-`/`npm_`/`AIza`/`sk_live_`/`xapp-`, leaves `bypass=0` and `compass=north` untouched, and `redact::all` leaves `evidence.checkpoints[0].files[0].path` and `attention[0].answer` uncorrupted even when containing a 41-char path segment; (2) `excerpt::excerpt` returns empty for `{"result":"passed","failed":0}` and for `\x1b[38:2::0:0:31mgreenish\x1b[0m` while still matching `\x1b[31merr\x1b[0m` and `\x1b[38;2;0;0;31;31merr\x1b[0m`; (3) `factory docs check` reports an error when `docs/project.toml` has `gate_failure = ["^"]` or `[""]`.

## 8. Decisions log

No `OPEN-n`. Every fork and keep/drop cut was settled with the owner on 2026-11-27:

- Allow purely additive top-level Rust `use` and JS/TS `import` merges in `testguard` (keyed by full path + local binding, outside test bodies, verified with a regression corpus) and treat stdout-to-file redirects (and not `2>/dev/null` or `< file`) as non-standalone in `shell::first_cmd_standalone` (§3).
- Honor `.gitattributes` `linguist-generated`/`linguist-vendored` via shared `check_attr` / `check_attr_at --source <base_ref>` in `leaks` (anti-tampering) and `ctx refs` with no line-length heuristic (§4).
- Fix `mods/cockpit` `TS2345` (`isCollapsed: false`) and `discover_models` `ETXTBSY` retry, add `# Sent back` (telling respawn to re-drive all rows) + collision-free `cp-{n}.sent-back-{k}/` rotation on checkpoint send-back, and add exact verification phrases to `/prd`, `/slice`, `/polish` (§5).
- Enrich `status --json` with `body`/`phase` + symlink-safe bounded reads in both `board` and `status`; fix `live` post-fix CI `waiting` state, `--since` cap direction, post-cap `rev` with 16-byte placeholder, and canonical lane paths (§6).
- Harden `live/redact` (new shapes, `bypass=` fix, functional field exemptions) and fix `excerpt` JSON `"failed": 0`, colon-SGR `38:2`, and empty-matching `gate_failure` (§7).

## 9. QA plan

**Gate**: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test && claude plugin validate mods/cockpit && claude plugin test mods/cockpit` *(declared in `docs/project.toml`)*
**Surface**: `testguard`, `factory hook`, `factory leaks`, `factory ctx refs`, `mods/cockpit` validation, `checkpoint` send-back, `factory status --json`, `factory live`, `live/redact`, `excerpt`, and `factory docs check`. No GUI: every FR is proven below it.
**Harness**: `tests/guardrails_hardening.rs` (new), `tests/leaks.rs`, `tests/ctx.rs`, `tests/checkpoint_send_back.rs` (new), `tests/status_and_live_hardening.rs` (new), `tests/redact_and_excerpt.rs` (new), `tests/skills_bundle.rs`, `tests/cockpit.rs`, `mods/cockpit/tests/cockpit.test.tsx`.
**Seed state**: fixture diffs with additive vs destructive imports, shell commands with stdout/stderr/stdin redirects, `.gitattributes` `linguist-generated` bundles (on `main` vs branch-only), short checkpoint records with existing `cp-1/` evidence, FIFO-symlinked `prd.md`, `CiRed` → `CiFixCommitted` event streams, over-cap `--since` snapshots, secret/non-secret strings, and `gate_failure = ["^"]` configs.

| FR | Proof | Evidence | Human? |
|---|---|---|---|
| FR-1 | run `testguard_allows_additive_imports_and_denies_removals_and_in_test_edits` and `grep_hook_allows_output_redirects_and_keeps_standalone_denied` | test transcript | no |
| FR-2 | run `leaks_honors_base_gitattributes_and_blocks_branch_self_silencing` and `ctx_refs_honors_gitattributes_generated_unless_vendor_flag_set` | test transcript | no |
| FR-3 | run `checkpoint_send_back_tells_respawn_why_and_rotates_stale_evidence` and `claude plugin validate mods/cockpit` | test transcript | no |
| FR-4 | run `status_json_and_live_fixes_body_symlink_ci_waiting_cap_rev_and_lane_labels` | test transcript | no |
| FR-5 | run `redact_hardening_and_excerpt_gate_failure_edge_cases` | test transcript | no |

No row needs a human, so no `qa-guide.md` is generated.

## 10. Slices

_(Not sliced yet. Run `/slice`.)_

## 11. Non-goals

All items below were explicitly evaluated and permanently dropped on 2026-11-27 (their 11 idea files are closed by this PRD):

- **`claude plugin test mods/cockpit` gate spelling change.** Verified live at `00287c4`: `claude plugin test mods/cockpit` already runs cleanly (`14 pass, 0 fail` in 0.38 s).
- **Minified-line length heuristic in `leaks` / `ctx refs`.** Not handled (2026-11-27): `.gitattributes` `linguist-generated`/`vendored` is the single source of truth; reopen only if repos without `.gitattributes` hit real minified noise.
- **Duplicate `rev` field on `status --json`.** Not handled (2026-11-27): `factory live --json` already owns `rev` for change detection.
- **`factory live` micro-edge-cases.** Not handled (2026-11-27): 600 KiB `attention[].body` cap, 4x `log.jsonl` parse caching, changing `decisions.count` semantics, sub-second `evidence[].stable` jitter, sidecar `.txt` tie-break, `SliceBlocked { block_id: None }`, cherry-pick conflict `reason.source`.
- **Exotic `live/redact` shapes.** Not handled (2026-11-27): double-encoded JSON, multi-line pretty-printed JSON, percent-encoded token tails, `HOME` unset.
- **`excerpt` re-litigations.** Not handled (2026-11-27): first-hit vs last-20 title, `}{` inside JSON messages, `\r` progress split, raw SGR CI log storage, non-exclusive tiers (settled in AD-6).

## 12. Verification notes

Checked against `main` at `00287c4`, 2026-11-27, plus live `claude plugin validate mods/cockpit`, `claude plugin test mods/cockpit`, and `git check-attr --source` (`git 2.56.0`); revised 2026-11-27 after cross-vendor adversarial pass (`claude:opus`, log `20261127-141852`). **Re-verify at slice entry.**

- `src/testguard.rs:47-69` (`is_test_path`), `:111-162` (`parse_diff`), `:219-260` (`hunks_in_chunk`).
- `src/shell.rs:89-135` (`tokenize`, `Tok::Redirect`, `redirect`), `:150-204` (`first_command_standalone`, `first_cmd_standalone`).
- `src/review.rs:298-336` (`skip`, `tool_written`), `:351-386` (`check_attr`, `parse_check_attr`).
- `src/leaks.rs:541-613` (`Report`, `render`), `:837-896` (`scan`, `allowlist_at_ref`).
- `src/ctx/walk.rs:8-91` (`VENDOR_DIRS`, `ROOT_DIRS`, `walk`), `src/ctx/refs.rs:49-105` (`find`).
- `mods/cockpit/tests/cockpit.test.tsx:11-22` (`BAND.props` missing `isCollapsed: false`, causing `TS2345` at `:154`, `:168`).
- `src/provider.rs:266-315` (`discover_models`), `:508-582` (`discover_models_parses_live_outputs_and_kills_hung_probe`, `script` helper).
- `src/checkpoint.rs:301-430` (`run`, `Pack::for_checkpoint`, send-back `missing`/`unproven`, `prepare_lane`).
- `src/pack.rs:427` (`Pack::for_checkpoint`, called in tests `:1237, 1301, 1484, 1711`).
- `src/cockpit.rs:215-268` (`Question`, `question`).
- `src/cli/status.rs:185-187` (`run_json`), `:235-275` (`questions`, `feature_folder`, `prd_title`), `:917` (`QuestionJson` literal).
- `src/docs/board.rs:169-203` (`scan_folder_on`, `:186` `read_to_string`).
- `src/live/mod.rs:89-117` (`snapshot`), `:158-206` (`cap`, `cut_timeline`).
- `src/live/gate.rs:182-230` (`ci`), `src/live/now.rs:188-227` (`ci_wait`).
- `src/live/workers.rs:78-105` (`Paths::of`, `relative`).
- `src/live/redact.rs:15-68` (`RULES`, `all`, `walk`).
- `src/excerpt.rs:41-42` (`FAILED`, `ZERO_FAILED`), `:145-147` (`generic`), `:194-227` (`has_red`).
- `src/docs/check.rs:257-270` & `src/docs/project.rs:41` (`Project::failure_patterns` regex validation).
- `tests/cockpit.rs:276-333` (`status_json_carries_the_board_and_what_the_owner_owes`), `:381-417` (`status_json_gives_a_preflight_ask_the_command_that_answers_it`).
- `tests/skills_bundle.rs:416-436` (pinned skill passages), `tests/leaks.rs` & `tests/ctx.rs` (in `[leaks] allow`).
