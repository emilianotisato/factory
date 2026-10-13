# Slice 11 — ctx search: grep crates, definition regexes, printer (FR-12 slice 1)

**Source**: [`prd.md`](prd.md) → §14 FR-12 decision 1 (three verbs, plain text, exit 0 on empty, caps and context lines), decision 2 (the engine: ripgrep crates; the tree-sitter half is slice 12), decision 5 ("Three slices … (1) grep-crate walker, per-language definition regexes, compact capped printer"), and its acceptance. **Depends on**: 01 only (the crate). Independent of 02 to 10: §14 "Slices 1 and 2 touch no driver code and can run beside FR-1/FR-2."
**Created**: 2026-10-09 · **Status**: pending

**Progress** (2026-10-10, lane 1):
- **Built.** `factory ctx def|refs|outline` (`src/ctx/`, wired from `src/main.rs`; `CtxVerb` there is gone). `walk.rs`: the `ignore` parallel walker with the six vendor directories skipped at any depth unless `--vendor`, which also lifts gitignore (AD-1103); `Lang::from_path`; files over 2 MB counted at the walk, binary files (a NUL byte) counted when read; results sorted by path, and `scan` searches the files on all cores and hands results back in path order. `def.rs`: the slice's regexes, `NAME` escaped, one `grep-regex` matcher per table (Vue uses TypeScript's) run with `grep-searcher`; kind = the keyword before the name, `method` for a TypeScript member (AD-1107); heuristic body (AD-1101). `refs.rs`: `\bNAME\b`, first `--per-file` per file, first `--max` overall, test paths left out and counted unless `--tests` (AD-1106). `print.rs`: `path:line  kind  signature`, context indented four spaces, blank line between hits, trailer last (AD-1102). `outline` prints `outline needs tree-sitter (slice 12)` and exits 2 (AD-1105). Exit 2 on a bad flag (clap) or an unreadable root. `--kind class|fn|method|type` is validated and ignored, as `--help` says.
- **Crates.** `grep-searcher` 0.1.17, `grep-regex` 0.1.14, `ignore` 0.4.33, as §14 names; nothing else added.
- **Fixtures.** `tests/fixture/ctx/`: `php/` (`UserService` with a starless docblock line `class OtherService, which this one wraps.`, `OtherService`, trait `HasTimestamps`, interface `Notifier`), `vue/UserCard.vue`, `go/client.go`, `rs/config.rs` (`make_fn!(made_by_macro)`), `refs/` (300 textual hits of `planted` in 40 files, two in a comment and a string; one more in `refs/tests/`, AD-1104). 48 source files.
- **Tests.** `tests/ctx.rs`: the twelve snapshots in `tests/snapshots/ctx/` (each command run twice, byte-identical; `FACTORY_BLESS=1` rewrites, AD-1108), the task 2 and 3 acceptance lines, the outline stub, exit 2 cases, and the walker on a copy of the fixture with planted `vendor/`, `node_modules/`, `php/build/`, a gitignored file, a binary file and a 2 MB file. Two `#[ignore]`d tests hold the slice 12 states (docblock mention dropped; comment and string hits dropped with `2 dropped (comments/strings)`). Unit tests in `walk.rs`, `def.rs`, `refs.rs`.
- **Inner loop:** 3 runs of `cargo test`, 1 red, about 1 min. The red run was `--test ctx` before the snapshots existed (by design: blessed, then read line by line). The full `cargo test --no-fail-fast` was green on its first run (38 s). `cargo clippy --all-targets -- -D warnings` caught one `double_ended_iterator_last` in `def::kind_of`; `cargo fmt` reflowed the new files only.
- **Earlier-slice expectation shifts:** `tests/cli.rs::every_subcommand_says_not_built_yet_and_exits_2` (`ctx def`, `refs`, `outline` leave the not-built list; `hook` stays).
- **Deviations:** AD-1101 (body rule made bracket-aware). AD-1104 and AD-1105 settle two places where the slice contradicts itself.
- **Prove (task 7), CP-6 2026-10-10:** passed. Pinned output: `cargo test --test ctx -- --nocapture` green, 17 tests (the 20 snapshots each run twice, byte-identical, and the acceptance assertions), and the same commands driven by hand on the fixture, 34 of 34 comparisons byte-identical. Timing: release build (`cargo build --release`), one warm-up then three runs each under `time`, in the owner's checkouts, which sit one level below the paths this slice names: icg-cloud at `~/Projects/ICG/icg-cloud` (`feat/arca-setup`), arca-api at `~/Projects/Prokey/arca-api` (Go, `make verify`); only `factory ctx` and read-only commands ran there. Every run is under 1 s warm, the slowest 0.060 s. Slice 12's `outline` rows are here too: icg-cloud has no `app/Services`, so the densest PHP directory stands in for it, and arca-api's `internal/` holds only packages (non-recursive, AD-1207), so `internal/caea` stands in for it; both suggested paths were timed as given. Evidence: [`evidence/cp-6-fr12-ctx-tests.txt`](evidence/cp-6-fr12-ctx-tests.txt), [`evidence/cp-6-fr12-pinned-output.txt`](evidence/cp-6-fr12-pinned-output.txt), [`evidence/cp-6-fr12-timing.txt`](evidence/cp-6-fr12-timing.txt), [`evidence/cp-6-gate.txt`](evidence/cp-6-gate.txt).

  | verb | repo | command | runs (s) | median (s) |
  |---|---|---|---|---|
  | def | icg-cloud | `factory ctx def Company` | 0.052, 0.033, 0.050 | 0.050 |
  | refs | icg-cloud | `factory ctx refs Company` | 0.044, 0.041, 0.038 | 0.041 |
  | outline | icg-cloud | `factory ctx outline app/Core/Http/Controllers/Admin` (29 files) | 0.046, 0.045, 0.046 | 0.046 |
  | outline | icg-cloud | `factory ctx outline app/Services` (absent: exit 2) | 0.002, 0.002, 0.002 | 0.002 |
  | def | arca-api | `factory ctx def Code` | 0.017, 0.030, 0.033 | 0.030 |
  | refs | arca-api | `factory ctx refs Code` | 0.046, 0.044, 0.035 | 0.044 |
  | outline | arca-api | `factory ctx outline internal/caea` (116 files) | 0.060, 0.052, 0.047 | 0.052 |
  | outline | arca-api | `factory ctx outline internal/` (no files in it) | 0.002, 0.002, 0.002 | 0.002 |

