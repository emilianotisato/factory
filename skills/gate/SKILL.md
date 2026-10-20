---
name: gate
description: Interview a project into one verify gate — a single command bundling tiered quality checks (deps, lint, types, coverage floors, complexity, mutation, size budgets, arch rules), measured against reality, proven to fail, then declared in docs/project.toml so every slice's Verify task, every checkpoint and every polish pass runs it. Any stack — PHP/Laravel, Node/TS, Go, Kotlin, Python, Rust. Use when `factory docs gate` reports none declared, when a project has tests but nothing enforcing their quality, or when the user says "gate", "guardrails", "set up the gate", "harden this repo", "add coverage/mutation/CI enforcement", "make verify". `gate audit` reviews an existing gate instead of building one — stale floors, docs disagreeing with the code, decorative tiers, undocumented CI divergence, what the last runs' numbers say — and changes nothing.
argument-hint: [audit]
---

# Gate — one command, measured, proven to fail

A project's gate is the single command that proves a change is done. This builds it: a tiered ladder of checks, each proving something the previous cannot, ordered cheapest-first so a failure costs seconds instead of minutes, wrapped in **one command** and declared once in `docs/project.toml` so `/slice`, `/polish`, `prune` and the `factory` run all read it from the same place.

**Two rules outrank everything else here.**

**Measure before you propose.** Every floor comes from a number you ran in this repo today. A coverage floor of 80 in a repo sitting at 61 is a gate that is red on day one, which teaches everyone to bypass it. Aspirational numbers are how gates die.

**Floors ratchet up, never down.** When actual rises, bump the floor to just under it. When a change would drop below the floor, the answer is cover it or refactor it, never lower the floor. Write that rule into the project's own docs, because it is the one an agent will otherwise quietly break to get green.

**Attended work.** This is an interview. A `factory` run refuses to start without a gate (`factory docs preflight`) and never edits one. A retro row that calls a tier too strict, or a test that reddens inner loops now and then, is evidence for `/gate audit`, never a reason to change the gate mid-run.

`$ARGUMENTS` = `audit` → the project already has a gate; skip to §5 and report what is missing from the ladder, with the cost of adding each. No changes without approval.

## 0. Preflight

```
factory docs gate --candidates
```

- **A gate is already declared and resolves** → this is an `audit`, not a build. Go to §5.
- **Candidates found but nothing declared** → a gate may already exist unnamed. Read what those scripts actually do before proposing anything: a `composer check` that runs Pint alone is a lint step, not a gate. Say which you found.
- **Nothing** → full build, below.
- **Not on the framework** (`factory docs doctor` says so) → `factory docs init` first, one command. The gate lives in `docs/project.toml`, and `init` writes that file.

Then detect the stacks. A repo is often two or three: a PHP backend with a TS frontend, a Go service with a Kotlin app. **Every stack gets its own tiers; the wrapper is what makes them one command.** [references/stacks.md](references/stacks.md) has the detection markers and the exact tool commands per stack.

## 1. Read what already exists — never propose what is already there

- **Existing scripts**: `composer.json`, `package.json`, `Makefile`, `justfile`, `mise.toml`. What runs today, and what each actually covers.
- **CI**: `.github/workflows/*.yml`, `.gitlab-ci.yml`. **Which checks CI runs is a fact you need**, because §4's fork is whether the gate and CI stay in sync or deliberately diverge.
- **Existing config**: `phpstan.neon`, `eslint.config.js`, `.golangci.yml`, `detekt.yml`, `pyproject.toml`, `phpunit.xml` testsuites, coverage settings.
- **The test suite's actual shape**: how many tests, how long it takes, whether it is green right now. **A red suite stops this cold** — say so and fix or report it first. Building a gate on a red suite bakes in a bypass.
- **The project's own rules**: `CLAUDE.md`, `AGENTS.md`, `.ai/guidelines/`, `docs/decisions.md`. A decision there about testing already answers some of what you are about to ask.

## 2. Measure the baseline — run the tools, record the numbers

