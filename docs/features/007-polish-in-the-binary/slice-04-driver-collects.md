# Slice 04 — Driver collects at polish

**Source**: [`prd.md`](prd.md) → §7 FR-5. **Depends on**: 03 (the driver calls the collect library slice 03 wrote, and reads back the file it writes)
**Created**: 2026-10-15

**Progress**: `polish()` now prepares the lane first, then `collected()` (`src/closeout.rs`) reuses `<run dir>/collect.md` when its head line names the lane's HEAD, or runs the collect against the default branch with `cfg.review`, writes the file and prints `polish: collected <path> against <base> in N.N s`. A failed collect prints `polish: collect failed in N.N s — <reason>` and the section is `collect failed: <reason>`; no file is written. `pack::collected` lifts the plan, the leaks `verdict:` line, the second opinion's lines above the first pass and each `### Pass … — counted|unreviewed` heading (as `- Pass …`), then the file's path; `for_polish` places `# Collected` second. Tests: `the_collected_section_keeps_the_status_lines_and_leaves_the_findings_in_the_file`, `a_collect_that_fails_is_the_section_and_writes_no_file`, and `polish_collects_once_and_a_rate_limited_respawn_reuses_the_file` (`tests/quota.rs`). Inner loop: 2 runs, 1 red (a `cargo fmt` diff), about 2 min. Deviation: none; AD-4 on the progress lines and the head-line match. FR-5's Prove row is left for the checkpoint.
Checkpoint 2 (FR-5): `collect.md` written before the first polish spawn, the pack's `# Collected` section names it, the rate-limited respawn reuses it with no new reviewer call: `docs/features/007-polish-in-the-binary/evidence/cp-2-polish-collects.txt`; the first polish pack: `docs/features/007-polish-in-the-binary/evidence/cp-2-polish-pack-first.txt`.

**Slice**: After this slice, the driver runs the collect in the polish lane against the default branch before the polish worker spawns, writing `<run dir>/collect.md`, and prints one line with how long it took. The polish pack gains a `# Collected` section: the plan, the leaks verdict, each second-opinion pass's status line, and the file's absolute path. A respawn reuses the file while its head line matches the lane's HEAD. A collect that fails never stops the run: the section says `collect failed: <reason>`. The skill and the contract learn about the section in slice 05.

## Current code / environment

- **The polish loop**: `polish()` (`src/closeout.rs:134-319`). Before the loop it computes `default_branch` and `feature_branch` (`:143-144`). Each pass of the loop: `await_quota` (`:147-149`), the death cap (`:150-191`), `attempt` and `spawn` (`:192-193`), **the pack** (`Pack::for_polish`, `:194-204`), the open blocks (`:205-208`), **then the lane** (`closeout_lane(board)` and `scheduler::prepare_lane(run, lane, true, out)`, `:209-210`), `drop_ref` (`:211`) and the spawn (`:212-230`). The pack is built before the lane exists, so the collect forces a reorder: lane, collect, pack. A rate limit pauses and `continue`s (`:262-280`); a dead worker `continue`s (`:281-284`). Both come back through the same lane preparation.
- **The branch guard runs first**: `branch_guard` (`src/closeout.rs:1109-1139`) is called before `polish()` (`:93`), so HEAD is on the feature branch when the collect runs. `tests/closeout.rs:1776-1793` pins that the guard blocks before any polish worker runs; it stays green only if the collect is inside `polish()`.
- **The lane**: `prepare_lane` (`src/scheduler.rs:763`) with `fresh = true` resets the lane to the feature tip on every spawn, so a respawn after a rate limit sees the same HEAD unless something landed. Lanes are detached worktrees of the main repo, so the default branch's name resolves inside one.
- **The base**: `default_branch(run, docs)` (`src/closeout.rs:2254-2265`) reads the run's saved preflight, falling back to `"main"`.
- **The config**: `polish()` gets the run's `cfg: &Config`; `cfg.review` is the `[review]` table (`src/config.rs:16-21`).
- **The pack**: `Pack::for_polish` (`src/pack.rs:445-519`) assembles `# Branch`, the log section, `# Retro`, `# PR body`, `# Contract`, `# Autonomy`. `assemble` (`:66-80`) names each part from its first line only, so `# Collected` becomes the part `collected`. The worker already reads and writes outside the repo: `# PR body` points it at `pr-body.md` in the run dir. Callers: `src/closeout.rs:195` and the tests at `src/pack.rs:1162-1166` (asserts the section list `branch, log, retro, pr_body, contract, autonomy`) and `:1241`.
- **The stub `claude` reads the pack's headings**: it is a merge worker when the first `# ` heading is `# Conflict`; a fix worker on a `# Gate failure` or `# CI failure` line; a checkpoint on a `# Gate transcript` line; a polish worker on a `# Branch` line anywhere (`tests/stubs/claude:138-147`). It reads the run dir from the pack's second header line and the feature dir from the first `<!-- docs/features/…` comment (`:320-323`). The `# Collected` body must hold none of those lines; keep its own headings below `#`.
- **The event sequence is pinned**: `tests/loop.rs:574-596` asserts the run's whole event list, `polish_spawned` and `polish_done` included. The PRD asks for a printed line, not an event.
- **The driver gains the reviewer's signal handler**: running the second opinion in-process installs `review::forward_signals` (`src/review.rs:186-204`) in the driver. The handler kills the reviewers and re-raises, so a signalled driver still dies as it does today.
- **The fixture branch is big enough to call the stub**: every stub slice writes its own `src/slice-NN.txt` (`tests/stubs/claude:481-485`), so a run's branch has more than one changed file, and the size row asks for the second opinion.
- **A rate-limit test pattern**: `a_rate_limited_fix_worker_is_not_a_red` (`tests/quota.rs:414-460`) drives a `rate_limited` step on a non-slice worker with the fake clock and checks `outcomes(&fx, &dir, role)`. `driver_log(dir)` (`:127`) reads what the driver printed. The stub `cursor-agent` logs every call to `cursor-agent.log` in `fx.stub_dir()`.
- **Deliberate non-changes**: *(prd §13 "Not handled: `--others` inside a run.")* The driver's collect has no `--others`. *(prd §13 "Not handled: the driver running the fallback as its own workers.")* An unreviewed collect leaves the fallback to the worker. *(prd §7 "The findings text stays in the file; the worker reads it at §2, so it does not anchor the local review.")* No finding goes into the pack.

