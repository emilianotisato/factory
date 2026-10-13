---
slice: 10
status: done
depends_on: [5, 6, 9]
lane: 2
---

# Slice 10 — Closeout and worker-facing rest (FR-18 rest, FR-19 write path)

**Source**: [`prd.md`](prd.md) → §8 FR-18 (the closeout half of the removals bullet, the worker-contracts bullet, the `/factory-deliver` bullet), §9 FR-19 (the write-paths bullet, the `factory retro` line, the closeout half of the acceptance), §10 FR-20 (the "worker contracts and assets move with it" bullet). **Depends on**: 05 (the contracts and `autonomy.md` name `factory docs idea add`, which must exist), 06 (the `retros` table, its upsert and `docs retro NNN --set score=`), 09 (the driver runs in process on v3: `Docs::open`, `log_add`, `set_slice_status`, the v3 harness and stub, and no `render_write`).
**Created**: 2026-10-13 · **Status**: pending

**Progress**:
- **Built.** (1) Ship upserts the feature's `retros` row from `report::compute` (`closeout::retro_row`, `score` left to the upsert's COALESCE); `RETRO_KEYS`, `fill_retro`, `retro_values` and their unit tests are gone. (2) `assets/retro.md`: the skill template with no frontmatter and no "**Frontmatter**" paragraph, `pack::RETRO`; `retro_template()` takes no `Docs`; the polish `# Retro` section says the driver writes `## Numbers` and the store row. (3) `commit_ci_fix` stages no `retro.md` and bumps nothing. (4) Already done by slice 09: `shipped_at_head` reads `state.toml` at HEAD, `commit_polish` stages `state.toml` and no `prd.md`, `fallback_body` names `log show --decide`; verified. (5) The five contracts call `{factory} docs set|log add|check|retro`, filled with `paths::factory_command()`; every `{seed}` gone; the slice contract fills `{N}` unpadded. (6) A `# Log` section, `log::render` of `log.jsonl` under `<!-- factory docs log show NNN -->`, "none yet" when empty, in all five packs; each contract reads it in place of `agent-log.md` § Traps. (7) The hook's whole-read exemption is `log.jsonl`; a listed `agent-log.md` now gets the Outline deny. (8) `/factory-deliver` §3 reads `log show --decide` and `--json`, writes `log add --kind review --ref AD-n --verdict`. (9) `autonomy.md`, `pr-body.md` name `docs set`, `log add`, `idea add`, `log show`; no render, fragment or `ideas.md`. (10) Cockpit text "no open block in the log", the `MergeRefused` arm marked legacy, the `events.rs` docs; a unit test reads a real `log_merged` and `merge_refused` line. (11) `factory retro` prints `score: N|none` when the row exists.
- **Tests.** `tests/closeout.rs`: the row's twelve numbers equal `report --json` (minutes within 0.005), `run_id` the run's; `retro.md` has no frontmatter; `factory retro` before and after `docs retro 001 --set score=4`; the CI fix commit lists `src/lib.txt` only and leaves `retro.md` as ship wrote it; the polish pack's `# Log` equals `log show 001` stdout; `the_slice_contracts_commands_run_on_the_fixture` runs the seven `{factory} docs …` lines of the slice contract and finds decision, note, block and slice 1 `blocked`. `harness::assert_v3_contract` checks the slice, checkpoint, polish, CI-fix (closeout) and fix (checkpoints) packs: the absolute binary before ` docs `, no seed, placeholder, fragment, `agent-log.md` or render. The stub `claude` wants the template to open with `# Retro — PRD-` and writes no frontmatter.
- **Inner loop.** 4 runs, 2 red, 2.2 min. Red 1: `factory retro`'s new `score:` line in the second-run test. Red 2: `tests/cockpit.rs` needles on the old §3 text, and `tests/loop.rs` wanting the `log-slice-01.md` name. Last run: 410 passed, 0 failed; `cargo fmt --check` clean; clippy's one warning is in slice 07's `migrate.rs`.
- **Deviations.** The contract-command tests live in `tests/closeout.rs` and `tests/checkpoints.rs` beside the packs they read, not in `tests/worker.rs`. The `RETRO_KEYS` grep still hits the vendored v2 `docs.mjs` (AD-1004). The row keeps ship-time `ci_cycles` (AD-1001, decide).
- **Prove (left for the checkpoint).** FR-19 closeout: the `sqlite3` read-back of the `retros` row next to `## Numbers`, the `factory retro` transcripts around `--set score=4`. FR-18 contracts: the `cargo test` count with the contract-command test, the empty asset grep.

