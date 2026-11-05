---
number: 015
slug: cli-docs-correctness
title: "CLI, docs and log correctness"
# type: feature | bugfix | enhancement | refactor
type: feature
created: 2026-11-05
author: Emiliano Tisato
summary: |
  Docs verbs write where the reader reads and never report a write that did not happen; the run log takes one lock and one reader; slice commit subjects fit 50 characters; relock, skills update and from-idea PRDs stop tripping on their own output. Draft from the 2026-11-05 idea triage, not interviewed.
---

# PRD-015 — CLI, docs and log correctness

> **Draft from the 2026-11-05 idea triage.** The owner chose what is in and out, group by group; nothing here is interviewed or verified yet. `/prd` continues this draft: interview, recon, FRs, QA plan. `file:line` facts are from `main` at `ca128f7`; re-verify them.

## 1. Why now

`factory docs set NNN reviewed=` printed "state.toml updated" and wrote nothing the lane could see (2026-11-05); `log add` / `log show` split the same way in two client runs. Relock has hit four PRDs. Slice commits break the owner's 50-character rule on every run. Each item is small; together they are the places an agent's own tool lies to it.

## 2. Candidate FRs

Kept by the owner at the triage. `/prd` turns each into an FR or cuts it.

- **C-1 — One tree for docs verbs in a lane.** `board_root` sends `set`, `retro`, `status` and `log add` to the main tree while `check` and `log show` read the lane (`src/cli/docs.rs:253-264`); the "updated" message does not name the tree (`:706`). One rule, and the message says where it wrote. `/polish` §8 names the verb, not the frontmatter (`skills/polish/SKILL.md:155`).
- **C-2 — Slice commit subjects fit 50 characters.** `src/scheduler.rs:987` builds `feat(NNN): slice-NN <title>`, uncapped (`src/commit.rs:149`). Trim; the full title moves into the body.
- **C-3 — Relock replaces the hash of an untagged version** (`src/skills.rs:165-182`), and refuses once `vX.Y.Z` exists locally or on origin. `/polish` §5 says to relock from the base while untagged.
- **C-4 — The skills-update commit line is scoped**: `git commit -m … -- <dirs>` (`src/cli/skills.rs:94`).
- **C-5 — From-idea PRDs rewrite the idea's relative links** (`src/docs/scaffold.rs:363`).
- **C-6 — Docs verbs exit 2 on an I/O or TOML error** (`src/main.rs:108-111`, `src/cli/docs.rs:213-220`).
- **C-7 — The run log: one lock, one reader.** `factory answer` checks "already resolved" under `docs.lock` (`src/answers.rs:243`, `src/cli/answer.rs:205-208`); `settle_dead` writes the block and its note in one append (`src/scheduler.rs:1914-1928`); `answers::entries` uses `docs::log`'s reader (`src/answers.rs:150-160`).
- **C-8 — Board scan and `renumber` skip symlinks** (`src/docs/board.rs:159`, `src/docs/renumber.rs:27`).
- **C-9 — Two verbs agents hand-roll today:** append the Code-review fixes to the PRD (`skills/polish/SKILL.md:150`), and merge a superseded AD in the docs log.

## 3. Not in scope (decided 2026-11-05)

- **`Docs::dir` rescans the board per call** — dropped: no measured cost.
- **`factory upgrade` cannot tell a cargo install from a release install** — moved to *Factory as a consumer*.
- **Small tool gaps 1, 3, 4** (Boost `--no-scripts` trap, a per-slice inner-loop verb, Prove vs gate minutes) — dropped: project-specific or report-only. Gap 6 is moot, gap 7 is done.
- **friction-icg's leaks bullet** — in *Guards a branch can't loosen*.

## 4. Source ideas

The full text of each idea this draft absorbed; the idea files are deleted.

### `factory docs set NNN reviewed=<date>` reports success but writes nothing

_added 2026-11-05, origin: factory upstream_

`factory docs set 011 reviewed=2026-11-05` printed "state.toml updated", but no `reviewed` key appeared in the lane's `state.toml`, and `factory docs check` kept warning `W-NOT-REVIEWED`. Either the key goes to the driver's store while the check reads the lane, or the set is a no-op.

