# Slice 02 — Driver owns done

**Source**: [`prd.md`](prd.md) → §3 FR-8, and FR-10's contract line. **Depends on**: — (independent of every other slice: no other slice touches `assets/contract-slice.md`, `src/scheduler.rs` or `src/stream.rs`)
**Created**: 2026-10-15

**Progress**: built — the slice contract drops the building and done-and-check lines and its Verify line asks for the quiet flag on a targeted test run; `done_by_driver` (`src/scheduler.rs`) runs before routing in `reap` and in `settle_orphans`: a build worker's `done` hint on a `building` slice checks the lane, writes `done`, or writes a `docs check failed in slice NN's lane` BLOCK listing each error with its code (an open one is reused) and sets `blocked`; the module and `StatusHint` docs say so; the stub gains `board: false`; three stub-run tests in `tests/loop.rs` (board-free done lands, a broken link blocks with `E-LINK-BROKEN`, a driver kill while a board-free builder lingers lands on restart), the self-set-done case rides slices 02-03 of the first. Inner loop: 3 runs, 1 red, ~4 min — the red caught `the_slice_contracts_commands_run_on_the_fixture` (`tests/closeout.rs`) pinning 7 contract commands, now 4. Deviation: that pin in `tests/closeout.rs` is outside the files the slice names; it changes with the contract. Prove rows FR-8 and FR-10 (contract half) are left for the checkpoint.
- **Checkpoint 1, FR-8**: the board-free builder lands done, the board-setting builders land as before, the broken link blocks with `E-LINK-BROKEN` in the BLOCK body, the killed driver lands the slice on restart with no respawn — cp-1/fr8-driver-done.txt
- **Checkpoint 1, FR-10 (contract half)**: a slice pack's `# Contract` Verify line asks a targeted test run for the quiet flag — cp-1/fr10-contract-quiet.txt

> **Highest risk in this PRD.** It changes how the driver routes every build worker's exit, live and after a restart. Task 2 carries the risk.

**Slice**: A builder no longer sets its own slice building or done, and no longer runs `docs check`. When a build worker exits with `STATUS: done` and the board still says building, the driver checks the worker's lane tree: clean, it sets the slice done and lands it; errors, it writes a BLOCK holding them and parks the slice. A restarted driver does the same for a builder that finished while it was down. A builder on an old pack that sets done itself routes as today. The contract's Verify line also asks for quiet targeted test runs (FR-10); the inner-loop string and `/gate` are slice 06's.

## Current code / environment

