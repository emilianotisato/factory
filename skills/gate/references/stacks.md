# Per-stack tooling — detection, commands, wiring

Verify every command against the versions actually installed before writing it into a script. Flags move between majors, and a gate that fails on a bad flag teaches everyone to distrust it.

**Install posture: push, take one refusal.** A missing tier tool is proposed as the recommended option with its exact install command and its cost. One "no" is honoured; then say in the report which tier the project is going without and what that leaves unguarded.

---

## PHP / Laravel

**Detect**: `artisan`, `composer.json`, `phpunit.xml`.

| Tier | Command | Install |
|---|---|---|
| deps | `composer validate --strict && composer audit && npm audit --audit-level=high` | built in |
| format | `pint --parallel` / `pint --parallel --test` | `laravel/pint` |
| static | `phpstan analyse` | `larastan/larastan` |
| type coverage | `pest --type-coverage --min=<n> --parallel` | `pestphp/pest-plugin-type-coverage` |
| tests + coverage | `pest --testsuite=Unit,Feature --parallel --coverage --min=<n> --coverage-clover=build/clover.xml` | needs PCOV or Xdebug |
| complexity | an artisan command parsing the `crap` attribute out of the clover XML: fail > 30, warn > 15 | write it (~80 lines, itself tested) |
| browser | `pest --testsuite=Browser` (serial, no coverage) | `pestphp/pest-plugin-browser` + `npx playwright install chromium` |
| mutation | `pest --mutate --covered-only --parallel --min=<n>` scoped to classes changed vs `origin/main` | core in Pest 5 |
| arch | `arch()->expect('App')->not->toUse(['dd','dump'])` in a normal test file | core |

**Build the frontend before any tier that exercises it.** Measured on a real repo: `ci:check` built the embedded widget but never ran the main `npm run build`, so the browser suite drove whatever bundle happened to be on disk and a Vite break only surfaced in CI. Worse where the framework generates route or client files at build time and gitignores them — a type check then validates stale generated code and passes for the wrong reason. Cost to fix: one `npm run build` before the browser tier, about three seconds.

**Notes that cost hours if missed.** Coverage, TIA and mutation need a coverage driver, and a static PHP build often cannot load one — the project may need a second binary for those steps, injected via `PHP_INI_SCAN_DIR` inside the scripts so nobody has to remember. Pest has no native diff-scoped mutation: a small artisan wrapper computes the changed classes against the trunk (fetch first) and passes `--class=`; no changed classes is a clean skip. Paratest is core in Pest 5; the browser suite stays serial. `artisan test` and a coverage-enabled binary do not always cooperate — call `vendor/bin/pest` directly there.

**Wrapper**: composer scripts. `verify` = `@fix` + `@ci:check`; `ci:check` chains the tiers. Each tier is its own script so it can be run alone.

---

## Node / TypeScript

**Detect**: `package.json`, `tsconfig.json`.

| Tier | Command | Install |
|---|---|---|
| deps | `npm audit --audit-level=high` | built in |
| format | `prettier --check .` | `prettier` |
| lint | `eslint .` | `eslint` |
| types | `tsc --noEmit` (`vue-tsc --noEmit` for Vue) | built in |
| type coverage | `type-coverage --at-least <n>` | `type-coverage` |
| tests + coverage | `vitest run --coverage` with `coverage.thresholds` in the config; Jest uses `coverageThreshold` | built in |
| e2e | `playwright test` | `@playwright/test` |
| size | `size-limit` with a `size-limit` block naming the initial-load chunks, `gzip: true`, a `limit` string | `size-limit` + `@size-limit/file` |
| mutation | `stryker run --incremental` | `@stryker-mutator/core` |
| arch | `dependency-cruiser` rules, or `eslint-plugin-boundaries` | either |

**Notes.** Put thresholds in the test runner's config, not on the command line, so an interactive run enforces them too. `size-limit` needs a build first, so the size tier runs after the build tier. In a workspace/monorepo, the gate iterates the packages rather than one root command — a `--workspaces` run or an explicit list, so a failure names the package.

**Wrapper**: npm scripts, or a `Makefile` target when the repo is polyglot.

---

## Go

**Detect**: `go.mod`.

| Tier | Command | Install |
|---|---|---|
| deps | `govulncheck ./...` | `golang.org/x/vuln/cmd/govulncheck` |
| format | `gofmt -l .` (non-empty output is a failure) or `gofumpt -l .` | built in / `mvdan.cc/gofumpt` |
| lint | `golangci-lint run` (bundles `vet`, `staticcheck`, `errcheck`) | `golangci-lint` |
| tests + coverage | `go test -race -coverprofile=cover.out ./...` then a threshold check on `go tool cover -func=cover.out` | built in |
| mutation | `gremlins run` | `gremlins` |
| arch | `depguard` inside golangci-lint, or `go-arch-lint` | either |

