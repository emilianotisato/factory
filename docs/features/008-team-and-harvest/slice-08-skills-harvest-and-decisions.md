# Slice 08 — The skills: /harvest, decisions without numbers, 0.6.0

**Source**: [`prd.md`](prd.md) → §3 FR-6, FR-7, and the skill lines of FR-2 and FR-5. **Depends on**: 05 (the skill runs `docs harvest --done`, and both edit `assets/docs/convention.md`), 06 (the skill runs `factory upstream`)
**Created**: 2026-10-15

**Progress**:
- Built: `skills/harvest/SKILL.md` (six steps, upstream draft → `check` → owner's yes → `post`, `docs check` after `--done`), its `.claude/skills/harvest` link, `NAMES`/`FILES`, the convention line, init counts 21 files / 10 links. Decisions are named entries cited as `docs/decisions.md#{slug}` across polish, ux, gate, autonomy, both contracts, the PR body and `skills-decisions.md:3`; polish's template is `## {Short name}`. Polish and slice close out with `docs ship` only; prune's evidence line names `$FACTORY_EVIDENCE`. `no_skill_or_asset_asks_for_a_numbered_decision` in `tests/skills_bundle.rs` (red on an appended `DEC-1` line, checked by hand and reverted). 0.6.0, lock rewritten once.
- Inner loop: 3 runs (1 relock with `FACTORY_BUNDLE_WRITE=1`, 2 plain), 1 red, 4 min — caught: `cargo fmt` on the new test.
- Deviation: `skills/README.md` gains a `harvest` row; it sits outside the bundle hash.
- Prove (FR-7) left for the checkpoint.

**Slice**: After this slice, the bundle holds a tenth skill, `/harvest`, which walks the owner through a batch's lessons, routes each one, converts `decisions.md`, and archives the batch. Polish writes decisions as named entries with no number, and no skill or asset asks for `DEC-n`. Polish and slice close out with `docs ship` only, and prune names the run dir for evidence. The crate is 0.6.0 with the bundle lock rewritten. Every `skills/` edit of this feature sits here, so one bump covers them. Converting this repo's own `decisions.md` is the owner's first `/harvest`, after the merge.

**Highest risk**: the bump. Any edit under `skills/` after task 5 changes the bundle hash, and the lock refuses a second hash at 0.6.0 (`src/skills.rs:157-173`). Land every skill text first, relock last.

## Current code / environment

- **The bundle**: `NAMES: [&str; 9]` (`src/skills.rs:21-23`). `FILES` is sorted by path (`files_are_sorted_without_duplicates`, `src/skills.rs:859`): `harvest/SKILL.md` goes between `harness/references/skill-template.md` and `lanes/SKILL.md` (`:60-63`). `every_file_under_skills_is_in_the_bundle_byte_for_byte` (`tests/skills_bundle.rs:26`) fails a file on disk with no entry. `this_repos_claude_skills_link_into_the_source` (`:50`) wants `.claude/skills/harvest -> ../../skills/harvest`, committed.
- **The init test's counts**: `tests/init.rs:130` (20 files), `:135` (9 links), and the report strings at `:208` and `:383` ("(20 files, factory.toml)", "(9 links)"). They become 21 and 10.
- **The lock**: `skills/bundle.lock` pins `0.5.0` and one hash. `relock` refuses a new hash unless the version is past the lock's (`src/skills.rs:157-173`); the fix it prints is "bump version in Cargo.toml, then `FACTORY_BUNDLE_WRITE=1 cargo test`" (`:152`). A skill fix found at polish needs the next bump (`docs/features/007-polish-in-the-binary/retro.md:31`).
- **The style to copy**: `skills/lanes/SKILL.md`: frontmatter with `name` and a `description` that names its triggers (`:1-4`), the attended paragraph (`:12`), `AskUserQuestion` with the recommendation first (`:44`), one commit at the end (`:92-94`).
- **The convention's skill list**: `assets/docs/convention.md:8` lists eight skills; slice 05 adds a history line to the same file. Keep to `:8`.
- **The `DEC-n` texts**: `skills/polish/SKILL.md:22,87,110,111,147,159,196` cite `DEC-n`; `:163,173,210` say "DEC"; the entry template is `:175-184` and the numbering rule `:186`. Also `skills/ux/SKILL.md:42`, `skills/gate/SKILL.md:40`, `assets/autonomy.md:17,23,38`, `assets/contract-slice.md:17`, `assets/contract-merge.md:18` ("a DEC"), `assets/pr-body.md:16` ("DEC candidates") and `docs/reference/skills-decisions.md:3` ("as `DEC-n`"). Today's entries look like `docs/decisions.md:15`: `### DEC-1 — How does the driver commit? · 2026-10-11 · PRD-001 polish`.
- **The closeout lines**: `skills/polish/SKILL.md:197` and `skills/slice/SKILL.md:155` offer `docs ship` or `docs archive`. `skills/prune/SKILL.md:28` says checkpoint evidence lives "in the feature folder or the harness artifacts".
- **The words that stay**: the contracts' `DECISIONS:` return line is the `AD-n` list (`assets/contract-slice.md:29`). The leaks rule keeps `DEC-\d+` (`src/leaks.rs:43,157`). `tests/fixture/v2/` holds `DEC-n` and is v2 input, outside `skills/` and `assets/`.
- **The commit hook runs the check**: `factory docs hook` runs `factory docs check` on any staged `docs/` path (`src/docs/hook.rs:22-25`), and `links()` fails a live doc linking into a folder that moved (`src/docs/check.rs:734-767`). The skill's commit meets both after `--done`.
- **Deliberate non-changes**: `docs/decisions.md` is not edited here: *(prd §3 FR-7 "Converting an existing `decisions.md` is the harvest skill's step 3.")*, and a run never writes it (`assets/autonomy.md:45`). *(prd §3 FR-7 "Records keep their ids: logs, PR bodies, retros, the archive and `skills-decisions.md`.")* Only `skills-decisions.md:3` changes. *(prd §7 "Not handled: checking that links into `decisions.md` resolve.")* *(prd §7 "Not handled: retiring a dead decision.")* No `## Retired` list in the template. *(prd §3 FR-6 "It is attended only, with the owner present, and never a run role.")* No contract asset and no driver role for it. Tagging 0.6.0 is the owner's, after the merge.

## Tasks

### 1. The `/harvest` skill — high — blocked by: —
**Where**: a new `skills/harvest/SKILL.md`, the link `.claude/skills/harvest`, `src/skills.rs:21-23,60-63`, `assets/docs/convention.md:8`, `tests/init.rs:130,135,208,383`.

**Source**: §3 FR-6: "A new skill, `skills/harvest/SKILL.md`, in the bundle and in the convention block's skill list. It is attended only, with the owner present, and never a run role. Its steps:
1. `factory docs harvest [NNN…]` for the lessons.
2. Each lesson, with the owner, by `AskUserQuestion` with the recommendation first, goes to one of four places. **The project**: a diff here, in the harness, the gate, `CLAUDE.md`, `docs/reference`, `docs/ux-rubric.md` or a `decisions.md` entry. **The framework in this project**: a `docs/project.toml` knob. **Factory itself**: `factory upstream` (FR-8), or `factory docs idea add` when this repo is the factory checkout. **Nowhere**: dropped, said aloud.
3. Compact `docs/decisions.md` by FR-7's rules, as one diff the owner reviews.
4. On a repo's first harvest: add FR-5's convention line to `CLAUDE.md` and `AGENTS.md`, update `docs/index.md`'s archived definition, and rewrite by hand the home paths `W-HOME-PATH` still reports outside the batch (the friction ideas' `From` lines, `research.md`).
5. Move a record worth keeping to `docs/reference`, then run `factory docs harvest NNN… --done`.
6. Commit on a new `harvest/<date>` branch when on the default branch, else on the current one. Never push."

