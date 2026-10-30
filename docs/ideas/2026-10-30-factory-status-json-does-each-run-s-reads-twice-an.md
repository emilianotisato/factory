---
title: "factory status --json does each run's reads twice and opens its db for --line"
state: "open"
added: "2026-10-30"
origin: "PRD-013 polish"
---

The cockpit mod runs `factory status --here --json` every 10 s per open session. Per live run, `run_json` (`src/cli/status.rs`) reads `events.jsonl` and opens the docs board once for `line_of`, then again inside `board()`. Every listing also `Run::open`s each run, which opens the sqlite db and runs `migrate` in an IMMEDIATE transaction (`src/db.rs`, busy timeout 5 s), even for `--line`, which never reads the db. Measured 13 ms per board (PRD-013 §13), so nothing hurts yet; a long run's large log or a driver holding a write lock is where it would. Shape of a fix: `board_from(run, &events, docs_status)` shared by both, and a read-only state + events path for `--line`.