**Notes.** Go has **no built-in coverage floor** — the total line of `go tool cover -func` is parsed and compared in a few lines of shell or a tiny Go program; write it once and commit it. `-race` belongs in the gate and out of the inner loop, it roughly doubles the runtime. Go's static analysis is strong enough that a separate type-coverage tier does not apply.

**Wrapper**: a `Makefile` target. `make verify`.

---

## Kotlin / Android

**Detect**: `build.gradle`, `build.gradle.kts`, `gradlew`.

| Tier | Command | Install |
|---|---|---|
| deps | `./gradlew dependencyCheckAnalyze` | OWASP dependency-check plugin |
| format | `./gradlew spotlessCheck` (ktlint underneath) | `spotless` |
| lint | `./gradlew detekt` and `./gradlew lint` (Android lint) | `detekt` |
| tests + coverage | `./gradlew test jacocoTestCoverageVerification` with a `violationRules` minimum | `jacoco` |
| e2e | `./gradlew connectedAndroidTest`, or a Maestro flow | built in / `maestro` |
| size | an APK/AAB size check task comparing the built artifact against a limit | write it |
| mutation | `./gradlew pitest` | `pitest` gradle plugin |
| arch | Konsist tests, run as normal unit tests | `konsist` |

**Notes.** JaCoCo's floor lives in `jacocoTestCoverageVerification`'s `violationRules`, not on the command line. `connectedAndroidTest` needs a device or emulator, so it is usually a separate lane rather than part of the local gate — say which, do not leave it ambiguous. Gradle's own caching makes tier ordering matter less, but keep it anyway for the failure message.

**Wrapper**: a `verify` gradle task depending on the others, or a `Makefile` when the repo also holds a backend.

---

## Python

**Detect**: `pyproject.toml`, `pytest.ini`, `conftest.py`.

| Tier | Command | Install |
|---|---|---|
| deps | `pip-audit` | `pip-audit` |
| format | `ruff format --check .` | `ruff` |
| lint | `ruff check .` | `ruff` |
| types | `mypy .` or `pyright` | either |
| tests + coverage | `pytest --cov --cov-branch --cov-fail-under=<n>` | `pytest-cov` |
| mutation | `mutmut run` | `mutmut` |
| arch | `lint-imports` | `import-linter` |

**Notes.** `ruff` replaces the old flake8/isort/black stack; do not add those beside it. Branch coverage is worth the extra runtime here — Python's dynamism means line coverage overstates what was tested.

**Wrapper**: a `Makefile` or `just` target; `pyproject.toml` scripts if the project already uses a runner.

---

## Rust

**Detect**: `Cargo.toml`.

| Tier | Command | Install |
|---|---|---|
| deps | `cargo audit` or `cargo deny check` | `cargo-audit` / `cargo-deny` |
| format | `cargo fmt --check` | built in |
| lint | `cargo clippy --all-targets -- -D warnings` | built in |
| tests + coverage | `cargo llvm-cov --fail-under-lines <n>` | `cargo-llvm-cov` |
| mutation | `cargo mutants` | `cargo-mutants` |

**Notes.** The compiler and clippy do most of what a static-analysis and type-coverage tier does elsewhere, so the ladder is genuinely shorter here. That is a real answer, not a gap.

---

## Polyglot repos

**One string, always.** Several stacks means one wrapper target calling each stack's own gate:

```make
verify: verify-api verify-web verify-mobile
```

Rules that keep it usable:

- **Each stack's gate stays runnable alone**, so someone touching only the frontend does not wait on the Go suite.
- **Order the stacks by what breaks most often**, so the common failure comes back first.
- **The wrapper is what `factory docs gate set` declares.** The sub-gates are not declared; the framework holds exactly one string.

---

## CI wiring

**When CI and local run the same gate, CI calls the same command.** Not a re-listed copy of its steps: a list that drifts is worse than no list.

```yaml
- name: Verify
  run: make verify        # or: composer verify
```

**When CI deliberately runs a subset** — a real choice when the full gate needs a database, browsers and minutes — say which subset and *why*, in the project's verification doc. Otherwise someone later "fixes" the difference and either slows CI to a crawl or quietly weakens the local gate.

Whatever the split, **CI never runs the incremental test path**. A baseline-recording job that gates nothing is fine and useful; it must say so in its own name and its own file.
