---
number: 023
slug: structured-status
title: "Structured run status for the herdr-app cockpit"
type: enhancement
created: 2026-11-06
author: Emiliano Tisato
summary: |
  `factory status --json` carries each run's repo, slug, title, slices, budget, gate, PR, CI and open questions as fields, each question with the exact `factory answer` argv, and `--recent` adds runs that ended in the last 7 days. Schema stays 1 and the prose stays, so the Claude Code cockpit mod needs no code change. Built for herdr-factory PRD-001, which waits on this release.
from_idea: "2026-11-06-feed-a-herdr-app-cockpit-from-factory-status"
---

# PRD-023 — Structured run status for the herdr-app cockpit

## 1. Why now

herdr-factory PRD-001 (`~/Projects/herdr-factory/docs/features/001-factory-cockpit/prd.md`) was agreed on 2026-11-06. It draws each repo's factory runs in the herdr-app spaces sidebar, and its §3 decided that factory ships the fields first: "PRD-001 cannot start until that factory release exists." A herdr-app mod can only `exec` a program and draw what it prints, so `factory status --json` is its whole data source.

Today that JSON is not enough, in five ways (idea `2026-11-06-feed-a-herdr-app-cockpit-from-factory-status`, folded in here):

- **A run does not say which repo it belongs to.** `RunJson` has `id`, `feature`, `phase`, `line`, `owner`, `board` (`src/cli/status.rs:43-50`). `repo` and `branch` live in `state.json` (`src/run.rs:96-98`) but not in the JSON, so one call cannot map runs to spaces.
- **Everything worth drawing is prose.** Slices, checkpoints, the last gate, the budget and the PR are lines of `board()` (`src/cli/status.rs:223`), such as `budget: 2.9M/30M`. A slice bar or a budget gauge would regex those strings. PRD-013 §3 accepted that "until a structured field is added". This is the reader that needs it.
- **Open questions cannot be answered from a button.** `owner` holds rendered strings (`cockpit::owner`, `src/cockpit.rs:204`). The events carry the parts apart (`SliceBlocked { slice, block_id, question }`, `src/events.rs:49-55`).
- **A finished run vanishes at once.** With no run id, the listing keeps only runs whose phase is not over (`live_runs`, `src/cockpit.rs:311-316`). The sidebar cannot say "shipped · PR #100" or "failed" while the owner still has to act on it.
- **Nothing says something changed.** Without the last event's `seq`, a reader diffs rendered strings between polls.

Left alone: the plain and `--line` outputs, the `owner` and `board` prose, and the Claude Code cockpit mod.

## 2. Blast radius

- **The Claude Code cockpit mod** reads `factory status --here --json` every 10 s (`mods/cockpit/hooks/register.tsx:63`). It marks itself stale on any `schema` other than 1 or any `factory` version other than its pin (`:48`). It reads only `id`, `feature`, `phase`, `line`, `owner`, `board` (`:32-42`) and ignores other keys. Additive fields with schema 1 keep it working; FR-4 is a flag, so its listing does not change.
- **The status JSON becomes a contract with a second reader** in another repo. Renaming or removing any field this PRD adds is a schema bump from here on (`SCHEMA`, `src/cli/status.rs:32-33`, "bumped when a field changes meaning or goes away").
- **Cost per poll with `--recent`.** A board costs about 30 ms per run (12 `factory status --json <id>` calls took 362 ms, measured 2026-11-06). 25 of today's 67 run folders were updated in the last 7 days, so a `--recent` call would build about 25 boards. herdr PRD-001 FR-3 polls every 10 s while its window is in front.

## 3. FR-1 — where a run belongs and what it is

Today a run's JSON has no repo, branch, slug, title, times or change marker (`src/cli/status.rs:43-50`, `run_json` `:123-134`).

**Decided 2026-11-06.** Each run object gains, beside the six existing keys:

- `repo`: the run's repo path as `state.json` records it (`RunState.repo`, `src/run.rs:96`), which is the git top level (`src/paths.rs`).
- `branch`: the run's branch, `null` before one is cut (`RunState.branch`, `:98`).
- `slug`: the feature folder's slug (`structured-status` for this one), from the same lookup as `title`. herdr PRD-001 FR-4 draws `019 platform-runtime`, which is a slug, not a title. `null` when no feature folder is found.
- `title`: the PRD's `title:` from its frontmatter (`src/docs/frontmatter.rs`), looked up under `docs/features/` then `docs/archive/`. `null` when no `prd.md` is found or it does not parse.
- `startedAt`, `updatedAt`: RFC 3339 UTC, as `state.json` holds them (`:101-102`).
- `elapsedSeconds`: what the glance line already computes (`span`, `src/cli/status.rs:204-213`): start to now, or to the last update for a run that is over.
- `seq`: the `seq` of the run's last readable event (`Event.seq`, `src/events.rs:277`), `0` with none.

