# Slice 02 — The handed row list

**Source**: [`prd.md`](prd.md) → §4 (FR-2, the pack side) and §5 (FR-3). **Depends on**: — _(independent, but it is the spine: slice 04 cannot check evidence until the pack returns the row keys. Both FRs rewrite `qa_rows` and `for_checkpoint`, so they are one slice.)_
**Created**: 2026-11-23

**Progress**: built — `qa_rows` returns `(text, Vec<QaRow>)` with `FR-<n>`/`FR-<n>.<k>` keys counted over the whole body and a `human` flag read from the `Human?` column; each row prints `` key: `…` `` (and `(deferred: Human? = yes)`); `Pack.rows` carries the checkpoint's handed rows; the whole-table fallback is gone; an empty body says `0 rows handed`; `all_done` hands every row and says so. Inner loop: 2 runs, 1 red, 7 min — the red caught seven `tests/checkpoints.rs` tests leaning on the removed fallback (fixture fixed, AD-3). Deliberate reds: `one_fr_with_three_rows_gets_three_keys` (no occurrence count → three `FR-6`; Human cell ignored → no human row), `a_covered_slice_naming_no_row_hands_nothing` (fallback back → every row), `the_last_checkpoint_is_handed_every_row_in_the_table` (no `all_done` arm → `[FR-8]`), `an_earlier_checkpoint_is_handed_only_its_covered_rows` (always whole table → five keys), `a_prd_with_no_fr_rows_hands_nothing` (sentinel as signal → wrong line). Deviations: AD-2 (key print shape), AD-3 (harness fixture).
- Checkpoint 1, FR-2 (pack half): `cp-1/fr-2-pack-keys.log`, `cp-1/fr-2-recorded-keys.md` — `one_fr_with_three_rows_gets_three_keys` and `a_prd_with_no_fr_rows_hands_nothing` green; keys FR-6.1, FR-6.2, FR-7 (deferred), FR-6.3, FR-8 recorded.
- Checkpoint 1, FR-3: `cp-1/fr-3-handed-rows.log` — `the_last_checkpoint_is_handed_every_row_in_the_table`, `an_earlier_checkpoint_is_handed_only_its_covered_rows`, `a_covered_slice_naming_no_row_hands_nothing` green.

**Slice**: After this, `for_checkpoint` returns the keys of the rows it handed the worker, the pack prints each row's key beside it, a `Human? = yes` row is marked deferred, and the last checkpoint is handed the whole table while an earlier one is handed only its covered slices' rows — including when that set is empty. Nothing enforces any of it yet; the driver's check is slice 04.

## Current code / environment

- **`qa_rows` returns a `String` that is rendered and dropped**: `src/pack.rs:689-722`, called at `:490-492`. `Pack` (`:82-88`) holds `path`, `prompt`, `files`, `missing_sections` and `parts` — no row list, so no caller can know what was handed. This is the gap the slice closes.
- **`named` does not come from `covers`**: `src/pack.rs:452-461` — for each covered slice it reads the slice file, takes `prove_section`, and collects `fr_ids` from it. So the handed set is decided by what the slices' `### Prove` sections name, not by the slice numbers.
- **The whole-table fallback**: `src/pack.rs:716-722` — `if picked.is_empty() { out.extend(body) }`. A covered slice whose Prove section names no row therefore hands **every** row today. §5 decides that case hands nothing instead.
- **The `|---` term**: `src/pack.rs:690-693` — `rows` collects lines starting `| FR` **or** `|---`, so `rows.is_empty()` (and its sentinel at `:694-695`) only fires when the PRD has no markdown table at all. *prd §4: the no-rows signal is an empty row **body**, not the sentinel string.*
- **The `Human?` column is read by nothing**: `grep -rni human src/` finds only `src/closeout.rs:909`, a PR-body pointer. Five `Human? = yes` rows exist in this repo's own PRDs (`docs/archive/013-live-status/prd.md:159`, `docs/archive/004-skills/prd.md:219,225,228`, `docs/archive/003-observatory/prd.md:126`). The column is the 4th cell.
- **`all_done` is already in hand**: `src/docs/board.rs:326`, on `Status` at `src/docs/mod.rs:149`, and `for_checkpoint` already takes `board: &Status` (`src/pack.rs:413-417`).
- **The key format is fixed by the PRD** — *prd §4: `FR-<n>` when the FR has one row in the table, `FR-<n>.<k>` for the k-th row when it has several. Do not invent another.*
- **Do not add a row list to `state.toml`** — *prd §10 cut per-slice gate numbers and any second store; the keys live in `Pack`, in memory, for the driver that built it.*
- **The run builds with the installed binary (0.13.5)**: this slice's own checkpoint pack is built by the old code, so its pack will not show keys.

