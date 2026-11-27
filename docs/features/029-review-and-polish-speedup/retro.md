# Retro — PRD-029

Written by `/deliver` in the closeout commit, from the agent's side of the run.

**The point is cost next to catch.** An agent asked only about friction votes to cut every check, because every check costs it time. A verdict with an empty Caught column is a preference, not evidence.

## Cost vs catch

| Step | Cost | Caught | Verdict |
|---|---|---|---|
| Inner loop, per slice | four slice verify runs, each one green on first try | nothing | keep |
| Checkpoints (full gate) | four checkpoint gates, each one green | nothing (mutation at CP-1/2/4 proved skip logic) | keep |
| Prove, at checkpoints | manual evidence paths; BLOCK-1 once for missing FR-3 disk proof | dropped prove rows twice before block | tune — prove contract and pack must name tests that do not exist yet |
| Polish — local review | triage over pre-collected `collect.md` plus targeted reads | claims-index pipe split, preflight ask gap, classifier false positives | keep — driver collect amortizes cost |
| Polish — security pass | one subagent on `main...HEAD` | nothing | keep — cheap for CLI/signals diff |
| Polish — UX pass | skipped | n/a | cut on this feature — no GUI; correct skip |
| Polish — second opinion (claude:opus) | five passes, all counted | same defects local triage would have missed without cross-pass merge | keep |
| prune | skipped after green polish gate | n/a | keep — few new tests, fixes added regression assertions inline |

**A "cut" or "tune" names what the step failed to catch.** Checkpoint prove rows that reference slice-04 tests before slice-04 lands are a template bug, not a worker bug; tune the checkpoint pack to defer FR rows.

## Where I guessed

- **`marker_matches` for `"login"`** — PRD §5 lists bare `"login"` as an auth marker; polish narrowed it to phrases because cross-vendor review showed trace false positives. Kept `"not logged in"` and siblings as-is.
- **Whether docs-only planning branches should review their own PRD when absorbing ideas** — §3 FR-1 says drop `feature-doc` whenever any non-idea path changes; idea deletions count. Treated second-opinion concern as re-litigation, not fix.

## What I wished existed

- A single `factory review triage` output that merges Claims index, leaks, and local lint into one machine-readable file so polish does not re-walk five pass sections.
- `factory docs set reviewed=` optionally accepting `reviewed_head=<sha>` in one call for attended polish rounds.
- Shared-lane `CARGO_TARGET_DIR` guard in the gate that rebuilds `factory` before integration tests that shell out.

## Project friction → `/gate audit`

- Parallel lanes sharing a global `CARGO_TARGET_DIR` made integration tests flaky against a stale `factory` binary; the feature log Note caught it, but the default gate does not enforce rebuild-before-shell-out tests.

## Framework friction → factory

- **`/polish` §1** — when `# Collected` is present, the skill still says “local review first” but the worker pack already ran collect + second opinion; clarify that triage may start from Claims index without a second full local subagent when collect is fresh.
- **`/deliver` checkpoint Prove rows** — naming FR acceptance tests before the slice that adds them caused BLOCK-1; checkpoint templates should mark rows as `deferred until slice-NN`.
- **Leaks check** — comment-share was clean; no miss. (No `file:line` leak this run.)

## Score

4/5 — Feature delivered with strong test surface; polish caught real product gaps in parsing and settled-block shape. Lost a point to checkpoint/evidence friction and attended `reviewed_head` semantics left for a follow-up idea.