# Slice 02 — Split a huge file by hunks and lines

**Source**: [`prd.md`](prd.md) → §6 FR-4, §13. **Depends on**: 01 (both change what reaches `review::group` and the `second()` flow in `src/cli/review.rs`. Land the skip first, so this diff is only about splitting.)
**Created**: 2026-11-12

**Highest risk in the feature**: diff parsing. A wrong `@@` header or a dropped line means a reviewer reads a diff that does not match the code. Pin it with unit tests on real `git diff` output before wiring it in.

**Progress**:

- **Built.** `review::split_diff` cuts one file's diff into parts. Each part is the file header plus whole hunks, greedily packed up to the cap. A hunk over the cap is first cut by `cut_hunk` into ceil(n / cap) even chunks with recomputed `@@` headers. An empty side names the line before it, as git does (`-0,0`). A `\ No newline` line stays with its line. `review::plan` is `group`, then each `over` pass above 2 × cap is replaced by its parts. `collect` and `second()` both call it, and the diff fetch stays in `review::diff`. `Pass` gained `part: Option<Part>` (`k`, `n`, `diff`) and derives `Default`. `diff_pass_prompt` writes `part.diff` when present. The heading and the stderr progress line read `path (part k of n)`, and the prompt opens "Part k of n of the diff of …" (AD-3). Even chunks, and a failed diff read leaving the file whole: AD-4.
- **Tests.** Unit tests `a_huge_hunk_splits_with_headers_git_can_apply` cover three single-hunk shapes: interleaved context, a last line with no newline, and pure additions mid-file. Every part passes `git apply --check --unidiff-zero` and then applies in order, and the result equals the new file. `--unidiff-zero` is needed because a chunk cut mid-change has no trailing context, so plain `git apply` would pin it to the end of the file. The other unit tests are `whole_hunks_pack_up_to_the_cap` and `the_file_header_heads_every_part`. Integration tests: `a_huge_file_splits_at_hunks` (4 passes: 600/600/600/200, whole hunks, all 10 covered in order), `a_file_under_twice_the_cap_stays_whole` (900, plus the 1,200 boundary) and `an_added_huge_file_splits_at_lines` (3 × 500, `@@ -0,0 +1/501/1001,500 @@`, lines reconstruct the file). `edited()` and `configure()` were added to the fixture helpers. Both existing grouping tests pass unchanged.
- **Inner loop:** 2 runs, 1 red, ~4 min. Clippy caught `mem::replace` with `Default` (now `mem::take`). Two targeted unit runs before that were red on test-side issues: the owner's git config writes `c/`/`i/` prefixes, and my count included the `---`/`+++` header lines. 789 tests pass.
- **Prove (checkpoint):** FR-4's row (`a_huge_file_splits_at_hunks`, `a_file_under_twice_the_cap_stays_whole`, `an_added_huge_file_splits_at_lines`).
- **Evidence (checkpoint 1):** FR-4 → `cp-1/fr-4-huge-file-split.txt` (3 passed).

**Slice**: After this slice, a file with more than 2 × `pass_lines` changed lines is reviewed in parts of about `pass_lines` each instead of one doomed pass. Its diff is cut at hunk boundaries, and a hunk bigger than the cap is cut at line boundaries with a recomputed `@@` header. Each part is its own pass, headed `path (part k of n)`. Files up to 2 × `pass_lines` behave as today. The `--since` range is slice 03's, so this slice builds against `<base>...HEAD` and keeps the diff fetch in one place 03 can change.

## Current code / environment

- **`Pass`** (`src/review.rs:27-32`): `files: Vec<String>`, `lines: u64`, `over: bool`. It is constructed in `group` (`:34-70`) and in `second()`'s doc mode (`src/cli/review.rs:121-125`). A new field breaks that literal. Give it a default-able shape, such as `part: Option<Part>`.
- **`group`** (`src/review.rs:34-70`) sees only numstat totals. A file over the cap goes alone with `over: true` (`:54-64`). Splitting needs the file's diff text, so do it as a step after `group`, over the `over` passes whose `lines > 2 * cap`. Don't teach `group` about git.
- **The diff each pass reads**: `diff_pass_prompt` (`src/review.rs:523-534`) writes `review::diff(root, base, &pass.files)` (`:97-107`, `git diff --no-renames {base}...HEAD -- files`) to `review-{n}.diff`. A part's pass must write only its own slice. Store the part's diff text on the `Part`, so `diff_pass_prompt` writes it instead of calling git.
- **The stub keys on `review-(\d+)\.diff`** in the prompt's last argument (`tests/stubs/cursor-agent:54-55`), so each part needs its own pass number like any other pass.
- **The heading**: `pass_header` (`src/review.rs:507-521`) prints `file_list(&pass.files)` and ` (over {cap}, alone)`. For a part, print `path (part k of n)` with the part's changed-line count. Collect's ` — counted` / ` — unreviewed` suffix is appended after it (`src/collect.rs:190-192`), and the parsers in `passes_and_log` and `pack::collected` key only on that suffix.
- **An added file is one hunk.** `git diff` of a new 2,000-line file is a single `@@ -0,0 +1,2000 @@` hunk. prd §6 (corrected 2026-11-12) says such a hunk is cut at line boundaries. This is the case the field report most likely was.
- **Hunk line kinds**: ` ` (context, old and new), `-` (old only), `+` (new only), `\ No newline at end of file` (belongs to the line before it, counts nowhere). The file header (`diff --git`, `index`, `---`, `+++`, and `new file mode` / `deleted file mode`) is repeated at the top of every part.
- **Tests that pin today's grouping**: `files_group_in_order_and_an_oversized_one_is_alone` (`src/review.rs:722`) and `a_diff_is_three_passes_and_the_oversized_file_is_alone` (`tests/review.rs:112`). Its `c` file has 700 lines, under 2 × 600, so it must stay one pass. Both keep passing unchanged.
- **Fixture**: `branch()` (`tests/review.rs:26-43`) only adds files on `feat/review`. An edited-in-ten-places file needs a helper that commits the file on `main` first, then edits it on the branch (prd §13 seed state).
- **Deliberate non-change**: prd §15 "Not handled: a timeout that scales with pass size". Splitting is the answer to the one oversized case seen, not a new timeout rule.

