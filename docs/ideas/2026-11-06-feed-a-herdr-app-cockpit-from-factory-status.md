---
title: "Feed a herdr-app cockpit from factory status"
state: "open"
added: "2026-11-06"
origin: "owner, herdr-factory brainstorm"
retrigger: "herdr-factory PRD-001 is agreed and names the fields it draws"
---

A sister repo, `herdr-factory`, ships a herdr-app layout and mods that coworkers clone and install. Each repo's spaces sidebar row would show that repo's factory runs: progress, who is waiting on whom, and what the run cost. herdr-app mods are JavaScript that can only `exec` a program and draw what it prints (`~/.config/herdr-app/herdr.d.ts`, `exec`), so `factory status --json` is the whole data source, as it already is for the Claude Code cockpit mod (PRD-013). Today's output is not enough for that sidebar, in five ways.

- **A run does not say which repo it belongs to.** `RunJson` carries `id`, `feature`, `phase`, `line`, `owner`, `board` (`src/cli/status.rs:43-50`). `repo` and `branch` are in `state.json` but not in the JSON. Mapping runs to spaces then needs one `--here` process per space per tick, run from that space's folder, instead of one call for every space. A space opened in a lane worktree (`~/.local/share/factory/lanes/...`) cannot be mapped to its run at all.
- **Everything worth drawing is prose.** Slices per status, lanes, checkpoints, the last gate, budget used against the limit, quota and the PR are lines of `board` (`board()`, `src/cli/status.rs:223`). A finished run prints `slices 5: done: 01, 02, 03, 04, 05 · building: — …` and `budget: 2.9M/30M`. A segmented slice bar, a budget gauge or a gate dot would have to regex those strings, and break on the first wording change. PRD-013 §3 accepted exactly this cost "until a structured field is added", because nothing read it yet. This is the reader.
- **Open questions cannot be answered from a button.** `owner` holds rendered strings (`cockpit::owner`, `src/cockpit.rs:204`), such as `BLOCK-1 slice 02: Which storage? (answer: factory answer BLOCK-1 "...")`. The events already carry the parts apart: `{"kind":"slice_blocked","block_id":"BLOCK-1","slice":2,"question":"…"}`. An Answer button needs the id, the slice and the question text as fields to call `factory answer`.
- **A finished run vanishes at once.** With no run id, the listing keeps only runs whose phase is not over (`live_runs`, `src/cockpit.rs:311-316`). The sidebar loses the run the moment it ships, so it cannot say "shipped · PR #100 · CI green" or "failed" while the owner still has to act on it. The data is there: `pr_url` in `state.json`, `ci_green` and `run_finished` in `events.jsonl`, the last with duration, slices and weighted tokens.
- **Nothing says something changed.** A mod should raise a toast and the blocked sound when a question opens, once. Without the last event's `seq` in the JSON it has to diff rendered strings between polls.

Constraints found in recon:

- The Claude Code cockpit mod marks itself stale on any `schema` other than 1 (`mods/cockpit/hooks/register.tsx:48`). Whatever changes has to keep that reader working, or move it in the same release.
- A board costs about 27 ms (measured 2026-11-06 on a finished run). There are 67 run folders today and `list_runs` loads every `state.json` (`src/run.rs:545`).
- herdr-app runs `exec` with an `on: <machine>` option, so a space on the owner's second machine asks that machine's `factory`. Nothing here needs a network API.

Insight beyond the live run, only if the herdr-factory PRD asks for it: per-repo history from the observatory (the last runs' duration, weighted tokens, gate and CI reds) would let a space show a trend, not only the run in flight. `factory report` prints one run today.
