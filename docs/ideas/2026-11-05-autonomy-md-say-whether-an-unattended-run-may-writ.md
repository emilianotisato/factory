---
title: "autonomy.md: say whether an unattended run may write research notes and operations docs"
state: "open"
added: "2026-11-05"
origin: "factory upstream"
---

`autonomy.md` § "What an unattended run may write" leaves two common cases unnamed, and runs guessed:

- **Research notes the run added itself.** A run misplaced a paragraph in a `docs/research/` note on its own branch and repaired it, unsure whether that was allowed.
- **Operational procedure docs the feature makes stale** (a project's `docs/operations/` or similar: runbooks, compliance procedures someone signs). Reviewers found dead links and built work described as pending. The run fixed the text but left the open checklist rows alone, because closing one changes what a human signs.

Should: name both explicitly, one way or the other. A reasonable default: a run may repair what it wrote on its own branch, and may update project docs its FRs make factually stale, but never closes a human sign-off item.