## Tasks

### 1. Collect, or reuse, before the pack — high — blocked by: —
**Where**: `src/closeout.rs:192-230`.

**Source**: §7 FR-5: "Before the first polish spawn, the driver prepares the polish lane and runs the collect in it against the default branch, writing `<run dir>/collect.md`. A respawn reuses the file when its head line matches the lane's HEAD, and collects again when it does not." And: "A collect that fails (a git error) does not stop the run: the section says `collect failed: <reason>` … The driver prints one line with the collect's duration."

Move `prepare_lane` above the pack. Then: read `<run dir>/collect.md`; when its head line names the lane's HEAD, reuse it; otherwise run slice 03's collect in the lane against `default_branch` with `cfg.review`, write the file, and print one `polish:` line with the duration and the path. A collect error becomes the section's `collect failed: <reason>` and the loop goes on to the spawn.

**Acceptance:** task 3's test sees the file before the spawn and the line in the driver log; a unit test turns a failed collect into the `collect failed: …` section.

### 2. `# Collected` in the pack — high — blocked by: 1
**Where**: `src/pack.rs:445-519` and its tests at `:1162-1166` and `:1241`.

**Source**: §7 FR-5: "The pack gains a `# Collected` section: the plan, the leaks verdict, each second-opinion pass's status line, and the file's absolute path. The findings text stays in the file; the worker reads it at §2, so it does not anchor the local review."

`for_polish` takes the section body and puts `# Collected` right after `# Branch`. Lift the plan, the leaks verdict and each pass's status line out of the file by the headings slice 03 gave them, and end with the absolute path. A reused file from an earlier driver gives the same section. Update the section list to `branch, collected, log, retro, pr_body, contract, autonomy`.

**Acceptance:** the updated pack test lists `collected` second; a unit test builds the section from a collect file holding findings and asserts the findings text is not in it.

### 3. A run collects once, and a respawn reuses it — high — blocked by: 1, 2
**Where**: `tests/quota.rs` (it has the fake clock), or `tests/closeout.rs`.

**Source**: §7 FR-5 acceptance: "In the stub harness, a run reaching polish writes `collect.md` before the polish worker's spawn, and the polish pack holds `# Collected` with the file's path. A rate-limited polish worker's respawn reuses the file without a second stub `cursor-agent` call."

A plan whose polish step is `rate_limited`, then a `done` polish. Run to `finished`.

**Acceptance:** `collect.md` is in the run dir with a head line; the first polish pack's `# Collected` holds its absolute path; the driver log has the duration line before the first `polish: worker … spawned` line; the stub `cursor-agent` was called during the first collect (at least one call, so the next check proves something), and the respawn's pack exists with the call count unchanged; `outcomes(&fx, &dir, "polish")` is `["rate_limited", "done"]`.

### 4. Verify — medium — blocked by: 1, 2, 3
`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 5. Prove — at the checkpoint — blocked by: 4
Rows from [`prd.md`](prd.md) §11 this slice proves, below the UI:

- **FR-5**: "run a stubbed feature to polish: `collect.md` exists before the polish spawn, the pack names it, a respawn reuses it" — task 3's test transcript, and the first polish pack's `# Collected` section.

**Acceptance:** after the checkpoint, Progress holds the transcript and the section.
