# Slice 04 — Status JSON body/symlink safety and factory live fixes

**Source**: [`prd.md`](prd.md) → §6 (FR-4). **Depends on**: — _(independent of Slices 01–03 and 05)_
**Created**: 2026-11-27

**Progress**: checkpoint prove: `cp-1/FR-4/transcript.txt`; leak fix: dynamic block bodies and ids in `tests/status_and_live_hardening.rs`.

**Slice**: Enriches `status --json` open questions with full `body` from `log.jsonl` and `CloseoutBlocked.phase`, guards `prd.md` reads against symlinks/FIFOs/unbounded files in BOTH `board::scan_folder_on` and `status::prd_title`/`feature_folder`, fixes `live::gate::ci` to report `waiting` (clearing stale `red`/excerpt) after `PrOpened`/`CiFixCommitted`/`CiRerun`, fixes `live::snapshot` to hash `rev` *after* `cap` using a 16-byte placeholder and drop newest timeline events when `--since` is set, and canonicalizes lane/repo roots in `live::workers::Paths::of`.

## Current code / environment

- **`cockpit::Question` & `cli::status`**:
  - `src/cockpit.rs:215-268`: `Question { id, at, n, text, answer }`. `CloseoutBlocked` currently ignores `phase` (`:241-243`).
  - `src/cli/status.rs:185-187` (`run_json` calls `Docs::status` BEFORE `prd_title`), `:235-275` (`questions`, `feature_folder`, `prd_title`), `:917` (unit test `QuestionJson { .. }` literal — update with `phase: None, body: None`).
  - `src/docs/board.rs:169-203` (`scan_folder_on`: `:186` calls plain `fs::read_to_string(dir.join("prd.md"))`, which hangs on a symlinked FIFO!).
  - **Authorized test**: `tests/cockpit.rs::status_json_carries_the_board_and_what_the_owner_owes` (`:322-332`) asserts exact JSON for `BLOCK-1` (which has a `log.jsonl` body). `ASK-1` (`:401-410`) has no `log.jsonl` body, so `#[serde(skip_serializing_if = "Option::is_none")]` keeps `:401-410` green untouched!
- **`live::gate::ci`**: `src/live/gate.rs:182-230` — only matches `CiGreen | CiRed | CiWaiting | CiNoChecks | CiLocal`, leaving `state = "red"` and stale `excerpt` after `CiFixCommitted` / `CiRerun` / `PrOpened`.
- **`live::snapshot` & `cap`**: `src/live/mod.rs:89-117, :158-206` — currently sets `"rev": ""`, computes `v["rev"] = rev(&v)` BEFORE `cap(&mut v)`, and `cut_timeline` always drains from the front (`..drop`).
- **`live::workers::Paths::of`**: `src/live/workers.rs:82-105` — pushes only lexical `lanes::dir(...)` and `ctx.state.repo`.
- **Deliberate non-changes** — *(prd §11 Non-goals)*:
  - Do NOT add a duplicate `rev` field on `status --json`.
  - Keep `at: "closeout"` on `CloseoutBlocked` questions; put `phase` in its own optional field (`#[serde(skip_serializing_if = "Option::is_none")]`).
  - Do NOT change `decisions.count` semantics, `evidence[].stable`, or `log.jsonl` parsing count.

## Tasks

### 1. Enrich `cockpit::Question` with `body`/`phase` and guard `prd.md` reads against symlinks/FIFOs — high — blocked by: —
**Source**: §6 FR-4: "`status --json` & symlink-safe bounded reads (`src/cockpit.rs`, `src/cli/status.rs`, `src/docs/board.rs`)".

`src/cockpit.rs`, `src/cli/status.rs`, `src/docs/board.rs`, `tests/cockpit.rs`.
1. In `src/cockpit.rs` (`:215-268`):
   - Add `#[serde(skip_serializing_if = "Option::is_none")] pub phase: Option<String>` and `#[serde(skip_serializing_if = "Option::is_none")] pub body: Option<String>` to `Question`.
   - In `question(...)`, populate `phase: Some(phase.clone())` for `EventKind::CloseoutBlocked { phase, .. }` (`None` for others), and `body: None`.
2. Add a shared helper `pub(crate) fn read_regular_capped(path: &Path, max_bytes: u64) -> Option<String>` (in `src/docs/board.rs`, reused by `src/cli/status.rs`):
   - Call `let meta = fs::symlink_metadata(path).ok()?;` — return `None` unless `meta.file_type().is_file()` (this rejects symlinks, FIFOs, sockets, and device nodes BEFORE opening!).
   - Open the file and read at most `max_bytes` (`256 * 1024`) via `std::io::Read::take(max_bytes)`, converting UTF-8 cleanly.
3. Use `read_regular_capped(&dir.join("prd.md"), 256 * 1024)` in BOTH `src/docs/board.rs:186` (`scan_folder_on`) AND `src/cli/status.rs:272-275` (`prd_title`). In `src/cli/status.rs:243-270` (`feature_folder`), use `e.file_type().is_ok_and(|t| t.is_dir())` (non-following!) and `fs::symlink_metadata(&dir).is_ok_and(|m| m.file_type().is_dir())`.
4. In `src/cli/status.rs` (`questions`, `:235-240`): accept `feature_dir: Option<&Path>`, read `log.jsonl` via `log::entries` (only when `symlink_metadata(log::path(dir)).is_ok_and(|m| m.file_type().is_file())`), and for each `BLOCK-n` question set `q.body` to the matching block entry's trimmed non-empty body. Update the `QuestionJson` literal at `src/cli/status.rs:917`.
5. Update authorized test `tests/cockpit.rs::status_json_carries_the_board_and_what_the_owner_owes` (`:322-332`) to expect the `BLOCK-1` `"body"` field (do NOT touch any other test in `tests/cockpit.rs`!).

