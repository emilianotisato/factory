---
slice: {{NN}}
status: pending
depends_on: {{DEPENDS}}
---

# Slice {{NN}} — {{TITLE}}

**Source**: [`prd.md`](prd.md) → §_(n)_. **Depends on**: {{DEPENDS_PROSE}} _(and why)_
**Created**: {{DATE}} · **Status**: pending

**Progress**: _(written as it lands — what was built, the gate numbers, what a review caught)_

**Slice**: _(one paragraph — what this makes true that is not true today, and what it leaves to another slice)_

## Current code / environment

- **_(the fact that decides the design)_**: `file:line` — _(precise description, and the consequence)_
- **_(the constraint that is not obvious)_**
- **_(the deliberate non-change)_** — *(prd §n OPEN-k decided against it. Do not add it here.)*

## Tasks

### 1. _(title)_ — high — blocked by: —
**Source**: §_(n)_ FR-_(x)_: "_(the PRD words this task builds)_".

`path/to/file`. _(What to do and how. The pattern to follow.)_

**Acceptance:** _(a test that could fail — what it asserts, on what input)_

### 2. Verify — medium — blocked by: 1
{{GATE}} _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 3. Prove — at the checkpoint — blocked by: 2
_(Name the `prd.md` § QA plan rows this slice must prove on the real surface — the user path, not the test suite. They are driven at the checkpoint that covers this slice, together with the full gate, and the evidence goes in this slice's **Progress**. Keep this task only when the slice has a user-visible FR; otherwise delete it and say so here.)_

**Acceptance:** _(after the checkpoint, each row named here has evidence — screenshot, transcript, read-back of the stored value)_
