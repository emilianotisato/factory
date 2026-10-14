---
title: "friction: factory PRD-006"
state: "open"
added: "2026-10-14"
origin: "factory PRD-006 run 006-20261014-0643"
---

- **polish § 1, second opinion.** When Cursor is `unreviewed`, the fallback costs one fresh agent per 600-line pass. On this 2000-line diff that was three agents, mostly re-finding the local review's points. A cheap auth probe in `/deliver` preflight would move the failure to where it costs nothing.
- **The worker PreToolUse hook.** It refuses any Bash command whose text contains the push or GitHub CLI words, even inside a heredoc of an edit script. Every multi-line edit had to go through a script file first.
- **polish § 8.** "The slice's Code-review fixes section" assumes one slice. A per-slice rule ("each fix goes to the slice that owns the code it touches") would remove the guess.

From /home/emiliano/Projects/factory/docs/features/006-comment-discipline/retro.md