**Slice**: After this slice nothing the driver writes or tells a worker names `agent-log.md`, a `log-*.md` fragment, retro frontmatter or `render --write`. At ship, closeout upserts the feature's `retros` row from `report::compute` and writes no frontmatter. The retro template is an in-repo asset with no key list. A CI fix commit no longer touches `retro.md`. The five worker contracts call `<factory> docs log add`, `<factory> docs set` and `<factory> docs check` through the resolved binary path, with no AD seeds. Each pack carries `log show NNN` output where the contract pointed at `agent-log.md`. The PreToolUse hook exempts `log.jsonl`, `/factory-deliver` §3 reads and writes the log, and `autonomy.md` and `pr-body.md` name `factory docs idea add`. `factory retro` prints the score line once the row exists. The `hook` verb, the skills edit and the install are slice 11.

## Current code / environment

Line numbers verified 2026-10-13 against `main` at `26d3c0a`, before slice 09 lands. Slice 09 edits the same functions, so re-verify each one at slice entry.

- **The ship step today** (`src/closeout.rs:541 ship`): `docs.ship` `:553`, `set_run_log_counts` `:596`, `report::compute` `:613`, the retro created from the template `:616-627`, `fill_retro(&retro, &report)` `:628`, `retro::write` (the FR-25 `## Numbers` section) `:630`, `docs.render_write()` `:633`, `commit::expand(&repo, &[board.dir, "docs/index.md"])` `:634`, subject `commit::subject(board, "docs", "ship PRD-{feature}")` `:638`. Slice 09 task 5 removes `:633` and stages `board.dir` alone. It keeps `fill_retro` and `RETRO_KEYS` for this slice (slice-09 task 5, last bullet).
- **The subject is `docs(NNN): ship PRD-NNN`, not `docs: ship PRD-NNN`.** `src/commit.rs:578-580 subject` formats `{kind}({number:03}): {text}`; `tests/closeout.rs:439,444` assert `docs(001): ship PRD-001`. Keep the code's form. The PRD's `docs: ship PRD-NNN` (§8, citing `closeout.rs:9`) is shorthand; the module doc at `:10` has the real form.
- **What goes**: `pub const RETRO_KEYS: [&str; 12]` `:50-63`; `pub fn fill_retro` `:659-663` (calls `scheduler::set_frontmatter`); `fn retro_values` `:665-680`; the unit tests `:2323-2339` (fill_retro over a frontmatter string) and `:2387-2399` (`the_fallback_template_has_every_key`, asserting `RETRO_KEYS.len() == 12`). The `use crate::scheduler::{…, set_frontmatter, …}` at `:40` loses `set_frontmatter` if nothing else in the file uses it.
- **The `ci_cycles` bump**: `commit_ci_fix` (`:1704` doc, `:1708` fn) counts `EventKind::CiFixCommitted` events and calls `set_frontmatter(&retro, &[("ci_cycles", committed + 1)])` at `:1735`. It adds `retro.md` to the commit's paths at `:1736-1738`. `tests/closeout.rs` asserts that frontmatter at `:936` (`"1"`), `:1011`, `:1115` (`"3"`) and `:1597` (`"4"`), and asserts that the CI fix commit's files are `[RETRO, "src/lib.txt"]` at `:931-934`.
- **The CI count survives without the bump.** `factory report` derives `ci_cycles` from events at read time: `src/cli/report.rs:658 ci_cycles: count("ci_fix_committed")`. The `run_finished` event carries it too (`tests/closeout.rs:583`). Only the `retros` row loses it after ship: it holds the ship-time value, which is 0 before any CI round. The PRD names no later write to the row (see the task 3 trap).
- **The retro template comes from beside the docs tool**: `fn retro_template(docs: &Docs, board: &Status)` `:2178-2199` reads `docs.tool().parent().parent()/templates/retro.md`. That is `~/.claude/skills/feature-docs/templates/retro.md`, whose lines 1-15 are the 13-key frontmatter and lines 21-27 a "**Frontmatter**" paragraph naming `measure.py`. Otherwise it builds a fallback frontmatter from `RETRO_KEYS` with `ci_cycles: 0`. Slice 09 removes `Docs::tool`, so this function cannot compile as is after 09. If 09 left a stub, this slice replaces it. Callers: polish `:198`, ship `:618`, the polish resume `:1837`. The pack embeds the result: `src/pack.rs:393` param, `:421-429` section text.
- **Resume after ship reads v2 frontmatter**: `fn shipped_at_head` `:2051-2054` runs `git show HEAD:{dir}/prd.md` and compares `frontmatter_value(…, "status")` (`:2057`) to `shipped`. A v3 `prd.md` has no `status`; it lives in `state.toml`. Slice 09 does not name this function. If 09 did not switch it, every v3 resume after a ship sees "not shipped" and ships twice. Its caller is the resume check `:115-122`.
- **The polish commit**: `fn commit_polish` `:367-440` creates `retro.md` from the template when the worker did not (stand-in AD-904), then commits the stream paths plus `{dir}/prd.md` and the retro. On v3 the polish worker sets `reviewed` with `docs set`, so the PRD file is no longer what changed. Slice 09's stub already writes `reviewed` that way (slice-09 task 6).
- **The contracts** (`assets/`, embedded by `src/pack.rs:23-27`):
  - **`contract-slice.md`**: `:7` read `agent-log.md` § Traps. `:10` "`status: building` in the slice frontmatter. Do not run `render --write`". `:15` AD into `log-slice-{NN}.md`, "start at AD-{seed}". `:16` Traps into the fragment. `:17` "`status: done`, `docs check`". `:19` "your fragment". `:21` `BLOCK-{seed}` into the fragment and slice `status: blocked`.
  - **`contract-checkpoint.md`**: `:1` "the driver has merged the slice logs into `agent-log.md`". `:8` Traps. `:13-14` CP record under `## Checkpoints` in `agent-log.md`. `:15` "Do not run … `docs log merge`, `render --write`". `:18` BLOCK into `agent-log.md`.
  - **`contract-fix.md`**: `:6` Traps. `:12` off-limits list naming fragments, `agent-log.md` and `docs/index.md`. `:13` AD in `log-fix-{n}.md`. `:16` `BLOCK-{seed}`.
  - **`contract-polish.md`**: `:6` § Traps and § Decisions. `:12` "Keep the frontmatter block exactly as the template has it… `score` is the owner's". `:14` `reviewed: {date}` in `prd.md` frontmatter. `:15` AD in `agent-log.md` at `AD-{seed}`. `:18` BLOCK under `## Blockers`.
  - **`contract-ci-fix.md`**: `:6` Traps. `:11` off-limits list naming fragments, `agent-log.md`, `retro.md`, `docs/index.md`. `:12` AD in `log-ci-{round}.md`. `:15` `BLOCK-{seed}`.
