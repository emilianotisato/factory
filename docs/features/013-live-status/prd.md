---
number: 013
slug: live-status
title: "Live run status in the cockpit"
type: feature
created: 2026-10-30
author: Emiliano Tisato
summary: |
  A Claude Code mod, bundled in the binary and installed at user scope by `factory cockpit install`, shows this repo's live runs in the cockpit session at zero model tokens: a status-line glance, a band only when the owner is needed, a full board in a pane `/factory-board` toggles. It polls a new `factory status --here --json`, whose `line`, `owner` and `board` strings the CLI renders. Agreed 2026-10-30, nothing cut; built by a factory run.
from_idea: "2026-10-30-live-run-status-in-the-cockpit-cli-feeds-and-a-fac"
---

# PRD-013 — Live run status in the cockpit

## 1. Why now

The `/deliver` cockpit sees a run only through `factory wait`, which blocks for up to 55 minutes (`--timeout 3300`, `skills/deliver/SKILL.md:23`) and prints only `still running: …` on timeout until the owner is needed (`src/cli/wait.rs:92-101`). Besides that, the skill runs `factory status <id>` once per BLOCK or failure (`skills/deliver/SKILL.md:28,34`). Watching progress today means a second terminal: `watch -n 10 factory status <id>`, or `tail -f` on the driver log. The owner hit this during the PRD-012 run (2026-10-30) and filed the idea this PRD graduates from.

One outright bug rides along. `factory status` with no id lists live runs from every repo on the machine and exits 1 (`src/cli/status.rs:23-26`). From this repo it listed a `sii-api` run, and the exit 1 reads as a failure to any script.

A Claude Code mod (a plugin hooks module) can draw inside the cockpit session with no model call. A timer (`$.clock.every`) runs the CLI (`$.process.run`) and draws the result. The CLI stays the whole data source and renders every string the mod shows, so the mod, a `statusLine` script and tmux all agree.

Left alone: `factory wait`, its cursor and the cockpit skill's `PushNotification` calls. The mod never advances `cockpit.cursor` (`src/cockpit.rs:11`).

## 2. Blast radius

- **`factory status` output.** Without the new flags it prints exactly what it prints today. The no-id listing's exit code goes from 1 to 0 (FR-3). Nothing in the skills reads that exit code: `skills/deliver/SKILL.md:50` only runs it to list runs.
- **Every Claude Code session on the machine.** A user-scope plugin loads into every session, the factory workers included. In each interactive session outside a worker, the mod spawns `factory status --here --json` every 10 s. Measured 2026-10-30 against 45 run folders: 3 ms to list, 13 ms for one board. Accepted cost: one short process every 10 s per open session. Inside a worker the mod does nothing (FR-4).
- **The owner's `~/.claude` config.** `factory cockpit install` adds a marketplace and a plugin at user scope (FR-5). It is the only verb that writes there, and only when run.
- **The gate and CI.** The gate gains two `claude plugin` commands (FR-6), so the gate needs the `claude` CLI on PATH. Every lane worker has it, since it is one. CI gains a step that installs it.

## 3. FR-1 — `factory status --json`

Today `status` prints the board as prose for a human (`board()`, `src/cli/status.rs:39-134`). A consumer that parses those lines breaks on the first wording change.

**Decided 2026-10-30.** `--json` prints one JSON object, with or without a run id:

```json
{ "schema": 1, "factory": "0.12.0", "runs": [ { … }, … ] }
```

`factory` is the binary's own version (`CARGO_PKG_VERSION`). `runs` holds the one run named, or the live runs the listing would show (FR-3 narrows them). Each run carries:

- `id`, `feature`, `phase` (the `Phase` display string, `src/run.rs:69-90`).
- `line`: the glance line of FR-2.
- `owner`: one string per thing the owner must act on now, open questions in seq order, then the park, then a dead driver last (it has no seq). Each open question renders as `cockpit::line` renders its opening event (`src/cockpit.rs:43`), for example `BLOCK-1 slice 02: Which storage? (answer: factory answer BLOCK-1 "...")`. A park adds the park line, and a dead driver adds `driver dead; run: factory run NNN`. The questions are every open one (`open_questions`, `src/cockpit.rs:140`), each from its latest opening event. They are not `wait`'s owed set (`owed`, `:170`), which keeps only what the cursor has already passed. The park comes from `parked_on` (`:222`), the dead driver from `driver_dead` (`:234`). Empty when nothing is needed.
- `board`: the lines `factory status <id>` prints today, unchanged (`board()`).

