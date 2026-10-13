# Stack Detection & Command Adapters

Resolve four commands before starting: **full run**, **single file run**, **coverage run**, and the **verify gate** (if the project defines one).

**Always prefer the project's own entry points** over raw tool invocations — check, in order: `composer.json` scripts, `package.json` scripts, `Makefile` / `justfile` targets, CI workflow files (`.github/workflows/*.yml`) for the commands the project actually uses.

## Verify gate

Many projects define a single command bundling tests + static analysis + mutation testing + formatting checks. Look for, in order:

- **First**: `factory docs gate` — the framework's declared gate, for a project on it. Exit 0 prints the command; exit 1 means none declared, fall through to the hunt below.
- `composer.json` scripts: `verify`, `check`, `ci`, `qa`
- `Makefile` / `justfile` targets: `verify`, `check`, `ci`
- `package.json` scripts: `verify`, `check`, `ci`
- Agent instruction files (`CLAUDE.md`, `AGENTS.md`) naming a verify command

When one exists, it is the authoritative final gate in Phase 4 — it supersedes a plain full-suite run. Its mutation-testing step (Infection, Stryker, mutmut) is the only tooling that detects weakened assertions, which coverage cannot see. Don't run the full gate after every batch (mutation testing is slow) — per-batch discipline stays file-level test runs; the gate runs once at the end.

## Detection table

| Ecosystem | Detect by | Full run | Single file | Coverage |
|-----------|-----------|----------|-------------|----------|
| PHP (Laravel + Pest/PHPUnit) | `artisan`, `phpunit.xml`, `tests/Pest.php` | `php artisan test` (or `./vendor/bin/pest`) | `php artisan test tests/Feature/FooTest.php` | `php artisan test --coverage` (needs Xdebug/PCOV) |
| PHP (plain) | `phpunit.xml`, `composer.json` | `./vendor/bin/phpunit` | `./vendor/bin/phpunit tests/FooTest.php` | `./vendor/bin/phpunit --coverage-text` |
| JS/TS (Vitest) | `vitest.config.*`, deps in `package.json` | `npx vitest run` | `npx vitest run path/foo.test.ts` | `npx vitest run --coverage` |
| JS/TS (Jest) | `jest.config.*`, deps in `package.json` | `npx jest` | `npx jest path/foo.test.ts` | `npx jest --coverage` |
| Python | `pytest.ini`, `pyproject.toml`, `conftest.py` | `pytest` | `pytest tests/test_foo.py` | `pytest --cov --cov-branch` |
| Go | `go.mod` | `go test ./...` | `go test ./pkg/foo/` | `go test ./... -cover` |
| Rust | `Cargo.toml` | `cargo test` | `cargo test --test foo` | `cargo llvm-cov` (if installed) |
| Ruby (RSpec) | `Gemfile`, `.rspec` | `bundle exec rspec` | `bundle exec rspec spec/foo_spec.rb` | SimpleCov (via spec_helper) |
| .NET | `*.csproj`, `*.sln` | `dotnet test` | `dotnet test --filter FullyQualifiedName~Foo` | `dotnet test --collect:"XPlat Code Coverage"` |

Notes:

- Respect version managers/wrappers if present: `sail test`, `herd`, `nvm`, `mise`, `asdf`, `poetry run`, `uv run`, `pnpm`/`yarn`/`bun` instead of `npx`.
- Verify each resolved command with a quick real run before recording the baseline.
- Timing: capture wall-clock with `time <command>` or the runner's own summary line.
- Flaky-order check: `pytest -p no:randomly` vs `pytest -p randomly`, Jest `--runInBand`, Pest `--order-by random`, Go `-shuffle=on`.
- If coverage tooling is missing and cheap to add (dev-dependency only), ask the user; otherwise proceed without coverage deltas.