**Code-review fixes (2026-10-10).**
- `def`'s regex prefilter missed shapes that `outline` lists (TypeScript `export default function`, getters, generic members and multi-line parameters; PHP modifiers in any order; Rust `const fn` and `extern "C" fn`; Go's grouped `type ( … )`), so `def` said not found and the pack's recon dropped them silently (C1). Each language's patterns are widened, the tree still confirms every hit, and `def_finds_every_shape_outline_lists` pins each shape against `outline` (AD-40001).
- `refs::is_test` matched substrings, so `Inspector.php`, `respect.rs`, `latest/` and `contest/` were hidden as tests (C6). A test path is now a `test`, `tests`, `spec`, `specs` or `__tests__` segment, or a file name ending `_test.go` or holding `.test.` or `.spec.`, which refines AD-1106 (AD-40003).
- `build/`, `dist/` and `storage/` were skipped at any depth, which hid source such as `src/build/` or `app/storage/` (Cn5). They are now skipped only directly under the search root, while `vendor/`, `node_modules/` and `target/` stay skipped at any depth; this slice's walk test now plants `php/build/` as walked, not skipped (AD-40004, `decide`).
- `refs`' `\b` boundary cannot match before a `$`, so a `$`-led name such as `$user` was never found (Cn6). It is found now.

**Slice**: After this slice `factory ctx def`, `refs` and `outline` exist with the textual engine: an `ignore`-based walker that skips vendor and gitignored paths, a per-language definition regex for PHP, TypeScript, Go and Rust run through the grep crates, a word-boundary reference search collapsed per file with caps and a trailer, and the compact printer with its two context lines. `def` prints the regex match with context; the exact body and the comment/string filtering arrive with tree-sitter in slice 12, so `--no-body` exists but the body printed here is the context window. `outline` is a stub that says so and exits 0. Fixture files per stack pin the output.

## Current code / environment

