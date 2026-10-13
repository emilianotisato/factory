---
slice: 03
status: done
depends_on: []
lane: 2
---

# Slice 03 — Pack composition (FR-22)

**Source**: [`prd.md`](prd.md) → §4 FR-22 ("`workers.pack_parts`, JSON, one entry per section the pack wrote"). **Depends on**: — (independent of slices 01, 02, 04 and 05: its own column, its own report line; it only appends a tuple to the shared migration list in `src/db.rs`).
**Created**: 2026-10-13 · **Status**: pending

**Progress**: 2026-10-12, built tasks 1-4; task 5 (Prove) waits for CP-2.
- **Built**: `pack::Part { section, chars }` and `Pack.parts`. The five builders now write their body through one `assemble(header, sections)`: each section runs from its `# Heading` to the blank line before the next, the last one ends with the final newline, and its name is the heading in lower snake case. The pack text is byte-for-byte what it was, so the stub's role and run-dir reads are untouched. `workers.pack_parts` (TEXT, JSON) is set by `Db::set_worker_pack_parts` from `record()`, beside `pack_files`, and read into `WorkerRow.pack_parts`. `report` has a new last key, `pack_by_role`. The text reads `build slice … outline … total … start_context … · checkpoint … · polish …`. The JSON is role → `{sections: {name: mean} in pack order, total, start_context}`, or null for a role with no `pack_parts` row.
- **Tests**: `pack::tests::every_pack_kind_measures_the_sections_it_wrote` builds all five kinds against a temp repo with the vendored `docs.mjs`. It checks that the parts sum to the text after the header (including non-ASCII chars), that a slice pack without a block has no `block_and_answer` entry and with one has it, and that the polish pack starts at `branch`. `report::tests::pack_by_role_means_each_section_in_pack_order` covers the means, the order, null rows and `—`. `tests/metrics.rs` `every_worker_row_records_what_its_pack_was_made_of` covers every row of `finished_run()` against the pack text the stub logged for its session, the three section lists (`slice…`, `gate_transcript…`, `branch…`), and `pack_by_role` in both `--json` and text.
- **Inner loop**: 2 full runs, 1 red, about 11 min. Run 1 (`cargo test`, 9m50s) was red and then SIGTERMed in `tests/knobs.rs`, slice 04's untracked in-progress file (`a_project_file_sets_the_polish_model_and_the_row_says_so` asserts `effort_source`, nothing of this slice). Run 2, every target except `knobs`, was green in 1m03s: 137 lib tests and 17 test binaries. The new tests each passed on their first run.
- **Deviations**: none from the tasks. AD-301 to AD-303 cover what the slice left open.
- **Prove (task 5), driven at CP-1 (2026-10-12)**: the FR-22 row, green. In all 5 worker rows, `pack_parts` sums exactly to the pack minus its header. There are three section lists (`slice …`, `gate_transcript …`, `branch …`) and one `pack_by_role` line. Evidence: `evidence/cp-1-fr22-pack-parts.txt`.

**Slice**: After this slice every worker row says how many characters each section of its pack held, and `report` prints per role the mean per section and the pack total beside `start_context`, so the next pack experiment can see what the 45k start is made of. It leaves the retro table that prints the builders' composition to slice 06.

## Current code / environment

- **`Pack` has no sizes**: `src/pack.rs:54-65` `Pack { path, prompt, files, missing_sections }`. Only `files` is persisted, by `set_worker_pack_files` (`src/db.rs:471`) from `Worker::record()` (`src/worker.rs:384-446`) right after `insert_worker`.
- **Five pack builders, each with its own section list**, all writing a text that starts with header comment lines and a blank line: slice `:145-178` (`<!-- factory-pack files: … -->\n\n` then `# Slice`, optional `# Block and answer` `:148-154`, `# PRD sections`, `# Outline`, `# Lookup`, `# Contract`, `# Autonomy`, `# Inner loop`); fix `:199-249` (header at `:217`); checkpoint `:257-346` (`:314`); polish `:355-424` (**two** header lines, the second `<!-- run dir: … -->`, `:385-388`; sections `# Branch`, `# Retro`, `# PR body`, `# Contract`, `# Autonomy`); CI fix `:431-481` (`:450`). The stub `claude` reads the role from the first section heading (`tests/stubs/claude:124-131`) and the polish stub reads the run dir from the second header line (`:287`): the header lines must stay exactly as they are.
- **Caps**: `OUTLINE_CAP` 8000, `DEF_CAP` 6000, `TAIL_LINES` 200 (`src/pack.rs:45-47`): the Outline section is already size-bounded; nothing records what the bound produced.
- **Schema and report conventions**: the guarded `ALTER` tuple list in `migrate()` (`src/db.rs:257-267`) is where every 003 slice appends its columns; `SCHEMA_VERSION` (`:15`) is set to 2 by whichever 003 slice lands first. `WorkerRow` (`:126-156`) and `workers()` (`:507-547`) list columns by hand. The report unit test asserts the KEYS+EXTRA_KEYS order and `lines()[16]`, `[17]`, `[21]`: append new keys at the end of `EXTRA_KEYS` (`src/cli/report.rs:80-89`). `ROLE_ORDER` (`:304`) and `by_role_text` (`:273`) are the per-role line pattern. `start_context` per role is not computed today; `compute()` (`:386-473`) sums over `workers`.
- **The stub records the pack path**: `tests/stubs/claude:100-111` writes `pack` and `pack_source` into its call log, read back by the harness `calls()` helper (`tests/harness/mod.rs`), so a test can open the very file the worker was handed.
- **Deliberate non-changes** — *(prd §4: "No token estimate from chars: the ratio differs between prose and code, and a wrong estimate would be read as a measurement." Print chars only. Accepted cost: "the remainder is inferred, not measured".)*