Accepted cost: the mod draws strings the CLI rendered and parses no structure out of them. A consumer that wants a figure (slices done, budget used) has to parse a string until a structured field is added. The structured shape lost because nothing reads it yet (§12).

**Acceptance**: on a fixture run with one open BLOCK, `factory status <id> --json` parses as JSON. Its `runs[0].board` equals the plain output line for line. Its `owner` holds exactly the BLOCK's `wait` line. After `factory answer`, `owner` is `[]`.

## 4. FR-2 — the glance line

There is no short form today; the board is 8 to 10 lines (`src/cli/status.rs:50-132`, `tests/cockpit.rs:165`).

**Decided 2026-10-30.** The `line` field, and `factory status [RUN] --line`, which prints one `line` per run and nothing when there is none. The format, with the feature number first:

| state | line |
|---|---|
| building | `013 · building · 1/3 done · 12m` |
| owner needed | `013 · BLOCK-1 open` (several: `013 · BLOCK-1, ASK-2 open`) |
| parked | `013 · parked: budget` |
| usage pause | `013 · paused until 14:30` |
| driver dead | `013 · driver dead` |
| run over (named by id) | `013 · finished · 3/3 done · 1h 02m` |

The rows apply top-down, so the first matching state wins, with one exception: the progress row (`building` above) is the fallback, tried last, and shows the run's own phase (`starting`, `checkpoint`, `closeout`). A run that is over goes straight to the over row, as `owed` skips it (`src/cockpit.rs:171-173`). The time is elapsed in whole minutes (`12m`), or hours and minutes from one hour (`1h 02m`). The board's `elapsed` (`src/cli/status.rs:136`) prints seconds under an hour, so the line needs its own short form. The pause time is local `HH:MM`. A board that will not open drops the `n/m done` part.

The line is a pure function of a small input: phase, open ids, parked reason, pause instant, driver dead, slices done and total (or none), elapsed seconds. `status` fills it from state, board, events and the clock.

*Corrected 2026-10-30 (slicing): read literally, the `building` row came first, so a dead driver or an open BLOCK during the `building` phase could never show. The progress row is the fallback.*

*Corrected 2026-10-30 (adversarial pass): an earlier draft pinned the rows "from a fixture event list". Events alone do not decide them: `n/m done` comes from the docs board, elapsed from `started_at`, the pause from `state.paused_until`, a park from the phase and a dead driver from pid liveness.*

**Acceptance**: a unit test pins each row by calling that function with each row's input. No line is longer than 60 characters for a feature number below 1000 with three or fewer open ids.

## 5. FR-3 — this repo's runs, and a listing that exits 0

`list_live` prints every live run under the data dir, whatever its repo (`src/cockpit.rs:277-308`), and `status` with no id returns 1 (`src/cli/status.rs:25`).

**Decided 2026-10-30.** `--here` keeps only the runs whose `repo` is the git top level of the current directory, both sides canonicalized. Outside a git repo, `--here` lists nothing. A listing exits 0 whether or not it found runs, with or without `--here`. `no run <id>` still exits 1. `--here` combines with `--json` and `--line`.

Accepted cost: a script that read exit 1 as "no run given" now reads success. No skill does (§2). `tests/cockpit.rs:191-192` asserts the old exit 1 and changes with this FR.

*Corrected 2026-10-30 (adversarial pass): an earlier draft said nothing in this repo reads that exit code. The cockpit test does.*

**Acceptance**: with two live fixture runs in two repos, `factory status --here --json` from one repo's subdirectory lists only that repo's run and exits 0. From outside both repos it prints `{"schema":1,…,"runs":[]}` and exits 0.

## 6. FR-4 — the cockpit mod