And §3 FR-8: "The skill rewrites a lesson in general terms; the verb is the last check before anything leaves."

The skill, in `lanes`' shape: frontmatter (`name: harvest`, a `description` with its triggers, `argument-hint: [NNN…]`), the attended paragraph, then the six steps. For **Factory itself**, the skill writes the draft in general terms, shows it to the owner, runs `factory upstream check`, offers a comment by hand on a listed match, and posts with `factory upstream post` only on the owner's yes. `NAMES` gains `harvest`, `FILES` its entry, the convention line `/harvest`, and the init test its counts.

The trap: step 5's `--done` moves folders that live docs may link into, and the commit hook runs `factory docs check` (`src/docs/hook.rs:22-25`). The skill runs `factory docs check` after `--done` and fixes each `E-LINK-BROKEN` before it commits.

**Acceptance:** the bundle tests pass with the new file and link; `factory init` in a fresh repo writes `.claude/skills/harvest/SKILL.md` and reports 21 files and 10 links.

### 2. Decisions without numbers — high — blocked by: —
**Where**: `skills/polish/SKILL.md:22,87,110-111,147,159,163,173,175-186,196,210`, `skills/ux/SKILL.md:42`, `skills/gate/SKILL.md:40`, `assets/autonomy.md:17,23,38`, `assets/contract-slice.md:17`, `assets/contract-merge.md:18`, `assets/pr-body.md:16`, `docs/reference/skills-decisions.md:3`.

