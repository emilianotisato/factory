---
number: 012
slug: one-tap-build
title: "From an agreed PRD to a running build in one tap"
# type: feature | bugfix | enhancement | refactor
type: enhancement
created: 2026-10-30
author: Emiliano Tisato
summary: |
  An agreed PRD ends with one AskUserQuestion; "Build it now" slices, clears what preflight can fix alone, and starts the run in the same session, which then waits on it with a 55-minute poll. Stale skills or binary update themselves with a chore commit. Blast radius becomes optional and `author` comes from git. Ready, built by a factory run.
---

# PRD-012 — From an agreed PRD to a running build in one tap

## 1. Why now

The owner accepts the slice plan every time, and still has to drive four manual steps after the interview: finish `/prd`, type `/slice`, type `/clear`, type `/deliver NNN`. Each one waits for a typed reply, so the phone never buzzes and the build starts only when the owner comes back to the terminal.

- **Pain, every feature:** the hand-off in `/prd` §8 (`skills/prd/SKILL.md:165`) tells the owner to run `/slice`, then `/clear`, then `/deliver NNN`. `/slice` then stops at "Write these? [Y/n]" (`skills/slice/SKILL.md:107`), the one answer the owner never changes.
- **Pain, most features:** `doctor` reports the skills or the binary behind, the skill asks, and the answer is always "update in this branch, chore commit here" (owner, 2026-10-30).
- **Improvement:** `## 2. Blast radius` is a required section that is often empty for internal work, and `author: _(name)_` is never filled (`assets/docs/prd.scaffold.md:8`).

Left alone: the interview itself (the owner says it works), how a run builds, and the review interview at the end of `/deliver`.

## 2. Blast radius

- **Every repo that runs `factory skills update`** gets the new `/prd`, `/slice`, `/deliver` and `/lanes` text. The old three-step flow keeps working: `/slice` typed by hand still asks "Write these?", and `/deliver NNN` typed by hand gets every change in FR-2 to FR-5 (it slices, fixes mechanical refusals, updates stale skills, waits 55 minutes).
- **The cockpit holds the interview's context** for the whole run (FR-5). That is a token cost per run, measured, not a contract break.
- **`factory docs new`** writes a real author (FR-7). The frontmatter keys do not change, so `factory docs check` and the scaffold test keep their seven keys.

## 3. FR-1 — `/prd` offers the build with one tap

**Today.** `/prd` §8 ends in prose. Its "built without them" line says to run `/slice`, then `/clear`, then `/deliver NNN`, and to "say the `/clear` part" (`skills/prd/SKILL.md:165`).

**Decision (2026-10-30).** When §7 has set the PRD `ready` and committed it, §8 asks one `AskUserQuestion`: **Build it now (Recommended)** / **Slice only** / **Stop here**. The question exists because a tool question notifies the phone and a prose offer does not.

- **Build it now** → invoke `/deliver NNN` through the Skill tool in the same session. `/deliver` slices it (FR-2).
- **Slice only** → invoke `/slice NNN`, attended as today.
- **Stop here** → end with the hand-off line as today, minus the `/clear` advice.

A PRD still `draft` (open OPENs) gets no build offer: §8 names the OPENs, as today. The `/clear` sentence goes. The owner may still clear and type `/deliver NNN` at any time (FR-5).

**Accepted cost:** the cockpit carries the interview's 100–250k of context for the whole run (FR-5 measures it). The alternative, a clean session, needs the owner to type `/clear`, and no skill can type it (§13).

**Acceptance:** after the owner agrees a PRD, `/prd`'s last act is one `AskUserQuestion` with those three options, Build first. Picking Build starts `/deliver NNN` with no further typed command.

## 4. FR-2 — `/deliver` slices an unsliced PRD itself

**Today.** `/deliver` §1 runs `factory run NNN` straight away (`skills/deliver/SKILL.md:12`). Preflight does not refuse a feature with no slices (`src/docs/preflight.rs:255-345` has no slice-count check), so the run starts, finds nothing ready, and ends `nothing ready` (`src/scheduler.rs:477`): `all_done` needs at least one slice (`src/docs/board.rs:307`). `/slice` calls itself attended work (`skills/slice/SKILL.md:13`) and asks "Write these? [Y/n]" before writing (`:107`, `:114`).

**Decision (2026-10-30).**

