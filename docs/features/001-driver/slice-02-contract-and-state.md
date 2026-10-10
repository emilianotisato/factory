---
slice: 02
status: done
lane: 1
depends_on: [1]
---

# Slice 02 — Contract and state: docs.mjs wrapper, run dir, config, schema (FR-1, FR-8)

**Source**: [`prd.md`](prd.md) → §3 FR-1 bullets 1, 2 and 4 (run state, preflight by code, `factory stop` from the first slice); §10 FR-8 bullet 1 (SQLite tables `runs`, `workers`, `gates`, `events`); §11 FR-9 bullet 2 (`~/.config/factory/config.toml`); §15 OPEN-12; §20 "`status --json` keys" and "`preflight --json` keys". **Depends on**: 01 (the fixture is the only thing the wrapper can be tested against).
**Created**: 2026-10-09 · **Status**: pending

**Progress**: Built: `src/docs.rs` (`Docs::locate` prefers the vendored `.docs/bin/docs.mjs`, then the skill's copy; `status`, `preflight` with its exit, `gate`, `gate_inner`, `log_merge`, `ship`, `render_write`; every struct `#[serde(default)]`; errors name the command and quote stderr); `src/run.rs` + `src/events.rs` (`RunId` `<NNN>-<yyyymmdd>-<4 hex>`, `state.json` through a tmp-and-rename, `events.jsonl` with `seq`/`ts`/`kind` and the payload flattened, `emit` writes the line and the `events` row, `find_live` by `/proc/<pid>/stat`, `find_resumable`); `src/cli/run.rs` (live → exit 1 naming `factory stop <id>`; resumable → `run_resumed` and the board, no preflight; `refuse[]` → the tool's `  ✗` lines and `refused: N blocker(s)`, exit 1, no run dir; `ask[]` → a run parked in `waiting-owner` with one `ask_opened` ASK-n each, exit 0; otherwise `ensure_branch` per `branch.{default,feature,featureExists}`, one `judge_noted` per `judge[]` line (printed as `  · …`), `run_started`, ready slices as `<NN> <file>`); `src/config.rs` (defaults 30M / opus / high, unknown keys warned on stderr); `src/db.rs` (rusqlite bundled, `schema_version` + the four tables, idempotent, run row at create and on every phase change, `finished_at` on a terminal phase); `src/cli/stop.rs`; `src/clock.rs`, `src/paths.rs`, `src/error.rs`; 12 tests in `tests/{docs,run}.rs`. Inner loop: 8 `cargo test` runs, 3 red, under 1 min (37 s wall in total, 8 s of it the first compile; the suite runs in 5 s warm); the red runs caught nothing in the code and three wrong test expectations — the clock test's epoch constants were off by a day, `docs log merge` on the fixture exits 2 ("no agent-log.md … nothing to merge into"), and the fixture's `judge[]` holds four lines with the unstruck-OPEN-1 line first; four `cargo clippy --all-targets -- -D warnings` passes, the first of which caught a derivable `Default` in `config.rs`, fixed. Blocked once: built to FR-1 as first written, `ask[]` and `judge[]` both parked the run, and `docs preflight` never returns an empty `judge[]` (the fixture gets four lines), so the clean fixture parked and task 3's and task 5's clean-fixture acceptances could only be proven at lib level — BLOCK-201. The owner chose "Deliver should be unattended": only `ask[]` parks, `judge[]` is logged. Resolved by one branch in `src/cli/run.rs` and a `judge_noted {text}` kind in `src/events.rs`; `ask_opened` lost its `source` field (AD-205). The clean fixture now goes through the binary end to end (`tests/run.rs::run_on_the_clean_fixture_starts_notes_the_judge_lines_then_resumes`: branch `feat/fixture`, `judge_noted` ×4, `run_started`, slices 01 and 02 printed, runs row `starting`, then the live-pid refusal and the dead-pid resume), and a `lanes: 1` fixture proves an `ask[]` line still parks (`run_parks_on_an_ask_line_and_creates_no_branch`). Deviations: no `preflight_refused` event — a refusal creates no run dir, so there is nowhere to write one; the refuse text goes to stdout only. When a run parks on `ask[]`, its `judge[]` lines are not noted; slice 04 notes them when it continues the run, if it wants them. `tests/cli.rs` (slice 01) lost its `run` and `stop` entries from the not-built list, which `tests/run.rs` replaces. A resume adopts the pid and re-applies `--budget` when given. Decisions AD-201 to AD-204 merged into `agent-log.md`, AD-205 in `log-slice-02.md`. Prove: the FR-1 refuse row left for the checkpoint (`tests/run.rs::run_refuses_a_dirty_tree_with_no_run_dir_and_no_spawn` is the transcript to attach).

