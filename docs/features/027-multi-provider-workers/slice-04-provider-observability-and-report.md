# Slice 04 — Provider observability in `factory.db`, `runs.knobs`, and `factory report`

**Source**: [`prd.md`](prd.md) → §2, §7 (FR-5), §9, §11. **Depends on**: 03 (needs multi-provider workers from slice 03 to prove a real Codex-only run with no `claude` on `PATH` and mixed-provider `factory report`)
**Created**: 2026-11-25

**Progress**:
- Built: `workers.provider TEXT` joins `db::COLUMNS` (33), `WorkerRow.provider` reads it, and `Db::set_worker_provider` is called by `worker::record` right after `insert_worker`. `cli::run::probe_provider_versions` replaces `claude_version`: it runs `<bin> --version` through `provider::run_bounded` (5 s, own process group, `CLAUDECODE` dropped) only for `ResolvedWorkers::distinct_providers` and warns per provider that does not answer. A run parked on the worker ask probes nothing, and `cli::run::set_worker_knobs` rewrites `workers`, `provider_versions` and `claude_version` once `worker_answer` resolves them. `config::knobs` takes the resolved workers, `--worker` specs, the resolved review, lanes and `BTreeMap<Provider, String>` versions. Per role it writes `provider`, `model`, `effort` (null when none), `source` and the optional `provider_source`/`effort_source` (`config::role_sources`). It adds `review` and `provider_versions` and keeps `claude_version` (null when Claude is unused). In `factory report`, `knobs_text` prints `<role> <provider>:<model>[/<effort>] <src>` and sorted `<provider> <version>` pairs when present, else the legacy shape. `--per-worker` lines and JSON carry `provider`. `--recount` skips the subagent lookup for a non-Claude worker with no note.
- Inner loop: 2 runs, 1 red, 3 min — caught: the per-worker line pinned in `tests/metrics.rs` (updated to the provider-qualified line, AD-11).
- Seen red: `a_store_without_worker_providers_gains_the_column_and_reads_none` — `workers.provider` dropped from `COLUMNS`: `has_column` false. `codex_only_run_without_claude_on_path_records_knobs_and_reports_cleanly` — Claude always probed: stderr `claude --version: not on PATH`; `set_worker_provider` call removed: worker row `provider` was `None`. `report_knobs_renders_multi_provider_and_legacy_rows` — legacy rows rendered as `claude:<model>`: the legacy assertion failed.
- Deviations: `src/provider.rs` touched for `Ord` and the shared bounded runner (AD-8); version shown as the first digit-led token (AD-9); knobs before the worker answer and source attribution (AD-10); `tests/metrics.rs` per-worker pin (AD-11).
- Prove (CP-2): FR-5 — `codex_only_run_without_claude_on_path_records_knobs_and_reports_cleanly`, `report_knobs_renders_multi_provider_and_legacy_rows`.

**Slice**: Adds the additive `workers.provider` SQLite column (`src/db.rs`), records each worker's `provider` on spawn (`src/worker.rs`), replaces unconditional `claude --version` probing with `probe_provider_versions` over only the distinct providers used by the run (`src/cli/run.rs`), enriches `runs.knobs` JSON with per-role `provider`, `review`, and `provider_versions` (`src/config.rs`), and updates `factory report` (`knobs_text`, `--per-worker`, and `--recount`) to display providers cleanly while keeping legacy pre-PRD-027 DB rows readable (`src/cli/report.rs`, `tests/knobs.rs`).

## Current code / environment

- **Database schema**: `src/db.rs:28-51` (`workers` table), `:115-148` (`COLUMNS` array of 32 guarded `ALTER TABLE ADD COLUMN` migrations), `:554-580` (`insert_worker`), `:690-760` (`workers()` query and `WorkerRow`).
- **Run start & knobs**:
  - `src/cli/run.rs:217-285` (`record_start`, `claude_version`) currently spawns `Command::new("claude").arg("--version")` unconditionally and warns on stderr if `claude` is absent.
  - `src/config.rs:398-430` (`knobs`) emits `workers.<role>` (`model`, `effort`, `source`, optional `effort_source`), `budget`, `ci`, `lanes`, `factory_version`, `claude_version`.
- **Report**:
  - `src/cli/report.rs:188-265` (`WorkerLine` / `--per-worker`) prints `worker <id> <role>... model_id <m>`.
  - `src/cli/report.rs:732-768` (`knobs_text`) prints `<role> <model>/<effort> <src>` and ends with `claude <ver>`.
  - `src/cli/report.rs:835-845` (`recount`) reads subagents via `metrics::read_subagents` unconditionally — must skip `~/.claude` lookup when `w.provider` is a non-Claude provider!
- **Pinned test**: `tests/knobs.rs:59-159` asserts `knobs_line.contains("build opus/high default")` and `knobs_line.ends_with("lanes 1 · factory ... · claude 2.1.296")`. PRD-027 §2 & §7 explicitly authorize updating `tests/knobs.rs` to assert the provider-qualified format while adding a unit test proving legacy pre-PRD-027 rows without `provider` / `provider_versions` still render identically.
- **Deliberate non-changes (PRD-027 §11)**:
  - Keep `"claude_version"` in `runs.knobs` populated when `claude` is among the run's providers (or `null` when not used) for JSON backward compatibility.

## Tasks

### 1. Add `workers.provider` column in `src/db.rs` and persist it in `src/worker.rs` — high — blocked by: —
**Source**: §7 FR-5: "Append `(\"workers\", \"provider\", \"TEXT\")` to `db::COLUMNS` (`src/db.rs:115-148`), add `pub provider: Option<String>` to `WorkerRow`, and persist `provider.as_str()` in `insert_worker`".

