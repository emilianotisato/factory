---
name: prune
description: Clean up, consolidate, stabilize, and prune a project's test suite without losing coverage. Use when the user wants to remove scaffolding/stepping-stone tests, deduplicate overlapping tests, fix flaky tests, or rationalize a test suite. Two modes — full (audit the entire suite, for legacy or neglected projects) and pr (only tests added or changed in the current branch/PR).
argument-hint: [full|pr] [base-branch]
---

# Test Suite Cleanup & Pruning

Clean, consolidate, stabilize, and prune tests **without dropping functional coverage or breaking domain confidence**. Application code is off-limits — only test files, helpers, and fixtures may change.

**Inside a `factory` run, the contract in your pack wins.** Nobody answers a question and you never commit: where this skill asks or commits, the contract's rule replaces it. The one rule that is this skill's own: in doubt about a test, keep it and log an `AD-n` naming the doubt. Keeping never loses coverage.

## Mode Selection

Parse `$ARGUMENTS`:

- `pr [base-branch]` → **PR mode**: only classify tests added or modified relative to the base branch.
- `full`, empty, or anything else → **Full mode**: audit the entire suite.

If the user's request says "this PR", "this branch", or "the tests I just wrote", prefer PR mode even without the argument.

## Preconditions (both modes)

1. Must be a git repository. Working tree must be clean, or dirty only with the changes under review — if there are unrelated uncommitted changes, stop and ask the user to commit or stash first. Pruning must be reviewable and revertible. Inside a `factory` run the uncommitted polish fixes are the changes under review.
2. In full mode, create a working branch (e.g. `prune/<yyyy-mm-dd>`) unless already on a non-default branch.
3. Detect the stack and resolve the commands (full run, single file, coverage, and the project's verify gate if one exists — `composer verify`, `make verify`, etc.) — see [references/adapters.md](references/adapters.md). Verify each command works before relying on it.
4. **One test process per checkout, ever.** Do not split this work across agents that each run tests. Every agent on the checkout shares one test database, and often the probe files a suite writes into the tree. Two `RefreshDatabase` runs wiped each other's schema mid-run in a real cleanup. Parallelise the reading and classifying, then run tests one at a time. **Never create a scratch database** to get around the shared one; a database you cannot share is a blocker, not a workaround.
5. **Evidence and Progress lines name test methods.** Checkpoint evidence in the run dir (`$FACTORY_EVIDENCE` in a run, `runs/<id>/evidence/` under factory's data dir after it) or the harness artifacts, and the Progress lines of a slice, name the methods that proved it at the time. A consolidation that renames one leaves those as they are: do not edit a transcript, and do not rewrite a Progress line. Prefer keeping the name. When a rename is still right, say the old and new names in the commit body or your report. One run renamed a method a slice's Progress named and had to put it back.

## Full Mode Workflow

### Phase 1 — Baseline audit

1. Run the full suite once. Record: total test count, pass/fail counts, wall-clock time.
   - If the suite is already red, report the failures and ask whether to proceed — cleanup on a red suite can mask real regressions.
2. Run the coverage command and save baseline line/branch percentages. If no coverage tooling exists, note it and skip coverage deltas (rely harder on the classification rules).
3. **Flakiness check:** run the suite once in shuffled/random order (`--order-by random`, `-shuffle=on`, `-p randomly` — see adapters.md). Order-coupling is the main flake source and a single shuffled run exposes it cheaply. Then re-run any suspicious tests (shuffle failures, timing-dependent tests, tests hitting network/clock/filesystem) 3 times in isolation. A test that both passes and fails → tag FLAKY. Note in the report that low-frequency flakes can escape this check.

### Phase 2 — Classification

Read [references/rules.md](references/rules.md), then build an inventory:

| File | Test | Classification | Rationale |
|------|------|----------------|-----------|

Classifications: **KEEP**, **CONSOLIDATE**, **STABILIZE**, **PRUNE**.

No approval pause — proceed directly to execution; the user reviews the resulting diff and commits. Two obligations replace the pause:

1. **Every PRUNE gets a written rationale** naming the covering test(s) that make it redundant. These rationales go in commit messages and the final report.
2. **If genuinely in doubt** about a specific test (can't find a clear covering test, business logic looks load-bearing but unfamiliar), ask the user in-session about that test — don't guess, and don't silently KEEP everything to avoid asking. With nobody to ask, keep it and log the doubt, as the top of this file says.

### Phase 3 — Execution

Work one module or feature area at a time:

1. Apply consolidations (parameterized / table-driven tests).
2. Stabilize FLAKY tests (Rule D in rules.md).
3. Delete PRUNE-tagged tests.
4. After editing each file, run that file's tests immediately. Green before moving to the next file.
5. **Commit each logical unit** (one module/feature area) once its file-level runs are green. Plain `git commit` under the user's normal git identity. Message format: `test: <what> (<module>)` subject, body listing what was pruned/consolidated and why — commit history doubles as the audit trail, and a bad batch stays individually revertible. Inside a `factory` run the driver commits everything at once, so the per-unit rationale goes in your report instead.

### Phase 4 — Safety verification

1. **Project verify gate first:** if the project defines one (`composer verify`, `make verify`, `npm run verify`, `just verify` — see adapters.md), run it and require a full pass. These gates typically bundle tests, static analysis, and mutation testing — mutation testing is the only real detector of assertion loss, so a passing gate is the strongest signal available. Fix any regressions the gate reveals before proceeding.
2. If no verify gate exists: full suite run with **0 failures**.
3. Coverage within 1–2% of baseline (or better). A larger drop means a high-value test was mistakenly pruned — restore it and reclassify. Note the limits honestly: deleting a redundant test moves coverage 0%, and coverage never detects weakened assertions — the real safety review is the per-prune rationale (and mutation testing, where the verify gate includes it).
4. Runtime vs baseline: report it, but it's informational — consolidation can slightly increase per-case overhead and that's acceptable.
5. `git log --stat` audit across the cleanup commits: only test files, helpers, and fixtures changed.

## PR Mode Workflow

1. Resolve the base ref: use the `[base-branch]` argument if given, else `git merge-base HEAD origin/<default-branch>`.
2. Scope: `git diff --name-only <base>...HEAD` filtered to test files. From the diff, extract the individual test functions that were added or modified — pre-existing tests in touched files are out of scope.
3. Baseline: run the scoped test files 3 times (cheap at this scope) to catch flakiness; record count and timing.
4. Classify only the scoped tests using rules.md — **but check redundancy against the entire existing suite**: a new test that duplicates a pre-existing test is PRUNE.
5. Execute changes with the same per-file run and per-logical-unit commit discipline as full mode.
6. Verify: run the project verify gate if one exists, otherwise the **full** suite once (scoped runs miss cross-test coupling). If failures appear, check whether they pre-exist on the base ref (`git stash` / checkout base and re-run the failing tests): pre-existing failures are out of scope — report them, don't fix them; failures introduced by the cleanup must be fixed.
7. No approval pause — same rules as full mode: rationale per prune, ask in-session when in doubt, or keep and log when nobody answers.

## Final Report

Always end with:

- **Tests before → after:** X → Y
- **Verify gate:** command run and result (or "none defined — plain suite run")
- **Flaky fixed:** Z (list them)
- **Runtime:** A s → B s (informational)
- **Coverage:** C% → D% (line / branch), or "no coverage tooling"
- **Pruned:** one line per test — file, test name, and the rationale naming the covering test(s)
- **Consolidated:** N tests → M parameterized tests, per file
- **Commits:** list of cleanup commits (subject lines)