**Slice**: After this slice `factory run NNN` does everything a run does before its first worker: runs preflight through `docs.mjs`, routes refuse/ask/judge by code, creates or switches to the feature branch, creates the run directory with its state file and `events.jsonl`, opens the SQLite store with its schema, reads the config, and refuses a second run on a live repo. It then reads the board once, prints the ready slices and exits; slice 03 replaces that exit with the loop. `factory stop` exists as a request file the loop will honour. Nothing is spawned here. This is the contract slice: every later slice reads the board through `docs.rs`, writes events through `events.rs` and rows through `db.rs`, so their shapes land before the checkpoint.

## Current code / environment

- **The board JSON is the contract**: `status --json` keys (§20) `number, slug, dir, kind, status, reviewed, gate{declared,status,detail,candidates}, qaPlan, harnessLine, harness, rubric, sliced, allDone, slices[], checkpointEvery, checkpoints[], unverified[], checkpointDue, next, ready[], building[], lanes, freeLanes[], spawnable, blocked[], openBlocks[], openOpens[], log{decisions,flagged,decide,blocks}, retro, branch`, built at `docs.mjs:1372-1416`. `preflight --json` keys `feature, pass, refuse[], ask[], judge[], ok[], branch{current,default,feature,alt,featureExists}`; exit 1 refused, 2 asks pending (`docs.mjs:2066`). Commands: `status` `:2055`, `log` `:2061`, `preflight` `:2066`, `render` `:2069`, `gate` `:2081`, `ship` `:2084`. Deserialize with `serde(default)` on every optional array so a tool upgrade that adds a key does not break the driver.
- **Which `docs.mjs` to run**: the repo's `.docs/bin/docs.mjs` when present, else `~/.claude/skills/feature-docs/bin/docs.mjs`, the same preference `docs.mjs:1926-1927` applies. The fixture always has the vendored one; the dogfood projects have neither yet, so the fallback is the live path for FR-11.
- **Preflight refuses a resume with a parked lane**: `docs.mjs:1703` refuses on any open BLOCK. That is why §3 says preflight "gates only the start of a new run" and a resume "reads `status --json` directly and skips preflight". A run id on disk for this repo and feature means resume.
- **Today's preflight routing is prose**: `deliver/SKILL.md:15-68`, with `:34` "Any `ask` line → stop and put it to the owner … 'proceed as-is' always an option". The ask and judge items become cockpit questions here; the answer path (`factory answer ASK-n`) is slice 04's, the relay is slice 10's.
- **Where state lives**: §15 OPEN-12 "`~/.config/factory/`, `~/.local/share/factory/`". Tests must not touch the owner's dirs: honour `FACTORY_DATA_DIR` and `FACTORY_CONFIG_DIR` first, `XDG_DATA_HOME`/`XDG_CONFIG_HOME` second, the defaults last. The fixture from slice 01 already sets both.
- **Config keys the PRD names**: `budget_weighted_tokens` default 30M (§11); "Models and effort come from `~/.config/factory/config.toml` defaults for now" (§19). Nothing else; do not invent keys.
- **Crates checked 2026-10-09**: rusqlite 0.40.2 (`bundled` feature so no system SQLite), serde_json 1.0.151, toml 1.1.8, clap 4.6.7.
- **Deliberate non-changes**: *(prd §3 "Rejected: reimplementing preflight in Rust. That is PRD-002's job." The wrapper shells out; no routing logic is re-derived from the board.)* *(prd §19 "Not handled: two runs on the same repo at once … `factory run` refuses when a run is live for that repo." A live run is a state file whose `pid` is alive; a dead pid with an unfinished run is a resume.)*

