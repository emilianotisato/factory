# PR body — the shape /deliver fills in

GitHub does not resolve repository-relative links in a PR body, so paths go in backticks. Drop a section only when it would say nothing; keep **Needs your review** and **Human QA** even when empty, saying "none", so the owner knows they were checked.

````markdown
## PRD-{NNN} — {title}

{One paragraph: what this makes true that was not true before.} PRD: `docs/features/{NNN}-{slug}/prd.md`

> Opened by `/deliver`, unattended. **Not merged — that is yours.**

### Needs your review

- **AD-{n}** · slice-{NN} — {the question} → {what was chosen}
  *(entries marked `Review: decide`, then one line "+ {n} fyi entries"; every decision is in `factory docs log show {NNN}`. After the owner review in `/deliver` § 8, each reviewed entry ends in `— kept` or `— changed: {to what}`.)*
- **Decision candidates** — {the question and the recommendation} · or none
- **Rubric proposals** — {the line} · or none
- **OPENs carried, parked by design** — OPEN-{n} · or omit

### Human QA

- [ ] {a Human row from § QA plan} — ~{minutes} min

*(From `qa-guide.md`. Or: "None — every QA-plan row was proven by the run.")*

### Slices

| Slice | FRs | Gate | Proof |
|---|---|---|---|
| {NN} — {title} | FR-{n} | inner {runs}/{red} red · verified at CP-{n} | {CP-{n}: {rows}, or "no Prove rows: why"} |

Evidence stays in the run dir of the machine that ran the build, under `evidence/cp-{n}/`; none of it is committed.

### Review

- **Reviewers**: local ✓ · security {✓ \| skipped — why} · UX {✓ \| skipped — why} · second opinion {model} {✓ \| unreviewed — why}
- **Fixed**: {n} — see each slice's *Code-review fixes*
- **Rebutted**: {the finding} — {why it is wrong}
- **Parked → `docs/ideas/`**: {id}

### Gate & CI

- Gate `{command}`: {runs} runs, {red} red, {minutes} min in total
- CI: {green first time \| green after {n} fix rounds \| no CI configured} · {flaky job re-run once: {name}}

### Retro

Score {n}/5. {The single biggest framework friction, one line.} Full: `docs/features/{NNN}-{slug}/retro.md`
````
