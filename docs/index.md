# Docs Index — Product Map & Lifecycle

**This file is the only entry point.** Start here. Everything else is either a live feature folder, an idea, or an archived record. The board is not kept here: `factory docs status` computes it from the folders.

> **Layout:**
> - `project.toml` — the project's knobs under `[docs]`: the framework version, the verify gate and its inner loop, lanes, checkpoint cadence, size ceilings.
> - `features/{NNN}-{slug}/` — live product work. `prd.md` = the *what/why* (durable), its frontmatter carries the title and the `summary`. `state.toml` = the feature's status and each slice's status, lane and dependencies, written only by `factory docs`. `slice-NN-*.md` = the *how*, one vertical slice each (disposable after ship). Small feature = `prd.md` only, no slices.
> - `features/{NNN}-{slug}/` log and `retro.md` — written by an unattended run: what it decided alone, where it stopped, and its report on the process. Reviewed by the owner in the PR.
> - `ideas/` — the inbox for undocumented ideas. An idea graduates to a `features/` folder when it earns a PRD.
> - `reference/` — ops, estimates, guides. Not feature work.
> - `archive/` — frozen history. **Never edited, never relinked.** Read-only record of what was.

## Status vocabulary

`idea → draft → ready → in-progress → shipped → archived`

- **idea** — an entry in `ideas/`, no PRD yet.
- **draft** — PRD being written, not agreed.
- **ready** — PRD agreed, not started (may not be sliced yet).
- **in-progress** — at least one slice building.
- **shipped** — every slice done and this branch completes the feature. Set in the last commit of its PR, never in a follow-up.
- **archived** — folder moved to `archive/`, holding the records only: `prd.md` with its history banner, `state.toml`, `log.jsonl` and `retro.md`. `/harvest` routes the lessons first, then `factory docs harvest NNN --done` (or `factory docs archive NNN`) leaves this shape.
- **retired** — never built, number burned. A tombstone folder in `archive/` with `status = "retired"` says why.

A slice is `pending → building → done`, or `blocked` while it waits on the owner.

**Build order:** 009 is the only live feature; 001 to 008 are archived. When more than one feature is live, state the order here and say why: shared files, migration risk, whichever ships second eats the merge.

**Retired numbers — never reused:** _(none yet)_

## Reference (not feature work)

- [adopt-a-repo.md](reference/adopt-a-repo.md) — the prompt that moves another repo onto factory v3.
- [skills-decisions.md](reference/skills-decisions.md) — the skill family's decisions from 2026-08 to 2026-10, imported from the old `feature-docs` repo. History only: new ones go to `decisions.md`.
- [lanes/](reference/lanes/) — `lane-1.md` … `lane-4.md`, the env and setup the driver gives each lane.
- [ctx-research.md](reference/ctx-research.md) — the 2026-10-09 recon behind `factory ctx`: transcript mining, a typed lookup layer weighed and left out.
- [ctx-jev-research.md](reference/ctx-jev-research.md) — Jev System One weighed as a `ctx` picker, with the replays on real lookups. Parked.
- [ab/](reference/ab/) — the A/B method: `runs.md` (setup, timeline, blind review of the icg-cloud and arca-api runs) and `worktrees.sh`, which cuts the two worktrees.
- [ctx-replay-research.md](reference/ctx-replay-research.md) — 2026-10-15 replay of worker lookups over 14 runs: what `ctx` would have answered against grep, sed and cat, and why forcing it loses.
- [builder-cost-research.md](reference/builder-cost-research.md) — where a builder's start context and turns go, over 91 builders, and the levers ranked by measured saving.
- [leaks-research.md](reference/leaks-research.md) — code comments and agents: the studies, vendor guidance and field counts behind the leaks rules.

---

## Feature lifecycle (canonical — CLAUDE.md & AGENTS.md point here)

**Numbers are two independent axes. Do not conflate:**
- **PRD number** (`{NNN}`) = identity. Monotonic, never reused. Next free number wins, regardless of ship order. `factory docs new` allocates it.
- **Priority / order** = the status + explicit notes here. A later number can ship first. The number does not imply the queue.

**FR ids are feature-scoped:** `FR-1`, `FR-2`, … within a PRD; across features the reference is `PRD-NNN FR-n`. A cut FR keeps its section, titled `FR-n — RETIRED (date)`. PRDs written before v3 (PRD-001 … PRD-003) keep their global FR numbers.

### Create a feature
1. If it's a loose idea, put it in `ideas/` first.
2. When it earns real work: `/prd` → `factory docs new <slug> --title "…"` → `features/{NNN}-{slug}/prd.md` and `state.toml` (`draft`).
3. Agreed? `factory docs set NNN status=ready`.
4. Big PRD? `/slice` → `factory docs slice NNN <subject> [--depends a,b]` per slice. Small PRD? No slices — build straight from `prd.md`.
5. Slice numbering is **per-feature and local** (`slice-01`…).

### While building
- A slice's status lives in `state.toml`: `factory docs set NNN slices.N.status=building`, then `done`.
- `factory docs status` is the board; `factory docs status NNN` is one feature's state.

### When done
Set the terminal status **in the last commit of the feature's PR**, never after the merge. The docs ride the same branch as the code, so the main branch flips at the exact moment the merge lands.
- Something survives the merge (a post-deploy measurement, a QA guide still to run, a follow-up FR) ⇒ **shipped**.
- The closeout ships, never archives. Archiving comes later, from `/harvest`. Do not edit files after archiving.
- Both refuse while any slice is not `done`.

### Rules that keep this working for agents
- **Archived docs are frozen.** Move folders, never rename or relink files inside `archive/`. History is a record, not a live surface.
- **One board.** `factory docs status`. Don't spawn a second status file.
- **Slice, don't fatten.** Never grow a single doc to the size where an agent that reads it whole loses its context budget. Split into slices.
- **Ideas live in `ideas/`**, not scattered TODO files.