`src/db.rs`, `src/worker.rs`.
- Add `("workers", "provider", "TEXT")` to `db::COLUMNS` (updating array length `32 → 33`), add `pub provider: Option<String>` to `WorkerRow`, include `provider` in the `workers()` `SELECT`, and pass `provider: &str` into `insert_worker` (or add `set_worker_provider` called right after `insert_worker`, keeping any direct `insert_worker` unit tests clean).
- In `src/worker.rs` (`record`), persist `cfg.provider.as_str()` on the worker row.
- Verify `Db::open` on a pre-PRD-027 SQLite file (without `workers.provider`) migrates cleanly and reads existing rows with `provider: None`.

**Acceptance:** Unit test in `src/db.rs` opens a pre-migration DB lacking `workers.provider`, verifies `has_column("workers", "provider") == true`, verifies old rows read `provider: None`, and verifies newly inserted workers return `provider: Some("codex")` (fails if migration or column persistence is missing).

### 2. Probe only used provider versions, enrich `runs.knobs`, and update `factory report` — high — blocked by: 1
**Source**: §7 FR-5: "In `config::knobs`, include `provider`... `review`... `provider_versions`... Replace unconditional `claude_version()` with `probe_provider_versions(used_providers, err)`... Update `knobs_text`, `--per-worker`, and `recount`".

`src/config.rs`, `src/cli/run.rs`, `src/cli/answer.rs`, `src/cli/report.rs`, `tests/knobs.rs`.
- In `src/config.rs` (`knobs`):
  - Accept resolved `workers` (and sources), `review` summary, `lanes`, and `provider_versions: &BTreeMap<Provider, String>`.
  - Under `workers.<role>`, include `"provider": w.provider.as_str()`, `"model": w.model`, `"effort": w.effort` (null when `None`), `"source": ...`, optional `"provider_source"` / `"effort_source"`.
  - Include `"review": {"provider": ..., "model": ..., "source": ...}` and `"provider_versions": provider_versions`.
  - Keep `"claude_version": provider_versions.get(&Provider::Claude)` (`null` when `claude` is not configured for the run).
- In `src/cli/run.rs`:
  - Replace unconditional `claude_version` with `probe_provider_versions(providers: &[Provider], err: &mut dyn Write) -> Result<BTreeMap<Provider, String>>` that runs `<bin> --version` with a 5 s bounded timeout (reusing process-group timeout logic so lazy shims never hang!) **only** for the distinct providers in `providers` (and scrubs `CLAUDECODE` for Claude). When a run starts parked on an unset worker ask (no resolved workers yet), probe none and refresh `runs.knobs` when `worker_answer` resolves them in `src/cli/answer.rs`.
- In `src/cli/report.rs`:
  - `knobs_text`:
    - When `w["provider"]` is a string `p`, format `<role> <p>:<model>/<effort> <src>` (omitting `/<effort>` when `effort` is null/empty, e.g. Cursor). When `w["provider"]` is absent (pre-PRD-027 row), keep legacy `<role> <model>/<effort> <src>`.
    - When `k["provider_versions"]` is a non-empty object, format sorted `<provider> <first-token-of-version>` pairs (e.g. `claude 2.1.296 · codex 0.172.0`); otherwise fall back to legacy `claude <ver>`.
  - `--per-worker` (`WorkerLine`): include `w.provider` when `Some` (e.g. `worker 1 build codex slice 01 ...`).
  - `recount` (`:835-845`): when `w.provider.as_deref()` is `Some(p)` and `p != "claude"`, skip `metrics::read_subagents` and set `sub = None` with no unreadable warning.
- Update `tests/knobs.rs` to assert the provider-qualified `knobs_text` and add `codex_only_run_without_claude_on_path_records_knobs_and_reports_cleanly` (in `tests/knobs.rs`) + `report_knobs_renders_multi_provider_and_legacy_rows` (in `src/cli/report.rs`).

**The trap**: Remember `src/cli/report.rs:1033-1047` has existing unit tests on `knobs_text` with legacy JSON (no `"provider"`, no `"provider_versions"`)! Those MUST continue passing unchanged — that is the exact backward-compatibility check for pre-PRD-027 DB rows.

**Acceptance:** Tests `codex_only_run_without_claude_on_path_records_knobs_and_reports_cleanly` (in `tests/knobs.rs`) and `report_knobs_renders_multi_provider_and_legacy_rows` (in `src/cli/report.rs`) pass:
- Runs a Codex-only feature on a `PATH` containing **no** `claude` binary, asserting zero `claude --version` warnings on stderr, `knobs["provider_versions"]["codex"]` populated, `knobs["claude_version"].is_null()`, `workers` rows storing `provider == Some("codex")`, and `factory report --per-worker` printing `codex:gpt-5.4/high` and `codex` on worker lines (fails if `claude --version` is still called unconditionally or `workers.provider` is not stored).
- `knobs_text` renders both multi-provider rows (`build codex:gpt-5.4/xhigh user · fix cursor:grok-4.7-high user · ... · codex 0.172.0`) and legacy pre-PRD-027 rows (`build opus/high default · ... · claude 2.1.296`) cleanly (fails if either new or legacy formatting breaks).

### 3. Verify — medium — blocked by: 1, 2
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q`

**Acceptance:** inner loop green, one commit.

### 4. Prove — at the checkpoint — blocked by: 3
Drive this row from [`prd.md`](prd.md) §9 QA plan at CP-2 (final checkpoint — re-proves FR-1 through FR-5):
- **FR-5**: run `cargo test codex_only_run_without_claude_on_path_records_knobs_and_reports_cleanly` and `cargo test report_knobs_renders_multi_provider_and_legacy_rows`.

**Acceptance:** FR-5 row (and every prior row re-proved at final CP-2) has passing test transcripts captured in checkpoint evidence and recorded in **Progress**.
