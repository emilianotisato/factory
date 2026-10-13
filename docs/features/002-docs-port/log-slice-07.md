## Traps

- Lanes share one worktree, and HEAD alone does not build mid-run (`src/docs.rs` and `src/docs/mod.rs` both exist at 0d6d340, E0761). Wait for the sibling's edits to compile with a private `CARGO_TARGET_DIR`; a scratch build of HEAD plus your files does not work.

## Decisions

### AD-701 — What `type` does a v2 PRD get when `**Type**:` holds prose? · slice-07 · 2026-10-13
**Chose:** the first word of the `**Type**:` value, lowercased, when it is one of `feature | bugfix | enhancement | refactor` (a `fix` plan row names the old value); any other value is kept whole with a `warn` row.
**Over:** the whole value, as the slice says ("the whole `**Type**:` value").
**Because:** this repo's PRDs read `**Type**: Feature — …`, and slice 03's `check` reports a `type` outside the four as an error, so the whole value would fail the acceptance "`factory docs check` is clean".
**Undo:** write the whole value in `prd_md` (`src/docs/migrate.rs`) and drop the fix row.
**Review:** decide

### AD-702 — In what order do `log.flagged` and `log.decide` list ids after migration? · slice-07 · 2026-10-13
**Chose:** ascending by id, which is the order `log.jsonl` holds after migration; the harness compares them sorted.
**Over:** v2 file order (`BLOCK-201`, `AD-101` … `AD-9001`, `AD-501` in PRD-001).
**Because:** slice 03's `E-LOG-ORDER` needs each sequence to increase, so v2 file order cannot survive; the set of ids is what the acceptance names.
**Undo:** none needed; a reader that wants file order reads the old `agent-log.md` from git.
**Review:** fyi

### AD-703 — A heading or Owner date written `YYYYMMDD` · slice-07 · 2026-10-13
**Chose:** rewrite it `YYYY-MM-DD` in `ts` with a `fix` plan row, as task 3 does for `reviewed`.
**Over:** refusing the migration.
**Because:** this repo already has one such value (`003` `reviewed: 20261013`); the same slip in a log heading should not stop a whole repo.
**Undo:** make `day()` refuse non-ISO dates.
**Review:** no

### AD-704 — The plan is clock-free; `written_at` is stamped at run · slice-07 · 2026-10-13
**Chose:** `plan` carries `written_at` empty and `run` sets it with `clock::now_rfc3339()` per upsert.
**Over:** stamping it in `plan`.
**Because:** task 6 asks that `plan` twice gives equal `Plan`s, and the acceptance allows `written_at` alone to differ.
**Undo:** move the stamp into `retro_rows`.
**Review:** no

### AD-705 — An empty index Notes cell · slice-07 · 2026-10-13
**Chose:** `summary: ""` and a `warn` row, so the seven keys are always present.
**Over:** omitting the key.
**Because:** slice 03's `check` warns on an empty summary only on a `ready` feature; an absent key would read as a malformed header.
**Undo:** skip the key in `prd_md`.
**Review:** no

### AD-706 — The PRD-001 copy test meets unmerged fragments mid-run · slice-07 · 2026-10-13
**Chose:** before migrating the `git archive HEAD docs` copy, run `node docs.mjs log merge NNN` in that copy for every feature holding a `log-*.md`, as migrate's refusal tells the owner.
**Over:** the bare extract task 8 describes, which migrate refuses while slices are building (HEAD held `log-slice-03/04/05.md` of 002).
**Because:** the refusal is right, and the test proves migration, not the run's timing; at a checkpoint the merge is a no-op.
**Undo:** delete the merge loop in `prd_001_copy_migrates` (`tests/migrate.rs`).
**Review:** fyi

## Untracked leftovers

- src/answers.rs
- src/checkpoint.rs
- src/cli/answer.rs
- src/cli/report.rs
- src/cli/run.rs
- src/cli/status.rs
- src/closeout.rs
- src/cockpit.rs
- src/commit.rs
- src/error.rs
- src/events.rs
- src/pack.rs
- src/retro.rs
- src/scheduler.rs
- tests/checkpoints.rs
- tests/cockpit.rs
- tests/commits.rs
- tests/docs_v3.rs
- tests/fixture/.docs/VERSION
- tests/fixture/.docs/bin/docs.mjs
- tests/fixture/.docs/templates/agent-log.md
- tests/fixture/.docs/templates/decisions.md
- tests/fixture/.docs/templates/ideas.md
- tests/fixture/.docs/templates/index.md
- tests/fixture/.docs/templates/prd.scaffold.md
- tests/fixture/.docs/templates/retro.md
- tests/fixture/.docs/templates/slice.scaffold.md
- tests/fixture/v2/.docs/VERSION
- tests/fixture/v2/.docs/bin/docs.mjs
- tests/fixture/v2/.docs/templates/agent-log.md
- tests/fixture/v2/.docs/templates/decisions.md
- tests/fixture/v2/.docs/templates/ideas.md
- tests/fixture/v2/.docs/templates/index.md
- tests/fixture/v2/.docs/templates/prd.scaffold.md
- tests/fixture/v2/.docs/templates/retro.md
- tests/fixture/v2/.docs/templates/slice.scaffold.md
- tests/fixture/v2/README.md
- tests/fixture/v2/docs/features/001-fixture/agent-log.md
- tests/fixture/v2/docs/features/001-fixture/prd.md
- tests/fixture/v2/docs/features/001-fixture/retro.md
- tests/fixture/v2/docs/features/001-fixture/slice-01-a.md
- tests/fixture/v2/docs/features/001-fixture/slice-02-b.md
- tests/fixture/v2/docs/features/001-fixture/slice-03-c.md
- tests/fixture/v2/docs/ideas.md
- tests/fixture/v2/docs/index.md
- tests/fixture/v2/docs/project.toml
- tests/fixture/v2/gate.sh
- tests/knobs.rs
- tests/lanes.rs
- tests/loop.rs
- tests/metrics.rs
- tests/quota.rs
- tests/stubs.rs
- tests/stubs/claude
