---
title: "Cross-repo ideas scanner: `factory ideas --json` plus an `/ideas` skill"
state: "open"
added: "2026-10-12"
---

From one session, the owner sees the open ideas of every framework repo and picks the next PRD. The binary scans configured roots, dedupes by git common dir (icg-cloud's `wt-a`, `wt-b`, `wt-base` worktrees would otherwise count it four times), and prints rows with `project`, `repo`, `id`, `title`, the first sentence, `state`, `origin`, `added`, `age_days`, `retrigger`, `priority` and the body length, plus per project its in-flight PRDs, next free number, branch and last commit date. Sorting and filtering are mechanical; the judgment happens in the skill, with the owner. The handoff prints `cd <repo>` and a ready `/prd … --from-idea <id>` prompt, because a PRD needs a session in its own repo. Default output stays short (about 500 ideas would be ~60k tokens in full); `show <id>` prints one body. Brainstormed 2026-10-12; it reads PRD-002 FR-20's `idea list --json` per repo, with a best-effort `ideas.md` read flagged `"format": "v2"` for repos not migrated yet. Read-only in v1; triage (drop, merge) waits for evidence it is needed. Reads `factory bugs` rows too (next line): both by default, `--ideas` or `--bugs` to narrow, each row tagged `"from": "idea"|"bug"`.
