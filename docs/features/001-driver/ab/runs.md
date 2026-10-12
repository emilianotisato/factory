# FR-11 A/B runs

## icg-cloud: PRD-018 cross-client inbox

Base `cde31ac` on `docs/prd-018` (main `4f93556` plus the PRD, its 6 slices and the approval). PR #23 (PRD-017) left open and unmerged. Worktrees made by `worktrees.sh`. Base gate green in `wt-a`: 2,378 tests, 424 s.

Both runs: Opus 5.5, effort high, bypass permissions, `autoCompactWindow` 200000. Factory workers use the same, from `config.rs` defaults. Preflight ask (6 slices, `lanes: 1`) answered "proceed, serial" for both, by the owner, before launch.

| run | tool | worktree / branch | started | session | PR |
|---|---|---|---|---|---|
| A | `/deliver 018` | `wt-a` · `feat/018-cross-client-dashboard` | 2026-10-11 12:09 | session_01WGNvo9koY6fY8J21kJQjZY, local `480153fa-444d-439a-936f-8dfbccc268ac` (tmux `ab-a`) | [#24](https://github.com/ICG-Argentina/icg-cloud/pull/24), draft at 15:16 |
| B | `/factory-deliver 018` | `wt-b` · `feat/cross-client-dashboard` | 2026-10-11 16:06 | session_017KA5ccBXhGm2fxFttyc2MS, local `708546d1-e7b4-4dea-b685-03dc9331686b` (tmux `ab-b`); run `018-20261011-6490` | |

### Owner answers

Every answer one run gets, the other gets word for word, so the runs differ only in the tool.

**BLOCK-101 (run A, slice-01, 12:24).** A gap in the shared base, so run B should hit it too. PRD §9 seeds the inbox fixtures in `QaSeeder::run()`, and 32 Sync feature tests count whole tables after seeding. The owner chose the builder's option 1, sent at 12:26:

> BLOCK-101: option 1. Scope the whole-table SyncRun and StagedRecord counts in the failing Sync tests to the profile under test, and exclude the seeded inbox run where a test asserts a list. Fix the browser-test assertions that break the same way (AD-101). §9 stays as written: the inbox fixtures stay in QaSeeder::run(). Record this as the resolution and resume the run.

Run B gets the same text under its own block id, with "option 1" dropped if its options differ.

**Owner steering (run A, 13:37, during CP-2).** Unprompted, in the session: "But checkpoint should not take this long". The orchestrator narrowed CP-3's browser sweep to the feature's own tests in response. This can only have made run A cheaper, so it biases the comparison against run B, not for it. Run B gets no unprompted messages: the owner answers what the cockpit asks and nothing else.

### Run A outcome

- Draft PR #24 at 15:16. CI green first time; the re-run after the owner review was pending at 15:50.
- Review interview (15:25–15:42): all reviewed, 8 ADs kept, 1 changed (AD-601, fixed in `ddedf1b`). Three UX-rubric lines added; both DEC candidates kept local.
- Open quality questions for the questionnaire: the Cursor second opinion timed out (a fresh Claude reviewer stood in), and 13 browser tests outside the gate failed at CP-2. Rerun 15:48 in a third worktree `wt-base`, same setup for both, without the generated Demo and Second modules: the base `cde31ac` and run A's head `6dae3d0` give identical results file by file. The same 5 tests fail on both, and 15 module-dependent tests skip on both. Run A broke none of them; the 5 are already red before PRD-018. Run B's head gets the same check.

### Questionnaire, run A

Answered by the owner 2026-10-11 at 16:04, before seeing any token or cost number, after manual QA in `wt-a` (seeded, logged in, inbox driven).

| # | question | run A | run B |
|---|---|---|---|
| 1 | Does the result do what the PRD asked? | yes | yes |
| 2 | Merge after normal review, at most small fixes? | yes | yes |
| 3 | Decisions you would overturn | 2–3 | 0 |
| 4 | Code quality against the project's standard (1–5) | 5 | 5 |
| 5 | Tests prove the FRs, or only pass? | prove | prove |
| 6 | Built but not asked for | none | none |
| 7 | Asked for but missing | none | none |
| 8 | Overall | ship after minor fixes | ship as is |

Run B answered by the owner 2026-10-11 at 19:31, through two question rounds, before seeing any token or cost number for run B. The cockpit's decision review had come first: 8 kept, 0 changed (`806992c`).

Run B passes the questionnaire if it scores yes on 1 and 2, lists nothing on 6 and 7, scores at least 4 on question 4, and is "ship after fixes" or better on 8.

### Numbers, run A

Measured 2026-10-11 after the questionnaire, from the orchestrator transcript and its 21 subagent transcripts (`icg/run-a-*.txt`). Weighted with `quota.rs`: input + cache write + 0.1 × cache read + 5 × output.

- **weighted_tokens(A) = 16,149,327.** The orchestrator alone is 2,178,976 (13%).
- Build 115 min, 220 min wall from start to the last CI check.
- **Run B's bar: at most 9,689,596 weighted tokens**, with its cockpit at most 5% of its own total.

### Run B launch

Started 16:06, the same day as run A, at the owner's call ("dont wait, lets do it now"), not after the Oct 15 reset. At launch `factory status` read the weekly limit at `allowed_warning 85%`, so a quota pause mid-run is likely; factory pauses and resumes by itself, and a pause counts as wall-clock, not tokens.

The launch text carried the same preflight answer as run A's. The cockpit passed it through as `factory answer ASK-1 "proceed as-is, serial (1 lane)"`.

Factory finding, minor: `factory answer` notes the ask under `## Decisions` as `- **DEC (2026-10-11)**: …`. That is neither an `AD-n` nor a project `DEC-n`.

**No BLOCK-101 twin so far (16:18, slice-01 at 12 min).** Run B's slice-01 worker is editing the same Sync feature tests (`ExplorationTest`, `IngestApiTest`, `RunNowTest`, …) and four browser tests itself, the owner's option 1, instead of blocking. Both slice contracts carry the same rule, "touch nothing outside your slice's files… a file another slice owns is off limits; put it in an AD instead" (`deliver/SKILL.md`, `assets/contract-slice.md:19`). Run A's builder read the Sync tests as off limits; run B's worker reads them as owned by no slice. That is model variance under the same rule, not a factory design difference. It saves run B the block and the slice-01 redo, which run A paid. Check at review: run B must log the scope call as an AD; an unlogged edit outside the slice is a quality finding.

**Run B logged it (16:49, slices 01–04 done, CP-1 green, CP-2 running).** `AD-101 · slice-01 · Review: decide`: run B edited nine tests outside the slice, and chose a different fix from the owner's run A answer. The tests delete the seeded fixture run in `beforeEach`, after the `RunNowTest` precedent, instead of scoping their counts to the profile under test. The owner judges it in run B's review interview. An overturn counts toward question 3.

Factory finding, minor: run B's commit subjects carry no feature scope and no slice number (`feat: Core sync kinds`, `docs: checkpoint 1`). Run A's read `feat(018): slice-03 core sync inbox kinds` and `verify(018): checkpoint after slice-02`.

**BLOCK-1, an owner interrupt run A did not have (16:55).** Checkpoint 2's gate was green, then `docs log merge` refused: `log-slice-03.md` and `log-slice-04.md` each open with a `# Log fragment — slice NN` title above their first `## ` heading. Neither the factory contract nor the feature-docs template writes that title; the two workers added it. The driver parked the run as BLOCK-1 instead of fixing a formatting slip. The cockpit put it to the owner naming only `log-slice-03.md`; the owner picked "Move lines, retry (Recommended)" at 16:58. On resume the driver reran the 7-minute gate that had just passed.

Factory findings: a merge refused for format is mechanical and should be fixed by the checkpoint worker, as `/deliver`'s checkpoint contract does ("fix the offending fragment … and re-run"); the cockpit's question dropped the second file; a resume after a merge refusal should not rerun a green gate on an unchanged tree.

**Slice 02's core code is not committed (found 16:59).** Slice 02's worker wrote 11 new files with Bash heredocs (`cat > app/Core/Inbox/Inbox.php <<'EOF'`): the `Inbox` service, `InboxItem`, `InboxSeverity`, `InboxSource`, `ShowsOnInbox`, `InboxController`, and their tests and fixture modules. The driver commits only paths a `Write`, `Edit` or `NotebookEdit` call named (`src/commit.rs:7`, `WRITING_TOOLS`), so commit `23c08c8` holds the page, the route and the types but none of these. They were listed under `## Untracked leftovers` (`untracked_leftovers`, slice 2) and every later baseline treats them as pre-existing, so no later commit takes them. The local gate stays green because the files sit in the tree. CI on the pushed branch will fail on missing classes.

Factory finding, major: with one lane, a new untracked file that appears while the only worker runs is that worker's; listing it instead of committing it ships a branch that does not build.

Owner's call at 17:01: leave run B alone. No hand commit; the run either recovers in CI or fails on it, and `factory report` splits out what the CI-fix workers cost.

**BLOCK-2, the same refusal again (17:05).** The owner's BLOCK-1 answer was relayed as text and nobody acted on it: the cockpit only relays, and the driver reran the gate (attempt 2, 7.12 min) and refused the same merge. The block's own text says "answer this block once the fragment is fixed", so the fix is the owner's by design, but neither the cockpit's question nor its options said so. Its options are written for an AD-number clash ("renumber the clashing entry"), not for a format slip.

Owner work at 17:09, by the A/B operator at the owner's call: removed the title line and the blank line under it from both fragments. A check meant as a dry run (`docs log merge 018 --dry-run`; the flag is `--dry`) ran the real merge, so both fragments were folded into `agent-log.md` and deleted by hand, not by the driver. The result is the same file the driver's merge would write; on resume the driver finds no fragment (`Fold::Nothing`, no `log_merged` event for CP-2) and its checkpoint commit takes `agent-log.md` and the two deletions with the feature folder. The owner answered BLOCK-2 in the cockpit at 17:21; gate attempt 3 started at once.

Gate cost of the format slip: two extra full-gate runs, about 14 minutes of wall-clock, and two owner interrupts. Run A paid none.

**The cockpit found the leftovers itself (17:21).** After BLOCK-2 it checked `git ls-files` and `git status`, named the 11 uncommitted files, and asked the owner whether to commit them now or after the run. The owner's earlier call stands: leave them to the run.

**The cockpit committed the leftovers (17:23).** The owner told the cockpit "Proceed as you see fit", overriding the earlier call. Between slices, it committed the 11 files by pathspec as `37c30c0 fix(018): add files slice 02 left uncommitted`. This is owner-approved repair by the cockpit, and its tokens count as cockpit tokens.

**Slice 06 left two edited files out (17:46).** `Inbox.php` and `InboxController.php` were tracked by then, and slice 06's worker changed them. The driver still skipped them: `scheduler::unnamed` drops any path an earlier `untracked_leftovers` event listed (`listed_before`), even after the path is tracked. Once a path is listed, the driver never commits it again. The cockpit found the gap and committed `6d0f223 fix(018): add Inbox::forRequest slice 06 left out`.

**BLOCK-3, the same format slip in `log-slice-05.md` and `log-slice-06.md` (17:53).** Four of six workers wrote the `# Log fragment — slice NN` title. The cockpit answered by reusing the BLOCK-2 text and edited nothing, so the run would have looped again. At 17:55 the A/B operator removed both title lines and checked with `docs log merge 018 --dry`. That check merged nothing, so the driver's own merge folds both fragments after gate attempt 2.

**PR #25 opened at 19:07, draft, with polish's fixes half committed.** Polish ran 58 minutes and committed `ccad523 polish: review fixes` (10 files). Its changes to four more files stayed in the tree, uncommitted and not listed anywhere: `Inbox.php`, `InboxController.php`, `InboxSeverity.php` and `InboxContractTest.php`, 50 lines added and 17 removed. All four are on slice 02's old leftover list, so `listed_before` drops them silently; no `closeout_leftovers` event names them. The PR body still claims both fixes: "Rows on an inactive company dropped from the inbox, badge and picker (AD-9901)" and "Each Core source is now built inside its own `try`". Neither fix, nor its regression tests, is on the branch. The pushed code is self-consistent (`openCompanies` is unused at HEAD), so CI can go green on a branch that lacks two review fixes its PR claims.

Factory finding, major: the leftover rule turns one missed commit into a permanent blind spot. Every later worker's changes to those paths are dropped, with no record.

**Run B finished at 19:14 (3h 08m), CI green on the first run (7m53s), PR #25 still a draft.** That green covers the branch without the four polish files. The cockpit found the gap on its own after `run_finished` and, at 19:15, was testing the four files and committing them before the decision review.


**Run B's questionnaire: pass.** Yes on 1 and 2, nothing on 6 and 7, 5 on question 4, "ship as is" on 8. Run B matches or beats run A on every question: 0 decisions to overturn against 2–3, and "ship as is" against "ship after minor fixes". One caveat stays with the grade: the shipped branch is what the cockpit's three repair commits made it (`37c30c0`, `6d0f223`, `615fda5`), not what the driver committed alone.

### Slice-13 ranged-read check (AD-1304), run B

From the six build-worker streams in `runs/018-20261011-6490/workers/`, mapped by `slice_spawned`:

| builder | slice | first write to its slice file |
|---|---|---|
| `85baace8` | 01 | `Edit`, straight from the pack, landed |
| `19e9f0f6` | 02 | a `python3` heredoc in `Bash`, no `Read` or `Edit` |
| `82eb969d` | 03 | `Edit` after one ranged `Read`, landed |
| `45b98923` | 04 | `Edit`, straight from the pack, landed |
| `78bfbab6` | 05 | a `python3` heredoc in `Bash`, no `Read` or `Edit` |
| `1e7c80a9` | 06 | a `python3` heredoc in `Bash`, no `Read` or `Edit` |

**Pass on the guide's terms:** no "file has not been read" error and no loop of retried whole reads in any stream. Three of six builders never tried `Read` or `Edit` on the slice file, so the deny was not what pushed them to Bash. The only deny in those three streams hit builder 02's `cat` of `CompanyDirectoryController.php` and two other files, all in its pack under `# Outline`; it did not retry the whole read. Bash writes are the same habit that left slice 02's code uncommitted.

Where the title line came from: builder 06 read `log-slice-05.md`, which already had `# Log fragment — slice 05`, before writing its own fragment. Builders copy a sibling fragment's shape, so one slip spreads to every later slice.

### Browser check, run B

The same 11 browser files as run A's check, in `wt-base` at B's head `806992c`, give identical results to the base `cde31ac` file by file: the same 5 failures (Fr16, Fr1, Fr42, Fr47, Fr86Fr88) and the same 15 module-dependent skips. Run B broke none of them. The four browser files run B edited (Fr109, Fr32, Fr45, Fr51) pass on both the base and the head. Its two new files pass in full: `Fr125InboxTest` 8 of 8, `Fr130InboxBadgeTest` 4 of 4.

### Numbers, run B

Measured 2026-10-11 after the questionnaire: `factory report 018-20261011-6490 --cockpit 708546d1-….jsonl`, with the cockpit idle.

- **Workers: 6,078,688 weighted** (10 workers: 6 build, 3 checkpoint, 1 polish). **Cockpit: 619,229.** **Run B total: 6,697,917**, 41.5% of run A's 16,149,327. **Pass**, 2.99M under the 9,689,596 bar.
- **Cockpit share: fail.** 619,229 is 10.2% of the workers' `weighted_tokens` the report prints, or 9.2% of the run's total; the bar is 5%. It is still 3.5 times smaller than run A's orchestrator (2,178,976). The cockpit did more than relay: three BLOCK relays, finding the leftovers twice, three repair commits, the decision review and the CI watch, most of it caused by the driver's commit bug.
- Gates: 6 driver gate runs, 0 red, 42.7 min; 11 worker inner-loop runs, 0 red. Polish 58.0 min. 3h 08m wall to `run_finished`.
- CI: green on the first run, but on a branch missing polish's fixes. The cockpit's repair push started a second run, still pending at measurement. `ci_cycles: 0`, since the report counts only `ci_fix_committed`.
- Lookups: `ctx_share` 0.073 (under the 10% threshold), `repeat_rate` 0.003, `followup_reads` 7 of 21 (33%, over the 30% threshold).
- Cockpit context: started at 49,700, peak 101,336.

**FR-8 cross-check: pass on what the report exposes.** `measure.py --retro` on the cockpit session gives `out_tokens` 17,844, `cache_read_tokens` 4,553,507, `start_context` 49,700, `peak_context` 101,336, cache write 74,504. Weighted, that is 619,075 against the report's 619,229 (−0.025%; `measure.py` does not count input tokens). Peak context matches exactly. Gap: `factory report` prints only `cockpit_tokens` and `cockpit_peak_context`, so the cockpit's own output, cache-read and start-context figures cannot be compared one by one as the guide asks.
