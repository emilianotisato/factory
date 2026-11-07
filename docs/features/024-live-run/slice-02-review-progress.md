# Slice 02 — Review collect reports its passes as they finish

**Source**: [`prd.md`](prd.md) → §7 FR-5 (`collect_started`, `review_pass_done`, `collect_done`), §2, §11 slice recon corrections. **Depends on**: 01 (the three event kinds it declares). Independent of 03 to 07.
**Created**: 2026-11-07

**Progress**:
- **Built:** `review::Progress {Started, PassDone}` and `review::Sink`. `run_passes` gets `(k, elapsed)` from each pass thread over an `mpsc` channel, passes it to the sink on the scope's own thread while the passes run, and keeps receiving after a sink error so it returns that error only after every reviewer has ended. `collect` sends `Started {passes: 0, dir: None}` when the size rule skips the second opinion, and `second_opinion` sends `Started {passes, dir}` once the dir exists, before the probe. `collect::passes_and_log` reads a reused file's pass headings and `log:` line. `closeout::collected` takes `run` and emits `collect_started`, `review_pass_done`, and `collect_done {seconds, ok}` on the fresh path, and `collect_started {reused: true}` then `collect_done {0, true}` on the reuse path. The `factory review` CLI passes `None`. AD-3 covers what happens when the sink fails.
- **Tests:** `review_passes_report_as_they_finish` is in `tests/review_progress.rs`, a binary of its own because it sets the process env. It also checks that a failing sink still waits for every reviewer. Extended `a_finished_board_polishes_ships_pushes_and_opens_the_draft_pr` and `polish_collects_once_and_a_rate_limited_respawn_reuses_the_file`. The pinned review tests are unchanged and green.
- **Deliberate:** ten existing tests pin the exact event-kind sequence of a whole run (in `tests/checkpoints.rs`, `closeout.rs`, `commits.rs`, `loop.rs`, and `run.rs`). The new kinds were added to their expected lists before `polish_spawned`. Nothing was removed from them.
- **Inner loop:** 3 runs, 2 red, about 3.5 min. Both red runs were the same failure: those ten sequence pins.
- **Prove:** `review_passes_report_as_they_finish` transcript is left for the checkpoint.
- **Proved (checkpoint 1):** FR-5, this slice's part: `review_passes_report_as_they_finish`, 1 passed — `cp-1/fr-5-phase-events.txt`

**Highest risk of this feature.** It changes the shape of `review::run_passes` and `collect::collect`, which the run-less `factory review` CLI also calls, and moves event emission into a scoped-thread join. Keep the CLI's output byte for byte.

**Slice**: After this slice, the polish phase's review collect, today the longest silence in the research (15 to 30 min with no event), writes `collect_started` when it starts, one `review_pass_done {k, of, seconds}` as each pass ends in finishing order, and `collect_done {seconds, ok}` when it is over, on the fresh and the reuse path. `factory review` with no run passes no sink and prints what it prints today. Reading these events (`now.waitingOn = review`, `now.progress`) is slice 05's.

## Current code / environment

- **The passes**: `review::run_passes` (`src/review.rs:405-443`) writes every pass's progress line up front (`:414-423`), spawns one scoped thread per pass (`:424-436`), and joins them in pass order (`:437-440`). A pass's time is `Ran.elapsed` (`:399-402`). The reviewer runs inside `run_pass` (`:371`).
- **`Db` is not Sync**: `pub struct Db { conn: Connection }` (`src/db.rs:172-174`) and `events::emit` takes `&Db` (`src/events.rs:284`). prd §7: "the pass threads send on a channel, and the caller emits through `run`." The receive loop must run on the scope's own thread **while the passes run**, before the joins; draining after the joins reports every pass at the end, in any order, and the acceptance's "k=2 before k=1" still passes only by luck.
- **`collect`**: `collect::collect(root, base, cfg, progress)` (`src/collect.rs:43`) runs `numstat`, then the leaks scan (`:47`), then `needs_second_opinion` (`:48`). A small diff skips the second opinion (`:49-59`). `second_opinion` (`:103-155`) creates the review log dir with `review::new_log_dir` (`:124`), probes (`:128`), groups the passes (`:127`), runs them (`:128-138`), and writes `log: <dir>` (`:153`) and one `### Pass n — … — counted|unreviewed` heading per pass (`:146`).
- **The callers**: `closeout.rs` `collected()` (`src/closeout.rs:335-379`), called from `:196`; `cli/review.rs:86` (`factory review collect`) and `cli/review.rs:136` (`factory review --diff/--doc`). prd §2: "The run-less `factory review` CLI passes none."
- **The two collected paths**: reuse when `collect::reusable` holds at the lane's HEAD (`src/closeout.rs:342-350`), else fresh (`:351-379`), whose `match` on `written` (`:359-378`) logs success or "collect failed" and never errors. `collected()` has no `run` today; prd §7: "`collected()` (`:335`) gains a `run` parameter, or the emits move to its caller at `:196`."
- **Where `passes` and `dir` are known**: not at `closeout.rs:352-354`. They exist only inside `second_opinion`, after the leaks scan (§11 slice recon correction). So the fresh path's `collect_started` is the sink's first message, sent once they are known: `{passes: passes.len(), dir: Some(dir)}` from `second_opinion`, or `{passes: 0, dir: None}` when the size rule skips it. The reuse path reads them from the file: the count of `### Pass ` headings, the `log:` line, `reused: true`.
- **Reuse also ends**: slice 05 reads `review` as "a `collect_started` with no later `collect_done`" (prd §4 rule 7). The reuse path emits `collect_done {seconds: 0, ok: true}` right after its `collect_started` (§11 slice recon correction).
- **Tests that pin today's behaviour**: `the_passes_run_at_once_and_print_in_pass_order` (`tests/review.rs:174`) uses the reviewer stub's `sleep_secs` per pass (`tests/stubs/cursor-agent:17-18`) and asserts pass-order stdout and every `review pass n/3:` line on stderr. `collect_writes_the_head_the_plan_the_leaks_and_one_section_per_pass` (`tests/review.rs:501`) and `collect_skips_the_second_opinion_of_one_small_file_without_calling_the_reviewer` (`:588`) pin the file. All must stay green unchanged.
- **Deliberate non-change**: prd §14 "No transcript, pack, collect.md or review viewer". The events carry counts and seconds only, never review text.