- **The seeds** (`src/pack.rs`): slice `{seed}` = `{slice}01` `:178`; fix `9{n}01` `:251`; polish `"9901"` `:414`; ci-fix `8{round}01` `:486`. The checkpoint contract has none (`:311-316`).
- **The resolved binary path already reaches packs one way.** `pack::lookup` (`src/pack.rs:37-43`) replaces each `` `factory ctx `` in `assets/lookup.md` with `paths::factory_command()` (`src/paths.rs:66-73`: the absolute path when the running binary is named `factory`, else the plain name). The recon trailers do the same at `:627`. Under `cargo test` the binary is `target/debug/factory`, so packs carry its absolute path.
- **The pack never inlines `agent-log.md`.** The contracts tell the worker to open it. The only pack code naming it is a test `BlockSection` (`src/pack.rs:974`), built for real at `src/scheduler.rs:766-774` (the answer source is `answers::DIR/{id}.json`). "The pack includes `log show NNN` output where it included `agent-log.md`" (§8) therefore means a new section that replaces the contracts' "read `agent-log.md`" step.
- **The hook exemption**: `src/cli/hook.rs:120-122` returns `None` for `agent-log.md` and `log-*.md`, so a whole read of those pack-listed files is allowed. The tests at `:911` and `:1090` use `agent-log.md`.
- **`/factory-deliver` §3**: `skills/factory-deliver/SKILL.md:35` "Read `docs/features/NNN-*/agent-log.md`… Collect the ADs with no `**Owner` line yet: `**Review:** decide` lines first, then `fyi`". `:37` "under each answered AD append `**Owner ({date}):** kept`… Append `**Owner review ({date}):** k kept, c changed` to the feature's `retro.md`".
- **`assets/autonomy.md`** (a pack section): `:35` the Park row says "an `ideas.md` line". `:43` the May list names `prd.md` frontmatter status, `agent-log.md` / `log-*.md` fragment, the index row and Notes cell, the `inner:` key in `docs/index.md` frontmatter, and "lines in `docs/ideas.md`". `:83` BLOCK into `agent-log.md` or the fragment. `:84` "set that slice `status: blocked`, then `docs render --write`". `:86` notify example "in agent-log BLOCK-1". `:99` "render; commit". **`assets/pr-body.md`**: `:15` "every decision is in `docs/features/{NNN}-{slug}/agent-log.md`"; `:39` "**Parked → `docs/ideas.md`**: {the line}". **`closeout.rs:524-534 fallback_body`**: `:530` names `{dir}/agent-log.md § Decisions`.
- **Stale text that stays as data**: `src/cockpit.rs:64` "slice {NN} blocked with no BLOCK-n in its fragment"; `:73` the "log merge refused" label; `src/events.rs:156` the `WorkerWroteProtectedFile` doc (lists `agent-log.md`, `docs/index.md`); `:161-170` the `LogMerged { fragments }` and `MergeRefused` docs. **Keep the event kinds**: old `events.jsonl` files from PRD-001 and PRD-003 runs must still deserialize for `factory report` and the cockpit. Change only the doc comments and the live UI strings.
- **`factory retro`** (`src/cli/retro.rs:24-50`) opens the store, finds the run row, rewrites the `## Numbers` section and prints `retro: {path} updated|unchanged`. `tests/closeout.rs:520-523` assert that stdout exactly.
- **Deliberate non-changes** — *(prd §9: "`factory report` is unchanged"; "the retro line `**Owner review (date):** k kept, c changed` stays prose in `retro.md`" (§8). prd §8: "a worker that edits `log.jsonl` or `state.toml` by hand is caught by `check` at the next checkpoint, not denied. (Rejected: a hook deny on those two files.)" Do not add a deny rule.)*

