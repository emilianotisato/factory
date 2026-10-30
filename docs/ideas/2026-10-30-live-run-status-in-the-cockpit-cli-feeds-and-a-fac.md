---
title: "Live run status in the cockpit: CLI feeds and a factory-shipped Claude mod"
state: "open"
added: "2026-10-30"
origin: "owner"
retrigger: "PRD-012 merged"
---

The `/deliver` cockpit sees a run only through `factory wait`, which blocks for up to 25 minutes and prints nothing until the owner is needed. Watching progress today means a second terminal (`watch -n 10 factory status <id>`, or `tail -f` on the driver log). A Claude Code mod (plugin hooks module) can show the board inside the cockpit session at zero model tokens: a band above the prompt, a status-line entry, a side pane, and native notifications, refreshed by a timer (`$.clock.every`) that runs the CLI (`$.process.run`). The mod never touches the model; the CLI is the whole data source. Raised by the owner during the PRD-012 run (2026-10-30).

## What the CLI lacks for that

1. **`factory status [RUN] --json`.** The board is prose built for a human; a mod parsing it breaks on the first wording change. One JSON object with the fields the board already shows: run id, PRD, phase (`closeout`, …), elapsed, slices by state, lanes, checkpoints and the last gate result, open questions (BLOCK-n / ASK-n with their text), quota and budget, PR url, the last few events. `factory report --json` is the precedent.
2. **`factory status [RUN] --line`.** One line under ~60 characters for a status line or a phone glance, e.g. `012 · closeout · 2/2 done · gate green · 15m`, and `012 · BLOCK-1 open` when the owner is needed. The format lives in the CLI, so every consumer (mod, `statusLine` script, tmux) shows the same thing.
3. **A cursor-free event read**, e.g. `factory events <RUN> [--since <seq>] [--json]`. `wait` advances the cockpit's cursor, so a mod that used it to raise notifications would swallow BLOCK / ASK / finished before the cockpit saw them. The mod needs to read the same cockpit events by sequence number and keep its own position. **This is the safety requirement**; without it a second reader is a hazard.
4. **Live runs filtered to this repo, exit 0.** `factory status` with no id lists live runs from every repo on the machine (seen: a `sii-api` run listed from this repo) and exits 1. A `--here` (or default-to-cwd) filter that exits 0, plus `--json`, lets a mod find "this repo's run" with no id handed to it.

## Nice to have

- **`factory status <RUN> --follow`**: one JSON line per board change, so a consumer spawns once (`$.process.spawn`) instead of polling. Polling every ~10 s is cheap enough to ship without it.
- Keep `status` fast and offline (no git or network calls) so a 10 s poll is free.

## The cockpit mod, shipped by factory

The mod belongs in this repo and in the binary, like the skills: factory owns its assets, and a mod copied by hand drifts from the CLI it reads. It is a small hooks module: a band above the prompt with the board, a status-line entry from `--line`, an optional side pane, and `$.ui.notify` on BLOCK / ASK / parked / finished read from `events` (never `wait`).

It needs an install path, Claude Code only for now:

- **`factory cockpit install [--agent claude]`** (name open) writes the bundled mod and registers it. `--agent` takes only `claude` today; when factory drives other providers (Codex, Grok, ChatGPT), each gets researched for its own extension surface and added behind the same flag.
- **Route A, no new step:** Claude Code auto-loads a plugin folder placed in a project's `.claude/skills/<name>/` (manifest `.claude-plugin/plugin.json` + `hooks/hooks.json` + the module). Factory already writes `.claude/skills/`, so `factory init` and `factory skills update` could lay the mod there and `factory docs doctor` could report it stale like a skill. Unverified: whether that auto-load asks for consent or workspace trust first.
- **Route B:** write the mod to a folder factory owns (e.g. `~/.local/share/factory/mods/`), then `claude plugin marketplace add <folder>` and `claude plugin install`. A folder marketplace is read from the folder itself, so a factory upgrade reaches the session on `/reload-plugins` with no reinstall.
- A release bumps the mod with the skills (version bump, tag, `factory skills update`), and `claude plugin validate` belongs in the gate for the bundled copy.

A plain `statusLine` script in `settings.json` can consume `--line` without the mod, but it refreshes on chat activity, not on a timer.

## Open questions for the PRD

- JSON shape: one versioned schema shared by `status --json` and `events --json`, or two?
- Does `events` replace `wait --replay` internally, with `wait` becoming "events + block + advance the cursor"?
- Should the cockpit skill switch its `PushNotification` calls to the mod's notifications when the mod is present?
- Route A or Route B, and is install part of `factory init` or its own verb?
- Other providers: which extension surface each one has, researched when factory first drives them.
