# Slice 05 — Retro columns and the friction line

**Source**: [`prd.md`](prd.md) → §6 FR-4 (the two columns, the friction prompt line, its acceptance); §7 FR-5 (its run row, rerun on the tree that ships). **Depends on**: 01 (ship calls the `scan` slice 01 wrote and stores the counts its `Report` carries)
**Created**: 2026-10-14

**Progress**: _(written as it lands — what was built, the gate numbers, what a review caught)_

**Slice**: After this slice, ship scans the feature's added lines against the default branch and stores two figures on the run: the comment share and the id hits in comments, names and strings. The retro's compare table shows both, for this run and the three earlier ones, with `—` for a run that has none. The retro template's `## Framework friction → skills repo` section gains one prompt line asking for a comment or name the check missed, with `file:line`, and closeout files that section as an idea when the line is filled, as it files any friction today.

## Current code / environment

- **The store's run columns**: `src/db.rs:115` `const COLUMNS: [(&str, &str, &str); 29]`, added with `ALTER TABLE ADD COLUMN` guarded by the live column list (its doc line, `:114`). The last two are `("runs", "lines_added", "INTEGER")` and `("runs", "lines_removed", "INTEGER")`: follow them. No schema version moves.
- **`RunRow` derives `Eq`** (`src/db.rs:181-203`), so a float field does not compile. Store the two counts the share is made of, not the share: the added comment lines and the added non-blank lines that slice 01's `Report` carries, plus the id-hit count. Three `INTEGER` columns, e.g. `comment_lines`, `nonblank_lines`, `id_hits`; the array becomes 32.
- **Where the run row is read**: two SELECTs list the columns by hand, `run_row` (`src/db.rs:431-442`) and `earlier_runs` (`:446-465`), and `run_from_row` (`:971-1004`) reads `lines_added` by name with `.map(|n| n.max(0) as u64)`. All three gain the new columns, or the compare table reads `None` for every run.
- **The setter to copy**: `set_run_lines` (`src/db.rs:467-473`), an `UPDATE runs SET … WHERE id = ?1` taking an `Option`, so a failed measurement stores `NULL`.
- **Ship**: `src/closeout.rs:861-869` takes `let base = default_branch(run, docs);`, measures `retro::changed_lines(&repo, &base)`, and on an error says `ship: no changed lines stored — {e}` and stores `None`. `report::compute` follows (`:870`), then the template stand-in (`:871-881`), `retro_row` (`:882`) and `retro::write(&retro, &retro::numbers(...))` (`:883`). `repo` is `run.state().repo`, the checkout on the feature branch. The ship commit after it (`:887`) touches only `docs/`, which the scan excludes, so a scan taken before it reads the tree that ships.
- **The compare table**: `src/retro.rs:200-213`. With no earlier finished run it prints `Compared with: {NO_EARLIER}` and no table (`:200-201`); otherwise the header `| run | feature | weighted | slices | per slice | lines changed | per line |` (`:205`), the alignment row (`:208`), this run's `compare_row` and up to `COHORT = 3` earlier ones (`:16`). `compare_row` (`:121-145`) builds each row; `dash` (`:91-93`) prints `—` for `None`. `factory retro <id>` regenerates the section from the store (`tests/closeout.rs:742`), so the figures are stored once at ship and never rescanned.
- **A repo's first run shows no table**, so its figures first appear as an earlier row in the next run's retro. That is today's behavior for every column, and FR-4 asks for the trend "run over run", not for a first-run table. Leave it.
- **The template**: `assets/retro.md`, compiled in as `pack::RETRO` (`src/pack.rs:18`) and returned by `retro_template()` (`src/closeout.rs:2256-2258`). Its friction section (`assets/retro.md:33-35`) holds one line: "A step, rule or template in the framework itself. Name the skill and the section." `assets/` is not in the skills bundle (`src/skills.rs:1`, `skills/bundle.lock`), so this slice moves no version.
- **Friction filing**: `file_friction` (`src/closeout.rs:321-354`) reads the retro's section with `friction` (`:356-366`) and files it in the `[source]` checkout only when it is not empty and differs from `friction(&retro_template())` (`:335`). A section the worker left as the template files nothing.
- **Two tests pin the template's one-line section**: the unit test `the_friction_section_is_read_to_the_next_heading_or_the_end` (`src/closeout.rs:2418-2423`) and `PLACEHOLDER` (`tests/closeout.rs:2032-2033`), which `a_placeholder_only_friction_section_files_nothing` (`:2139-2155`) writes as the whole section.
- **The stub**: its `friction` key writes the step's text as the retro's whole friction section (`tests/stubs/claude:330`); `skip: ["retro"]` leaves the retro out, and ship creates it from the template (`stood_in_run`, `tests/closeout.rs:792-815`). A slice step with no `touch` writes `src/slice-NN.txt` (`tests/stubs/claude:445`), an `id-name` path hit, so a test that pins the id-hit count names every slice's `touch`.
- **Tests that read the compare rows**: `a_second_run_of_the_repo_compares_with_the_first` (`tests/closeout.rs:685-745`) asserts the first run's row whole at `:736-740`; the unit test `numbers_compares_with_the_three_newest_earlier_runs_of_the_repo` (`src/retro.rs:366`) builds runs with `insert` (`:348-363`).
- **Deliberate non-changes**: *(prd §11 "Not handled: a `## Leak candidates` retro section.")* One prompt line in the existing friction section, no new section and no new filing path. The `retros` table (`RETRO_COLUMNS`, `src/db.rs:147`) gains nothing: FR-4 names the numbers table only.

