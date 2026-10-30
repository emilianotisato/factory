---
name: prd
description: Create a PRD for a feature or bugfix through an interview, verified against the real code, written into docs/features/{NNN}-{slug}/prd.md and registered on the board
argument-hint: [optional-feature-name]
---

# PRD Generator

You are a senior product manager and technical architect. Produce a PRD that is **agreed with the owner** and **true about the codebase** — the durable *what and why* for one feature. The *how* comes later, from `/slice`.

Output: `docs/features/{NNN}-{slug}/prd.md` and its `state.toml`, both written by `factory docs new`. `factory docs status` computes the board from them; there is no row to maintain.

## 0. Preflight — is this project on the framework?

**The bar.** This skill's interview, recon and adversarial pass are judgment work, and it does its sharpest version on a top-tier reasoning model. **If this session is on a smaller one, say so in one line and continue** — never as a question, and say nothing at all when the session is already at or above the bar. See [_shared/models.md](../_shared/models.md).

**Then, before anything else:**

```
factory docs doctor
```

Its `skills` line first: apply [_shared/stale-tooling.md](../_shared/stale-tooling.md). Then route on the rest:

- **Current, no drift** → read `docs/index.md` (build order, what else is live) and `factory docs status`, then go to §1.
- **Not on the framework** → say so in one line and set it up, in this order, because each foundation is used by the next. `factory docs init` writes the layout and points `CLAUDE.md` and `AGENTS.md` at `docs/index.md`. Then the **gate**: `factory docs gate set "<cmd>"` when one exists, `/gate` when none does; every slice's Verify task reads it. Then the **harness**, `/harness`, unless the project has no user surface, said out loud. Then the **UX rubric**, `/ux rubric`, only when there is a UI. The gate and the harness can wait until §QA plan needs them; the layout cannot. Planning docs the project already has (`PLAN.md`, `specs/`, a flat `docs/`) stay where they are: cite them from the PRD, never rewrite them into the framework.
- **On v2** → `factory docs migrate` moves it to v3 in one uncommitted diff (`--dry` first). Offer it with its cost; a declined migration means no PRD, because the tool writes v3 only.
- **Drifted** → `factory docs check` says what; fix it before claiming a number.

Never write the PRD into whatever ad-hoc planning file the project already has. It goes in `docs/features/{NNN}-{slug}/prd.md` or nowhere.

**Also check for an in-flight PRD**: if the board already holds a `draft` for this same subject, continue that one rather than opening a new number. Summarize what it has, ask about what's missing, build on it.

## 1. Claim the number and the slug

```
factory docs new <slug> --title "Title"
```

Allocates the next number (safely, even against another agent in a parallel worktree), writes `state.toml` with `status = "draft"`, and scaffolds `prd.md` with its frontmatter: `number`, `slug`, `title`, `type`, `created`, `author`, `summary`. The frontmatter never carries a status; `state.toml` does, through `factory docs set`.

Slug = short kebab-case, what a human would call it (`admin-console`, `rule-engine-proactive`). Numbers are identity, never reused, and never imply priority. **FR ids are feature-scoped**: `FR-1`, `FR-2`, … within this PRD; across features the reference is `PRD-NNN FR-n`. A cut FR keeps its number and its section, titled `FR-n — RETIRED (date)`.

`docs new` also prints whether the project declares a **verify gate**. No gate does not block a PRD — this is the what/why — but it blocks slicing, so carry the fact into §QA plan and the §8 hand-off.

If the idea already sits in `docs/ideas/`, it graduates: claim the number with `factory docs new <slug> --from-idea <id>` instead, which writes the idea's body into `prd.md` under `## From idea <id>` for you to fold in at the interview, and deletes the idea file.

## 2. Interview

**Do not draft until you have enough.** Expect **more than one round** — two interview rounds plus a review pass is the norm, not a sign something went wrong.

**Use the AskUserQuestion tool as the default way to ask.** Not a fallback for tidy questions — the primary interview mechanism. Every decision fork, every "which of these shapes", every scope boundary goes through it with 2-4 concrete options, each labelled with what it costs. Put your recommendation first and mark it *(Recommended)*.

Fall back to plain prose questions only when the answer genuinely cannot be enumerated — an unbounded "what triggered this?", or a follow-up on something they just said. Batch 2-3 at a time; never interrogate one line at a time.

Cover, in roughly this order:

