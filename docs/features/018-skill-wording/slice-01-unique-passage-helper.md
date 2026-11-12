# Slice 01 — Unique-passage helper

**Source**: [`prd.md`](prd.md) → §13 FR-11 (the helper and the conversion; the `/slice` §4 sentence is slice 04's), §15. **Depends on**: — (the spine: every later slice's skill-text tests call this helper)
**Created**: 2026-11-12

**Highest risk in the feature**: the conversion. Some needles occur more than once today. Each one must be lengthened to the one passage it guards, never weakened, and the converted test must still pass on today's text.

**Progress**: _(written as it lands — what was built, the gate numbers, what a review caught)_

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