## Tasks

### 1. Store the figures — high — blocked by: —
**Where**: `src/db.rs` (`COLUMNS`, `RunRow`, the two SELECTs, `run_from_row`, a setter).

**Source**: §6 FR-4: "**Two columns** in the numbers table, filled from `factory leaks --base` at ship time: the added comment share and the id hits in comments, names and strings."

Add the three columns to `COLUMNS`, three `Option` fields to `RunRow` (comment lines, non-blank lines, id hits, each `None` when the scan did not run), read them by name in `run_from_row`, add them to both SELECTs, and add `set_run_leaks(run_id, Option<…>)` beside `set_run_lines`. An old store gains the columns on open, `NULL` for every run already in it.

**Acceptance:** a unit test in `src/db.rs`: a run with the figures set reads them back from `run_row` and from `earlier_runs`; a run without them reads `None`.

### 2. Measure at ship — high — blocked by: 1
**Where**: `src/closeout.rs:861-870`.

**Source**: as task 1, "filled from `factory leaks --base` at ship time".

After `set_run_lines`, call slice 01's `scan` with the same `repo` and `base`, and store its comment lines, non-blank lines and id-hit count. The library call is the verb's work without a process: same report, same allowlist. On an error, say `ship: no leaks figures stored — {e}` and store `None`, as the changed-lines measurement does. The scan never stops the ship.

**Acceptance:** task 4's run reads the stored figures back from the store.

### 3. Two columns in the compare table — high — blocked by: 1
**Where**: `src/retro.rs:121-145`, `:205-208`.

**Source**: §6 FR-4: "They are compared with the three earlier runs, as the table already does. This is how the owner sees whether FR-1 and FR-2 work, run over run."

The header gains `comment share` and `id hits` after `per line`, and the alignment row two more `---:`. `compare_row` prints the share as slice 01's report does, one decimal and a percent sign (`66.7%`), and the id hits as a count. A run with no figures, or with no non-blank added lines, prints `—` in that cell.

**Acceptance:** the unit test at `src/retro.rs:366` sets figures on one run and none on another: the first's row ends `| 66.7% | 1 |` for 2 comment lines of 3 and one id hit, the second's `| — | — |`, and the header holds both names.