## Tasks

### 1. `docs.rs`: the `docs.mjs` wrapper — high — blocked by: —
**Source**: §2: "`factory` reads them through `docs.mjs status --json` and `preflight --json`". §3 FR-1: "Preflight is `docs.mjs preflight --json`, routed by code".

`src/docs.rs`: `Docs::locate(repo_root)`; `status(nnn) -> Status`, `preflight(nnn) -> Preflight` (with the exit code), `gate() -> String`, `gate_inner() -> String`, `log_merge(nnn)`, `ship(nnn)`, `render_write()`, each running `node <tool> <args>` in the repo root and returning stdout, stderr and exit status. Typed structs for the two JSON shapes above, `#[serde(default)]` everywhere the key may be absent. Errors name the command and quote the tool's stderr.

**Acceptance:** against the slice 01 fixture, `status(1)` deserializes with three slices, `ready.len() == 2`, `spawnable == 2`, `branch` non-empty; `preflight(1)` has `pass == true`; a fixture with an uncommitted file makes `preflight` return exit 1 and a non-empty `refuse`.

### 2. Run directory, state file, `events.jsonl` — high — blocked by: —
**Source**: §3 FR-1: "Run state lives in `~/.local/share/factory/runs/<id>/` (state file + `events.jsonl`), the slice board stays the source of truth for what is built." §9 FR-7: "events it missed are replayed from `events.jsonl`."

`src/run.rs` and `src/events.rs`: `RunId` = `<NNN>-<yyyymmdd>-<4 hex>`; `runs/<id>/state.json` with `id, repo, feature, branch, phase, pid, started_at, updated_at, budget_weighted_tokens, stop_requested`; `phase` enum `starting | building | checkpoint | waiting-owner | paused | closeout | finished | failed`. `events.jsonl` append-only, one object per line: `{seq, ts, kind, ...payload}`; `kind` strings are a Rust enum serialized as `snake_case` (`run_started`, `preflight_refused`, `ask_opened`, `slice_spawned`, … added by later slices). Every event is also inserted into the `events` table (task 5) by the same `emit()`; nothing writes one without the other. `Run::find_live(repo, feature)` finds a state file for this repo and feature whose `pid` is alive (`kill -0`).

**Acceptance:** `emit` twice → two lines with `seq` 1 and 2 and two `events` rows; `find_live` returns the run while its pid lives and `None` after the process is gone.

### 3. `factory run NNN` up to the first board read — high — blocked by: 1, 2
**Source**: §3 FR-1: "`refuse[]` exits 1 with the tool's own text. `ask[]` and `judge[]` go to the cockpit (FR-7) as one question each; a run never starts on an unanswered one. Branch creation follows `branch.feature` from the same JSON. A resume of a live run reads `status --json` directly and skips preflight." *(FR-1 corrected 2026-10-09 by the owner, BLOCK-201: only `ask[]` parks; `judge[]` is logged as `judge_noted` and never waited on.)* §3 acceptance: "A refused preflight prints the refusal and exits 1 with no worker spawned." §3: "`factory run NNN` on a feature with an existing run id continues from the board."

