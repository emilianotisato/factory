# Retro — PRD-030

Written by `/deliver` in the closeout commit, from the agent's side of the run.

**The point is cost next to catch.** An agent asked only about friction votes to cut every check, because every check costs it time. A verdict with an empty Caught column is a preference, not evidence.

## Cost vs catch

| Step | Cost | Caught | Verdict |
|---|---|---|---|
| Inner loop, per slice | 5 slices × ~6 inner runs × ~5 min | stub/plan mismatches, missing `SentBack` section, cockpit embed link, live `waiting` regression | keep |
| Checkpoints (full gate) | 1 run, 0 red, ~2 min | nothing (leak comment cleanup already green) | keep |
| Prove, at checkpoints | driven at CP-1 | mutation on CI `waiting` assert | tune |
| Polish — local review | triage over 6 second-opinion passes + fixes | `>& file` redirect, redact `answer` arrays, git 2.40 `--source`, test holes | tune |
| Polish — security pass | skipped — CLI/backend only, no auth surface change beyond existing redact | — | keep |
| Polish — UX pass | skipped — no user-visible UI diff beyond authorized cockpit prop; API-only status/live | — | keep |
| Polish — second opinion (claude:opus) | 6 passes, ~18 min (pre-collected) | same mediums as local verify; several test-only gaps | keep |
| prune | skipped — gate still green after fixes; no redundant tests removed | — | tune |

**A "cut" or "tune" names what the step failed to catch.** Prove mutation caught live CI state but not FIFO hang regressions until polish. Second opinion duplicated local signal on redact/shell but was the only pass that tied grep allow tests to non-recursive grep.

## Where I guessed

- **`src/review.rs` git 2.40 fallback:** PRD verified `--source` on 2.56 only; on older git I fall back to working-tree `check-attr`, losing branch anti-tampering on those hosts rather than hard-failing every `leaks` run.
- **`tests/status_and_live_hardening.rs` rev stability:** Checkpoint events in the cap fixture did not surface in `evidence` the way the second-opinion fix assumed; kept the original rev check plus a 16-hex `rev` length pin instead of a stronger checkpoint-evidence assertion.

## What I wished existed

- A **machine-check** that integration tests calling `factory ctx` use `Fixture::command`, like the harness lint for git env.
- **Pre-collected local review** in the polish pack (today only second opinion + leaks are collected).
- **Stub mode** to leave stale checkpoint evidence in place so send-back rotation is provable without fighting the Claude stub.

## Project friction → `/gate audit`

- No new tier needed; polish caught issues the suite already owns. The gap was **corpus tests that never exercised the hook** (`grep` without `-r`) — worth an arch test on “denied grep cases must include `-r`/`rg`” only if that repeats.

## Framework friction → factory

- **`/polish` §1 Collect:** second opinion claims index is excellent but dense; a one-line “already fixed at checkpoint” flag per file would cut re-read time on polish workers.
- **`factory docs idea add`:** required `--body-file`; stdin works but is easy to forget unattended.

**Leaks should have caught:** nothing new in added comments after CP-1 cleanup (verdict was clean).

## Score

4/5 — strong checkpoint catch and second opinion, but review duplicated work and two integration tests still weak on end-to-end stale-evidence and cap/rev evidence.