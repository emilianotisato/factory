# Slice 04 — Unreviewed failure causes, branch-scoped --since, and polish skill

**Source**: [`prd.md`](prd.md) → §5 (FR-3a & FR-3b collect/closeout/skill) and §4/§6 (`skills/polish/SKILL.md` + `skills/bundle.lock`). **Depends on**: Slice 02 _(uses `state.toml` `reviewed_head`)_ and Slice 03 _(both edit `src/review.rs`, `src/collect.rs`, `tests/review.rs`, `tests/review_claims_and_since.rs`)_
**Created**: 2026-11-26

**Progress**: _(written as it lands — what was built, the gate numbers, what a review caught)_

**Slice**: Classifies unreviewed pass/probe failures (`usage limit > auth > no such model > timeout > stopped`) directly on `collect.md`'s Plan line without false-positiving healthy exit-0 probes; wires `collect::head_of` into `closeout::ship` to stamp `reviewed_head` in `state.toml`; intersects `collect --since` paths with `<base>...HEAD` (falling back cleanly to `<base>...HEAD` if `--since` was rewritten by a rebase); and updates `skills/polish/SKILL.md` (+ regenerates `skills/bundle.lock`) to use `reviewed_head`, `### Claims index`, and `factory review settled <NNN>`.

## Current code / environment

- **`collect::second_opinion` unreviewed line**: `src/collect.rs:314-317` — currently formats `let reason = format!("no pass of {} counted", passes.len());`.
- **`review::probe_resolved`**: `src/review.rs:671-739` — on exit != 0 returns `"{what} exited {code}: {first}"`; on Cursor exit 0 checks `text.contains("login") || text.contains("Authentication required")`.
- **Authorized test in `tests/review.rs`**: `tests/review.rs::collect_owes_the_fallback_when_every_pass_timed_out` (`:698-721`) asserts `plan.contains("\nsecond opinion: fallback owed — unreviewed: no pass of 2 counted\n")` at `:704`. Declared in `prd.md` `authorized_tests`!
- **`collect::collect` `--since` path**: `src/collect.rs:123-135` — computes `range = opts.range()` and calls `review::numstat(root, range, &[])?` directly (fails if `since` commit no longer exists, and includes merged `main` files).
- **`closeout::ship`**: `src/closeout.rs:1055-1168` — runs `docs.ship(&feature)` and commits `ship PRD-{feature}`. `run.dir().join(COLLECT_FILE)` (`collect.md`) holds `<!-- head: <sha> -->` on line 1.
- **`skills/polish/SKILL.md` & `tests/skills_bundle.rs:416-436`**:
  - `tests/skills_bundle.rs:428-433` asserts these 4 passages exist **uniquely** in `skills/polish/SKILL.md` §1:
    1. `"add `--feature <NNN>`, so every pass prompt carries what is settled"`
    2. `"--since <commit>"`
    3. `"git log -1 --format=%H -G '^reviewed' -- docs/features/<NNN>-*/state.toml"`
    4. `"A finding that contradicts an entry names it"`
  - Keep all 4 passages intact! Update `skills/bundle.lock` via `FACTORY_BUNDLE_WRITE=1 cargo test --test skills_bundle` (`v0.18.0` is untagged, so no version bump is needed).
- **Deliberate non-changes** — *(prd §10 Non-goals)*:
  - Do NOT change `leaks` scan scope (`<base>...HEAD`).
  - Do NOT modify `tests/stubs/cursor-agent` (use fresh tempdir stubs in `tests/review_claims_and_since.rs`).

## Tasks

### 1. Add deterministic `review::classify_failure` and surface causes in `probe_resolved` + `collect::second_opinion` — high — blocked by: —
**Source**: §5 FR-3a: "Add `pub fn classify_failure(ran: &[Ran]) -> Option<&'static str>`", "strict deterministic priority order: `usage limit > auth > no such model > timeout > stopped`", "probe_resolved ... never bare `429` or `quota` on exit 0".