- **The numbers behind the shape** (§14, `ctx-research.md` §2): 86.9% of definition greps asked for the body, median 25 lines; 10 was the commonest cap on caller lists; bare locations tripled follow-up reads (15.2 vs 3.2 per task); capped, summarised search beat iterative search by 6 points. So: `def` with body by default, `refs --max 10 --per-file 3`, two context lines, a trailer.
- **Crates** (cargo search 2026-10-09): `grep-searcher` 0.1.17, `grep-regex` 0.1.14, `ignore` 0.4.33 (the ripgrep crates, MIT/Unlicense). `ignore::WalkBuilder` honours `.gitignore` and hidden-file rules; add overrides for `vendor/`, `node_modules/`, `target/`, `dist/`, `build/`, `storage/` (Laravel) unless `--vendor`. Multi-threaded walk via `build_parallel()`; the printer collects then sorts by path and line so output is deterministic.
- **Definition regexes** (one per language, anchored at line start after whitespace and modifiers):
  PHP `(abstract\s+|final\s+)?(class|interface|trait|enum)\s+NAME\b` and `(public|protected|private|static|\s)*function\s+&?NAME\s*\(`;
  TypeScript/JavaScript `(export\s+)?(default\s+)?(abstract\s+)?(class|interface|type|enum)\s+NAME\b`, `(export\s+)?(async\s+)?function\s*\*?\s*NAME\s*[(<]`, `(export\s+)?(const|let|var)\s+NAME\s*(:[^=]+)?=\s*(async\s*)?(\(|[A-Za-z_$][\w$]*\s*=>)`, and class members `(public|private|protected|static|readonly|async|\s)*NAME\s*\([^)]*\)\s*(:[^{]+)?\{`;
  Go `func\s+(\([^)]*\)\s*)?NAME\s*[(\[]` and `type\s+NAME\b`;
  Rust `(pub(\([^)]*\))?\s+)?(async\s+)?(unsafe\s+)?(fn|struct|enum|trait|type|mod|const|static)\s+NAME\b` and `macro_rules!\s+NAME`.
  Language by extension: `.php`, `.ts .tsx .js .jsx .mjs .vue`, `.go`, `.rs`. The kind printed here is the keyword matched (`class`, `fn`, `function`, `type`, …); slice 12 maps it to the `--kind` vocabulary `class|fn|method|type`.
- **Fixtures** (`tests/fixture/ctx/`): `php/` a class with a docblock that mentions another class's name (the "docblock mention" the acceptance drops: in this slice it is still printed, in slice 12 it goes; the test pins both states by marking the slice 12 assertion `#[ignore]` until then), a trait, an interface, a method; `vue/` a single-file component with `<script setup lang="ts">` defining a function and a `const` arrow function; `go/` a struct, a method with a receiver, a function; `rs/` a struct, an impl with two methods, a trait, a macro-made function (the accepted textual miss, §14); `refs/` a file with a symbol that has 300 planted hits across 40 files plus one hit inside a comment and one inside a string. The acceptance in §14 names "300 planted hits".
- **Timing targets**: §14 acceptance "each verb answers in under 1 s warm" on the icg-cloud and arca-api checkouts (paths in the owner's `~/Projects`; the test cannot assume them, so the timing is a Prove step run by the owner's checkpoint, written as a table in Progress). Measured elsewhere: 7 to 44 ms for rg-style search on 59k to 3.45M lines.
- **Deliberate non-changes**: *(prd §14 "Rejected: an LSP proxy."; no language server, no daemon, no index on disk.)* *(prd §14 "Rejected: in-process ast-grep as the search engine".)* *(prd §14 "Accepted cost: callers through Laravel facades, Eloquent magic, scopes and route strings stay textual"; do not add facade or route resolution.)* *(prd §19 "Not handled: a tags cache"; every call re-reads the tree.)* *(prd §19 "Not handled: a sha256 seen-before stub"; cut 2026-10-09.)*

## Tasks

### 1. Walker and language table — high — blocked by: —
**Source**: §14 FR-12: "Vendor and gitignored paths are skipped unless `--vendor`." "Grammars: PHP, TypeScript, Go, Rust … Vue".

`src/ctx/walk.rs`: `walk(root, opts) -> impl Iterator<Item = (PathBuf, Lang)>` over `ignore::WalkBuilder` with the vendor overrides and `--in <dir>` as the root; `Lang::from_path`. Binary and files over 2 MB skipped with a count in the trailer. `src/ctx/mod.rs` wires `factory ctx def|refs|outline` into clap with the flags from §14 (`def --kind --no-body`, `refs --in --max --per-file --tests`, `outline --depth`); `--kind` is accepted and ignored until slice 12 (a note in `--help`).

**Acceptance:** on the fixture, `walk` yields every source file under `php/ vue/ go/ rs/ refs/` and nothing under a planted `vendor/` and `node_modules/`; with `--vendor` the planted files appear; a `.gitignore`d file is skipped.

