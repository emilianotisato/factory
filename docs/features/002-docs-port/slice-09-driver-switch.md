---
slice: 09
status: done
depends_on: [4]
lane: 1
---

# Slice 09 — Driver switch (FR-18 core)

**Source**: [`prd.md`](prd.md) → §8 FR-18 (the library decision, the workarounds decision, the harness decision, the Acceptance), §4 FR-14 (the "driver's own writes" bullet and the `phase` bullet), §3 FR-13 ("The driver calls the same function in process"). **Depends on**: 04 (the driver calls `gate`, `gate_inner`, `preflight` and `ship`; they must run natively on v3 before the subprocess goes. Slices 01-03 come with it: `state::set`, `log::add`/`log::entries`, `check`).
**Created**: 2026-10-13 · **Status**: pending

**Size, accepted at slicing**: about 35 files, likely over 90 minutes. It stays one slice: the driver switch and the harness switch must land together, so a split leaves a near-empty first half (prd §11, settled at slicing).

**Progress**:
- **Built.**
  - **Task 1.** `Docs::open` refuses v2 with `Error::NotV3(V2_HINT)`. `status`, `preflight`, `gate`, `gate_inner` and `ship` run in process, and `ship` returns `Ship::Done` or `Ship::Refused`. Added `set*`, `log_add` and `log_entries`. Deleted `locate`, `run`, `ToolOutput`, `log_merge*` and `render_write`.
  - **Task 2.** Every slice status and lane write goes through `state::set`. The scheduler's frontmatter helpers are gone.
  - **Task 3.** Blocks, notes, asks and answers are `log.jsonl` entries, and `answers.rs` reads the log. The parked-run-first rule holds across features that each have a `BLOCK-1`.
  - **Task 4.** Fragments, fold, merge, the AD-503 workarounds and the render calls are deleted. Commits stage `state.toml` and `log.jsonl`.
  - **Task 5.** The closeout matches `Ship::Refused` and blocks in phase `polish`. The ship commit stages the feature dir.
  - **Task 6.** The harness builds the fixture with the binary. The stub `claude` writes through `$FACTORY_BIN docs set|log add`. `tests/fixture/.docs` is deleted.
  - **Task 7.** The suites run on v3.
  - **Task 8.** `factory run` on v2 exits 2 with the hint. The node scan test expects only `cli/docs.rs`.
- **Inner loop.** 3 full `cargo test` runs, 1 red, about 4 min of runtime over 11 min.
  - The red run had 87 failures in 14 suites, nearly all stale reads of fragments and `agent-log.md`.
  - The rewrite caught two driver bugs. `pr_title` took the v3 frontmatter's `# type:` comment as the H1. The polish commit named `prd.md` where `reviewed` now lives in `state.toml`. Both are fixed in `src/closeout.rs`.
  - The final run had 409 passed, 0 failed, and clippy was clean outside slice 07's `migrate.rs`.
- **Deliberate deviations.** AD-901 to AD-909 in `log-slice-09.md`:
  - `log_add` returns an `Option`.
  - `tests/docs.rs` was rewritten as task 1's tests, not deleted.
  - The markdown-parser and frontmatter unit tests went with their code.
  - The metrics repeat test changed meaning.
- **Tests deleted.** Besides the merge, clash and fragment tests task 4 names: AD-906 and AD-908.
- **CP-2 Prove, FR-18** (`evidence/cp-2-fr18.txt`): `grep -rn 'Command::new("node")' src` gives `src/cli/docs.rs:316` only, the FR-17 shim. `factory run 001` on the v2 fixture prints ``factory: docs are v2: run `factory docs migrate --dry` `` and `[exit 2]`, data dir empty. `cargo test` (the checkpoint gate run, `gates/2-2.log`): 27 suites, 410 passed, 0 failed, 1 ignored.

**Slice**: **Highest risk of the PRD.** After this slice the driver reads and writes docs in process through `crate::docs`, on v3 only. `Docs::open` refuses a v2 repo, so `factory run` exits 2 with the migrate hint before preflight. Every slice-status and lane write goes through `state::set`. Every BLOCK, ask, answer and leftover note the driver writes is a `log::add` entry. Fragments, the merge, the four `agent-log.md` workarounds and the pre-commit `render --write` calls are gone. The harness builds its fixture with the binary, the stub `claude` writes through `factory docs`, and `tests/fixture/.docs` is deleted. The suites that drive a run pass with their assertions unchanged in meaning. Left to slice 10: `fill_retro`, `RETRO_KEYS` and `retro_row` (FR-19), the `ci_cycles` bump, the worker contract prose, the pack's `log show` section, the hook exemption, `/factory-deliver`, `autonomy.md`, `pr-body.md` and the unused event variants. Task 5 names the closeout parts that cannot wait for 10.

