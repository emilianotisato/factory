# Slice 04 — Commit lines and from-idea links

**Source**: [`prd.md`](prd.md) → §6 FR-4, §7 FR-5, §12. **Depends on**: —. Both FRs edit `src/docs/scaffold.rs` (`:131` and `with_idea` at `:349-370`), so they are one slice. It is independent of 01-03 and 05.
**Created**: 2026-11-12

**Progress**:

- **Built.** The `commit:` lines from `factory skills update` (`src/cli/skills.rs:94`) and `factory init` (`src/docs/scaffold.rs:132`) now end `-- <same dirs>`, and `tests/init.rs`'s whole-line assertion moved with them. `factory docs new --from-idea` passes the idea body through `migrate::rebase_links(body, idea dir, PRD dir)`. It rebases each relative link outside fenced blocks with `edit_links` and keeps anchors and titles. `edit_links`'s callback also receives the whole link source now. AD-1 (fyi) covers what stays as written beyond URLs, anchors and fences.
- **Tests.** `the_printed_commit_line_leaves_other_staged_files_alone` (`tests/skills_fence.rs`) and `a_from_idea_prd_keeps_its_relative_links` (`tests/docs_ideas.rs`). The sibling idea is a real `idea add` file linked as `./{id}.md`, because `docs check` validates every file in `docs/ideas/`.
- **Seen red once each.** Without the ` -- {dirs}` suffix, the commit test fails at the "only skill dirs" assertion: `src/x.rs` rode along. With the body pasted unrebased, the links test fails at the `](../../reference/x.md` assertion. With fence skipping disabled, it fails at the fenced-block byte-for-byte assertion. Each test is green again after the restore.
- **Inner loop:** 1 run, 0 red, 2 min (`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q`, 116 s). Caught nothing.
- **Prove (checkpoint):** FR-4 `run the_printed_commit_line_leaves_other_staged_files_alone`, FR-5 `run a_from_idea_prd_keeps_its_relative_links`. Transcripts land here after the checkpoint.
- **Proven (checkpoint 1):** FR-4 `the_printed_commit_line_leaves_other_staged_files_alone` passed — `cp-1/fr4-tests.txt`
- **Proven (checkpoint 1):** FR-5 `a_from_idea_prd_keeps_its_relative_links` passed — `cp-1/fr5-tests.txt`

**Slice**: After this slice:

- the `commit:` lines that `factory skills update` and `factory init` print commit only their own dirs (`git commit -m "…" -- <dirs>`);
- `factory docs new --from-idea` rewrites the idea body's relative markdown links from `docs/ideas/` to the PRD's folder, leaving URLs, `#` anchors and fenced code blocks as written.

## Current code / environment

- **FR-4**: two lines.
  - `src/cli/skills.rs:94`: `"commit: git add {} && git commit -m \"chore: skills {v}\""`.
  - `src/docs/scaffold.rs:131`: `"commit: git add {} && git commit -m \"{subject}\""`.
  - Both become `… git commit -m "…" -- <same dirs>`. The paths are tracked after the `add`, so new files are included. The adversarial pass checked that `git commit -m … -- docs` works on a first commit and leaves another staged file staged.
- **FR-4 tests**:
  - `tests/init.rs:380` (`init_prints_the_commit_line_and_commits_nothing`) compares the whole line and moves with it.
  - `tests/skills_fence.rs:286,403` and `tests/init.rs:217` match a prefix and stay.
  - `skills/_shared/stale-tooling.md:7` says to run "the `commit:` line it prints" and needs no change. Do not edit `skills/`: that would force a version bump this slice does not need.
- **FR-5**:
  - `with_idea` (`src/docs/scaffold.rs:349-370`) pastes `body` under `## From idea {id}` (`:363`). The idea lived in `docs/ideas/`, and the PRD lives in `docs/features/{NNN}-{slug}/`, one level deeper.
  - `src/docs/migrate.rs:1242-1328` has the link helpers (`link_at`, `normalize`, `edit_links(text, dir, edit)`). They skip `:`-containing hrefs, `#`-only hrefs and images, but not code fences. Reuse them, widening visibility as needed, and add fence skipping around the call.
- **The check flags fenced links too**: `check`'s `links()` (`src/docs/check.rs:791-862`) does not skip fences, so a fenced relative link is still `E-LINK-BROKEN` (prd §7, corrected). The acceptance therefore checks the fenced block byte-for-byte, and runs `docs check` only on a body without one.
- **FR-5 test today**: `new_from_idea_carries_the_body_and_deletes_the_file` (`tests/docs_ideas.rs:271`) asserts the body is carried.
- **Deliberate non-change**: prd §7 adds no new check rule (`E-LINK-BROKEN` already exists).

## Tasks

### 1. Scoped commit lines — high — blocked by: —
**Source**: §6 FR-4: "Both lines end `git commit -m \"…\" -- <dirs>`."

Edit `src/cli/skills.rs:94` and `src/docs/scaffold.rs:131`, and move `tests/init.rs:380` with them.

**Acceptance:** `the_printed_commit_line_leaves_other_staged_files_alone`. In a fixture repo with `src/x.rs` staged, run `factory skills update`'s printed `commit:` line through `sh -c`:

- the new commit holds only the skill dirs;
- `src/x.rs` is still staged.

It fails on today's line.

### 2. From-idea PRDs keep working links — high — blocked by: —
**Source**: §7 FR-5: "`with_idea` rewrites each relative markdown link target from the idea's folder to the PRD's folder… URLs, `#` anchors and links inside fenced code blocks stay as written."

Edit `with_idea`, reusing `migrate.rs`'s helpers.

**Acceptance:** `a_from_idea_prd_keeps_its_relative_links`. Use an idea with markdown links to `../reference/x.md`, `./other-idea.md`, `https://x` and `#a`, plus a fenced block holding a link to `../y.md`.

- The first two resolve, from the PRD's folder, to the same files as from `docs/ideas/`.
- The URL, the anchor and the fenced block are byte-for-byte unchanged.
- The same idea without the fence then passes `docs check` with no `E-LINK-BROKEN`.

### 3. Verify — medium — blocked by: 1, 2
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q`

**Acceptance:** inner loop green, one commit.

### 4. Prove — at the checkpoint — blocked by: 3
prd §12 rows:

- **FR-4:** `run the_printed_commit_line_leaves_other_staged_files_alone`.
- **FR-5:** `run a_from_idea_prd_keeps_its_relative_links`.

They are proven below the UI.

**Acceptance:** after the checkpoint, the transcripts are in Progress.

**Code-review fixes (2026-11-13).**

- `rebase_links` knew only ``` fences, so links inside `~~~` fences, or inside a ```` block that holds ```, were rewritten, against FR-5. Fences now match by char and run length.
- Relative images were left unrebased, but `docs check` resolves them, so a from-idea PRD with an image failed check at once. Images are rebased now (AD-5, superseding AD-1's image clause).
- AD-1's rules had no test. `rebased_links_keep_fences_roots_anchors_and_titles` covers fences, root-absolute targets, anchors, titles, images and inline code.