## Tasks

### 1. Split one file's diff into parts — high — blocked by: —
**Source**: §6 FR-4: "A file with more than 2 × `pass_lines` changed lines is cut at hunk boundaries (`@@` headers of `git diff`) into parts of about `pass_lines` each… A single hunk larger than the cap is cut at line boundaries into parts of about `pass_lines`, each with its own recomputed `@@` header."

`src/review.rs`: a pure function from one file's diff text and the cap to a list of part diffs. Each part holds the file header plus whole hunks, greedily packed up to the cap in changed lines (`+` and `-`). A hunk over the cap is cut into chunks of about the cap in changed lines. Each chunk gets a recomputed `@@ -a,b +c,d @@`:

- `a` and `c` advance by the old and new lines of the chunks before it.
- `b` counts the chunk's context and `-` lines, and `d` counts its context and `+` lines.

A `\ No newline` line stays with the line before it.

**Acceptance:** unit tests on real `git diff` text:

- `a_huge_hunk_splits_with_headers_git_can_apply`: each recomputed part passes `git apply --check` against the base file, in order, in a temp repo.
- `whole_hunks_pack_up_to_the_cap`.
- `the_file_header_heads_every_part`.

### 2. Wire parts into the passes — high — blocked by: 1
**Source**: §6 FR-4: "one pass per part… Its pass heading reads `path (part k of n)`… `Pass` gains the part it covers, and `diff_pass_prompt` writes only that part's slice of the file's diff."

Add `part: Option<Part>` to `Pass`, where `Part` holds `k`, `n` and the part's diff text. After `group`, in one function both `collect` and `second()` call, replace each pass with `over && lines > 2 * cap` by its parts. Each part is a `Pass` with the same single file, `lines` = the part's changed lines, `over: false` and `part: Some(..)`. Fetch the file's diff with `review::diff`.

`diff_pass_prompt` writes `part.diff` when present. `pass_header` prints `path (part k of n)`. Doc mode's literal gets `part: None`.

**Acceptance:**

- `a_huge_file_splits_at_hunks`: a file on `main` edited in ten places for 2,000 changed lines becomes 4 passes. Each `review-n.diff` holds whole hunks and about 600 changed lines at most, and the collect headings read `(part k of 4)`.
- `a_file_under_twice_the_cap_stays_whole`: 900 lines, one pass.
- `an_added_huge_file_splits_at_lines`: a new 1,500-line file, 3 parts.

### 3. Verify — medium — blocked by: 2
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q`

**Acceptance:** inner loop green, one commit.

### 4. Prove — at the checkpoint — blocked by: 3
prd §13 row **FR-4** (`a_huge_file_splits_at_hunks`, `a_file_under_twice_the_cap_stays_whole`, `an_added_huge_file_splits_at_lines`). It is proven below the UI: the checkpoint runs these and keeps the transcript.

**Acceptance:** after the checkpoint, the transcript is in Progress.

**Code-review fixes (2026-11-12).**

- `review::diff` ran plain `git diff`, so `color.diff=always` or a `diff.external` tool hid every `@@` line from the splitter. The file then went to the reviewer as one oversized pass, the very case this slice exists to remove. Its paths were also git pathspecs, so `a[1].rs` pulled in `a1.rs`'s diff too. It now runs `git --literal-pathspecs diff --no-color --no-ext-diff`.
- A file that changed type (to or from a symlink) prints two file headers, and `split_diff` cut the second one into the first one's last hunk with wrong `@@` counts. Such a diff now stays whole (`a_diff_with_two_file_headers_stays_whole`).
