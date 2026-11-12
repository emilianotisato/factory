# Slice 01 — Honest passes: timeouts and skipped files

**Source**: [`prd.md`](prd.md) → §4 FR-2, §5 FR-3, §2, §13. **Depends on**: — (the first slice. It fixes the two field bugs and touches only the pass accounting and the file list.)
**Created**: 2026-11-12

**Progress**:
- **Built:** `body()` no longer counts a killed pass with partial text (unit case flipped). `review::skip` splits numstat rows into kept and skipped (`tool-written` by path shape, `vendored`/`generated` via `git check-attr -z --stdin`, stdin written on a thread). A check-attr failure skips nothing for that reason and says so on the `skipped:` line. `collect()` and `second()` both call it before sizing and grouping. Collect's plan prints the `skipped:` block before `size:`, and with nothing kept the opinion reads `skipped by size: no file left after skipping …`. `second()` prints the block before the first pass, or `unreviewed: no changes against <base> left after skipping …` with exit 2 (AD-2). Stub `sleep_after_assistant` takes `{"<pass>": n}`. `branch()` creates parent directories, and `branch_off` commits files on `main` first.
- **Tests:** `a_timeout_is_unreviewed_and_keeps_its_partial_text` (replaces `a_timeout_keeps_the_partial_text`), `a_timeout_beside_a_counted_pass_exits_0`, `collect_owes_the_fallback_when_every_pass_timed_out`, `collect_skips_tool_written_and_vendored_files_and_lists_them`, `second_skips_the_same_files_and_lists_them_before_the_passes`, plus unit tests for the path shape, the check-attr parse and the `skipped:` block.
- **Inner loop:** 2 runs, 1 red, about 3 min. The red run was clippy's `chunks_exact_to_as_chunks` on the check-attr parser.
- **Prove:** FR-2 and FR-3 rows are left for the checkpoint.

**Slice**: After this slice, a second-opinion pass killed at its timeout never counts. Its partial text is kept, but it is unreviewed, so the fallback fires for it. `review collect` and `review second --diff` drop tool-written feature files (`docs/features/*/log.jsonl`, `docs/features/*/state.toml`) and paths `.gitattributes` marks `linguist-vendored` or `linguist-generated` before grouping. The plan lists them under a `skipped:` line. Hunk splitting is slice 02's, and the settled block and `--since` are slice 03's.

## Current code / environment

- **The counted timeout**: `body()` (`src/review.rs:540-558`). The `Ended::Killed` arm returns `(… "(timed out after {timeout_secs} s, partial)" …, true)` when there is partial text. Its doc comment (`:540`) says "A kill's partial counts (it had the whole timeout)", and that comment goes with the change.
- **Unit test pinning it**: `each_ending_has_its_marker_and_only_text_that_finished_counts` (`src/review.rs:641`). Its `Ended::Killed, vec![OPENING]` case expects `true`, and flips to `false` with the same text.
- **Integration test pinning it**: `a_timeout_keeps_the_partial_text` (`tests/review.rs:244-258`) asserts exit `0`. prd §13 names its replacement `a_timeout_is_unreviewed_and_keeps_its_partial_text`, with exit `2` and the same partial-text assertions.
- **`review second`'s exit**: `Ok(if reviewed > 0 { 0 } else { 2 })` (`src/cli/review.rs:154`). Pass headings there carry no counted mark (`:150`), and prd §4 keeps that.
- **Collect's marks**: `collect.rs:187-201` writes ` — counted` / ` — unreviewed` from `pass_text`'s bool, and `fallback owed` when `counted == 0`. Nothing to change there beyond what `body()` returns.
- **The stub can't time out one pass alone**: `sleep_after_assistant` (`tests/stubs/cursor-agent:10-12`, `:75`) applies to every review call. `sleep_secs` is per pass (`:17-19`, `:103`), but a pass sleeping before any text already dies unreviewed today. Add a per-pass form, `sleep_after_assistant: {"<pass>": n}`, mirroring how `sleep_secs` reads an object (`:103`), and document it in the header comment.
- **The file list**: `collect()` calls `review::numstat(root, base, &[])` (`src/collect.rs:70`). `second()` calls `review::numstat(&cwd, base, &a.paths)` (`src/cli/review.rs:112`). Both go straight to `review::group`. Put the skip in one `review` function both call, so the two verbs cannot drift.
- **The size row**: `needs_second_opinion(&files)` (`src/collect.rs:53-56`, called at `:73`) counts files and lines. prd §5 (corrected 2026-11-12): it counts only the files left after the skip. With none left, the state is `skipped by size`, and the text names the skipped files instead of "n files under 50 changed lines" (`:83-89`).
- **`check-attr`**: plain `git check-attr` reads the working tree's `.gitattributes`. prd §5 chose the checkout, not `--source=HEAD` (needs git 2.40+). Use `git check-attr -z --stdin linguist-vendored linguist-generated` with the paths on stdin, so a long file list never hits the argv limit. `-z` output is `path NUL attr NUL value NUL` triples, and the value is `set`, `unset`, `unspecified` or a string. `set` and `true` mean skip.
- **Fixture limits**: `branch()` (`tests/review.rs:26-43`) writes `fx.path().join(f)` with no parent-directory creation, and commits only on `feat/review`. A `docs/features/001-x/log.jsonl` path needs `create_dir_all`. The `.gitattributes` must be committed on `main` before the branch, or it becomes a changed file itself (prd §13 seed state).
- **Consumers of the collected file**: `collect::passes_and_log` and `reusable` (`src/collect.rs:30-50`) and `pack::collected` (`src/pack.rs:144`) key on the ` — counted` / ` — unreviewed` suffix and the `second opinion:` line. A new `skipped:` plan line touches none of them. Keep it inside `## Plan`, before `size:`.
- **Deliberate non-change**: prd §15 "Not handled: a timeout that scales with pass size" and "Not handled: a `[review] exclude` list in `docs/project.toml`". The leaks scan (`collect.rs:72`, `leaks_part`) is unchanged and still reads every added line (prd §5).

