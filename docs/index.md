---
framework: 2
gate: cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
inner: cargo test
lanes: 4
---

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

| # | Feature | Status | PRD | FR refs | Notes |
|---|---|---|---|---|---|
| 001 | [Driver: the unattended deliver loop](features/001-driver/prd.md) | **shipped** | created 2026-10-09 | FR-1 … FR-12 | **Shipped 2026-10-10**, pending owner review: 13 slices, 7 checkpoints, polish by a local review, a security pass and Cursor `grok-4.7-high` (about 100 findings, fixed by 5 fixers; 3 parked in `ideas.md`). 145 decisions, 31 flagged `decide` and 47 `fyi`. PR from `feat/driver`. Human QA pending (FR-7, FR-11). Rust `factory` binary replaces the `/deliver` session. Detached driver, `claude -p` workers with context packs, lanes that park on a block and keep going, gates/commits/log merge/CI/PR run by the driver at zero tokens, thin Claude Code cockpit for blocks and the closing review (phone), SQLite metrics, quota pause plus per-run budget. Calls `docs.mjs` as a subprocess; the port is PRD-002. Proof: stub harness in `cargo test`, then A/B against `/deliver` on icg-cloud and arca-api (tokens −40%, owner questionnaire). FR-12 `factory ctx` (`def`/`refs`/`outline`, ripgrep crates + tree-sitter tags, no LSP) after a 27-agent research pass over 55 owner sessions: lookups are 56–74% of worker calls and 92% of result bytes. Cut 2026-10-09: closeout worker folded into polish, gate cache keyed on code paths only. All 16 OPENs closed. Jev (TypeSafe) assessed 2026-10-09: not now, parked in `ideas.md`, memo `ctx-jev-research.md`; three Jev-free grafts kept from it (`followup_reads`, pack-doc hint, `def` body default), the seen-before stub cut. Gate declared; a bare crate skeleton was committed 2026-10-09 so preflight resolves it (slice 01 fills it in). Sliced 2026-10-09 into 13 slices (§17): spine 01 harness → 02 contract/state → 03 one worker (highest risk) → 04 lanes → 05 commits/log → 06 checkpoints → 07 quota → 08 metrics → 09 closeout/PR/CI → 10 cockpit; branch 11 ctx search → 12 ctx tags off 01; 13 ctx-in-the-loop joins both. Checkpoints every 2. FR-11 A/B is a post-ship owner measurement in `qa-guide.md` (with FR-7), no slice. One fact corrected at slicing: the worker guard hook is new (`factory hook` via `--settings`), not existing. |
| 002 | [Docs port: docs.mjs into factory, v3 format](features/002-docs-port/prd.md) | **draft** | created 2026-10-11 | FR-13 … FR-19 | **Agreed 2026-10-11** (seven interview forks; Cursor `grok-4.7-high` adversarial pass, 22 findings corrected inline; the cut kept all eight extras). Ports `docs.mjs` into `factory docs` and moves the format to v3: `docs/project.toml` instead of index frontmatter; one `state.toml` per feature, written only by `factory docs set` under a per-feature lock; an append-only `log.jsonl` per feature with ids allocated at append, `log add | show`, no fragments and no `log merge`; a frontmatter-only PRD header whose `summary` replaces this Notes cell; feature-scoped FR ids (pre-v3 PRDs keep their numbers); retro numbers in a `retros` table of the metrics store (schema v3 once 003 ships first, see Build order). Verbs kept: `hook`, `lanes`, `renumber`; cut: `vendor`, `render`, `log merge`, `check --fix`. `factory docs migrate --dry` prints the plan, undo is git, the archive is grandfathered. Skills and the global hook call `factory docs`, which routes v2 repos to `docs.mjs`. The driver reads v3 only, in process; the stub harness moves to a v3 fixture built by the binary. **Hard dependency: the FR-11 A/B is running; no build touches `docs.mjs`, the skills repo or the owner repos until it ends.** This repo migrates by hand after the merge, owner repos on the owner's go. Not sliced. |
| 003 | [Observatory: measure, experiment and correct factory runs](features/003-observatory/prd.md) | **shipped** | created 2026-10-13 | FR-21 … FR-25 | **Agreed 2026-10-13** (three interview rounds, eight forks; Cursor `grok-4.7-high` adversarial pass, 25 findings, 20 corrected inline; the cut kept all nine extras). The eyes for the full switch decided 2026-10-12: no more A/B, experiments are knob changes measured by the store. FR-21 counts the subagents a worker spawns (read from the Claude Code transcript dir after exit; `report` under-counted both A/B rounds by 2.3M and 2.1M), `in_subagent` on `tool_calls`, `report --recount` to fix stored runs, token columns on `--per-worker`. FR-22 `pack_parts`: chars per pack section, printed per role beside `start_context`. FR-23 project scope `docs/project.toml` `[workers.<role>]` and `[run]`, `runs.knobs` with each value's source, `--hypothesis`, `model_id` from the stream. FR-24 quality at zero tokens: a `FINDINGS high= medium= low= fixed= parked=` line in the polish return, `fix_commits`, `ci_red_rounds`, `ci_reruns`, decision counts; no composite score. FR-25 `factory retro <run>` writes a `## Numbers` section into `retro.md` between markers (per-role own/subagent, knobs, quality, pack composition, comparison with the repo's last three runs per slice and per changed line); retires `measure.py`/`weigh.py`. Schema v1 → v2 (added columns only). Proof: gate + stub with a subagent recording, then a replay on the stored arca (18,465,008) and icg (8,131,594) runs. Not handled: learn loop, `factory grade`, owner signals, post-merge churn, stream fallback, per-turn series. **Sliced 2026-10-13, six slices:** 01 subagent counts (spine: schema v2, split collector, transcript reader, exit record, totals, harness scenario) → 02 recount + per-worker tokens (depends 01; replay on the stored runs) ▲ · 03 pack composition, 04 knobs, 05 quality signals (independent of each other and of 01/02) ▲ · 06 retro numbers (depends 01-05; `factory retro`, changed lines, retires `measure.py` step) ▲. QA guide: FR-25 Human row + the ten-minute replay. |

**Next free PRD number: 004.** **Next free FR: FR-26.**

**Build order:** **003 before 002.** Both touch the schema in `src/db.rs`, the ship step in `src/closeout.rs` and `docs/project.toml`. 003 is small and the next project run needs the counts; 002 eats the merge (its `retros` table lands at schema v3, its `## Numbers` table is superseded by FR-25's section, its `project.toml` must keep the `[workers]` and `[run]` tables). See 003 §11.

**Retired numbers — never reused:** _(none yet)_

## Archived

Shipped or resolved features, folder moved to `archive/`. **Frozen** — read for history, never edit or relink.

| What | Where | FR refs | Note |
|---|---|---|---|
| — | | | |

## Reference (not feature work)

_(none yet)_

---

## Feature lifecycle (canonical — CLAUDE.md & AGENTS.md point here)

**Numbers are two independent axes. Do not conflate:**
- **PRD number** (`{NNN}`) = identity. Monotonic, never reused. Next free number wins, regardless of ship order.
- **Priority / order** = the Status column + explicit notes here. A later number can ship first. The number does not imply the queue.

### Create a feature
1. If it's a loose idea, add a one-liner to [`ideas.md`](ideas.md) first.
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
