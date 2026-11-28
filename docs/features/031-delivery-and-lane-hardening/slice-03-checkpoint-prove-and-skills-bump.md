# Slice 03 — Checkpoint Prove rules, local PR note, and 0.20.0 bump

**Source**: [`prd.md`](prd.md) → §4 (FR-2) and §6 (FR-4 PR-body note, `skills/deliver/SKILL.md`, `0.20.0` version bump). **Depends on**: Slice 01 (same files: `src/pack.rs`, `src/closeout.rs`, `tests/delivery_hardening.rs`)
**Created**: 2026-11-27

**Progress**: _(written as it lands — what was built, the gate numbers, what a review caught)_ · cp-2/FR-1/transcript.txt · cp-2/FR-2/checkpoint.txt · cp-2/FR-2/skills_bundle.txt · cp-2/FR-4/delivery_hardening.txt · cp-2/FR-4/cockpit.txt

**Slice**: Finishes FR-2 and FR-4: defers checkpoint Prove rows for FRs still owned by incomplete slices (scanning `Prove` + pre-`Depends on` `**Source**:`), lets clean-tree checkpoint `run` Prove rows cite the full green gate log file, adds the two UI Prove rules to `skills/slice/SKILL.md`, adds a token-free squash-merge skip-CI note to `closeout` PR bodies (`ci = local`) and `skills/deliver/SKILL.md`, and bumps `0.19.0` → `0.20.0` with an offline-safe `skills/bundle.lock` relock. Leaves FR-3 (`reviewed_head` & `docs log add`) to Slice 04.

**Highest-risk**: (1) NEVER write a literal `[skip ci]` inside `pr-body.md` (squash-merge with PR description would itself skip `main` workflows!); (2) scan ONLY the part of `**Source**:` BEFORE `**Depends on**` in `handed_qa` so dependency rationale like `Depends on: Slice 02 (so FR-4 ...)` never defers a finished FR; (3) keep `0.19.0` → `0.20.0` synchronized across all 6 versioned files.

## Current code / environment