### 4. A two-run stub test shows both columns — high — blocked by: 2, 3
**Where**: `tests/closeout.rs:685-745`.

**Source**: §6 FR-4 acceptance: "A stub run to ship writes a `retro.md` whose numbers table holds both columns".

Extend `a_second_run_of_the_repo_compares_with_the_first`. Its plan names every slice step's `touch` (no `src/slice-NN.txt`), and its first slice step's `sh` writes a `.rs` file of known shape: two comment lines, one of them an id comment such as `// FR-2: guard`, and one code line. The second run replays the same plan, so the `sh` must leave the same file when it runs again; the touched files keep each commit non-empty. Both runs then store 2 comment lines of 3 and one id hit. Extend the whole-row assert at `:736-740` with `| 66.7% | 1 |`, and assert the header line.

**The trap**: the table renders only when an earlier run exists (`src/retro.rs:200-201`). A one-run test sees `Compared with: No earlier finished run in this repo.` and no column, whatever the code does.

**Acceptance:** the extended test passes; with task 3's cells removed it fails on the row assert.

### 5. The friction prompt line — high — blocked by: —
**Where**: `assets/retro.md:33-35`; `src/closeout.rs:2418-2423`; `tests/closeout.rs:2031-2033`.

**Source**: §6 FR-4: "**One prompt line in `## Framework friction → skills repo`:** "A comment or name the leaks check should have caught: `file:line` and the line." Closeout already files that section here as an idea, so a missed class reaches this repo's board … Evidence only, with `file:line`: the template's own warning applies, an agent asked for candidates invents them."

Add the line under the existing one, a blank line between them, word for word as the PRD gives it.

**The trap**: the line changes what `friction(&retro_template())` returns. Left alone, `PLACEHOLDER` (`tests/closeout.rs:2032`) holds only the old line, which no longer equals the template's section, so `a_placeholder_only_friction_section_files_nothing` files a placeholder as an idea and fails. Set `PLACEHOLDER` and the unit test's expected text to the new two-line section.

The new line says "A comment", not "in a comment", so FR-2's grep row (`grep -rniE "in a comment|code comment" skills/ assets/`) stays empty.

**Acceptance:**
- `a_retro_the_polish_worker_left_out_is_a_flagged_stand_in` (`tests/closeout.rs:824`) also asserts the template retro holds the new line.
- A new test like `a_friction_section_is_filed_as_an_untracked_idea_in_the_source` (`:2076`) writes the friction section as the template's two lines plus one filled entry, e.g. ``- `src/order.rs:3` `// until slice 4` ``. The idea file in the `[source]` checkout holds the entry, and the driver log says `friction: filed …`.
- `a_placeholder_only_friction_section_files_nothing` passes with the new `PLACEHOLDER`.

### 6. Verify — medium — blocked by: 1, 2, 3, 4, 5
`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 7. Prove — at the checkpoint — blocked by: 6
Rows from [`prd.md`](prd.md) §9 this slice proves, below the UI:

- **FR-4**: "run a stub run to ship: `retro.md` holds the two new columns and the friction line; with the line filled, closeout files an idea in the `[source]` checkout" — the transcripts of task 4's test and task 5's two tests, with the second run's `## Numbers` compare table and the filed idea file read back.
- **FR-5**, on the tree this checkpoint covers, once slice 03 is done: "run `factory leaks --all` on the feature branch, which holds the merged cleanup: zero `fix` hits outside the allowlist, and `factory docs doctor` prints `leaks    clean`; the gate is green". Run both with the lane's build, as slice 03's Prove does. Slice 03 owns this row; it is named here so the final checkpoint reruns it on the tree that ships, whichever slice lands last. If slice 03 is not done when this checkpoint runs, Progress says so and slice 03's own checkpoint runs it.

**Acceptance:** after the checkpoint, Progress holds the three test transcripts, the compare table and the idea file, and the `--all` and `doctor` output or the line saying slice 03 runs it.
