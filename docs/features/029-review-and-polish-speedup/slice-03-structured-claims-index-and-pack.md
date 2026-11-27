# Slice 03 — Structured claims index and unanchored pack::collected

**Source**: [`prd.md`](prd.md) → §4 (FR-2). **Depends on**: Slice 01 _(both edit `src/review.rs`)_
**Created**: 2026-11-26

**Progress**: _(written as it lands — what was built, the gate numbers, what a review caught)_

**Slice**: Extends `review::diff_prompt` to ask each pass for a compact `| file:line | severity | claim |` summary table, adds deterministic `collect::extract_claims` + merged `### Claims index` rendering at the top of `## Second opinion` in `collect.md` (sorted `high` → `medium` → `low`), and updates `pack::collected` so `### Claims index` stays inside `collect.md` and never leaks findings into the polish worker's initial `# Collected` pack before the unanchored local review runs.

**Highest-risk**: `pack::collected` (`src/pack.rs:148-159`) currently copies every non-empty line before `### Pass ` into `# Collected`. Stopping before ANY `### ` heading is mandatory to preserve the unanchored local-review invariant.

## Current code / environment

- **`review::diff_prompt`**: `src/review.rs:1446-1451` — currently returns `"Adversarial code review. {what} {against} is in {}: read it, and read any file in the repository you need for context. Report only defects you can point to at file:line with a failure scenario; no praise, no style. Do not edit anything."`.
- **Authorized test in `tests/review.rs`**: `tests/review.rs::no_feature_keeps_the_prompt_as_today` (`:1016-1046`) compares the full `diff_prompt` string with `assert_eq!` at `:1040-1043`. Declared in `prd.md` `authorized_tests`!
- **`testguard` rule on `tests/review.rs` & `tests/stubs/*`**:
  - Only `no_feature_keeps_the_prompt_as_today` (and in Slice 04, `collect_owes_the_fallback_when_every_pass_timed_out`) are authorized in `tests/review.rs`. Do NOT modify any other existing test or merge top-level `use` imports!
  - NEVER modify `tests/stubs/cursor-agent` (`tests/stubs/` is a test path guarded by `testguard`). Write a fresh temporary stub script inside the new test's tempdir instead!
- **`collect::second_opinion`**: `src/collect.rs:247-322` — builds `sections` from `passes.iter().zip(&ran)` and returns `(state, format!("exit: 0\nlog: {}\n{sections}", dir.display()))`.
- **`pack::collected`**: `src/pack.rs:126-165` — iterates `opinion.lines()` with `in_pass = false`; sets `in_pass = true` on `line.strip_prefix("### Pass ")`. If `### Claims index` sits above `### Pass 1`, `:156-158` would copy all claim rows into `# Collected` unless stopped at any `### ` heading!
- **Deliberate non-changes** — *(prd §10 & Slice boundary)*:
  - No LLM post-pass summarizer; table extraction is pure Rust.
  - Leave `skills/polish/SKILL.md` and `skills/bundle.lock` for Slice 04 so all skill edits land in one bundle update.

## Tasks

### 1. Extend `review::diff_prompt` and update authorized test — high — blocked by: —
**Source**: §4 FR-2: "extend `diff_prompt` with one short instruction asking the reviewer to end its response with a `### Summary` markdown table with columns `| file:line | severity | claim |`", §2 Blast radius (`no_feature_keeps_the_prompt_as_today`).

`src/review.rs`, `tests/review.rs`.
1. In `src/review.rs` (`diff_prompt`, `:1446-1451`): keep the exact existing prefix and append one concise sentence:
   `" End with a `### Summary` markdown table (`| file:line | severity | claim |`, severity `high`, `medium`, or `low`; omit the table if there are no defects)."`
2. In `tests/review.rs` (`no_feature_keeps_the_prompt_as_today`, `:1040-1043`, authorized in `prd.md`): update the expected `assert_eq!` string to match the updated `diff_prompt` (touching NO other lines in `tests/review.rs`).

**Acceptance:** `cargo test --test review no_feature_keeps_the_prompt_as_today` passes and `diff_prompt` contains `| file:line | severity | claim |` (fails if the table instruction is removed).

### 2. Add `collect::extract_claims`, render `### Claims index`, and keep `pack::collected` unanchored — high — blocked by: 1
**Source**: §4 FR-2: "Add a pure helper `pub fn extract_claims(pass_n: usize, text: &str) -> Vec<Claim>`", "render a `### Claims index` markdown table (`| severity | file:line | pass | claim |`)", "Keep `# Collected` unanchored (`src/pack.rs:148-159`)".

