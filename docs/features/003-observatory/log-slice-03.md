## Traps

- Siblings share this working tree: `cargo fmt` rewrites their in-flight files too. Check your own files with `rustfmt --edition 2024 --check <file>` and leave theirs alone.
- A sibling's untracked, in-progress test target (here `tests/knobs.rs`) can turn a bare `cargo test` red and hang it for about 10 minutes before SIGTERM. Run the inner loop as `cargo test --lib --bins --test <each target but theirs>` and say so in Progress.
- `WorkerRow` gains fields from several slices at once: a full struct literal in a test breaks on every sibling's column. It now derives `Default`, so build test rows with `..WorkerRow::default()`.

## Decisions

### AD-301 — How is a section mean computed when only some of a role's packs wrote it? · slice-03 · 2026-10-12
**Chose:** each section's mean is taken over every row of the role that recorded `pack_parts`. A row without that section (an unanswered slice has no `block_and_answer`) counts 0. Sections are listed in the order the packs first wrote them, and the JSON object keeps that order.
**Over:** a mean over only the rows that have the section, which shows what a block costs when present but makes the section means stop adding up to `total`.
**Because:** the line exists to split the start into its shares, so the parts should sum to the whole they sit beside.
**Undo:** divide each section's sum by its own row count in `pack_by_role` (`src/cli/report.rs`).
**Review:** fyi

### AD-302 — What do a role with no `pack_parts`, and a run with no workers, print? · slice-03 · 2026-10-12
**Chose:** a role whose rows all have null `pack_parts` prints `{role} —` and is `null` in the JSON. A null `start_context` prints `start_context —`. A run with no workers prints `pack_by_role: none`, like `weighted_by_role`.
**Over:** leaving such roles out, which hides that the role ran with an unmeasured pack.
**Because:** the slice settles `—` for a role with none; `none` for an empty run follows the neighbouring per-role line.
**Undo:** filter `None` entries out of `pack_by_role`.
**Review:** no

### AD-303 — Touching shared lines in `db.rs` and `report.rs` · slice-03 · 2026-10-12
**Chose:** these edits touch lines other slices also edit:
- the `pack_parts` migration tuple sits right after `in_subagent`, not at the list's end, so it does not clash with sibling appends;
- `workers()` reads it by name, `r.get("pack_parts")`;
- `WorkerRow` derives `Default`;
- in `the_lines_carry_the_keys_in_order`, slice 04's assertions `&EXTRA_KEYS[16..]` and `lines().last() == "hypothesis: —"` become `[16..18]` and `lines()[KEYS.len() + 17]`, so `pack_by_role` can be last as task 3 requires.
**Over:** appending at the tail of the shared lists, and keeping slice 04's assertions unchanged, which a new last key breaks.
**Because:** task 3's acceptance puts `pack_by_role` last, and the order of tuples in the migration list does not matter.
**Undo:** move the tuple and restore the two assertions if `pack_by_role` stops being last.
**Review:** fyi
