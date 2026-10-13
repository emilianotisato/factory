---
title: "Decide what a team repo commits: evidence, recordings and local paths"
state: "open"
added: "2026-10-13"
origin: "owner"
retrigger: "the first repo with a second developer, or PRD-004"
---

Every feature folder commits run output as it came off the owner's machine, and nobody chose that for a team repo. A sweep on 2026-10-13 found local paths (`/home/emiliano`, `/tmp/claude-…`, `/mnt/devcache/…`) in 57 tracked files under `docs/`: `evidence/` transcripts (24 in 001, 15 in 002), the `claude-p-*` recordings, the ctx-jev replay, three committed `__pycache__` files, and PRD text itself. No secrets turned up, but nothing stops one either: a recording or a gate log can carry an env dump, a token, a customer name.

To decide: what a feature folder holds when several devs share the repo. Lean: specs, the log and the retro are committed; evidence and recordings are run artifacts that live in the run dir (or one gitignored folder) and are linked from the PR, not the tree. Whatever is committed goes through a scrub step (home dir → `~`, scratch paths → `<scratch>`, a secret-pattern refusal) written once by the driver, not by each worker. Also: `docs/.gitignore` gets the obvious junk (`__pycache__`), and `docs check` warns on an absolute home path in a tracked doc. Applies to every repo on the framework, so it lands with PRD-004's installer at the latest.