`src/review.rs`, `src/collect.rs`, `tests/review.rs`.
1. In `src/review.rs`:
   - Add a helper `classify_error_text(text: &str) -> Option<&'static str>` matching case-insensitively:
     - `"usage limit"`: `"usage limit"`, `"rate limit"`, `"quota"`, `"too many requests"`, `"429"`, `"out of credits"`, `"credit balance"`
     - `"auth"`: `"authentication required"`, `"not logged in"`, `"unauthorized"`, `"invalid api key"`, `"login"`, `"401"`, `"403"`
     - `"no such model"`: `"model not found"`, `"no such model"`, `"unknown model"`, `"invalid model"`, `"not in --list-models"`
   - Add `pub fn classify_failure(ran: &[Ran]) -> Option<&'static str>`:
     - Inspect every failed `Ran` (`Err(e)` via `classify_error_text(&e.to_string())`, or `Ok(o)` where `!body(o, ...).1`: check `o.parsed.error` + `o.stderr` via `classify_error_text`, else `Ended::Killed => Some("timeout")`, `Ended::Stopped => Some("stopped")`).
     - Return the highest-priority cause seen across all failed passes: `"usage limit" > "auth" > "no such model" > "timeout" > "stopped"`.
   - Also add `pub fn first_failure_detail(ran: &[Ran]) -> Option<String>` returning the first non-empty error/stderr line from a failed `Ran` (trimmed, truncated safely at char boundary to 120 chars) for unclassified failures.
   - In `probe_resolved` (`:723-731`):
     - On `Ended::Exited(code) if code != Some(0)`: if `classify_error_text(&text) == Some("usage limit")`, return `Err(format!("usage limit: {first}"))`.
     - On exit 0 for `Provider::Cursor`: check explicit phrases (`let lower = text.to_ascii_lowercase(); lower.contains("hit your usage limit") || lower.contains("usage limit reached")`) -> `Err(format!("usage limit: {first}"))` (NEVER match bare `"429"` or `"quota"` on exit 0!).
2. In `src/collect.rs` (`second_opinion`, `:314-317`):
   - When `counted == 0`:
     ```rust
     let reason = match review::classify_failure(&ran) {
         Some(cause) => format!("{cause} (no pass of {} counted)", passes.len()),
         None => match review::first_failure_detail(&ran) {
             Some(detail) => format!("no pass of {} counted: {detail}", passes.len()),
             None => format!("no pass of {} counted", passes.len()),
         },
     };
     ```
3. In `tests/review.rs` (`collect_owes_the_fallback_when_every_pass_timed_out`, `:704`, authorized in `prd.md`):
   - Update `:704` to `"\nsecond opinion: fallback owed — unreviewed: timeout (no pass of 2 counted)\n"`.

**The trap**: Do NOT match `"429"` or `"quota"` on exit-0 probe output — a healthy `--list-models` or `--version` containing `429` or `quota` in a model name or build string must still pass `probe_resolved`!

**Acceptance:** `collect_owes_the_fallback_when_every_pass_timed_out` passes with `timeout (no pass of 2 counted)`; unit tests prove `classify_failure` priority order (`usage limit > auth > no such model > timeout > stopped`) and exit-0 probe accepts `model-429-quota` (fails if priority is wrong or exit-0 rejects `model-429-quota`).

### 2. Add `collect::head_of`, branch-intersected `--since` with rebase fallback, and `closeout::ship` `reviewed_head` recording — high — blocked by: —
**Source**: §5 FR-3b: "Add `pub fn head_of(text: &str) -> Option<&str>`", "In `closeout::ship`: write `reviewed_head=<sha>`", "In `collect::collect`: check whether `c` resolves to a valid commit... Filter the since files to only those whose path is present in the branch's `<base>...HEAD` numstat".

