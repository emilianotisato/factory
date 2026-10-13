---
name: slice
description: Slice a PRD into vertical, independently shippable slice-NN-*.md files with recon, per-task acceptance criteria and dependencies, optimized for AI agent execution
argument-hint: [feature-number-or-path]
---

# PRD to Slices

Turn a PRD's *what/why* into the *how*: a small number of **vertical slices**, each one a commit's worth of work that leaves the product working.

Output: `docs/features/{NNN}-{slug}/slice-NN-{subject}.md`, one file per slice, plus the prose in `prd.md` § Slices. `factory docs status` computes the board from the files and `state.toml`; nothing here is a table to maintain by hand.

This is not a task-tracker export. The slice files are the durable artifact; they outlive any session's todo list. **Slicing is attended work.** It happens before a `factory` run starts, with the owner in the room; the run builds what you wrote here and never re-slices.

## 0. Preflight

**The bar.** Slicing is judgment work — the ordering argument, the dependency reasoning, and the recon that keeps a slice executable. It does its best version on a top-tier reasoning model. **Under the bar → one line, then continue**; at or above it, say nothing. See [_shared/models.md](../_shared/models.md).

```
factory docs doctor
```

**Not on the framework, behind, or drifted** → `/prd` first: its preflight sets the project up, and without a PRD on the framework there is nothing to slice. Never write slices into whatever ad-hoc task file the project already has.

**Current** → read `docs/index.md` (build order, what else is live, which files are contended), then the target PRD. Resolve the target from `$ARGUMENTS`: a number (`013`), a slug, or a path. No argument → `factory docs status` lists the features; ask which of the `ready` / `in-progress` ones.

**Then the gate:**

```
factory docs gate
```

It prints the project's verify command, or exits 1 with candidates. **No gate → do not write slices.** A Verify task holding a placeholder is a slice that gets declared done on vibes. Say so in one line and offer, with rough cost: `/gate` (an interview that builds the gate and its tiers — the right answer for a repo that has none) or `factory docs gate set "<cmd>"` when the command already exists and merely is not declared. Then continue.

## 1. Decide whether to slice at all

**Small PRD: do not slice.** One or two FRs, one area of the code, a day's work — building straight from `prd.md` is correct. Say so, write "No slices — build straight from this document" into the PRD's § Slices, and stop. **Unless a `factory` run will build it:** the run records Progress on slice files, resumes from them and blocks on them, so a small PRD headed for `/factory-deliver` gets exactly one slice.

**Do not slice a PRD with load-bearing OPENs.** If the decisions log still holds unsettled questions that change the shape of the work, say which ones block and send the user back to `/prd`. A slice written against a guess is a rewrite.

**Do not slice a PRD with no QA plan.** Every FR needs a proof row before it can become a Prove task. Missing → back to `/prd`; `factory docs check` is already warning about it (`W-NO-QA-PLAN`). If the plan says the harness is missing, **the harness slice goes first**, whatever the ordering argument says about the rest. Its one task is `/harness`; its acceptance is the generated skill's own proof run — launch, doctor, one drive, evidence surviving cleanup.

Slice when: several FRs, more than one layer (schema / server / client), or a diff big enough that one review and one CI run would swallow it.

## 2. Design the slices

**A slice is vertical, not a layer.** "Migrations" is not a slice; "status spine: scopes, migrations and every server-side read that depends on them" is.

Rules:

- **Each slice leaves the product working.** A slice that lands a migration nothing reads yet is fine only if the app behaves identically to before.
- **Size by one builder's sitting, not by task count.** A slice is what one builder finishes cold in one go: read the spec and the code it cites, build, run the inner loop, write Progress. There is no size cap to obey. The observatory records every build's pack and peak context, and the retro says when a slice was too big; for scale, the first 32 slices of this family (2026-10) ran 7 to 26 minutes at about 150k peak context, and every one finished. Count generated call sites before you judge: a route, schema or contract change touches every generated client (a slice estimated at 45 files landed at 121).
- **Dependencies are explicit and minimal.** Say *why* B depends on A ("same files — land the one-liner first so this diff is about failure handling only"). Slices with no dependency between them should say they are independent.
- **Highest-risk work gets its own slice**, and the slice says so at the top so whoever picks it up is warned.
- **Order for what it buys.** Cheapest bug-fixing slice first if it unblocks the owner; the prerequisite first if everything else needs it. State the argument.
- **Some slices legitimately end without code.** A measurement slice can close with "measured, nothing to fix" — that is a result, not a failure. Plan it that way if the PRD says the cause is unknown.
- **Every slice ends in two tasks.** **Verify** runs the inner loop — `factory docs gate --inner` prints it (the full gate when none is declared) and `factory docs slice` writes it into the scaffold. **Prove** names this slice's rows from the PRD's § QA plan; they are driven at the **checkpoint** that covers the slice, together with the full gate, and the evidence goes in the slice's Progress. A row with no UI to drive it (a scheduler, a worker, an API contract) is proven **below the UI**: its Proof cell starts with `run` and names the test or command, and the checkpoint runs it and keeps the transcript instead of launching a browser. A slice with nothing to prove (a migration nothing reads yet, a measurement) names no rows and says so. Never invent a gate command: if `factory docs gate` has none, stop and go back to §0.
- **Checkpoints fall every `checkpoint_every` done slices** (`docs/project.toml`, default 2) and once at the end. Put the slices that later slices build on — a schema, a contract, a resolver — where a checkpoint lands right after them, so nothing is built on an unproven assumption for long.

Number per-feature and local: `slice-01`, `slice-02`, …

## 3. Recon each slice before writing it

Read the code the slice will touch. Every slice file carries a **Current code / environment** section of `file:line` facts — the things that decide the implementation and the things that will bite:

- what the current shape actually is, quoted precisely
- the constraint that is not obvious (a cast that means no migration is needed; a queue with `minReplicas: 0` that makes "slow" a real state)
- the deliberate non-change, with the PRD section that decided it (*"prd §7 OPEN-7 decided against phase reporting. Do not add phases here."*). Every `Not handled:` non-goal the slice's code path touches is quoted here, so the builder does not rebuild what the owner declined.
- test files that already assert the behaviour you are about to change
- the fixture or seed an acceptance will lean on, and whether it holds the case the FR is about. One run's only transcript fixture had no forked subagent, so the fork double count passed every slice and every checkpoint; the fixture that would have caught it was one line to ask for here.

This is what lets an agent execute the slice without reading the whole PRD.

## 4. Write the slice files

`factory docs slice` writes the scaffold (§5); you fill it. Each task inside a slice carries:

**`### N. {Title} — {priority} — blocked by: {n, m | —}`**

- **Where**: the exact file(s).
- **Source**: the PRD words this task builds, quoted with their section: an FR's text, its acceptance, a decision. A prerequisite quotes the FR it unblocks.
- **What and how**: the approach, the pattern to follow, the decision already made. Include the table/schema/signature if the PRD has one.
- **The trap**, when there is one: a fact of today's code, cited `file:line`, that makes the obvious approach wrong. *"`markFailed()` never clears the failure keys (`{file}:{line}`), so clear them on a fresh attempt or a retry looks failed forever."* Never a what-if that needs new machinery. That is an extra the PRD did not ask for.
- **`**Acceptance:**`** — a test that could fail. Name what it asserts, not "it works".

**Every task traces to the PRD.** A task you cannot quote a source for is not written. This is where a slice quietly adds requirements that every later stage then treats as the spec: a figure nobody asked for, a guard for a case the PRD never names, a mechanism the PRD orders for one path copied to another path where its reason does not apply. Verify, Prove and the harness slice's task cite § QA plan. Ask the owner about an unsourced task with "Leave it out" as the recommended option; one they keep goes into the PRD as a decision first, so the slice has something to quote.

Priorities: `high` for anything the rest depends on or that carries the risk, `medium` for the rest, `low` for polish. Most tasks in a well-shaped slice are `high`; if most are `low`, the slice probably should not exist yet. Blocked-by refers to task numbers **within the slice**; cross-slice dependencies live in `state.toml`, written by `factory docs slice --depends`, and their *reason* in the header's **Depends on** line.

## 5. Show the plan, then write

Present the slice board before creating files:

```
PRD-013 → 5 slices

01 — Nav, toast & the wizard hang     FR-97, FR-101a, FR-102   deps: —
02 — Wizard failure path              FR-101b                  deps: 01 (same files)
03 — Content table                    FR-99                    deps: —
04 — Tooltips                         FR-103                   deps: —
05 — Performance                      FR-100                   deps: merge → deploy → measure

01 first: clears both bugs the owner hit, touches nothing PRD-012 rewrites.
02/03/04 are mutually independent. 05 cannot be measured until 01-04 are deployed.

Write these? [Y/n]
```

