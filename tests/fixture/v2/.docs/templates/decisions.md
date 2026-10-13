# Engineering Decisions

Standing calls that outlive the feature that raised them. Written during `/polish`, when a review finding needed a design decision that could plausibly come up again.

**Read this before escalating a review finding.** An exact match means the question is already answered — apply it and cite it, do not ask again. A near match means ask, but lead with this as the recommended option.

**Rules**
- `DEC-n` is monotonic and never reused.
- A reversed decision is **superseded, not deleted** — leave the entry and add `**Superseded by DEC-n.**`
- **Most decisions do not belong here.** An entry earns its place only if it could recur, was not obvious, and has a real **Applies when** trigger. If the trigger is one function in one file, it is a fix, not a decision. Noise makes this unsearchable, which defeats the point.
- Not a style guide and not a rules file — those are `CLAUDE.md` / `AGENTS.md` / `.ai/guidelines/`. This is a record of forks taken, with the road not travelled.

---

### DEC-1 — {the question, phrased as a question} · {YYYY-MM-DD} · {PRD-013 slice-03}

**Chose:** {what}
**Over:** {the alternative, and why it lost}
**Because:** {the reason that will still make sense in six months}
**Applies when:** {the trigger a future agent will recognise}
