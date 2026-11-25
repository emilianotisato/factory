# Slice 03 — Local CI closeout, skip-ci commits, and live status

**Source**: [`prd.md`](prd.md) → §5 (FR-3, Rust half). **Depends on**: 01 _(reads `CiMode` and `RunState.ci_mode`; touches no files in common with slice 02 or slice 04)_
**Created**: 2026-11-25

**Slice**: Implements the `local` CI closeout path in the driver: `ship()` and `refresh_numbers()` append ` [skip ci]` to their commit subjects, `closeout::run()` skips `watch_ci()` after opening the draft PR and emits `EventKind::CiLocal { head }`, and every status/live consumer (`events.rs`, `live/timeline.rs`, `live/label.rs`, `live/gate.rs`, `live/now.rs`, `cli/status.rs`) renders `"local"` cleanly without changing `factory wait`'s `finished: <pr_url>` output.

## Current code / environment

- **`src/closeout.rs:119-131`**: `polish` → `ship` → `open_pr` → `watch_ci`. Effective CI mode is `run.state().ci_mode.or(cfg.ci.mode).unwrap_or(CiMode::Remote)`.
- **`src/closeout.rs:1004-1008` (`ship`) & `:1222-1226` (`refresh_numbers`)**: Build subjects via `commit::message(board, "docs", &format!("ship PRD-{feature}"))` and `commit::message(board, "docs", "refresh numbers")`. `commit::message` (`src/commit.rs:148-165`) truncates at 50 chars; `docs(001): ship PRD-001 [skip ci]` is 33 chars and `docs(001): refresh numbers [skip ci]` is 36 chars.
- **`src/events.rs:208-236`**: CI variants (`CiGreen`, `CiRed`, `CiRerun`, `CiFixSpawned`, `CiWaiting`, `CiNoChecks`). `Event` flattens `EventKind` with `#[serde(tag = "kind", rename_all = "snake_case")]` (`:342-348`), so `EventKind::CiLocal { head: String }` serializes with `"kind": "ci_local"`.
- **`src/live/timeline.rs:38`** & **`src/live/label.rs:11-179`**: Both matches on `EventKind` are exhaustive (no wildcard `_`).
- **`src/live/gate.rs:191-202`**: `ci(ctx)` maps `EventKind` to state string (`"green"`, `"red"`, `"waiting"`, `"noChecks"`, add `"local"`) and `head` (`Some(head.clone())`).
- **`src/live/now.rs:190-236`**: **Critical trap caught by adversarial pass**: adding `CiLocal` to `is_ci` (`:228-236`) alone causes the match at `:193-201` to hit `_ => {}` and fall through to `:203-217`, which matches `PrOpened` and falsely reports `"CI checks to register on the PR"` while `phase == Phase::Closeout`. `ci_wait` must **explicitly return `None`** on `EventKind::CiLocal { .. }` (e.g. `EventKind::CiLocal { .. } => return None,` inside `:193-201`, AND include `CiLocal` in `is_ci`).
- **`src/cli/status.rs:283-298`**: `ci(events, pr, over)` maps the newest CI verdict to `"green"` / `"red"` / `"local"` / `"none"` / `"pending"`, exposed on `runs[0].ci` in `factory status --json` (`:227`).
- **`src/cockpit.rs:86-88, :115`**: `RunFinished` prints `finished: <pr_url>`, pinned verbatim by `tests/cockpit.rs:226-240`. `CiLocal` must hit `_ => return None` at `:115` so `factory wait` stays byte-identical.
- **Deliberate non-changes**:
  - *prd §5 & §10*: do **not** run a second clean-worktree gate before push; do **not** amend an already-committed `ship` subject on a resume-at-`pr` path.
  - *prd §5*: `/deliver` §3 skill wording lives in slice 04 (`skills/deliver/SKILL.md`).

## Tasks

### 1. Add `EventKind::CiLocal` and wire `live` and `status` consumers — high — blocked by: —
**Source**: §5 FR-3: `EventKind::CiLocal { head }` serialized as `ci_local`, wired through `timeline`, `label`, `gate`, `now`, and `cli/status`.

`src/events.rs`, `src/live/timeline.rs`, `src/live/label.rs`, `src/live/gate.rs`, `src/live/now.rs`, `src/cli/status.rs`.
- Add `CiLocal { head: String }` to `EventKind` in `src/events.rs` (next to `CiNoChecks`), and add a round-trip case to the serde test in `src/events.rs`.
- `src/live/timeline.rs`: map `K::CiLocal { .. } => "info"`.
- `src/live/label.rs`: map `K::CiLocal { head } => format!("CI local (remote skipped on {})", short(head, 7))`.
- `src/live/gate.rs`: map `EventKind::CiLocal { .. } => Some("local")` in `state`, and include `EventKind::CiLocal { head }` in `head`.
- `src/live/now.rs`: add `EventKind::CiLocal { .. }` to `is_ci` **and** add `EventKind::CiLocal { .. } => return None,` inside `ci_wait`'s first match (`:193-201`) so `ci_wait` never falls through to `PrOpened`.
- `src/cli/status.rs`: map `EventKind::CiLocal { .. } => Some(Some("local"))` in `ci()`.

