# Classification Taxonomy & Pruning Rules

Evaluate every test in scope against these four categories:

| Category | Description | Action |
|----------|-------------|--------|
| **KEEP** | Tests core user flows, public API contracts, edge cases, or documented business-logic boundaries. | Preserve untouched. |
| **CONSOLIDATE** | Multiple tests checking minor variations of the same function or path with repetitive setup. | Refactor into table-driven / parameterized tests. |
| **STABILIZE** | Flaky tests caused by race conditions, non-deterministic timers, mutable shared state, or improper mocks. | Fix isolation/async issues (Rule D). |
| **PRUNE** | Scaffolding tests of private implementation details, zero-assertion tests, or 100% redundant duplicates. | Delete. |

When in doubt between KEEP and PRUNE — no clear covering test found, or the business logic looks load-bearing but unfamiliar — ask the user in-session, presenting the test and the case for each side. Don't guess, and don't silently KEEP everything to avoid asking.

## Rule A — The Stepping-Stone Test

Ask: *"Does this test assert a contract or behavior, or did it only exist to verify a single function while the feature was being built?"*

Scaffolding signals:

- Tests private methods or internal state already indirectly verified by higher-level public tests.
- Test name references an implementation step ("test_helper_returns_dict") rather than a behavior.
- Mocks so much that only the mock wiring is being asserted.

Action: PRUNE only if a public-contract test demonstrably covers the same execution path. Verify by reading the covering test, not by assumption.

## Rule B — Redundancy & Consolidation

Ask: *"Are there N separate tests with identical setup, differing only in input values?"*

Action: combine into one parameterized test:

- Python: `@pytest.mark.parametrize`
- JS/TS: `it.each` / `test.each`
- PHP (Pest): `->with([...])` datasets; PHPUnit: `#[DataProvider]`
- Go: table-driven tests
- Ruby: shared examples or looped `it` blocks

Keep failure output readable: each case must be identifiable on failure (named parameters / dataset keys).

## Rule C — Assertion Quality

Ask: *"Does this test assert a meaningful outcome?"*

Scaffolding signals:

- `assert result is not None` (or equivalent) with no property checks.
- Zero assertions — the test only exercises code for coverage.
- Asserts only that no exception was thrown, for a pure function.

Action: upgrade the assertion to check expected state/output if the path is otherwise uncovered; PRUNE if covered elsewhere.

## Rule D — Flakiness Resolution

For each FLAKY-tagged test, check in order:

1. **Sleeps/timeouts:** replace hardcoded delays (`sleep(100)`) with explicit waits — events, polling with timeout, promise resolution, framework fake timers.
2. **Shared mutable state:** ensure DB/state resets in teardown or fixtures (transactions, `RefreshDatabase`, per-test temp dirs).
3. **Order dependency:** run the test in isolation and in shuffled order; if it only fails in sequence, find and break the coupling to the earlier test.
4. **External data:** freeze time, seed randomness, mock network/API calls.

If a flaky test can't be stabilized in reasonable effort AND its path is covered elsewhere → PRUNE with an explicit note in the report. Never silently skip/disable a test.