New keys are camelCase, like `factory docs status --json` (`qaPlan`, `harnessLine`).

**Accepted cost:** the title is read from disk on every call, one small file per run. Caching it in `state.json` lost: the title can change while the run builds, and the run's state is not this PRD's to reshape.

**Acceptance**: a test pins all eight keys on a fixture run, `slug` and `title` matching its feature folder and PRD frontmatter, and `seq` equal to the last line of its `events.jsonl`. A run whose PRD is missing has `"slug": null` and `"title": null` and still lists.

*Corrected 2026-11-06 (adversarial pass): an earlier draft shipped `title` only. herdr PRD-001 FR-4 draws the slug.*

## 4. FR-2 — progress as fields

Today each of these is a phrase inside `board` (`slices_line` `:327`, `checkpoints_line` `:371`, the quota and PR lines `:262-294`).

**Decided 2026-11-06.** Each run object gains:

- `slices`: `[{"n": 1, "status": "done"}, …]` in slice order, status one of `pending`, `building`, `done`, `blocked`, read from the same board `board()` reads (`Docs::open(&s.repo).status(..)`, `:236`). `null` when that board is unavailable, as `board` then says `slices: board unavailable`.
- `budget`: `{"used": n, "limit": n}` in weighted tokens. `used` is rounded up, as the board line does (`:279-280`).
- `checkpoints`: `{"done": n, "lastGate": "green" | "red" | null}`, from the same events `checkpoints_line` counts (`GateRun`, `GateCached`, `CheckpointDone`).
- `pr`: the PR URL, `null` before one opens (`RunState.pr_url`, `src/run.rs:114`).
- `ci`: `null` with no PR. Otherwise the latest of `CiGreen` → `"green"`, `CiRed` → `"red"`, a `CiRerun` or `CiFixCommitted` after the last verdict → `"pending"`. With a PR and no verdict yet: `"pending"` while the run is live, `"none"` once it is over. A run can finish with no checks reported (`Verdict::NoChecks`, `src/closeout.rs:1376`), and `"pending"` there would never clear.
- `pausedUntil`: RFC 3339 UTC while a pause is in the future (`RunState.paused_until`, `src/run.rs:109`), else `null`.
- `parked`: the park reason (`RunParked.reason`, `src/events.rs:83-87`) while `cockpit::park` holds (`src/cockpit.rs:252-262`), else `null`.
- `driverDead`: `cockpit::driver_dead` (`:264-272`), which is already false for a run parked or waiting on preflight by design.
- `stopped`: `RunState.stopped` (`src/run.rs:106`). A stopped run's phase is `finished` too (`set_stopped`, `:377-381`), so this is the only field that tells stopped from shipped.

**Accepted cost:** `board` and these fields say the same thing twice, and the two can drift. A test that builds both from one fixture and checks they agree is the guard, not a comment.

**Acceptance**: for one fixture run in each state (building with mixed slice statuses, blocked, parked on budget, paused, driver dead, shipped with CI green, CI red, finished with no checks, stopped, failed), a test pins every field above. In the same test the `board` lines still read as today.

## 5. FR-3 — questions apart, each with its answer command

Today `owner` is a list of rendered strings (`cockpit::owner`, `src/cockpit.rs:204-216`). `factory answer` takes `--feature <NNN>` for a `BLOCK-n` and `--run <id>` for an `ASK-n` (`src/cli/answer.rs:17-29`, `:39-48`), and its text is a trailing argument (`:21`), so a flag after it becomes answer words. herdr PRD-001 FR-6 passes `--feature` for every id.

**Decided 2026-11-06.** Each run object gains `questions`, one per open id, in the order `owner` lists them: `needed` (`src/cockpit.rs:183-200`) takes each open id's **latest** opening event and sorts them by its `seq`. Every field below comes from that latest event.

*Corrected 2026-11-06 (adversarial pass): an earlier draft said "one per open id from `open_questions`, in the order `owner` lists them". `open_questions` keeps first-opened order and `owner` sorts by the latest event, so the two disagree once an id is opened twice.*

Each question:

- `id`: `BLOCK-n` or `ASK-n`.
- `at`: `slice`, `checkpoint`, `closeout`, `merge` or `preflight`, from the opening event (`SliceBlocked`, `CheckpointBlocked`, `CloseoutBlocked`, `MergeRefused`, `AskOpened`; `opens`, `:156-167`).
- `n`: the slice or checkpoint number, else `null`.
- `text`: the question in full, unrendered. For a refused merge, `checkpoint::refusal_summary(detail)`, as the owner line uses (`:62-66`).
- `answer`: the argv to run from `repo`, with the answer text appended as the last element. `["factory", "answer", "--feature", "<NNN>", "<BLOCK-n>"]` for a block, `["factory", "answer", "--run", "<run id>", "<ASK-n>"]` for an ask.