## Current code / environment

- **The subprocess wrapper**: `src/docs.rs:18 struct Docs`, `:25 ToolOutput`, `:35-53 locate`, `:64-79 run` (the one `Command::new("node")`, `:65`). Methods: `status :109`, `preflight :117` → `(Preflight, i32)`, `gate :130`, `gate_inner :135`, `log_merge :141`, `log_merge_dry :146`, `ship :151`, `render_write :156`. Structs: `Status :175`, `OpenBlock :262`, `LogSummary :271`, `Preflight :283`. Slice 01 moved this file to `src/docs/mod.rs`; re-check the lines there.
- **`ship` returns `ToolOutput`, which goes.** `closeout.rs:553-572` reads `shipped.code`, `stderr` and `stdout` to build a BLOCK body. "Keep their signatures" cannot hold for `ship`; it needs a result that carries the refusal text (task 1).
- **Call sites, 2026-10-13**: `cli/run.rs:63,100,181,218,361,376`; `scheduler.rs:172,174,271,312,335,499,523,638`; `checkpoint.rs:144,339,831`; `closeout.rs:553,633,1836,2160`; `commit.rs:513,563`; `pack.rs:147,245-246,480-481` (and the test at `:944`); `cli/status.rs:66`; `cli/report.rs:35,910`; `retro.rs:13,274`. `Docs::locate` is called at `retro.rs:274`, `cli/status.rs:66`, `cli/run.rs:63,181`, `cli/report.rs:910`, `closeout.rs:1836`, `pack.rs:944`.
- **An `Err` exits 1, not 2.** `src/main.rs:62-65` maps every `Err` to `ExitCode::from(1)`. The trap: if `Docs::open` returns `Err` and `cli/run.rs:63` passes it up with `?`, `factory run` exits 1 and the FR-18 acceptance fails. `run` must match the v2 refusal, print the hint on stderr and return `Ok(2)`.
- **The fragment machinery** (`src/commit.rs`): `protected_files :32-38` (prd.md, agent-log.md, docs/index.md), `fragment_path :41`, `Pathspec::from :71-78` (adds the slice file and the fragment), `Fold :495`, `fold_logs :508`, `check_logs :525`, `ready_fragments :539` (AD-503 workaround at `:546`), `refusal :563`, `untitled :587`, `fragments_in :606`, `is_fragment :621`. Its tests are at `:724`, `:787` and `:946`.
- **Checkpoint** (`src/checkpoint.rs`): `agent_log :78`; AD-503 `:91-95`; pre-gate `check_logs :130`; BLOCK `append_under` at `:186` (red cap), `:284` (dead worker), `:925` (merge refusal) and `:968` (`settle_dead`); `fold_logs` and the `LogMerged` emit `:218-238`; CP detection by `docs.status` then `after.checkpoints` at `:339-340`; `commit_checkpoint :820`, whose `render_write` is at `:831` and which stages `board.dir` plus `docs/index.md`. `block_on_refusal :893`, `open_checkpoint_blocks :999-1010` (the `phase == "checkpoint"` filter at `:1002`), `park_line :1048`, `lowest_free_block :1056`.
- **Scheduler** (`src/scheduler.rs`): `set_frontmatter` lane `:387` and status+lane `:421`; leftovers into the fragment `:988-990`; protected files `:1082`; fragment skip `:1149`; dead checkpoint Traps into agent-log `:1380-1395`; rate-limit reset `:1445-1446` (`status: pending` then `remove_frontmatter_keys(["lane"])`); dead worker Traps in the fragment `:1485-1535`; `dead_block_id :1584` (seeded `slice*100+k`); `open_dead_block :1601`; block entry in the fragment `:1657`, set blocked `:1659`. Helpers: `append_under :1785`, `set_slice_status :1838`, `read_slice_status :1843`, `set_frontmatter :1863`, `remove_frontmatter_keys :1877`, `write_atomic :1912`; tests `:1935-2058`.
- **Answers** (`src/answers.rs`): `entries :209` parses `### BLOCK-n` headings; `log_files :274-297` walks `agent-log.md` and `log-*.md`; `find :299`, `find_all :306`, `open_block_of_slice :316`, `entry_text :334`, `resolve :346` (flips `open` → `resolved` and inserts `**Resolution (date):**`); tests `:387-549`. `cli/answer.rs`: `answer_block :68` (the miss message names "a log-*.md fragment or agent-log.md", `:84`), slice status read and set `:191`, `:224-226`, AD-503 `:362`, preflight AD via `append_under` `## Decisions` `:380`, `next_preflight_ad` (below 100), `agent_log :497-503`.
- **Closeout** (`src/closeout.rs`): `agent_log :98`, AD-503 `:108`, `:195`, `:301`; `ship :553`; `fill_retro :662` (slice 10); `render_write :633` then `commit::expand([board.dir, "docs/index.md"])` `:634`; PR body names agent-log `:530`; block scan and write `:1925-1978` (`append_under :1959`); Traps `:2104-2125`.
- **Events**: `events.rs:163 LogMerged`, emitted only at `checkpoint.rs:228`. Keep the variant so old run logs still parse. Stop emitting it. Slice 10 cleans up.
- **The harness** (`tests/harness/mod.rs`): `new :51` → `with_index :57`; `build :96-141` copies `.docs` (`:116`) and runs node `init`, `new`, `slice` and `render` (`:123-130`); `write_index_frontmatter :143`; `patch_prd :160`; `docs :215`, `docs_json :234`; `slice_path :275`; `slice_status :289` reads frontmatter; `chain :304` edits `depends_on` and renders; `build_env :420-461` sets `FACTORY_STUB_DIR`, `FACTORY_DATA_DIR` and `FACTORY_CONFIG_DIR`, and no binary path. Slice 01 added `Fixture::v3`, `fx.factory_docs` and `fx.factory_docs_json`.
- **The stub `claude`** (`tests/stubs/claude`, node, 734 lines): `setStatus :535` rewrites frontmatter (called at `:335`, `:400`, `:420`, `:469`); a blocked slice writes `BLOCK-${nn}01` into `log-slice-NN.md` (`:410-440`); a done slice writes `AD-${ad}` into its fragment (`:465`, `:617`); `lowestFreeBlock :578` scans agent-log and fragments; the checkpoint step appends CP-n and BLOCK into `agent-log.md` (`:243-290`); polish stamps `reviewed:` into prd.md frontmatter (`:352-367`) and writes a `· polish ·` BLOCK into agent-log (`:299-318`). The `ad:` and `prose:` clash options exist only to test the merge refusal.
- **Tests that touch logs or fragments** (`grep -c "agent-log\|log-slice\|fragment"`): commits 37, lanes 31, checkpoints 14, stubs 6, quota 5, loop 4, docs 3, closeout 2, metrics 2. `Fixture::new` is used by stubs, quota, harness, cockpit, run, commits, docs, metrics, knobs, checkpoints, worker, lanes, closeout and loop.
- **This repo stays v2 during the build** (prd §14): the run reads this repo through the installed binary's subprocess path. Nothing in this slice runs on this repo's own docs; every test uses a temp fixture.
- **Deliberate non-changes** — *(prd §8: "**No dual reader**: the driver reads v3 only." Do not keep a v2 path in `Docs`. prd §8: a hand edit of `log.jsonl` or `state.toml` "is caught by `check` at the next checkpoint, not denied". Do not add a hook deny.)*

