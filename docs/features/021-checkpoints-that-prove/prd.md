---
number: 021
slug: checkpoints-that-prove
title: "Checkpoints that actually prove"
# type: feature | bugfix | enhancement | refactor
type: feature
created: 2026-11-05
author: Emiliano Tisato
summary: |
  A checkpoint closes only when it proved what it claims: its covers match the unverified slices and no FR is left `unreached`, or the driver blocks. The mutation tier's survivors are recorded by file:line and handed to the second opinion and prune. Draft from the 2026-11-05 idea triage, not interviewed.
---

# PRD-021 — Checkpoints that actually prove

> **Draft from the 2026-11-05 idea triage.** The owner chose what is in and out, group by group; nothing here is interviewed or verified yet. `/prd` continues this draft: interview, recon, FRs, QA plan. `file:line` facts are from `main` at `ca128f7`; re-verify them.

## 1. Why now

A checkpoint closes as soon as any `CP-n` exists (`src/checkpoint.rs:294`). PRD-002 and PRD-005 had FRs pass green checkpoints unproven. In one client feature, 22 surviving mutants were rediscovered by the second opinion at about 20 minutes a pass.

## 2. Candidate FRs

Kept by the owner at the triage. `/prd` turns each into an FR or cuts it.

- **C-1 — `covers` is enforced** against `board.unverified`, or the driver blocks.
- **C-2 — The `PROVE: … unreached:` line is read:** sent back once, then blocked (prose only today, `assets/autonomy.md`).
- **C-3 — Surviving mutants by file:line**, stored per checkpoint and handed to the second opinion and to prune; per-slice gate numbers in `state.toml`.

## 3. Not in scope (decided 2026-11-05)

- **`docs check` output redirect in packs, the merge pack reading the slice from the commit, skill text awaiting a release** — dropped (the last is *CLI, docs and log correctness* C-3).
- **The gate's pid** — in *Runs that don't hurt the machine or lose work* C-2.
- **Prove runs the happy path only** — in *Skill wording, contradictions and Prove guidance* C-3.

## 4. Source ideas

The full text of each idea this draft absorbed; the idea files are deleted.

### Checkpoint covers enforcement and pack loose ends

_added 2026-10-14, origin: PRD-005 polish_

From the PRD-005 polish review.

- **Checkpoint `covers` is not enforced.** A checkpoint worker that records `CP-n` without `--covers` leaves slices unverified forever; the driver could check the entry's covers against `board.unverified` (`src/checkpoint.rs` after the worker lands) and block. The death-block hint now names `--covers`.
- **`docs check` output is not redirected** in the worker packs, and a redirect that fails fails silently.
- **The merge pack reads the slice file from the main tree** (`src/pack.rs` `for_merge`), not from the commit it resolves; `git show <sha>:<slice file>` would match what the builder saw.
- **The gate's pid is not recorded**, so a killed driver's gate child is not reaped on resume.
- **Skill text the review flagged** under `skills/` waits for the next skill release: a skill edit needs a version bump and the bundle lock refuses a second change at one version.
- **A checkpoint closes with an FR unproven** (harvest 2026-10-15, from PRD-002). It closes as soon as any `CP-n` entry exists (`src/checkpoint.rs`): the contract's `PROVE: … unreached:` line is never read, and `assets/autonomy.md`'s "sent back once, then blocked" is prose only. In PRD-002 no checkpoint's Prove line named FR-20, and it stayed unproven until polish. Refuse to close while an FR of the covered slices has no evidence.
- **Prove runs the happy path only** (harvest 2026-10-15, from PRD-005). Every scenario was the happy path or the one conflict a slice named; polish then found 4 highs that four green checkpoints passed. The checkpoint contract could ask for one hostile case per FR.

### Checkpoints should record surviving mutants by file:line

_added 2026-11-05, origin: factory upstream_

A checkpoint's full gate runs the mutation tier and records only the efficacy number. In one feature, 22 mutants lived at the second checkpoint, all in two slices' tests. The second-opinion reviewer later spent about 20 minutes per pass rediscovering exactly those weak assertions, and PR-mode `prune` re-read every test instead of starting from them.

Should:

1. The checkpoint record lists lived mutants by `file:line` (and the mutated operator).
2. The second opinion and `prune` receive that list as a starting point.
3. Optionally, gate numbers per slice in `state.toml` (runs, red runs, minutes), so the retro's Cost column is filled from disk rather than guessed.
