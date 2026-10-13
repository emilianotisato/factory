---
number: {{NNN}}
slug: {{SLUG}}
status: draft
owns_fr: [{{FR}}]
retired_fr: []
---

# PRD-{{NNN}} — {{TITLE}}

**Type**: _(Feature | Bugfix | Enhancement batch | Refactor — which surfaces, and whether it carries migrations or contract breaks)_
**Status**: **DRAFT** — _(what is actually true right now, with dates)_
**Created**: {{DATE}}
**Author**: _(name)_ (with Claude)

**Owns FR range**: **FR-{{FR}} …**. *(Numbers are never reused.)*

---

## 1. Why now

_(The pressure. Name the bugs someone actually hit, separately from the improvements.)_

## 2. Users affected

- _(audience — which FRs touch them)_

## 3. FR-{{FR}} — _(requirement title)_

_(What is true today, with `file:line`. Then the decision, with its date. Then the accepted cost.)_

**Acceptance**: _(a criterion someone can check)_

## 4. Decisions log

- **OPEN-1** — _(the question)? What each option costs._

## 5. QA plan

**Gate**: {{GATE}}
**Surface**: _(web app | CLI | API | mobile — what a user actually touches)_
**Harness**: _(`.claude/skills/verify-<app>/` exists | none — the first slice builds it | gate-only: nothing user-visible changes)_
**Seed state**: _(seeder / fixture command, or "none needed")_

| FR | Proof on the real surface | Evidence | Human? |
|---|---|---|---|
| FR-{{FR}} | _(the user path that proves it — not "tests pass")_ | _(screenshot, transcript, read-back)_ | no |

_(Every FR gets a row. "Human?" is yes only for a judgment a machine cannot make — feel, copy, mobile — and those rows are all `qa-guide.md` is generated from.)_

## 6. Slices

_(Not sliced yet. Run `/prd-to-tasks`, or write "No slices — build straight from this document" if it is small.)_

<!-- docs:slices -->
<!-- /docs:slices -->

## 7. Non-goals

- _(the thing people will assume is in scope, and where it actually lives)_
- **Not handled: _(scenario)_.** Accepted _(date)_: _(why)_. Revisit if _(signal)_.

## 8. Verification notes

Checked against `main` {{DATE}}. **Re-verify at slice entry.**

- `file:line` — _(the fact that decides a design choice above)_