## Tasks

### 1. `Docs::open` and the in-process library — high — blocked by: —
**Source**: §8 FR-18: "`crate::docs` becomes the library `factory docs` is built on; the driver calls it directly. `Docs::locate(repo)` becomes `Docs::open(repo)`, which refuses a v2 repo with "docs are v2: run `factory docs migrate --dry`"… `status`, `preflight`, `gate`, `gate_inner` and `ship` keep their signatures and return the same structs; `log_merge`, `render_write`, `run` and `ToolOutput` go. New: `set_slice_status`, `set_status`, `set_reviewed` (FR-13 writes), `log_add`, `log_entries` (FR-14)."

`src/docs/mod.rs`: `Docs { root }` with no `tool`. `open(repo)`: when `project::is_v3(repo)` holds, return it. Otherwise return a typed refusal (`Error::DocsV2` or similar) whose text is exactly the quoted hint. Do the same for a repo off the framework, with its own text. The methods call the slice 01-04 free functions: `board::status`, the slice 04 `preflight`, `gate` and `ship`. `set_slice_status(f, n, s)`, `set_status(f, s)` and `set_reviewed(f, date)` call `state::set`. `log_add(f, Entry) -> Result<String>` (the id) and `log_entries(f)` call `log::add` and `log::entries`. `ship` returns `Result<Ship>` with `Ship::Done` and `Ship::Refused(String)`, since `ToolOutput` goes (see Current code). Delete `locate`, `run`, `log_merge`, `log_merge_dry`, `render_write`, `ToolOutput` and `tool()`.

