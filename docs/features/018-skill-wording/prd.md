---
number: 018
slug: skill-wording
title: "Skill wording, contradictions and Prove guidance"
# type: feature | bugfix | enhancement | refactor
type: feature
created: 2026-11-05
author: Emiliano Tisato
summary: |
  The skills stop contradicting each other and stop leaving runs to guess: ten one-line fixes, autonomy rules for research notes and runbooks, Prove guidance that checks the spec and proves the negative, a prune that can consolidate cited tests, /slice linking slices that share a file, and skill-text tests that fail when their passage goes. Draft from the 2026-11-05 idea triage, not interviewed.
---

# PRD-018 — Skill wording, contradictions and Prove guidance

> **Draft from the 2026-11-05 idea triage.** The owner chose what is in and out, group by group; nothing here is interviewed or verified yet. `/prd` continues this draft: interview, recon, FRs, QA plan. `file:line` facts are from `main` at `ca128f7`; re-verify them.

## 1. Why now

Each item is a guess an unattended run logged, or a defect that passed a green checkpoint: PRD-005's polish found 4 highs that four green checkpoints had passed.

## 2. Candidate FRs

Kept by the owner at the triage. `/prd` turns each into an FR or cuts it.

- **C-1 — The one-liners.** Five contradictions: 1 the Notes cell's "PR number" (`skills/polish/SKILL.md:201`), 2 `{date}` local or UTC, 3 reversing a proven decision, 4 prune's precondition vs polish's own docs edits, 5 `{runs}` undefined (`assets/pr-body.md:43`). Wording gaps: 2 the polish UX row on an API-only change (`skills/polish/SKILL.md:45,59`), 3 polish §8 names frontmatter, not the verb (`:155`, shared with *CLI, docs and log correctness* C-1).
- **C-2 — What an unattended run may write:** research notes, runbooks and operations docs, sign-off closing, a later fix to a file the PRD ordered (`assets/autonomy.md:45-49`).
- **C-3 — Prove guidance.** The expected result comes from the spec's rule, not the code's output (`skills/slice/SKILL.md:58,143`); each Prove names how it fails without the feature, and one hostile case per FR (`:85`, `assets/contract-checkpoint.md:15`).
- **C-4 — Prune vs cited test names** (`skills/prune/SKILL.md:28`, `skills/polish/SKILL.md:144`), and pr mode on a whole-suite branch reuses polish's gate run (`skills/prune/SKILL.md:73-76`).
- **C-5 — `/slice` marks two slices dependent** when their Current code names the same file (`skills/slice/SKILL.md:53`).
- **C-6 — A deferred harness repair has an owner:** the slice that broke the drive, or the checkpoint itself.
- **C-7 — `assert_unique_passage`** for skill-text tests (only an ad-hoc `count()==1` today, `tests/init.rs:263`), used by every skill-text test; `/slice` asks for it.

**Build note:** every item edits `skills/`, so one version bump. Build after *CLI, docs and log correctness* C-3 (relock), or polish fixes on this branch trip the lock.

## 3. Not in scope (decided 2026-11-05)

- **Wording gap 5** (a PRD dictating a plan-id comment) — done in PRD-006.
- **friction-arca's bash-hook bullet** — done in PRD-009. Its docs-log bullet is in *CLI, docs and log correctness* C-1.

## 4. Source ideas

The full text of each idea this draft absorbed; the idea files are deleted.

### Five small contradictions between factory skills, found in one unattended run

_added 2026-10-29, origin: factory upstream_

One `/deliver` run hit five places where two skills disagree or one is silent. Each cost a guess, now recorded in that run's log. Each is a one-line fix.

