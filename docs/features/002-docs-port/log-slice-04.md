## Decisions

### AD-401 — What ship, archive and renumber stage · slice-04 · 2026-10-13
**Chose:** each verb stages only the paths it wrote or moved. `ship` stages the feature's `state.toml`. `archive` stages the moved folder's `state.toml`, and `git mv` stages the move. `renumber` stages the renamed folder and each rewritten markdown file.
**Over:** v2's `git add -- docs` (`docs.mjs:1810`, `:1914`).
**Because:** with lanes ≥ 2, `docs/` holds a sibling's unstaged work. `git add -- docs` would claim it into a commit that is not the sibling's.
**Undo:** one `git add` call per verb in `src/docs/ship.rs` and `src/docs/renumber.rs`.
**Review:** fyi

### AD-402 — renumber on an archived feature · slice-04 · 2026-10-13
**Chose:** `renumber` refuses an archived feature with exit 2 ("the archive is frozen").
**Over:** v2's `resolveFeature`, which also finds archive folders.
**Because:** v3 never writes under `archive/`, and the pre-commit hook refuses the commit anyway.
**Undo:** delete the `Kind::Archive` guard in `src/docs/renumber.rs` `run`.
**Review:** no

### AD-403 — The gate's verify hints · slice-04 · 2026-10-13
**Chose:** at most three hints in total, across `CLAUDE.md`, `AGENTS.md` and `.ai/guidelines/verification.md`.
**Over:** v2's per-file `break` (`docs.mjs:505`), which lets a second file add one more hint after the third.
**Because:** the cap of three is the intent. The overflow is a loop-placement accident that no caller relies on.
**Undo:** move the length check in `src/docs/gate.rs` `candidates`.
**Review:** no

## Traps

- `Fixture::v3_with` appends its keys to `[docs]`. Passing a key the fixture already sets (`gate_parallel`, `lanes`) makes a duplicate TOML key. Edit the line in place and commit it instead (`edit_project` in `tests/docs_lifecycle.rs`).
