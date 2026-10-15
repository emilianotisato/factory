# Slice 03 — factory review collect

**Source**: [`prd.md`](prd.md) → §5 FR-3. **Depends on**: 01 (the second opinion runs through the parallel pass runner slice 01 moved into the `review` library; both slices edit `src/cli/review.rs` and `tests/review.rs`)
**Created**: 2026-10-15

**Progress**: _(written as it lands — what was built, the gate numbers, what a review caught)_

**Slice**: After this slice, `factory review collect --base <branch> [--out <file>]` does polish's three mechanical steps and writes one markdown file: the head line, the plan (changed files and lines, the size row), the leaks scan with its verdict, and the second opinion when the size row asks for it. It exits 0 when the file was written, whatever each part found, and 1 on a usage error. The work lives in the library, so slice 04's driver calls it in-process. Slice 05 points the skill at it.

**Highest risk**: task 1's file format. Slice 04 builds the pack's `# Collected` section from this file, including a file an earlier driver process wrote, so each part needs a heading it can find.

## Current code / environment

- **The verb enum**: `ReviewVerb` has one variant, `Second(SecondArgs)` (`src/cli/review.rs:15-19`), dispatched from `src/main.rs:39,85`. `review()` opens with the irrefutable `let ReviewVerb::Second(a) = verb;` (`:48`), which stops compiling once a second variant exists. `USAGE` (`:40`) lists only `second`. The module doc (`:1-2`) is the exit-code contract of `second`: 0 a pass counted, 2 none did or clap refused a flag, 1 a bad combination.
- **Usage errors by hand**: `SecondArgs` takes every flag as an `Option` and checks the combination itself (`:49-57`), so a bad call exits 1. A clap-`required` flag would make clap exit 2 instead, which is `second`'s "unreviewed" code. A base starting with `-` is refused there (`:50-51`) and by `leaks::scan` (`src/leaks.rs:751-752`).
- **The plan's numbers**: `review::numstat(cwd, base, paths)` (`src/review.rs:83-94`) diffs `<base>...HEAD` and returns `(path, added + deleted)` per file (`parse_numstat`, `:71-80`), paths relative to the repo root.
- **The second opinion, as `second` runs it**: config from `config::load(&paths::config_dir())` (`src/cli/review.rs:62-66`); no changed file → `unreviewed: no changes against {base}`, exit 2 (`:74-77`); `review::group(&files, cfg.pass_lines)` (`:78`); a log dir from `review::new_log_dir(&paths::data_dir())` and the probe (`:91-95`); then slice 01's runner. The section text and whether a pass counted come from `body()` (`:169-218`), which lives in the CLI module today: move it, and the section header format (`:121-129`), into the library so `second` and the collect print the same thing.
- **The trap: `second` probes before anything else** (`:91`). The collect must decide the size row first: a branch the size row keeps local never calls `cursor-agent`, not even `--list-models`.
- **The leaks part**: `factory leaks --base` is `leaks::scan(&root, Input::Base(base))` then `report.render()` (`src/cli/leaks.rs:22-37`); a scan error is "did not run — {e}" (`:29-31`). `render()` (`src/leaks.rs:518-545`) prints each rule's `fix:` line above its hits and ends with `clean`, `1 fix hit` or `N fix hits`. `Report::fix_hits()` (`:494`) gives the verdict.
- **The fixture**: `tests/review.rs` `fixture(name, timeout)` (`:17-33`) commits five files on `feat/review` off `main` and sets `[review] timeout_secs`; `scenario` (`:35-37`) writes the stub's `cursor-agent.json`; `calls` (`:51-59`) reads every stub call, `--list-models` included. A branch with other files is a few lines in the same style.
- **Deliberate non-changes**: *(prd §13 "Not handled: `--others` inside a run.")* The collect takes no `--others`; `factory review second --others` stays for attended use. *(prd §5 "Accepted cost: the 50-line threshold is a number, not the table's judgment.")* No path or content rule moves a small change to the second opinion. *(prd §5 "The security and UX passes stay the skill's call (FR-7).")* The plan names no security or UX pass.

