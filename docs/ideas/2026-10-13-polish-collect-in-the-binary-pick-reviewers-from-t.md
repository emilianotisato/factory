---
title: "Polish collect in the binary: pick reviewers from the diff, run them, hand the model only triage"
state: "open"
added: "2026-10-13"
origin: "polish skill review"
---

The collect half of `/polish` is mechanical: diff size and touched paths decide which reviewers run (security paths, UI paths, migrations, auth, money), and the second opinion is a shell recipe. Today the model does both at 30 to 60 minutes and 9 to 18 dollars per polish worker. Shape: `factory review collect` reads the diff against the base, applies the reviewer table from the skill §1 with project-declared path patterns, runs the second opinion (`factory review second`, PRD-004) and returns one findings file; the skill starts at §2 verify and dedup. The local code review, security review and UX review stay model work, spawned by the skill, because they need judgment. Retrigger: after PRD-004 ships and `factory review second` has run in two features.