The `owner` strings, the park and the dead-driver lines stay where they are. The park and the dead driver are already FR-2's `parked` and `driverDead`. A run that is over has no questions, as `owner` has none (`:205-207`).

**Accepted cost:** factory now owns a command line a second repo runs. Changing `factory answer`'s flags means changing this argv in the same commit, which a test pins.

**Acceptance**: a test reads `questions` and runs each `answer` argv with a text appended, from `repo`. Each exits 0, writes its answer file (`src/answers.rs`), and the next `--json` no longer lists that id.

The questions must come from a real stubbed run, never from hand-written event lines. `factory answer BLOCK-n` needs the block's entry in `docs/features/*/log.jsonl` and the slice file (`src/cli/answer.rs:70-87`, `:125-126`), and an answered run with no driver spawns one (`:380-381`). The BLOCK case extends `status_json_carries_the_board_and_what_the_owner_owes` (`tests/cockpit.rs:273`), which parks a stubbed run on `BLOCK-1` and answers it. The ASK case uses the preflight ask of `a_preflight_answer_logs_one_ask_entry_and_a_retry_adds_none` (`tests/loop.rs:1202`).

*Corrected 2026-11-06 (adversarial pass): an earlier draft seeded this with `write_live_run` plus event lines. `factory answer` refuses those, so the test could never go green.*

## 6. FR-4 — `--recent`: runs that ended in the last 7 days

Today a listing with no run id keeps only runs whose phase is not over (`live_runs`, `src/cockpit.rs:311-316`).

**Decided 2026-11-06.** `factory status --json --recent` also lists the runs whose phase is over and whose `updatedAt` is within 7 days of now. The window is fixed. It combines with `--here`. It takes `requires = "json"` and `conflicts_with = "run"`, the second as `--here` already does (`src/cli/status.rs:28`). Without the flag the listing is unchanged, so the cockpit mod's status line never shows a finished run.

Live and ended runs are one list in `list_runs`' order (`src/run.rs:561-565`): oldest start first.

*Corrected 2026-11-06 (adversarial pass): an earlier draft also said "the live runs, then" the ended ones, a second order that contradicts this one. It also cited `--line` as the clap pattern; `--line` has no conflict with the run id.*

**Accepted cost:** about 25 boards per call at today's run folders, roughly 0.75 s (§2), every 10 s while herdr's window is in front. A lighter record for ended runs lost: no measurement says 0.75 s hurts, and one record shape is one thing to keep right. A `--since <days>` knob lost too: nothing has asked to turn it.

**Acceptance**: with fixture runs live, finished 2 days ago and finished 8 days ago, `--json --recent` lists the first two and `--json` lists only the live one. `--recent` without `--json`, and with a run id, exits non-zero with clap's message.

## 7. Decisions log

- ~~JSON shape~~: decided 2026-11-06, additive fields with schema 1. Schema 2 without the prose lost: it rewrites the cockpit mod in the same release for no reader's benefit.
- ~~Ended runs~~: decided 2026-11-06, a `--recent` flag, 7 days fixed. Live-only was weighed and dropped by the owner, because it would cut herdr PRD-001's shipped and failed strips and its FR-7.
- ~~Answer command~~: decided 2026-11-06, each question carries its argv.
- ~~Proof~~: decided 2026-11-06, gate plus a CLI transcript, no harness slice.
- ~~Field casing~~: decided 2026-11-06, camelCase, as `factory docs status --json`.

## 8. QA plan

