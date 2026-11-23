# Slice 01 — covers required on write

**Source**: [`prd.md`](prd.md) → §3 (FR-1, the write side). **Depends on**: — _(independent: it edits only the log's validator and the CLI arg, which no other slice touches)_
**Created**: 2026-11-23

**Progress**:
- Built: `validate()` in `src/docs/log.rs` now refuses a checkpoint whose `covers` is empty ("a checkpoint needs --covers, every slice it proved"). That covers both an omitted flag and `--covers ""`, and the log is never read or locked. New test `log_add_checkpoint_refuses_a_missing_covers` (`tests/docs_log.rs`). The unit refusal list gains a no-covers case.
- Inner loop: 1 run, 0 red, 5 min. Caught nothing.
- Seen red: `log_add_checkpoint_refuses_a_missing_covers` with the refusal stubbed to `false`. The command exited 0 and the `!o.status.success()` assertion failed. Green again once restored.
- Deviation (AD-1, fyi): the existing tests that wrote a covers-less checkpoint now pass `--covers`, and the `cp(&[])` "no covers field" assertion is dropped because it asserted the write FR-1 forbids. `a_log_without_covers_keeps_the_high_water_rule` and `covered()` are untouched.
- Left for the checkpoint: the §8 FR-1 `run` rows `log_add_checkpoint_refuses_a_missing_covers` and `a_log_without_covers_keeps_the_high_water_rule`.
- Checkpoint 1, FR-1 (write half): `cp-1/fr-1-write-side.log` — `log_add_checkpoint_refuses_a_missing_covers` and `a_log_without_covers_keeps_the_high_water_rule` green.

**Slice**: After this, a checkpoint entry with no `covers` cannot be created, so the false-green path in §3 closes for every new run. It leaves the read side alone: `covered()` keeps its high-water branch for the 18 covers-less entries in archived 001 to 005, and the driver still closes on the number alone — that is slice 03.

## Current code / environment

- **The refusal pattern to copy**: `src/docs/log.rs:195-200` — `validate()` already refuses a checkpoint with no `--slice`: `if e.kind == Kind::Checkpoint && e.slice.is_none() { return refuse("a checkpoint needs --slice, the last slice it covers".into()); }`. The line above it refuses `--covers` on a non-checkpoint. Add the new refusal beside them, in the same style, and `add()` calls `validate()` first (`:218`).
- **"Empty" means absent or `[]`**: `src/docs/log.rs:84-86` — `covers: Vec<u32>` carries `#[serde(default, skip_serializing_if = "Vec::is_empty")]`, so an omitted flag and `--covers ""` both arrive as an empty vec. One check covers both.
- **The CLI arg already exists**: `src/cli/docs.rs:164-191`, `LogVerb::Add`. Nothing to add there unless the arg needs a required-ness change; prefer the validator, so the refusal text is the same from every caller.
- **The one owner-facing caller already passes it**: `src/checkpoint.rs:226` — the death block's hand-run line is built with `--covers {covers_csv}`, so no instruction an owner copies goes stale.
- **Do not touch `covered()` or its test** — *prd §3 decided the high-water branch stays: `src/docs/board.rs:338-346` and `tests/docs_log.rs:289-308` (`a_log_without_covers_keeps_the_high_water_rule`, with fixture `tests/fixture/log-without-covers/log.jsonl`). `assets/autonomy.md:62` forbids a run weakening a test it did not write, and removing the branch is PRD-022's follow-up.*
- **The run builds with the installed binary (0.13.5)**: a lane runs the installed `factory`, so this slice's own checkpoint is not enforced by this slice's code. Write evidence the way today's contract says (`$FACTORY_EVIDENCE/cp-{n}/`), not the per-row layout slice 04 introduces.

## Tasks

### 1. `validate()` refuses a checkpoint with no covers — high — blocked by: —
**Source**: §3 FR-1: "`factory docs log add --kind checkpoint` requires `--covers`, so a covers-less entry cannot be created again"; Acceptance: "with no `--covers` is refused, naming the flag, and exits non-zero".

`src/docs/log.rs`. Add the refusal in `validate()`, next to the `--slice` one at `:198-200`. The message names the flag and what it is for, in the voice of the line above it. Keep the non-checkpoint `--covers` refusal unchanged.

**The trap**: `add()` takes the lock and reads the file *after* `validate()` (`:218-220`), so a refusal must stay in `validate()` to avoid touching the log at all. Put it there, not in the CLI, so every caller gets the same text.

**Acceptance:** `log_add_checkpoint_refuses_a_missing_covers` in `tests/docs_log.rs` — `docs log add <f> --kind checkpoint --slice 3 --title x` with no `--covers` exits non-zero, the message names `--covers`, and `log.jsonl` is unchanged on disk. A second case passes `--covers 1,2` and succeeds. **Fails if removed**: without the refusal the command exits 0 and appends an entry, so the unchanged-file assertion goes red.

### 2. Verify — medium — blocked by: 1
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q`

**Acceptance:** inner loop green, one commit.

### 3. Prove — at the checkpoint — blocked by: 2
`prd.md` §8, the **FR-1** row — this slice's half: `log_add_checkpoint_refuses_a_missing_covers` and `a_log_without_covers_keeps_the_high_water_rule`. Both are `run` rows, proven below the UI: the checkpoint runs them and keeps the transcript. The send-back tests in the same row belong to slice 03.

**Acceptance:** after the checkpoint, the transcript shows the refusal test green and the high-water test still green, untouched.
