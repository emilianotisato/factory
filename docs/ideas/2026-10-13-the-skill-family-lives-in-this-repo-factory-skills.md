---
title: "The skill family lives in this repo; `factory skills install | update` puts it where agents read it"
state: "open"
added: "2026-10-13"
---

Decided 2026-10-13 during the PRD-002 revision: **this is PRD-004, built after 002.** The owner wants the skills held here as the source, installed into a repo (`.claude/skills`, and the Codex and Cursor equivalents) or the user dir, and updated by `factory` when the framework evolves, in full coordination with the binary. Recon (2026-10-13): `~/.claude/skills` symlinks to `~/Dropbox/dev/claude-code-skills`, a git repo with no remote (486 files, 13 MB with `synced/`); the family is deliver 56K, feature-docs 260K, guardrails 40K, polish 20K, prd 28K, prd-to-tasks 28K, test-cleanup 16K, ux-review 28K, verify-app 52K, about 530 KB; `skills/factory-deliver` already lives here and is symlinked out, the pattern to generalise. Questions for the PRD: copy or symlink per target; how a repo pins a skill version against the binary's; whether `caveman`, `codex` and the other non-family skills stay in Dropbox; what `docs doctor` says when the installed family is older than the binary. PRD-002 FR-17 edits the 23 `docs.mjs` lines in place first; 004 redoes them as in-repo files.
