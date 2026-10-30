# Slice 01 — CLI: unsliced refusal, author from git, optional Blast radius

**Source**: [`prd.md`](prd.md) → §4 FR-2 (decision 5), §8 FR-6 (the scaffold half), §9 FR-7, §11 QA plan. **Depends on**: — (independent of slice 02: this slice touches Rust, the scaffold and tests; 02 touches `skills/`, `Cargo.toml` and the bundle lock)
**Created**: 2026-10-30

**Progress**:
- Built: `feature()` in `src/docs/preflight.rs` refuses `not sliced — /slice NNN` when the board has no slices. `scaffold::new_from` fills `author:` from `git config user.name` (trimmed; bare when `frontmatter::parse` reads it back unchanged, else double-quoted; placeholder kept when empty, unreadable or git fails). The scaffold's §2 carries `_(Delete this section and renumber when nothing outside the FRs can break.)_`.
- Tests: `an_unsliced_feature_is_refused_before_a_run_starts` (`tests/preflight.rs`), `the_author_comes_from_git_user_name` (also `[Ada]`, `1984`, a quote-led name, a blank name) and `the_scaffold_says_blast_radius_may_go` (`tests/docs_v3.rs`); `new_scaffolds_…` now pins `author` as `_(name)_`.
- Inner loop: 1 run, 0 red, ~2.5 min.
- Prove (FR-2 preflight half, FR-6, FR-7): left for the checkpoint.
- **Checkpoint 1 (2026-10-30), run with the lane's 0.11.0 build in scratch repos made by `factory init`:**
  - FR-2 (preflight half): `cp-1/fr2-preflight-test.txt` (the test passes) and `cp-1/fr2-preflight-unsliced.txt`, where `factory docs preflight 001` on an unsliced feature prints `✗ not sliced — /slice 001` and exits 1.
  - FR-6: `cp-1/fr6-blast-radius.txt`. §2 reads `_(Delete this section and renumber when nothing outside the FRs can break.)_`. After deleting §2 and renumbering 3–8 to 2–7, `docs check` reports 0 errors, and it is `clean` once the scratch repo declares a gate.
  - FR-7: `cp-1/fr7-ada.txt` (`user.name = Ada` gives `author: Ada`) and `cp-1/fr7-noname.txt` (no `user.name` gives `author: _(name)_`).

**Slice**: After this slice, `factory docs preflight` (and so `factory run`) refuses a live feature with no slices, instead of starting a run that ends `nothing ready`. `factory docs new` writes the author from `git config user.name`, and the scaffold says §2 Blast radius may be deleted. The skill text for all three is slice 02's. **No version bump here**: slice 02 bumps to 0.11.0 once for the whole PR, because a second bump is refused (`src/skills.rs:161-178`).

## Current code / environment

- **The refusal goes in `feature()`**: `src/docs/preflight.rs:245`, called from `run()` at `:134-146` with `slices = f.board_slices()` already computed (`:136`). `refuse` lines print `✗` and make `factory run` exit 1 before any run exists (`src/cli/run.rs:93-99`). Today nothing counts slices in preflight (`:255-345`).
- **No test relies on a zero-slice pass**: on 2026-10-30 a probe added the refusal and the whole suite stayed green. So nothing breaks, and nothing yet pins it.
- **The fixture's feature has slices**: `Fixture::new` builds a v3 repo (`tests/harness/mod.rs:138-146`) whose feature 001 (`FEATURE_DIR`, `:16`) has 3 slices (`tests/docs_v3.rs:46-51` reads them). A zero-slice test must strip them: delete the `slice-*.md` files and their `[slices.*]` tables from `state.toml`.
- **`factory docs status` already says `not sliced`** for an empty slice list (`src/cli/docs.rs:638-644`). Use the same words.
- **The render**: `src/docs/scaffold.rs:296-301` replaces `{{NNN}}`, `{{SLUG}}`, `{{TITLE}}`, `{{DATE}}` and `{{GATE}}` in `PRD` (`include_str!` of `assets/docs/prd.scaffold.md`, `:16`). `author: _(name)_` is a literal on `assets/docs/prd.scaffold.md:8`. `scaffold.rs` already imports `std::process::Command` (`:4`).
- **Tests already run git with no global config**: `docs_in` sets `GIT_CONFIG_GLOBAL=/dev/null` and `GIT_CONFIG_NOSYSTEM=1` (`tests/docs_v3.rs:11-20`), and the harness does the same with an empty file (`tests/harness/mod.rs:233-234`, `:616-617`). So `new_scaffolds_the_prd_and_draft_state_and_worktrees_never_share_a_number` (`tests/docs_v3.rs:235`) already runs with no `user.name`: it is the placeholder case. `git_in` passes `-c user.name=T` per command (`:22-24`), which stores nothing.
- **The seven keys stay**: `tests/docs_v3.rs:275` and `src/docs/scaffold.rs:492` pin `number, slug, title, type, created, author, summary`. Only the author's value changes.
- **§2 is cut by number, not by name**: the pack cuts sections on `## {n}.` (`src/pack.rs:948-958`); `tests/loop.rs:260` only checks a fixture pack. Deleting §2 needs no code.
- **Not handled here**: the skill text of FR-2, FR-6 (`/prd` §2 and §6) and every other FR. That is slice 02's.