`src/collect.rs`, `src/pack.rs`.
1. In `src/collect.rs`:
   - Define `#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)] pub enum Severity { High, Medium, Low }` (note: with `High` first in enum order, `Ord` sorts `High < Medium < Low`, so sorting ascending by `severity` puts `High`, then `Medium`, then `Low`!) and `#[derive(Debug, Clone, PartialEq, Eq)] pub struct Claim { pub pass: usize, pub location: String, pub severity: Severity, pub summary: String }`.
   - Implement `pub fn extract_claims(pass_n: usize, text: &str) -> Vec<Claim>`:
     - Scan lines for a pipe-delimited markdown table whose header row has 3 cells (trimmed, lowercased) matching `(location|file|file:line)`, `(severity|sev)`, and `(claim|defect|summary|issue|finding)` (preferring a table under `### Summary` / `## Summary` if multiple tables exist; otherwise matching any table with that 3-column header).
     - Skip the `|---|---|---|` separator row.
     - Parse consecutive `| ... |` rows until a blank/non-table line: trim cells, ignore `"none"`/`"—"`/`"-"` placeholder rows, parse severity case-insensitively (`"high"` → `High`, `"medium"` / `"med"` → `Medium`, `"low"` → `Low`; skip rows with unrecognized severity).
     - Sanitize cells (collapse whitespace/newlines to a single space, escape literal `|` as `\|` when rendering).
   - In `second_opinion` (`:305-321`): for every counted pass `(i + 1, &text)`, extend `claims` with `extract_claims(i + 1, &text)`. Sort `claims` by `(c.severity, c.location.as_str(), c.pass)`. When `!claims.is_empty()`, build:
     ```markdown
     ### Claims index

     | severity | file:line | pass | claim |
     |---|---|---|---|
     | high | src/a.rs:10 | 2 | ... |
     ```
     and place `\n{index}` right after `exit: 0\nlog: {}\n` and before `{sections}`. When `claims.is_empty()`, emit nothing (output matches today).
2. In `src/pack.rs` (`collected`, `:148-159`):
   - Rename `in_pass` to `in_section` (or check `if line.starts_with("### ") { in_section = true; }`):
     ```rust
     for line in opinion.lines() {
         if let Some(pass) = line.strip_prefix("### Pass ") {
             in_section = true;
             if pass.ends_with(" — counted") || pass.ends_with(" — unreviewed") {
                 status.push(format!("- Pass {pass}"));
             }
         } else if line.starts_with("### ") {
             in_section = true;
         } else if !in_section && !line.trim().is_empty() {
             status.push(line.to_string());
         }
     }
     ```
   - Add a unit test in `src/pack.rs` proving `pack::collected` strips `### Claims index` and all its claim rows from `# Collected` while preserving `exit: 0`, `log: ...`, and `- Pass 1 ... — counted` lines!

**The trap**: Double-check `collect::passes_and_log` (`src/collect.rs:42-55`) — it filters for `l.starts_with("### Pass ")` and `l.strip_prefix("log: ")`, so putting `### Claims index` AFTER `log: ...` and BEFORE `### Pass 1` keeps `passes_and_log` working identically!

**Acceptance:** Unit tests in `src/collect.rs` and `src/pack.rs` prove claim extraction, severity sorting, empty-table omission, and unanchored `# Collected` stripping (fails if `pack::collected` leaks any claim row or if sorting is wrong).

### 3. Add integration test in `tests/review_claims_and_since.rs` — high — blocked by: 2
**Source**: §4 FR-2 Acceptance.

`tests/review_claims_and_since.rs` (new file).
Add `collect_merges_structured_claims_index_sorted_by_severity`:
1. Write a fresh executable node/shell stub script inside the test's tempdir (do NOT touch `tests/stubs/cursor-agent`) that handles `--list-models` and emits distinct `### Summary` tables per pass (Pass 1 returns a `low` and a `medium` row; Pass 2 returns a `high` and a `low` row) when run over a 2-pass fixture diff.
2. Run `factory review collect --base main` using that tempdir stub on `PATH`.
3. Assert `collect.md` contains `### Claims index` BEFORE `### Pass 1`, with the `high` row (pass `2`) first, `medium` row (pass `1`) second, and `low` rows after, and assert `factory::pack::collected(&text, Path::new("collect.md"))` contains `- Pass 1` and `- Pass 2` and does NOT contain `"### Claims index"` or any of the claim summaries!
4. Run a second case where the stub returns plain prose with no summary table and assert `collect.md` omits `### Claims index` while retaining the full `### Pass` prose.

**Acceptance:** `cargo test --test review_claims_and_since collect_merges_structured_claims_index_sorted_by_severity` passes; removing `extract_claims`, sorting, or the `pack::collected` guard breaks the test.

### 4. Verify — medium — blocked by: 3
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 5. Prove — at the checkpoint — blocked by: 4
**Source**: §8 QA plan FR-2 row.
- **FR-2**: run `cargo test --test review_claims_and_since collect_merges_structured_claims_index_sorted_by_severity -- --nocapture` and keep the transcript. Expected: passes proving merged `### Claims index` sorted `high` → `medium` → `low`, clean omission when no table is returned, and unanchored `pack::collected` stripping.

**Acceptance:** after the checkpoint, the FR-2 row has transcript evidence in this slice's **Progress**.
