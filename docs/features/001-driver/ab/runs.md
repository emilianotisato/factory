# FR-11 A/B runs

## icg-cloud: PRD-018 cross-client inbox

Base `cde31ac` on `docs/prd-018` (main `4f93556` plus the PRD, its 6 slices and the approval). PR #23 (PRD-017) left open and unmerged. Worktrees made by `worktrees.sh`. Base gate green in `wt-a`: 2,378 tests, 424 s.

Both runs: Opus 5.5, effort high, bypass permissions, `autoCompactWindow` 200000. Factory workers use the same, from `config.rs` defaults. Preflight ask (6 slices, `lanes: 1`) answered "proceed, serial" for both, by the owner, before launch.

| run | tool | worktree / branch | started | session | PR |
|---|---|---|---|---|---|
| A | `/deliver 018` | `wt-a` · `feat/018-cross-client-dashboard` | 2026-10-11 12:09 | session_01WGNvo9koY6fY8J21kJQjZY, local `480153fa-444d-439a-936f-8dfbccc268ac` (tmux `ab-a`) | |
| B | `/factory-deliver 018` | `wt-b` · `feat/cross-client-dashboard` | | | |

### Owner answers

Every answer one run gets, the other gets word for word, so the runs differ only in the tool.

**BLOCK-101 (run A, slice-01, 12:24).** A gap in the shared base, so run B should hit it too. PRD §9 seeds the inbox fixtures in `QaSeeder::run()`, and 32 Sync feature tests count whole tables after seeding. The owner chose the builder's option 1, sent at 12:25:

> BLOCK-101: option 1. Scope the whole-table SyncRun and StagedRecord counts in the failing Sync tests to the profile under test, and exclude the seeded inbox run where a test asserts a list. Fix the browser-test assertions that break the same way (AD-101). §9 stays as written: the inbox fixtures stay in QaSeeder::run(). Record this as the resolution and resume the run.

Run B gets the same text under its own block id, with "option 1" dropped if its options differ.
