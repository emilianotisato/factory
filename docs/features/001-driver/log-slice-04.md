## Traps

- A new `tests/*.rs` file that says `mod harness;` fails with E0761: `tests/harness.rs` (the harness's own test) and `tests/harness/mod.rs` both exist — write `#[path = "harness/mod.rs"] mod harness;` as `loop.rs` and `run.rs` do.
- Every pack carries `assets/autonomy.md`, whose § Resuming quotes `**Resolution ({date}):**` — a stub that looks for that string to tell an answered respawn from a first spawn sees it every time and builds the slice to `done` instead of blocking it. The driver's `# Block and answer` heading is the only reliable mark.
- A fixture lives in a `TempDir` that a panic unwinds away, so a detached driver's `driver.log`, `events.jsonl` and `state.json` are gone before anyone can read them — `tests/lanes.rs::wait_until` dumps all three on timeout; without that the first `waiting-owner` timeout was a 60 s blank.

## Blockers

_(none)_

## Decisions

### AD-401 — Who keeps a blocked slice's lane out of `freeLanes`? · slice-04 · 2026-10-10

**Chose:** the driver: `scheduler::free_lanes` subtracts the `lane:` of every slice in the board's `blocked[]` (and of the worker table) from the board's `freeLanes`.
**Over:** trusting the board, as task 2 reads it ("the board keeps it out of `freeLanes`, which is the parking"), or changing `docs.mjs`.
**Because:** `docs.mjs status` computes `heldLanes` from `building[]` only, so a blocked slice's lane is free again on the next read and the third slice would have taken lane 1 beside the parked block. The tool is out of this repo's hands; the parking is the driver's rule to keep, and the acceptance (02 and 03 on lane 2) holds.
**Undo:** drop the `blocked` filter in `free_lanes` once the tool holds the lane itself.
**Review:** fyi

### AD-402 — Same second, or one tick, for the two-spawn acceptance? · slice-04 · 2026-10-10

**Chose:** `tests/lanes.rs` asserts the two `slice_spawned` timestamps are at most 1 s apart (and the two workers alive together, both `building` on lanes 1 and 2 before either exits).
**Over:** the literal "two `slice_spawned` with the same second" of task 1's acceptance.
**Because:** both spawns happen in one tick, but a tick that starts at `.98` crosses a second boundary between them; a `docs status` read sits in between. The same-second form fails on timing the code does not control. What the criterion measures — one tick, not one-after-the-other — is what the gap and the two-alive assertions pin.
**Undo:** `gap == 0`.
**Review:** fyi

### AD-403 — Checkpoint or park when both are due? · slice-04 · 2026-10-10

**Chose:** the checkpoint wins: with the table empty, `checkpoint_due` ends the loop at phase `checkpoint` before open blocks park it.
**Over:** parking first, or spawning across a due checkpoint.
**Because:** slice 03 settled that a due checkpoint stops spawns (AD-302) and slice 06 owns what happens next; a parked run with verified-nothing would hide the checkpoint behind the owner's answer. The cost: task 2's acceptance (`blocked, done, done` → `waiting-owner`) only holds when the checkpoint is not due at that point — with the fixture's default `checkpoint_every: 2`, two `done` slices make it due — so the lanes tests set `checkpoint_every: 10`.
**Undo:** swap the two branches at the bottom of `drive_inner`.
**Review:** fyi

### AD-404 — Who starts the driver of an ask-parked run, and from which preflight? · slice-04 · 2026-10-10

**Chose:** `factory answer` launches the daemon (`daemon::spawn`) when the run's `pid` is dead and its phase is `waiting-owner`; the driver then takes the branch from `runs/<id>/preflight.json`, which `factory run` writes at ask time, and runs `begin` (branch, `judge_noted`, `run_started`, board) exactly as the `pass` path does. A run parked on asks that still has a live driver is picked up on its slow tick.
**Over:** telling the owner to run `factory run NNN` after the last answer (two commands for one act), or re-running preflight in the driver (a second `docs preflight` whose asks may differ from the ones answered).
**Because:** task 4 says "the driver (slow tick) proceeds", but an ask-parked run has no driver — `factory run` returns after printing the asks. The answer is the one owner command the slice promises. The saved preflight makes the run proceed on the questions the owner actually answered.
**Undo:** remove the `daemon::spawn` branch of `answer::wake` and `load_preflight`.
**Review:** fyi

### AD-405 — How do the slice-03 tests survive two lanes? · slice-04 · 2026-10-10

**Chose:** the serial-order tests chain the fixture (`Fixture::chain()`: slice 02 `depends_on: [1]`, rendered and committed) so the plan still runs 01, 02, 03 in order; the stub takes its call number — hence its plan line — from the pack name's spawn suffix (`slice-NN-<spawn>.md`), not from the race to claim `calls/N.json`; `two_dead_workers_block_the_slice_and_park_the_run` runs detached and ends with `factory stop`, since a run with an open block now parks alive instead of finishing.
**Over:** rewriting the slice-03 assertions around either order, or giving the stub a lock.
**Because:** with `lanes: 2` in the index, 01 and 02 spawn together and the stub's "first free number" let whichever started first eat the other's plan line — a `dead` meant for 01 killed 02. The spawn suffix is the driver's order, which is the plan's.
**Undo:** `chain()` is one call per test; the stub falls back to the first free number when the pack name carries no suffix.
**Review:** fyi

### AD-406 — What does an answer file carry? · slice-04 · 2026-10-10

**Chose:** `runs/<id>/answers/<ID>.json` is `{id, slice, text, at}` — the slice an answered `BLOCK-n` belongs to, `null` for an `ASK-n`.
**Over:** the `{text, at}` of task 3.
**Because:** the driver finds the answer for a `building` slice nobody runs by that `slice` field, which is what tells an answered slice from a dead worker's (task 3's trap) without parsing the fragment again.
**Undo:** drop the two fields and look the block up by id.
**Review:** no
