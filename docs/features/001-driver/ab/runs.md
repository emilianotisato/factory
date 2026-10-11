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
| 1 | Does the result do what the PRD asked? | yes | |
| 2 | Merge after normal review, at most small fixes? | yes | |
| 3 | Decisions you would overturn | 2–3 | |
| 4 | Code quality against the project's standard (1–5) | 5 | |
| 5 | Tests prove the FRs, or only pass? | prove | |
| 6 | Built but not asked for | none | |
| 7 | Asked for but missing | none | |
| 8 | Overall | ship after minor fixes | |

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
