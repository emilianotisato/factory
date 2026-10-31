---
title: "Five skill wording gaps seen across four unattended runs"
state: "open"
added: "2026-10-31"
origin: "factory upstream"
---

Four consecutive feature runs in one repo hit the same small gaps in the skills' wording. Each cost a guess the worker had to log, and none needs new machinery: a sentence in the skill would have settled it.

1. **`prune` § PR Mode on a branch that adds the whole suite.** PR mode scopes to "tests added or modified on this branch". On a first feature, that is every test, so PR mode is really full mode without the baseline coverage step. Separately, step 3 ("run the scoped test files 3 times") runs inside `polish` right after a full gate run; when the scoped files are most of a database-backed suite, that repeats a gate's worth of time for little signal. Proposal: say that a branch adding most of the suite runs the full-mode baseline, and let polish pass its fresh gate result in place of the repeat runs.

2. **`polish` § 1, the UX row, for an API-only change.** "Anything user-visible gets the UX pass" does not say what to do when the only screen is a third-party API-docs renderer and the harness is curl-only. Workers skipped it and explained why each time. Proposal: name that case (skip, and point at the QA plan's Human row if there is one).

3. **`polish` § 8 says to set `reviewed:` in the PRD frontmatter**, while the worker contract says `factory docs set NNN reviewed=…`. The worker followed the contract. Proposal: the skill names the verb.

4. **Autonomy, "What an unattended run may write".** "The PRD outranks this list" covers creating a file the PRD names, such as a CI workflow. It does not say whether a later polish fix to that same file is covered. A worker found a real defect in a PRD-ordered workflow and had to decide alone whether it could repair it. Proposal: one sentence either way. A related gap: an operations runbook under `docs/reference` that contradicted the deploy workflow was on neither the May nor the Never list.

5. **A PRD that dictates a code comment the leaks check rejects.** A PRD prescribed a lint-suppression comment that cited a plan id, such as `//nolint:<rule> // <reason> (D-00N §1)`. The leaks check then rejected the plan id at the first checkpoint, and the builder had to strip it. Proposal: `/prd` flags a prescribed code comment that carries a plan id, so the PRD is leaks-clean before anything is built.
