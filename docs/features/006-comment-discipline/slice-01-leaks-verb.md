# Slice 01 — The `factory leaks` verb

**Source**: [`prd.md`](prd.md) → §5 FR-3 (the verb, the input, the rule table, the allowlist; its first acceptance bullet), §7 FR-5 (the ten-step order the fix lines come from, the false positives the tuning keeps out). **Depends on**: — (the spine: slices 03, 04 and 05 call the library this slice writes)
**Created**: 2026-10-14

**Progress**:
- **Built.** `src/leaks.rs`: the nine-row `RULES` table (id, title, looks, detector, severity, fix line), the per-extension comment and string split, the `-U0` diff and `--name-status -z` parsers, the `[leaks] allow` reader, and `scan(repo, Input::Base | Input::All) -> Report` with `fix_hits()`, `id_hits()`, `share()` and `render()`. `check()` is the pure half, which the unit tests drive without git. `src/cli/leaks.rs` and `Command::Leaks` take exactly one of `--base` and `--all`. `"leaks"` is in `PROJECT_TOP_KEYS`, and `regex = "1"` is a direct dependency. The git calls pin `--src-prefix=a/ --dst-prefix=b/`, `-M`, `--no-textconv` and `core.quotepath=off`, because this machine's git config uses mnemonic prefixes (`i/`, `w/`).
- **Tests.** 18 unit tests in `src/leaks.rs`: one per row with a hit and a non-hit, plus the split, the share, the report shape, the allowlist and both parsers. Also one `config.rs` test, and 3 integration tests in `tests/leaks.rs`: base against all, the flags, and the seeded branch.
- **Inner loop.** 2 runs, 0 red, about 3.5 minutes, 592 tests. The second run only re-checked a comment fix.
- **Dogfood.** The verb, run over this slice's own diff, found one leak outside test data: the `name_hit` doc comment listed id examples. It was rewritten. `--all` over factory's tree takes 0.8 s and gives 742 hits: 651 `id-string` (data, for slice 03's allowlist), 39 `id-name` (the PRD's predicted `ad1`/`AD1` locals plus `SLICE_01`/`slice_01` names), 16 `id-comment` (the known fixture narrations), 35 `line-cite` in recorded output, and 1 `section-cite`. Comment share is 2.7%.
- **The seeded branch** (task 5's fixture, `factory leaks --base main`, exit 1):
  ```
  id-comment: a plan id in a comment
    fix: Delete the id. Keep the fact as one short line only if the code cannot show it.
    src/guard.rs:1  // FR-3: guard the lane before the merge
    src/guard.rs:2  // until slice 4 this runs twice

  id-name: a plan id in a name or path
    fix: Rename after the behavior and update every reference.
    tests/guard.rs:2  fn fr_12_guards_the_lane() {}
    tests/fixture/prd-9/case.jsonl

  history: history in a comment
    fix: Delete it. A comment says what the code is; git log says what it was.
    src/guard.rs:2  // until slice 4 this runs twice

  comment share: 44.4% (4 of 9 added lines)
  5 fix hits
  ```
- **Deviations.** AD-1 covers what a `strings` entry turns off and what the share counts. AD-2 covers the extension table and the split rules. A line that hits two rules is listed under both.
- **Proved at checkpoint 1.** FR-3 (first half): task 5's test transcript `evidence/cp-1-fr3-seeded-test.txt` (1 passed); the real binary over a seeded branch, report and exit 1, then a why-and-quirk branch printing `clean` and exit 0, `evidence/cp-1-fr3-seeded-cli.txt`.

**Slice**: After this slice `factory leaks --base <ref>` reads the added lines and added paths of `<ref>...HEAD` outside `docs/`, and `factory leaks --all` reads the whole tree against the empty tree. Both run one embedded rule table over them, print the hits grouped by rule under each rule's fix line, print the comment share of the added lines, and exit 1 on any `fix` hit. An allowlist in `docs/project.toml` turns rules off per path. The scan is a library function that returns the hits and the figure, so the checkpoint (slice 04), the retro (slice 05) and the doctor line (slice 03) call it without shelling out. Tuning the detectors against cody-agent, arca-api and factory's own tree, and factory's own allowlist, are slice 03's.

**Highest risk**: task 1. The comment, string and name split per language decides every rule, and a wrong split floods the report with false hits or hides real ones.

## Current code / environment

- **The verb wiring**: `src/main.rs:19-69` is `enum Command`, each variant carrying its clap `///` help line (the one doc comment FR-1 keeps: a CLI contract). Dispatch is `:73-90`, e.g. `Command::Report(a) => cli::report::report(&a, &mut stdout())`. `src/cli/mod.rs` lists one module per subcommand, alphabetical; `src/lib.rs:2-32` lists the library modules, alphabetical. A verb takes a writer and returns an exit code (`src/cli/mod.rs:1`), so a test can drive it in-process.
- **The repo root**: `paths::repo_root(Path::new("."))`, as `src/cli/docs.rs:217` uses it. Running from inside another repo's checkout works with no flag, which slice 03's tuning relies on.
- **Git calls**: `ci::git(repo, &[...])` returns an output with `.ok()`, `.stdout`, `.text()` (used by `src/retro.rs:37-46` `changed_lines`, whose `{base}...HEAD` three-dot form this verb shares).
- **The project file**: `src/docs/project.rs:38-78` `load` reads only the `[docs]` table into `Project`; `:81-100` `seed` reads `[lanes] seed` separately from the raw table. Read the allowlist the same way `seed` does, in its own function, so `Project` and its `KEYS` (`:10-19`) stay as they are.
- **The trap, an unknown table warns**: `src/config.rs:126` `PROJECT_TOP_KEYS = ["docs", "lanes", "run", "workers"]`, and `:186` warns on any other top-level table in `docs/project.toml`. A `[leaks]` table warns on every `factory` call until `leaks` is in that list.
- **Regex**: `regex` 1.13.1 is in `Cargo.lock` already (through `grep-regex`, a direct dependency at `Cargo.toml:30`), so adding `regex = "1"` to `[dependencies]` downloads nothing new.
- **The harness**: `tests/harness/mod.rs` `Fixture::new(name)` builds a v3 repo on `main` (`git init -q -b main`), with `fx.git(&[...])`, `fx.path()`, `fx.command(bin)` and the env the stubs need. Every `tests/*.rs` file starts with `mod harness;`.
- **The figures the PRD baselines with**: FR-1's acceptance measured "12.0% of non-blank lines as comments" on PRD-005's added lines (`git diff 8458b30 611dcd1 -- src tests`). The share line this slice prints is the same measure: comment lines over non-blank added lines.
- **Deliberate non-changes**: *(prd §11 "Not handled: a `note` on a comment block of three or more lines." Every row at ship is `fix`. Do not add a `note` row.)* *(prd §11 "Not handled: a per-repo rule table (rules in `project.toml`)". `project.toml` holds the allowlist only, never a rule.)* *(prd §11 "Not handled: per-language comment linters". One verb, one table.)* *(prd §11 "Prose in `docs/`, READMEs, commit messages and PR bodies." Markdown and `docs/` are never read.)*

## Tasks

### 1. The rule table and the scanner — high — blocked by: —
**Where**: `src/leaks.rs` (new), `src/lib.rs` (`pub mod leaks`), `Cargo.toml` (`regex = "1"`).

**Source**: §5 FR-3: "A verb, `factory leaks [--base <ref> | --all]`, built as a rule runner. One embedded table of rules. Each rule is an id, a detector over added lines or paths, a severity and a one-line fix instruction. The report groups hits by rule and prints the rule's fix line above them … A new class of leak is one row in the table and nothing else changes." And: "An extension table tells comments from code: rs, go, php, js/ts/vue, py, sh, toml/yaml, json/jsonl. Markdown outside `docs/` is skipped, like READMEs." And: "A `fix` hit exits 1; a figure is never a hit."

**The table.** A `const` array of rules, one row each, in the PRD's order. Each row: an id, a title, where it looks (comment, string, name, path), a compiled detector, a severity (every row `fix`) and its fix line. The rows, with the PRD's detectors:

| id | looks in | detector (from §5's table) |
|---|---|---|
| `id-comment` | comments | `(PRD\|FR\|AD\|CP\|OPEN\|OQ\|DEC\|BLOCK\|ASK)-\d+`, `slice[- ]?\d+`, `§`; any case |
| `id-name` | identifiers, added paths | the same items, any case, joined by `-`, `_` or nothing, at an identifier or camelCase boundary; `§` reads as `section` |
| `id-string` | strings | the same items as `id-comment`, without a bare `§` |
| `history` | comments | "until slice", "for now", "previously", "no longer", "added in", "used to", "not built yet", "a later slice", "before PRD", "since slice", "as corrected", "confirmed on" |
| `line-cite` | comments, strings | `\w[\w.-]*\.(md\|mjs\|js\|ts\|rs\|go\|php\|py):\d+(-\d+)?` |
| `section-cite` | comments, strings | `/\w+ §`, `\w+\.md §`, `(PRD §`, `prd §` |
| `step-marker` | comments | `\b[Tt]asks? \d`, `/// Task \d+:`, a banner `── (task \d+\|PRD-\|FR-\|slice \d)` |
| `finding-id` | comments | `\b(C\|Cn\|K\|L\|Ln\|S\|X[a-d])\d{1,2}\b`, `finding [A-Z]+\d`; case-sensitive |
| `plan-words` | comments | "the spec says", "the PRD says", "qa-guide row", "slice-entry", "checkpoint's transcript" |

`L1` alone is a cache level (the table's own note, "tuned: `L1` is also a cache"), so it is not a `finding-id` hit. The comment share is not a row: it is a figure on the report.

**The fix lines** come from FR-5's ten-step order (§7: "a plan cite → drop it and keep the fact as one line when the code cannot show it; … history → delete; … a plan-named test → rename; a plan-citing string → rewrite after a grep of `tests/` and `src/`"). One line each, written for an agent reading the report cold, e.g. `id-comment`: "Delete the id. Keep the fact as one short line only if the code cannot show it." `id-name`: "Rename after the behavior and update every reference." `id-string`: "Rewrite the string without the id, after a grep of tests/ and src/ for every match on it."

**The split per language.** An extension table maps each file to its comment markers and string delimiters: `//`, `/* … */` and a leading `*` for rs, go, php, js, ts, vue; `#` too for php; `#` for py, sh, toml, yaml; none for json and jsonl (strings only). A line's comment part runs from the first marker outside a string literal to the end of the line; a line whose trimmed text starts with a marker is a comment line. A `-U0` diff gives lines without their neighbours, so a block comment's middle line counts by its leading `*` and nothing else. Every other extension is skipped by the line rules, Markdown included.

**What the scan returns.** `scan(repo, input) -> Result<Report>`, where the input is a base ref or the whole tree (task 2). `Report` holds every hit (rule id, path, line or none for a path hit, the line's text), the added comment lines and the added non-blank lines of the files the line rules read. It renders as: per rule with hits, a heading line with the rule's title, the fix line, then `path:line  text` per hit (`path` alone for a path hit); then `comment share: 12.0% (720 of 5976 added lines)`; then `N fix hits` or `clean`. Slices 03 to 05 need the `fix` hit count, the share, and the count of hits on `id-comment`, `id-name` and `id-string`: give `Report` those three.

**The allowlist** (task 3) is applied inside `scan`, so every caller gets the same answer.

**Acceptance:** unit tests in `src/leaks.rs`, one per row, each with one line that hits and one that does not. The hits are the PRD's own examples: `// FR-3: guard`, `prd_003`, `tests/fixture/prd-004/x.jsonl`, `slice2Allowlists`, `section_14`, `'FR-157: Support switches…'`, `// until slice 4`, `// docs.mjs:1930`, `"see autonomy.md § Caps"`, `// ── task 3 (a):`, `// C3: the race`, `// the spec says`. The non-hits are FR-5's "False positives the tuning keeps out" (§7): `the_prd_scaffold_frontmatter_has_the_seven_keys`, `version_3_store`, `"checkpoint"`, `"PRD-{feature}"`, `"run polish §5"` as a string, the `── polish ──` banner, `SKILL.md` alone in a string, "no longer" in a string, `L1` in a comment. A `.md` file and a file under `docs/` produce no hit. The share of a two-line diff with one comment line renders as `50.0% (1 of 2 added lines)`.

### 2. The input: `--base` and `--all` — high — blocked by: 1
**Where**: `src/leaks.rs`.

**Source**: §5 FR-3: "`--base <ref>` reads `git diff -U0 <base>...HEAD` outside `docs/`: the added lines and the added paths. `--all` reads the whole tree against the empty tree, for adoption and for the doctor line."

Lines: `git diff -U0 --no-color <base>...HEAD -- . ':(exclude)docs'`. Parse each `+++ b/<path>` header and each `@@ … +<start>[,<count>] @@` hunk header, and number every `+` line from `<start>`. Added paths: `git diff --name-status <base>...HEAD -- . ':(exclude)docs'`, the new path of every `A` or `R` row. The path rule reads every added path whatever its extension; the line rules read only the extension table's files. `--all` runs the same two commands with the empty tree as the base and `HEAD` as the tip (two dots, since the empty tree shares no history). Take the empty tree's id from `git hash-object -t tree /dev/null`: a SHA-256 repo has a different one. A failed git call is an `Err` naming the command and its output, as `changed_lines` does.

**The trap**: three dots, not two (`src/retro.rs:36`): `main..HEAD` would count commits the base gained since the branch left it. The PRD's own style note says the two read differently.

**Acceptance:** a unit or integration test on a temp repo: a commit on `main`, a branch with one added `.rs` line and one renamed file into `tests/fixture/prd-9/`. `--base main` reports the added line with its right line number and the renamed path, and nothing from `main`'s own commit. `--all` on the same branch reports `main`'s lines too.

### 3. The allowlist — high — blocked by: 1
**Where**: `src/leaks.rs`, `src/config.rs:126`.

**Source**: §5 FR-3: "An allowlist in `project.toml`, because factory uses ids as data. … A path allowlist. An entry carries a scope: `strings` (the default) turns off the string check under the path and nothing else; `all` turns off every rule there, comments, names and paths included."

The shape, read from `docs/project.toml` beside `[lanes] seed` (`src/docs/project.rs:81-100` is the pattern):

```toml
[leaks]
allow = [
  { path = "tests/lanes.rs" },
  { path = "tests/fixture/v2/", scope = "all" },
]
```

An entry matches the file it names, or every file under it when it names a directory (a prefix ending at a `/`). An unknown scope is an `Err` naming the entry. A repo with no `[leaks]` table has an empty list. Add `"leaks"` to `PROJECT_TOP_KEYS` (`src/config.rs:126`); see the trap above.

**Acceptance:** unit tests: a `strings` entry drops a string hit in its file and keeps a comment hit in the same file; an `all` entry on a directory drops a comment hit, a name hit and a path hit under it; a file next to the directory still hits. A `config.rs` test loads a `docs/project.toml` with a `[leaks]` table and gets no warning.

### 4. The verb — high — blocked by: 2, 3
**Where**: `src/cli/leaks.rs` (new), `src/cli/mod.rs`, `src/main.rs`.

**Source**: §5 FR-3: "A verb, `factory leaks [--base <ref> | --all]`" and "A `fix` hit exits 1; a figure is never a hit."

`Command::Leaks(cli::leaks::LeaksArgs)` with a one-line `///` help, e.g. "Find plan ids and history in added code and comments". `--base <ref>` and `--all` are one required group: exactly one of them. The verb prints `Report`'s rendering to its writer and returns 1 when the report has a `fix` hit, else 0. Errors return `Err`, which `main` prints as `factory: …` with exit 1.

**Acceptance:** covered by task 5's test, plus: no flag is a clap usage error (exit 2), and `--base main --all` is one too.

### 5. The seeded fixture branch — high — blocked by: 4
**Where**: `tests/leaks.rs` (new).

**Source**: §5 FR-3 acceptance: "On a fixture branch with seeded lines, `factory leaks --base main` lists the planted leaks under their rule, each under its fix line, and exits 1. The planted leaks are an id comment, an "until slice 4", a test named after an FR, and a fixture folder named `prd-9`. It leaves the real why, the foreign quirk and an allowlisted `BLOCK-1` alone, and prints the share." §9 seed state: "an id comment, a history note, a real why, a real foreign quirk, a test named after an FR, a fixture folder named `prd-9`, and an allowlisted `BLOCK-1` string."

`Fixture::new("leaks-seeded")`, then a branch off `main` that adds: a `.rs` file holding the id comment, the "until slice 4" note, a real why (an invariant no name shows) and a foreign quirk (a fact of a tool the repo cannot change); a test function named after an FR; a file under `tests/fixture/prd-9/`; a string `"BLOCK-1"` in a file the fixture's `docs/project.toml` allowlists as `strings`; and an id in `README.md` and in a `docs/` file. Commit, then run the binary (`env!("CARGO_BIN_EXE_factory")`) with `leaks --base main` in the fixture.

**Acceptance:** exit 1. Stdout holds the id comment under `id-comment`'s heading, the history note under `history`'s, the test name under `id-name`'s and the `prd-9` path under `id-name`'s, each heading followed by its fix line. The why, the quirk, `BLOCK-1`, `README.md` and the `docs/` line are absent. A `comment share:` line is present. A second branch with only the why and the quirk exits 0 and prints `clean`.

### 6. Verify — medium — blocked by: 1, 2, 3, 4, 5
`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 7. Prove — at the checkpoint — blocked by: 6
Rows from [`prd.md`](prd.md) §9 this slice proves, below the UI:

- **FR-3**, its first half: "run `factory leaks --base main` on the seeded fixture branch: it lists the id comment, the history note, the FR test name and the `prd-9` folder under their rules, each under its fix line, skips the why, the quirk and the allowlisted string, prints the share, exits 1" — task 5's test. The read-only `--all` over the three repos is slice 03's.

**Acceptance:** after the checkpoint, Progress holds task 5's test transcript and the report it printed.

**Code-review fixes (2026-10-14).**

- `factory leaks` exited 1 both on hits and when the scan could not run, so an agent read a bad ref as hits to fix. It now exits 2 with `factory leaks: did not run — …` on stderr (`src/cli/leaks.rs`); the polish skill already reads anything but 0 and 1 as not run.
- A `--base` starting with `-` reached `git diff` as an option (`--output=<file>` writes a file). `scan` refuses it before git runs; the verb test drives `--base=--output=leaked` and checks no file appears.
- A diff header for a path with a space ends in a tab, and a quoted path (`core.quotepath`, a `"` in the name) came back with its quotes, so its hits were filed under a name that matched no allowlist entry. `header_path` drops the tab and C-unquotes; a unit test and `odd_paths_and_the_local_git_config_hide_no_hit` (local `diff.noprefix`, `mnemonicPrefix`, `relative`, `color.ui=always`) cover it.
- A git failure printed git's whole stderr into a one-line report; it keeps the first line. A hit on a minified line printed the whole line; `cut` keeps 160 characters.
- The `Syntax.hash` doc said "after a space" where the code reads any whitespace.