## Tasks

### 1. Ship writes the `retros` row; `fill_retro` and `RETRO_KEYS` go — high — blocked by: —
**Source**: §9 FR-19: "at ship, closeout calls `retro_row(repo, feature, run_id, &report)` where `fill_retro` wrote frontmatter, and **writes no frontmatter and no table of its own**: the `## Numbers` section PRD-003 FR-25 already writes is what a reader sees, and `fill_retro`, `RETRO_KEYS` in `closeout.rs` and the `ci_cycles` frontmatter bump go." §8 FR-18: "`fill_retro` and `RETRO_KEYS` go too (FR-19); `retro::write` stays."

`src/closeout.rs`: at `:628`, call the upsert slice 06 lands (named `retro_row` in the PRD) with the run's repo, the feature number, `run.id()` and the `Report` from `:613`. Use the slug from the board and today's `shipped_at`. Leave `score` untouched on an existing row: an upsert must not null a score the owner set before a re-ship. Delete `RETRO_KEYS`, `fill_retro`, `retro_values` and their two unit tests. `tests/closeout.rs`: drop its `RETRO_KEYS` copy (`:29`) and the frontmatter block of assertions (`:465-482`). Assert the row instead, read with `sqlite3` or through the lib's `Db`. Keep the `## Cost vs catch` prose check.