`src/collect.rs`, `src/closeout.rs`.
1. In `src/collect.rs`:
   - Add `pub fn head_of(text: &str) -> Option<&str>`: parse line 1 `text.lines().next()?.strip_prefix("<!-- head: ")?.strip_suffix(" -->")` (non-empty trimmed).
   - In `collect()` (`:123-135`):
     - When `let Some(since_sha) = opts.since`:
       - Verify `since_sha` is a valid commit in `root` via `Command::new("git").args(["rev-parse", "--verify", "--quiet", &format!("{since_sha}^{{commit}}")]).current_dir(root)...` (success).
       - If valid: set `range = Range::Since(since_sha)`, compute `let base_files: HashSet<String> = review::numstat(root, Range::Base(opts.base), &[])?.into_iter().map(|(p, _)| p).collect();`, compute `let since_files = review::numstat(root, range, &[])?;`, and filter `since_files.into_iter().filter(|(p, _)| base_files.contains(p)).collect()` before passing to `review::skip(root, ...)`.
       - If NOT valid (rebased away / unknown SHA): fall back cleanly to `range = Range::Base(opts.base)` and `review::numstat(root, range, &[])?`!
2. In `src/closeout.rs` (`ship`, `:1055-1168`):
   - Before calling `docs.ship(&feature)` (and before the ship commit): if `let Ok(text) = fs::read_to_string(run.dir().join(COLLECT_FILE))` and `let Some(head) = collect::head_of(&text)`:
     - Check whether the feature's `state.toml` currently has `reviewed` set (`crate::docs::state::read(&feature_dir).ok().flatten().is_some_and(|s| s.reviewed.is_some())`).
     - If so, record `let _ = docs.set(&feature, &[("reviewed_head", head)]);` (best-effort or logged on failure, never blocking ship if `head` is valid).

**The trap**: Keep `tests/review.rs:1076` (`collect_since_reviews_only_later_commits`) passing: when `since_sha` is valid and `later-b.txt`/`later-c.txt` were added on the branch, both are in `base_files` (`main...HEAD`), so `range.spec()` still prints `against {r}..HEAD (since the last review)` with 2 files, 200 lines!

**Acceptance:** Merging an unrelated commit on `main` after `since` excludes `main`'s files from `collect --base main --since <sha>`, a dead/rebased `--since` falls back cleanly to `main...HEAD` (exit 0), and `collect::head_of` extracts the SHA from line 1 (fails if intersection or rebase fallback is removed).

### 3. Update `skills/polish/SKILL.md` and regenerate `skills/bundle.lock` — high — blocked by: 1, 2
**Source**: §4 FR-2, §5 FR-3b, §6 FR-4, §2 Blast radius (`skills/polish/SKILL.md` & `skills/bundle.lock`).

`skills/polish/SKILL.md`, `skills/bundle.lock`.
1. In `skills/polish/SKILL.md`:
   - Line 33 (§1 Collect): keep all 4 passages asserted by `tests/skills_bundle.rs:428-433` verbatim (`add \`--feature <NNN>\`, so every pass prompt carries what is settled`, `--since <commit>`, `git log -1 --format=%H -G '^reviewed' -- docs/features/<NNN>-*/state.toml`, `A finding that contradicts an entry names it`), and prepend the `reviewed_head` preference: when `state.toml` sets `reviewed_head`, pass `--since <reviewed_head>`; otherwise fall back to `git log -1 --format=%H -G '^reviewed' -- docs/features/<NNN>-*/state.toml`.
   - Lines 72-81 (§1 Fallback brief): mention `factory review settled <NNN>` once as the command that prints `<the settled block>` for fallback briefs.
   - Line 89 (§2 Verify and dedup): mention starting dedupe from `### Claims index` when present in `collect.md` and verifying each row against `### Pass` and the code.
2. Regenerate `skills/bundle.lock` by running `FACTORY_BUNDLE_WRITE=1 cargo test --test skills_bundle` (do NOT hand-edit `skills/bundle.lock` and do NOT bump `Cargo.toml` — `v0.18.0` is untagged).

