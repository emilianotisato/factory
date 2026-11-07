---
title: "factory live: cap, rev and owner-section edge cases"
state: "open"
added: "2026-11-07"
origin: "PRD-024 polish"
---

Smaller issues in `factory live`'s envelope and its owner sections, found in the PRD-024 polish review:

- **`--since` with the cap** (`src/live/mod.rs`, `cut_timeline`): the cap drains the oldest events. With `--since`, those are exactly the ones the caller has not seen, so a pager can never reach them. With `--since`, drop the newest instead, so the next call's `since` is the last kept seq.
- **`rev` is hashed before the cap**, so a write to an evidence file the cap removed still ends a `--wait`.
- **The cap can be exceeded** when the bulk is in a section neither cutter touches, such as a 600 KiB `attention[].body`. `CAP`'s doc says "never exceeds".
- **`decisions.count`** counts `Kind::Decision` only; the board and `docs log show` count decisions plus asks (`src/docs/board.rs`). Pick one meaning.
- **`log.jsonl` is parsed four times per snapshot** (`decisions`, `evidence::bodies`, and `owner_entries` twice through `now` and `attention`). Load it once into `Ctx`, with the project's `gate_failure` patterns.
- **A slice blocked with no `BLOCK-n`** (`SliceBlocked { block_id: None }`, then `run_waiting` with a non-empty `blocked`) gives no attention entry, and `waitingOn` falls through to `nothing`. `bare_park` only takes a `run_waiting` with no blocks and no asks.
- **A fix worker's cherry-pick conflict block** (`CheckpointBlocked` from `block_conflict`) gets `reason.source: gate` and the red gate's excerpt, because the reason looks for the newest red gate before the block.
- **Lane-relative labels** miss when the data dir sits behind a symlink, or the run started from a linked worktree: `lanes::dir` is lexical and keyed on `state.repo`, while the worker's cwd is canonicalized. The label falls back to the bare tool name.
- **`evidence[].stable`** compares millisecond mtimes with `now` in whole seconds, so it can stay false up to 1 s longer than 2 s.
- **A sidecar** attaches to the newest row with the same stem, which can be a `.txt` rather than the screenshot it describes.