- **`src/pack.rs:749-771`**: `handed_qa(repo, board, covers, prd_text)` collects `FR-n` ids from `covers`'s `Prove` sections without checking remaining slices whose `status.as_deref() != Some("done")`.
- **`assets/contract-checkpoint.md:14-17`**: Step 2 (`**Prove**`) requires running every `run` row from scratch even when step 1 made zero edits and the full green gate log (named in `# Gate transcript`'s HTML comment at `src/pack.rs:522-524`) already ran that test.
- **`src/closeout.rs:1005-1015, 1241-1255`**: `open_pr` writes `fallback_body` if missing and calls `flag_body(run, &body)?` before creating/editing the PR. `local: bool` is already a parameter of `open_pr` (`:1216`).
- **`skills/slice/SKILL.md:58`**: Prove bullet under §2 (`Design the slices`).
- **`skills/deliver/SKILL.md:45-46`**: §3 step 4 closing note for `ci = "local"`.
- **Version `0.19.0` → `0.20.0`**: `Cargo.toml`, `Cargo.lock`, `skills/bundle.lock` (regenerated offline-safely via `FACTORY_BUNDLE_WRITE=1 cargo test --test skills_bundle` once `Cargo.toml` is `0.20.0`), `mods/cockpit/.claude-plugin/plugin.json:3`, `mods/cockpit/hooks/register.tsx:5`, `mods/cockpit/tests/cockpit.test.tsx:5` (authorized in `prd.md` frontmatter).
- **Deliberate non-change**: Do NOT modify `skills/polish/SKILL.md` (`tests/review_claims_and_since.rs:363` pins `reviewed_head` count == 1).

## Tasks

### 1. Defer incomplete-slice FRs in `pack::handed_qa` — high — blocked by: —
**Source**: §4 FR-2: "In `handed_qa`, when `!board.all_done`: scan every slice in `board.slices` whose `status.as_deref() != Some(\"done\")`, collecting every `FR-n` cited in: (1) its `Prove` section (`prove_section(&slice_text)`), AND (2) the `**Source**:` line strictly before `**Depends on**`... Exclude those incomplete `FR-n` ids from `named`, and append a one-line note to the rendered `# QA rows` text naming any deferred FRs (e.g., `(deferred until remaining slice(s) finish: FR-3)`)."

`src/pack.rs`.
- Add helper `source_before_depends(slice_text: &str) -> &str` finding the `**Source**:` line and slicing strictly before `**Depends on**` (if present).
- In `handed_qa(repo, board, covers, prd_text)`:
  - When `!board.all_done`: collect `incomplete: Vec<String>` of all `FR-n` ids from `prove_section(&text)` and `source_before_depends(&text)` across every slice in `board.slices` where `s.status.as_deref() != Some("done")`.
  - Compute `deferred: Vec<String>` = ids in `named` that also appear in `incomplete` (in order, deduped), and remove them from `named`.
  - Call `qa_rows(prd_text, Some(&named[..]))`.
  - When `!deferred.is_empty()`, append `\n(deferred until remaining slice(s) finish: {})` to the returned QA table text.
- **The trap**: Existing `src/pack.rs:1242-1321` tests construct `board.slices` with `status: Some("done")` on every slice, so they stay 100% untouched and green!
- Add unit tests in `src/pack.rs` proving: (a) covered Slice 02 (`done`) citing `FR-3` + pending Slice 04 citing `FR-3` in `Prove` or pre-`Depends on` `**Source**:` defers `FR-3` and prints the deferred note; (b) pending Slice 03 mentioning `FR-4` ONLY after `**Depends on**:` does NOT defer `FR-4`; (c) when `board.all_done` is `true`, `FR-3` is handed normally.

**Acceptance:** Unit tests in `src/pack.rs` prove all three cases above (fails if incomplete-slice deferral or the pre-`Depends on` split is removed).

### 2. Add clean-tree full-gate-log citation rule to `assets/contract-checkpoint.md` — high — blocked by: —
**Source**: §4 FR-2: "Update step 2 (`**Prove**`) of `assets/contract-checkpoint.md`: when step 1 (Leaks) made zero code edits AND a `run` row names a test that the checkpoint's green gate already executed and passed (verified by checking the full gate log file at the path named in `# Gate transcript`'s header comment, not only the 200-line tail!), the checkpoint worker may cite the gate log instead of re-running that test — writing a short citation file (e.g., `gate-transcript.txt` naming the gate log path and the passing test line(s)) into `$FACTORY_EVIDENCE/cp-{n}/<key>/`... If step 1 edited any file, or if the row drives anything outside the gate... the row MUST still be executed fresh; the checkpoint's single mutation check MUST still run as before."

`assets/contract-checkpoint.md`, `tests/skills_bundle.rs`.
- Add the concise clean-tree full-gate-log citation rule inside step 2 (`**Prove**`) of `assets/contract-checkpoint.md`, clearly requiring: (a) step 1 left the tree unchanged, (b) verify the passing test line in the full gate log file path named in `# Gate transcript`'s header comment (not only the 200-line tail), (c) write a short citation file into `$FACTORY_EVIDENCE/cp-{n}/<key>/`, (d) non-gate drives and the checkpoint's single mutation check still run fresh.
- Add a test in `tests/skills_bundle.rs` using `assert_unique_passage` to assert the unique gate-log citation passage occurs exactly once in `assets/contract-checkpoint.md`.

**Acceptance:** `cargo test --test skills_bundle` asserts the unique gate-log citation passage occurs once in `assets/contract-checkpoint.md` (fails if the citation rule is removed).

### 3. Add token-free squash-merge skip-CI note to `closeout` PR body — high — blocked by: —
**Source**: §6 FR-4: "NEVER put the literal `[skip ci]` token inside `pr-body.md`! ... In `src/closeout.rs::open_pr`: when `local == true`, ensure `pr-body.md` carries a token-free note (idempotently, same pattern as `flag_body`): `> Local CI mode: remove any skip-CI tags from the squash-merge commit message on merge so \`main\` workflows still run.`"

`src/closeout.rs`.
- Add a pure helper (e.g. `local_ci_note_body(text: &str) -> String` / `ensure_local_ci_note(body: &Path) -> Result<()>`) modeled on `flag_body` (`src/closeout.rs:1005-1015`):
  - Constant note line: `> Local CI mode: remove any skip-CI tags from the squash-merge commit message on merge so \`main\` workflows still run.`
  - **The trap**: Verify the constant contains NO literal bracketed skip-CI token (assert `!text.contains("[skip ci]")` in tests!).
  - Idempotent: if the body already contains the note line, leave unchanged; otherwise append (or prepend right after the header/banner) cleanly.
- Call it in `open_pr` (`src/closeout.rs:1251`) right after `flag_body(run, &body)?` ONLY when `local` is `true`.
- Add unit tests in `src/closeout.rs` proving `local == true` adds the note idempotently without ever emitting literal `[skip ci]`, and `local == false` does not add it.

**Acceptance:** Unit tests in `src/closeout.rs` prove the local-CI note is added once (idempotent on repeat), omitted in remote mode, and never contains the literal `[skip ci]` token (fails if `ensure_local_ci_note` is removed or emits literal `[skip ci]`).

### 4. Update `skills/slice/SKILL.md` & `skills/deliver/SKILL.md`, bump `0.19.0` → `0.20.0`, relock bundle, and add integration tests — high — blocked by: 1, 2, 3
**Source**: §4 FR-2, §6 FR-4, & §2 Blast radius: "Under §2 (`Design the slices`, right under the Prove bullet at `:58`), add: (1) Each form a slice adds gets one empty submit and one narrow-screen drive (375 px). (2) A Prove row that picks between options seeds a state where the options give different results... In `skills/deliver/SKILL.md:46`: extend the `ci = \"local\"` closing note with: `Also strip any skip-CI lines from GitHub's default squash-merge commit message so \`main\` workflows still run on merge.`... Bump `0.19.0` → `0.20.0` across `Cargo.toml`, `Cargo.lock`, `mods/cockpit/.claude-plugin/plugin.json`, `mods/cockpit/hooks/register.tsx`, and `mods/cockpit/tests/cockpit.test.tsx`, and run `FACTORY_BUNDLE_WRITE=1 cargo test --test skills_bundle`."

`skills/slice/SKILL.md`, `skills/deliver/SKILL.md`, `Cargo.toml`, `Cargo.lock`, `skills/bundle.lock`, `mods/cockpit/.claude-plugin/plugin.json`, `mods/cockpit/hooks/register.tsx`, `mods/cockpit/tests/cockpit.test.tsx`, `tests/skills_bundle.rs`, `tests/delivery_hardening.rs`.
- Edit `skills/slice/SKILL.md:58` to add both UI Prove rules concisely.
- Edit `skills/deliver/SKILL.md:46` to append the squash-merge skip-CI reminder.
- Bump `0.19.0` → `0.20.0` in all 5 versioned files and run `FACTORY_BUNDLE_WRITE=1 cargo test --test skills_bundle` to rewrite `skills/bundle.lock` at `0.20.0`.
- In `tests/skills_bundle.rs`, add `assert_unique_passage` checks for:
  - both UI Prove rules in `skills/slice/SKILL.md`
  - the squash-merge reminder in `skills/deliver/SKILL.md`
- In `tests/delivery_hardening.rs`, add:
  - `checkpoint_defers_pending_slice_frs_and_bundle_carries_prove_rules` (proving `handed_qa` deferral, pre-`Depends on` isolation, `all_done` full handoff, and bundle rules)
  - `docs_check_feature_ctx_def_generated_leaks_attributes_and_local_pr_note` (proving `docs check [FEATURE]`, `ctx::def` linguist filter, `leaks` local `core.attributesFile` isolation vs committed `.gitattributes` on `main`, and `closeout` token-free local PR-body note).

**Acceptance:** `cargo test --test skills_bundle`, `cargo test --test delivery_hardening`, and `claude plugin validate mods/cockpit && claude plugin test mods/cockpit` all pass at `0.20.0` (fails if any skill passage, deferral check, local PR note, or version file is reverted).

### 5. Verify — medium — blocked by: 1, 2, 3, 4
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 6. Prove — at the checkpoint — blocked by: 5
**Source**: §8 QA plan FR-2 and FR-4 rows: "run `checkpoint_defers_pending_slice_frs_and_bundle_carries_prove_rules` and `cargo test --test skills_bundle`" and "run `docs_check_feature_ctx_def_generated_leaks_attributes_and_local_pr_note` and `claude plugin test mods/cockpit`".

At Checkpoint 2, run `cargo test --test delivery_hardening checkpoint_defers_pending_slice_frs_and_bundle_carries_prove_rules`, `cargo test --test skills_bundle`, `cargo test --test delivery_hardening docs_check_feature_ctx_def_generated_leaks_attributes_and_local_pr_note`, and `claude plugin test mods/cockpit`, recording the transcripts in **Progress**.

**Acceptance:** Checkpoint 2 transcripts in **Progress** confirm every FR-2 and FR-4 test and cockpit plugin test passes (would fail if deferral, contract/skill text, CLI/ctx/leaks hardening, local PR note, or `0.20.0` sync were broken).
