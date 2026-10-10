---
slice: 04
status: done
depends_on: [3]
lane: 1
---

# Slice 04 — Lanes and parking (FR-3)

**Source**: [`prd.md`](prd.md) → §5 FR-3 (all bullets and acceptance); §3 FR-1 bullet 2 ("`ask[]` and `judge[]` go to the cockpit … a run never starts on an unanswered one") *(FR-1 corrected 2026-10-09 by the owner, BLOCK-201: only `ask[]` parks; `judge[]` is logged as `judge_noted` and never waited on.)*. **Depends on**: 03 (the serial loop and the worker spawn this slice widens to N lanes).
**Created**: 2026-10-09 · **Status**: done

**Progress**: 2026-10-10 — built: `src/scheduler.rs` rewritten around a worker table (`HashMap<slice, Running{worker, lane}>`; each 1 s tick reaps exits, re-reads the board, respawns `building` slices nobody runs — answered ones with the block in the pack, dead ones under slice 03's cap — then spawns `spawnable` ready slices, writing `status: building` + `lane: N` into the frontmatter before each spawn; a `blocked` exit → `slice_blocked {slice, block_id, question}` read from the fragment's `## Blockers`, the lane kept out of the free set (AD-401); nothing running + nothing spawnable + open blocks → `run_waiting`, phase `waiting-owner`, a 5 s slow tick that wakes early on a change in `runs/<id>/answers/`; a run parked on asks goes through branch and board when the last is answered), `src/lanes.rs` (`docs/reference/lanes/lane-N.md` → `FACTORY_LANE=N` plus the file's `env:`, `setup:` once per lane per run), `src/answers.rs` (`answers/<ID>.json`, `BLOCK-n` entries parsed from the fragments and `agent-log.md` with fences skipped, `resolve` = `**Resolution ({date}):**` appended and the heading `open` → `resolved`), `src/cli/answer.rs` (`factory answer BLOCK-n|ASK-n "<text>"`: the resolution, the slice back to `building` on its lane, the answer file, `block_answered`; `ASK-n` → `DEC` line in `agent-log.md`, `ask_answered`; a live driver is told, a dead one is launched), `src/pack.rs` (`# Block and answer` after the slice section), `preflight.json` saved beside an ask-parked run, the stub's `blocked` outcome and its call number from the pack's spawn suffix, harness `chain()`, `tests/lanes.rs` (two lanes at once with lane env and setup; blocked → parked → answered → respawned with the answer → done; asks answered → the run starts). Inner loop: 8 `cargo test` runs, 6 red, ~3 min — caught a `mod harness;` clash with `tests/harness.rs`, the stub taking autonomy.md's quoted `**Resolution (` for an answer (slice 01 built to `done` instead of blocking — the one real bug), `answer` and `blocked` still listed as "not built yet" in two tests, and a fence-end expectation in the entries unit test. Deviations: AD-401–AD-406 in `log-slice-04.md`; the two-spawn acceptance is asserted as ≤ 1 s apart rather than the same second (AD-402); task 2's `waiting-owner` acceptance needs `checkpoint_every` above 2 on the fixture (AD-403). Proven at CP-2 (2026-10-10): **FR-3** — `tests/lanes.rs::a_blocked_slice_parks_its_lane_and_the_answer_respawns_it` on the kept fixture; the `events.jsonl` excerpt runs seq 8 `slice_blocked` 01 (BLOCK-101, lane 1 parked) → 9 `slice_done` 02 → 11 `slice_done` 03 (both on lane 2) → 12 `run_waiting {blocked: [1]}` → 13 `block_answered` → 14 `slice_spawned` 01 with `# Block and answer` in its pack → 15 `slice_done` 01 → 16 `checkpoint_due`, and the fragment's `BLOCK-101 … · resolved` entry carries `**Resolution (2026-10-10):** Pick the Recommended option` — `evidence/cp-2-fr3-blocked-answered-events.txt`.

**Slice**: After this slice the driver runs as many workers as `status --json` says it may, assigns each a lane from `freeLanes`, and treats a block as a parked lane rather than the end of the run. Independent slices keep building while a block waits. When nothing is spawnable and a block is open, the run parks in `waiting-owner` and stays alive watching for an answer. `factory answer` is the owner's half: it writes the resolution into the log, flips the slice back to `building`, and the next worker on that slice gets the block and the answer in its pack. The same command clears a preflight ask from slice 02.

## Current code / environment

- **Lanes come from the board, not from the driver.** `docs.mjs:1381-1416`: `freeLanes` = declared lanes minus the lanes held by `building` slices (frontmatter `lane:`); `spawnable = checkpointDue ? 0 : min(freeLanes.length, ready.length)`; `ready[]` is dependency-satisfied `pending` slices in number order. The driver spawns `spawnable` workers for the first `spawnable` entries of `ready[]` and writes `lane: N` into each slice's frontmatter itself (today the builder did it, `deliver/SKILL.md:100-135`; §5 "the driver assigns a lane from `freeLanes` and marks the slice `building`").
- **Lane files carry env and setup**: `docs.mjs:1470-1490` reads `docs/reference/lanes/lane-N.md` with frontmatter `lane`, `env` (a list of `KEY=value` lines) and a `setup:` command. If slice 01 found the fixture needs lane files, the driver passes `env` into the worker's environment and runs `setup` once per lane per run before the first spawn on it; if the index key alone suffices, `env` and `setup` are empty and nothing runs. Either way the worker sees `FACTORY_LANE=N`.
- **The block shape**: `autonomy.md:81-88`: a `BLOCK-n` entry (shape in `templates/agent-log.md`) in the slice's fragment under `## Blockers`, slice `status: blocked`, phase named (`preflight | slicing | <slice> | polish | pr | ci`). `:93-100` resuming: append `**Resolution ({date}):**` under the entry, flip the heading `open` → `resolved`, blocked slice back to `building`, render, commit. `status --json` surfaces them as `openBlocks[]` and `blocked[]`.
- **The worker does not commit** (slice 03's contract) and this slice does not either; slice 05 adds commits. Until then a block's log edit sits in the working tree, which is fine for the fixture: preflight is skipped on resume.
- **The `blocked` scenario the stub needs** (added to `tests/stubs/claude` here): apply `status: building`, write the fragment with a `BLOCK-NN01` entry (`open`, phase = the slice, a question with a Recommended option), set `status: blocked`, emit the stream ending `STATUS: blocked\nBLOCK: BLOCK-NN01`. A follow-up spawn on the same slice whose pack contains `Resolution` builds it to `done`.
- **Preflight asks** were parked in slice 02 as `ASK-n` events with phase `waiting-owner`; `deliver/SKILL.md:34` makes "proceed as-is" always an option. `factory answer ASK-n proceed` is that option; any other text is recorded as the owner's instruction and the run proceeds too (a preflight ask has no machine-readable branch to take; the text goes into `agent-log.md` as a `DEC` line so the polish worker sees it).
- **Deliberate non-changes**: *(prd §5 "Rejected: the driver answers blocks itself from a rubric. The block exists because judgment was needed; the owner answers, the driver only routes.")* *(prd §19 "Not handled: the review interview as a driver phase".)* *(prd §19 "Not handled: a `--resume` of a dead worker's session"; the answered slice gets a fresh worker.)*

## Tasks

### 1. Spawn to width and assign lanes — high — blocked by: —
**Source**: §5 FR-3: "`status --json` gives `freeLanes[]` and `spawnable`; the driver spawns that many workers for the first `spawnable` entries of `ready[]`, assigns each a lane from `freeLanes` and marks the slice `building`." §3: "as many workers as `status --json` says it may".

`scheduler.rs`: replace the one-at-a-time wait with a worker table (`HashMap<slice, Running{child, lane, session_id, started}>`); each loop tick: reap exited children (completing rows and re-reading the board as slice 03 does), then read `status --json` and spawn `spawnable` workers, writing `lane: N` into each slice's frontmatter from `freeLanes` before the spawn so the next board read sees the lane held. Lane env from the lane file when present; `FACTORY_LANE` always. The tick is a `poll` on the children with a 1 s cap; no busy loop.

**Acceptance:** fixture `lanes: 2`, plan `done{delay_ms:1500}, done{delay_ms:1500}, done`: after 500 ms two stub processes are alive, slices 01 and 02 read `building` with `lane: 1` and `lane: 2`, slice 03 spawns only after 02 exits; `events.jsonl` shows two `slice_spawned` with the same second.

### 2. Park the lane, keep building — high — blocked by: 1
**Source**: §5 FR-3: "A `blocked` slice parks its lane; independent slices keep building. When nothing is spawnable and a block is open, the run parks in `waiting-owner`."

On a worker exit whose slice reads `blocked`: `slice_blocked {slice, block_id, question}` event (the id and question read from the fragment's `## Blockers`), lane stays recorded on the slice (the board keeps it out of `freeLanes`, which is the parking), loop continues. When `spawnable == 0`, no worker is running, and `openBlocks` is non-empty → phase `waiting-owner`, event `run_waiting`, and the driver keeps running at a slow tick (5 s) watching `runs/<id>/answers/` (task 3). `allDone` with no blocks → `finished` as before.

**Acceptance:** plan `blocked, done, done` with slice 03 depending on 02: slice 01 `blocked` with lane 1, slices 02 and 03 build to `done` on lane 2, then phase `waiting-owner`, driver still alive; the `blocked[]` entry in `status --json` names slice 01.

### 3. `factory answer <BLOCK-n> "<text>"` — high — blocked by: 2
**Source**: §5 FR-3: "`factory answer <BLOCK-n> "<text>"` writes the resolution into the log in the shape `autonomy.md` prescribes, flips the slice back to `building`, and the next worker on that slice gets the block and the answer in its pack." Acceptance: "an `answer` resumes it; the log shows the resolution."

`src/cli/answer.rs`: find the `BLOCK-n` heading in the slice fragments and `agent-log.md`; append `**Resolution ({YYYY-MM-DD}):** <text>` under the entry; replace `open` with `resolved` in its heading (`autonomy.md:97`); set the slice `status: building` (`:99`), lane unchanged; write `runs/<id>/answers/<BLOCK-n>.json {text, at}` and emit `block_answered`. The driver's slow tick sees the slice `building` with no running worker: it treats it as "answered" (not dead), and respawns it with a pack that carries, after the slice section, a `# Block and answer` section with the entry and the resolution verbatim. Phase back to `building`. `pack.rs` gets that optional section.

Trap: slice 03's dead-worker path fires on "`building` with no live worker". Distinguish the two by the `answers/` file or the `block_answered` event: answered → respawn without touching the dead-worker cap.

**Acceptance:** continuing the task 2 run: `factory answer BLOCK-101 "Pick the Recommended option"` → the fragment heading reads `resolved`, a `**Resolution (2026-…)**` line follows the entry, slice 01 is `building`, within 5 s a new stub call exists whose recorded pack contains both the `BLOCK-101` question and the resolution text, the stub (plan step `done`) builds it, and the run finishes with three `done` slices and `workers.outcome` = `dead` count 0.

### 4. `factory answer ASK-n` clears a preflight ask — medium — blocked by: 3
**Source**: §3 FR-1: "`ask[]` and `judge[]` go to the cockpit (FR-7) as one question each; a run never starts on an unanswered one." *(FR-1 corrected 2026-10-09 by the owner, BLOCK-201: only `ask[]` parks; `judge[]` is logged as `judge_noted` and never waited on.)* `deliver/SKILL.md:34`: "'proceed as-is' always an option."

Same command, `ASK-n` ids from slice 02: record the text in `runs/<id>/answers/ASK-n.json`, append a `DEC` line to `agent-log.md` (`- **DEC ({date})**: preflight ask "<item>" answered: <text>`), emit `ask_answered`; when every `ASK-n` has an answer, the driver (slow tick) proceeds to branch and board as slice 02 task 3 does for `pass`. A judge item is the same path.

**Acceptance:** fixture whose index drops `inner:` (so preflight asks, `docs.mjs:1708`): `factory run 001` parks with `ASK-1`; `factory answer ASK-1 proceed` → `agent-log.md` holds the DEC line, the run proceeds and builds all three slices.

### 5. Verify — medium — blocked by: 1–4
`cargo test`. The full gate runs at this slice's checkpoint.

**Acceptance:** inner loop green, one commit.

### 6. Prove — at the checkpoint — blocked by: 5
Rows from [`prd.md`](prd.md) § QA plan: **FR-3** ("stub blocks slice 01 while 02 builds; run parks; `answer` resumes it; the log shows the resolution", evidence: `events.jsonl`). At the checkpoint: the task 3 run on the fixture; attach the `events.jsonl` excerpt from `slice_blocked` to `run_finished` and the fragment's resolved entry to this slice's **Progress**.

**Acceptance:** the events excerpt shows `slice_blocked` for 01, `slice_done` for 02 and 03 before `block_answered`, then `slice_done` for 01.