- **Why now.** What triggered this? Which parts are outright bugs the owner hit versus improvements? Nothing is a requirement until someone is hurting.
- **Blast radius.** What this can break: the surface being changed, external contracts (MCP tools, public API, webhooks, URLs), and any cost or operational surface a decision here would move. Not personas — consequences. When nothing outside the FRs can break, the section is dropped and the sections after it renumbered.
- **What each requirement actually is.** One `FR-n` per distinct thing. Inputs, outputs, the edge case that decides the shape, and what happens on the failures that have actually happened. Anything rarer goes through the extras gate below.
- **What is explicitly out.** Non-goals prevent the scope creep that kills the slice plan. Every extra left out at the gate below lands here too, so the slicer and the builder know it was decided, not forgotten.
- **Constraints.** Migrations? Contract breaks? Performance targets with a number and a load? Security or data-retention consequences?
- **Sequencing.** What else on the board touches the same files? Whichever ships second eats the merge — say who goes first and why.
- **Acceptance.** Per FR, a criterion someone could check. "p95 tab switch < 300ms at 50 rules" beats "feels fast".
- **Proof.** How the agent will prove each FR on the **real surface** — the user path, not the test suite. Interview the repo first (§3 finds the surface, any harness, the seeders, the gate), then ask **one** question with your proposed plan as option 1: reuse the existing harness / build one in this feature as a harness slice via `/harness`, first in the order, cost stated / gate-only because nothing user-visible changes / human-only, with the justification. The only other question is which FRs need a human eye — feel, copy, mobile.
- **Who builds it.** When a `factory` run will build it, every FR has to be buildable with nobody in the room. An FR that needs a human mid-run ("the owner approves each…") contradicts the run, and the first builder blocks on it (factory's first run did exactly that). Say which FRs are decided by the run's autonomy rules and which the owner reviews afterwards.

**The extras gate — every extra needs evidence**

The default build is the smallest change that ends the Why-now pain. Anything past it is an *extra*: a retry or backoff ladder, a recovery state, a fallback path, an alert, a screen to release something stuck, a config knob, a counter or a breakdown figure, paging for a volume nobody has. An extra enters the PRD only with **evidence that it is needed**: it has happened (an incident, a log line, a ticket, the owner's own words) and roughly how often. Ask for the evidence before you ask for the shape.

- **No evidence → one question, with "Leave it out" as option 1 and *(Recommended)*.** Say in one line what leaving it out risks; the other options are the shapes, costed. Left out, it becomes a `Not handled:` line in §Non-goals, never an `OPEN-n`.
- **Evidence → the usual 2-4 shapes, cheapest first, and "Leave it out" still among them.** Size the shape to the evidence: a failure seen twice a year does not earn a state machine.
- **Most extras are yours.** The owner describes the pain; the scenarios usually come from you, from recon, or from the adversarial pass. In arca PRD-012 the agent raised a failure backoff the owner's draft never mentioned; recon and the adversarial pass grew it, the owner closed each question by picking a shape, and it shipped as ~850 lines for a failure the PRD itself called harmless today. Apply the gate to your own questions first.
- **Defensive code is not an extra.** Validating input, a null check, a `try/catch`, a sane default: build them, and never ask about them.

**Interview discipline**

- Probe vague answers for a concrete example.
- When the owner is unsure, propose two or three shapes with their costs, don't ask an open question.
- Reflect back what you understood before moving on.
- **Anything unsettled becomes an `OPEN-n`**, not a guess. A parked decision is a legitimate deliverable: a PRD that stays in `draft` because its *point* is fifteen open questions is doing its job, not failing. An extra with no evidence is not unsettled: it is left out until the evidence shows up.

## 3. Recon — verify every code claim

**The PRD's authority comes from this step.** Before writing, go read the code.

- Ground every factual claim in `file:line`. "The controller is slow" is worthless; "`playbookProps()` (`:245-271`) runs three `whereJsonContains` counts per rule over 28 days" is actionable.
- Check the claims the *owner* made too. This is where recon earns its keep: a reported bug turns out to be a deliberate behaviour, or a confidently diagnosed root cause turns out to be a different one-line bug somewhere else.
- Find the dead code. A file with zero importers is not the live path, and specifying against it wastes a slice.
- Note anything that will collide with other live features.
- **Recon corrects facts; it does not add machinery.** A finding that suggests a new mechanism (a cap, a retry, a guard, a sweep) is an extra: it goes to the owner through the gate in §2, never straight into an FR. A code fact is evidence only when it shows the failure happening today, not that it could.
- **Find what can prove the work.** The surface a user touches; an existing harness (`.claude/skills/verify-*`, `tests/Browser`, a Playwright config, a CLI test harness); seeders or fixtures that reach the needed state; the gate (`factory docs gate`). This feeds §QA plan — ask the owner only what the code cannot answer.

Collect it all in a **Verification notes** section at the end of the PRD, dated, with a "re-verify at slice entry" line.

## 4. Adversarial pass — hand the draft to something that did not write it

**Do not review your own draft.** An author defends its own reasoning; that is why this step used to find so little. The pass is delegated to a reviewer with **fresh eyes and no inherited context**, using the `second opinion` role in [_shared/models.md](../_shared/models.md): run `factory review second --doc docs/features/{NNN}-{slug}/prd.md --prompt "<the instruction in the next paragraph>"`, cross-vendor and unconditional. Exit 2 is unreviewed: give the same instruction to a fresh agent on the judgment model instead, and say so. Never a `fork`: a fork inherits the reasoning you are asking it to attack.

Give it the draft, the `file:line` claims, and one instruction: **find what cannot be true.** Not style, not scope, not whether the feature is a good idea, and not more failure scenarios the draft could also handle. Claims about the code that recon got wrong, decisions that contradict each other, acceptance criteria nothing could actually check, an FR that two sections define differently, an FR a `factory` run could not build without a human — and, **for every acceptance criterion, the surface that would show it moving**: a criterion with no observable surface passes every review and breaks at slicing (arca PRD-015 FR-40 reached the slicer before anyone noticed that nothing an integrator could see moved with a tenant clock).

Then judge what comes back yourself. A reviewer without the interview in its context will flag things the owner already settled — that is re-litigation, not a finding, and it gets a one-line rebuttal rather than a change. **A scenario the draft does not handle is not a finding either**, unless the reviewer shows it has happened or that today's code makes it happen. A what-if gets the same one-line rebuttal; if it seems worth the owner's time, it goes through the extras gate in §2, "Leave it out" first. **You own the draft; the reviewer only owns the challenge.**

For each claim that genuinely breaks:

**Correct it inline and visibly.** Do not quietly delete the wrong version:

> *"An earlier draft claimed the sidebar was flat and cited `AppSidebar.vue:33-66`. That file is dead code — the live nav is `useNavItems.ts`, and it is already grouped."*

The wrong version having been confidently argued is information for the next reader. Mark corrections with the date. The same rule holds after the PRD is agreed: a builder or a polish pass that falsifies a claim writes the dated correction into the PRD, because the next run reads the PRD, not the chat (an A/B run built the uncorrected PRD and shipped the leak it described).

**Say in the report who reviewed it and what they found** — including "the pass found nothing", which is a result. A skipped adversarial pass is stated as skipped, with the reason.

## 5. The cut — one question before the PRD is final

Extras pass the gate one at a time, each looking reasonable on its own; together they are where a PRD doubles. So once the adversarial pass is judged, **list every mechanism in the draft that no Why-now pain asks for**: each extra, each figure or counter, each recovery or release path, each place a mechanism is applied beyond where its evidence came from. Give each one line: the scenario it handles, its evidence or "none", and a rough size.

Then ask **one multi-select AskUserQuestion: "keep which?"** Everything unticked is cut: its FR section stays, titled `RETIRED`, and it becomes a `Not handled:` line in §Non-goals. Nothing to list → skip the question and say so in the report; that is the result a good interview produces. The tool takes 2-4 options per question and up to four questions per call, so past four items split them by FR, one question each; a lone item is a keep/cut pair.

## 6. Write it

`factory docs new` wrote the scaffold; you fill it. Adapt the section list to the feature — a bugfix PRD is a few hundred words, an enhancement batch is a dozen FR sections. **Keep the sections numbered**: slices cite them (`prd.md → §3`), and a builder's pack carries only the sections its slice cites, so a fact that belongs to one slice goes in that slice, not here.

**This is a build document, not a presentation.** The readers are the author and the coding agent. No personas, no stakeholder tables, no success-metrics dashboards, no rollout-phase theatre — if a section cannot change a line of code, it is context the agent pays for and gets nothing back. If a human audience ever needs a version of this (a client, a PM), generate that separately, on request.

Section by section, what the scaffold leaves to you:

- **Frontmatter.** `type`, `author`, and **`summary`**: two or three lines on what this makes true and where it stands. `factory docs status` prints it, so keep it current through the feature's life; it replaces any notes cell. Never a status, and never `factory docs set NNN reviewed=…`: `reviewed` records that the *code* went through `/polish`, and setting it early makes a run's resume skip polish.
- **§Why now.** The pressure. The bugs the owner actually hit, separately from the improvements, and one line on what this deliberately leaves alone.
- **§Blast radius.** Not personas — what this can break: who uses the surface and what they would notice, external contracts (untouched and decided, or what breaks and how it is versioned), cost and operational surfaces only if a decision moves them. Drop any bullet with no consequence for the code. When nothing outside the FRs can break, delete the section and renumber the ones after it.
- **One section per FR.** What is true today, with `file:line`. Then the decisions taken, dated, each with its **accepted cost** stated plainly and why the alternative lost; a decision with no stated cost reads as unconsidered. Then **Acceptance**: a number with a load, or an observable behaviour. An FR never asks the code to carry a comment: an invariant it must keep becomes a test that pins it, a type or a name. A retired FR keeps its section: whose call, why, and that the recon went to `docs/ideas/` with `factory docs idea add` so a future PRD does not redo it.
- **§Decisions log.** Every `OPEN-n`, with what each option costs and what unblocks it. A settled one stays, struck through, with the date and the answer.
- **§QA plan.** Gate, surface, harness, seed state, and one proof row per FR: a path a user could take, not "tests pass". An FR with no UI (a scheduler, a worker, an API contract) is proven below the UI: its Proof starts with `run` and names the test or command whose transcript is the evidence. The Proof cell names a test by its behavior name, and the test never names the FR: the row is the only map from FR to test. **Human?** is yes only for a judgment a machine cannot make, and those rows are all `qa-guide.md` is generated from. A missing harness is a decided prerequisite with a cost (a harness slice, first in the order), not an OPEN. The PRD is not `ready` without this section; `factory docs check` warns `W-NO-QA-PLAN` until it exists.
- **§Slices.** Leave the scaffold's line; `/slice` fills it.
- **§Non-goals.** The thing people will assume is in scope and where it lives instead, and a `Not handled:` line for every extra the gate or the cut left out, with the date, the why, and the signal that would reopen it. Slices quote these as deliberate non-changes.
- **§Verification notes.** Checked against which branch, when, corrected when; one `file:line` fact per line, the dead code named; "re-verify at slice entry".
- **A bugfix** collapses to: what happens vs what should, root cause with `file:line` (a falsified first diagnosis stays, with its refutation), impact, the fix with its accepted cost, acceptance (the reproduction no longer reproduces, plus the regression test), QA plan, verification notes.

Style: link sideways to other PRDs, ideas and the archive, and keep the prose precise enough that a builder cannot read it two ways (`main..HEAD` and `main...HEAD` count different commits; one PRD said the first and meant the second).

## 7. Register it

Not done until the board agrees and the check is clean:

1. **`summary:`** says what is actually decided.
2. **Build order** in `docs/index.md`, when this feature contends with another for the same files, with the reason.
3. A cancelled *feature* keeps its number burned: a tombstone folder in `archive/` with `status = "retired"` in its `state.toml` and one page on why it died and where the research went. That is what keeps the next free number computable from the folders alone.
4. If the feature came from an idea, `--from-idea` already deleted its file.
5. Run it, and **fix what it reports** until it is clean:

```
factory docs check
```

6. **Agreed → `factory docs set NNN status=ready`.** Still open → it stays `draft`, and the summary says which OPENs hold it. Commit on `feat/{slug}`, never the default branch (`git switch -c feat/{slug}` first when HEAD is there): `docs(NNN): PRD-NNN <title>`.

## 8. Hand off

End with one recommendation:

- **Big / multi-FR** → `/slice` to slice it.
- **Small** → no slices, build straight from `prd.md`. Say so in the summary so nobody looks for a missing slice board.
- **Still open** → say which `OPEN-n` block the build, and that the PRD is deliberately unsliceable until they close.
- **Agreed** (§7 set it `ready` and committed it) → the last act is one `AskUserQuestion`: **Build it now (Recommended)** / **Slice only** / **Stop here**. A tool question reaches the owner's phone; a prose offer does not.
  - **Build it now** → invoke `/deliver NNN` with the Skill tool, in this session; it slices the PRD itself. Running it is the approval: it builds and proves each slice, polishes, and opens a pull request with CI green. It never merges.
  - **Slice only** → invoke `/slice NNN`, attended.
  - **Stop here** → end with the recommendation above that fits.

  The owner may still `/clear` and type `/deliver NNN` at any time.

And, whichever of those applies:

- **No verify gate declared** (`factory docs gate` exits 1) → say so, and that `/slice` will not write slices against a blank Verify task. `/gate` builds one; `factory docs gate set "<cmd>"` declares one that already exists.
- **§QA plan says the harness is missing** → the harness slice goes first, whatever else the ordering argument says.