**Source**: §3 FR-7: "An entry is `## {Short name}` with **Chose**, **Over**, **Because** and **Applies when**, and no number, date or origin. Its link is the heading's own GitHub anchor, the slug." "A renamed or merged entry keeps its old slugs and its `dec-n` as `<a id="…"></a>` lines above the heading, so every existing link resolves." "A reversed or merged loser folds into the winner's **Over**." "Polish writes new entries in this form and every skill cites `decisions.md#{slug}`. `autonomy.md`'s "does the PRD, the slice, or a `DEC-n` settle this" reads "or `docs/decisions.md`", and the PR body's "DEC candidates" becomes "Decision candidates"."

The polish template becomes the `## {Short name}` form with the four lines. The `:186` rule becomes the slug and anchor rule and the fold into **Over**. Every other line cites a decision by `docs/decisions.md#{slug}` or says "a decision in `docs/decisions.md`". `pr-body.md:16` only: slices 03 and 05 edit `:10`, `:30` and `:32`.

The trap: polish still meets unconverted files. This repo's three numbered entries stay until its first harvest (`docs/decisions.md:15,22,29`), and so do arca-api's. Word the lookup at `:108-111` by the question and **Applies when**, not by the heading's form.

**Acceptance:** task 4's test; the template in `skills/polish/SKILL.md` holds `## {Short name}` and no `###` entry heading.

### 3. The closeout and evidence lines — high — blocked by: —
**Where**: `skills/polish/SKILL.md:197`, `skills/slice/SKILL.md:155`, `skills/prune/SKILL.md:28`.

**Source**: §3 FR-5: "**The closeout ships, never archives.** `skills/polish/SKILL.md:197` and `skills/slice/SKILL.md:155` say `factory docs ship`." §3 FR-2: "`skills/prune/SKILL.md:28` stops naming the feature folder."

Polish and slice close out with `factory docs ship <NNN>`, and say archiving is `/harvest`'s. Prune's evidence line names the run dir (`$FACTORY_EVIDENCE`) and the harness artifacts.

**Acceptance:** no `docs archive` in either closeout line; no "feature folder" in prune's evidence line.

### 4. The test that no text asks for `DEC-n` — high — blocked by: 1, 2
**Where**: `tests/skills_bundle.rs` (its `walk`, `:13`).

**Source**: §3 FR-7 acceptance: "No skill or asset tells an agent to write or cite `DEC-n`: a bundle test greps `skills/` and `assets/` for `DEC-` and allows only the leaks pattern and history lines."

Walk `skills/` and `assets/`, read each file, and fail naming every line holding `DEC-`, case-sensitive. The leaks pattern lives in `src/leaks.rs`, outside both trees, and no history line sits in them after task 2, so the test allows none.

The trap: the harvest skill must describe the old numbered entries without the literal. Name them by their `dec-n` anchors, which the case-sensitive match lets through.

**Acceptance:** a test named for the behavior (e.g. `no_skill_or_asset_asks_for_a_numbered_decision`), green after tasks 1 and 2 and red on a line holding `DEC-1`.

### 5. 0.6.0 — high — blocked by: 1, 2, 3, 4
**Where**: `Cargo.toml:3`, `Cargo.lock`, `skills/bundle.lock`.

**Source**: §3 FR-6 acceptance: "The skill renders through `factory init` and passes the bundle test."

Bump to `0.6.0`, run `FACTORY_BUNDLE_WRITE=1 cargo test` once to rewrite the lock, then the inner loop. No tag.

**Acceptance:** `skills/bundle.lock` says `0.6.0`, and the inner loop is green without `FACTORY_BUNDLE_WRITE`.

### 6. Verify — medium — blocked by: 1, 2, 3, 4, 5
`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 7. Prove — at the checkpoint — blocked by: 6
Rows from [`prd.md`](prd.md) §5 this slice proves, below the UI:

- **FR-7**: "run: the bundle test that no skill or asset asks for `DEC-n`; the conversion itself rides on the FR-6 row" — task 4's test transcript.

FR-6 is the Human row: the owner's first `/harvest` after the merge, in [`qa-guide.md`](qa-guide.md). No checkpoint drives it.

**Acceptance:** after the checkpoint, Progress names the transcript.