Related: `/polish` § 8 says to "set `reviewed: {date}` in the PRD's frontmatter", while the contract says `factory docs set NNN reviewed=…`. A run followed the contract and left the frontmatter alone. The two should name one place.

Should: `docs set reviewed=` writes where `docs check` reads (or refuses loudly), and § 8 points at the same mechanism.

### friction: icg-cloud PRD-019

_added 2026-10-15, origin: icg-cloud PRD-019 run 019-20261015-48af_

- **`polish` §1, the second opinion**: the passes run serially at ~12 min each with a 900 s timeout. A feature-sized diff (17 passes) has no budget, and there is no rule for when slow counts as failed. Run the passes in parallel, or give the step a wall-clock cap that hands the remaining passes to the fallback.
- **`factory docs log`**: `log add` in a lane writes the main checkout's log, while `log show` in the lane reads the lane's copy. That is how AD-14 duplicated AD-13, and an append-only log cannot take it back.
- **Leaks check**: it missed a plan number inside a string. `tests/Browser/GuideTest.php:217` had `guideShot($page, 'fr123-7-guide-staff-new-slot', narrow: true);`, a screenshot name carrying `fr123-7`. The local review caught it, and it is now `guide-staff-new-slot`.

From ~/Projects/ICG/icg-cloud/docs/features/019-arca-setup-papercuts/retro.md

### /deliver slice commit subjects exceed 50 characters

_added 2026-11-05, origin: factory upstream_

The driver writes slice commits as `feat(NNN): slice-NN <slice title>`. With ordinary slice titles this reaches 53 to 73 characters, for example `feat(011): slice-03 Certification pages, last of FR-3`. Owners who follow the common 50-character subject rule (and Conventional Commits tooling that warns on it) get history a worker cannot fix after it is published; a local review then flags it on every feature.

Should: the commit template cuts the title to fit a 50-character subject (or a configurable limit), and moves the full slice title into the body.

### Let relock replace the hash of a version that has no tag

_added 2026-10-15, origin: harvest 2026-10-15_

`skills/bundle.lock` pins one hash per version, and `relock` (`src/skills.rs`) refuses a second hash at the same version. On a feature branch a slice bumps and locks the version, and the tag comes only after the merge. A stale skill line polish finds then cannot be fixed on the branch: bumping again breaks the FR that pinned the version. PRD-005 and PRD-007 parked their fixes, and PRD-008 relocked by hand from main's lock. Releasing on time does not help, because the clash happens before any tag exists.

Fix: `relock` replaces the hash while no `vX.Y.Z` tag exists for that version, locally or on origin, and keeps refusing once one does, since that hash is what users installed.

Seen again in PRD-012 (harvest 2026-11-05): a polish fix to skill text tripped the lock under the untagged 0.11.0; the run restored the base branch's lock and relocked by hand (AD-1). Neither `/polish` § 5 nor the Autonomy contract says which to do, so the skill should say it once this lands.

### Scope the skills-update commit line to the skill dirs

_added 2026-10-30, origin: PRD-012 polish_

From PRD-012 polish (second opinion, grok). `factory skills update` prints `commit: git add <skill dirs> && git commit -m "chore: skills X"` (`src/cli/skills.rs` near the end of `update`). `git commit` with no pathspec takes the whole index, so anything already staged rides in the chore commit. PRD-012 made `/prd`, `/slice`, `/deliver` and `/lanes` run that line without asking (`skills/_shared/stale-tooling.md`), which raises the odds. Shape: print `git commit -m "…" -- <dirs>` (paths are tracked after the add, so new files are included); tests that match the commit line (`tests/init.rs`, `tests/skills_bundle.rs`, `src/skills.rs` unit tests) move with it. Acceptance: with `src/x.rs` staged, running the printed line commits only the skill dirs.

### From-idea PRDs keep the idea's relative links

_added 2026-11-02, origin: owner_