1. `/deliver` §1 checks `factory docs status NNN`. No slices → it invokes `/slice NNN` first.
2. `/slice`, when `/deliver` called it, **reports the plan and writes it** with no "Write these?". It still asks only where it would stop today: a task with no source in the PRD (`skills/slice/SKILL.md:87`), a load-bearing `OPEN-n`, no gate, no QA plan.
3. A small PRD headed for a run still gets exactly one slice (`skills/slice/SKILL.md:37`). That line's `/factory-deliver` becomes `/deliver`.
4. `/slice` typed by the owner keeps "Write these? [Y/n]".
5. **Defensive:** `factory docs preflight` refuses a feature with zero slices: `not sliced — /slice NNN`. Today that run starts and ends with nothing built.

**Accepted cost:** a slice plan the owner would have changed gets built. The owner has accepted every plan so far, and the board is in the report, so a bad one is visible before the first builder finishes.

**Acceptance:** `/deliver NNN` on a `ready` PRD with no slices, a gate, a QA plan, no load-bearing OPEN and no unsourced task writes and commits the slices, prints the board, and starts the run without asking. `factory docs preflight NNN` on a fixture feature with no slices exits 1 with `not sliced`.

## 5. FR-3 — `/deliver` clears what preflight can fix alone

**Today.** `factory run` runs preflight first. A refusal prints `✗` lines and exits 1 (`src/cli/run.rs:93-99`). Asks already become `ASK-n` events that `/deliver` relays (`src/cli/run.rs:106-127`). Judge lines are noted only. The branch is made by the run itself when HEAD is on the default branch (`src/cli/run.rs:131`, `:289`).

**Decision (2026-10-30).** On a refusal, `/deliver` fixes the mechanical ones and runs `factory run NNN` again:

- **Working tree not clean** with only this feature's docs in it → commit them by pathspec (`docs(NNN): …`). Any other dirty path → ask.
- **docs check errors** in this feature's own files → fix them, commit, retry.
- **Not sliced** (FR-2's refusal) → `/slice NNN`.

Everything else is relayed as one `AskUserQuestion` each, with the fix it needs as option 1: no gate (`/gate`), no lanes seed (`/lanes`), a branch the owner must pick, an open blocker, an archived PRD. `/deliver` never runs an interview skill unasked. Judge lines stay notes.

**Accepted cost:** `/deliver` commits on the owner's behalf. A refusal fix commits only paths under `docs/features/NNN-*`, which it or `/prd` wrote. FR-4's chore commit is the one other commit, and it holds only the skill dirs `factory skills update` names.

**Acceptance:** with this PRD's own docs uncommitted, `/deliver NNN` commits them and the run starts with no question. With an unrelated file dirty, it asks.

## 6. FR-4 — stale tooling updates itself

**Today.** `/prd` §0, `/slice` §0 and `/lanes` §0 run `factory docs doctor` (`skills/prd/SKILL.md:20`, `skills/slice/SKILL.md:20`, `skills/lanes/SKILL.md:17`). `/slice` routes "Not on the framework, behind, or drifted" to `/prd` (`skills/slice/SKILL.md:23`). The skills verdict prints a version line, `behind (repo X, binary Y) → factory skills update` or `ahead (repo X, binary Y) → <hint>`, and then `hand-edited: …` and `missing: …` when they apply, so one verdict can carry several lines (`src/skills.rs:474-491`). The ahead hint is `factory upgrade` only for a release install; from source it is the `cargo install` line (`src/cli/upgrade.rs:40-47`). `factory skills update` refuses while any skill is hand-edited and prints the `git checkout` per file (`src/cli/skills.rs:55-65`); on success it prints the commit line to run (`:91-96`). Preflight lists the same lines as judge, never a refusal (`src/docs/preflight.rs:113-116`). The owner answers the same way every time.

**Decision (2026-10-30).** In `/prd`, `/slice`, `/deliver` and `/lanes`, step 0 acts on the skills line without asking:

- **Any `hand-edited:` line** → ask first, one question: restore the files and update (Recommended), or keep the edit and skip the update. It wins over every other line, because `factory skills update` refuses while it stands. A hand edit is the one case where the owner's answer varies.
- **ahead** (the binary is older than the repo's skills) → run the command after the `→`, then doctor again.
- **behind / differs / missing**, no hand edit → `factory skills update`, then the commit line it prints, on the current branch.
- **source checkout** (this repo) → nothing.

**Accepted cost:** a chore commit rides in a feature PR. The owner wants exactly that (2026-10-30).

**Acceptance:** in a fixture repo whose skills lock is one version behind the binary, `/prd`'s step 0 runs `factory skills update` and commits `chore:` with no question. A hand-edited skill still gets a question.

## 7. FR-5 — the cockpit waits 55 minutes and survives a compaction

**Today.** `/deliver` §2 waits with `factory wait <id> --timeout 1500` in a background Bash and says one Bash call sleeping longer "was not verified" (`skills/deliver/SKILL.md:17`). §3's title says "as `/deliver` §8", a section that does not exist (`:33`). §4 restarts a dead cockpit from `factory wait <id>` (`:44`). `factory wait` already returns on every cockpit event, the ones the owner must see; progress shows only in `factory status` (`src/cli/wait.rs:54-57`, `src/cockpit.rs:13-14`).

**Decision (2026-10-30).**

- **Poll:** `factory wait <id> --timeout 3300` in a background Bash whose own `timeout` is above 3,300,000 ms. A background Bash runs up to 2 h when its `timeout` asks for it (Claude Code docs, 2026-10-30). The session wakes on any cockpit event, or every 55 minutes, inside the 1-hour prompt-cache life.
- **Re-entry:** after a compaction or a lost context, `/deliver` resumes at §4. `factory status` lists live runs when the id is gone.
- **The owner may clear any time:** `/clear` then `/deliver NNN` picks up the live run, because `factory run` reports a live run and §1 drives that id (`skills/deliver/SKILL.md:13`). A new line in §1 tells the owner so.
- Fix §3's title.

**Accepted cost:** a cockpit that starts fat. Clean cockpits cost 0.34–0.46M weighted tokens per run (runs 004, 021, 022, 028). The fat one is estimated at +0.2–0.5M: about 5% of a 9.8M run, but up to 70% of a 0.7M one. `factory report <run> --cockpit <session>` measures it.

**Acceptance:** the cockpit's `wait` calls carry `--timeout 3300`, and a run of 60+ minutes shows a call that waited past 30 minutes without the Bash being killed.

## 8. FR-6 — Blast radius is optional

**Today.** The scaffold always writes `## 2. Blast radius` (`assets/docs/prd.scaffold.md:19`), and `/prd` §6 describes it as a section to fill (`skills/prd/SKILL.md:131`). Many internal PRDs have nothing outside their FRs that breaks.

**Decision (2026-10-30).** The scaffold's §2 placeholder says to delete the section and renumber when nothing outside the FRs can break. `/prd` §2 and §6 say the same. Why now stays: it is where the owner's real bugs are separated from the improvements, and the extras gate reads it.

**Accepted cost:** none in code. Nothing parses §2 by name (`tests/loop.rs:260` checks a fixture pack does not carry it).

**Acceptance:** `factory docs new x` writes a §2 whose placeholder says it may be deleted, and `factory docs check` is clean on a PRD with no Blast radius section.

## 9. FR-7 — `author` comes from git

From idea 2026-10-15 "Fill a PRD's author from git", cut from PRD-008 FR-9 when the repo had one author. Folded in here because this PRD already edits the scaffold.

**Today.** `factory docs new` writes `author: _(name)_` and never fills it (`assets/docs/prd.scaffold.md:8`, rendered at `src/docs/scaffold.rs:296-301`).

**Decision (2026-10-30).** Fill it from `git config user.name`. Keep `_(name)_` when that is empty or git fails.

**Accepted cost:** one git call per `docs new`.

**Acceptance:** a fixture with `user.name = "Ada"` makes `factory docs new x` write `author: Ada`. A fixture with no `user.name` keeps `author: _(name)_`.

## 10. Decisions log

- ~~Same session or clean session for the cockpit?~~ **2026-10-30: same session, fat cockpit, 55-minute poll.** The owner may still `/clear` and `/deliver NNN`.
- ~~Show the slice plan for approval, or report it?~~ **2026-10-30: report it.** Questions stay only where `/slice` would stop today.
- ~~What does `/deliver` do with a refusal?~~ **2026-10-30: fix the mechanical ones, relay the rest.**
- ~~Stale tooling: ask or act?~~ **2026-10-30: act on skills and binary, ask on a hand edit.**
- ~~Raise the autocompact window?~~ **2026-10-30: 400k**, set in the owner's `~/.claude/settings.json` outside this repo.

## 11. QA plan

**Gate**: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` *(declared in project.toml)*
**Surface**: the skills as a Claude Code session runs them, and the `factory` CLI.
**Harness**: gate-only for FR-2's refusal, FR-6 and FR-7 (CLI tests). The skill text in FR-1, FR-3, FR-4 and FR-5 is proven on the owner's next feature.
**Seed state**: CLI fixtures under `tests/`; never this repo's live `docs/`.

| FR | Proof on the real surface | Evidence | Human? |
|---|---|---|---|
| FR-1 | The owner agrees the next PRD and gets one Build / Slice / Stop question on the phone; Build starts the run | the session transcript | yes |
| FR-2 | run `factory docs preflight` on a fixture feature with no slices: exits 1, `not sliced`; on the next feature `/deliver` writes the slices with no "Write these?" | test transcript, session transcript | no |
| FR-3 | run `/deliver NNN` with the feature's own docs uncommitted: it commits them and the run starts | session transcript | no |
| FR-4 | run `/prd` in a repo whose skills are behind: `factory skills update` and a `chore:` commit, no question | `git log` of that repo | no |
| FR-5 | on the first fat-cockpit run of 60+ minutes, read the session jsonl: one `wait --timeout 3300` call returned after 30+ minutes, not killed. Run `factory report <run> --cockpit <session>` and record the cockpit tokens against the 0.34–0.46M clean baseline (no pass bound: a measurement) | session jsonl timestamps, report output | no |
| FR-6 | run `factory docs new x` in a fixture: §2 placeholder says it may go; `docs check` clean without it | test transcript | no |
| FR-7 | run `factory docs new x` in a fixture with `user.name = "Ada"`: `author: Ada`; with none: `_(name)_` | test transcript | no |

## 12. Slices

_(Not sliced yet. Run `/slice`.)_

## 13. Non-goals

- **Triggering `/compact` or `/clear` from a skill.** No model, skill or hook can run either (Claude Code docs, 2026-10-30). The owner can, any time.
- **A subagent cockpit.** Subagents cannot use `AskUserQuestion`, so the phone would never buzz.
- **Spawning a fresh terminal session for the cockpit.** Out of scope; the owner's `/clear` covers it.
- **The autocompact window.** User config, set to 400k on 2026-10-30. `DISABLE_COMPACT` was rejected: a run longer than the window would end the cockpit.
- **Dropping Why now.** It stays (FR-6).
- **Unattended `/gate` or `/lanes`.** They are interviews; FR-3 relays them.

## 14. Verification notes

Checked against `feat/one-tap-build` at `4995817`, 2026-10-30. **Re-verify at slice entry.**

- `skills/prd/SKILL.md:165` — §8's `/slice` → `/clear` → `/deliver` hand-off.
- `skills/slice/SKILL.md:13` — "Slicing is attended work."; `:37` still says `/factory-deliver`; `:87` unsourced-task question; `:107` "Write these? [Y/n]"; `:114` "On approval".
- `skills/slice/SKILL.md:23` — "Not on the framework, behind, or drifted" routes to `/prd`.
- `skills/deliver/SKILL.md:12` — §1 runs `factory run NNN` directly; `:17` the 25-minute poll; `:33` "as `/deliver` §8", a section that does not exist; `:44` dead-cockpit re-entry.
- `src/docs/preflight.rs:113-116` — skills verdict is a judge line. `:255-345` — no check on slice count.
- `src/docs/preflight.rs:159-175` — a dirty tree refuses unless a slice is building.
- `src/docs/preflight.rs:264-268` — a `draft` PRD passes preflight. *Corrected 2026-10-30:* an earlier draft said it then becomes ready. Nothing writes the status; the line is only text, and `Docs::set_status` (`src/docs/mod.rs:113`) has no callers. `/prd` §7 sets `ready` itself, so FR-1 does not lean on it.
- `src/docs/board.rs:307` — `all_done` needs a non-empty slice list; `src/scheduler.rs:477` — `finish(run, out, "nothing ready")`.
- `src/cli/run.rs:93-99` refusal exit 1; `:106-127` asks become `ASK-n` and `WaitingOwner`; `:131`, `:289` `ensure_branch` makes the feat branch.
- `src/skills.rs:474-491` — the version line, then `hand-edited:` and `missing:`, can print together. `src/cli/upgrade.rs:40-47` — the ahead hint depends on the install. `src/cli/skills.rs:55-65` — update refuses on a hand edit; `:91-96` prints the commit line.
- `src/cli/wait.rs:54-57`, `src/cockpit.rs:13-14` — `wait` returns on cockpit events only.
- `src/cli/docs.rs:638-644` — `factory docs status` already prints `not sliced`; preflight does not.
- `assets/docs/prd.scaffold.md:8` — `author: _(name)_`; `:19` — `## 2. Blast radius`.
- `src/docs/scaffold.rs:296-301` — scaffold render; `:492` — the seven-key test.
- Cockpit cost: runs 004, 021, 022, 028 at 0.34–0.46M weighted; run lengths 60–184 min, 0.7–9.8M weighted (`factory report`, 2026-10-30).
