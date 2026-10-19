---
title: "Polish treats only OPEN-n and decisions as settled, so reviewers re-argue slice text and earlier ADs"
state: "open"
added: "2026-10-19"
origin: "factory upstream"
---

## What happened

Across three shipped features, `/polish` reviewers and the `/ux` collector raised findings that the feature's own docs had already settled:

- **Slice text.** A UX pass asked about two strings that a slice's tasks wrote out verbatim. `/polish` § 2 counts only `OPEN-n` and `DEC-n` as settled, so the worker had no rule for it. It treated the slice text as settled anyway and wrote no AD.
- **Earlier ADs.** Five of seven UX asks re-argued choices that the PRD, a slice or an earlier AD had pinned. The UX collector does not read the slice files or the AD list, while `/polish` § 0 hands both to triage.
- **PRD non-goals.** A second-opinion pass re-raised a race the PRD's non-goals section had already accepted.

Two cases have no row in `autonomy.md` at all:

1. **A later phase overturning an earlier AD.** A polish finding asked for exactly what an implementation AD had rejected (a 500 for a malformed id). `autonomy.md` covers an AD against the PRD or a DEC, not an AD against an earlier phase's AD. The worker wrote a new fyi AD citing the old one and left the old one standing.
2. **An Ask whose fix changes the contract while today's behaviour is safe to ship.** Refusing a disabled record needed a new error reason in the public code table. Under the autonomy test that is a block, but nothing waited on it. The worker parked it as an idea. The Mapping table has no row for this.

## What it should do

- Treat slice task text, earlier ADs and the PRD's non-goals as settled, beside `OPEN-n` and `DEC-n`. A finding against one of them is rebutted with a citation, not re-litigated.
- Give the `/ux` collector the same context `/polish` § 0 gives triage: the slice contracts and the AD list.
- Add an `autonomy.md` row for a later AD that overturns an earlier one: allowed, cite the overturned AD, and mark the old one superseded.
- Add a Mapping row for a contract-changing Ask when current behaviour is safe: park it as an idea for the owner, and do not block the run.