**Acceptance:** unit tests on `Fixture::v3`: `Docs::open` succeeds and `status("001")` equals `factory docs status 001 --json` deserialized; `set_slice_status("001", 2, "building")` then `status` shows slice 2 building; `log_add` of a `block` with `phase: checkpoint` returns `BLOCK-1`, and `status.open_blocks` lists it with that phase. On a v2 fixture, `Docs::open` errors with exactly "docs are v2: run `factory docs migrate --dry`". `ship` on a feature with a pending slice returns `Ship::Refused` with text naming the slice.

### 2. The driver's state writes go through `state::set` — high — blocked by: 1
**Source**: §3 FR-13: "`factory docs set NNN key=value…` with dotted keys (`status=ready`, `slices.3.status=building slices.3.lane=1`) is the one write path… The driver calls the same function in process (FR-18)." §8 FR-18 Today: "The driver edits slice frontmatter itself (`scheduler.rs:1796-1830`)."

Replace every frontmatter write with `docs.set_slice_status` or `state::set` on `slices.N.*`: `scheduler.rs:387` (lane), `:421` (status and lane), `:1659` (blocked), and `:1445-1446`. At `:1445-1446` the lane clear is `slices.N.lane=` (the empty-value removal from slice 01). Also `cli/answer.rs:191,224-226`. Replace each `read_slice_status` with `board.slices[n].status` from `docs.status`. Delete `set_slice_status`, `read_slice_status`, `set_frontmatter` and `remove_frontmatter_keys` from `scheduler.rs`, with their tests at `:1986-2058`. Slice files are now read-only to the driver.

**Acceptance:** `tests/lanes.rs` on v3: a rate-limited worker's slice reads back from `state.toml` as `status = "pending"` with no `lane` key, and its `slice-NN-*.md` is byte-identical before and after. `factory answer BLOCK-n` on a blocked slice leaves `slices.N.status = "building"` in `state.toml`. `grep -n "set_frontmatter\|remove_frontmatter_keys" src` returns nothing.

### 3. The driver's log writes: blocks, asks, answers and notes — high — blocked by: 1
**Source**: §4 FR-14: "The driver's own writes become log entries: a checkpoint's BLOCK (`checkpoint.rs:176`) is `kind: block`; `factory answer` writes `kind: answer` with `ref` where `answers::resolve` edits the heading today; the preflight AD entry (`cli/answer.rs:296`) is `kind: ask`; a checkpoint record is `kind: checkpoint` with `slice` = the last covered slice." §4 FR-14, added at slicing: "an optional `phase` field (`preflight | slicing | checkpoint | polish | pr | ci`) on `block`, `ask` and `checkpoint` entries… `checkpoint.rs` filters open blocks on `phase == "checkpoint"`." §4 FR-14: "Only the tool appends, and it allocates the id… the seed, the clash check and the merge refusal all go."