`src/cli/run.rs`. New run (no run dir for this repo+feature): `preflight`; `refuse[]` non-empty → print each line as the tool printed it, exit 1, no run dir; `ask[]` non-empty → create the run, emit one `ask_opened` event per item with ids `ASK-1…n` and the item text, phase `waiting-owner`, print them, exit 0 (the answer path is slice 04); `judge[]` lines → one `judge_noted` event each, printed, never parking the run *(FR-1 corrected 2026-10-09 by the owner, BLOCK-201: only `ask[]` parks; `judge[]` is logged as `judge_noted` and never waited on.)*; `pass` → branch: on `branch.default` with `featureExists == false` → `git switch -c <branch.feature>`; `featureExists == true` → `git switch <branch.feature>`; already on it → nothing; on any other branch → refuse with a message naming the two acceptable branches (today's `/deliver` rule, `deliver/SKILL.md:15-68`). Then create the run, emit `run_started`, read `status --json`, print the ready slices as `<number> <file>`, exit 0. Existing run dir: skip preflight, emit `run_resumed`, read the board, same print. A live run (pid alive) → exit 1 naming the run id and `factory stop <id>`.

**Acceptance:** fixture with a dirty tree → exit 1, stdout holds the tool's refuse text, no `runs/` entry, no stub call recorded; clean fixture → run dir exists, `events.jsonl` has `run_started`, stdout lists slices 01 and 02, the branch is `feat/fixture`; a second call while the first process is alive (hold it with `delay_ms` later, here with a fake live pid in the state file) → exit 1 naming the run; a third call after the pid dies → same run id, `run_resumed`.

### 4. `config.toml` — medium — blocked by: —
**Source**: §11 FR-9: "`budget_weighted_tokens` in `~/.config/factory/config.toml`, overridable with `--budget`. Default 30M weighted". §19: "Models and effort come from `~/.config/factory/config.toml` defaults for now."

`src/config.rs`: `Config { budget_weighted_tokens: u64 = 30_000_000, workers: { build: Worker, fix: Worker, checkpoint: Worker, polish: Worker } }` with `Worker { model: String, effort: String }`; defaults model `opus`, effort `high` for every role until a retro says otherwise. Missing file → defaults; unknown keys → a warning line, not an error. `--budget <n>` on `factory run` overrides and is stored in `state.json`.

**Acceptance:** no file → 30M; a file with `budget_weighted_tokens = 5` → 5; `factory run 001 --budget 7` writes 7 into `state.json`.

### 5. SQLite store and schema — high — blocked by: —
**Source**: §10 FR-8: "SQLite at `~/.local/share/factory/factory.db`. Tables: `runs`, `workers` (role, slice, session_id, model, effort, input, cache_write, cache_read, output, thinking, cost_usd, num_turns, duration_ms, terminal_reason, outcome, peak_context), `gates` (source: driver or worker, checkpoint or slice, cmd, red, minutes), `events`. … Written as events happen, not reconstructed."

`src/db.rs`, rusqlite `bundled`. `schema_version` table; `runs(id PK, repo, feature, branch, started_at, finished_at, phase, budget_weighted_tokens)`; `workers(id PK, run_id, role, slice, session_id, model, effort, input, cache_write, cache_read, output, thinking, cost_usd, num_turns, duration_ms, terminal_reason, outcome, peak_context, started_at, ended_at)`; `gates(id PK, run_id, source, kind, cmd, red, minutes, at, worker_id NULL)`; `events(id PK, run_id, seq, ts, kind, payload JSON)`. `tool_calls` is slice 08's. Migrations are idempotent (`CREATE TABLE IF NOT EXISTS` plus the version row). The run row is written when the run dir is created and updated on every phase change.

**Acceptance:** opening the store twice applies the schema once; `factory run` on the fixture leaves one `runs` row with `phase = 'starting'` and the `run_started` event row.

### 6. `factory stop <run>` as a request — medium — blocked by: 2
**Source**: §3 FR-1: "a runaway run needs `factory stop <id>`, which must exist from the first slice." §9 FR-7: "`factory stop <run>` ends it cleanly after the in-flight workers exit."

`src/cli/stop.rs`: sets `stop_requested: true` in `state.json`, emits `stop_requested`; the loop (slice 03) checks it between spawns and finishes in-flight workers before exiting with phase `finished`. With no live pid, `stop` sets phase `finished` itself and says so.

**Acceptance:** on a run with no live pid, `factory stop <id>` sets `finished` and emits the event; on a run with a live pid, the flag is set and the phase is unchanged.

### 7. Verify — medium — blocked by: 1–6
`cargo test`. The full gate runs at this slice's checkpoint.

**Acceptance:** inner loop green, one commit.

### 8. Prove — at the checkpoint — blocked by: 7
Rows from [`prd.md`](prd.md) § QA plan: the second half of **FR-1** ("refused preflight exits 1 with no spawn", evidence: test transcript). The kill-and-resume half is slice 03's. Driven at the checkpoint on the fixture; the transcript goes in this slice's **Progress**.

**Acceptance:** the FR-1 refuse transcript is attached, showing exit 1, the tool's text, and an empty `calls/` directory.