**The trap**: Remember `run_json` calls `Docs::status` (`board::scan_folder_on`) BEFORE `prd_title`! If `board.rs:186` still uses `fs::read_to_string`, a FIFO symlinked at `prd.md` hangs `Docs::status` before `prd_title` ever runs! Guard BOTH places!

**Acceptance:** `status --json` includes `body` on open blocks and `phase` on `CloseoutBlocked`, and completes immediately with `title: null` on a feature whose `prd.md` is a symlink to a FIFO (`mkfifo`).

### 2. Fix `live::gate::ci` waiting state, `live::snapshot` post-cap `rev` & `--since` cap direction, and canonical lane paths — high — blocked by: —
**Source**: §6 FR-4: "`factory live` CI `waiting`, `--since` cap, post-cap `rev`, and canonical lane paths (`src/live/gate.rs`, `src/live/mod.rs`, `src/live/workers.rs`)".

`src/live/gate.rs`, `src/live/mod.rs`, `src/live/workers.rs`.
1. In `src/live/gate.rs` (`ci`, `:182-230`):
   - Scan `ctx.events` in reverse: when `!ctx.state.phase.is_over() && ctx.state.phase == Phase::Closeout`, match `EventKind::PrOpened { .. } | EventKind::CiFixCommitted { .. } | EventKind::CiRerun { .. }` BEFORE any older `CiRed`/`CiGreen`/`CiWaiting`/`CiNoChecks`/`CiLocal` as `Some("waiting")` — and when that pending-wait state wins, return empty `checks = []` and `excerpt = Value::Null` (never stale red checks/excerpt!).
2. In `src/live/mod.rs`:
   - In `snapshot` (`:89-117`): initialize `"rev": "0000000000000000"` (16 chars), run `redact::all(&mut v);`, run `cap(&mut v, opts.since.is_some());`, THEN set `v["rev"] = Value::String(rev(&v));`!
   - Update `cap(v: &mut Value, drop_newest_timeline: bool)` (`:158-176`) and `cut_timeline(v: &mut Value, over: usize, drop_newest: bool)` (`:194-206`): when `drop_newest` is `true`, pop elements from the back (`items.pop()`) until `freed >= over`; when `false`, drain from the front (`items.drain(..drop)`) as today. Update the existing unit test calls in `src/live/mod.rs:288, 304` to pass `false`.
3. In `src/live/workers.rs` (`Paths::of`, `:82-91`): for each lexical root pushed (lane dir and `ctx.state.repo`), also push `std::fs::canonicalize(&r)` when `Ok(c)` and `c != r`.

**The trap**: `"rev": "0000000000000000"` MUST be 16 chars BEFORE `cap` runs so replacing it with the 16-char hex digest after `cap` adds 0 bytes and strictly preserves `render(&v).len() <= CAP`!

**Acceptance:** Unit tests in `src/live/mod.rs`, `src/live/gate.rs`, and `src/live/workers.rs` (plus integration test in Task 3) prove post-fix `ci.state == "waiting"` with `excerpt == null`, `--since` keeps oldest unread events under `CAP`, `rev` stays unchanged when truncated evidence changes and `render(&v).len() <= CAP`, and canonicalized symlink lane paths strip cleanly.

### 3. Add integration test in `tests/status_and_live_hardening.rs` — high — blocked by: 1, 2
**Source**: §6 FR-4 Acceptance & §9 QA plan FR-4 row.

`tests/status_and_live_hardening.rs` (new file — keep free of plan-id strings so `factory leaks` stays clean!).
Add `status_json_and_live_fixes_body_symlink_ci_waiting_cap_rev_and_lane_labels` proving:
1. `status --json` includes `body` on an open `BLOCK-1` and `phase: "ci"` on `CloseoutBlocked { phase: "ci", .. }`.
2. Creating a FIFO (`libc::mkfifo`) outside the repo and symlinking `docs/features/001-x/prd.md` to that FIFO lets `factory status --json` exit `0` immediately (within 2s, no hang!) with `title: null`.
3. `factory live --json` on a closeout run with `CiRed` followed by `CiFixCommitted` reports `ci.state == "waiting"`, `ci.checks == []`, `ci.excerpt == null`.
4. `factory::live::snapshot` with `since: Some(0)` over `CAP` keeps `render(&v).len() <= CAP`, retains seq `1` (the oldest unread event, dropping newest), and changing only an evidence file that `cap` truncates does NOT change `v["rev"]`.
5. `Paths::relative` (or `factory live --json` with a symlinked `FACTORY_DATA_DIR`) strips a canonicalized worker path to its lane-relative path.

**Acceptance:** `cargo test --test status_and_live_hardening` passes; reverting any of the 5 fixes causes the test to fail.

### 4. Verify — medium — blocked by: 3
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 5. Prove — at the checkpoint — blocked by: 4
**Source**: §9 QA plan FR-4 row.
- **FR-4**: run `cargo test --test status_and_live_hardening status_json_and_live_fixes_body_symlink_ci_waiting_cap_rev_and_lane_labels -- --nocapture` and keep the transcript. Expected: passes proving `status --json` body/phase + FIFO non-hang, post-fix `ci.state == "waiting"`, `--since` cap + post-cap `rev`, and symlink lane-relative labels.

**Acceptance:** after the checkpoint, the FR-4 row has transcript evidence in this slice's **Progress**.

**Code-review fixes (2026-11-27).**
- `src/docs/board.rs` / `src/docs/state.rs`: capped regular-file reads for `prd.md` and `state.toml`; invalid UTF-8 at cap boundary truncates safely.
- `tests/status_and_live_hardening.rs`: FIFO check uses a 2s child timeout; waiting-owner wait asserts; temp dir cleanup.