- **BLOCK writes become `kind: block` entries, with `phase` and `slice` set.** Covers `checkpoint.rs:186,284,968`, `scheduler.rs:1657`, and the dead-worker block at `:1485-1535`. The id is the one `log_add` returns. Delete `dead_block_id :1584` (its `slice*100+k` seed) and `lowest_free_block :1056`. The merge-refusal block at `checkpoint.rs:925` goes with the merge (task 4).
- **The preflight answer becomes one `kind: ask` entry** with `phase: preflight`, `title` and `body`. It replaces the `append_under` at `cli/answer.rs:380`. The retry check `preflight_logged` reads `log_entries` for an `ask` with the same title. Delete `next_preflight_ad`: the tool allocates the AD id.
- **`answers.rs` reads the log.** `entries`, `find`, `find_all` and `open_block_of_slice` derive `BlockEntry` from `log_entries`. A block is resolved when an `answer` with `ref = id` exists. `resolve` becomes `log_add(kind: answer, ref: id, body: text)`. `log_files` goes. `BlockEntry.path`/`start` become the log path and the line number. The miss message at `cli/answer.rs:84` names `log.jsonl`.
- **`open_checkpoint_blocks` keeps its filter**: block entries with `phase == "checkpoint"` and no answer.
- **Traps and leftovers become `kind: note` entries** with `slice` set. This covers `scheduler.rs:988-990`, `:1380-1395` and `:1485-1535`, and `checkpoint.rs` `note_leftovers`.

**The trap**: `factory answer` used to match blocks across features by heading text. Ids now restart at `BLOCK-1` per feature, so the same id is common across features. Keep the parked-run-first rule at `cli/answer.rs:97-100`, and test two features that each hold a `BLOCK-1`.

**Acceptance:** `tests/checkpoints.rs`: a red-capped checkpoint writes one `block` line with `phase: "checkpoint"`. `factory answer BLOCK-1 "x"` appends one `answer` line with `ref: "BLOCK-1"` and leaves the block line byte-identical. `open_checkpoint_blocks` is then empty. `tests/loop.rs`: a preflight answer appends one `ask` line, and a retry of the same `factory answer ASK-1` adds none. A unit test: with `BLOCK-1` open in features 001 and 002 and a run parked on 002, the answer's `ref` lands in 002's log.

### 4. Remove the fragments, the merge, the workarounds and `render_write` — high — blocked by: 2, 3
**Source**: §8 FR-18: "The four `agent-log.md` workarounds go (`commit.rs:394`, `checkpoint.rs:91`, `closeout.rs:108`, `cli/answer.rs:321`), with `fold_logs`, `Fold`, `fragment_path`, the checkpoint merge step (`checkpoint.rs:207`) and the pre-commit `render --write` calls (`checkpoint.rs:813`, `closeout.rs:633`)… The checkpoint and closeout commits stage `state.toml` and `log.jsonl` where they staged slice files, fragments, `agent-log.md` and `docs/index.md`."

At today's lines the four workarounds are `commit.rs:546`, `checkpoint.rs:91-95`, `closeout.rs:108` and `cli/answer.rs:362`. The merge step is `checkpoint.rs:218-238` and the render calls are `checkpoint.rs:831` and `closeout.rs:633`.

- **Delete from `commit.rs`**: `fragment_path`, `Fold`, `fold_logs`, `check_logs`, `ready_fragments`, `refusal`, `untitled`, `fragments_in` and `is_fragment`.
- **Delete from `checkpoint.rs`**: the pre-gate `check_logs` at `:130`, the merge block, the merge-refusal BLOCK at `:925` and the `LogMerged` emit.
- **Delete the agent-log helpers**: the `agent_log` helpers in `checkpoint.rs`, `closeout.rs` and `cli/answer.rs`, and the fragment skip at `scheduler.rs:1149`.
- **Fix the staging paths.** `Pathspec::from` pushes `<dir>/state.toml` and `<dir>/log.jsonl` where it pushed the fragment. `protected_files` becomes `prd.md` and `docs/index.md`, and `agent-log.md` leaves the list. `commit_checkpoint` stages `board.dir` and drops `docs/index.md`. The feature folder already holds both new files.
- **Delete the merge tests**: the merge-clash and title-strip tests at `commit.rs:724,787,946`, and the stub's `ad:` and `prose:` options.

**The trap**: with `lanes` ≥ 2, one slice's commit stages the whole `log.jsonl` and `state.toml`. A sibling's in-flight lines ride along. That is correct for an append-only file. A test that asserts a slice commit touches only its own lines would be wrong. Assert the paths instead.

**Acceptance:** `tests/commits.rs`: a done slice's commit lists exactly its stream paths, its slice file, `state.toml` and `log.jsonl`. A checkpoint commit has no `docs/index.md` and no `log-slice-*.md`. No `log-*.md` file exists in the fixture after a full run. `grep -rn "fold_logs\|fragment_path\|render_write\|AD-503" src` returns nothing.