## Tasks

### 1. A progress sink through `run_passes` and `collect` — high — blocked by: —
**Source**: §7 FR-5: "`review_pass_done {k, of, seconds}`, emitted as each pass ends. `run_passes` prints every line up front and joins in order … the pass threads send on a channel, and the caller emits through `run`. `collect` and `run_passes` take an `Option` sink. The run-less `factory review` CLI passes `None`."

`src/review.rs`, `src/collect.rs`, `src/cli/review.rs`. A small enum in `review.rs` (e.g. `Progress::Started { passes, dir }`, `Progress::PassDone { k, of, seconds }`) and a sink type such as `Option<&mut dyn FnMut(Progress) -> Result<()>>`. In `run_passes`, each thread sends `(k, elapsed)` on an `mpsc` channel when its pass ends; the scope's own thread drops its sender, receives until the channel closes and calls the sink for each, then joins in pass order as today. With `None`, behaviour and output are exactly today's. `collect` passes `Started` as described in Current code, then forwards its sink to `run_passes`.

**The trap:** a sink error must not leave reviewer processes running: today the progress lines are written "before any spawn" for that reason (`src/review.rs:413`). Keep receiving until every thread ends even if the sink fails, then return the first sink error.

**Acceptance:** `review_passes_report_as_they_finish`: a library-level test runs `collect::collect` (or `run_passes`) with the reviewer stub on `PATH` and `sleep_secs {"1": 2, "2": 0}` as `the_passes_run_at_once_and_print_in_pass_order` does, collects the sink's messages, and asserts `Started {passes: 2, dir: Some(_)}` first, then `PassDone k=2` before `PassDone k=1`, each with `of == 2`. The three existing review tests above stay green unchanged.

### 2. The polish collect emits through the run — high — blocked by: 1
**Source**: §7 FR-5: "`collect_started {passes, dir, reused}`, on both the fresh path (`closeout.rs:352-354`) and the reuse path (`:342-350`) … A small diff that skips the second opinion has `passes: 0`", "`collect_done {seconds, ok}` at `closeout.rs:359-378`"; §11 slice recon corrections (the sink carries `collect_started`; the reuse path emits `collect_done`).

`src/closeout.rs` (`collected()` and its call at `:196`). Give `collected()` `run: &mut Run` (it is called with `run` in scope at `:196`). The sink closure emits `CollectStarted` on `Started` and `ReviewPassDone` on `PassDone`; `collect_done.ok` is `true` on `Ok(text)` and `false` on the `Err` arm (`:370-377`); `seconds` is the `secs` already computed at `:358`. Keep the "polish: …" driver-log lines as they are.

**The trap:** `collected()` borrows `out` for `Stamped::new(out)` (`:354`) while the sink needs `run`; both are separate `&mut` borrows, so build the sink closure over `run` only and leave `out` to `Stamped`.

**Acceptance:** extend `a_finished_board_polishes_ships_pushes_and_opens_the_draft_pr` (`tests/closeout.rs:366`) to assert, in seq order, `collect_started` with `reused: false`, `review_pass_done` events whose `k` set is `1..=of` (none when `passes` is 0), then `collect_done` with `ok: true`. Extend `polish_collects_once_and_a_rate_limited_respawn_reuses_the_file` (`tests/quota.rs:496`, the one end-to-end reuse) to assert the respawn's `collect_started {reused: true}` is followed by a `collect_done`. The unit test `a_file_at_the_same_head_is_reused_unless_it_owes_a_rerun` (`src/closeout.rs:2548`) is updated for the new signature and keeps its asserts.

### 3. Verify — medium — blocked by: 2
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 4. Prove — at the checkpoint — blocked by: 3
prd §12 row **FR-5**, this slice's part: run `review_passes_report_as_they_finish` and keep the transcript. (`ci_waiting_emits_on_a_change_of_checks_only` and `an_empty_fix_is_an_event` are slice 04's.)

**Acceptance:** after the checkpoint, this slice's Progress names that transcript.
