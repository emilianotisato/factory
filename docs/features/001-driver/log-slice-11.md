## Traps

- The cargo target dir here is `$CARGO_TARGET_DIR` (`/mnt/devcache/emiliano/cargo-target`), not `./target`: `target/debug/factory` does not exist. Run the built binary from `$CARGO_TARGET_DIR/debug/factory`, or `cargo run -q -- …`; tests use `env!("CARGO_BIN_EXE_factory")`.

## Blockers

## Decisions

### AD-1101 — Where does a heuristic `def` body end? · slice-11 · 2026-10-10

**Chose:** the body goes on while a bracket the definition opened is still open, or the next non-blank line is indented deeper than the match, or that line opens with `{`; it stops at the first line whose indentation returns to the match's, keeping the closing brace, and at 25 lines (`def::body_end`). Trailing blank lines are dropped; a body cut at 25 lines is counted in the trailer (`1 body cut at 25 lines`).
**Over:** the slice's rule read literally ("up to the first line whose indentation returns to the match's"): on the PHP fixture `final class UserService` is followed by `{` at the same indentation, so the body would be empty or the lone `{`, and a PSR-12 signature spread over lines (`): User` then `{`) would stop at the `)`.
**Because:** task 2 gives the rule in one line and its own fixture breaks it; bracket depth plus indentation keeps the rule's intent on all four stacks. Slice 12 replaces the body with the tree-sitter node range.
**Undo:** one function in `src/ctx/def.rs` and the def snapshots.
**Review:** fyi

### AD-1102 — What does a `refs` hit print, and what does the trailer say when nothing was cut? · slice-11 · 2026-10-10

**Chose:** `path:line  ref  <hit line trimmed>`, then the next line indented four spaces (left out when it is blank or the file ends). Trailers: `N more hits in M files (--max, --per-file to widen)` when the caps cut (M = files with a hit not shown), `N hits in M files (searched K files)` when they did not, `no references to NAME found (searched K files)` on none; then `; N in test files (--tests)` when test files held hits, and `; skipped …` for binary or over-2 MB files. `def` ends `N definitions (searched K files)` or the slice's `no definition of NAME found (searched N files)`.
**Over:** a bare `path:line  text` line for refs (the printer task spells `path:line  kind  signature` for both verbs and a ref has no kind in this slice), and a trailer only when something was cut.
**Because:** the slice spells the cut trailer and the empty `def` line but not the others; a model reading a trailerless list cannot tell whether it is complete, and test files are cut silently otherwise.
**Undo:** `src/ctx/print.rs` and the snapshots.
**Review:** no

### AD-1103 — What does `--vendor` lift, and what counts as binary? · slice-11 · 2026-10-10

**Chose:** `--vendor` (on `def` and `refs`) lifts both the vendor directories and every gitignore rule (`standard_filters(false)`, hidden files still skipped). Binary = a NUL byte anywhere in the file, read whole since files over 2 MB are already skipped at the walk.
**Over:** `--vendor` lifting only the six directory overrides: `vendor/` and `node_modules/` are gitignored in every Laravel and Node checkout, so the flag would do nothing there.
**Because:** §14 "Vendor and gitignored paths are skipped unless `--vendor`"; the slice does not say how binary is detected.
**Undo:** `walk::walk` and `walk::scan`.
**Review:** no

### AD-1104 — How many `planted` hits does the `refs/` fixture hold? · slice-11 · 2026-10-10

**Chose:** 300 textual hits across 40 files, two of them the comment and the string in `refs/caller01.ts` (298 real), plus one in `refs/tests/planted.test.ts`.
**Over:** 300 real hits plus the comment and string (302 textual), which the Current code section's "300 planted hits … plus one hit inside a comment and one inside a string" reads as.
**Because:** task 3's acceptance says `290 more hits` with the comment hit still printed, and slice 12 task 3 expects "the count of real hits is `300 - 2`"; only 300 textual satisfies both.
**Undo:** regenerate `tests/fixture/ctx/refs/` and re-bless the refs snapshots.
**Review:** no

### AD-1105 — Does the `outline` stub exit 0 or 2? · slice-11 · 2026-10-10

**Chose:** exit 2 with `outline needs tree-sitter (slice 12)` on stderr, as task 5 and its acceptance say.
**Over:** exit 0, which the slice's summary paragraph says ("a stub that says so and exits 0").
**Because:** task 5 states the reason (no caller mistakes silence for an empty outline) and its acceptance is the check.
**Undo:** one line in `ctx::run`.
**Review:** no

### AD-1106 — What is a test path matched against? · slice-11 · 2026-10-10

**Chose:** the path relative to the search root (`--in`, else the current directory), with the slice's pattern `tests?/|spec|_test\.go|\.test\.|\.spec\.` written as plain substring checks (`refs::is_test`).
**Over:** the absolute path, which hides every file of a checkout that lives under a `tests/` directory (this fixture does), and the `regex` crate for one pattern, a new runtime dependency.
**Because:** the slice gives the pattern, not what it is matched against. A consequence: `refs X --in tests` searches the test tree without `--tests`.
**Undo:** one function in `src/ctx/refs.rs`.
**Review:** no

### AD-1107 — How is the printed kind found, and what is it for TypeScript's class-member regex? · slice-11 · 2026-10-10

**Chose:** the last keyword of the language's table that precedes the symbol on the matched line (`def::kind_of`); a TypeScript class member, whose regex has no keyword, prints `method`.
**Over:** a capture group per regex, which needs the `grep-matcher` trait as a direct dependency the slice does not name.
**Because:** the slice says "the kind printed here is the keyword matched" and gives the member regex without one.
**Undo:** one function in `src/ctx/def.rs`.
**Review:** no

### AD-1108 — How are the outputs pinned? · slice-11 · 2026-10-10

**Chose:** a plain string compare against `tests/snapshots/ctx/*.txt` (twelve commands, each run twice and compared byte for byte); `FACTORY_BLESS=1 cargo test --test ctx` rewrites them. The slice 12 states are `#[ignore]`d tests beside the slice 11 ones.
**Over:** the `insta` crate, which the slice offers as the other option.
**Because:** twelve files and one loop need no dev dependency; slice 12 re-blesses with the same variable.
**Undo:** `tests/ctx.rs` and the snapshot folder.
**Review:** no
