---
slice: 06
status: done
depends_on: [1]
lane: 3
---

# Slice 06 — Retros store (FR-19 part)

**Source**: [`prd.md`](prd.md) → §9 FR-19 (the `retros` table, `SCHEMA_VERSION` 3, `factory docs retro NNN [--set score=N]`, "`factory report` is unchanged"), §11 OPEN-11 (kept thin). **Depends on**: 01 (the `docs retro` arm goes into slice 01's flat `DocsCli` match and resolves `NNN` through slice 01's board scan).
**Created**: 2026-10-13 · **Status**: pending

**Progress**:
- **Built.** `src/db.rs`: `SCHEMA_VERSION` 3, the `retros` table appended to `SCHEMA`, `RETRO_COLUMNS`, `RetroRow`, `upsert_retro` (`ON CONFLICT … DO UPDATE`, `score = COALESCE(excluded.score, retros.score)`, `written_at` set inside), `retro`, `set_retro_score`. The guarded-column list moved out of `migrate` into `const COLUMNS`, unchanged, so the v2-store test can build every column (AD-602). `src/docs/retro.rs` (new): `show`, `set_score`, `lines`. `src/cli/docs.rs`: the `Retro` verb, run from `native` because it alone exits 1 (AD-601). `src/db.rs:1058` and `tests/run.rs:277` now expect version `SCHEMA_VERSION` and `retros`.
- **Tests.** `src/db.rs`: a v2 store gains `retros`, keeps its `runs` row, and has three version rows after a second open; the PRD-003 migrate body runs on a v3 store; upsert idempotence; a score set first survives the numbers, then `Some(3)` overrides it; `2.23` and `16.0` read back. `src/docs/retro.rs`: `--set` parsing, line order. `tests/docs_retro.rs`: the task 3 acceptance on `Fixture::v3`. `tests/metrics.rs`: `a_version_2_store_gains_retros_and_reports_the_same`, plain and `--json`, against a `cp -a` copy taken while the store was at version 2 (AD-603). `tests/metrics.rs:284`, `:418`, `:503`, `:656` untouched and green.
- **Inner loop** (`cargo test`): 3 runs, 1 red, about 3 min. The red run caught two test-setup faults: the base `SCHEMA` already declares some guarded columns (`commit_sha`), so the hand-built v2 store must check before each `ALTER`; and `retros.run_id` is enforced as a foreign key, so the retro unit tests need their `runs` row first.
- **Deviation.** `docs retro` prints the `REAL` minutes in their shortest form (`16`, `2.23`), not `factory report`'s two decimals (AD-604).
- **Prove (task 6)** is left for the checkpoint: the store half of the FR-19 QA row.
- **Prove FR-19 store half (CP-1, 2026-10-13)**: a `.backup` copy of the real store (version 2) plus `runs/003-20261013-4139`: `factory report` installed vs branch, plain and `--json`, both `diff`s empty; version `3` and the `.schema retros` text after the branch binary opened it; the installed binary still reports the same on the v3 copy; the original stays at 2. In the v3 repo `docs retro 001 --set score=4` exit 0, `--set score=high` exit 2, `SELECT feature, score FROM retros` → `1|4` — `evidence/cp-1-fr19.txt`.

**Slice**: After this slice the metrics store is at schema version 3 and holds a `retros` table, one row per `(repo, feature)`, with `score`, `shipped_at`, `run_id` and the twelve retro keys. A store the PRD-003 binary made gains the table on first open, and the PRD-003 binary still opens a v3 store. `factory docs retro NNN` prints a feature's row, and `factory docs retro NNN --set score=N` is the one way to set `score`. Two writers call this slice's upsert later: closeout at ship (slice 10, `retro_row`) and `migrate` for every pre-v3 `retro.md` (slice 07). Neither is wired here. `factory report` and `factory retro` do not read the table.

## Current code / environment

