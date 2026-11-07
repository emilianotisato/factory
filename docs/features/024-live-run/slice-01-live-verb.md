# Slice 01 — The read-only live verb: envelope, events and timeline

**Source**: [`prd.md`](prd.md) → §3 FR-1, §7 FR-5 (the event kinds and `timeline[]`), §4 FR-2 (`run_waiting.reason`), §2. **Depends on**: — (the spine: every later slice fills one section file this one lays out, and needs the event kinds it declares)
**Created**: 2026-11-07

**Progress**:
- Built: the FR-5 kinds (`gate_started`, `collect_started`, `review_pass_done`, `collect_done`, `ci_waiting`, `ci_no_checks`, `fix_empty` in both shapes) and `run_waiting.reason`, set from `park_with`'s `what`. The board's shortener moved to `EventKind::short` and now skips an empty subject, so an old `run_waiting` still reads `run_waiting`. `factory live <id> --json [--since] [--wait] [--evidence-limit]` in `src/cli/live.rs` over `src/live/{mod,now,workers,gate,evidence,timeline,redact}.rs`; `now`, `gate`, `ci` are null, `attention` is `[]`, `evidence` is `{root: null, checkpoints: [], skipped: 0, truncated: false}`, `redact::all` is the identity. `feature_folder` in `cli/status.rs` is `pub(crate)` and shared. `tests/live.rs` with stubs `tests/live/{now,workers,gate,evidence,redact}.rs`.
- `paths::data_dir()` creates the data dir when it is missing, so `live` resolves the run through the new `paths::data_dir_path()`, which only computes it.
- Inner loop: 1 run, 0 red, about 2 min (the 11 live tests ran green first, in 4 s).
- Deviation: `--wait` stats the run's two files every 100 ms and does a full reread at least once a second (AD-1, fyi).
- Prove: left for the checkpoint.
- **Proved (checkpoint 1):** FR-1, `live_answers_one_line_of_schema_1_json`, `live_writes_nothing`, `live_wait_returns_when_something_visible_changes`, `live_rev_ignores_a_growing_stream`, `live_caps_output_and_names_what_it_cut`, 5 passed — `cp-1/fr-1-live-verb.txt`

**Slice**: After this slice, `factory live <id> --json [--since] [--wait] [--evidence-limit]` prints one schema-1 JSON object built without opening `factory.db` or writing anything: the full envelope, a `timeline` with tones and labels, `decisions.count`, a minimal `workers[]` (identity, `alive`, `lastWriteAt`), `rev`, `--wait`, and the 512 KiB cap. Every new event kind of FR-5 and `run_waiting.reason` exist in `EventKind`, so the driver slices emit them and the reader slices read them in parallel. `now`, `attention`, `gate`, `ci`, `evidence` and the worker detail fields are stubs here (null or empty), each in its own file, so slices 03, 05, 06, 07 and 09 edit only their own file.

## Current code / environment

