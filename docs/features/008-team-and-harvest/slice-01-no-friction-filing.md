# Slice 01 — The closeout files no friction

**Source**: [`prd.md`](prd.md) → §3 FR-1. **Depends on**: — (independent: it touches `src/closeout.rs`'s friction code and the retro template only)
**Created**: 2026-10-15

**Progress**:

**Slice**: After this slice, no closeout writes anything into the `[source] path` checkout. `file_friction`, its two calls and its helpers are gone, and the retro template's heading reads `## Framework friction → factory`. The section stays in every retro as the only home of a run's friction until a harvest reads it. Reading that section again is slice 04's job; routing it to factory is slice 06's.

## Current code / environment

- **The function and its calls**: `file_friction` (`src/closeout.rs:416-449`), called in `polish()` once the run's polish worker settles (`:330`), and in `settle_orphan` (`:2031`) inside `if w.role == "polish" { … config::load_with_project … file_friction … }` (`:2135-2140`). Its helpers are `only_placeholder` (`:452-459`) and `friction()` (`:461-471`), pinned by two unit tests (`:2534-2560`).
- **`friction()` goes too**: the PRD keeps the section reader "only if FR-5 reuses it". It does not: slice 04's `factory docs harvest` prints every retro section but Numbers, and reads no one section.
- **Imports that go unused**: `use crate::docs::ideas::{self, Idea};` (`:16`) and `scaffold` in `:17` serve only `file_friction` (`:420,437-446,440`). `:1029` spells `crate::docs::scaffold::today()` in full, and `retro_template()` (`:2369`) still has callers (`:146,539,986`).
- **The heading**: `assets/retro.md:33` is `## Framework friction → skills repo`. The stub writes the same heading (`tests/stubs/claude:366`) and documents it (`:52-53`). `tests/fixture/v2/.docs/templates/retro.md:57` is v2 migration input: leave it.
- **The integration tests**: `tests/closeout.rs:2064-2309` is the friction block. `source_repo` (`:2071-2095`) runs `git init` and `docs init` in a sibling dir named `skills` and points `[source]` at it, but never commits, so its `git status --porcelain` is already non-empty. `polish_with_friction` (`:2107`) makes the stub fill the section. The orphan test (`:2215-2259`) also pins that a resumed driver lands polish once (`:2252-2258`). `a_ci_fix_orphan_adopted_by_a_resumed_driver_lands_once` (`:2261`) is not friction code: keep it.
- **`[source]` stays**: `src/config.rs:19,25-27`, read by `factory upgrade` (`src/cli/upgrade.rs:32-39`) and by slice 06.
- **Deliberate non-changes**: the four friction ideas already in `docs/ideas/` keep their `From /home/…` lines; the first harvest fixes them by hand (prd §3 FR-6 step 4). The PR body's Score line keeps the biggest friction (`assets/pr-body.md:48`).

## Tasks

### 1. Delete the filing — high — blocked by: —
**Where**: `src/closeout.rs:16-17,329-331,415-471,2135-2140,2534-2560`.

**Source**: §3 FR-1: "Delete `file_friction` and its two calls. Keep the section reader only if FR-5 reuses it."

Delete `file_friction`, `only_placeholder` and `friction()`, the call in `polish()`, and the whole `if w.role == "polish"` block in `settle_orphan`: `settle(…)` then `Ok(None)` remains. Delete the two unit tests and the imports that go unused.

The trap: `settle_orphan`'s block also loads the config (`:2137`). Nothing else in that function reads it, so the load goes with the call.

**Acceptance:** task 3's test, and `grep -rn file_friction src/` finds nothing.

### 2. The heading — medium — blocked by: —
**Where**: `assets/retro.md:33`, `tests/stubs/claude:52-53,366`.

**Source**: §3 FR-1: "The heading becomes `## Framework friction → factory`: the skills live in this repo now."

Change the template heading and the stub's heading and its doc comment to match.

**Acceptance:** task 3's test passes with the stub writing the new heading, and the retro it leaves holds `## Framework friction → factory`.

### 3. The test — high — blocked by: 1, 2
**Where**: `tests/closeout.rs:2064-2309`.

**Source**: §3 FR-1 acceptance: "A stub run whose polish fills the Framework friction section, with `[source] path` set to a second checkout, leaves that checkout's `git status --porcelain` empty."

Rewrite `a_friction_section_is_filed_as_an_untracked_idea_in_the_source` into one test: `source_repo`, a run with `polish_with_friction(FRICTION)`, then the source checkout's `git status --porcelain --untracked-files=all` is empty and the feature's retro holds the section. Keep the orphan test's landing assertions (`:2252-2258`) under a new name that says what it pins, minus the friction lines. Delete the other friction tests and the helpers nothing uses any more (`ideas_in`, `friction_id`, `today()`, `PLACEHOLDER`, `MISSED`).

The trap: make `source_repo` commit after `docs init`, or "empty" fails before the run starts. Commit with `-c user.name=… -c user.email=…`: the fixture's `GIT_CONFIG_GLOBAL` is empty. Keep the `docs init` there too: `file_friction` returned early when `<source>/docs/ideas` was missing (`src/closeout.rs:420`), so a source without it would pass against today's code.

**Acceptance:** a test named for the behavior (e.g. `a_filled_friction_section_leaves_the_source_checkout_untouched`) fails against today's code and passes after task 1.

### 4. Verify — medium — blocked by: 1, 2, 3
`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 5. Prove — at the checkpoint — blocked by: 4
Rows from [`prd.md`](prd.md) §5 this slice proves, below the UI:

- **FR-1**: "run: a stub run with a filled Framework friction section and `[source] path` set to a second checkout; `git status --porcelain` in that checkout" — task 3's test transcript, and `grep -rn file_friction src/`.

**Acceptance:** after the checkpoint, Progress names the transcript.