**This is the step that makes the interview honest.** Before proposing a single threshold, run each candidate tier once and record what it says. Install what you need to measure with (§3's posture applies: push, take one refusal).

Record, per stack, whichever apply:

| What | Command | Why it matters |
|---|---|---|
| Line coverage | see stacks.md | sets the coverage floor |
| Type coverage | see stacks.md | sets the type floor |
| Static analysis violations | at the current level | decides whether a level bump is in scope |
| Complexity-vs-coverage offenders | CRAP > 30 and > 15 | a fail list you must clear *before* the tier can be green |
| Bundle / binary size | the built artifact, gzipped | sets the size budget |
| Suite wall-clock | alone, then two copies at once | decides tier order, the inner loop, and `gate_parallel` |
| Mutation score, on a sample | one module, not the repo | a full run is too slow to measure with |
| Dependency vulnerabilities | audit tools | usually a quick green, occasionally a surprise |

**Report the table before you ask anything.** The numbers are the argument. "Coverage is 61.4%, and 9 methods have CRAP over 30" turns an abstract question about rigor into a concrete one about which of nine methods get tests this week.

**The parallel run is not optional.** A `factory` run with `lanes = 2` or more runs the gate from several lanes at once, in one checkout. Time the suite once alone, then two copies started together, and watch for reds that only appear together: a shared database, a fixed port, a temp path with a fixed name, a file both runs write. The answer goes into `docs/project.toml` as `gate_parallel = true` or `false` under `[docs]`. Preflight asks for it before the first multi-lane run, and a `false` means every gate run pays serial wall-clock, dozens of times per run.

Some tiers cannot be green on day one without work. That is a legitimate outcome and it becomes part of the plan: cody-agent's PRD-011 had 14 CRAP offenders and **fixed all 14 by writing tests** rather than grandfathering them, because CRAP falls as coverage rises. Grandfathering a baseline is the alternative, and it is worse: a suppression list nobody ever burns down.

## 3. Interview — three rounds, numbers in hand

**AskUserQuestion**, recommendation first, cost stated. Read [references/tiers.md](references/tiers.md) before asking: it says what each tier proves that the previous one cannot, which is the only honest way to justify one.

**Round 1 — shape.**
- **Where it runs**: local only / CI only / both. *(Recommended: both, same command, so they cannot drift. Local-only is a real choice when the full gate needs a database, browsers and minutes — cody-agent chose it deliberately and kept CI as a cheap subset. Say which trade the project is making.)*
- **The wrapper**: which entry point becomes the one command. Prefer what the repo already uses — a `composer` script for Laravel, an npm script for Node, a `Makefile` or `justfile` target for anything polyglot. One string, always.
- **Right-size it.** A prototype does not need mutation testing; a payments service does. Ask what this code costs when it breaks, and drop tiers that do not earn their runtime.

**Round 2 — the ladder.** Multi-select over the tiers from tiers.md, each labelled with its measured baseline and its wall-clock cost. Default-on: deps audit, format, lint, static analysis, tests with a coverage floor. Default-off until argued for: mutation, complexity gate, size budget, arch rules, type coverage — each is on when the baseline shows it would catch something real.

**Round 3 — floors, escapes, and the inner loop.**
- **Each floor**, proposed at *measured actual minus a small margin* (~1 point), never a round number someone likes.
- **Escape hatches** for slow tiers: an env var that skips one (`SKIP_MUTATION=1`), named in the project docs. A tier with no escape hatch gets bypassed by deleting it from the script, which is worse.
- **Scoping for the expensive ones**: mutation runs **diff-scoped** against the trunk, not repo-wide, or it is unusable. Same for anything else measured in minutes.
- **The inner loop**, the command a builder runs between edits. **It runs the formatter in write mode, and the gate keeps check mode**: a builder's unformatted line is fixed where it was written, and the gate still fails on one nobody fixed. Each stack's two forms are side by side in the stack table. Two rules, not a preference. **When the full gate runs in about a minute, the inner loop is the gate's own tiers, with the formatter in write mode and the tests quiet.** Otherwise the inner loop is the cheap tiers plus the unit tests, and **format and lint are always in it**, because those are the reds a checkpoint otherwise pays for. PRD-001 of the factory ran `cargo test` alone as its inner loop: 83 runs, 46 red, and the first checkpoint spent four red runs on fmt and clippy nobody had run, in a repo whose whole gate takes under a minute.
- **Quiet tests in the inner loop**, where the runner has a flag for it: a passing run should print a summary line, not a line per test. Rust `cargo test -q`, Python `pytest -q`, Kotlin `./gradlew test -q`, Pest `--compact`, Vitest `--reporter=dot`. Go's `go test ./...` is already quiet without `-v`. The gate keeps its normal output.

## 4. Build it — one tier per script, chained by the wrapper

Each tier is **its own runnable script**, and the gate chains them cheap-first. That is what makes a failure debuggable: the agent that hits a red coverage tier reruns that tier alone, not the whole ladder.

```
verify   = fix + check                  # format first, then the read-only gate
check    = deps → format:check → lint → types → type-coverage
           → tests+coverage → complexity → e2e/browser → size → mutation
```

Ordering rules that are not negotiable:

- **Cheapest first.** Dependency audit and format take seconds; mutation takes minutes. A typo should not cost a mutation run.
- **Coverage before anything that reads coverage.** The complexity tier parses the coverage report the test tier wrote.
- **Slow and flaky last.** Browser suites and mutation at the end, after everything deterministic has passed.
- **Never make the gate's own test run incremental.** Test-impact analysis, `--changed`, `--tia`, `--onlyChanged` belong in the *inner loop* command, never in the gate. A gate that runs a subset is not a gate. Write that warning into the project's docs — it is the single most common regression here.
- **Every tier measured in minutes gets a `timeout`**, and the runner names the stuck test when it fires. Two hung browser runs lost 35 minutes in one unattended build.

Wire it per [references/stacks.md](references/stacks.md). The inner loop from round 3 is a second declared string, and the docs say plainly which is which: agents iterate with the inner loop and finish with the gate.

## 5. Prove it — green, then prove it goes red

A gate nobody has seen fail is a decoration.

1. **Run the whole gate once.** It must be green. Not green → fix the project, or lower the *tier's inclusion*, never the floor below what the code actually does.
2. **Then break it on purpose, one tier at a time.** Introduce a sacrificial violation, confirm that tier fails and names the problem, **remove it immediately**. An uncovered class for the coverage floor, a removed return type for type coverage, an unused import for lint, a fattened function for complexity.

   Two shortcuts that work on any threshold tier: **raise the floor past actual** (`--min=99` against 90.9% must exit 1, and the message should name the real number), or add one sacrificial file. Raising the floor is cheaper and touches nothing.

3. **Some tiers resist proof, and mutation is the one that always does.** A healthy suite kills mutants redundantly, so gutting a single assertion often leaves the score unchanged, and the floor cannot be raised past 100%. Weakening an existing test therefore proves nothing either way. The technique that does work is a **new sacrificial class with a deliberately assertion-free test** — that creates a mutant nothing kills, which is the exact condition the tier exists to catch. When even that is impractical, **prove what you can**: that the tier runs, scopes to the diff correctly, short-circuits on its escape variable, and exits clean on an empty diff. Then record the tier as **unverified, not disproven**, and say which half you proved.

4. **Prove the escape paths on every tier that has one**, not just the tier itself. A skip variable that silently does nothing, or a diff-scoped tier that errors on an empty diff instead of passing, both fail exactly when someone is relying on them.

5. **Record which tiers you proved red** and which you did not. An unproven tier is a claim, not a guarantee, and the report says so in those words.

### Audit mode

An `audit` starts here rather than at §4, and it is mostly about **decay**, because a gate that was right when it was built stops being right quietly.

**Start with the runs, not the configs.** The project's own runs have already measured this gate. Read every retro's gate rows (`docs/features/*/retro.md`, the inner-loop and checkpoint lines) and the store:

```
sqlite3 -header ~/.local/share/factory/factory.db \
  "select kind, count(*) runs, sum(red) red, round(sum(minutes),1) minutes from gates group by kind"
sqlite3 -header ~/.local/share/factory/factory.db \
  "select cmd, count(*) runs, sum(red) red from gates where kind='slice' group by cmd order by runs desc"
```

`kind = checkpoint` is the driver running the declared gate; `kind = slice` is what builders actually ran as their inner loop, command by command. The findings that come only from here:

- **A tier red at checkpoints and never in the inner loop** belongs in the inner loop (round 3's rule). Fmt and clippy red at a checkpoint and never in the inner loop is the pattern.
- **Builders not running the declared inner loop.** When the `cmd` column is a hand-rolled subset instead of the declared string, the declaration is wrong-sized or unknown to them.
- **A test red in one run of five** is flaky. It needs a stabilise pass, named as a finding with the test's name, not a retry rule. One such test cost a run a full gate rerun and a Trap lookup each time it fired.
- **Minutes that dwarf reds.** A tier that burns most of the gate's wall-clock and never fails has no timeout, no escape hatch, or no reason to be in the gate.

Then run the gate, prove its tiers as above, and check all of these:

- **Stale floors.** Compute the headroom on every threshold — actual minus floor. A tier with several points of headroom is not enforcing what the code already achieves, and the ratchet rule says bump it. **This is usually the highest-value finding in an audit** and it is always a one-line change.
- **Docs that disagree with the code.** Read every threshold out of the scripts and configs, then out of `CLAUDE.md`, `AGENTS.md` and the verification doc, and diff them. A documented floor that is two ratchets behind teaches agents the wrong number, and it is the most common drift in a healthy repo.
- **Does the gate build everything it then tests?** A gate that runs browser or integration tests without rebuilding the frontend drives whatever happens to be on disk, so a green result can come from a bundle days old. Check generated files too — routes, clients or types produced by a build step and gitignored — because a type check against stale generated code passes for the wrong reason.
- **A gate that mutates the working tree.** When the entry point runs a formatter or fixer before the read-only checks, it rewrites files, and firing it on a dirty tree is not safe. That is a fine design; it needs one line in the docs.
- **Undocumented CI divergence.** When CI runs a different set from the local gate, name which tiers are local-only and why. Undocumented divergence gets "fixed" by accident later, in whichever direction is worse.
- **Available but unused.** Tooling the stack already ships that the gate does not use — arch-test support in the installed test runner, a security scanner in the linter. Cheap tiers, already paid for.
- **Grandfathering.** Static-analysis baselines, complexity allowlists, skipped test lists. Each is a debt with no burn-down date, and an audit is where it gets named.
- **Incremental selection in the gate.** `--tia`, `--changed`, `--onlyChanged` anywhere in the gate or a gating CI job. This is the regression that comes back; check for it every time.
- **A slow tier with no timeout.** A browser or e2e tier that can hang costs more than it catches. Every tier measured in minutes gets a `timeout`, and the runner names the stuck test when it fires.
- **The inner loop, against round 3's rule.** Not declared → `factory docs gate --inner` falls back to the full gate, so every slice pays for the slow tiers. Declared without format and lint → the checkpoints pay for them instead. A gate that runs in about a minute with a smaller inner loop beside it → make the inner loop the gate's own tiers, with the formatter in write mode and the tests quiet. An inner loop that formats in check mode, or runs its tests loud where the runner has a quiet flag → the builders pay a red run, or a page of output, for nothing.
- **`gate_parallel` unmeasured.** Preflight judges it on every run until `docs/project.toml` holds it. Measure it as in §2 and write the answer; it has to be known before the first `lanes = 2` run.
- **Diff-scoped tiers that read only committed changes.** A mutation or arch tier scoped by `git diff origin/main` silently skips everything in the working tree; an agent that has not committed yet gets a clean pass for code the tier never saw. Include the working tree, or have the tier print which packages it skipped.

Report each with its cost. **Change nothing without approval** — an audit that edits is not an audit.

## 6. Declare and document

- **Declare both strings**: `factory docs gate set "<cmd>"` and `factory docs gate set --inner "<cmd>"`. They land under `[docs]` in `docs/project.toml`, which is where every slice's Verify task, every checkpoint, `/polish` and `prune` read them. Write `gate_parallel` there too, by hand, from the §2 measurement.
- **Write the project's verification doc** — the section in `CLAUDE.md` / `AGENTS.md` the project already uses, or `.ai/guidelines/verification.md`. Three parts, no more: the **inner loop** (fast feedback while iterating), **finishing** (the gate is the only accepted proof of done, and the tier list with its thresholds), **environment facts** (the PHP binary, the database container, the coverage driver — whatever bites someone whose machine is not yours).
- **Write the ratchet rule into that doc**, in the project's own words: floors rise when actual rises; never lower a floor to get green; cover or refactor instead.
- **CI**, if round 1 said so: the same command, not a re-listed copy of its steps. When CI deliberately runs a subset, say *which* subset and *why* in the doc, so nobody later "fixes" the difference by accident.
- **Commit** the scripts, the configs, `docs/project.toml` and the doc together.

## Report

The baseline table, as measured, **with a headroom column** — threshold, actual, and the distance between them. Headroom is what makes a stale floor visible at a glance, and it is the column the owner acts on:

| Tier | Threshold | Actual | Headroom |
|---|---|---|---|
| Line coverage | `--min=87` | 90.9% | **3.9 pts — stale, bump to 90** |
| Type coverage | `--min=96` | 96.1% | 0.1 pts |
| Complexity | fails > 30 | worst 21.2 | 8.8 |

Then:

- the tiers included, each with its threshold and where the number came from
- the tiers deliberately excluded, and why
- **which tiers you proved red, and how** — plus any you could only partially prove, named as unverified rather than passed
- the gate's wall-clock, split into the fast half and the long pole; the inner-loop command beside it, and whether it is the gate; `gate_parallel` as measured, with the reds that decided it
- what has to be cleaned up before a tier can go green, when something was left red on purpose
- for an audit: what the runs said first, then each gap with its cost, and nothing changed
