# Slice 01 — Unique-passage helper

**Source**: [`prd.md`](prd.md) → §13 FR-11 (the helper and the conversion; the `/slice` §4 sentence is slice 04's), §15. **Depends on**: — (the spine: every later slice's skill-text tests call this helper)
**Created**: 2026-11-12

**Highest risk in the feature**: the conversion. Some needles occur more than once today. Each one must be lengthened to the one passage it guards, never weakened, and the converted test must still pass on today's text.

**Progress**:

- **Built.** `tests/common/passage.rs` holds `assert_unique_passage(label, text, needle)`; its own test `a_passage_must_occur_exactly_once` is in `tests/passage.rs`. `skills_bundle.rs`, `cockpit.rs`, `init.rs` and `loop.rs` include it with `#[path]`. `src/pack.rs` includes it as a file-level `#[cfg(test)] #[path = "../tests/common/passage.rs"] mod passage`: a `#[path]` inside the inline `mod tests` resolves from `src/pack/tests/`, which does not exist.
- **Converted.** All 17 `skills_bundle.rs` assert lines, the `cockpit.rs` deliver-skill loop, `init.rs`'s per-file convention line (labelled `CLAUDE.md` / `AGENTS.md`), `loop.rs`'s `quiet flag`, and `pack.rs`'s three positive autonomy checks (`checkpoint` holds each cut section, `builder` holds each kept one, both hold "Several tool calls side by side"). Absence checks, line selectors and rendered-command checks are left as they were.
- **Needles lengthened**: 13 occurred more than once on today's text. Each now quotes the one passage it guards.
  - deliver: `factory run`, `factory wait`, `factory status`, `factory stop`, `AskUserQuestion`, `not sliced`;
  - gate round 3: `write mode`;
  - polish: `factory review second` and `factory review second --diff` (§1), `--feature <NNN>` (§1);
  - prd: `Slice only`, `Stop here`.
  - `models_md_holds_no_invocation_…` now pairs each skill with its own needle: polish's bare `factory review second` occurs twice.
- **Seen red**: one needle per file, passage deleted locally, targeted test run, file restored with `git checkout`. Every run exited 101 with the helper's message:
  - `skills/polish/SKILL.md`, "Run no test and no fix in that checkout until the UX pass returns" → `polish_starts_ux_first_and_holds_tests`: `polish §1: … occurs 0 times`;
  - `skills/deliver/SKILL.md`, the "If the owner asks to stop: `factory stop <id>`" line deleted → `the_cockpit_skill_…`: `occurs 0 times`. The old `contains("factory stop")` would have stayed green, because step 1 still names `factory stop <id>`;
  - `assets/docs/convention.md`, the history line → `the_convention_block_lands_once_in_each_file`: `CLAUDE.md: … occurs 0 times`;
  - `assets/contract-slice.md`, "quiet flag" → `pack_is_the_slice_the_cited_sections_…`: `the pack's # Contract: … occurs 0 times`;
  - `assets/autonomy.md`, "Several tool calls side by side" → `a_builder_reads_autonomy_…` (lib): `autonomy: … occurs 0 times`.
- **Inner loop**: 5 runs, 3 red, about 8 min. Two reds were the `pack.rs` `#[path]` resolution, a compile error. The third was the 13 duplicate needles. The final run: clippy clean, 0 failures across 43 test binaries.

**Slice**: After this slice, `assert_unique_passage(label, text, needle)` exists in a file under `tests/`, reachable from every integration test and from `src/pack.rs`'s test module. Every positive skill or asset passage assertion uses it, so a deleted rule turns its test red even when another line shares its words. No skill or asset text changes here, so no version bump.

## Current code / environment

- **No helper exists.** About 24 positive passage assertions use plain `contains`:
  - `tests/skills_bundle.rs`: 17 assert lines across 11 functions, from `:115` to `:357`;
  - `tests/cockpit.rs:792` (a loop of about 20 needles over the deliver skill);
  - `tests/init.rs:285-288`;
  - `tests/loop.rs:273` (`contract.contains("quiet flag")`).
- **Not passage assertions:**
  - `tests/loop.rs:277` asserts the pack holds the whole `pack::builder_autonomy()` copy;
  - `src/pack.rs:1071-1072` assert a rendered checkpoint command and record, not skill text.
  - Leave both as they are, unless one turns out to quote skill or asset text.
