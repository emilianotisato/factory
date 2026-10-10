## Decisions

### AD-50001 — Who handles a HEAD off the run's branch once the closeout is due? · polish · 2026-10-10

**Chose:** the closeout. The tick's branch check (AD-20005) parks the run as before while slices are left, a worker is in flight, or the board is not all done and checked. Once the closeout owns the run (`scheduler::closeout_owns`), the tick skips that check and the closeout's `branch_guard` runs at its entry, before polish, ship, push and the CI watch, and again before each CI fix round. Off the branch, it writes a `BLOCK-n` in the closeout's phase. Option 1 is "put HEAD back on the feature branch and answer this block". Both holds stay: `commit::hold_branch` keeps every commit on the branch, and `ci::branch_problem` keeps every push there.
**Over:** the tick's silent `waiting-owner` park for every phase. Under that park, `the_push_refuses_a_head_off_the_feature_branch_and_the_pr_names_its_head` waited for a BLOCK-2 that never came. The other options were dropping the closeout's guard, or making the tick write the block.
**Because:** in the closeout, the owner is already answering blocks, and a park with no block and no ask says nothing in `factory status` about what to do. Before the closeout, the self-clearing park keeps lanes of slices moving without an answer step. Each phase has one owner and one outcome. The cost: if HEAD moves while a polish or CI fix worker is running, `hold_branch` still makes that worker's commit error, and the run fails (AD-20005's cost, unchanged).
**Undo:** drop the `closeout_owns` condition on the tick's `branch_refusal` check in `scheduler::drive_inner`, and the entry `branch_guard` call in `closeout::run`.
**Review:** fyi

### AD-50002 — Whose are the files a closeout worker changed without naming them? · polish · 2026-10-10

**Chose:** the same rule as AD-20007, with `alone: true`, since a polish or CI fix worker always runs alone. Each worker's spawn baseline is saved under `runs/<id>/untracked-before/closeout-polish.json` or `closeout-ci-fix-<round>.json`. A tracked, unprotected file it changed unnamed goes in its commit. A new untracked file is listed in a new `closeout_leftovers {role, files}` event and in the driver log, and stays in the tree. A resumed driver settling an orphan loads the saved baseline. A file listed once is never swept later.
**Over:** leaving closeout commits to the named paths only, which strands a formatter's or a `sed -i` edit outside every commit; or reusing `untracked_leftovers`, whose `slice` field a closeout worker has no value for.
**Because:** the slice and checkpoint workers already work this way (a354d0d). The polish and CI fix workers were the two left out. Both run alone, so a tracked change under them is their work.
**Undo:** pass `alone: false` to `scheduler::unnamed` in `commit_polish` and `commit_ci_fix`, or drop the `unnamed` calls and `EventKind::CloseoutLeftovers`.
**Review:** fyi

### AD-50003 — How does the CI fix contract name the round? · polish · 2026-10-10

**Chose:** "fix round {round}", with no cap in the sentence. The asset's first line is edited in place, so it is still 22 lines. `a_red_check_gets_one_fix_round_then_green` now asserts "fix round 1, unattended".
**Over:** "fix round {round} of three".
**Because:** the cap is three rounds since the owner last answered (autonomy.md:77), so after an answer the round number passes three and the old line read "fix round 4 of three". The cap is the driver's to enforce, not the worker's to know.
**Undo:** restore the "of three" wording on line 1 of `assets/contract-ci-fix.md` and in the test.
**Review:** no

### AD-50004 — How does `factory status` round the budget use? · polish · 2026-10-10

**Chose:** up: `ceil` of the unrounded weighted sum (`quota::weighted_sum`, now `pub`).
**Over:** the rounded `quota::weighted`, which shows 1.4 against a budget of 1 as "1/1"; or a fractional formatter.
**Because:** the budget parks a run when the sum is greater than the budget. For a whole-number budget, `ceil(sum) > budget` holds exactly when that is true, so the line shows "over" exactly when the run is over. Above 1k, the change is invisible. `run_parked.weighted` and `report` keep the rounded value, and `STUB_WEIGHTED` in tests/quota.rs holds.
**Undo:** use `quota::weighted` again in `cli::status::board`.
**Review:** no
