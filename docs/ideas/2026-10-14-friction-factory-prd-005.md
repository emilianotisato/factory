---
title: "friction: factory PRD-005"
state: "open"
added: "2026-10-14"
origin: "factory PRD-005 run 005-20261014-d1ad"
---

- **The worker hook's push rule matches the words, not the command.** It refused a Python heredoc that held Rust's `.push(` next to the word `git`. It also refuses recursive grep over `src`. Each refusal cost a round trip and a scratch file. Match the parsed command instead, in the factory hook that `factory init` installs.
- **`/polish` §1, second opinion.** The fallback when Cursor is out is a same-vendor agent. The skill says this is weak evidence, but nothing tells the owner before the run starts. Preflight could check `cursor-agent` auth once and name it.
- **`/polish` §8 and the bundle lock.** A skill edit needs a version bump, and the lock refuses a second change at one version. So a polish worker cannot fix a stale skill line it finds. It can only park the line.
- **`/deliver`, the slice commit by pathspec.** It is the cause of AD-5, AD-6 and AD-20. PRD-005 deletes it. The friction is recorded here because this was the last run to pay for it.

From /home/emiliano/Projects/factory/docs/features/005-isolated-lanes/retro.md
