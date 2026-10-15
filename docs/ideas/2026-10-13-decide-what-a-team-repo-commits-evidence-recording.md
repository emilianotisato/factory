---
title: "Decide what a team repo commits: evidence, recordings and local paths"
state: "open"
added: "2026-10-13"
origin: "owner"
retrigger: "the first repo with a second developer, or PRD-004"
---

Every feature folder commits run output as it came off the owner's machine, and nobody chose that for a team repo. A sweep on 2026-10-13 found local paths (`/home/emiliano`, `/tmp/claude-…`, `/mnt/devcache/…`) in 57 tracked files under `docs/`: `evidence/` transcripts (24 in 001, 15 in 002), the `claude-p-*` recordings, the ctx-jev replay, three committed `__pycache__` files, and PRD text itself. No secrets turned up, but nothing stops one either: a recording or a gate log can carry an env dump, a token, a customer name.

To decide: what a feature folder holds when several devs share the repo. Lean: specs, the log and the retro are committed; evidence and recordings are run artifacts that live in the run dir (or one gitignored folder) and are linked from the PR, not the tree. Whatever is committed goes through a scrub step (home dir → `~`, scratch paths → `<scratch>`, a secret-pattern refusal) written once by the driver, not by each worker. Also: `docs/.gitignore` gets the obvious junk (`__pycache__`), and `docs check` warns on an absolute home path in a tracked doc. Applies to every repo on the framework, so it lands with PRD-004's installer at the latest.

**2026-10-14, after PRD-007.** The owner set the frame: weight does not matter; what matters is what agents read and what a team of people and their agents on different machines shares. A run starts and ends on one machine, and nothing hands it to another mid-way. So evidence has two readers, the run and the PR reviewer, and neither needs the tree. PRD-007's run committed 8 evidence files while its PR body said "Evidence lives on the machine that ran the build": the polish contract already assumes this model and the driver does not do it yet. The lean sharpens:
- **The run dir, not a gitignored folder.** Lanes are separate worktrees, so an ignored file never leaves the lane that wrote it. Evidence belongs beside `driver.log`.
- **Copied into the PR, not linked.** A teammate cannot open a path on the owner's machine, and the owner reads the PR from a phone. The driver already owns every GitHub call (DEC-2), so it puts the text evidence in the PR (a collapsed block per checkpoint, the body caps at 65,536 characters). Open: screenshots. As far as known, `gh` cannot attach an image to a PR; test an alternative before deciding (an orphan branch the driver pushes and links would need the pre-push guard to allow it).
- **Nothing is lost from the tree.** The log's checkpoint entries already sum up each evidence file in a line, and that is the part worth keeping.
- Slices stay committed until the feature is harvested: see `2026-10-15-archive-harvests-what-a-feature-taught`.
