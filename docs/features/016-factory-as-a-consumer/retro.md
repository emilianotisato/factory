# Retro — PRD-016

Written by `/deliver` in the closeout commit, from the agent's side of the run.

**The point is cost next to catch.** An agent asked only about friction votes to cut every check, because every check costs it time. A verdict with an empty Caught column is a preference, not evidence.

## Cost vs catch

| Step | Cost | Caught | Verdict |
|---|---|---|---|
| Inner loop, per slice | | CP-1 mutation on bare `/home/x`; CP-2 mutation on pull list assertion | keep |
| Checkpoints (full gate) | | nothing at CP-2 after slice-04 | keep |
| Prove, at checkpoints | | | keep |
| Polish — local review | | paths `/users/create` false positive; post hash bypass; empty `commit` before `gh` close | tune |
| Polish — security pass | | unscreened issue body on `pull --idea` (medium, parked) | tune |
| Polish — UX pass | | skipped — CLI-only surface | cut |
| Polish — second opinion (claude:opus) | | edge `sed -i` on macOS; same paths/upstream items as local pass | keep |
| prune | | nothing removed — new assertions are load-bearing | keep |

**A "cut" or "tune" names what the step failed to catch.** UX was correctly skipped. Security pass found real pull-screening gap but polish cannot retrofit full upstream screen without scope; tune means brief it with PRD §QA rows next time. Local review and second opinion overlapped heavily — keep both because cross-vendor agreement on paths/upstream was the signal; tune by feeding high findings into `--others` earlier.

## Where I guessed

- **Autonomy Never vs edge.yml defect:** Second opinion flagged BSD `sed -i` as ship-blocking on macOS matrix legs; worker contract forbids editing `.github/workflows/**`. Parked as `docs/ideas/2026-11-27-edge-yml-bsd-sed-i-breaks-macos-matrix-legs.md` and listed under **Needs your review** instead of blocking the run.
- **`/users/ana` on Linux:** PRD wanted case-insensitive `users` on Unix; polish split case on `Users` vs `users` and used a route-segment denylist rather than reopening the interview.
- **Pull retry after partial `gh` failure:** Accepted ship-with-documented risk — commit-before-close is intentional; duplicate idea on retry is real but needs a heavier idempotency design.

## What I wished existed

- A polish exemption or owner-pre-staged workflow patch queue for **defect fixes** in CI files the PRD itself added (edge.yml), without classifying them as “get green” edits.
- `factory review collect` attaching rebuttal templates when a finding contradicts a PRD acceptance line (flat install prefix, lane folder name variants).
- Commit-failure injection in the upstream `gh` stub so slice-04 “local commit before GitHub” is provable without hooks.

## Project friction → `/gate audit`

- Shared `CARGO_TARGET_DIR` on this machine (`/mnt/devcache/...`) served stale test binaries during checkpoint mutation — documented in the agent log; lane-local `target/lane-2` is the fix, not a gate tier change.
- Gate is long (~2.5 min full `cargo test`) but caught nothing new at polish; keep.

## Framework friction → factory

- **`polish` / Autonomy — Never CI workflows:** Correct for scope creep, but it blocks fixing a high finding in a workflow the same PRD ordered. Name the skill sections: `polish` §0 collect + Autonomy **Never** list. Suggest a narrow **workflow defect fix** allowlist when the file appears in the feature diff and collect marks it high.
- **`leaks` check:** Clean this run; comment share 0.4% — no rule gap for plan-item names in code.

## Score

3/5 — strong slice proof and review depth, but the highest-severity finding (edge macOS publish) could not land in-repo under unattended rules and needs owner action before merge confidence is complete.