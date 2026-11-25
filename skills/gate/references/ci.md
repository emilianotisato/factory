# Cloud CI — what it proves, and what it costs

Cloud CI minutes are metered on private repos, and an agent-driven repo pushes far more often than a human one. Design CI the way the gate designs tiers: each job earns its minutes by proving something the local gate did not.

## The design rule

Ask **what cloud CI proves that the local gate did not.** Two things, usually:

- **Clean-environment reproducibility**: the gate passes on a fresh machine from a fresh checkout, with no stray build output, local config or tool version hiding a failure.
- **The merge check**: a required status check on the default branch, so nothing merges unproven.

Everything the local gate already proved on the same commit is paid for twice. A fast gate (under about two minutes) can run the same command in both places, because drift costs more than the minutes. A slow or metered one runs the full gate locally and a fast clean-room core in CI.

## Draft-silent PR CI

`factory` opens every pull request as a draft and pushes to it several times. Keep CI silent until the owner marks it ready for review:

```yaml
on:
  pull_request:
    types: [opened, reopened, synchronize, ready_for_review]

jobs:
  gate:
    if: github.event.pull_request.draft == false
```

A draft burns 0 cloud minutes; a multi-push PR costs one check, at merge time. Marking the PR ready for review (`ready_for_review`) is what triggers it, unless the head commit says `[skip ci]`, as a `factory` run in local CI mode leaves it: GitHub then skips every `pull_request` run, `ready_for_review` included, and one new commit without the token starts the check.

## Split-tier CI

Heavy tiers stay in the local gate, or in an optional nightly schedule: mutation testing, multi-shard browser suites, complexity/CRAP. PR CI runs the fast clean-room core: dependency audit, format, lint, static analysis, and the tests. Name the split in the project's verification doc, so nobody later "fixes" it by accident.

## CI cost audit checklist

Read every file in `.github/workflows/*.yml` and check each item. Report each leak with its estimated cost: minutes per run times runs per week.

1. **Draft PRs running the full suite.** No `types:` filter or no `draft == false` condition, so every push to a draft pays for a full run nobody reads.
2. **Duplicate `push: [main]` runs after a PR-gated merge.** When branch protection requires the branch to be up to date before merging, the PR's check already ran on the tree that lands; a second full run on `main` proves it again. The same leak hides in deploy workflows that chain `workflow_run` on a redundant full verify instead of triggering on `push: [main]` themselves.
3. **No concurrency group.** Without `concurrency: { group: ..., cancel-in-progress: true }`, a superseded push keeps running to the end.
4. **No `paths-ignore`.** A docs-only change runs the whole suite; add `paths-ignore: ['docs/**', '**.md']`. Never on the workflow behind a required check: GitHub leaves a required check pending when path filters skip its workflow, so a docs-only PR could never merge. There, keep the workflow running and exit its job early on a docs-only diff.
5. **No dependency or build caches.** Composer, npm/pnpm, Cargo and Go downloads and builds repeated from scratch on every run.
6. **Local parallelism hardcoded on 2-vCPU runners.** A fixed `--processes=10` tuned for a workstation thrashes a standard cloud runner; use `$(nproc)` or `2`.
7. **Coverage extensions loaded for nothing.** `coverage: xdebug` without `--coverage` slows every test and reports nothing.
8. **Heavy local-only tiers or daily cron jobs on metered minutes.** Mutation runs, test-impact (TIA) baselines or a daily schedule that rebuilds them, burning private-repo minutes for a tier the local gate already owns.