## Tasks

### 1. A killed pass is unreviewed — high — blocked by: —
**Source**: §4 FR-2: "A pass that ended `Killed` never counts, with or without text. Its partial text stays in its section under `(timed out after N s, partial)`."

`src/review.rs` `body()`: the `Ended::Killed` arm with partial text returns the same text and `false`. Rewrite the doc comment to say why: a kill means the pass did not finish its files. Flip the unit test's case. In `tests/review.rs`, replace `a_timeout_keeps_the_partial_text` with `a_timeout_is_unreviewed_and_keeps_its_partial_text`: same scenario, exit `2`, partial text still present.

**Acceptance:** `a_timeout_is_unreviewed_and_keeps_its_partial_text` asserts exit `2` and the partial text. The unit test's Killed-with-text case asserts `false`.

### 2. One timeout beside a counted pass — high — blocked by: 1
**Source**: §4 FR-2 acceptance: "With a second pass that finishes, it exits `0`… In collect, the timed-out pass's heading ends ` — unreviewed`, and every pass timing out writes `fallback owed`."

`tests/stubs/cursor-agent`: accept `sleep_after_assistant` as `{"<pass>": n}` too. Then add:

- `a_timeout_beside_a_counted_pass_exits_0`: `review second` over two passes, pass 2 timing out after text, exits `0`.
- `collect_owes_the_fallback_when_every_pass_timed_out`: in collect, the timed-out pass's heading ends ` — unreviewed`. With every pass timing out after text, the plan reads `fallback owed`.

**Acceptance:** both tests pass. `a_timeout_beside_a_counted_pass_exits_0` also passes on today's code, since it pins that one timeout does not sink the run. The collect test is the one that fails today, because the timed-out pass's heading reads ` — counted`.

### 3. Skip tool-written and vendored files — high — blocked by: —
**Source**: §5 FR-3: "Before grouping, both `review collect` and `review second --diff` drop: tool-written files… vendored or generated files… The plan always lists skipped files under a `skipped:` line with the reason."

In `src/review.rs`, add a function that splits numstat rows into kept and skipped, each skipped row with its reason (`tool-written`, `vendored` or `generated`):

- **Tool-written:** a path matching `docs/features/<dir>/log.jsonl` or `docs/features/<dir>/state.toml`, one directory level, no glob crate needed.
- **Vendored or generated:** through `check-attr` as above, run in `root`.

Then wire it into both verbs:

- **`collect()`** filters before `needs_second_opinion`, `group` and the plan's file list. The plan prints `skipped:` and one `- path (reason)` per file, only when any were skipped. With nothing kept, the state is `skipped by size`, with text naming the skipped files.
- **`second()`** (`src/cli/review.rs:112`) filters the same way and prints the same `skipped:` lines before the passes. With nothing kept, it keeps today's no-changes path (`:113-116`) and names the skipped files.

A `check-attr` failure is not fatal. Skip nothing for that reason and say so on the `skipped:` line: a defensive default, not a new mechanism.

**Acceptance:** `collect_skips_tool_written_and_vendored_files_and_lists_them`. A branch changes `docs/features/001-x/log.jsonl`, `docs/features/001-x/state.toml`, `testdata/a.xsd` (`.gitattributes` on `main`: `testdata/** linguist-vendored`) and `src/a.rs`. Only `src/a.rs` reaches the stub (`review_calls`, the `review-n.diff` contents), and the plan's `skipped:` names the other three with reasons. A second case changes only the two tool files, and the plan reads `skipped by size` with no reviewer call.

### 4. Verify — medium — blocked by: 1, 2, 3
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q`

**Acceptance:** inner loop green, one commit.

### 5. Prove — at the checkpoint — blocked by: 4
prd §13 rows **FR-2** (`a_timeout_is_unreviewed_and_keeps_its_partial_text`, `a_timeout_beside_a_counted_pass_exits_0`, `collect_owes_the_fallback_when_every_pass_timed_out`) and **FR-3** (`collect_skips_tool_written_and_vendored_files_and_lists_them`). They are proven below the UI: the checkpoint runs them and keeps the transcript.

**Acceptance:** after the checkpoint, the transcript of each named test is in Progress.
