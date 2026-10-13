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
- **archived** — every slice done *and* nothing left to do; folder moved to `archive/`. A feature with nothing outstanding goes here directly, skipping `shipped`.
- **retired** — never built, number burned. A tombstone folder in `archive/` with `status = "retired"` says why.

A slice is `pending → building → done`, or `blocked` while it waits on the owner.

**Build order:** _(state it here when more than one feature is live, and say why — shared files, migration risk, whichever ships second eats the merge.)_

**Retired numbers — never reused:** _(none yet)_

## Reference (not feature work)

_(none yet)_

---

## Feature lifecycle (canonical — CLAUDE.md & AGENTS.md point here)

**Numbers are two independent axes. Do not conflate:**
- **PRD number** (`{NNN}`) = identity. Monotonic, never reused. Next free number wins, regardless of ship order. `factory docs new` allocates it.
- **Priority / order** = the status + explicit notes here. A later number can ship first. The number does not imply the queue.

**FR ids are feature-scoped:** `FR-1`, `FR-2`, … within a PRD; across features the reference is `PRD-NNN FR-n`. A cut FR keeps its section, titled `FR-n — RETIRED (date)`.

### Create a feature
1. If it's a loose idea, put it in `ideas/` first.
2. When it earns real work: `/prd` → `factory docs new <slug> --title "…"` → `features/{NNN}-{slug}/prd.md` and `state.toml` (`draft`).
3. Agreed? `factory docs set NNN status=ready`.
4. Big PRD? `/prd-to-tasks` → `factory docs slice NNN <subject> [--depends a,b]` per slice. Small PRD? No slices — build straight from `prd.md`.
5. Slice numbering is **per-feature and local** (`slice-01`…).

### While building
- A slice's status lives in `state.toml`: `factory docs set NNN slices.N.status=building`, then `done`.
- `factory docs status` is the board; `factory docs status NNN` is one feature's state.

### When done
Set the terminal status **in the last commit of the feature's PR**, never after the merge. The docs ride the same branch as the code, so the main branch flips at the exact moment the merge lands.
- Something survives the merge (a post-deploy measurement, a QA guide still to run, a follow-up FR) ⇒ **shipped**.
- Nothing left at all ⇒ folder moved to `archive/`, status **archived**. Do not edit files after archiving.
- Both refuse while any slice is not `done`.

### Rules that keep this working for agents
- **Archived docs are frozen.** Move folders, never rename or relink files inside `archive/`. History is a record, not a live surface.
- **One board.** `factory docs status`. Don't spawn a second status file.
- **Slice, don't fatten.** Never grow a single doc to the size where an agent that reads it whole loses its context budget. Split into slices.
- **Ideas live in `ideas/`**, not scattered TODO files.
