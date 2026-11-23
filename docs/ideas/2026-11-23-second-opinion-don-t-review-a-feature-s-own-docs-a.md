---
title: "Second opinion: don't review a feature's own docs as code"
state: "open"
added: "2026-11-23"
origin: "harvest 2026-11-23"
---

**What happened.** `factory review collect` groups the diff into ~600-line passes and sends `docs/features/NNN-*/` with the code. Three features measured the cost:

- **PRD-014**: about 150 of the 252 lines in the one pass were `prd.md`, the slice, `state.toml` and `log.jsonl`. The pass ran 957 s and every finding it made was in `src/closeout.rs`.
- **PRD-015**: pass 1 was 579 lines of planning docs. The fallback spent a fresh agent on it.
- **PRD-024**: passes 1 and 2 were the PRD, slices and log. They spent about 28 minutes re-reviewing, through the docs, code that passes 3 to 15 covered again.

PRD-017 FR-3 already drops tool-written files (`log.jsonl`, `state.toml`) from a pass. The hand-written planning docs still go.

**What it should do.** When the diff holds code, fold a doc-only pass into the code passes it describes, or leave `docs/features/**` out of the pass grouping. A docs-only diff still gets reviewed as before.

**Minimal shape.** A 250-line bugfix diff, 150 lines of it the feature folder: one pass of about 100 code lines, not one pass of 252. Roughly half the input and half the clock on a small fix.