## Tasks

### 1. `qa_rows` returns rows with keys, and reads the Human column — high — blocked by: —
**Source**: §4 FR-2: "`for_checkpoint` must return the keys of the rows it handed, and the driver must keep them"; "the key is `FR-<n>` when the FR has one row in the table and `FR-<n>.<k>` for the k-th row when it has several"; "a `yes` row is listed in the record as deferred, not as evidence owed".

`src/pack.rs`. Change `qa_rows` to return the rendered text **and** a list of handed rows, each with its key and whether its `Human?` cell is `yes`. Keep the rendered table's columns as they are and print the key beside each row so the worker can see which directory to use. Compute the key from the first cell's FR id plus the occurrence count of that id **in the whole table body**, not in the picked subset: an FR with three rows keys `FR-6.1`, `FR-6.2`, `FR-6.3` whichever subset is handed.

**The trap**: the first cell can carry more than the id (`| FR-1 after merge |`), which is why `picked` misses it today (`:710-716`). Parse the id with the same `FR-<digits>` shape `fr_ids` uses, so a decorated cell still keys and still matches.

**Acceptance:** `one_fr_with_three_rows_gets_three_keys` — a table with three `FR-6` rows and one `FR-7` row returns keys `FR-6.1`, `FR-6.2`, `FR-6.3`, `FR-7`, and the `FR-7` row carrying `yes` in its 4th cell comes back marked human. **Fails if removed**: without the occurrence count all three `FR-6` rows key `FR-6`, so the assertion on three distinct keys goes red.

### 2. An empty picked set hands nothing, and the no-rows signal is the body — high — blocked by: 1
**Source**: §5 FR-3: "When it is false, the pack carries the covered slices' rows and nothing else, including when that set is empty"; §4: "A PRD with no `| FR-` rows hands nothing, and the record says `0 rows handed`".

`src/pack.rs`. Drop the `picked.is_empty() { out.extend(body) }` fallback at `:716-722`: an empty picked set renders the header with no rows and returns no keys. Decide "the PRD has no QA table" from an empty row **body**, not from `rows.is_empty()`, so a PRD holding other markdown tables is still recognised.

**The trap**: `:694-695`'s sentinel string is reachable only when the PRD has no table of any kind. Leave the string where it is for that case, but do not use it as the signal.

**Acceptance:** `a_covered_slice_naming_no_row_hands_nothing` — a covered slice whose `### Prove` section names no FR id yields zero keys and a table with no body rows, where today it yields every row. `a_prd_with_no_fr_rows_hands_nothing` — a PRD whose only table is unrelated (its lines start `|---` and `| Step`) yields zero keys and does not print the sentinel as if it were a row. **Fails if removed**: with the fallback in place the first test sees every row and the key count assertion goes red.

### 3. The last checkpoint is handed the whole table — high — blocked by: 1
**Source**: §5 FR-3: "When `all_done` is true, the pack carries every `| FR-` row in the PRD and FR-2's evidence check applies to all of them, minus the `Human? = yes` rows."

`src/pack.rs`, `for_checkpoint`. When `board.all_done` is true, hand every row in the table body instead of the `named` subset, and say so in the `# QA rows` section's own line so the worker knows why it has more rows than its slices.

**Acceptance:** `the_last_checkpoint_is_handed_every_row_in_the_table` — with `all_done` true and `covers` naming one slice whose Prove section names one row, the pack's keys are every row in the table. `an_earlier_checkpoint_is_handed_only_its_covered_rows` — the same fixture with `all_done` false yields only that slice's row. **Fails if removed**: without the `all_done` arm both cases return the same subset, so the first assertion goes red.

### 4. Verify — medium — blocked by: 1, 2, 3
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q`

**Acceptance:** inner loop green, one commit.

### 5. Prove — at the checkpoint — blocked by: 4
`prd.md` §8, the **FR-3** row in full (`the_last_checkpoint_is_handed_every_row_in_the_table`, `an_earlier_checkpoint_is_handed_only_its_covered_rows`, `a_covered_slice_naming_no_row_hands_nothing`), and the **FR-2** row's pack half (`one_fr_with_three_rows_needs_three_directories`'s key half, `a_human_row_needs_no_evidence`'s marking half, `a_prd_with_no_fr_rows_hands_nothing`). All `run` rows, proven below the UI from the transcript.

**Acceptance:** after the checkpoint, the transcript shows each named test green, and the recorded keys for the fixture table appear in the evidence.
