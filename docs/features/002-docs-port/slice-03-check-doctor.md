---
slice: 03
status: done
depends_on: [2]
lane: 1
---

# Slice 03 — check and doctor (FR-15 part)

**Source**: [`prd.md`](prd.md) → §5 FR-15 (the `check` and `doctor` bullets, the exit codes), §4 FR-14 (the "`check` on the log" bullet). **Depends on**: 02 (the log rules read `log::entries`, and the lifecycle warnings read the open blocks and checkpoints slice 02 computes).
**Created**: 2026-10-13 · **Status**: done

**Progress**:
- **Built** (2026-10-13):
  - `src/docs/check.rs`: `Report` with the v2 print shape, and `run`. It covers the parse and key rules over `project.toml`, every `state.toml` and every PRD frontmatter, slice ↔ `[slices.N]`, deps and `find_cycle`, and `W-SIZE` minus the Progress region. It also covers `W-NO-QA-PLAN`, links, the frozen archive (untrimmed porcelain), second boards (pointing at `docs/ideas/`) and legacy dirs. Then come the FR-14 log rules, `W-SUMMARY-EMPTY`, and the lifecycle warnings on `state.toml` plus `log.jsonl`. Findings are collected per file, so one bad file never hides another.
  - `src/docs/doctor.rs`: `Framework`, `detect` and `run`, which never write.
  - `src/cli/docs.rs`: the `check` verb (exits 1 on errors) and the native `doctor` verb. `fn node(tool)` is shared by the exec path and doctor's captured `docs.mjs check`.
  - Tests: 13 unit tests in `check.rs` and 2 in `doctor.rs`. `tests/docs_check.rs` has 4: doctor on v3/v2/v1/none with exit 0 and an unchanged porcelain; check clean 0, warning-only 0, broken link 1, `--fix` and `--strict` 2; v2 `check` goes to the stub (exit 3) while `doctor` stays native; one `Command::new("node")` per module.
- **Inner loop** (`cargo test`): 3 runs, 1 red, about 3 min. The red run caught a unit test that guessed the moved folder's number; the allocator skips past the archive's 005.
- **Deviations**:
  - `check` scans `features/` and `archive/` through `board::scan_folder` instead of `board::scan` (AD-302).
  - A missing `prd.md` keeps `E-NO-PRD`, and a missing `state.toml` is `E-STATE-INVALID` (AD-301).
  - A v3 `project.toml` that does not parse still routes to `docs.mjs` (AD-303).
  - HEAD needs slice 01's uncommitted files to build (AD-305, decide).
- **Prove**: the FR-15 `check` and `doctor` rows of the verb → exit table are left for the checkpoint.

**Slice**: After this slice `factory docs check` validates a v3 repo natively: the parse and key rules over `project.toml`, every `state.toml` and every PRD frontmatter, the slice-file ↔ `[slices.N]` correspondence, deps and cycles, links, the frozen archive, legacy dirs and second boards, size ceilings, `W-NO-QA-PLAN`, the FR-14 log rules and the empty-summary warning on a `ready` feature. It exits 1 on errors. `factory docs doctor` says which framework version a repo is on (`v3`, `v2 (run factory docs migrate --dry)`, `v1`, `not on the framework`) and what `check` would say, in a v3 and a v2 repo alike, and never writes. Ideas checks are slice 05. `W-GATE-UNRESOLVED` is slice 04, which owns gate resolution. The pre-commit hook that calls `check` is slice 11.

## Current code / environment