`factory docs new <slug> --from-idea <id>` pastes the idea's body into the new PRD
unchanged. The idea lived in `docs/ideas/`, but the PRD lives in
`docs/features/{NNN}-{slug}/`, one folder deeper. So every relative link in the body
breaks: `../reference/x.md` now points at `docs/features/reference/x.md`.

The owner hit this on 8 PRDs in a downstream project and fixed every link by hand.

The copy happens in `with_idea()` in `src/docs/scaffold.rs`, which writes the body under
`## From idea {id}` as-is. Rewriting each relative markdown link target from the idea's
folder to the PRD's folder at that point would fix it. Absolute URLs, `#anchors` and
links inside code fences should stay as they are.

`factory docs check` could also flag broken relative links in PRDs, so the next case is
caught even if the rewrite misses one.

### Native `factory docs` verbs exit 1 on an I/O or TOML error

_added 2026-10-13_

`src/main.rs` maps every `Err` to exit 1, while v2's `die()` exited 2 and `check` uses 1 for "errors found"; a hook cannot tell a broken `state.toml` from a failed check. Map `Error::Io` and `Error::Toml` to 2 under `factory docs`. PRD-002 polish review (second opinion), 2026-10-13.

### `factory answer` reads and writes the log outside one lock

_added 2026-10-13_

Two `factory answer BLOCK-n` calls at once can both see the block open and both append an `answer` entry (`src/answers.rs` `resolve` has no already-resolved check under `docs.lock`), and `deliver` in `src/cli/answer.rs` reads the slice status and then calls `state::set` as two locked steps, so a driver write between them is overwritten. Fix: a `log::append_if` that re-reads under the lock, and a compare-and-set on `slices.N.status`. Owner-driven command, so the race needs two terminals; parked. PRD-002 polish review, 2026-10-13.

### `settle_dead` writes the block and its note as two log appends

_added 2026-10-13_

A crash between them leaves a `block` with no note. One append of both under the lock, or the note folded into the block body. PRD-002 polish review, 2026-10-13.

### `answers::entries` parses `log.jsonl` a second time

_added 2026-10-13_

`src/answers.rs` re-implements the line reader `src/docs/log.rs` already has; one reader would keep the two from drifting on a format change. PRD-002 polish review, 2026-10-13.

### `renumber` and the board scan follow symlinks

_added 2026-10-13_

A symlinked feature folder or slice file under `docs/features/` is read and renamed through the link. Refuse or skip symlinks, as `check` could flag them. PRD-002 polish review (security pass), 2026-10-13.

### Small tool gaps from three unattended runs

_added 2026-10-29, origin: factory upstream_

Each of these came up in a `/deliver` retro as something an agent did by hand or worked around.

1. **`--no-scripts` in the slice contract's Traps for Laravel projects using Boost.** Boost's `post-update-cmd` runs `boost:update`, which pulls Boost's newest guideline text and skills into `CLAUDE.md`, `AGENTS.md` and the skill folders. Inside a feature, an agent running `composer update` gets dozens of unrelated files in its diff. One slice knew to pass `--no-scripts`; the polish fixer in the same run did not, and spent 5 minutes restoring files.
2. **A verb to append a slice's `**Code-review fixes**` section.** Polish hand-edits every slice file for this; it is the one docs write polish still does without the tool.
3. **Per-slice inner-loop run counts readable from the store.** The PR body's Slices table asks for "inner {runs}/{red}", and the polish worker cannot see them.
4. **`measure.py` splitting a checkpoint's Prove minutes from its gate minutes.** Retros currently estimate the split.
5. **`docs log merge` renaming a superseded AD's heading.** After a merge, the superseded AD still reads as current unless you reach the one that supersedes it.
6. **The PreToolUse grep hook blocks literal text searches that contain markdown punctuation** (searching for `**Back off.**` is treated as a symbol lookup), so the agent had to read whole files instead.
7. **A `ctx refs` mode that searches `vendor/`.** The hook blocks `grep` for code symbols, and the one lookup that mattered in a run was inside a vendor package.

### Also from

friction-arca-api PRD-025, its `factory docs log`, lanes bullet: full text in *Skill wording, contradictions and Prove guidance*.