- **Shared-helper pattern**: `tests/harness/mod.rs` starts with `#![allow(dead_code)]` and is pulled in with `#[path = "harness/mod.rs"] mod harness;` (`tests/docs.rs:1`, `tests/cockpit.rs:1`). Do the same for the new file, e.g. `tests/common/passage.rs`. The gate runs clippy with `-D warnings`, so the allow is needed.
- **`src/pack.rs` tests** (`mod tests` at `:989`) can include the same file with a `#[path = "../tests/common/passage.rs"]` module inside `mod tests`. `AUTONOMY` and `CUT` are private, so the autonomy tests at `:1377-1395` stay in `pack.rs`.
- **The signature takes a label** (prd §13, corrected 2026-11-12): the text is often a section slice (`polish_section`, `tests/skills_bundle.rs:275`), so the helper cannot know the file. The message names the label.
- **Absence checks stay `!contains`** (prd §13):
  - `src/pack.rs:1144,1361,1384`;
  - `tests/skills_bundle.rs:112,122,149,165,237,251`;
  - `tests/loop.rs:274`;
  - the forbidden loop in `tests/cockpit.rs:796-798`.
- **Not converted:**
  - line selectors like `.lines().find(|l| l.contains(…))` (`tests/skills_bundle.rs:287,344,354`) are not assertions;
  - the cut markers occur four times by design (`assets/autonomy.md:31,95,106,112`) and keep their own checks;
  - `tests/init.rs:284`'s `matches(HEADING).count() == 1` is already a count.
- **Autonomy's "weaken a test this run did not write"** (`assets/autonomy.md:60`, under "Never, to get green") does not apply. prd §13 orders the conversion, and a needle only ever gets longer, which strengthens the test.
- **Deliberate non-change**: prd §13 decided the helper names the rule, not the file. `/slice` §4's sentence is slice 04's, so do not edit `skills/` here.

## Tasks

### 1. The helper — high — blocked by: —
**Source**: §13 FR-11: "`assert_unique_passage(label, text, needle)` fails when the needle occurs zero times or more than once. Its message names `label`."

Create `tests/common/passage.rs` with `#![allow(dead_code)]` and a `pub fn assert_unique_passage(label: &str, text: &str, needle: &str)`. It counts with `text.matches(needle).count()` and panics with the label, the needle and the count when the count is not 1. Its own unit tests sit in the same file under `#[cfg(test)]`, or in a small `tests/passage.rs` that includes it.

**Acceptance:** `a_passage_must_occur_exactly_once`: the helper panics on zero occurrences and on two, and passes on one. Use `std::panic::catch_unwind`, or `#[should_panic]` cases.

### 2. Convert every passage assertion — high — blocked by: 1
**Source**: §13 FR-11: "Every one listed above moves to the helper. A needle that occurs twice is lengthened until it quotes the one passage it guards."

Pull the helper into:

- `tests/skills_bundle.rs`
- `tests/cockpit.rs`
- `tests/init.rs`
- `tests/loop.rs`
- `src/pack.rs`'s test module, if any of its assertions quote asset text

Replace each positive `assert!(x.contains(needle), …)` on skill or asset text with `assert_unique_passage(label, x, needle)`. When a needle fails the helper because it occurs twice, lengthen it to quote more of the one passage it guards. Never shorten it, and never point it at different text.

**The trap:** `tests/init.rs:285-288` checks a rendered `CLAUDE.md` and `AGENTS.md`, where the convention block appears once per file. Keep the per-file label.

**Acceptance:** every converted test passes on today's text. For one converted needle per file, delete its passage locally, see the test go red, then restore the passage. Write those runs in Progress (prd §9: see it red once).

### 3. Verify — medium — blocked by: 2
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q`

**Acceptance:** inner loop green, one commit.

### 4. Prove — at the checkpoint — blocked by: 3
prd §15 row **FR-11**, its first half: `run a_passage_must_occur_exactly_once`. The second half, `slice_asks_for_a_passage_that_occurs_once`, is slice 04's. It is proven below the UI: the checkpoint runs the test and keeps the transcript.

**Acceptance:** after the checkpoint, the transcript is in Progress.
