# QA guide — PRD-006 Comment discipline

Generated 2026-10-14 from the one Human row of [`prd.md`](prd.md) § QA plan: **FR-5**, your sample read of the comment cleanup. Every other row is proven by a test or a command at the checkpoints; that evidence is in the slice files' Progress.

The cleanup is already merged (`9e79a87`, §7), so this read runs on the merge's diff and needs no branch. Do it whenever suits you; it does not block the run. Budget: **about 30 minutes**. ★ marks the checks that matter most.

All commands run in the factory checkout.

## FR-5 — Sample read of the cleanup merge (≈30 min)

1. ★ **20 comments the cleanup deleted** *(12 min)*. Each line prints with its file:

   ```
   git diff -U0 9e79a87^1 9e79a87 -- '*.rs' | awk '/^--- a\//{f=substr($0,7)} /^-[[:space:]]*\/\//{print f": "substr($0,2)}' | shuf -n 20
   ```

   For each, ask one question: did it tell a reader something the code cannot show? **Pass:** none of the 20 did. **Fail:** one did. Find it in context with `git show 9e79a87^1:<file>` and note the file and the line. This is the check that matters most: a lost why is the one cost of the cleanup that a later run cannot see.

2. ★ **20 comments the cleanup kept** *(10 min)*:

   ```
   git grep -nE '^\s*//' 9e79a87 -- '*.rs' | shuf -n 20
   ```

   For each, ask: does it state what the code cannot show, in one line, with no plan id, history or restated name? **Pass:** all 20 do. **Fail:** one restates its code or cites the plan. Recording data is allowed: a fixture's own slice numbers in `tests/checkpoints.rs`, `tests/closeout.rs`, `tests/merge.rs` and `tests/quota.rs` are data.

3. **The renames** *(5 min)*. Six test functions and one fixture folder:

   ```
   git diff -U0 9e79a87^1 9e79a87 -- '*.rs' | grep -E '^[-+]\s*(pub )?(async )?fn [a-z_0-9]+\(\)'
   git diff -M --name-status 9e79a87^1 9e79a87 | grep '^R'
   ```

   For each pair, ask: does the new name say what the test checks, without the plan's words? **Pass:** all seven do.

4. **Record the result** *(3 min)*. A miss becomes a dated line in [`prd.md`](prd.md) §7 FR-5 under "Merged 2026-10-14", with the file and the line. Then the fix is a follow-up commit, or a new row in FR-3's table if the miss is a class the check should catch. No miss: write one dated line saying so.

## Not in this guide

FR-1 to FR-4 and FR-5's run row are machine rows, proven at the checkpoints of slices 01 to 05.
