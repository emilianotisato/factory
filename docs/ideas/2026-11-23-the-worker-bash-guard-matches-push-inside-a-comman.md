---
title: "The worker Bash guard matches 'push' inside a command body"
state: "open"
added: "2026-11-23"
origin: "harvest 2026-11-23"
---

**What happened.** The guard that denies a worker `git push` and `gh` matches the text of the whole Bash call, not the command words. In PRD-024 it refused:

- a Python edit script containing `out.push(`;
- a heredoc whose body (an AD entry) used the word push.

Both were refused as "gh or git push". That cost two retries and pushed every multi-line edit through a scratch file for the rest of the run.

**What it should do.** Match the command words: the program being run and its arguments, not string literals, heredoc bodies or method calls. A worker calling `git push` or `gh` is still denied whole, which is the rule from `decisions.md#who-talks-to-github` and is not in question here.

**Minimal shape.** A worker sends `python3 - <<'PY' … out.push(line) … PY` → it runs. A worker sends `git push origin HEAD` → still denied.

Not the same as the 2026-11-10 idea about the bare-name grep hook refusing a batched command; that is a different hook with a different fix.
