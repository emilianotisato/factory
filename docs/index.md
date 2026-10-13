# Docs Index — Product Map & Lifecycle

**This file is the only entry point.** Start here. Everything else is either a live feature folder, an idea, or an archived record. If this index and a doc disagree, fix this index.

> **Layout:**
> - `features/{NNN}-{slug}/` — live product work. `prd.md` = the *what/why* (durable). `slice-NN-*.md` = the *how*, one vertical slice each (disposable after ship). Small feature = `prd.md` only, no slices.
> - `features/{NNN}-{slug}/agent-log.md`, `retro.md` — written by an unattended `/deliver` run: what it decided alone, where it stopped, and its report on the process. Reviewed by the owner in the PR.
> - `ideas.md` — one-liner inbox for undocumented ideas. Graduate to a `features/` folder when it earns a PRD.
> - `reference/` — ops, estimates, guides. Not feature work.
> - `archive/` — frozen history. **Never edited, never relinked.** Read-only record of what was.

## Status vocabulary

`idea → draft → ready → in-progress → shipped → archived`

- **idea** — a line in `ideas.md`, no PRD yet.
- **draft** — PRD being written, not agreed.
- **ready** — PRD agreed, not started (may not be sliced yet).
- **in-progress** — at least one slice building.
- **shipped** — every slice done and this branch completes the feature. Set in the last commit of its PR, never in a follow-up.
- **archived** — every slice done *and* nothing left to do; folder moved to `archive/`. A feature with nothing outstanding goes here directly, skipping `shipped`.
- **retired** — never built, number burned. A tombstone folder in `archive/` with `status: retired` says why.

## Live features

**Build order:** **003 shipped 2026-10-13; 002 is next, alone.** 002 ate the merge in text on 2026-10-13 (`retros` at schema v3, no `## Numbers` table of its own, `[docs]` table beside `[run]` and `[workers]`; 002 §11 OPEN-10…12). **PRD-004 (the skill family held in this repo, installed and updated by `factory`) comes after 002**: it redoes 002's one-line skill edits as in-repo files, so it must not start before 002 merges. See `ideas.md`.

**FR numbers:** PRDs written before v3 (PRD-001 … PRD-003) keep their global FR numbers; from v3 on, FR ids are feature-scoped: `FR-1` within a PRD, `PRD-NNN FR-n` across.

**Retired numbers — never reused:** _(none yet)_

## Archived

Shipped or resolved features, folder moved to `archive/`. **Frozen** — read for history, never edit or relink.

## Reference (not feature work)

_(none yet)_

---

## Feature lifecycle (canonical — CLAUDE.md & AGENTS.md point here)

**Numbers are two independent axes. Do not conflate:**
- **PRD number** (`{NNN}`) = identity. Monotonic, never reused. Next free number wins, regardless of ship order.
- **Priority / order** = the Status column + explicit notes here. A later number can ship first. The number does not imply the queue.

### Create a feature
1. If it's a loose idea, add a one-liner to [`ideas.md`](ideas/README.md) first.
2. When it earns real work: `/prd` → `features/{NNN}-{slug}/prd.md` (next free `{NNN}`).
3. Add a row to **Live features** above with status `draft`/`ready`.
4. Big PRD? `/prd-to-tasks` → `slice-01-{subject}.md`, `slice-02-…` in the same folder. Small PRD? No slices — build straight from `prd.md`.
5. Slice numbering is **per-feature and local** (`slice-01`…).

### While building
- Each slice carries its own status line at the top. Update it as it moves.
- Keep the slice board / status here current — **updating this index is part of finishing a slice**, not optional.

### When done
Set the terminal status **in the last commit of the feature's PR**, never after the merge. The docs ride the same branch as the code, so the main branch flips at the exact moment the merge lands.
- Something survives the merge (a post-deploy measurement, a QA guide still to run, a follow-up FR) ⇒ `docs ship <NNN>` → **shipped**.
- Nothing left at all ⇒ `docs archive <NNN>` → folder moved to `archive/`, status **archived**, row moved to **Archived** above. Do not edit files after archiving.
- Both refuse while any slice is not `done`. Without the tool: set `status:` in `prd.md` frontmatter and move the row by hand.

### Rules that keep this working for agents
- **Archived docs are frozen.** Move folders, never rename or relink files inside `archive/`. History is a record, not a live surface.
- **One index.** This file. Don't spawn a second status board.
- **Slice, don't fatten.** Never grow a single doc to the size where an agent that reads it whole loses its context budget. Split into slices.
- **Ideas live in `ideas.md`**, not scattered TODO files.
