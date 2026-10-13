# Agent log — PRD-001

What an unattended run (`/deliver`) decided on its own, and where it stopped. **The owner reviews this in the pull request.** What earns an entry is defined in the feature-docs skill's `autonomy.md`.

**Rules**
- `AD-n` (decisions), `BLOCK-n` (blockers) and `CP-n` (checkpoints) are numbered per feature, monotonic, never reused. `docs check` rejects a duplicate.
- Only what the PRD, the slice or a `DEC-n` did not already settle.
- Never delete an entry. When the owner reviews a decision (`/deliver` § 8), append `**Owner ({date}):** kept` or `**Owner ({date}):** changed — {to what}` under it, and leave the entry as it was.
- **Headings are machine-read — keep their shape.** `docs status` and `docs preflight` resume a run from them.

## Traps

What this repo does that cost a builder a run and will cost the next one too: a tool that takes no paths, a test that needs hidden setup, a formatter that rewrites files outside the slice. One line each, with what works instead. Every builder reads this first and adds what it learned.

- `gate.sh --inner` exits 0 without running anything: run `./gate.sh` to see a real failure.

## Blockers

### BLOCK-201 — Slice c needs a failing gate mode · slice-03 · resolved

**Happened:** `gate.sh` has no way to fail, so slice c cannot prove its red path.
**Options:**
1. add a `--fail` flag — one line in `gate.sh`
2. skip the red path — the slice's acceptance goes unproven
**Recommended:** 1, because it is one line.

**Resolution (2026-10-03):** add the flag, as recommended.

## Decisions

### AD-10001 — Re-run the gate at the checkpoint? · checkpoint · 2026-10-03

**Chose:** one more full run.
**Over:** trusting the slice runs.
**Because:** two lanes merged into one tree.
**Undo:** drop the run.
**Review:** fyi — a reviewer reading the diff would miss the extra run

### AD-201 — Name the scenario files by slice · slice-02 · 2026-10-02

**Chose:** `scenario/slice-02.txt`.
**Over:** one shared file.
**Because:** two lanes write at once.
**Undo:** rename the files.
**Review:** fyi

**Owner (2026-10-04):** changed — name them by step, `scenario/step-2.txt`.

### AD-101 — Keep lib.txt plain text · slice-01 · 2026-10-01

**Chose:** plain text.
**Over:** JSON.
**Because:** the gate greps it.
**Undo:** convert it.
**Review:** decide

**Owner (2026-10-04):** kept

### AD-1 — Preflight lanes: proceed as-is · preflight · 2026-09-30

**Asked:** two lanes on a three-slice fixture.
**Chose:** proceed as-is
**Review:** no

### AD-501 — Shorten the README · polish · 2026-10-05

**Chose:** one paragraph.
**Over:** the full walk-through.
**Because:** the PRD holds the walk-through.
**Undo:** restore it from git.
**Review:** decide

## Checkpoints

### CP-1 — after slice-02 · 2026-10-03

**Covers:** slice-01, slice-02.
**Gate:** green, 2 runs.

---

Entry shapes. Copy them above; examples inside this fence are ignored by the tool.

```markdown
### BLOCK-1 — {what stopped the run} · slice-02 · open

**Happened:** {what, with the `file:line` or command output that proves it}
**Options:**
1. {option} — {its cost}
2. {option} — {its cost}
**Recommended:** {n}, because {why}
**Resolution ({date}):** {appended when the owner answers — then the heading ends in `resolved`}

### AD-1 — {the question, phrased as a question} · slice-02 · {YYYY-MM-DD}

**Chose:** {what}
**Over:** {the alternative, and why it lost}
**Because:** {the reason, citing the PRD section that left it open}
**Undo:** {what reverting costs — a file, a migration, a dependency}
**Review:** {decide | fyi | no}

### CP-1 — after slice-03 · {YYYY-MM-DD}

**Covers:** slices 02, 03
**Gate:** {runs} runs, {red} red, {minutes} min — caught: {what, or nothing}
**Prove:** {the QA-plan rows driven} — evidence: {paths}
**Fixed:** {what a red run or a failed row needed, and its commit} · or none
```

- The middle field is `slice-NN`, or the phase when no slice owns it: `preflight`, `slicing`, `polish`, `pr`, `ci`.
- **`Review: decide`** when the owner must answer before it ships (a deviation from the slice or an acceptance criterion, a schema or public interface the PRD did not specify, a new runtime dependency); **`fyi`** when a reviewer reading the diff would miss it; **`no`** otherwise, which is most entries. `decide` entries go to the top of the PR body.
- A `CP` heading names the last slice it covers; `docs status` computes what is unverified from it.