**The trap**: Every passage added to `skills/polish/SKILL.md` (`reviewed_head`, `factory review settled <NNN>`, `### Claims index`) must appear **exactly once**, and all existing assertions in `tests/skills_bundle.rs` must stay green!

**Acceptance:** `cargo test --test skills_bundle` passes; `skills/polish/SKILL.md` contains `reviewed_head`, `factory review settled <NNN>`, and `### Claims index` each exactly once (fails if any is deleted or duplicated, or if `skills/bundle.lock` is stale).

### 4. Add FR-3 & full FR-4 integration tests in `tests/review_claims_and_since.rs` — high — blocked by: 3
**Source**: §5 FR-3 Acceptance, §6 FR-4 Acceptance, §8 QA plan FR-3 & FR-4 rows.

`tests/review_claims_and_since.rs`.
1. Add `collect_classifies_unreviewed_causes_and_since_ignores_merged_main_and_uses_reviewed_head`:
   - Temporary stub failing all passes with `"You've hit your usage limit"` → `collect.md` Plan line contains `fallback owed — unreviewed: usage limit (no pass of 2 counted)`.
   - Temporary stub failing with `"Authentication required"` → `auth (no pass of ... counted)`; `"model not found"` → `no such model (no pass of ... counted)`.
   - Temporary stub returning exit 0 with `--list-models` listing `model-429-quota` → `probe_resolved` succeeds (no false positive!).
   - Branch with `reviewed=2026-11-26` + `reviewed_head=<r>`: commit a polish fix after `<r>` on the branch, commit an unrelated file `main-only.txt` on `main`, merge `main` into the feature branch → `factory review collect --base main --since <r>` includes the polish fix and EXCLUDES `main-only.txt`; clearing `reviewed=` clears `reviewed_head`.
   - Running `factory review collect --base main --since deadbeefdeadbeef` (unknown/rebased SHA) exits `0` and falls back to `main...HEAD`.
   - Assert `skills/polish/SKILL.md` contains `reviewed_head` exactly once and `### Claims index` exactly once.
2. Add `settled_includes_answered_asks_prints_via_cli_and_signals_are_unified` (covering the PRD §8 FR-4 row end-to-end):
   - Verifies `settled::block` includes answered `Kind::Ask` (`- AD-n — <title>: answered — <answer>`) and omits unanswered `Kind::Ask`.
   - Verifies `factory review settled 001` matches `settled::for_feature` and exits `0`, and `factory review settled 999` exits `1`.
   - Verifies `skills/polish/SKILL.md` contains `factory review settled <NNN>` exactly once.
   - Verifies `src/review.rs` and `src/checkpoint.rs` contain zero `sigaction` calls and `crate::signals` is wired in both (pairing with Slice 01's subprocess `SIGTERM` kill test).

**Acceptance:** `cargo test --test review_claims_and_since` passes; reverting any FR-3 or FR-4 behavior makes the corresponding assertion fail.

### 5. Verify — medium — blocked by: 4
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 6. Prove — at the checkpoint — blocked by: 5
**Source**: §8 QA plan FR-3 & FR-4 rows.
- **FR-3**: run `cargo test --test review_claims_and_since collect_classifies_unreviewed_causes_and_since_ignores_merged_main_and_uses_reviewed_head -- --nocapture` and keep the transcript. Expected: passes proving failure-cause classification (`usage limit`, `auth`, `no such model`), healthy `model-429-quota` probe, `reviewed_head` + merged-`main` exclusion, and rebase fallback.
- **FR-4**: run `cargo test --test review_claims_and_since settled_includes_answered_asks_prints_via_cli_and_signals_are_unified -- --nocapture` and keep the transcript. Expected: passes proving answered asks in settled block, `factory review settled <NNN>`, `SKILL.md` unique passage, and unified signals wiring.

**Acceptance:** after the checkpoint, the FR-3 and FR-4 rows have transcript evidence in this slice's **Progress**.
