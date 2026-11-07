# Slice 08 — A red gate's block title names the failure

**Source**: [`prd.md`](prd.md) → §10 FR-8, §2 (block titles change), §11 slice recon corrections. **Depends on**: 03 (the excerpt extractor in `src/excerpt.rs` and the `gate_failure` reader). Independent of every other slice: it touches `checkpoint.rs:125-135` only, which slice 04 does not edit.
**Created**: 2026-11-07

**Progress**:
- **Built**: `failure_line` in `src/checkpoint.rs` reads the last 1 MiB of the red gate log (`live::gate::read_log`), runs the excerpt with the main tree's `gate_failure` patterns, and takes the first hit (never a context line) for the block title behind the unchanged prefix, through `block_title`. No hit, or an unreadable log, keeps the last-non-empty-line title. The body's 40-line quote is unchanged. New fixture gate `tests/fixture/gate-pest.sh` (Pest JSON failure mid-log, ends on `Script @ci:check was called via verify`) and test `a_red_gate_block_names_the_failure` in `tests/checkpoints.rs`; `three_reds_block_the_run_and_the_answer_reruns_the_gate` stays green unchanged.
- **Inner loop**: 2 runs, 0 red, about 4 min — caught nothing.
- **Deviations**: none.
- **Prove**: FR-8 (`a_red_gate_block_names_the_failure`) is left for the checkpoint.
- **Proved (checkpoint 2):** FR-8, `a_red_gate_block_names_the_failure`, 1 passed — `cp-2/fr-8-red-gate-title.txt`

**Slice**: After this slice, when the gate goes red three times at a checkpoint, the block's title names the failure the excerpt found, behind the same "gate red three times at checkpoint N:" prefix, instead of the log's last non-empty line. The title still goes through `block_title` and its 160-char cap, and a log where the excerpt finds nothing keeps today's last-line title. `status --json` `questions[].text` and the herdr strip show the new words; the schema does not change.

## Current code / environment

- **Today's title**: `src/checkpoint.rs:124-135`: once `reds_in_a_row` reaches `RED_CAP`, it reads the whole gate log (`fs::read_to_string(&result.log)`, `:126`), takes the last non-empty trimmed line, else "no output" (`:127-133`), and builds `answers::block_title(&format!("gate red three times at checkpoint {n}: {line}"))` (`:134-135`). The body quotes the last 40 lines (`:136-139`); prd §10 changes the title only.
- **`block_title`** (`src/answers.rs:134-147`): collapses whitespace, replaces `·`, caps at `TITLE_MAX = 160` (`:131`) with `…`. `add_block` (`:224-235`) applies it again with `tilde`.
- **Other `block_title` callers stay**: `checkpoint.rs:932` (a refused merge) and `closeout.rs:2140` (closeout blocks). prd §14 "Not handled: a CI block's title".
- **The extractor** (slice 03): `src/excerpt.rs`, which marks matches apart from context lines. Take the **first match**, never a context line above it. Project `gate_failure` patterns come from `docs/project.toml` of `run.state().repo` (the main tree, where the project file is committed); a pattern that does not compile is skipped.
- **The test to model on**: `three_reds_block_the_run_and_the_answer_reruns_the_gate` (`tests/checkpoints.rs:1071`), not `:883` as prd §10 first said (§11 slice recon correction): `:883` is red-red-green and never blocks. It asserts today's title at `:1106` with `FAIL_LINE` (`:23`), the one line `tests/fixture/gate.sh:27-29` prints. The generic `^FAIL` matches that line, so `:1106` and `:1672` (`starts_with`) stay green unchanged.
- **The fixture gate**: `Fixture::new` copies `tests/fixture/gate.sh` to the fixture root and declares `./gate.sh` as the gate (`tests/harness/mod.rs:265-276`). A test that needs a different gate copies its own script over `gate.sh` and commits before `start`. Keep `tests/fixture/gate.sh`'s scenario mechanism (the `scenario/gate` file, `gate.log`) so `fx.script_gate` and `fx.gate_log()` work with it.
- **Release notes**: prd §10 says they call out the change of text. This repo has no release-notes file (no `CHANGELOG`; releases are a version commit and a tag, e.g. `df3a82d`). Nothing to write in this slice; the PR body names the change.

## Tasks

### 1. The title from the excerpt — high — blocked by: —
**Source**: §10 FR-8: "The red-gate title takes the excerpt's first line, behind the existing 'gate red … at checkpoint N:' prefix. It still goes through `block_title` and its 160-char cap, and falls back to the last line when the excerpt is empty."

`src/checkpoint.rs:124-135`. Read the last 1 MiB of the log as slice 03's reader does (the whole-file read stays for the body's 40-line quote if that is simpler; prd §3's budget is `live`'s, not the driver's).

**Acceptance:** `a_red_gate_block_names_the_failure`: a new fixture gate `tests/fixture/gate-pest.sh` that, on `red`, prints noise lines, then a Pest JSON line `{"failures":[{"test":"it rejects an expired token","file":"tests/Feature/AuthTest.php","line":42,"message":"Failed asserting that 200 is 401."}]}` mid-log, more noise, and ends on `Script @ci:check was called via verify`. Driven like `three_reds_block_the_run_and_the_answer_reruns_the_gate` (three reds), the block's `title` contains `it rejects an expired token` and does not contain `Script @ci:check`. `three_reds_block_the_run_and_the_answer_reruns_the_gate` stays green with its title assert unchanged.

### 2. Verify — medium — blocked by: 1
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 3. Prove — at the checkpoint — blocked by: 2
prd §12 row **FR-8**: run `a_red_gate_block_names_the_failure` and keep the transcript.

**Acceptance:** after the checkpoint, this slice's Progress names that transcript.

**Code-review fixes (2026-11-07).**

- `failure_line` had been inserted between `leaks_section` and its doc comment, so it carried the Leaks doc and `leaks_section` had none; each doc is back on its own function.