- **The contract**: `assets/contract-slice.md:12` "`{factory} docs set {NNN} slices.{N}.status=building`"; `:14` the Verify line; `:19` "`{factory} docs set {NNN} slices.{N}.status=done`, then `{factory} docs check` with no errors"; `:23` the Blocked line, which stays; `:26` `STATUS: done | blocked`, which stays. It is `pack::CONTRACT`, `include_str!` at `src/pack.rs:10`.
- **The driver already sets building**: `spawn_on()` (`src/scheduler.rs:658`) writes `slices.N.status=building` and the lane when `fresh` (`:674-681`). A respawn after an answer finds building too: `factory answer` sets an answered slice back to building (`src/cli/answer.rs:208`).
- **Live routing**: `reap()` (`:489-519`) re-reads `docs.status(feature)` after a worker exits (`:507`), then calls `route_exit()` (`:510`). `route_exit()` (`:822-889`) sends a merge worker to `route_merge` by role, then matches `slice_status(board, slice)`: `"done"` lands through `land_slice` and settles; `"blocked"` settles Blocked, finds the block with `open_block` and emits it; anything else goes to `settle_limited` or `settle_dead`. The hint is `exit.summary.status_hint` (`:847-850` prints it).
- **Restart routing**: `settle_orphans()` (`:1560`) gets a `board` its caller computed once (`:218`, `:259`), reads each dead worker's `summary_from_disk` (`:1600`), takes `status` from that board (`:1612`) and matches it (`:1628`). It has no `Docs` handle.
- **The lane tree**: builders always run in a lane worktree (`prepare_lane`, `:763`). Its path is `lanes::dir(&paths::data_dir(), &lanes::main_tree(&repo)?, lane)`, as at `:1086-1087`. `Running` carries `lane`; `WorkerRef` (`src/run.rs:139`) carries `lane`, `role` and `session_id`. A build worker's role is `"build"` (`src/worker.rs:121`).
- **The check**: `check::run(root) -> Report` (`src/docs/check.rs:82`), today called only by the preflight (`src/docs/preflight.rs:97`). `Report.errors` holds `(code, message)` pairs; `lines()` (`:60-64`) formats them. The lane's `state.toml` is stale, because a worker's `docs set` writes the main tree through `FACTORY_BOARD` (`src/worker.rs:55`, `:246`), and builders already ran `docs check` in that lane with no errors.
- **Writing a block**: `block_dead_slice()` (`:1985-2030`) writes `answers::add_block(&feature_dir, Some(slice), None, &title, &body)` with a `**Happened:**`, `**Options:**`, `**Recommended:**` body, sets `slices.N.status=blocked` in `repo.join(&board.dir)`, then `emit_blocked`. `open_dead_block()` (`:1978`) finds an entry a kill left before the status flipped, so a restart writes no second one.
- **The hint's rule**: `src/scheduler.rs:1` "The board is the truth: every tick routes on it, and a worker's `STATUS:` line is a hint the row keeps." `src/stream.rs:99` "A hint the row keeps; the driver routes on the board, never on this." `StatusHint::from_result` is at `src/stream.rs:117-131`.
- **The stub builder writes the board on every call**: `setStatus(feature, nn, 'building')` (`tests/stubs/claude:453`) and `setStatus(feature, nn, 'done')` (`:524`). Its step keys are documented at `:22-58`. `sh` runs a script in the lane after the touched files; `linger_ms` sleeps after done is on the board and before the stream.
- **Tests that pin the old contract**: `pack_is_the_slice_the_cited_sections_the_contract_and_the_inner_loop` (`tests/loop.rs:235`) asserts the pack holds "docs set 001 slices.1.status=done`" (`:281-284`) and "`factory docs set 001 slices.1.status=building`" (`:287-290`). FR-8 flips both. Slice 04 edits the `AUTONOMY` assertion three lines below (`:294`), so keep these edits to their own lines.
- **The driver-kill pattern**: `tests/loop.rs:1167-1178` reads the driver's pid from the run state, waits for a park, and sends `SIGKILL`. `the_driver_killed_mid_slice_resumes_and_finishes` and `a_dead_worker_is_noted_and_respawned_once` are in the same file.
- **Deliberate non-change**: *prd §7: "Not handled: resuming a builder when the driver's check fails. Accepted 2026-10-15: the slice blocks with the errors."* Do not resume the worker with the errors.
- **Deliberate non-change**: a merge worker still routes by role, and every hint other than `done` on a build worker routes as today.

## Tasks

### 1. The contract drops the builder's bookkeeping — high — blocked by: —
**Source**: §3 FR-8: "The contract drops the building line and the done-and-check line." §3 FR-10: "The contract's Verify line says a targeted test run uses the quiet flag too."

`assets/contract-slice.md`: delete `:12` and `:19`. Add to the Verify line (`:14`) that a targeted test run uses the stack's quiet flag, as the inner loop does. The Blocked line (`:23`) and the `Return exactly` block stay.

**Acceptance:** `pack_is_the_slice_the_cited_sections_the_contract_and_the_inner_loop` asserts the slice pack holds no `status=done` and no `status=building` line, still holds `--kind block --slice 1 `, and its `# Contract` holds the quiet-flag rule.

