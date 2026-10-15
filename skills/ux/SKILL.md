---
name: ux
description: Review changed UI against the project's own design rules by driving the real app — capture every state (empty, loading, error, focus, disabled, mobile), check the mechanical things taste usually catches late (spacing off the scale, missing focus rings, no empty state, errors with no way out, contrast, touch targets), and report what a human still has to judge. Also builds and updates docs/ux-rubric.md. Runs inside /polish when the diff touches UI. Use for "ux", "ux review", "check the UI", "does this look right", "review the design", "check the states", "set up the ux rubric".
argument-hint: [rubric]
---

# UX review — the polish you would otherwise do by hand

Most of what gets called taste is not taste. It is a short list of things that are simply missing: the empty state nobody built, the error with no way out, the focus ring someone removed, the spacing that came from nowhere, the button that vanishes at 375px. Those are **checkable by driving the app**, and this skill checks them so the human's attention goes to the part that genuinely is taste.

The two halves:

- **`/ux`** — review the UI this branch changed, against `docs/ux-rubric.md`, by driving it and looking at every state.
- **`/ux rubric`** — write or update the rubric: the project's own answers about spacing, tokens, empty states, error copy, motion, breakpoints. Interview the code first, the owner second.

**This needs a harness.** It drives the real app through `.claude/skills/verify-<app>/`. Missing → say so and run `/harness` first; a UX review from reading source is a guess.

**It also needs eyes.** Half of this skill reads screenshots, so a session without vision cannot do that half — and it fails silently rather than loudly, which is worse. **Degrade in a stated way, never quietly:** run every check that comes from the diff and the accessibility tree (spacing off the scale, off-token color, removed focus indicators, missing accessible names, tab order, contrast and touch targets from computed values, overflow from measured widths), skip the visual judgment, and **say plainly which half you did and which nobody looked at.** Same posture as a missing harness: never imply coverage you do not have. See [_shared/models.md](../_shared/models.md).

## Rubric mode

`$ARGUMENTS` = `rubric`, or no `docs/ux-rubric.md` exists when a review starts and the owner is in the room; see § 2.

**When one already exists, this is a refresh, not a rewrite.** A rubric decays the same way a coverage floor does — by neglect, not violation. Audit it: lines now enforced by a lint rule or arch test (**delete them and move them to the enforced-mechanically list** — that is the file working as designed), lines contradicted by what the code now does, patterns that have appeared since with no line covering them, and reference implementations that have moved or been superseded. Report what you would change, and change nothing without approval.

Otherwise, to build one, follow [references/rubric.md](references/rubric.md): read the design system out of the code (tokens, the component library, the Tailwind config, existing empty states and error components), propose the rubric with every value pre-filled from what you found, and ask the owner only where the code is inconsistent or silent. **A rubric invented from a style guide is noise.** One that writes down what this codebase already does, and settles the two or three places it contradicts itself, is what makes a review objective.

Then stop. A fresh rubric plus a review in one pass means reviewing against rules the owner has not read yet.

## Review mode

### 1. Scope — what actually changed on screen

`git diff --stat` against the base. Keep the files a user sees: pages, components, templates, styles, and the server code that decides what the page renders (an empty list, an error branch, a permission check).

**Map each changed surface to a route or entry point**, using the verify skill's feature map. A component with three call sites gets reviewed at all three — a card that looks right in the list and breaks in the sidebar is the normal case. Nothing user-visible changed → say so and stop; do not review unrelated screens.

### 2. Load the rules

- `docs/ux-rubric.md` — the project's answers. Absent → rubric mode first, when the owner is in the room. The rubric is the owner's file, so a `factory` run never writes one: it runs the mechanical checks against the tokens and components the code defines and says the review had no rubric, which `factory docs preflight` already judged before the run started.
- The feature's `prd.md` §QA plan Human rows, and its Non-goals. **A finding that contradicts a settled decision is re-litigation, not a defect** — same rule as `/polish`.
- `docs/decisions.md` for any decision about the interface.

### 3. Drive every state, not every screen

This is the part that is worth the runtime. For each changed surface, reach the states in [references/inspection.md](references/inspection.md)'s matrix — empty, loading, error, disabled, focus, long content, narrow viewport, and whichever others apply — and capture each one.

**Reaching a state is the hard part and the skill of it is the deliverable.** Empty means a seeded account with no rows, not a CSS toggle. Error means a failing request, throttled or pointed at a dead endpoint. Loading means a delayed response, not a screenshot taken quickly. Use the harness's seeders and its interactive driver; `terminal-browser action -- snapshot | click | fill | screenshot` is the usual way in.

**A state you could not reach is reported as unreached, with what blocked it.** Never as passing.

Evidence lands in the harness's artifacts directory, named by surface and state: `rules-list.empty.png`, `rules-form.error.png`.

### 4. Check what is mechanical

Run [references/inspection.md](references/inspection.md)'s checks against each captured state and against the diff. Spacing off the scale, colors outside the tokens, missing focus indicator, icon-only control with no accessible name, contrast under the threshold, touch target under 44px, animation with no reduced-motion path, text that overflows at length, layout that shifts as it loads.

**Cite what you can measure.** "The row gap is 13px; the scale has 12 and 16" beats "spacing feels off". A finding with a number is a finding someone can act on without opening the page.

### 5. Judge the rest, briefly

*(Skip this step, and say you skipped it, when the session cannot read the screenshots. Steps 3 and 4 stand on their own; this one does not.)*

What is left is real taste, and it gets a short list, not an essay: hierarchy that fights itself, a primary action that is not the visually primary thing, copy that says what the system did instead of what the person should do next, a flow that takes four steps where two would do.

**Say what you would change and why, in one line each.** No mockups, no redesigns. If you find yourself writing a paragraph, it is a design discussion and belongs in front of the owner as a question.

### 6. Report, and feed the loop

Findings sort into the same three buckets `/polish` uses, so they merge into one triage:

- **Fix** — the finding names the problem and the repair is forced. A missing empty state, a stripped focus ring, an unlabelled icon button, spacing off the scale.
- **Ask** — real, but the remedy is a genuine choice. Which of two hierarchies, whether a flow should lose a step.
- **Park** — real, outside this branch's job.

Then **close the loop, which is the point of the whole skill**:

- A correction the owner makes that this review should have caught becomes **a line in the rubric**, so the next review catches it.
- A rubric line that could be enforced mechanically becomes **a lint rule, an arch test, or a gate tier** instead — an ESLint rule banning arbitrary spacing values, a test asserting every icon button has a name, a contrast check in CI. Hand it to `/gate` as a tier. **A rule a machine can enforce should not stay a sentence in a document.**
- A gap the harness could not reach becomes a feature-map fix in the verify skill.

## Inside `/polish`

When the diff touches UI, this runs in `/polish` step 1 beside `code-review`, and its findings go through the same dedup and triage. It is a **collector**, not a separate ritual: one triage, one round of questions, one set of fixes.