**Gate**: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test && claude plugin validate mods/cockpit && claude plugin test mods/cockpit` *(declared in project.toml)*
**Surface**: CLI, the JSON `factory status --json [--recent] [--here]` prints for another program to read.
**Harness**: gate-only. Nothing a person looks at changes; the reader is herdr-factory PRD-001, built after this ships.
**Seed state**: FR-1, FR-2 and FR-4 use fixture runs written by the test, the way `write_live_run` already does (`tests/cockpit.rs:347`), plus `events.jsonl` lines for each state. FR-3 uses stubbed real runs (§5), because `factory answer` refuses event lines alone. Never this machine's live runs folder.

| FR | Proof on the real surface | Evidence | Human? |
|---|---|---|---|
| FR-1 | run `factory status --json` over a fixture run; read `repo`, `branch`, `title`, times and `seq` back against its `state.json`, PRD and `events.jsonl` | transcript + the behavior test that pins the keys | no |
| FR-2 | run `factory status --json` over one fixture run per state of §4's acceptance list | transcript + the test that pins each state's fields and the unchanged `board` | no |
| FR-3 | run each `questions[].answer` argv with a text appended, from the run's repo | transcript of both answers, the answer files, and the next `--json` without those ids | no |
| FR-4 | run `factory status --json --recent` and `factory status --json` over live, 2-day and 8-day fixture runs | transcript of both listings and of the two refused flag mixes | no |
| all | run `factory status <id> --json` over the parked stubbed run: `id`, `feature`, `phase`, `line`, `owner` and `board` read as today | `status_json_carries_the_board_and_what_the_owner_owes` passes with its existing asserts unedited | no |

*Corrected 2026-11-06 (adversarial pass): an earlier draft proved the cockpit mod by `claude plugin test mods/cockpit`. That test feeds hand-written JSON to a fake `process.run` (`mods/cockpit/tests/cockpit.test.tsx:52-68`), so it stays green whatever factory prints.*

## 9. What herdr PRD-001 must read differently

Its §3 says "the exact shape is factory's call" and its builder writes the shipped schema into its §15. Four things it will find:

- **Poll `factory status --json --recent`**, not `--json` (its FR-3). Without the flag, ended runs drop out, and its shipped and failed strips and FR-7 get no data.
- **Run `questions[].answer`** with the text appended (its FR-6). Its own `--feature` command is silently wrong for an `ASK-n` (`src/cli/answer.rs:42-43`).
- **`ci` can be `"none"`**: a run that finished with no checks reported. Its FR-4 draws only green, red or pending.
- **`slug` is the name it draws**, `title` the sentence.

## 10. Slices

_(Not sliced yet. Run `/slice`, or write "No slices — build straight from this document" if it is small.)_

## 11. Non-goals

- **The herdr-app strip, alerts, the Answer dialog and dismissing a run.** herdr-factory PRD-001 builds them. Factory ships data only.
- **Per-repo history and trends** (the last runs' duration, tokens, gate and CI reds). herdr PRD-001 put them out of scope on 2026-11-06. `factory report` prints one run.
- **Any code change to the Claude Code cockpit mod.** It keeps reading `line`, `owner` and `board`. The release that ships this bumps the mod's version pin (`register.tsx:5`), as every release does (`ca128f7`); that is the release's job, not this PRD's.
- **Not handled: a slow `--recent` call.** Accepted 2026-11-06: about 0.75 s at 25 recent runs, nothing measured hurts. Revisit if herdr's window stutters on a poll, or recent runs pass 60.
- **Not handled: a space opened inside a lane worktree.** Accepted 2026-11-06: herdr PRD-001 FR-3 shows a run on its main checkout's row only, so no lane path is exported.

## 12. Verification notes

Checked against `feat/structured-status` (from `main` at `86f34fd`) 2026-11-06. **Re-verify at slice entry.**

- `src/cli/status.rs:32-33` — `SCHEMA = 1`, "bumped when a field changes meaning or goes away".
- `src/cli/status.rs:43-50` — `RunJson` is `id`, `feature`, `phase`, `line`, `owner`, `board`.
- `src/cli/status.rs:74-89` — with no run id, the listing is `cockpit::live_runs`, filtered by `--here`.
- `src/cli/status.rs:223-312` — `board()` builds every progress fact as prose; it reads the docs board and `quota::weighted_sum`.
- `src/cockpit.rs:311-316` — `live_runs` drops every run whose phase `is_over()`.
- `src/cockpit.rs:204-216` — `owner` returns nothing for a run that is over.
- `src/cockpit.rs:140-167` — `open_questions` and `opens`: five event kinds open a question, `BlockAnswered` and `AskAnswered` close it.
- `src/cli/answer.rs:17-48` — `BLOCK-n` takes `--feature`, `ASK-n` takes `--run`; the text is `trailing_var_arg`; the repo comes from the current directory.
- `src/run.rs:57-85` — eight phases; `finished` and `failed` are over.
- `src/run.rs:377-381` — `set_stopped` sets `stopped` and phase `finished`.
- `src/closeout.rs:1371-1376` — CI green emits `CiGreen`; no checks reported emits no CI verdict.
- `mods/cockpit/hooks/register.tsx:32-48` — the mod reads six keys and checks `schema === 1` and `factory === MOD_VERSION`.
- `tests/cockpit.rs:256-334`, `:347` — existing status JSON tests and the `write_live_run` fixture.
- Measured 2026-11-06: 67 run folders, 25 updated in the last 7 days, about 30 ms per `factory status --json <id>`.
