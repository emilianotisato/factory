# Slice 02 — Lanes clean and run-start build-cache trimming

**Source**: [`prd.md`](prd.md) → §5 FR-3, §12 QA plan. **Depends on**: 01 (reuses `refuse_if_open` and edits `src/lanes.rs`, `src/cli/lanes.rs`, `tests/lanes.rs` after slice 01).
**Created**: 2026-11-26

**Progress**: `clean_above_gb` + docs check validation; `lanes clean [--dry] [--days N]` with nested dedup, live `main/target/lane-*` exclusion, `FACTORY_CARGO_SWEEP` direct invoke; `ensure_all` run-start trim when no open run. Inner loop 5 runs / 1 red / ~12 min (`collapsible_if`, `strip_prefix`→`Result` on edition 2024, dirty-tree preflight). Deliberate red: dropping live-target exclusion failed `lanes_clean_…` on stale `lane-1` line. QA FR-3 left for checkpoint.

**Slice**: Adds `[lanes] clean_above_gb` parsing and validation, implements `factory lanes clean [--dry] [--days N]` (deduplicating nested `CARGO_TARGET_DIR` paths, preserving live declared `main/target/lane-*` directories, invoking `cargo-sweep sweep --time <days>` directly with removal fallback), and enforces `[lanes] clean_above_gb` trimming during `ensure_all` at fresh run start when no other open run holds the repo's lanes. Leaves gate memory caps / reaping to slice 03 and `land_lane` / respawn lane retention to slice 04.

## Current code / environment