### 2. The driver sets done or blocks on a build worker's done hint — high — blocked by: 3
**Source**: §3 FR-8: "When a build worker exits with the hint `done` and the board still says building, the driver runs `check::run` on the worker's lane tree: **No errors:** it sets the slice done and routes it as today's done. **Errors:** it writes a BLOCK entry for the slice whose body holds the check's errors, sets the slice blocked, and routes it as today's blocked. The worker is not resumed. A restarted driver applies the same rule to a dead builder, from the hint in its stream file. A board that already says done … routes as today. No hint, or any other, routes as today."

`src/scheduler.rs`: one function both paths call before they match the slice status. It takes the run, the board, the slice, the role, the lane and the summary. When the role is `build`, the hint is `Done` and the slice is `building`, it runs `check::run` on the lane tree and writes `done`, or the block and `blocked`, to the board in the main tree (`repo.join(&board.dir)`). It returns the status to route on.
- Live: in `reap()`, before `route_exit()` (`:510`); re-read the board with `docs.status(feature)` after a write, so `route_exit` and `land_slice` see it.
- Restart: in `settle_orphans()`, before the `match status` (`:1628`); route on the returned status.

Write the block as `block_dead_slice()` does: its own title, a body whose `**Happened:**` lists the check's error lines with their codes, `**Options:**` and `**Recommended:**` (fix the docs in the lane by hand and set the slice done, or set it back to `pending`).

The rule at `src/scheduler.rs:1` and `src/stream.rs:99` changes with it, "and both say so" (§3 FR-8 Accepted cost): the board is still the truth, and a build worker's `done` hint plus a clean check of its lane is what writes done.

**The trap:** `settle_orphans` matches `status` from a board computed before its loop (`:1612`, `:1628`), so after the write it must route on the new status, not the stale one. And a kill between `add_block` and the status flip leaves the entry open: find it first, as `open_dead_block()` (`:1978`) does, or the restart writes a second one.

**Acceptance:** stub-run tests in `tests/loop.rs`:
- a builder that returns `STATUS: done` without touching the board lands its slice done and committed;
- a builder whose lane holds a `docs check` error (its `sh` step writes one, such as a broken link in a doc) blocks its slice, and the BLOCK body names the error's code; no second worker spawns on it;
- a builder that sets done itself lands as today;
- a driver killed while a board-free done builder lingers lands the slice on restart.

### 3. The stub builder can leave the board alone — high — blocked by: —
**Source**: §3 FR-8 Acceptance: "The stub builder sets building and done on every call today (`tests/stubs/claude:453`, `:524`), so it gains a step key that leaves the board alone."

`tests/stubs/claude`: a step key that skips both `setStatus` calls on a `done` slice. Document it with the others at `:22-58`. Task 2's tests use it.

**Acceptance:** task 2's board-free tests run on it; without the skip, they would pass on the stub's own `done`.

### 4. Verify — medium — blocked by: 1, 2, 3
`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 5. Prove — at the checkpoint — blocked by: 4
`prd.md` § QA plan rows, driven below the UI:
- **FR-8**: run the stub runs whose builder returns done without the board, with a check error, with the board set, and across a driver kill; keep the transcript of each.
- **FR-10**, its contract half: build a slice pack and show its `# Contract`'s quiet-flag line; keep the transcript. The `docs gate --inner` half is slice 06's.

**Acceptance:** after the checkpoint, each row has its transcript in this slice's **Progress**.

**Code-review fixes (2026-10-15).**
- The docs-check BLOCK recommended fixing the lane by hand and setting the slice `done`, but a `done` set by hand lands nothing: the worker row is already settled, so the lane's work never reached the branch and a later spawn reset the lane. The option now says to answer the block, which respawns a worker on the lane as it is; its `done` lands the slice once the check passes.
- The module docs in `src/scheduler.rs` and `src/stream.rs` said the `done` hint writes the board only on a clean check; they now say it writes `blocked` when the check fails.
