# The ladder — what each tier proves that the one before it cannot

A tier earns its runtime only by catching a failure no cheaper tier can see. That is the whole justification, and it is what you tell the owner when proposing one. Ordered cheapest-first, which is also the order the gate runs them.

| # | Tier | Catches what nothing above it can | Typical cost |
|---|---|---|---|
| 1 | **Dependency hygiene** | a known-vulnerable or invalid dependency, including one an agent added without asking | seconds |
| 2 | **Format** | style churn that makes every future diff unreadable | seconds |
| 3 | **Lint** | the bug class the compiler does not see: unused results, shadowing, banned APIs, import order | seconds |
| 4 | **Static analysis / types** | a type error the tests never reach because no test calls that path | seconds to a minute |
| 5 | **Type coverage** | code that type-checks because it is untyped — `any`, `mixed`, missing returns | seconds |
| 6 | **Tests + coverage floor** | behavior that broke, and code no test executes at all | the suite |
| 7 | **Complexity vs coverage** | the *dangerous* untested code specifically: high complexity and low coverage together | seconds, reads tier 6's report |
| 8 | **End-to-end / browser** | the wiring between layers that unit tests each pass in isolation | minutes |
| 9 | **Size / performance budget** | a dependency that quietly doubled the bundle users download | seconds, needs a build |
| 10 | **Mutation** | tests that execute code and assert nothing — coverage cannot see this, ever | minutes, diff-scoped |
| 11 | **Architecture rules** | a layering violation that is correct code and wrong design | seconds |

**The two most-skipped tiers are the two that catch what coverage cannot.** Coverage says a line ran. **Mutation** says an assertion would notice if that line changed. **Complexity-vs-coverage** says which uncovered code is actually dangerous. A suite with 90% coverage and no mutation pressure can be almost entirely assertion-free, and this is the normal failure mode of AI-written tests, which is exactly why it belongs in a repo written by agents.

## Thresholds

**Every number comes from a measurement in this repo, today.** Propose *measured actual minus about one point*, so a rounding wobble does not turn the gate red on unrelated work.

- **Coverage floor.** Measured actual minus ~1. Scope it to the code that matters — the application code, not generated files, migrations or vendored code. Unit and feature tests feed the number; browser tests usually run as plain pass/fail outside it, because instrumenting them is slow and their coverage is misleading.
- **Type coverage floor.** Same rule. This one is usually already high (90s) and cheap to hold.
- **Complexity gate.** CRAP = complexity² at zero coverage, ≈ complexity at full coverage, so a fully covered method is almost never an offender. Two bands: **fail above 30**, **warn above 15**. Clear the fail list by *writing tests* before turning the tier on; refactor only for a method still over 30 when fully covered.
- **Mutation score.** Diff-scoped, floor around 70 to start. A repo-wide mutation run is minutes to hours and will simply be bypassed. **This tier resists being proved red**, because a healthy suite kills mutants redundantly and the floor cannot exceed 100%: weakening one assertion usually changes nothing. Prove it with a new sacrificial class carrying an assertion-free test, or prove only its mechanics and record it as unverified. Do not report it as proven because it ran.
- **Size budget.** The gzipped size of what a user actually downloads, at whatever the current build produces plus a small headroom. Name the artifact precisely: the initial-load chunks, not the whole `dist/`.
- **Static analysis level.** Hold the current level and get green; a level bump is its own piece of work with its own fallout, never a side effect of building the gate.

## The ratchet

**Floors only go up.** When actual coverage rises, bump the floor to just under it — by hand, deliberately, as its own small commit. When a change would push below the floor, cover it or refactor it. **Never lower a floor to get green.**

This rule has to live in the project's own verification doc, not only here, because it is the one an agent under time pressure will otherwise quietly break — and a floor that ratchets down is indistinguishable from no floor at all.

**The failure mode is not violation, it is neglect.** Nobody lowers a floor; the code simply improves and the floor stays where it was, until it is several points behind and stops enforcing anything the code was not already doing. Measured on a well-maintained repo: a coverage floor four points stale while every other tier was current, with the documented value two ratchets behind the code on top of that. So an audit computes **headroom** — actual minus floor — for every threshold, and treats real headroom as a finding with a one-line fix.

**Thresholds live in exactly one place, and the docs quote it.** A floor written into a command's default *and* into `CLAUDE.md` *and* into a guidelines file is three values that drift apart, and agents believe the documented one. Say where the number lives and have the docs point there.

## Escape hatches

A slow tier needs a documented way to skip it (`SKIP_MUTATION=1`), or it gets deleted from the script instead, which loses it permanently and silently. The hatch is for a local iteration loop, never for the run that declares work done. Name every hatch in the verification doc.

## The incremental trap

Test-impact analysis (`--tia`), `--changed`, `--onlyChanged`, `--filter` are **inner-loop tools**. They make iteration fast and they must never enter the gate, or CI, because a gate that runs a subset proves a subset. Keep two commands and label them plainly. Write the warning into the project doc: this is the regression that comes back, because adding `--tia` to the gate looks like a pure speed win and quietly guts it.

## Arch rules worth having

Cheap, fast, and they encode a decision instead of repeating it in review. The recurring useful ones:

- no debug calls in production code (`dd`, `dump`, `console.log`, `println!`, `fmt.Println`)
- domain layer does not import the framework, the HTTP layer, or the database driver
- enums and constants live where the convention says they live
- two paths that must stay in sync actually use the same validator or schema
- nothing imports a module marked internal to another package

Each one should come from a real incident or a real convention in this repo. An arch rule invented from a style guide is noise; one that locks a convention already holding is free insurance.