The trap: `report.gate_minutes`, `build_minutes` and `polish_minutes` are `f64`. `fill_retro` wrote them with two decimals; the row stores `REAL`. Compare them with a tolerance in the test, not as strings.

**Acceptance:** `tests/closeout.rs` full-run test: after ship, the `retros` row for `(fixture repo, 1)` has `run_id` equal to the run's id. It holds `slices` 3, `checkpoints` 2, `gate_runs` 2, `gate_red` 0 and `ci_cycles` 0, the twelve values equal to `factory report <run> --json`. `retro.md` does not start with `---`. It contains the `## Numbers` section. `grep -n 'RETRO_KEYS\|fill_retro' src tests` returns nothing.

### 2. An in-repo retro template with no key list — high — blocked by: 1
**Source**: §9 FR-19: "A v3 `retro.md` has no frontmatter; the retro template loses the key list."

`assets/retro.md` (new, `include_str!` in `src/pack.rs` beside `PR_BODY`): `~/.claude/skills/feature-docs/templates/retro.md` lines 17-63 with two cuts. Drop lines 1-15 (the frontmatter). Drop lines 21-27 (the "**Frontmatter**" paragraph and the `measure.py` line). Keep `# Retro — PRD-{NNN}`, the "cost next to catch" paragraph and the six sections. `retro_template` takes no `Docs` and returns the asset. Its callers (`:198`, `:618`, `:1837`) drop the `docs` argument. `src/pack.rs:421-429`: the `# Retro` section says the driver writes the `## Numbers` section and the store row after the worker returns. Remove the twelve-key sentence and "`score` is the owner's". `tests/closeout.rs:408`: the pack wants `` "````markdown\n# Retro — PRD-001" `` in place of `` "````markdown\n---\nscore:\ngate_runs:\n" ``.

The trap: the skill template is the v2 tool's file, and v2 repos still get it through the shim (prd §7). Do not edit it. The copy is the only source after this slice.

**Acceptance:** `tests/closeout.rs`: the polish pack holds `# Retro — PRD-001` right after the `` ````markdown `` fence, and holds no `score:` line. A stand-in retro (`:758`, the worker wrote none) starts with `# Retro — PRD-001`, not `---`. A unit test asserts that `retro_template` contains no line ending in `:` before its first `#` heading.

### 3. A CI fix no longer touches `retro.md` — high — blocked by: 1
**Source**: §9 FR-19: "`fill_retro`, `RETRO_KEYS` in `closeout.rs` and the `ci_cycles` frontmatter bump go."

`src/closeout.rs:1726-1739`: delete the `retro_rel` block, so the CI fix commit stages the stream's paths and the swept leftovers only. Fix the `commit_ci_fix` doc at `:1704` and the module doc at `:1547`. `tests/closeout.rs`: `:931-934` expects `["src/lib.txt"]`. Delete the frontmatter `ci_cycles` asserts at `:936`, `:1011`, `:1115` and `:1597`. Each test already checks the cycle count through `factory report` or the `run_finished` event (`:583`); where one does not, assert `report --json` `ci_cycles` instead.

The trap: after this task the `retros` row keeps the ship-time `ci_cycles` (0 in a run whose CI goes red after ship). The PRD sources no row update at a CI fix commit. Do not invent one here: name it in an AD with `Review: decide`, and leave the value as ship wrote it. `factory report` still shows the true count (`src/cli/report.rs:658`).

**Acceptance:** `tests/closeout.rs` CI-fix test: the `fix(001): CI test` commit lists exactly `src/lib.txt`. `retro.md` is byte-identical before and after the CI round. `factory report <run> --json` reports `ci_cycles` 1.

### 4. Resume reads `shipped` from `state.toml` at HEAD — high — blocked by: —
**Source**: §8 FR-18: "The checkpoint and closeout commits stage `state.toml` and `log.jsonl` where they staged slice files, fragments, `agent-log.md` and `docs/index.md`. The closeout commit stays `docs: ship PRD-NNN` (`closeout.rs:9`)."