- **`Run::open` writes**: `Db::open` (`src/db.rs:348-359`) creates the data dir and migrates `factory.db` under an IMMEDIATE transaction (`:362-378`). prd §3: `live` never calls `Run::open`, `Db::open` or `cockpit::write_cursor` (`src/cockpit.rs:297-300`). Read with `run::load_state` (`src/run.rs:508`) and `events::scan` (`src/events.rs:324-343`), which returns `NotFound` as empty.
- **The unknown id**: `status --json` checks `Run::runs_dir(&data_dir).join(id).join(STATE_FILE).is_file()` and writes `no run {id}` on stderr, exit 1 (`src/cli/status.rs:116-123`). `Run::runs_dir` (`src/run.rs:219`) is a path join, no open. `cockpit::resolve` (`src/cockpit.rs:302-313`) calls `Run::open`: do not use it.
- **Liveness**: `RunState::driver_alive` / `worker_alive` (`src/run.rs:129-135`) over `/proc` (`:575-603`). Workers are `state.workers` (`WorkerRef`, `src/run.rs:139-153`: `slice`, `checkpoint`, `lane`, `role`, `session_id`, `pid`, `spawned_at`, `outcome`). The stream is `worker::stream_path(run_dir, session_id)` (`src/worker.rs:177-179`), `runs/<id>/workers/<session>.jsonl`.
- **EventKind** (`src/events.rs:17-260`): internally tagged `kind`, snake_case. `RunWaiting { blocked, asks }` at `:57-60`. `Unknown` is `#[serde(other, skip_serializing)]` (`:257-259`), so an older binary already parses the new kinds as `Unknown`. No `Eq` on the enum (`gate_run` carries a float, `:14`).
- **`RunWaiting` literals that break when the field is added**: `src/scheduler.rs:621` (the one emit, inside `park_with` `:603-627`, which has `what`), `src/cockpit.rs:531` and `src/cli/status.rs:1069` (tests). prd §4: "`run_waiting` gains `reason`, a short string (`#[serde(default)]`), set from `park_with`'s `what`". Setting it at `:621` is a one-liner and keeps `scheduler.rs` out of every later slice.
- **The board's shortener**: `short()` (`src/cli/status.rs:700-718`) picks `slice`, `n`, then the first of `block_id`, `id`, `check`, `literal`, `reason` as the subject; `last_three` (`:680-698`) skips `Unknown`. prd §2: with `reason`, the board's `last:` text for `run_waiting` changes. That is expected; a status test pinning `run_waiting` without a subject may need its expectation updated.
- **The feature folder and its log**: `feature_folder` (`src/cli/status.rs:299-322`, private) finds `docs/features/<NNN>-*` then `docs/archive/<NNN>-*`, given `Docs::open(repo).status(feature)` (`:194`). Both read only (`src/docs/mod.rs:45-70`). The log is `docs::log::path(dir)` (`src/docs/log.rs:127-128`), read with `log::entries` (`:154`). Make `feature_folder` `pub(crate)` or move it beside `cockpit`, so `live` and `status` share it (slice 05 also needs it for `body`).
- **`repo`** is printed as stored: `s.repo.display().to_string()` (`src/cli/status.rs:218`).
- **Subcommands**: `Command` enum in `src/main.rs:18-…` (`Wait`, `Status` at `:23`, `:27`; dispatch at `:93-94`). `tests/cli.rs:8-11` `SUBCOMMANDS` is a 14-entry array that `help_lists_every_subcommand` checks: add `"live"` and make it 15.
- **Hashing**: `sha2` and `regex` are already dependencies (`Cargo.toml`); no new crate is needed.
- **Test helpers to copy**: `tests/cockpit.rs` `factory_in` (`:24`), `write_live_run` (`:427`), `write_run` (`:809-840`, a run dir from a state override and event values), `status_json` (`:256`). `tests/harness/mod.rs` `Fixture` (`:128`). Tests read fixtures, never the machine's runs folder or this repo's `docs/`.
- **Deliberate non-change**: prd §1 "Left alone: `status --json` stays as PRD-023 shipped it", except the `last:` text above. prd §3: `live` never calls status's `board()` or `last_rate_limit`. prd §14: "No writes of any kind", "No `--follow` stream".

## Layout this slice sets up

So that slices 03, 05, 06, 07 and 09 can be built in parallel lanes without touching the same file:

- `src/live/mod.rs`: the args-free core. A `Ctx` (run dir, `RunState`, events, the feature folder if any, `now`) built once per read; `snapshot(ctx, opts) -> Value` that calls one function per section, then, in this order, `redact::all(&mut Value)` over the whole value, `rev`, and the cap with `truncated`. Redacting first keeps `rev` stable and caps what is printed.
- One file per section, each owning its types: `src/live/now.rs` (`now`, `attention`), `src/live/workers.rs`, `src/live/gate.rs` (`gate`, `ci`), `src/live/evidence.rs`, `src/live/timeline.rs`, `src/live/redact.rs` (identity here).
- `src/cli/live.rs`: clap args, the unknown id, `--wait`, printing.
- `tests/live.rs`: the shared helpers, plus `#[path = "live/<section>.rs"] mod <section>;` for `now`, `workers`, `gate`, `evidence`, `redact`, each file created here holding only a header comment. A plain `mod x;` in `tests/live.rs` would look for `tests/x.rs`, which Cargo also builds as its own test target.

