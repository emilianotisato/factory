# Slice 04 — Builder Autonomy

**Source**: [`prd.md`](prd.md) → §3 FR-7, and FR-1's Autonomy wording. **Depends on**: — (independent of every other slice: only this one touches `assets/autonomy.md` and the `AUTONOMY` pushes in `src/pack.rs`)
**Created**: 2026-10-15

**Progress**:
- **Built**: `assets/autonomy.md` holds four `<!-- builder: cut -->` / `<!-- builder: end -->` pairs around § Mapping, the four § Caps bullets, Blocker protocol steps 3-4, and § The owner answers mid-run through § Resuming; § Tokens reads "Several tool calls side by side". `src/pack.rs`: `AUTONOMY` is now private; `autonomy()` gives the whole file without marker lines (fix, merge, checkpoint, polish, CI fix packs), `builder_autonomy()` drops the marked parts (slice pack). The builder's Autonomy is 8,596 chars against 11,856 whole, 3,260 shorter. New pack test `a_builder_reads_autonomy_without_the_drivers_and_the_orchestrators_parts` shares the six-role pack setup with `every_role_reads_the_plan_stays_in_docs` through an extracted `every_role_pack` helper; `tests/loop.rs:294` compares against `builder_autonomy()`.
- **Inner loop**: 1 run, 0 red, ~2 min (a second `cargo test` pass to confirm no truncated failure: 37 binaries ok).
- **Deviations**: none. Marker shape is AD-3.
- **Prove**: FR-7 row left for the checkpoint.

**Slice**: The slice pack carries a builder's Autonomy: the whole file minus the parts that are the orchestrator's or the driver's job, about 800 tokens per builder start. Markers in `assets/autonomy.md` delimit the cut, and no pack carries a marker. Every other pack carries the file whole. § Tokens stops naming the Grep tool, which build, fix and merge workers lose in slice 03.

## Current code / environment

- **Every pack pushes the file whole**: `pub const AUTONOMY: &str = include_str!("../assets/autonomy.md")` (`src/pack.rs:17`). Six pushes, each `format!("# Autonomy\n\n{}", AUTONOMY.trim_end())`: slice `:229` (in `for_slice_with`, `:149`; `for_slice` only delegates), fix `:283`, merge `:362`, checkpoint `:466`, polish `:548`, CI fix `:595`. A marker left in `AUTONOMY` reaches all six.
- **The cut's parts** in `assets/autonomy.md` (11,932 bytes; the parts count 3,273 chars):
  - § Mapping, `:29-39`, with its heading;
  - § Caps bullets **Slice hand-back** (`:90`), **CI** (`:91`), **Flaky** (`:92`) and **Not this branch's** (`:93`). The **Gate** bullet (`:89`) stays;
  - § Blocker protocol steps 3 (Commit it, `:99`) and 4 (**Notify**, `:100`). Steps 1, 2 and 5 (**Stop.**, `:101`) stay;
  - § The owner answers mid-run (`:103-105`) and § Resuming (`:107` to the end), with their headings.
- **§ Tokens names the Grep tool**: `:80` "Several Read, Grep or Bash calls side by side are one model call."
- **Tests that pin today's pack**: `pack_is_the_slice_the_cited_sections_the_contract_and_the_inner_loop` asserts `text.contains(pack::AUTONOMY.trim_end())` for the slice pack (`tests/loop.rs:294`); FR-7 flips it. Slice 02 edits the contract assertions a few lines above it (`:281-290`), so keep this edit to its own line. `every_role_reads_the_plan_stays_in_docs` (`src/pack.rs:1278`) pins § The plan stays in docs/ in every role's pack and must stay green. No other test reads `AUTONOMY` or the cut headings.
- **Why the Slice hand-back bullet goes**: *§3 FR-7, corrected 2026-10-15: "the Slice hand-back bullet (`:90`) promising a builder 'sent back once' on `docs check` errors, which FR-8 does not do; it is the orchestrator's rule, so it joined the cut."*

## Tasks

### 1. The slice pack carries the builder's Autonomy — high — blocked by: —
**Source**: §3 FR-7: "The slice pack (`for_slice_with`, `src/pack.rs:149`; the push is at `:229`) leaves out § Mapping (`assets/autonomy.md:29-39`), the Slice hand-back, CI, Flaky and Not-this-branch's bullets of § Caps (`:90-93`), steps 3 and 4 of § Blocker protocol (`:99-100`, the driver's in a run), § The owner answers mid-run (`:103-105`) and § Resuming (`:107` on). Step 5's Stop stays. Every other pack carries the file whole. Markers in `autonomy.md` delimit the cut, and no pack carries a marker."

`assets/autonomy.md`: a marker line before and after each of the four cut parts. `src/pack.rs`: one function gives the whole text with the marker lines removed, another the builder text with the marked parts removed. The push at `:229` takes the builder text; the five others take the whole text.

**The trap:** `AUTONOMY` is the raw file. Once it holds markers, no push may use it directly, and `tests/loop.rs:294` must compare against the builder text, not `AUTONOMY`.

**Acceptance:** a pack test on the fixture: the slice pack's `# Autonomy` holds none of "## Mapping", "**Slice hand-back:**", "**CI:**", "**Flaky:**", "**Not this branch's:**", "**Notify**", "## The owner answers mid-run", "## Resuming"; it still holds § The plan stays in docs/ and the **Stop.** step; it is at least 2,500 chars shorter than the checkpoint pack's `# Autonomy`. No pack of any role holds a marker. `every_role_reads_the_plan_stays_in_docs` stays green.

### 2. § Tokens names no tool — medium — blocked by: —
**Source**: §3 FR-1 Accepted cost: "Autonomy § Tokens names the Grep tool ("Several Read, Grep or Bash calls side by side", `assets/autonomy.md:80`); it becomes "Several tool calls side by side", for every role."

`assets/autonomy.md:80`: "Several Read, Grep or Bash calls side by side" becomes "Several tool calls side by side". The line is outside the cut, so every pack carries it.

**Acceptance:** the pack test of task 1 finds "Several tool calls side by side" in the slice pack's and the checkpoint pack's `# Autonomy`, and "Read, Grep or Bash" in neither.

### 3. Verify — medium — blocked by: 1, 2
`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 4. Prove — at the checkpoint — blocked by: 3
`prd.md` § QA plan row, driven below the UI:
- **FR-7**: build a slice pack and a checkpoint pack from the fixture; diff their `# Autonomy` sections; keep the transcript.

**Acceptance:** after the checkpoint, the row has its transcript in this slice's **Progress**.