**Decided 2026-10-30.** A hooks module bundled in this repo at `mods/cockpit/`: `.claude-plugin/plugin.json` (name `factory-cockpit`, `version` equal to the crate's), `hooks/hooks.json`, `hooks/register.tsx`, its tests, and `types/index.d.ts` if it keeps `$.state`. The binary embeds it with `include_str!`, as it embeds the skills (`src/skills.rs:28-81`).

- **Idle in workers.** `$.env.get("FACTORY_REPO")` is set → the mod registers nothing that polls or draws. Workers get that variable (`src/worker.rs:259`).
- **Poll.** Every 10 s, `factory status --here --json`, run in the session's directory.
- **Status line.** Every run's `line`, joined with `  |  `, through `$.ui.status`. With `runs` empty it is cleared (`undefined`), unless the mod is stale (below).
- **Band above the prompt.** Drawn only while some run's `owner` is non-empty, one row per `owner` string, each prefixed with the feature number. Otherwise the hook answers `next(e)` and draws nothing.
- **Pane.** `/factory-board` toggles a pane titled `factory` that shows every run's `board`, one run after another. With no live run it says `no live run in this repo`.
- **Stale mod.** When the JSON's `factory` version differs from the mod's own, the status line ends with `· mod stale: factory cockpit install, then /reload-plugins`. With no live run the status line is the suffix alone: `factory: mod stale: factory cockpit install, then /reload-plugins`.

*Corrected 2026-10-30 (adversarial pass): an earlier draft both cleared the line on empty `runs` and showed the suffix with no run live. The stale suffix wins.*
- **Failure.** `factory` missing, a non-zero exit, or JSON that does not parse → the status line shows `factory: ` plus the first line of the error, and the next poll tries again. An unknown `schema` counts as stale.

Accepted cost: the band appears only when the owner is needed. Progress shows in the status line and the pane alone, so a session without the pane open sees no slice-by-slice detail.

**Acceptance**: `claude plugin test mods/cockpit` covers each of these. A fixture JSON with one BLOCK draws one band row, and an empty `owner` draws none. An empty `runs` clears the status line, and an empty `runs` with a different version shows the suffix alone. A different `factory` version adds the stale suffix. A non-zero `factory` exit shows the error. `FACTORY_REPO` set means no poll at all. `/factory-board` opens the pane, and a second call closes it.

## 7. FR-5 — `factory cockpit install`

There is no install path today.

**Decided 2026-10-30.** `factory cockpit install` does three steps.

1. It writes a folder marketplace at `<data_dir>/mods/`: `.claude-plugin/marketplace.json` names the marketplace `factory`, with one plugin `factory-cockpit` whose source is `./cockpit`. The bundled files go under `cockpit/`, replaced whole. `<data_dir>` is `paths::data_dir()` (`src/paths.rs:13`).
2. Unless `claude plugin marketplace list` already shows it, it runs `claude plugin marketplace add <data_dir>/mods`.
3. Unless `claude plugin list` already shows it, it runs `claude plugin install factory-cockpit@factory --scope user`.

It then prints `run /reload-plugins in open sessions`. Re-running it is safe and is the upgrade path, so FR-4's stale suffix names it. With `claude` not on PATH, it writes the folder, prints the two commands to run by hand, and exits 1.

A folder marketplace is read from the folder itself. An edited folder reaches a session on `/reload-plugins`, with no reinstall (Claude Code plugin reference, 2.1.312).

Accepted cost: an upgraded binary leaves the installed mod stale until the owner re-runs the verb. The stale suffix makes that visible. The alternatives each lost (2026-10-30): a refresh inside `factory skills update` misses source installs and repos that are already current (`src/cli/skills.rs:33-38` and `:67-69` return before rendering). A rewrite inside `status` turns a read verb into a writer every 10 s.

**Acceptance**: with a sandboxed `HOME` and `FACTORY_DATA_DIR` and the stub `claude` on PATH, a first run writes the folder and calls `marketplace add` then `install`. A second run rewrites the folder and calls neither. The written `cockpit/` matches `mods/cockpit/` byte for byte.

## 8. FR-6 — the bundled mod in the gate

**Decided 2026-10-30.** The gate in `docs/project.toml` gains `claude plugin validate mods/cockpit && claude plugin test mods/cockpit`. CI gains a step that installs the `claude` CLI at a pinned version, before the gate step (`.github/workflows/ci.yml:20`, which today runs the cargo gate alone). A cargo test pins `mods/cockpit/.claude-plugin/plugin.json`'s `version` and the mod's own version constant to `CARGO_PKG_VERSION`. A release bump then cannot ship a mod that calls itself stale.

Both commands ran clean with an empty `HOME` and no login (probe, 2026-10-30, Claude Code 2.1.312), so CI needs the binary only.

Accepted cost: a local gate needs `claude` on PATH, and CI pays an install step per run.

**Acceptance**: a deliberately broken `register.tsx` turns the local gate red. The PR's CI log shows both `claude plugin` commands ran and passed. Bumping `Cargo.toml` without the mod turns `cargo test` red.

## 9. Decisions log

- ~~**OPEN-1** — Route A (the project's `.claude/skills/<name>/`, auto-loaded) or Route B (folder marketplace at user scope)?~~ **2026-10-30: Route B, own verb.** Route A loads in every lane worktree, because those are checkouts that track `.claude/skills/` (`git ls-files .claude`). Its consent prompt is also unverified.
- ~~**OPEN-2** — One JSON schema for `status` and an `events` verb, or two?~~ **2026-10-30: no `events` verb** (§12). One schema, versioned by `schema`.
- ~~**OPEN-3** — Should the cockpit skill swap `PushNotification` for mod notifications?~~ **2026-10-30: no mod notifications** (§12). The skill is untouched.
- ~~**OPEN-4** — How does the mod stay current after an upgrade?~~ **2026-10-30: the mod flags itself stale, and `factory cockpit install` fixes it** (FR-4, FR-5).

## 10. QA plan

**Gate**: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`, plus FR-6's two `claude plugin` commands *(declared in project.toml)*
**Surface**: the CLI (`factory status`, `factory cockpit install`), and the Claude Code cockpit session the mod draws in.
**Harness**: `tests/harness` (the `Fixture`) with the stub `claude`, as `tests/cockpit.rs` drives a run. Plus `claude plugin test` for the mod. No new harness.
**Seed state**: fixture runs written by the harness. Never the owner's real data dir or `HOME`: FR-5's proof sets both to temp paths.

| FR | Proof on the real surface | Evidence | Human? |
|---|---|---|---|
| FR-1 | run `factory status <id> --json` on a fixture run before and after `factory answer`; test `status_json_carries_the_board_and_what_the_owner_owes` | test transcript | no |
| FR-2 | run `factory status <id> --line` across the table's states; unit test `the_glance_line_names_the_first_state_that_applies` | test transcript | no |
| FR-3 | run `factory status --here --json` from inside and outside a fixture repo with two live runs; test `here_keeps_this_repos_runs_and_a_listing_exits_zero` | test transcript | no |
| FR-4 | run `claude plugin test mods/cockpit` | test transcript | no |
| FR-4 | in a real cockpit during a live run: the status line moves, a BLOCK raises the band, `/factory-board` opens the pane, and nothing shows in a repo with no run | the owner's look | **yes**: readability at real terminal widths |
| FR-5 | run `factory cockpit install` twice under a sandboxed `HOME`; test `install_writes_the_marketplace_once_and_rewrites_the_mod` | test transcript | no |
| FR-6 | run the gate with a broken `register.tsx`, see red; read the PR's CI log for the two `claude plugin` steps | gate transcript, CI log | no |

**Who builds it**: a `factory` run, every FR unattended. The one human row is a judgment made after the run, at review.

## 11. Slices

Three slices, sliced 2026-10-30.

- **01 — Status as data** (FR-1, FR-2, FR-3). The CLI side: `--json`, `--line`, `--here`, and the listing's exit 0. One slice because all three write `src/cli/status.rs`.
- **02 — The cockpit mod and its gate** (FR-4, FR-6). The highest-risk work: the first mod in this repo. It builds against the envelope §3 defines, from fixture JSON, so it runs beside 01.
- **03 — `factory cockpit install`** (FR-5, and FR-4's embedding). Depends on 02, whose files it embeds and compares byte for byte.

**Order and shape.** 01 and 02 are independent and build at once in two lanes; 03 follows 02. The first checkpoint lands after the second slice done, so the JSON contract and the mod are proven before the installer is built on them. The last checkpoint covers 03.

**Workflow.** One branch (`feat/live-status`), one PR, one commit per slice, Prove at the checkpoints. FR-6's CI-log row is read at polish, once the PR's CI has run. The one Human row is in [`qa-guide.md`](qa-guide.md), for the owner at review.

## 12. Non-goals

- **A cursor-free `factory events` verb.** The idea's safety requirement for a second event reader. Without notifications the mod reads only `status`, so `wait`'s cursor has no second reader. If notifications return, diff successive `status --json` snapshots first (owner, 2026-10-30).
- **Not handled: native notifications from the mod.** Accepted 2026-10-30: the cockpit already pushes every BLOCK, ASK, park and end to the phone (`skills/deliver/SKILL.md:27`). Revisit if the owner misses one at the desk.
- **Not handled: structured board fields** (slices by state, quota, gate result) in the JSON. Accepted 2026-10-30: no consumer reads them; the mod draws CLI-rendered strings. Revisit when a consumer needs a figure.
- **Not handled: `factory status --follow`.** Accepted 2026-10-30: polling costs 3-13 ms per call. Revisit if a poll ever shows up in a profile.
- **Not handled: `--agent` and other providers** (Codex and others). Accepted 2026-10-30: builders and the cockpit run only Claude Code. Review passes spawn another vendor's CLI (`cfg.agent`, `src/config.rs:69`), but they have no cockpit session. Revisit when it drives another; research that provider's extension surface then.
- **Not handled: a poll-interval knob or a slower poll with no live run.** Accepted 2026-10-30: no evidence the 10 s poll costs anything. Revisit on a measured cost.
- **Not handled: a `factory docs doctor` line for the mod.** Accepted 2026-10-30: the mod's own stale suffix covers it.
- **Unchanged: `factory wait`, `cockpit.cursor`, the `/deliver` skill.**

## 13. Verification notes

Checked against `main` at `e0541dc`, 2026-10-30, with Claude Code 2.1.312. Adversarial pass: grok-4.7-high via `factory review second`, 12 findings, all corrected inline. **Re-verify at slice entry.**

- `src/cli/status.rs:23-26`: no id → `list_live`, return 1.
- `src/cli/status.rs:39-134`: `board()`, 8 lines plus up to two more; `:30` prints at most ten.
- `src/cockpit.rs:43`: `line()`, the owner-facing line of each cockpit event; `:140` `open_questions`; `:170` `owed`; `:234` `driver_dead`.
- `src/cockpit.rs:277-308`: `list_live` filters by phase only, never by repo.
- `src/cockpit.rs:11`, `src/cli/wait.rs:35-60`: `wait` reads and advances `cockpit.cursor`. This PRD touches neither.
- `src/cli/run.rs:61`: `Run::find_live` keys on repo **and** feature, so one repo can hold several live runs.
- `src/docs/board.rs:420`: `board::status` runs one local `git rev-parse` per board. The idea said status makes no git call; it makes this one, which is local and fast.
- `src/worker.rs:217,259`: workers run `claude -p` with `FACTORY_REPO` set, in lane worktrees.
- `git ls-files .claude`: `.claude/skills/*` is tracked, so every lane worktree carries it (why Route A lost).
- `src/cli/skills.rs:33-38`, `:67-69`: `skills update` returns early in a source checkout and when current.
- `tests/cockpit.rs:191-192`: asserts the no-id listing exits 1 (FR-3 changes it).
- `src/skills.rs:28-81`: skills embedded with `include_str!`, the precedent for `mods/cockpit/`.
- `src/paths.rs:13`: `data_dir()` honours `FACTORY_DATA_DIR`, then `XDG_DATA_HOME`.
- Claude Code 2.1.312 plugin reference: a project `.claude/skills/<name>` plugin auto-loads. A folder-marketplace plugin is read from its folder and re-read on `/reload-plugins`. `$.clock.every`, `$.process.run`, `$.env.get`, `$.ui.status`, the `AbovePrompt` band, `$.ui.open` panes and `$.command.register` all exist.
- Probe 2026-10-30: `claude plugin validate` and `claude plugin test` both run with an empty `HOME` and no login.
- Timing 2026-10-30, 45 run folders: `factory status` 3 ms, `factory status <id>` 13 ms.
- **Corrected 2026-10-30:** the idea said `wait` blocks "for up to 25 minutes". PRD-012 made the cockpit's poll 55 minutes (`skills/deliver/SKILL.md:23`).