## Tasks

### 1. The collect, in the library — high — blocked by: —
**Where**: a new library module (e.g. `src/collect.rs`), `src/review.rs`, `src/cli/review.rs:121-129,169-218`.

**Source**: §5 FR-3: "does the three mechanical steps and writes one markdown file: 1. **Plan.** The changed files and lines against `<base>...HEAD`, and the size row: one file and under 50 changed lines needs the local review only; anything larger adds the second opinion. … 2. **Leaks.** The same scan as `factory leaks --base <base>`, with its verdict (clean, hits, or did not run) and its hits under their fix lines. 3. **Second opinion.** `factory review second --diff <base>`, only when the size row asks for it, with its exit code, each pass's status line, and the findings text. When it is unreviewed, the plan says `fallback owed`: the worker runs the Claude fallback. The file starts with `<!-- head: <sha> -->`, the HEAD it reviewed."

One function takes the repo dir, the base and the review config, and returns the file's text (or a value that renders to it). In order: the head sha (`git rev-parse HEAD` in the repo dir), `numstat`, the size row, the leaks scan, then, only when the size row asks, the probe and the passes. A probe failure or no pass counted is unreviewed, with the reason, and the plan says `fallback owed`. A second opinion skipped by the size row says so in its section.

Give each part a fixed heading and keep the plan's size row and second-opinion state on lines of their own, so slice 04 can lift the plan, the leaks verdict and each pass's status line out of the file. Each pass's status line is its section header plus whether it counted.

The trap: a git error in the head or the numstat is the function's error; slice 04 turns it into `collect failed: <reason>`. A leaks error is not: it is the leaks part's "did not run".

**Acceptance:** task 3's tests.

### 2. The verb — high — blocked by: 1
**Where**: `src/cli/review.rs:1-57`, `USAGE` (`:40`).

**Source**: §5 FR-3: "A new verb, `factory review collect --base <branch> [--out <file>]` … Exit 0 when the file was written, whatever each part found; exit 1 on a usage error."

A `Collect` variant beside `Second`, with `--base` and `--out` as `Option`s checked by hand, so a missing base or one starting with `-` exits 1 with the usage line, as `second` does (see Current code). It runs the collect from the current directory's repo root and writes the file to `--out`. Without `--out` the PRD names no default: print the file to stdout, and log that as a decision. Add the collect's exit codes to the module doc.

**Acceptance:** task 3's usage test.

### 3. The collect's tests — high — blocked by: 2
**Where**: `tests/review.rs`.

**Source**: §5 FR-3 acceptance: "On the fixture repo with a two-file branch and the stub `cursor-agent`, `factory review collect --base main --out f.md` exits 0 and `f.md` holds the head line, the plan with its size row and changed files, the leaks verdict and one second-opinion section per pass. With the stub logged out, the file says unreviewed and the plan says `fallback owed`. On a one-file branch under 50 changed lines, the stub `cursor-agent` is never called and the file says the second opinion was skipped by size."

- a two-file branch: exit 0; the first line is `<!-- head: <sha> -->` with the branch's HEAD; the plan names both files and the size row; the leaks verdict; one pass section per pass the diff splits into;
- the same branch with `list_models: "auth"`: exit 0; unreviewed with the probe's reason; the plan says `fallback owed`;
- a one-file branch under 50 changed lines: exit 0; `calls(&fx)` is empty; the file says the second opinion was skipped by size;
- no `--base`, and `--base -x`: exit 1, the usage line, no stub call.

**Acceptance:** the four tests above, named by behavior.

### 4. Verify — medium — blocked by: 1, 2, 3
`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 5. Prove — at the checkpoint — blocked by: 4
Rows from [`prd.md`](prd.md) §11 this slice proves, below the UI:

- **FR-3**: "run `factory review collect` on a fixture branch: one file with the head line, plan, leaks and second opinion; logged out, `fallback owed`; a small one-file branch never calls the stub" — task 3's test transcript, and the written file of the two-file case.

**Acceptance:** after the checkpoint, Progress holds the transcript and the two-file case's file.
