---
number: {{NNN}}
slug: {{SLUG}}
title: "{{TITLE}}"
# type: feature | bugfix | enhancement | refactor
type: feature
created: {{DATE}}
author: _(name)_
summary: |
  _(Two or three lines: what this makes true that is not true today, and where it stands. `factory docs status` prints it; keep it current.)_
---

# PRD-{{NNN}} — {{TITLE}}

## 1. Why now

_(The pressure. Name the bugs someone actually hit, separately from the improvements.)_

## 2. Blast radius

- _(not personas: who uses the surface being changed, which FRs touch them, what they would notice)_
- _(external contracts — API, webhooks, URLs — untouched and decided, or what breaks and how it is versioned)_

## 3. FR-1 — _(requirement title)_

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
| FR-1 | _(the user path that proves it — not "tests pass")_ | _(screenshot, transcript, read-back)_ | no |

_(Every FR gets a row. "Human?" is yes only for a judgment a machine cannot make — feel, copy, mobile — and those rows are all `qa-guide.md` is generated from.)_

## 6. Slices

_(Not sliced yet. Run `/slice`, or write "No slices — build straight from this document" if it is small.)_

## 7. Non-goals

- _(the thing people will assume is in scope, and where it actually lives)_
- **Not handled: _(scenario)_.** Accepted _(date)_: _(why)_. Revisit if _(signal)_.

## 8. Verification notes

Checked against `main` {{DATE}}. **Re-verify at slice entry.**

- `file:line` — _(the fact that decides a design choice above)_