`src/closeout.rs:2051 shipped_at_head`: if slice 09 already switched it, verify and move on. Otherwise, run `git show HEAD:{dir}/state.toml` and parse it with slice 01's state parser (`src/docs/state.rs`). True when `status == "shipped"`. Delete `frontmatter_value` (`:2057`) if nothing else calls it. `commit_polish` (`:367-440`): stage `{dir}/state.toml` and `{dir}/log.jsonl` where it staged `{dir}/prd.md`. The polish worker's `reviewed` and its ADs land there now (task 5).

The trap: `git show` of a path missing at HEAD exits non-zero. That is "not shipped", not an error, as the v2 code treats it (`out.ok() &&`).

**Acceptance:** `tests/closeout.rs`: a run stopped after the ship commit and resumed does not create a second `docs(001): ship PRD-001` commit (`git log --oneline | grep -c "ship PRD-001"` is 1). The polish commit `fix(001): polish review fixes` lists `state.toml` and no `prd.md`.

### 5. The five worker contracts call the tool — high — blocked by: —
**Source**: §8 FR-18: "The worker contracts (`assets/contract-slice.md` and the checkpoint, fix, polish and ci-fix contracts beside it) change from "write a fragment" to "call `<factory> docs log add NNN …`", from `status: done` to `<factory> docs set NNN slices.N.status=done`, and from `docs check` to `<factory> docs check`, with the resolved binary path (`paths.rs:66 factory_command`)." §9 FR-19: "`factory docs retro NNN --set score=4` is how the polish worker and the owner set `score` (the tool writes the DB; the worker does not)."

`assets/contract-*.md`: add a `{factory}` placeholder. `src/pack.rs` replaces it with `paths::factory_command()` in each contract, as `lookup()` does (`:37-43`). Remove every `.replace("{seed}", …)` (`:178`, `:251`, `:414`, `:486`) and every `{seed}` in the assets. Per contract:
- **slice**: `:10` becomes `{factory} docs set {NNN} slices.{N}.status=building`, with the render sentence gone. `:15` becomes `{factory} docs log add {NNN} --kind decision --slice {N} --flag decide|fyi --title "…" --body-file -`, where the body holds `**Chose:**`, `**Over:**`, `**Because:**`, `**Undo:**`. `:16` Traps become `--kind note --slice {N}`. `:17` becomes `{factory} docs set {NNN} slices.{N}.status=done`, then `{factory} docs check` with no errors. `:19` "your fragment" goes. `:21` becomes `--kind block --slice {N}` plus `slices.{N}.status=blocked`. Use `{N}` unpadded for `set`; `{NN}` stays in prose.
- **checkpoint**: `:1` drops the merge clause. `:13-14` CP becomes `--kind checkpoint --phase checkpoint`. `:15` drops `docs log merge` and `render --write`. `:18` becomes `--kind block --phase checkpoint`.
- **fix** and **ci-fix**: the AD goes through `log add` with `--slice` for a fix and `--phase ci` for a ci-fix. The block uses the same phase. The off-limits list names `state.toml` and `log.jsonl` (edit only through the tool) instead of fragments, `agent-log.md` and `docs/index.md`.
- **polish**: `:12` drops "Keep the frontmatter block…" and says to set the score with `{factory} docs retro {NNN} --set score=<1-5>`. `:14` becomes `{factory} docs set {NNN} reviewed={date}`. `:15` and `:18` use `log add` with `--phase polish` on the block.

The trap: the stub `claude` (`tests/stubs/claude`) does not read the contracts, so the suites stay green even if a contract names a wrong flag. Only a test that runs each command a contract names catches that.

**Acceptance:** `tests/worker.rs` (or `tests/closeout.rs`): for each of the five contracts, the built pack contains the absolute `target/debug/factory` path before ` docs `. It contains no `{seed}`, `{factory}`, `agent-log.md`, `log-slice-`, `log-fix-`, `log-ci-` or `render --write`. A second test extracts each `` `{factory} docs …` `` command line from the slice contract. It runs each line on `Fixture::new` with the placeholders filled and asserts exit 0. `log show 001 --json` then holds the decision, the note and the block.