1. **The index Notes cell.** `deliver` §4 says the Notes cell must not carry the PR number, because the PR does not exist yet. `prd-to-tasks` §9 says the Notes cell holds the PR number. `deliver`'s reasoning holds (adding it later costs a CI run), so `prd-to-tasks` should change.
2. **`{date}` names no clock.** The autonomy doc, the agent-log template and `deliver` all write `{date}` without saying local or UTC. A checkpoint subagent stamped UTC while its orchestrator wrote the local date, so one heading in the folder is a day off from the rest. Say "the machine's local date" (or UTC) once.
3. **`polish` §4 is silent on reversing a proven decision.** The mapping says an Ask becomes an AD when "local and reversible", and a BLOCK otherwise. An Ask that supersedes an AD a checkpoint already proved is reversible but not small. Proposal: an AD marked `decide` when its own proof lands in the same pass, a BLOCK when it would need the checkpoint re-driven.
4. **The `test-cleanup` precondition under `/deliver`.** "Working tree clean, or dirty only with the changes under review": the tree holds polish's own uncommitted docs edits at that point. The order (cleanup before the `reviewed:` commit) is deliberate, so the precondition should say polish's own docs edits do not count as unrelated changes.
5. **`pr-body` "Gate: {runs} runs".** Full-gate runs only, or inner loops too? Define `{runs}` as full-gate runs (matching the retro's `gate_runs`) and give inner loops their own clause.

### Five skill wording gaps seen across four unattended runs

_added 2026-10-31, origin: factory upstream_

Four consecutive feature runs in one repo hit the same small gaps in the skills' wording. Each cost a guess the worker had to log, and none needs new machinery: a sentence in the skill would have settled it.

1. **`prune` § PR Mode on a branch that adds the whole suite.** PR mode scopes to "tests added or modified on this branch". On a first feature, that is every test, so PR mode is really full mode without the baseline coverage step. Separately, step 3 ("run the scoped test files 3 times") runs inside `polish` right after a full gate run; when the scoped files are most of a database-backed suite, that repeats a gate's worth of time for little signal. Proposal: say that a branch adding most of the suite runs the full-mode baseline, and let polish pass its fresh gate result in place of the repeat runs.

2. **`polish` § 1, the UX row, for an API-only change.** "Anything user-visible gets the UX pass" does not say what to do when the only screen is a third-party API-docs renderer and the harness is curl-only. Workers skipped it and explained why each time. Proposal: name that case (skip, and point at the QA plan's Human row if there is one).

3. **`polish` § 8 says to set `reviewed:` in the PRD frontmatter**, while the worker contract says `factory docs set NNN reviewed=…`. The worker followed the contract. Proposal: the skill names the verb.

4. **Autonomy, "What an unattended run may write".** "The PRD outranks this list" covers creating a file the PRD names, such as a CI workflow. It does not say whether a later polish fix to that same file is covered. A worker found a real defect in a PRD-ordered workflow and had to decide alone whether it could repair it. Proposal: one sentence either way. A related gap: an operations runbook under `docs/reference` that contradicted the deploy workflow was on neither the May nor the Never list.

5. **A PRD that dictates a code comment the leaks check rejects.** A PRD prescribed a lint-suppression comment that cited a plan id, such as `//nolint:<rule> // <reason> (D-00N §1)`. The leaks check then rejected the plan id at the first checkpoint, and the builder had to strip it. Proposal: `/prd` flags a prescribed code comment that carries a plan id, so the PRD is leaks-clean before anything is built.

### autonomy.md: say whether an unattended run may write research notes and operations docs

_added 2026-11-05, origin: factory upstream_

`autonomy.md` § "What an unattended run may write" leaves two common cases unnamed, and runs guessed:

- **Research notes the run added itself.** A run misplaced a paragraph in a `docs/research/` note on its own branch and repaired it, unsure whether that was allowed.
- **Operational procedure docs the feature makes stale** (a project's `docs/operations/` or similar: runbooks, compliance procedures someone signs). Reviewers found dead links and built work described as pending. The run fixed the text but left the open checklist rows alone, because closing one changes what a human signs.

Should: name both explicitly, one way or the other. A reasonable default: a run may repair what it wrote on its own branch, and may update project docs its FRs make factually stale, but never closes a human sign-off item.

### Prove guidance: check against the source's rule, and prove the negative

_added 2026-11-05, origin: factory upstream_

Two Prove-task weaknesses let defects through green checkpoints:

- **Expected output taken from the command itself.** A checkpoint's Prove row recorded a command's printed summary as the expected result. The input specification said, in its own words, which documents that report must contain; the output held a different set. All three checkpoints passed, and only polish caught it.
- **Specs that pass without the feature.** Two browser specs for a "submit once" behaviour would have passed with the feature's script removed: nothing held the request long enough for a second click to matter. No checkpoint asked whether the spec could fail.

Should: `/slice`'s Prove guidance (and the Prove task template) says (1) state the expected result from the spec, PRD or input file, never from what the command printed, and (2) for each behavioural spec, name how it would fail if the feature were removed, and check that at least once.

### prune runs after checkpoints have cited test names, so its own rule blocks every consolidation

_added 2026-10-29, origin: factory upstream_

`prune` (via `polish` §7) runs at the end of a feature. By then the checkpoints have recorded test names in their evidence and in the PRD. `prune`'s Preconditions §5 says to prefer keeping names that evidence cites. So consolidating any test the branch added renames something cited, and the skill's own rule argues against it. In one run, two same-setup pairs looked mergeable and stayed separate for exactly this reason.

Two ways out, either one works:
- Run `prune` per checkpoint, scoped to the tests written since the previous one, before their names are cited.
- Say that a citation only in a frozen checkpoint transcript does not bind, so `prune` may rename freely and leave the transcript as history.

Related: scope the end-of-feature `prune` to tests written since the last checkpoint, since earlier ones were already pruned.

### Skill-text tests: assert the quoted passage is unique

_added 2026-11-05, origin: harvest 2026-11-05_

From PRD-012's retro. Skill-text tests (`tests/skills_bundle.rs` and friends) assert that a skill file contains a needle. Three of them passed after the rule they guard was deleted, because another line in the same file held the same words. For prose FRs these tests are the gate's only proof, and they were the weakest tests in the diff.

Shape: a helper such as `assert_unique_passage(file, needle)` that fails when the needle occurs zero times or more than once, used by every skill-text test. `/slice` asks each skill-text test to quote the passage, not a word it shares with others. Acceptance: deleting the guarded sentence turns its test red.

### friction: arca-api PRD-025

_added 2026-10-15, origin: arca-api PRD-025 run 025-20261015-a235_

- **`polish` § 1, second opinion.** Two PRDs running, `factory review second` timed out on grok-4.7-high at 900 s on a pass of about 535 lines. Ten passes would take 2.5 hours. The pass prompt needs a time and tool-call budget, and passes should run in parallel.
- **`slice`, lane independence.** Slice 02 was marked independent of 01 while both rewrote the same four test files and the stub. Four blocks followed. The slicer should mark two slices dependent when their Current code sections name the same file.
- **`deliver`, deferred harness repair.** A checkpoint may defer a harness repair "to polish", but `/polish` has no step that receives it. Either the checkpoint repairs it, or the slice that broke the drive owns it.
- **`factory docs log`, lanes.** In a lane, `log add` writes the main checkout's log and `log show` reads the lane's, so a worker cannot read back what it just logged.
- **The bash hook** refuses any command containing `.push(` as if it were `git push`, so an inline script editing JavaScript had to go through a file. Match `git push` as a command, not a substring.

A comment or name the leaks check should have caught: none found; leaks was clean, and the local review read every added comment.

From ~/Projects/Prokey/arca-api/docs/features/025-manage-caea-pairing/retro.md

### Also from

checkpoint-covers-enforcement, its "Prove runs the happy path only" bullet (C-3): full text in *Checkpoints that actually prove*.