### 2. `def`: definition regexes through the grep crates — high — blocked by: 1
**Source**: §14 FR-12: "`def` runs a per-language definition regex through `grep-searcher`, `grep-regex` and `ignore`"; "`def` prints the body by default".

`src/ctx/def.rs`: for each language build the regexes above with `NAME` escaped, run `grep_searcher::Searcher` with `grep_regex::RegexMatcher` over the walker's files of that language; collect `Hit{path, line, kind, signature(line text trimmed)}`. Body in this slice = the match line plus the following lines up to the first line whose indentation returns to the match's or 25 lines, whichever first (the median body from the research; slice 12 replaces it with the node range). `--no-body` prints the location and signature only. No hit → exit 0 with `no definition of NAME found (searched N files)`.

**Acceptance:** `def UserService` on the PHP fixture prints `php/UserService.php:LINE class UserService` and the body; `def handle` prints the method with kind `function`; `def NewClient` on Go prints the func with its receiver-less signature; `def Config` on Rust prints the struct; `def made_by_macro` prints nothing and exits 0 with the trailer; `def useThing` finds the Vue `const` arrow function.

### 3. `refs`: word-boundary search, per-file collapse, caps — high — blocked by: 1
**Source**: §14 FR-12: "`refs` is a word-boundary search with comments and strings dropped, collapsed per file." "`refs` stops at 10". Flags `--in <dir> --max 10 --per-file 3 --tests`. Acceptance: "`refs` on a symbol with 300 planted hits prints at most `--max` lines plus a trailer".

`src/ctx/refs.rs`: `\bNAME\b` over the walker; group by file, keep the first `--per-file` lines per file, then the first `--max` lines overall, sorted by path then line; `--tests` includes paths matching `tests?/|spec|_test\.go|\.test\.|\.spec\.` which are otherwise excluded (test files are the noisiest callers in the transcripts); the trailer says `N more hits in M files (--max, --per-file to widen)`. Comment and string dropping is slice 12's (tree-sitter); here a comment hit is still printed and the test pins that with an `#[ignore]`d assertion for slice 12 to un-ignore.

**Acceptance:** `refs planted` on `refs/` prints exactly 10 hit lines and a trailer naming `290 more hits`; `--max 20 --per-file 1` prints 20 lines from 20 files; `--in refs/sub` restricts to that dir; a hit inside a test file appears only with `--tests`.

### 4. Printer with context lines and trailer — high — blocked by: 2, 3
**Source**: §14 FR-12: "Output is path:line, kind, signature, one or two context lines, then a trailer saying what was cut." "Why context lines: bare locations tripled follow-up reads".

`src/ctx/print.rs`: `path:line  kind  signature` then two context lines (the line after the hit for `refs`; the body for `def`) indented by four spaces, hits separated by a blank line; relative paths from the search root; the trailer on the last line; no colour, no box drawing; output width uncapped (the model reads it, not a terminal). Exit 0 always on a search that ran; exit 2 on a bad flag or an unreadable root.

**Acceptance:** snapshot tests (`insta` crate or a plain string compare) pin the full output of the task 2 and task 3 commands; output is byte-identical across two runs (the parallel walker's order is sorted).

### 5. `outline` stub — low — blocked by: 1
**Source**: §14 FR-12: "`factory ctx outline <path>... [--depth 1]`"; slice 12 implements it.

`factory ctx outline <path>` prints `outline needs tree-sitter (slice 12)` to stderr and exits 2 so no caller mistakes silence for an empty outline.

**Acceptance:** `outline php/UserService.php` exits 2 with that line.

### 6. Verify — medium — blocked by: 1–5
`cargo test`. The full gate runs at this slice's checkpoint.

**Acceptance:** inner loop green, one commit.

### 7. Prove — at the checkpoint — blocked by: 6
Rows from [`prd.md`](prd.md) § QA plan: the first part of **FR-12** ("fixture files per stack → each verb's output pinned; `def`, `refs`, `outline` under 1 s warm on icg-cloud and arca-api", evidence: test + timing table in slice Progress). At the checkpoint: the snapshot tests, and a timing table (`time factory ctx def <symbol>`, `time factory ctx refs <symbol>` on `~/Projects/icg-cloud` and `~/Projects/arca-api`, warm, three runs each) pasted into this slice's **Progress**. `outline` timing waits for slice 12. The `ctx_share`/`repeat_rate` part of the row is slice 13's.

**Acceptance:** the timing table shows every `def` and `refs` run under 1 s warm on both checkouts.