### 5. The closeout minimum that cannot wait for slice 10 — high — blocked by: 1, 3, 4
**Source**: §8 FR-18: "the pre-commit `render --write` calls (`checkpoint.rs:813`, `closeout.rs:633`)" go; "The checkpoint and closeout commits stage `state.toml` and `log.jsonl`… The closeout commit stays `docs: ship PRD-NNN` (`closeout.rs:9`)." §4 FR-14: "a checkpoint's BLOCK… is `kind: block`."

Slice 10 owns closeout's retro and PR side. This task covers only what `closeout.rs` needs to compile and pass `tests/closeout.rs` without `render_write` and `agent-log.md`:

- **`:553-572`** matches `Ship::Refused(text)` in place of `ToolOutput`.
- **`:633-634`** loses `render_write` and stages `board.dir` alone.
- **`:1925-1978`** (polish, PR and CI blocks) become `block` entries with their `phase`. The scan reads `docs.status().open_blocks`.
- **`:2104-2125`** (Traps) becomes `note` entries.
- **`:1836` and `:2160`** use `Docs::open`.
- **`fill_retro` and `RETRO_KEYS`** stay as they are; slice 10 removes them.

**Acceptance:** `tests/closeout.rs` on v3: the ship commit's subject is `docs: ship PRD-001` and its files hold `state.toml`, with `status = "shipped"` and a `shipped` date, and `retro.md`. A refused ship writes one `block` entry with `phase: "polish"`, whose body quotes the refusal text.

### 6. The harness and the stub on v3; delete `tests/fixture/.docs` — high — blocked by: 1
**Source**: §8 FR-18: "The harness builds the v3 fixture with the binary: `factory docs init`, `factory docs new fixture --title Fixture`, `factory docs slice 001 a|b|c --depends 2`, `patch_prd`, no render; `Fixture::docs()` and `docs_json()` run `env!("CARGO_BIN_EXE_factory") docs …`. The vendored `tests/fixture/.docs` goes, except a stub `docs.mjs` kept for FR-17's routing test and a small real v2 fixture… kept for FR-16's migration test."

**`tests/harness/mod.rs`:**
- **Construction.** `Fixture::new` and `with_index` build what `Fixture::v3` builds. `v3` stays as an alias.
- **The knobs.** `write_index_frontmatter` becomes a `[docs]` writer with the same knobs, or folds into the builder.
- **`docs()` and `docs_json()`** run the binary. `factory_docs` and `factory_docs_json` become aliases.
- **Slice state.** `slice_status :289` reads `state.toml`. `chain :304` writes `depends_on` through `factory docs set` when slice 01's `set` accepts the key; otherwise it writes through `factory::docs::state` (the lib crate, `src/lib.rs`).
- **The binary for the stub.** `build_env` adds `FACTORY_BIN = env!("CARGO_BIN_EXE_factory")`.

**`tests/stubs/claude`:**
- **Slice status.** `setStatus` becomes `execFileSync(FACTORY_BIN, ['docs','set',feature,`slices.${n}.status=${s}`])`.
- **Blocks and decisions.** The fragment BLOCK and AD writes become `docs log add --kind block|decision --slice n`, with the body on stdin.
- **Checkpoint.** CP and BLOCK become `--kind checkpoint|block --phase checkpoint`.
- **Polish.** Polish `reviewed:` becomes `docs set NNN reviewed=<date>`, and the polish BLOCK becomes `--phase polish`.
- **Block ids.** `lowestFreeBlock` goes, since the tool prints the id.

**Deletions.** Delete `tests/fixture/.docs`; `tests/fixture/stub-docs` and `tests/fixture/v2/.docs` (slice 07) stay. A v2 fixture constructor (`Fixture::v2`, on `tests/fixture/v2`) stays for task 8 and the slice 07 tests.

**The trap**: the stub is node and keeps running under node. `Command::new("node")` in `tests/` is not the `src` grep. The stub must not shell out to `docs.mjs` anywhere: `grep -n "docs.mjs" tests/stubs/claude` returns nothing.