## Tasks

### 1. The new event kinds and `run_waiting.reason` — high — blocked by: —
**Source**: §7 FR-5 "New events": `gate_started {n, attempt, cmd, log}`, `collect_started {passes, dir, reused}`, `review_pass_done {k, of, seconds}`, `collect_done {seconds, ok}`, `ci_waiting {checks}`, `ci_no_checks {minutes}`, `fix_empty {n, attempt, session_id}` and `fix_empty {round}`; §4 FR-2 "`run_waiting` gains `reason` … (`#[serde(default)]`), set from `park_with`'s `what`"; §2 "All are additive."

`src/events.rs`, `src/scheduler.rs:621`, `src/cockpit.rs:531`, `src/cli/status.rs:1069`. `fix_empty` is one variant whose fields are `Option` with `skip_serializing_if`, so a gate fix writes `n`, `attempt`, `session_id` and a CI fix writes `round`. `collect_started.dir` is `Option<String>` (a small diff has no review dir). Nothing emits the new kinds yet except `reason`.

**Acceptance:** `old_binary_skips_new_events`, a unit test in `src/events.rs` beside `an_unknown_kind_reads_as_unknown_and_an_unreadable_line_is_skipped` (`:415`): a line whose `kind` is not in the enum and carries fields (the shape a newer binary writes) scans as `Unknown`, and a `run_waiting` line without `reason` (an older binary's) parses with `reason == ""`. A round-trip test pins each new kind's JSON field names, both `fix_empty` shapes included.

### 2. The verb, read-only — high — blocked by: 1
**Source**: §3 FR-1: "Read-only, for real … never calls `Run::open`, `Db::open` or `write_cursor`", "One compact JSON object on stdout … An unknown id prints `no run <id>` on stderr and exits 1", "`--json` is required, and there is no human mode", the envelope list, "Times. Every time is UTC RFC3339", "Old layouts. A section whose files are missing is empty or null, never an error", "`decisions` is `{ count }`, counted from the feature's `log.jsonl`".

`src/main.rs`, `src/cli/mod.rs`, `src/cli/live.rs`, `src/lib.rs`, `src/live/*.rs`, `src/cli/status.rs` (share `feature_folder`), `tests/cli.rs`, `tests/live.rs`. Envelope keys in this order, camelCase: `schema` (1), `factory` (`env!("CARGO_PKG_VERSION")`), `id`, `feature`, `phase`, `repo`, `branch`, `seq`, `rev`, `at`, `now`, `attention`, `workers`, `gate`, `ci`, `timeline`, `evidence`, `decisions`, `truncated`. Here `now`, `gate`, `ci` are `null`; `attention` is `[]`; `evidence` is its empty shape. `workers[]` carries `sessionId`, `role`, `slice`, `checkpoint`, `lane`, `spawnedAt`, `alive` and `lastWriteAt` (the stream's mtime, RFC3339, or null), and leaves `job`, `doing`, `narration`, `running`, `outcome` null for slice 06.

**Acceptance:** `live_answers_one_line_of_schema_1_json` (exactly one line, parses, `schema == 1`, every envelope key present); `live_writes_nothing` (listing and mtimes of the run dir, `factory.db` and its directory equal before and after a call; build the data dir so `factory.db` exists first); `live_on_an_old_layout_run_is_empty_not_an_error` (a run dir with only `state.json`, exit 0, sections null or empty); `live_unknown_id_exits_1` (stderr `no run <id>`, stdout empty). `help_lists_every_subcommand` lists `live`.

### 3. The timeline — high — blocked by: 1, 2
**Source**: §7 FR-5: "`timeline[]`: `{ seq, ts, kind, tone, label, data }`", "`label` is factory's short sentence, extending the board's shortener", "`data` is a whitelisted subset per kind. Lists and blobs become counts", and the tone table; §3 "`--since <seq>` … Without it, the last 100."

`src/live/timeline.rs`. One `match` over every `EventKind` variant (no `_` arm, so a new kind fails to compile until it has a tone): `owner` for `ask_opened`, `run_waiting`, `run_parked`, `slice_blocked`, `checkpoint_blocked`, `closeout_blocked`, `block_answered`, `ask_answered`; `red` for `gate_run` red, `ci_red`, `worker_dead`, `merge_refused`, `landing_conflicted`, `fix_empty`, `run_failed`; `green` for `gate_run` green, `ci_green`, `slice_done`, `checkpoint_done`, `polish_done`, `pr_opened`, `shipped`, `run_finished`; `noise` for `ci_waiting`, `review_pass_done`; `info` for the rest, `unknown` included. Reuse `short()`'s subject rule for `label` (make it shared rather than copied). `data` keeps scalar fields and turns arrays into their length and nested objects into a count; `question`/`text`/`error` strings stay (redaction runs over them in slice 09).

**The trap:** `Unknown` is `skip_serializing` (`src/events.rs:257`), so `serde_json::to_value` of it fails; `EventKind::name` returns `"unknown"` for it (`:262-…`). Give it `data: {}`.

**Acceptance:** `live_timeline_tones_follow_the_table` (one fixture event per tone, including an unknown kind, each gets its tone; a red and a green `gate_run` split); `live_since_returns_only_newer_events` (`--since 3` over 6 events gives seq 4..6; no flag over 150 events gives the last 100).

### 4. `rev`, `--wait` and the cap — high — blocked by: 3
**Source**: §3 FR-1: "`rev` is a hash of the serialized snapshot minus its volatile fields: `at`, `now.idleSeconds`, `workers[].running.elapsedSeconds` and `workers[].lastWriteAt`", "`--wait <secs>`, 0 to 25, default 0. It rereads every second and returns when `rev` changes, or after `secs`. It exits 0 on timeout", "`--evidence-limit <n>`, default 200, max 300", "Output is capped at 512 KiB, cut first in evidence, then in the timeline. `truncated` names the cut sections in order."

`src/live/mod.rs`, `src/cli/live.rs`. `rev` is sha256 hex (short is fine) of the snapshot with those fields removed and `rev` itself absent. `--wait` compares against the `rev` of the first read. The cap drops evidence files from the oldest, then timeline events from the oldest, until the line fits; `truncated` is e.g. `["evidence","timeline"]`. Out-of-range flags are a clap error (exit 2). Parse `--evidence-limit` here; slice 07 applies it.

**Acceptance:** `live_wait_returns_when_something_visible_changes` (an event appended after 1 s ends a `--wait 5` within 2 s); `live_wait_times_out_with_exit_0` (`--wait 1`, nothing changes, exit 0 after ≥1 s); `live_rev_ignores_a_growing_stream` (two calls with a worker stream appended between them, same `rev`; an appended event changes it); `live_caps_output_and_names_what_it_cut` (a fixture with events past 512 KiB: output ≤ 512 KiB and `truncated` names `timeline`).

### 5. Verify — medium — blocked by: 4
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 6. Prove — at the checkpoint — blocked by: 5
prd §12 row **FR-1**: run `live_answers_one_line_of_schema_1_json`, `live_writes_nothing`, `live_wait_returns_when_something_visible_changes`, `live_rev_ignores_a_growing_stream`, `live_caps_output_and_names_what_it_cut`, and keep the transcript.

**Acceptance:** after the checkpoint, this slice's Progress names that transcript.

**Code-review fixes (2026-11-07).**

- The run id was joined onto the runs dir unchecked, so `live ../../x` read any dir with a `state.json`; an id that is not one plain path name is now `no run <id>`, exit 1.
- `data_dir_path` had been inserted under `dir`'s doc comment, so the reason `dir` absolutizes the path sat on the wrong function; each doc is back on its own item.