- **`src/docs/project.rs:100-121` (`seed`)**: reads `[lanes] seed` from `docs/project.toml`. Add `pub fn clean_above_gb(root: &Path) -> Result<Option<f64>>` beside it, returning `Some(gb)` for positive integer/float values.
- **`src/docs/check.rs:212-279` (`read_project`)**: validates `[docs]` keys; add validation for `[lanes] clean_above_gb` (if present, must be a positive integer or float `> 0.0`, else emit `E-PROJECT-INVALID`).
- **`src/cli/lanes.rs:12-37, :186-199` (`LanesVerb`, `lanes`, `ensure_all`)**: currently has `Up` and `Prune` (and slice 01's `refuse_if_open`). `ensure_all` runs at fresh `factory run` start (`src/cli/run.rs:136`) before any worker spawns (and is skipped on resume at `:84-102`).
- **Nested `CARGO_TARGET_DIR` trap**: this repo's own `docs/reference/lanes/lane-N.md` sets `CARGO_TARGET_DIR=target/lane-N`, which sits inside `<lane>/target`. Simply canonicalizing paths does NOT detect that `<lane>/target/lane-1` is a descendant of `<lane>/target` — sort shortest-first and drop any candidate that `starts_with` an already-kept candidate so sizes aren't double-counted and removals don't error on already-deleted children!
- **Live vs stale `main/target/lane-*` trap**: a lane file could declare an absolute `CARGO_TARGET_DIR=<main>/target/lane-1`; exclude any canonical path matching a `CARGO_TARGET_DIR` declared in a current `docs/reference/lanes/lane-*.md` before treating `main/target/lane-<digits>` as stale!
- **`cargo-sweep` invocation trap**: `tests/harness/mod.rs:587-596` does NOT include `~/.cargo/bin` on `PATH`, so running `cargo sweep` fails in tests. Invoke `cargo-sweep` directly as `<cargo-sweep> sweep --time <days>` with `CARGO_TARGET_DIR=<dir>`, resolving the binary from `FACTORY_CARGO_SWEEP` (if `"off"`, skip sweep and remove directly; if non-empty path, run that executable directly; if unset, search `PATH` for `cargo-sweep`).

## Tasks

### 1. Add `[lanes] clean_above_gb` parsing and `docs check` validation (`src/docs/project.rs`, `src/docs/check.rs`) — high — blocked by: —
**Source**: §5 FR-3: "Add `pub fn clean_above_gb(root: &Path) -> Result<Option<f64>>` in `src/docs/project.rs` reading `[lanes] clean_above_gb` (positive integer or float in GiB). Validate `[lanes] clean_above_gb` in `src/docs/check.rs` (`read_project`): if present and not a positive number (`> 0.0`), emit `E-PROJECT-INVALID`."

- `src/docs/project.rs`: add `pub fn clean_above_gb(root: &Path) -> Result<Option<f64>>` reading `table.get("lanes").and_then(|l| l.get("clean_above_gb"))`, accepting positive `Integer` or `Float` (`> 0.0`) and returning `None` if absent or non-positive.
- `src/docs/check.rs:228-269`: in `read_project`, when `table.get("lanes")` is `Some(toml::Value::Table(lanes))` and `lanes.get("clean_above_gb")` is `Some(v)`, check that `v` is an `Integer` or `Float` strictly `> 0.0` (and finite); otherwise emit `E-PROJECT-INVALID`.
- Add unit tests in `src/docs/project.rs` and `src/docs/check.rs` proving valid positive ints/floats (`15`, `0.5`) parse and pass `docs check`, while `0`, `-1`, `"15G"`, and `true` emit `E-PROJECT-INVALID`.

**Trap**: `docs/project.toml` already allows `[lanes]` in `src/config.rs:431` without warnings — verify `the_driver_tables_and_docs_share_the_file_without_warnings` stays green.

**Acceptance:** Unit tests prove `clean_above_gb` parses positive ints/floats and `docs check` rejects non-positive or non-numeric values with `E-PROJECT-INVALID`.

### 2. Implement build-cache discovery, `factory lanes clean [--dry] [--days N]`, and run-start trimming (`src/lanes.rs`, `src/cli/lanes.rs`) — high — blocked by: 1
**Source**: §5 FR-3: "Add `LanesVerb::Clean { #[arg(long)] dry: bool, #[arg(long, default_value_t = 7)] days: u32 }` ... `--dry` is read-only and allowed anytime; non-dry `clean` refuses (exit `1`, `"clean refused: ..."`) if the repo has an open run ... discover existing build-cache directories ... deduplicate nested descendants ... exclude any path that canonicalizes to a `CARGO_TARGET_DIR` declared by a current `lane-*.md` ... invoke `cargo-sweep sweep --time <days>` directly (honoring `FACTORY_CARGO_SWEEP`) ... in `ensure_all`: if `clean_above_gb` is `Some` AND `run::list_open(data, main)?.is_empty()`, trim any oversized lane and log a warning rather than failing `ensure_all` on removal errors".

- `src/lanes.rs`:
  - Add helper `pub fn dir_size(path: &Path) -> u64`: iteratively walks `path` using `symlink_metadata()` (never following symlinks!), summing regular file sizes (ignoring unreadable entries safely).
  - Add helper `pub fn cache_dirs(main: &Path, lane_dir: &Path, n: u32) -> Vec<(PathBuf, bool)>` (bool = is_cargo_target):
    - Collects existing candidate paths: `CARGO_TARGET_DIR` from `lanes::load(main, n)` (resolved relative to `lane_dir` if relative, marked `is_cargo_target = true`), `lane_dir.join("target")` (`true`), plus `lane_dir.join("node_modules/.cache")`, `lane_dir.join(".next/cache")`, `lane_dir.join("vendor/.cache")` (`false`).
    - Canonicalizes each existing path, sorts by component count / path length shortest-first, and drops any candidate whose canonical path `starts_with` an already-kept canonical path (preserving `is_cargo_target = true` if either parent or nested child is a Cargo target dir).
  - Add helper `pub fn stale_main_lane_targets(main: &Path) -> Vec<PathBuf>`: scans `main.join("target")` for direct subdirectories named `lane-<digits>` whose canonical path does NOT match any `CARGO_TARGET_DIR` declared in `docs/reference/lanes/lane-*.md` (or any lane dir's own `CARGO_TARGET_DIR`).
  - Add helper `pub fn trim_cache_dir(lane_dir: &Path, dir: &Path, is_cargo: bool, days: u32, force_remove: bool) -> Result<u64>` (returns freed bytes):
    - Computes `before = dir_size(dir)`.
    - If `is_cargo && !force_remove`, checks `FACTORY_CARGO_SWEEP`: if `"off"`, skips sweep; if non-empty path, runs `<path> sweep --time <days>` in `lane_dir` with `CARGO_TARGET_DIR=dir`; else looks up `cargo-sweep` on `PATH` and runs `cargo-sweep sweep --time <days>`. If sweep succeeds, computes `after = dir_size(dir)` and returns `before.saturating_sub(after)`.
    - If not cargo, `force_remove`, or sweep unavailable/failed: removes `dir` via `fs::remove_dir_all(dir)` and returns `before`.
- `src/cli/lanes.rs`:
  - Add `LanesVerb::Clean { #[arg(long)] dry: bool, #[arg(long, default_value_t = 7)] days: u32 }` and wire dispatch in `lanes()`.
  - Implement `pub fn clean(data: &Path, main: &Path, dry: bool, days: u32, out: &mut dyn Write) -> Result<i32>`:
    - If `!dry && refuse_if_open(data, main, "clean", out)? { return Ok(1); }` (`--dry` stays allowed during an open run!).
    - Measures each lane's `cache_dirs` and `stale_main_lane_targets(main)`.
    - If `dry`: prints human-readable sizes per lane and stale main dirs + total reclaimable, deletes nothing, exits `0`.
    - If `!dry`: removes stale `main/target/lane-*`, trims each lane's `cache_dirs`, prints freed bytes per lane and total freed, exits `0`.
  - In `ensure_all` (`:187-199`): after ensuring lanes, if `project::clean_above_gb(main)?` is `Some(limit_gib)` and `run::list_open(data, main)?.is_empty()`: compute `threshold = (limit_gib * 1024.0 * 1024.0 * 1024.0) as u64`. For each lane, if total `cache_dirs` bytes > `threshold`, trim each cache dir (trying sweep first, then `force_remove = true` if total still exceeds `threshold`), printing `"  lane-{n}  trimmed build cache (... > ...)"` or logging a warning line if removal errors (never failing `ensure_all`).
- `tests/lanes.rs` (purely additive new test): add `lanes_clean_reports_trims_and_enforces_clean_above_gb_at_run_start` proving:
  1. `factory lanes clean --dry` reports deduplicated sizes when `lane-1.md` sets `CARGO_TARGET_DIR=target/lane-1` (inside `<lane>/target`), lists stale `main/target/lane-9`, excludes a live `main/target/lane-1` declared by `lane-1.md`, and deletes nothing.
  2. `factory lanes clean` removes `main/target/lane-9` (keeping live `main/target/lane-1`), invokes a stub `FACTORY_CARGO_SWEEP` script with `sweep --time 7` and `CARGO_TARGET_DIR`, and removes `target/` directly when `FACTORY_CARGO_SWEEP=off`.
  3. Non-dry `factory lanes clean` refuses with exit `1` (`clean refused`) during an open run, while `factory lanes clean --dry` succeeds (`exit 0`).
  4. `[lanes] clean_above_gb = 0.00001` trims an oversized lane during `ensure_all` while leaving a lane below the threshold untouched.

**Trap**: `format_bytes` / human-readable output should handle small byte sizes cleanly (B / KiB / MiB / GiB) so tests with tiny fixtures print clear non-zero sizes instead of `"0.0 GiB"`.

**Acceptance:** `lanes_clean_reports_trims_and_enforces_clean_above_gb_at_run_start` passes; reverting nested deduplication, live `main/target/lane-*` exclusion, open-run refusal, or `ensure_all` trimming fails the corresponding assertion.

### 3. Verify — medium — blocked by: 1, 2
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 4. Prove — at the checkpoint — blocked by: 3
**Source**: §12 QA plan row FR-3:
- FR-3: `run lanes_clean_reports_trims_and_enforces_clean_above_gb_at_run_start` (evidence: test transcript)

**Acceptance:** after the checkpoint, the test runs under `cargo test` with full gate green and transcript recorded in Progress.