- **The migration pattern is fixed and the table needs only its first half**: `src/db.rs:15 SCHEMA_VERSION = 2`; `SCHEMA` (`:17-95`) is a batch of `CREATE TABLE IF NOT EXISTS`; `migrate` (`:285-341`) runs it, then the guarded `ALTER TABLE ADD COLUMN` list (`:290-328`), then `INSERT OR IGNORE` of the version row (`:336-339`), all in one `Immediate` transaction. So the `retros` table is one more `CREATE TABLE IF NOT EXISTS` in `SCHEMA` and the version constant goes to 3. Nothing else changes in `migrate`. The PRD-003 binary opening a v3 store runs its own `SCHEMA` (no `retros` there, `IF NOT EXISTS` on the rest) and inserts version 2 with `OR IGNORE`, which already exists: it works, as prd §9 needs.
- **Two tests pin today's version and table list, and go red**: `src/db.rs:984` asserts `schema_version() == 2` after a v1 store migrates (`a_version_1_store_migrates_in_place_and_keeps_nulls`, `:964`). Make it `SCHEMA_VERSION`. `tests/run.rs:277-287` asserts `tables()` is exactly `events, gates, runs, schema_version, tool_calls, workers`; `retros` goes between `gates` and `runs` (`tables()` sorts by name, `src/db.rs:888-894`).
- **`repo` must be the same string `runs.repo` holds**, so `run_id` joins and a later reader can match a run to its retro: `Run::create` canonicalizes the repo path (`src/run.rs:282 canonical`). `paths::repo_root` (`src/paths.rs:114`) returns `git rev-parse --show-toplevel`; canonicalize it the same way before using it as the key. `runs.feature` is the `"001"` text; `retros.feature` is an `INTEGER` (prd §9), so compare by number, never by string.
- **The store path**: `Run::db_path(&paths::data_dir())` (`src/run.rs:269`, `src/paths.rs:20`). The harness points `FACTORY_DATA_DIR` at a temp dir (`tests/harness/mod.rs:452`), so `docs retro` in a fixture never touches the owner's store.
- **`score` has two writers in an order that matters**: closeout runs the polish worker (`src/closeout.rs:167`) before `ship` (`:541`). From slice 10 on, the polish worker sets `score` with `docs retro NNN --set score=N` (prd §9: "is how the polish worker and the owner set `score`"), so a row can exist with only a score before closeout writes the twelve numbers. The upsert must therefore never clear a stored `score` when the incoming row has none. And `--set` on a feature with no row must insert one.
- **The number types are fixed by prd §9**: `gate_minutes`, `build_minutes` and `polish_minutes` are `REAL NULL`; the other nine are `INTEGER NULL`; `score` is `INTEGER NULL`. They match `src/cli/report.rs:541-560 Report` (`f64` minutes, `u64` counts) and `src/closeout.rs:50 RETRO_KEYS` (the twelve, in column order).
- **Deliberate non-changes** — *(prd §9: "**`factory report` is unchanged**". `src/cli/report.rs:543 Report.score` stays "The owner's; never computed" and `KEYS` (`:64`) is untouched; `tests/metrics.rs:284`, `:418`, `:503`, `:656` keep passing unchanged. prd §9 says `factory retro` "may print `score`"; it is left out, since nothing asks for it yet. prd §15: "Not handled: `Owner review` counts as `retros` columns" and "Not handled: retro numbers as columns on `runs`, or a key-value `retros`". The closeout write path, `fill_retro` and `RETRO_KEYS`' removal are slice 10.)*

## Tasks

### 1. The `retros` table at schema version 3 — high — blocked by: —
**Source**: §9 FR-19: "**A `retros` table, one row per `(repo, feature)`, wide columns**: `repo TEXT`, `feature INTEGER`, `slug TEXT`, `shipped_at TEXT`, `run_id TEXT NULL REFERENCES runs(id)`, `score INTEGER NULL`, the twelve `RETRO_KEYS` in that order, `gate_minutes`, `build_minutes` and `polish_minutes` as `REAL NULL`… and the other nine as `INTEGER NULL`, `written_at TEXT`; primary key `(repo, feature)`. **`SCHEMA_VERSION` becomes 3**…; `migrate()` gains the `CREATE TABLE IF NOT EXISTS`, nothing else, so a v2 store opened by the new binary gains the table on first open and a v3 store opened by the 003 binary still works."

`src/db.rs`: `SCHEMA_VERSION: i64 = 3`. Append to `SCHEMA`:

```sql
CREATE TABLE IF NOT EXISTS retros (
  repo              TEXT    NOT NULL,
  feature           INTEGER NOT NULL,
  slug              TEXT    NOT NULL,
  shipped_at        TEXT,
  run_id            TEXT    REFERENCES runs(id),
  score             INTEGER,
  gate_runs         INTEGER,
  gate_red          INTEGER,
  gate_minutes      REAL,
  ci_cycles         INTEGER,
  slices            INTEGER,
  checkpoints       INTEGER,
  build_minutes     REAL,
  polish_minutes    REAL,
  out_tokens        INTEGER,
  cache_read_tokens INTEGER,
  start_context     INTEGER,
  peak_context      INTEGER,
  written_at        TEXT    NOT NULL,
  PRIMARY KEY (repo, feature)
);
```

Fix `src/db.rs:984` to `SCHEMA_VERSION` and `tests/run.rs:277-287` to list `retros`.

**Acceptance:** a unit test in `src/db.rs` builds a v2 store by hand, as the v1 test at `:964` builds a v1 one: today's `SCHEMA` text without `retros`, every guarded column, version rows 1 and 2, and one `runs` row. `Db::open` on it gives `schema_version() == 3`, `tables()` contains `retros`, the `runs` row reads back unchanged, and a second `Db::open` leaves exactly three `schema_version` rows. A second test opens a fresh store, then runs the PRD-003 `migrate` body against it (the `SCHEMA` text before this slice, then `INSERT OR IGNORE` of version 2) and asserts no error.

### 2. `RetroRow`, the upsert and the reader — high — blocked by: 1
**Source**: §9 FR-19: "the `retros` row exists for the two things `runs` cannot hold, a `score` and the imported pre-v3 retros that have no run." §6 FR-16: "those are written with `INSERT OR REPLACE` keyed on `(repo, feature)`, and a re-run after an undo lands on the same rows." §8 FR-18: "New: … `retro_row` (FR-19)."

`src/db.rs`: `pub struct RetroRow { repo: String, feature: u32, slug: String, shipped_at: Option<String>, run_id: Option<String>, score: Option<i64>, gate_runs: Option<i64>, gate_red: Option<i64>, gate_minutes: Option<f64>, ci_cycles: Option<i64>, slices: Option<i64>, checkpoints: Option<i64>, build_minutes: Option<f64>, polish_minutes: Option<f64>, out_tokens: Option<i64>, cache_read_tokens: Option<i64>, start_context: Option<i64>, peak_context: Option<i64>, written_at: String }`, deriving `Debug, Clone, PartialEq, Serialize`. `pub fn upsert_retro(&self, row: &RetroRow) -> Result<()>`: `INSERT INTO retros (…) VALUES (…) ON CONFLICT(repo, feature) DO UPDATE SET` every column to `excluded.<col>`, except `score = COALESCE(excluded.score, retros.score)`. `written_at` is `clock::now_rfc3339()`, set inside `upsert_retro` so callers cannot forget it. `pub fn retro(&self, repo: &str, feature: u32) -> Result<Option<RetroRow>>` and `pub fn set_retro_score(&self, repo: &str, feature: u32, slug: &str, score: i64) -> Result<()>`, which inserts a row with every number `NULL` when none exists, else updates `score` and `written_at` only.

The trap: prd §6 says `INSERT OR REPLACE`. A literal `INSERT OR REPLACE` deletes the old row first, so closeout's write at ship (slice 10) would erase the `score` the polish worker set minutes earlier (Current code). `ON CONFLICT … DO UPDATE` with the `COALESCE` gives the same idempotence §6 wants and keeps the score. For `migrate` the result is the same as a replace: its rows carry the frontmatter `score`, and a re-run writes the same values.

**Acceptance:** unit tests: `upsert_retro` twice with the same row leaves one row equal in every column but `written_at`; `set_retro_score(…, 4)` on an empty table inserts a row with `score 4` and every number `None`; then `upsert_retro` with `score: None` and twelve numbers keeps `score 4` and stores the numbers; `upsert_retro` with `score: Some(3)` sets 3; `gate_minutes 2.23` reads back as `2.23` and `16` as `16.0`.

### 3. `factory docs retro NNN [--set score=N]` — high — blocked by: 2
**Source**: §9 FR-19: "`factory docs retro NNN` prints the row; `factory docs retro NNN --set score=4` is how the polish worker and the owner set `score` (the tool writes the DB; the worker does not)." Acceptance: "`factory docs retro 001 --set score=4` updates the row and refuses `score=high`."

`src/docs/retro.rs` (new) and one `retro` arm in `src/cli/docs.rs`'s `DocsCli` match. `pub fn show(root, feature) -> Result<Option<RetroRow>>` and `pub fn set_score(root, feature, value: &str) -> Result<RetroRow>`. Both resolve `NNN` (number, slug or path) through slice 01's board scan over `features/` and `archive/`, so `slug` comes from the folder name. A feature that does not exist exits 2. The key is the canonical repo root (Current code) and the store is `Run::db_path(&paths::data_dir())`. `show` prints one `key: value` line per column in table order, `repo` to `written_at`, with an empty value for `NULL`, as `factory report` prints its keys. With no row it prints `no retro row for PRD-NNN` on stderr and exits 1. `--set` takes exactly `score=<integer>`. Any other key exits 2 with "only `score` can be set". A value that does not parse as an integer exits 2 with "score must be an integer", and the store is not opened. On success it prints the row as `show` does.

**Acceptance:** `tests/docs_retro.rs` (new) on `Fixture::v3`: `factory docs retro 001` exits 1 before any write; `retro 001 --set score=4` exits 0 and prints `score: 4`; a second `retro 001` prints the same lines; `retro 001 --set score=high` exits 2, and a read through `Db::retro` still holds 4; `--set gate_runs=3` exits 2; `retro 999` exits 2; the store file is under the fixture's `FACTORY_DATA_DIR`.

### 4. `factory report` is unchanged — medium — blocked by: 1
**Source**: §9 FR-19 Acceptance: "a store created by the PRD-003 binary (`SCHEMA_VERSION` 2) opened by the new binary has `retros` and a `schema_version` row of 3, and `factory report` on an old run prints what it printed before."

No code change: this task is the test that proves none was needed. `tests/metrics.rs`: a test builds a v2 store by hand with one finished run and its workers, the same shape task 1 uses. It runs `factory report <run>` and `factory report <run> --json` against it, opens the store, and asserts `retros` exists, the version is 3, and both outputs are byte-identical to a snapshot taken from the same store before the open. Take the snapshot by running the report through a `Db` opened on a copy made before the migration.

The trap: `factory report` itself opens the store through `Db::open`, which migrates. "Before" must be read from a copy that the new binary never opened, or the test compares the v3 output with itself.

**Acceptance:** the test above passes, and `tests/metrics.rs:284`, `:418`, `:503`, `:656` pass unchanged.

### 5. Verify — medium — blocked by: 1, 2, 3, 4
`cargo test` (the inner loop; the full gate runs at this slice's checkpoint)

**Acceptance:** inner loop green, one commit.

### 6. Prove — at the checkpoint — blocked by: 5
Rows from [`prd.md`](prd.md) § QA plan: the store half of the **FR-19** row: "open a v2 store with the new binary: `retros` exists, version 3, `factory report` unchanged; … `retro 001 --set score=4`". Copy the owner's real store (`~/.local/share/factory/factory.db`, PRD-003 schema) into a temp dir, never the original. Point `FACTORY_DATA_DIR` at the copy. Run `factory report <a PRD-003 run>` with the installed binary and with the branch binary and `diff` the two. Then run `sqlite3 factory.db 'SELECT max(version) FROM schema_version; .schema retros'`, and in `Fixture::v3` run `factory docs retro 001 --set score=4`, `--set score=high` and `sqlite3 … 'SELECT feature, score FROM retros'`. The closeout half of the row (the row's twelve numbers equal FR-25's section, `retro.md` has no frontmatter) is slice 10's.

**Acceptance:** Progress holds the empty `diff`, the `3` and the `.schema` text, both `--set` transcripts with exit codes 0 and 2, and the read-back row with `score` 4.