**The board also shows the build shape.** Mark where checkpoints fall (`▲` after 02, 04, 05 with the default of 2). Name the **spine** — what everything else depends on, built first and alone — and the **branches** off it. **Every dependency you declare costs parallelism**, so declare the ones that are real and say plainly which slices are independent: `factory docs preflight` turns that graph into how many builders can run at once, and a dependency added "to be safe" is a builder that sits idle for hours.

Whether a run actually uses that width is not a slicing decision — it is `lanes` in `docs/project.toml`, which says how many isolated environments (database, ports, scratch paths) the project has. Do not ask about it here. The harness does not constrain this: builders never drive it, and the checkpoint that does runs alone.

On approval, create each slice:

```
factory docs slice 013 wizard-failure-path --depends 1 --title "Wizard failure path"
```

That allocates the number, records the slice and its dependencies in `state.toml`, writes the scaffold, and puts the inner loop into the Verify task. **What the scaffold leaves to you:**

- the **Source** line: the PRD section and FRs, and the *why* after **Depends on**
- the **Slice** paragraph, and the **highest-risk** line when one task carries the risk
- **Current code / environment**, from §3
- the tasks, from §4; Verify stays as written
- the **Prove** task's rows, or delete the task and say why in its place

**The file holds no status and no frontmatter.** `state.toml` carries the slice's status and dependencies, written through `factory docs slice` and `factory docs set`; a status written into the file is a second board that nothing reads. Progress, review fixes and deviations are added by whoever builds it, not by you.

## 6. Register

1. **`prd.md` § Slices** — prose, yours: the scope of each slice, the ordering argument, the checkpoints, and the workflow line (one branch, one PR, one commit per slice, Prove at the checkpoints — or the deviation). No table: `factory docs status` is the table.
2. **`qa-guide.md`** in the feature folder, **generated only from the QA plan's Human rows** — feel, mobile, copy, anything a machine cannot judge. Budget it in minutes and mark which checks matter most. No row says Human → no guide; say so in the PRD's § Slices prose so nobody looks for one.
3. **`factory docs check`**, and fix what it reports.
4. Commit: `docs(NNN): slice PRD-NNN`, with the board from §5 in the body so the plan is in the history.

## 7. While building

Inside a `factory` run the builder's contract covers this. Attended, it is the builder's checklist:

- Move the slice's status as it goes: `factory docs set NNN slices.N.status=building`, then `done` — or `blocked`, with a `BLOCK` entry in the log saying why. The feature itself: `factory docs set NNN status=in-progress` when the first slice starts. The board follows, which is the whole point: two agents in two lanes each touch only their own slice.
- Write the **Progress** paragraph when it lands: what was built, the gate numbers, what a review caught.
- **`done` means built, inner loop green, committed.** Its Prove rows are driven at the checkpoint that covers it, and the evidence (a screenshot, a transcript, a read-back of the stored value) lands in its Progress then. "Tests pass" is the Verify task, not the Prove task. When you drive a row by hand right after building, it still counts, as long as the evidence goes in Progress.
- Record **deliberate deviations** in the slice, with the reason. A slice that specified a 403 and shipped a 404 because route-model binding hides the row is a *better* outcome that must be written down, not a silent mismatch. A question the slice and the PRD left open is a decision in the log: `factory docs log add NNN --kind decision --slice N`.
- In-session tracking with TaskCreate/TaskUpdate is fine for the slice you are actively building. It is a convenience, not the record — the slice file is the record.

## 8. Re-running on an updated PRD

1. Read the existing slice files and `state.toml`.
2. **Never rewrite a slice marked `done`.** It is a record of what was built. New work is a new slice.
3. Diff the PRD against the pending slices; show added / changed / no-longer-needed.
4. Confirm, then update only the pending ones and the § Slices prose.

## 9. Shipping

**Every slice `done` and the checkpoints proved → `/polish`.** It owns the review, the fixes, the test cleanup and the closeout: `factory docs ship` or `archive`, inside the last commit of the PR. Nothing here duplicates it; `factory docs check` nudges with `W-NOT-REVIEWED`, then `W-READY-TO-SHIP`, until it has run.