### 6. The pack carries the log — high — blocked by: 5
**Source**: §8 FR-18: "The pack includes `log show NNN` output where it included `agent-log.md`."

`src/pack.rs`: a `# Log` section built from `docs.log_entries` (slice 09) rendered with slice 02's `render`. That is exactly what `factory docs log show NNN` prints. It goes in the slice, fix, checkpoint, polish and ci-fix packs. Each contract's "read `agent-log.md` § Traps" line (slice `:7`, checkpoint `:8`, fix `:6`, polish `:6`, ci-fix `:6`) becomes "the `# Log` section below, its `## Notes` first". The section opens with the comment `<!-- factory docs log show {NNN} -->` so a worker knows where it came from. An empty log prints the section with "none yet".

The trap: the pack's header lists the files a worker may not read whole (`<!-- factory-pack files: … -->`, `tests/closeout.rs:397`). The log is inlined text, so `log.jsonl` stays out of that list. Task 7 lets a worker read the file whole anyway.

**Acceptance:** `tests/closeout.rs`: after two slices with stub decisions, the polish pack's `# Log` section equals `factory docs log show 001` stdout byte for byte. A slice pack built on a fresh fixture holds `# Log` and "none yet".

### 7. The hook exemption moves to `log.jsonl` — medium — blocked by: —
**Source**: §8 FR-18: "The PreToolUse hook (`cli/hook.rs`) keeps its rules; its whole-read exemption for `agent-log.md` and `log-*.md` (`:120-122`) moves to `log.jsonl`."

`src/cli/hook.rs:120`: `if name == "log.jsonl" { return None; }`. Update the tests at `:911` and `:1090` to `log.jsonl`. Keep one test that a whole read of `agent-log.md` in a pack's file list is now denied like any other listed file.

**Acceptance:** `tests/hook.rs` or `src/cli/hook.rs` tests: a `Read` of `{dir}/log.jsonl` with no offset is allowed (empty stdout). The same read of `{dir}/agent-log.md`, when listed, gets the "Outline" deny.

### 8. `/factory-deliver` §3 on the log — medium — blocked by: —
**Source**: §8 FR-18: "`skills/factory-deliver/SKILL.md` §3 reads `factory docs log show NNN --decide` and writes `factory docs log add NNN --kind review --ref AD-n --verdict kept|changed`. The retro line `**Owner review (date):** k kept, c changed` stays prose in `retro.md`."

`skills/factory-deliver/SKILL.md:35`: read `factory docs log show NNN --decide` for the decide round. Read `factory docs log show NNN --json` for the `fyi` decisions with no `review` entry, since `--decide` drops them. `:37`: replace "under each answered AD append `**Owner ({date}):** kept`…" with one `log add --kind review --ref AD-n --verdict kept|changed` per answer, with the "to what" text as the body for `changed`. Keep the `retro.md` line as written.

The trap: this skill is symlinked into `~/.claude/skills/factory-deliver` (`ls -la ~/.claude/skills`), so the edit is live for the owner's next `/factory-deliver` the moment it is written. It is not in the skills repo; it commits here.

**Acceptance:** `grep -n 'agent-log\|\*\*Owner (' skills/factory-deliver/SKILL.md` returns nothing. `grep -c 'docs log' skills/factory-deliver/SKILL.md` is at least 2. Run the two commands the section names on `Fixture::new` after a stub run, as the transcript in Progress, both exit 0.

### 9. `autonomy.md`, `pr-body.md` and the fallback body name the log and the ideas verb — medium — blocked by: 5
**Source**: §10 FR-20: "The worker contracts and assets move with it (FR-18): `assets/autonomy.md:35,43` says `factory docs idea add` where it says "an `ideas.md` line", and `assets/pr-body.md:39` says `**Parked → docs/ideas/**: {id}`." §8 FR-18: the contracts change "from "write a fragment" to "call `<factory> docs log add NNN …`"".

