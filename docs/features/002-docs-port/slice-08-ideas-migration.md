# Slice 08 — Ideas migration (FR-20 rest)

**Source**: [`prd.md`](prd.md) → §10 FR-20 (the Migration bullet, the file format, the migration half of the acceptance), §6 FR-16 (`--dry`, the clean-tree refusal, git as the undo), §12 QA plan (the FR-20 row), §14 (`W-IDEAS-V2` for lines an old skill writes after the migration). **Depends on**: 05 (the idea file format, `slug`, the writer and `check`'s ideas rules), 07 (`migrate::plan`/`run`, the plan rows, the blame helper, the v2 fixture and `tests/migrate.rs`).
**Created**: 2026-10-13 · **Status**: pending

**Progress**:

- **Built**: `src/docs/migrate_ideas.rs` (new) holds `parse` (bounds, title and body, placement, drops), `fields` (`added`/`origin`/`retrigger`, plus the rule that gave `added`) and `plan`. `plan` computes ids with `-2` clashes against `docs/ideas/` and earlier ids, renders through slice 05's `ideas::render`, writes the README only when absent, deletes `ideas.md`, and splices the guess rows to the head of the plan. A dry run warns `added is today: line N is not committed` for a line git has not committed. `src/docs/migrate.rs` changes:
  - `blame_lines` returns each line's date and whether it is committed (the zero sha is not).
  - `unlink` now rides a generic `edit_links`, and the new `relink` points every link to `ideas.md` at `ideas/README.md`, anchor dropped.
  - `plan` calls `migrate_ideas::plan` before `project.toml`.
  - `run` creates the parent folder before a write.
  - The fixture `tests/fixture/v2/docs/ideas.md` holds the five placements (Alpha to Echo), a multi-line body, a Re-trigger and both placeholders. `tests/migrate.rs` gains `ideas_migrate` and extends `prd_001_copy_migrates`. The undo and re-run step of slice 07's fixture test snapshots all of `docs/`, so it now covers `docs/ideas/` with no new code.
- **Inner loop**: 7 runs, 1 red, about 4 minutes. The red was a compile error: the `flush` closure borrowed the bullet list while the loop pushed to it, so `flush` is now a nested fn. The last full `cargo test` was green in 52 s. `cargo fmt --check` and `cargo clippy --all-targets -D warnings` are clean (one redundant closure fixed).
- **Task 5, PRD-001 copy** (`cargo test --test migrate -- --ignored`, green): the copy has `docs/ideas/` with `README.md` plus 19 files, `ideas.md` is gone, and `check` reports no `E-`. Six links were relinked: the five of Current code plus `slice-11-skills-hook-install.md:27`, written after slicing. This repo's own `ideas.md`, with blame dates read in place:

  ```
  idea 2026-10-09 git blame line 14: Codex or Cursor as build workers
  idea 2026-10-09 git blame line 15: Decompose polish into driver-run steps
  idea 2026-10-09 git blame line 16: Typed lookup layer for `factory ctx` (LSP or SCIP)
  idea 2026-10-09 git blame line 17: Jev (TypeSafe System One) as a picker in `factory ctx` or the driver
  idea 2026-10-10 Added line 18: Uncle Bob's quality tools as gate tiers or polish inputs
  idea 2026-10-10 Parked line 19: Batch `def` lookups in the pack's recon
  idea 2026-10-10 Parked line 20: Pre-push guard for the driver's own git calls
  idea 2026-10-10 Parked line 21: Persist CI rerun rows across a driver restart
  idea 2026-10-11 Added line 22: Run the CI checks locally before the push (`nektos/act` or similar)
  idea 2026-10-11 Parked line 23: Block worker pushes to a remote with its own `pushurl`
  idea 2026-10-11 Added line 24: Port concepts from Cursor's pstack verification skills into `verify-app`
  idea 2026-10-12 Added line 25: Claude Code's advisor tool for workers
  idea 2026-10-12 git blame line 26: Cross-repo ideas scanner: `factory ideas --json` plus an `/ideas` skill
  idea 2026-10-12 git blame line 27: `factory bugs pull | list | drop | promote`: production errors as a local bug list
  idea 2026-10-13 git blame line 29: The skill family lives in this repo; `factory skills install | update` puts it where agents read it
  idea 2026-10-13 git blame line 30: Driver slice commit subjects run past 50 characters
  idea 2026-10-13 git blame line 31: Refresh `## Numbers` after the CI rounds
  idea 2026-10-13 git blame line 32: Mark partial figures in the retro's Numbers table
  idea 2026-10-13 git blame line 33: A missing transcript in a live run stores zeros
  ```

  8 take a phrase's date and 11 take blame, all from 2026-10-09 to HEAD.
- **Deviations**: two, AD-801 and AD-802 in `agent-log.md`. Plan rows carry the title, the line and where `added` came from. A stray non-bullet line under a heading is a `drop` row. Slice 05's round-trip of a title holding `"` already exists (`tests/docs_ideas.rs`), so that test needed no change.
- **Left for the checkpoint**: task 7, the migration half of the QA plan's FR-20 row.

**Slice**: After this slice `factory docs migrate` also turns `docs/ideas.md` into one file per open idea under `docs/ideas/`, writes `docs/ideas/README.md`, and deletes `ideas.md`. Each bullet is placed by its heading. Graduated bullets are not migrated. Bullets whose placement is a guess (a `### Parked…` heading below `## Graduated`, or an unknown heading) migrate as `open` and are printed first in `--dry`, so the owner reads the risky rows before anything else. Each body is the bullet's text after the title, byte for byte. A migrated fixture then passes `factory docs check` with no warning at all. The tool side of ideas (`idea add|list|drop`, `new --from-idea`, `check`) is slice 05's.

## Current code / environment

- **The real bullet shape is not the PRD's**: the PRD writes `- **Title** — body` (§10 Today), but this repo's 19 bullets (`docs/ideas.md:14-33`) are `- **Title.** body`: the title ends in a period inside the bold, and no dash follows. Line `:27` starts its title with a backtick and holds `: ` and `|` (`` `factory bugs pull | list | drop | promote`: production errors… ``). Line `:28` is a blank line between two bullets of `## Open`. So the parser takes the bold run as the title and strips at most one separator after it. A blank line ends a bullet, never a section.
- **The date phrases, as found**: `Added 2026-10-10` (`:18`), `Parked from PRD-001 polish (2026-10-10)` (`:19-21`) and `Parked from the PRD-001 owner review (2026-10-11)` (`:23`). 11 of the 19 bullets have neither phrase, so they take the `git blame` date of their first line. Other words with a date (`Decided 2026-10-13`, `:29`) are not one of the PRD's three phrases and are not read.
- **What is not a bullet under a heading**: the preamble and its `**Rules**` list (`:1-10`), which sits before any `##`, and the placeholder `- _(move lines here with their \`{NNN}\` when they get a PRD, or just delete)_` (`:37`) under Graduated. The node template (`tests/fixture/.docs/templates/ideas.md`) adds `- _(nothing yet)_` under `## Open`.
- **Slice 05's writer refuses a `"`**: `idea add` writes every field as one double-quoted line, and `frontmatter.rs` reads quotes with no escapes, so `add` refuses a `"` or a newline in a value (slice 05, Current code; its acceptance tests "a title holding `"` exits 2"). It also takes `added` from `src/clock.rs:59 today()`. Migration needs neither: the date comes from the bullet, and an owner's title may hold a `"`. No title in this repo does.
- **Five links point at `ideas.md`**: `docs/index.md:66`, `docs/features/001-driver/prd.md:19`, and `docs/features/002-docs-port/prd.md:18`, `:236`, `:253`. Slice 03's `check` reports each as `E-LINK-BROKEN` once the file is gone. Slice 05's `assets/docs/ideas-README.md` holds the inbox rules those links point to (slice 05, task 5).
- **Blame on the fixture is test time**: `Fixture::new` commits with the clock's date (`tests/harness/mod.rs:96-140` sets no `GIT_AUTHOR_DATE`), so a blame fallback would give today, and the acceptance could not name the date. Slice 07's `Fixture::v2` commits with a fixed author date for this reason (slice 07, task 7).
- **Slice 07 leaves `ideas.md` alone**, so its fixture test allows exactly one warning, `W-IDEAS-V2` (slice 07, task 7 step 3). This slice removes that allowance.
- **Deliberate non-changes** — *(prd §14: lines an old skill appends to `ideas.md` after the migration are `W-IDEAS-V2`, and "the owner moves those lines by hand with `idea add`". `migrate` runs once and never merges into an existing `docs/ideas/`. prd §10: there is no `graduated` or `dropped` state, so a Graduated bullet is not written anywhere. prd §15: the cross-repo scanner is a later PRD. prd §6: migration never rewrites prose, so bodies keep their phrases (`Added …`, `Parked from …`, `Re-trigger: …`) even when a field was read from them.)*

## Tasks

### 1. The bullet reader and its placement — high — blocked by: —
**Source**: §10 FR-20 Migration: "Each `- **Title** — body` bullet with its continuation lines becomes one file; the bullet's text after the title is the body, verbatim. **Placement**: under a `##` heading starting `Open` → `open`, `priority: high` when the heading says "high priority"; under one starting `Owed` → `owed`; under `## Graduated…` → **not migrated**, except bullets below a `### Parked…` heading there, which migrate as `open` and are marked as guesses in the plan; any other heading → `open`, marked as a guess." "Text that is not a bullet under a heading (the file's preamble and rules, placeholder lines like `_(move lines here …)_`) is dropped and listed."

`src/docs/migrate_ideas.rs` (new): `pub fn parse(text: &str) -> Vec<Bullet>`, a pure function. `Bullet { line, title, body, h2, h3, place }`, with `place` one of `Open { owed: bool, high: bool, guess: Option<&'static str> }`, `NotMigrated(rule)` and `Dropped(reason)`.

- **Bounds**: a bullet starts at `^- ` outside a fence and runs to the line before the next `^- `, the next blank line or the next heading. Its continuation lines are kept as written, indentation included.
- **Title and body**: `^- \*\*(.+?)\*\*(.*)$` gives the title, with one trailing `.` stripped (Current code). The body is the rest of the first line, minus one leading ` — `, ` – `, ` - `, `: ` or space, then each continuation line, then one `\n`. A bullet with no leading bold is migrated as a guess: its title is its text up to the first `. `, and its body is its whole text.
- **Placement**: the heading text is compared from its start, case-insensitive. `## Open…` gives `open`, plus `high` when the heading holds "high priority". `## Owed…` gives `owed`. `## Graduated…` gives `NotMigrated("under Graduated")`, except below a `### Parked…` there, which gives `open` with guess `"Parked below Graduated"`. Any other `##` gives `open` with guess `"unknown heading ## <text>"`. A `###` that is not Parked inherits its `##` placement. A `### Parked…` under Open or Owed keeps that placement and is no guess.
- **Dropped**: a bullet before any `##` (the `**Rules**` list), and a bullet whose whole text is `_(…)_`. Each gives one `drop` plan row with its line number.

**Acceptance:** unit tests on inline text: `- **A.** x` under `## Open` gives title `A`, body `x\n`, `open`; `- **B** — y\n  more` under `## Open — High Priority` gives body `y\n  more\n` and `high`; a bullet under `## Owed by someone else — pending confirmations` is `owed`; one under `## Graduated → became a feature` is `NotMigrated`; one under `### Parked from the PRD-001 polish review (2026-09-22)` below it is a guess; one under `## Someday` is a guess; the template's `**Rules**` list and both placeholders are `Dropped`; a fenced `- **C.**` line is not a bullet; this repo's `docs/ideas.md` at HEAD gives 19 `open` bullets, title `:27` equal to the bold text minus its period, and 6 drops (the five rules and the Graduated placeholder).

### 2. `added`, `origin` and `retrigger` — high — blocked by: 1
**Source**: §10 FR-20 Migration: "**Fields**: `added` from a date in "Parked from … (date)", "Added date" or the enclosing `### Parked … (date)` heading, else from `git blame` of the bullet's first line; `origin` from the "Parked from …" phrase or heading, else absent; `retrigger` from a "Re-trigger:" sentence, else absent." §10: "`added` (date, required), `origin` (optional, free text: `PRD-016 polish`, `owner`, `cut from PRD-012 FR-4`), `retrigger` (optional: the event that should make someone look again)".

`fn fields(b: &Bullet, blame: &str) -> (String, Option<String>, Option<String>)`, with `blame` the first line's date from slice 07's `migrate::blame_dates`. Read the bullet's text (title and body) first, then its `### Parked` heading, and the first match wins:

- `added`: `Parked from (.+?) \((\d{4}-\d{2}-\d{2})\)`, then `Added (\d{4}-\d{2}-\d{2})`, then the heading's `Parked from (.+?) \((\d{4}-\d{2}-\d{2})\)`, then `blame`. Each bullet's plan row names which one (`added from Parked`, `added from git blame`).
- `origin`: the first group of the same `Parked from` match, bullet before heading, with a leading `the ` stripped. So `Parked from the PRD-001 owner review (2026-10-11)` gives `PRD-001 owner review`.
- `retrigger`: the text after `Re-trigger:`, trimmed, up to and including the first `. `, `? ` or `! `, else to the end of the line.

The body keeps every phrase a field was read from (Deliberate non-changes).

The trap: an uncommitted `ideas.md` line has no blame date. `git blame --porcelain` gives it the author `Not Committed Yet` and the current time. A real `migrate` refuses a dirty `docs/` (slice 07, task 6), so it never sees one. `--dry` can, so it prints `warn added is today: line N is not committed` for that bullet.

**Acceptance:** unit tests: `Parked from PRD-001 polish (2026-10-10)` gives `added 2026-10-10` and `origin PRD-001 polish`; `Added 2026-10-11` gives that date and no origin; a bullet with no phrase under `### Parked from the PRD-001 polish review (2026-09-22)` gives `2026-09-22` and `PRD-001 polish review`; one holding both its own Parked phrase and that heading takes its own; `Re-trigger: when PRD-004 ships. Then look.` gives `when PRD-004 ships.`; a bullet with no phrase takes `blame`.

### 3. Ideas into the migration plan — high — blocked by: 2
**Source**: §10 FR-20 Migration: "`ideas.md` is deleted. `--dry` prints one row per bullet: the target id or "not migrated", the state, and the rule that placed it, with guesses first, so the owner reads the risky rows before anything else." "**One markdown file per open idea, at `docs/ideas/YYYY-MM-DD-slug.md`.** The filename stem is the id; the date is the day it was added". "**`init` creates `docs/ideas/` instead of `docs/ideas.md`**, with a `README.md` holding today's inbox rules".

`pub fn plan(root: &Path, plan: &mut migrate::Plan) -> Result<()>`, called from slice 07's `migrate::plan` when `docs/ideas.md` exists. `run` writes the files with the rest of `Plan.files`, before `project.toml`.

- **Ids**: `{added}-{ideas::slug(title)}`, with `-2`, `-3`… on a clash, as slice 05's `add` names them. Clashes are counted against the files already in `docs/ideas/` and the ids planned before this one, in file order, so a re-run picks the same ids. `add`'s `create_new` loop is not used: the plan must name every id before anything is written.
- **Files**: one `docs/ideas/<id>.md` per migrated bullet, through slice 05's `ideas::render(&Idea, added)`. This slice splits that out of `add`, which keeps its `today()` and its `create_new` loop. `render` writes a value holding a `"` in single quotes, since the parser strips either outer quote with no escapes. It still refuses a value holding both quotes, or a newline. Migration then refuses with the line number, and the owner edits the line, commits and runs again. Slice 05's "a title holding `"` exits 2" test becomes a round-trip test of that title. `docs/ideas/README.md` is slice 05's asset, written only when absent. Then `delete docs/ideas.md`.
- **Links**: every link to `ideas.md` in the `.md` files slice 07 scans gets the relative path to `ideas/README.md` instead, anchor dropped. Each is listed (`relink docs/index.md:66 → ideas/README.md`). The README holds the rules those links meant. Unlinking, as slice 07 does for `agent-log.md`, would lose the pointer.
- **Rows**: one per bullet, `idea <id> <state>[ high] <rule>` or `idea not migrated <rule>`, plus one `drop` row per dropped line. **Guess rows go first in the whole plan output**, each starting `guess`, before slice 07's file rows, then the other idea rows in file order.

**Acceptance:** unit tests: two bullets titled `Same` with the same date give `…-same` and `…-same-2`, in file order; a planned id that already exists in `docs/ideas/` moves to `-2`; a title `Say "hi"` renders `title: 'Say "hi"'` and parses back to `Say "hi"`; a title holding `"` and `'` refuses the plan with its line; `plan` on the same tree twice gives equal rows and files.

### 4. The fixture `ideas.md` and the FR-20 migration test — high — blocked by: 3
**Source**: §10 FR-20 Acceptance: "in the v2 fixture, an `ideas.md` holding one bullet each under `## Open`, `## Open — high priority`, `## Owed by someone else`, `## Graduated`, and `### Parked from the PRD-001 polish review (2026-09-22)` below `## Graduated`, plus a multi-line body and a bullet with "Re-trigger:", migrates to exactly four files with the states, `priority`, `added`, `origin` and `retrigger` above; the Graduated bullet is not migrated; `--dry` lists the Parked bullet as a guess before the others; each body round-trips byte for byte."

`tests/fixture/v2/docs/ideas.md`: the node template's preamble and rules, then `## Open` with `- **Alpha.** Added 2026-09-01. First line` plus an indented second line; `## Open — high priority` with `- **Bravo** — no date here. Re-trigger: when PRD-002 ships.`; `## Owed by someone else — pending confirmations` with `- **Charlie.** Parked from PRD-001 polish (2026-09-15), the vendor owes the answer.`; `## Graduated → became a feature` with `- **Delta** — became PRD-001` and the template's placeholder; and then `### Parked from the PRD-001 polish review (2026-09-22)` with `- **Echo.** worth a look`. Commit it in the fixture. `node .docs/bin/docs.mjs check` stays clean on it (slice 07's fixture README).

`tests/migrate.rs`, `fn ideas_migrate()`:

1. `migrate --dry`: the first row starts `guess` and names `Echo`, and no other row before it names an idea. The rows for Alpha, Bravo, Charlie and Echo name their ids, Delta's row says `not migrated`, and the rules list and both placeholders are `drop` rows.
2. `migrate`: `docs/ideas/` holds exactly `README.md` and four idea files, and `docs/ideas.md` is gone. `factory docs idea list --json` gives: Alpha `open`, `added 2026-09-01`, no origin; Bravo `open`, `priority high`, `retrigger "when PRD-002 ships."`, and `added` the fixture's fixed author date (slice 07, task 7); Charlie `owed`, `added 2026-09-15`, `origin PRD-001 polish`; Echo `open`, `added 2026-09-22`, `origin PRD-001 polish review`. Each file's body equals the bytes after the title in `git show HEAD:docs/ideas.md`, separator stripped, continuation line included.
3. `factory docs check` exits 0 with **no warning at all**. Slice 07's `W-IDEAS-V2` allowance (its task 7 step 3) goes.
4. Slice 07's undo and re-run step now also compares `docs/ideas/` byte for byte.

**Acceptance:** `cargo test --test migrate` green; the test fails if Delta is migrated, if any of the four fields above differs, or if `check` prints a warning.

### 5. This repo's ideas in the PRD-001 copy proof — medium — blocked by: 4
**Source**: §6 FR-16 Acceptance: "Run on a copy of this repo's `docs/` with PRD-001 …: the same invariants hold." §12 Seed state: "a copy of this repo's `docs/` at the merge base". §10 FR-20 Migration: "`added` … else from `git blame` of the bullet's first line".

Extend slice 07's `#[ignore]` `prd_001_copy_migrates`: after the migration, `docs/ideas/` holds `README.md` and 19 idea files, `docs/ideas.md` is gone, the five links of Current code point at `ideas/README.md`, and `check` has no `E-` code. The `git archive` copy has one commit, so its blame dates are the copy's date. So the same test also runs `migrate_ideas::parse` and `fields` on this repo's own `docs/ideas.md`, with blame dates read in place (read-only), and asserts that the 8 bullets with a phrase take the phrase's date. It also asserts that the 11 without one take a date from 2026-10-09 (this repo's first PRD) on, and none later than HEAD's commit date. Its rows go into Progress.

**Acceptance:** `cargo test --test migrate -- --ignored` green at the checkpoint, with the 19 idea rows in Progress.

### 6. Verify — medium — blocked by: 1, 2, 3, 4, 5
`cargo test` (the inner loop; the full gate runs at this slice's checkpoint)

**Acceptance:** inner loop green, one commit.

### 7. Prove — at the checkpoint — blocked by: 6
Rows from [`prd.md`](prd.md) § QA plan: the migration half of the **FR-20** row, "`migrate` on the v2 fixture's `ideas.md`: four files, the Graduated bullet not migrated, the Parked bullet listed first as a guess, bodies byte for byte". Drive it on the real surface with the branch binary in a temp copy of `tests/fixture/v2`: `factory docs migrate --dry` (the head of its output), `factory docs migrate`, `ls docs/ideas`, `cat` of each idea file, `diff` of each body against the source bytes, `factory docs idea list --json`, `factory docs check`. Also run it in a `git archive HEAD docs` copy of this repo and list its `guess` and `drop` rows. The human half is in [`qa-guide.md`](qa-guide.md), not here: the owner reads the guess rows of `migrate --dry` on arca-api and decides which Parked bullets are still open.

**Acceptance:** Progress holds each transcript with its exit code, the four file names, four empty body `diff`s, and the `check` output with no warning line.

**Code-review fixes (2026-10-13).**

- A fence at the margin under an idea bullet ended the bullet (AD-802), but its lines vanished from the plan. FR-20 says dropped text is listed. Each line now gets a `drop` row ("in a fence under no bullet"). Test: `bounds_fences_and_drops`.
- `this_repos_ideas_at_head` counted the live `docs/ideas.md`, so any parked idea would turn it red. It reads the frozen copy now (AD-9906).
- The migration half of FR-20 is proven in `evidence/polish-fr20.txt`. `migrate --dry` lists the Echo guess first and leaves `git status` empty. `migrate` writes the four idea files and does not migrate Delta, and `check` is clean.
