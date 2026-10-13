---
number: 001
slug: fixture
status: in-progress
owns_fr: [1]
retired_fr: []
---

# PRD-001 — Fixture

**Type**: Feature — the migration fixture
**Status**: **in-progress** — fixture PRD, slice c building
**Created**: 2026-10-13
**Author**: Fixture (with Claude)

**Owns FR range**: **FR-1 …**. *(Numbers are never reused.)*

---

## 1. Why now

_(The pressure. Name the bugs someone actually hit, separately from the improvements.)_

## 2. Users affected

- _(audience — which FRs touch them)_

## 3. FR-1 — _(requirement title)_

_(What is true today, with `file:line`. Then the decision, with its date. Then the accepted cost.)_

**Acceptance**: _(a criterion someone can check)_

## 4. Decisions log

- **OPEN-1** — _(the question)? What each option costs._

## 5. QA plan

**Gate**: `./gate.sh` *(declared in index.md)*
**Surface**: _(web app | CLI | API | mobile — what a user actually touches)_
**Harness**: none — fixture, gate-only
**Seed state**: _(seeder / fixture command, or "none needed")_

| FR | Proof on the real surface | Evidence | Human? |
|---|---|---|---|
| FR-1 | run slice 01 (a) in the fixture → the board shows it done | test transcript | no |
| FR-1 | run slice 02 (b) in the fixture → the board shows it done | test transcript | no |
| FR-1 | run slice 03 (c) in the fixture → the board shows it done | test transcript | no |

_(Every FR gets a row. "Human?" is yes only for a judgment a machine cannot make — feel, copy, mobile — and those rows are all `qa-guide.md` is generated from.)_

## 6. Slices

_(Not sliced yet. Run `/prd-to-tasks`, or write "No slices — build straight from this document" if it is small.)_

<!-- docs:slices -->
| Slice | Status | Depends on |
|---|---|---|
| [01 — A](slice-01-a.md) | ✅ done | — |
| [02 — B](slice-02-b.md) | ✅ done | — |
| [03 — C](slice-03-c.md) | 🔨 building | 02 |
<!-- /docs:slices -->

## 7. Non-goals

- _(the thing people will assume is in scope, and where it actually lives)_
- **Not handled: _(scenario)_.** Accepted _(date)_: _(why)_. Revisit if _(signal)_.

## 8. Verification notes

Checked against `main` 2026-10-13. **Re-verify at slice entry.**

- `file:line` — _(the fact that decides a design choice above)_