`assets/autonomy.md:35`: both cells say `factory docs idea add`. `:43`: replace "lines in `docs/ideas.md`" with "`factory docs idea add`". Replace the `agent-log.md` / fragment clause with "the log through `factory docs log add`". Replace "`prd.md` … frontmatter status" with "`factory docs set`". Drop "this feature's row and Notes cell in `docs/index.md`" and the `inner:` index-frontmatter clause, since v3 has neither. `:83-84`: the BLOCK goes through `log add --kind block`, and the slice goes `blocked` through `docs set`, with no render. `:86`: "in agent-log BLOCK-1" becomes "`factory docs log show NNN --open`". `:99`: drop "render". `assets/pr-body.md:15`: the decisions are in `factory docs log show {NNN}`. `:39`: `**Parked → docs/ideas/**: {id}`. `src/closeout.rs:530 fallback_body`: "`factory docs log show {NNN} --decide`".

The trap: `autonomy.md` is also the source of the skill copy at `~/.claude/skills/feature-docs/autonomy.md`, which slice 11 edits for the skills. Edit only this repo's asset here.

**Acceptance:** `grep -n 'agent-log\|log-\*\.md\|fragment\|ideas\.md\|render' assets/autonomy.md assets/pr-body.md assets/contract-*.md` returns nothing. `tests/closeout.rs` fallback-body test (`:815`, stand-in body): the body names `log show 001 --decide`.

### 10. Cockpit and event text — low — blocked by: —
**Source**: §8 FR-18: "The four `agent-log.md` workarounds go… with `fold_logs`, `Fold`, `fragment_path`, the checkpoint merge step (`checkpoint.rs:207`)…"

`src/cockpit.rs:64`: "slice {NN} blocked with no open block in the log". `:73`: keep the label for old `MergeRefused` events and mark it legacy in a comment. `src/events.rs:156`: the protected-file doc lists what v3 protects (`commit.rs:32-36` after slice 09). `:161-170`: the `LogMerged` and `MergeRefused` docs say "emitted by runs before PRD-002; kept so old event logs read". The test data at `src/cockpit.rs:409,420` stays: it is old events.

**Acceptance:** an `events.rs` unit test deserializes one `log_merged` and one `merge_refused` line copied from a PRD-001 run's `events.jsonl`. `grep -n 'fragment' src/cockpit.rs` matches only the legacy comment.

### 11. `factory retro` prints the score — low — blocked by: 1
**Source**: §9 FR-19: "`factory retro` (003) may print `score` beside its numbers once the row exists, one line, no new verb."

`src/cli/retro.rs:42-49`: after the `retro: {path} …` line, read the `retros` row for `(row.repo, row.feature)`. When it exists, print `score: {n}`, or `score: none` while unset. No row prints nothing. Update `tests/closeout.rs:520-523` for the second line.

**Acceptance:** `tests/closeout.rs`: `factory retro <run>` after ship prints `retro: … unchanged\nscore: none\n`. After `factory docs retro 001 --set score=4` it prints `score: 4`. On a store with no row for the feature (a run that never shipped) the output is the single `retro:` line.

### 12. Verify — medium — blocked by: 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11
`cargo test` (the inner loop; the full gate runs at this slice's checkpoint)

**Acceptance:** inner loop green, one commit.

### 13. Prove — at the checkpoint — blocked by: 12
Rows from [`prd.md`](prd.md) § QA plan: the closeout half of **FR-19** ("harness closeout row agrees with FR-25's section and `retro.md` has no frontmatter; `retro 001 --set score=4`"). Evidence: the `sqlite3` read-back of the `retros` row next to the `## Numbers` section of the same run, and the `factory retro` transcript before and after `--set score=4`. And the contract part of **FR-18**: `cargo test` on the v3 fixture with the task 5 contract-command test in the count, plus the task 9 grep over `assets/`. The `Command::new("node")` grep and the v2 refusal were proven in slice 09. Evidence goes in this slice's Progress.

**Acceptance:** after the checkpoint, Progress holds the row read-back with `run_id` equal to the run's, the two `factory retro` transcripts, the empty asset grep and the `cargo test` summary line.