- **The v2 check is one function to port selectively**: `docs.mjs:802-1106 check`, a `Report` with `err`/`warn` and `print` (`:791`): errors as `  ✗ CODE  msg`, warnings as `  ! CODE  msg`, then `N errors, M warnings`. The verb prints `docs check: clean` when both are empty and exits 1 on any error (`:2047-2052`). Warnings alone exit 0. `--strict` turned warnings into 1, and v3 drops it (prd §5).
- **The kept codes, and where each lives today**: `E-NUM-DUPLICATE` (`:837`), `E-NUM-MISMATCH` and `E-SLUG-MISMATCH` (`:857-860`), `E-STATUS-PLACE` (`:866-869`), `W-NO-QA-PLAN` (`:878`, live features at `ready` or `in-progress`), `E-SLICE-DUPLICATE` (`:906`), `W-SIZE` (`:849` PRD, `:909` slice), `E-DEP-MISSING` (`:926`), `E-DEP-CYCLE` (`findCycle`, `:1106`), `E-ARCHIVE-MODIFIED` (`:1040-1054`), `E-LINK-BROKEN` (`:1056-1068`), `W-SECOND-BOARD` and `W-LEGACY-DOCS` (`:1070-1082`, the lists at `:35` and `:39`). The PRD's keep list names these rules by content, not all by code. **The trap**: `W-SECOND-BOARD`'s message says "fold it into docs/index.md or ideas.md" (`docs.mjs:1074`); in v3 it says `docs/ideas/` (prd §10), since `ideas.md` is what `W-IDEAS-V2` warns about.
- **The slice size ceiling ignores the Progress region**: `docs.mjs:204-207` subtracts the text from `**Progress**:` up to `**Slice**:` before measuring, because "a done slice that trips W-SIZE on its own evidence cannot be split". Port that exactly. v3 slice files have no frontmatter (slice 01), so the measured text is the file minus that region.
- **The archive check reads the working tree, not only the index**: `docs.mjs:1042` runs `git status --porcelain -- docs/archive` **untrimmed** ("porcelain's first column is a significant space") and errors when the staged or the worktree column is `M` (`:1046-1048`). A move in (`R`, `A`, `??`) is how archiving works and passes. The PRD bullet says "staged modifications". The code also catches unstaged ones; port the code.
- **Links skip the archive and three schemes**: `:1058-1066`, regex `\[[^\]]*\]\(([^)\s]+)(?:\s+"[^"]*")?\)`, skips `http:`, `https:`, `mailto:` and `#…`, strips the `#fragment`, and never walks `docs/archive/` ("a link that went stale when the folder moved is the rule working").
- **The lifecycle warnings read the log**, and they are neither kept nor dropped by name in prd §5: `E-STATUS-INCONSISTENT` (`:968`, shipped or archived with open slices), `W-STATUS-BEHIND` (`:971`), `W-NOT-REVIEWED` and `W-READY-TO-SHIP` (`:977-979`), `W-BLOCKED-NO-ENTRY`, `W-CP-STALE`, `W-BLOCK-STALE` (`:944-951`), `W-NO-GATE` (`:827`). They check the lifecycle, which prd §15 leaves alone ("No change to the status vocabulary, the slice vocabulary, the lifecycle…"). This slice ports them onto `state.toml` and `log.jsonl`. `W-BLOCK-STALE` only looks at blocks with a `slice` (`:950`), so a phase-only block (polish, pr, ci) never trips it.
- **What the PRD drops**: every index-table and counter code (`E-NO-BOARD`, `E-INDEX-*`, `E-ARCHIVE-MISSING-ROW`, `W-NO-ARCHIVE-TABLE`, `W-ARCHIVE-UNLISTED`, `W-NO-COUNTERS`, `E-COUNTER-STALE`), `W-PROSE-STATUS`, `W-RETRO-FM`, `--fix` and `--strict`. `E-LOG-DUPLICATE` (`:936`) is replaced by the FR-14 rules. `E-FR-OVERLAP` and `E-FR-RETIRED-REUSED` read `owns_fr`, which the v3 frontmatter does not have (prd §3). `E-NO-INDEX` and `W-FRAMEWORK-BEHIND` describe a v2 repo, which never reaches native check (FR-17's shim). `W-NO-IDEAS` is slice 05's.
- **The v2 doctor is mostly a foundations report**: `docs.mjs:661-722` prints `Framework`, a requirements table, `drift` (check's counts), `gate`, `inner loop`, `harness`, `ux rubric`, `archive`, the upgrade guides, and has `--json` and `--scan` (`:2055-2060`). It never calls `process.exit`, so it exits 0 whatever it finds. The version is index.md frontmatter `framework`; absent is 1 (`loadProject`, `:300-305`). FR-15 asks for only the version line and "what `check` would say".
- **Skills route on doctor's output**: `~/.claude/skills/feature-docs/SKILL.md:20` and `:216` ("Run `docs doctor` and route on what it says"), `:36` and `:185-186` (`--scan`, `--json`), and the `/prd` and `/prd-to-tasks` preambles (`SKILL.md:20` in each). Those skills run node until slice 11 rewrites them. This slice must not change what `docs.mjs doctor` prints, and the native doctor's lines are what slice 11 rewrites them against.
- **Slice 01 routes `doctor` natively before the v3/v2 split** (slice 01 task 5: "`migrate` or `doctor` → native"). In a v2 repo the native doctor still needs check's verdict, and native check cannot read v2. It runs `docs.mjs check` through the shim's one `Command::new("node")` site in `src/cli/docs.rs`, captured instead of exec'd, so FR-18's grep still finds one site (prd §8 FR-18 acceptance).
- **Nothing in the driver calls check or doctor**: `grep -n '"check"\|"doctor"' src` hits only an unrelated column name in `src/cli/status.rs:302`. The callers are the skills, the owner and slice 11's hook.
- **Deliberate non-changes** — *(prd §15: "Not handled: `render`, `log merge`, `check --fix`, `--strict`… v3 stores nothing derivable." Do not add `--fix`, `--strict` or a `--json` the v2 check never had. prd §5: doctor "never writes". Do not add the v2 requirements table, `--scan` or the foundations lines here; FR-15 does not ask for them.)*

## Tasks

### 1. `check`: parse, keys and the slice ↔ state correspondence — high — blocked by: —
**Source**: §5 FR-15: "**`check`** keeps: `project.toml`, every `state.toml` and every PRD frontmatter parse with the required keys; every slice file has a `[slices.N]` entry and the reverse; `depends_on` names existing slices and has no cycle (`docs.mjs:1106 findCycle`)… size ceilings from `project.toml`; `W-NO-QA-PLAN`." "Exit codes as today: … `check` 1 on errors."

`src/docs/check.rs` (new): `pub struct Report { errors: Vec<(String, String)>, warnings: Vec<(String, String)> }` with `print` in the v2 shape above, and `pub fn run(root) -> Report`. It walks the board from slice 01 (`board::scan`) but reads each file itself, so a file that does not parse becomes a finding instead of an `Err`. The codes: `E-PROJECT-INVALID` (`project.toml` does not parse, or `[docs]` holds a value outside its type); `E-STATE-INVALID` (a `state.toml` that does not parse, or a feature or slice status outside the vocabulary); `E-FM-MISSING` and `E-FM-INVALID` (the seven PRD keys `number`, `slug`, `title`, `type`, `created`, `author`, `summary`); `E-NUM-DUPLICATE`, `E-NUM-MISMATCH`, `E-SLUG-MISMATCH`; `E-STATUS-PLACE`; `E-SLICE-DUPLICATE`; `E-SLICE-NO-STATE` (a `slice-NN-*.md` with no `[slices.N]`) and `E-STATE-NO-SLICE` (the reverse); `E-DEP-MISSING` and `E-DEP-CYCLE` (port `findCycle`); `W-SIZE` (PRD against `prd_max_kb` on live features, slice spec minus the Progress region against `slice_max_kb`); `W-NO-QA-PLAN` (live, `ready` or `in-progress`, no `QA plan` section, the board's `qa_plan`). Archive folders are checked only for `E-NUM-DUPLICATE` and `E-STATUS-PLACE`, through slice 01's archive status reader (a `state.toml`, else the v2 frontmatter `status`), so a frozen v2-shaped archive folder never needs v3 keys.

The trap: `state::read` and `board::status` return `Err` on a bad file, which is right for `status` and wrong here. One bad `state.toml` must not hide the findings of every other feature, so `check` collects per file and keeps going.

**Acceptance:** unit tests in `src/docs/check.rs` on temp trees built from `Fixture::v3`'s shape: the clean fixture has zero errors and zero warnings; each of these, one at a time, yields exactly its code: a `prd.md` without `summary` (`E-FM-MISSING`), `status = "later"` in `state.toml` (`E-STATE-INVALID`), a `slice-04-d.md` with no `[slices.4]` (`E-SLICE-NO-STATE`), a `[slices.5]` with no file (`E-STATE-NO-SLICE`), `depends_on = [9]` (`E-DEP-MISSING`), slices 2 and 3 depending on each other (`E-DEP-CYCLE`), `prd_max_kb = 1` (`W-SIZE` on the PRD), and a slice whose Progress region alone is 40 KB under a 30 KB ceiling (no `W-SIZE`); two broken files in two features give both findings in one run.

### 2. `check`: links, the archive, legacy dirs and second boards — high — blocked by: 1
**Source**: §5 FR-15: "links resolve (`E-LINK-BROKEN`, `:1066`); staged modifications under `archive/` (`E-ARCHIVE-MODIFIED`, `:1050`); legacy planning dirs and second boards (`W-LEGACY-DOCS`, `:1079`; `SECOND_BOARD_NAMES`, `:35`)."

`src/docs/check.rs`: port the three blocks as they are (Current code). Walk every `.md` under `docs/` except `docs/archive/`; that includes `docs/ideas/` (slice 05's files) and `docs/index.md`. Run `git status --porcelain -- docs/archive` through `std::process::Command` on `git`, untrimmed, and only when `git rev-parse --is-inside-work-tree` says `true` (`:1040`). Check the six second-board names in the repo root and in `docs/`, and the four legacy dirs in the root.

The trap: do not trim git's output before slicing columns. `" M docs/archive/…"` trimmed reads as staged `M`, tree `' '`, and the error fires on the wrong column, or not at all on `RM`.

**Acceptance:** unit tests: a link to `slice-09-missing.md` from the fixture PRD yields `E-LINK-BROKEN` naming both files; `[x](https://e.com)`, `[x](#qa-plan)` and a link with a `"title"` yield nothing; a broken link inside `docs/archive/001-old/prd.md` yields nothing; an archived folder edited and left unstaged yields `E-ARCHIVE-MODIFIED`, the same edit staged yields it, and a freshly `git mv`'d folder does not; `docs/TODO.md` and a root `plans/` yield `W-SECOND-BOARD` and `W-LEGACY-DOCS`.

### 3. `check`: the log rules and the lifecycle warnings — high — blocked by: 1
**Source**: §4 FR-14: "**`check` on the log** (FR-15): every line parses; ids are unique and increase per kind; every `answer` and `review` points at an id that exists." §5 FR-15: "the log rules in FR-14; an empty `summary` on a `ready` feature." §15: "No change to the status vocabulary, the slice vocabulary, the lifecycle…"

`src/docs/check.rs`: per live feature, `log::entries(dir)`. `E-LOG-PARSE` for each skipped line, with its line number; `E-LOG-DUPLICATE` for an id seen twice; `E-LOG-ORDER` for an id lower than the previous id of its sequence (`AD` shared by `decision` and `ask`, `BLOCK`, `CP`); `E-LOG-REF` for an `answer` or `review` whose `ref` names an id absent from the log. `W-SUMMARY-EMPTY` for a feature whose `state.toml` says `ready` and whose `summary` is empty or whitespace. Then the ported lifecycle warnings, on `state.toml` and the log: `E-STATUS-INCONSISTENT`, `W-STATUS-BEHIND`, `W-NOT-REVIEWED`, `W-READY-TO-SHIP` (reading `reviewed` from `state.toml`), `W-BLOCKED-NO-ENTRY`, `W-CP-STALE`, `W-BLOCK-STALE` (an open block is one no `answer` refs, as slice 02's board computes it), and `W-NO-GATE` when `project.toml` has no `gate`. Point every message at `factory docs …`, not at `docs …` or a frontmatter edit.

The trap: `log add` already refuses a dangling `ref` (slice 02 task 1), so the only way to get one into a test is to write `log.jsonl` by hand. Do that, not `log add`. A hand-edited or merge-conflicted log is exactly what `E-LOG-REF` exists for.

**Acceptance:** unit tests on hand-written `log.jsonl` files: a half line yields `E-LOG-PARSE` with line 3; `AD-1`, `AD-1` yields `E-LOG-DUPLICATE`; `AD-2`, then an `ask` `AD-1`, yields `E-LOG-ORDER`, while `AD-1`, `BLOCK-1`, `AD-2` yields nothing; an `answer` to `BLOCK-4` with no `BLOCK-4` yields `E-LOG-REF`; `status = "ready"` with `summary: |` and nothing under it yields `W-SUMMARY-EMPTY`, and the same at `draft` does not; slice 2 `blocked` with no open block naming it yields `W-BLOCKED-NO-ENTRY`; a `phase = "polish"` block with no slice yields no `W-BLOCK-STALE`.

### 4. `doctor` — high — blocked by: 1
**Source**: §5 FR-15: "**`doctor`** prints one of `v3`, `v2 (run factory docs migrate --dry)`, `v1`, `not on the framework`, plus what `check` would say; it never writes."

`src/docs/doctor.rs` (new): `pub enum Framework { V3, V2, V1, None }` and `pub fn detect(root) -> Framework`: `V3` when `docs/project.toml` has `[docs] framework = 3` (`project::is_v3`); else, when `docs/index.md` exists, its frontmatter `framework` through slice 01's frontmatter parser, `2` → `V2`, absent or `1` → `V1`; else `None`. `pub fn run(root) -> String`: the line `Framework      {state}`, then, for `V3`, the `gate` line (`project.toml` `gate`, or `none declared`) and `check::run`'s report in its print shape plus `drift          clean | N errors, M warnings`. For `V2` and `V1`, run `docs.mjs check` captured through the shim helper in `src/cli/docs.rs` (task 5) and print its output under the same `drift` line. When no `docs.mjs` is found, say so in that line. `None` prints the one line. Exit 0 in all four, as v2's doctor does; `check` is the verb that fails.

The trap: in a v2 repo there is no `docs/.gitignore` (slice 01 writes it only on `init`), so anything that creates `docs.lock` dirties the tree. Doctor must not call `state::lock`, `log::add` or any writer, and must not run `docs.mjs check --fix`.

**Acceptance:** `tests/docs_check.rs` (new): `doctor` prints `Framework      v3` in `Fixture::v3`; `v2 (run factory docs migrate --dry)` in the v2 fixture (`Fixture::new`, real vendored `docs.mjs`) with a `drift` line; `v1` in a repo whose `docs/index.md` has no `framework` key; `not on the framework` in an empty git repo; all four exit 0, and `git status --porcelain` is identical before and after each run.

### 5. The `check` and `doctor` arms and their exit codes — high — blocked by: 1, 2, 3, 4
**Source**: §5 FR-15: "Exit codes as today: 0 ok, 2 refused with the reason on stderr, `check` 1 on errors." Acceptance: "In the v3 fixture every verb above runs green in `cargo test` with the exit code stated."

`src/cli/docs.rs`: a `check` arm in the flat `DocsCli` match (prints `docs check: clean`, or the report on stderr; exits 1 on any error, 0 otherwise) and the native `doctor` route slice 01 reserved. Factor the shim's `Command::new("node")` into one `fn node(tool: &Path) -> Command` that the exec path and doctor's captured call share. `check --fix` and `check --strict` fall to clap's unknown-argument refusal, exit 2.

The trap: slice 01 routes `doctor` before the v3/v2 split, and `check` after it. A v2 repo's `check` must keep reaching `docs.mjs` through the shim, while its `doctor` does not. Test both in the v2 fixture.

**Acceptance:** `tests/docs_check.rs`: in `Fixture::v3`, `check` exits 0 with `docs check: clean`; after a broken link is added, exits 1 with `E-LINK-BROKEN` on stderr; with only `docs/TODO.md` added, exits 0 with `W-SECOND-BOARD`; `check --fix` exits 2. In the stub v2 fixture (`tests/fixture/stub-docs/docs.mjs`, slice 01) `check` prints `check` and exits 3, while `doctor` prints `Framework      v2 …`. `grep -rn 'Command::new("node")' src` lists exactly one line in `src/cli/docs.rs` plus `src/docs/mod.rs:65`, which slice 09 removes.

### 6. Verify — medium — blocked by: 1, 2, 3, 4, 5
`cargo test` (the inner loop; the full gate runs at this slice's checkpoint)

**Acceptance:** inner loop green, one commit.

### 7. Prove — at the checkpoint — blocked by: 6
Rows from [`prd.md`](prd.md) § QA plan: the **FR-15** row, its `check` and `doctor` part only: "each verb once in the fixture with its exit code" for `check` (clean 0, one error 1) and `doctor` (v3, v2, v1, none, all 0). The `--help` list, the other verbs and the `render`/`vendor`/`log merge` exits belong to slices 01, 04 and 05. Proven below the UI: the checkpoint runs `cargo test --test docs_check` and keeps the transcript, then runs `factory docs check` and `factory docs doctor` by hand in a `Fixture::v3`-shaped temp repo and in the v2 fixture. Evidence: the rows of the verb → exit table in this slice's Progress.

**Acceptance:** the FR-15 verb → exit table in Progress has its `check` and `doctor` rows, each with the command, the repo it ran in and the exit code.