## Tasks

### 1. Refuse a feature with no slices — high — blocked by: —
**Source**: §4 FR-2 decision 5: "**Defensive:** `factory docs preflight` refuses a feature with zero slices: `not sliced — /slice NNN`. Today that run starts and ends with nothing built."

`src/docs/preflight.rs`, in `feature()`. When `slices` is empty, push `not sliced — /slice NNN` (NNN zero-padded, as `nnn` at `:255`) to `refuse`. A feature whose status is `shipped by the run` still has its slices, so it is unaffected.

**Acceptance:** `an_unsliced_feature_is_refused_before_a_run_starts` in `tests/preflight.rs`: strip the fixture's slices, run `factory docs preflight 001`, assert exit 1 and a line holding `not sliced — /slice 001`.

### 2. Fill the author from git — high — blocked by: —
**Source**: §9 FR-7: "Fill it from `git config user.name`. Keep `_(name)_` when that is empty or git fails."

`src/docs/scaffold.rs`. Read `git config user.name` in the repo root. Trimmed and non-empty → replace the `_(name)_` on the `author:` line with it; otherwise leave the line as it is. Write it so `frontmatter::parse` reads it back as that exact string: quote it if the name holds a character the frontmatter would misread.

**Acceptance:** `the_author_comes_from_git_user_name` in `tests/docs_v3.rs`: in a fixture repo, `git config user.name Ada` (local), then `factory docs new x`, and the parsed frontmatter's `author` is `Ada`. Extend `new_scaffolds_the_prd_and_draft_state_and_worktrees_never_share_a_number` with `author` still `_(name)_`.

### 3. Mark Blast radius optional in the scaffold — medium — blocked by: —
**Source**: §8 FR-6: "The scaffold's §2 placeholder says to delete the section and renumber when nothing outside the FRs can break."

`assets/docs/prd.scaffold.md:19-22`. Add one placeholder line under `## 2. Blast radius`: delete this section and renumber when nothing outside the FRs can break.

**Acceptance:** `the_scaffold_says_blast_radius_may_go` in `tests/docs_v3.rs`: `factory docs new x` writes a §2 that holds the words "delete this section". The PRD's other check, `factory docs check` clean without the section, holds today (`src/docs/check.rs` reads no headings); the Prove task reads it back.

### 4. Verify — medium — blocked by: 1, 2, 3
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 5. Prove — at the checkpoint — blocked by: 4
§11 QA plan rows **FR-2** (the preflight half), **FR-6** and **FR-7**, all `run` rows:

- FR-2: the transcript of `an_unsliced_feature_is_refused_before_a_run_starts`, plus `factory docs preflight` on a scratch repo with an unsliced feature, printing `✗ not sliced — /slice NNN`, exit 1.
- FR-6: `factory docs new x` in a scratch repo, the §2 read back; then delete §2, renumber, and `factory docs check` prints clean.
- FR-7: `factory docs new x` in a scratch repo with `user.name = Ada`, the frontmatter read back; and one with no `user.name`, still `_(name)_`.

**Acceptance:** after the checkpoint, each of the three rows has its transcript in **Progress**.

**Code-review fixes (2026-10-30).**

- The `not sliced` refusal also fired on a `shipped` feature with a log, the case preflight lets through so a run resumes at the PR. It now skips that case (`src/docs/preflight.rs`), pinned by `a_shipped_feature_with_a_log_resumes_without_slices`.
- The author replacement matched the first `author: _(name)_` in the rendered body, so a title holding that text took the author instead. It now matches the whole frontmatter line, pinned by a titled case in `the_author_comes_from_git_user_name`.
- `author()` re-did `git_line`'s trim by hand; it calls `preflight::git_line` now.