## Tasks

### 1. Measure every section as the pack is written — high — blocked by: —
**Source**: §4 FR-22: "`workers.pack_parts`, JSON, one entry per section the pack wrote: `[{"section": "slice", "chars": 9120}, …]`, measured when the pack is written, for every pack kind."; Acceptance: "the entries' `chars` sum to the char count of the pack file's text after its header comment lines and the blank line that follows them".

`src/pack.rs`. Add `pub struct Part { pub section: String, pub chars: usize }` and `parts: Vec<Part>` on `Pack`. In each of the five builders, build the body as an ordered list of `(name, text)` where each section's text runs from its `# Heading` line to just before the next heading (separating newlines included in the preceding section), then join into the body and measure each with `str::chars().count()`. Names are the heading in lower snake case: `slice`, `block_and_answer`, `prd_sections`, `outline`, `lookup`, `contract`, `autonomy`, `inner_loop`, `branch`, `retro`, `pr_body`, and the fix and CI-fix headings likewise. Header comment lines and the blank line after them are not parts.

The trap: the polish header is two lines (`:385-388`) and the slice pack's `# Block and answer` is optional (`:148-154`): "one entry per section the pack actually wrote" means an absent section has no entry, not a zero entry.

**Acceptance:** a `pack.rs` unit test builds one pack of each kind against a fixture and asserts the parts' `chars` sum to `text.chars().count()` minus the header lines and their trailing blank line, that the slice pack without a block has no `block_and_answer` entry and with one has it, and that the polish pack's first part is `branch`.

### 2. Store `pack_parts` on the worker row — high — blocked by: 1
**Source**: §4 FR-22 Acceptance: "every worker row has `pack_parts` with one entry per section its pack actually wrote".

`src/db.rs`: append `("workers", "pack_parts", "TEXT")` to the migration list (set `SCHEMA_VERSION` to 2 if still 1); `WorkerRow.pack_parts: Option<String>`; extend `set_worker_pack_files` into `set_worker_pack(id, files, parts_json)` or add `set_worker_pack_parts`. `src/worker.rs` `record()` writes it beside `pack_files` with `serde_json::to_string(&pack.parts)`.

**Acceptance:** `tests/metrics.rs` after `finished_run()`: for every worker row, parse `pack_parts`, open the pack file the stub's call log names for that session, and assert the sum of `chars` equals the file's text after its header lines and the blank line; the three roles the plan spawns (slice, checkpoint, polish) show three different section lists.

### 3. `report` prints the per-role composition — high — blocked by: 2
**Source**: §4 FR-22: "`report` prints, per role, the mean chars per section and the pack's total chars beside `start_context`, so the reader sees the pack's share of the start and the remainder that is Claude Code's own."

`src/cli/report.rs`. New `EXTRA_KEYS` entry `pack_by_role`, appended last: text `build slice 9120 prd_sections 3100 outline 7800 … total 28400 start_context 45000 · polish …` (means over the role's rows, integers, `ROLE_ORDER` first), JSON a map role → `{ sections: { name: mean }, total: mean, start_context: mean }`. Rows with `pack_parts` null (old runs) are left out of the means; a role with none prints `—`.

**Acceptance:** the report unit test's order assertion gains `pack_by_role` last with `lines()[16]`, `[17]`, `[21]` unchanged; after `finished_run()`, `factory report <id> --json` has `pack_by_role.polish.sections.branch` equal to the polish row's `branch` chars (one polish worker, so mean = value) and `pack_by_role.build.start_context` equal to the mean of the build rows' `start_context`.

### 4. Verify — medium — blocked by: 1-3
`cargo test` (the inner loop; the full gate `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` runs at this slice's checkpoint).

**Acceptance:** inner loop green, one commit.

### 5. Prove — at the checkpoint — blocked by: 4
Rows from [`prd.md`](prd.md) §9 this slice must prove, below the UI: the **FR-22 row**: `run` any scenario → every worker row's `pack_parts` sums to its pack file size minus the header; `report` prints per-role section means. Driven at CP-2 together with the full gate; the test transcript goes in this slice's **Progress**.

**Acceptance:** the FR-22 row has its test transcript attached in Progress, showing the three different section lists and one `pack_by_role` line.