**The trap**: Forgetting the early `return None` in `src/live/now.rs:193-201` makes `factory live --json` claim the run is waiting for CI checks to register during closeout even after `ci_local` is emitted. Add a unit test in `src/live/now.rs` (or cover it in `tests/closeout.rs`) verifying `ci_wait` returns `None` when `Phase::Closeout` has `PrOpened` followed by `CiLocal`.

**Acceptance:** `cargo test` compiles cleanly with exhaustive matches and a test proves `ci_wait` returns `None` on `Phase::Closeout` + `PrOpened` + `CiLocal`, `status --json` reports `"ci": "local"`, and `live --json` reports `ci.state == "local"`. **Fails if removed**: dropping the early `return None` in `ci_wait` causes `ci_wait` to return `Some((_, "CI checks to register on the PR"))`.

### 2. Local closeout path in `src/closeout.rs` (`[skip ci]` + skip `watch_ci`) — high — blocked by: 1
**Source**: §5 FR-3: `[skip ci]` on `ship` and `refresh_numbers` commits; skip `watch_ci` and emit `CiLocal` in `Local` mode; `Remote` unchanged.

`src/closeout.rs`.
- Add helper `fn ci_mode(run: &Run, cfg: &Config) -> CiMode { run.state().ci_mode.or(cfg.ci.mode).unwrap_or(CiMode::Remote) }`.
- In `ship()` (`:1004-1008`): when `ci_mode(run, cfg) == CiMode::Local`, use subject `&format!("ship PRD-{feature} [skip ci]")` (thread `cfg` or `local: bool` into `ship` and `push`/`refresh_numbers`).
- In `refresh_numbers()` (`:1222-1226`): when `local`, use subject `"refresh numbers [skip ci]"`.
- In `closeout::run()` (`:128-130`): when `ci_mode(run, cfg) == CiMode::Local`:
  - Skip `watch_ci` completely.
  - Read `let head = ci::head(&repo)?;` (only when `run.state().pr_url.is_some()` or unconditionally on HEAD before returning `Done`), log `ci: local mode — remote CI skipped at {}; mark PR ready for review (or push) to run required merge checks` (`short(&head)`), emit `EventKind::CiLocal { head }` (idempotently: skip emit if `CiLocal` already logged for this run), and return `Ok(Outcome::Done)`.
- Keep `open_pr()` unchanged (still pushes and opens/refreshes the draft PR).

**The trap**: `push()` calls `refresh_numbers()` right before `git push` (`:1183-1184`). If `[skip ci]` were only added to `ship()` and not `refresh_numbers()`, a refreshed `retro.md` commit would become HEAD without `[skip ci]` and trigger GitHub Actions. Both commits must carry `[skip ci]`.

**Acceptance:** Two tests in `tests/closeout.rs`:
- `local_ci_skips_watch_stamps_skip_ci_and_reports_local_status`: runs `factory run 001 --ci local --foreground` through closeout and asserts: (1) `git log -1 --pretty=%s` ends with `[skip ci]`; (2) `fx.gh_log()` contains `pr create --draft` and zero `pr checks` calls; (3) `events.jsonl` has `ci_local` and zero `ci_waiting`/`ci_green`/`ci_no_checks`; (4) `factory status <id> --json` has `runs[0].ci == "local"`, `factory live <id> --json` has `ci.state == "local"`, and `factory wait <id>` prints `finished: <pr_url>`. **Fails if removed**: reverting the `Local` branch calls `gh pr checks`, drops `[skip ci]` from `git log -1`, and reports `"ci": "green"`.
- `remote_ci_keeps_plain_ship_subject_and_watches_checks`: runs with `--ci remote` and asserts `git log -1 --pretty=%s` does **not** contain `[skip ci]` and `fx.gh_log()` includes `pr checks`. **Fails if removed**: unconditionally appending `[skip ci]` fails the subject assertion.

### 3. Verify — medium — blocked by: 2
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q`

**Acceptance:** inner loop green, one commit.

### 4. Prove — at the checkpoint — blocked by: 3
`prd.md` §8, the **FR-3** row (Rust half): `local_ci_skips_watch_stamps_skip_ci_and_reports_local_status` and `remote_ci_keeps_plain_ship_subject_and_watches_checks`. Both are `run` rows, proven below the UI: the checkpoint runs them and keeps the transcript. (`deliver_skill_handles_local_ci_mode` belongs to slice 04.)

**Acceptance:** after the checkpoint, the transcript shows both tests green.
