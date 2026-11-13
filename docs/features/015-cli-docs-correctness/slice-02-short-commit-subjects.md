# Slice 02 — Short commit subjects

**Source**: [`prd.md`](prd.md) → §4 FR-2, §2, §12. **Depends on**: —. It is independent: no other slice edits `src/commit.rs`, `src/land.rs`, `src/scheduler.rs`, `src/closeout.rs` or `src/checkpoint.rs`.
**Created**: 2026-11-12

**Progress**: _(written as it lands — what was built, the gate numbers, what a review caught)_

**Slice**: After this slice, every subject `commit::subject` returns is at most 50 characters. An overflowing text is cut at the last space that fits, or at 50 chars when no space after the prefix fits. The full text becomes the commit body's first line. Slice, checkpoint, polish, CI-fix and ship commits all go through it.

## Current code / environment

- **The subject**: `pub fn subject(board: &Status, kind: &str, text: &str) -> String { format!("{kind}({:03}): {text}", board.number) }` (`src/commit.rs:149-151`).
- **Its callers**:
  - slice: `src/scheduler.rs:988`, `&format!("slice-{slice:02} {title}")`;
  - checkpoint: `src/checkpoint.rs:708,710`;
  - polish and CI fix: `src/closeout.rs:481,482`, where `fix(NNN): CI {check}` has an arbitrary check name;
  - ship: `:999`;
  - refresh numbers: `:1217`.
  - Grep `subject(` for any others.
- **The landing lookups compare `%s` exactly**:
  - `src/scheduler.rs:992-996`;
  - `src/closeout.rs:579-582`;
  - `src/checkpoint.rs:751-754`;
  - `tests/merge.rs:97`'s `grep -qxF`.
  - The cut must be a pure function of the text, so the same input always yields the same subject (prd §2).
- **Bodies already pass through one `-m`**: the held-commit path reuses a full `%B` (`src/scheduler.rs:1021-1024`, `land::commit_lane`). The plain helpers pass a single `-m` (`src/commit.rs:73`, `src/land.rs:71`). Carry the body as `subject + "\n\n" + full text` in that same `-m`, or return both from a helper. Do not add a second commit path.
- **Chars, not bytes**: titles hold "—". Cut on `char` boundaries, and count `chars()`.
- **Tests pinning subjects**:
  - `tests/commits.rs:162,228-230,328-344,397,484,503-532,576`;
  - `tests/checkpoints.rs:524-527`;
  - `tests/merge.rs:93-97`;
  - `src/pack.rs:1110,1431`;
  - `src/land.rs:409-705`.
  - All use one-letter titles, so they stay unchanged.
- **Deliberate non-change**: prd §4 rejected a configurable limit. 50 is a constant.

## Tasks

### 1. Cut the subject, keep the full text — high — blocked by: —
**Source**: §4 FR-2: "`commit::subject` returns at most 50 characters. When the text overflows, it is cut at the last space that keeps the subject within 50 **characters**… When no space after the `kind(NNN): ` prefix fits, it is cut at 50 characters, on a char boundary. The full text becomes the body's first line."

In `src/commit.rs`, have `subject` return the cut subject. Give callers a way to get the body: the full `kind(NNN): text` line, or the full text. Thread the body into the commit message at each caller above.

**Acceptance:** `a_long_subject_is_cut_at_a_word_with_the_rest_in_the_body`, a unit test in `src/commit.rs`:

- a 73-character slice title gives a subject of at most 50 chars, ending at a word;
- a spaceless 60-character CI check name gives exactly 50 chars;
- a title with "—" never panics;
- the same input twice gives the same subject.

### 2. A long slice title lands under its short subject — high — blocked by: 1
**Source**: §4 FR-2 acceptance: "the subject is at most 50 characters… the body's first line is the full text… a second call on the same input returns the same subject."

**Acceptance:** `a_long_slice_title_lands_under_a_short_subject`, in `tests/commits.rs`, using the existing landing fixture with a slice titled about 60 characters:

- `git log -1 --format=%s` is at most 50 chars;
- `%b`'s first line holds the whole title;
- a second landing pass finds the commit by subject and does not land it again.

### 3. Verify — medium — blocked by: 2
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q`

**Acceptance:** inner loop green, one commit.

### 4. Prove — at the checkpoint — blocked by: 3
prd §12 row **FR-2**: `run a_long_subject_is_cut_at_a_word_with_the_rest_in_the_body` and `run a_long_slice_title_lands_under_a_short_subject`. It is proven below the UI.

**Acceptance:** after the checkpoint, the transcripts are in Progress.