**Acceptance:** `cargo test --test harness --test stubs` green on the new fixture. `ls tests/fixture/.docs` fails. A fixture's `docs/features/001-fixture/` holds `prd.md`, `state.toml`, three slice files and no `agent-log.md`. After a stub slice run, `log.jsonl` holds a `decision` line with `slice: 1`.

### 7. The suites pass on v3 with their assertions unchanged in meaning — high — blocked by: 2, 3, 4, 5, 6
**Source**: §8 FR-18 Acceptance: "The `loop.rs`, `run.rs` and `lanes.rs` suites pass on the v3 fixture with their assertions on slice statuses, checkpoints, blocks and the closeout commit unchanged in meaning." §2: "The stub harness (`tests/harness/mod.rs`) and the PRD-001 test suite… are the net; they move to the v3 fixture in the same slice."

Rewrite each assertion that reads a slice file's `status:`, `agent-log.md`, a `log-slice-*.md` or `docs/index.md` to read `state.toml` (via `fx.slice_status`) or `log.jsonl` (via `factory docs log show --json`). Keep what it asserts. The order follows the reference counts: commits, lanes, checkpoints, stubs, quota, loop, docs, closeout, metrics. Then run cockpit, run, worker, knobs and ctx unchanged. **The trap**: a test that asserted a seeded id such as `BLOCK-201` or `AD-301` now gets `BLOCK-1`/`AD-1`. Change the literal, and keep the slice by asserting the entry's `slice` field. The `tests/docs.rs` board test on node goes, because slice 04's equivalence test replaced it.

**Acceptance:** `cargo test` green. A reviewer diffing each changed test sees the same states, counts and commit subjects asserted. `git diff --stat tests/` lists no deleted `#[test]` other than the merge, clash and fragment tests named in task 4.

### 8. The v2 refusal and the node grep — medium — blocked by: 1, 6
**Source**: §8 FR-18 Acceptance: "`grep -rn 'Command::new("node")' src` returns exactly one site, the FR-17 shim in `cli/docs.rs`, and `Docs::open` has no path to it… `factory run` on the v2 fixture exits 2 with the migrate hint before preflight." §2: "After FR-18 the driver cannot run on a repo that has not migrated: `factory run` refuses with the migrate hint."

`cli/run.rs:63,181`: match the v2 refusal from `Docs::open`. Write the hint to stderr and return `Ok(2)`; an `Err` would exit 1 (`main.rs:62-65`). The same applies wherever `cli/status.rs` and `cli/report.rs` open a run's repo: print the hint and do not fail the listing.

**Acceptance:** `tests/run.rs`: `factory run 001` on `Fixture::v2` exits 2. Stderr contains "docs are v2: run `factory docs migrate --dry`". No run dir is created and the stub `claude` has no calls. A test runs `grep -rn 'Command::new("node")' src` and asserts exactly one line, in `src/cli/docs.rs`.

### 9. Verify — medium — blocked by: 1, 2, 3, 4, 5, 6, 7, 8
`cargo test` (the inner loop; the full gate runs at this slice's checkpoint)

**Acceptance:** inner loop green, one commit.

### 10. Prove — at the checkpoint — blocked by: 9
Rows from [`prd.md`](prd.md) § QA plan: **FR-18**. Run `cargo test` on the v3 fixture. Run `grep -rn 'Command::new("node")' src` and get exactly the FR-17 shim. Run `factory run` on the v2 fixture and get exit 2 with the hint. Evidence: the grep output, the exit code and stderr, and the test count go in this slice's Progress.

**Acceptance:** after the checkpoint, Progress holds the grep line, the `factory run` transcript with `exit 2`, and the `cargo test` summary line.

**Code-review fixes (2026-10-13).**

- `factory answer BLOCK-n` had no way out when the id was open in two features. With per-feature numbering that is the common case, and "resolve the one you mean by hand" meant hand-writing `log.jsonl`. Added `--feature NNN` (AD-9902); the refusal names it. Test: the two-open case in `tests/lanes.rs`.
- `slice_status` in `src/cli/answer.rs` turned a `state.toml` parse error into "no status", so an answer left a blocked slice blocked and printed "left as is". It returns the error now. Test: `a_state_toml_that_does_not_parse_is_an_error_not_no_status`.
- The feature code of slices 01, 03, 09, 10 and 11 was in the working tree but not in any commit: the driver's pathspec commits left it out, and the gate ran on the dirty tree. The polish commit carries it (AD-9901).
