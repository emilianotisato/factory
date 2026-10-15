---
title: "The worker hook should judge an unsure command by what runs, not by its words"
state: "open"
added: "2026-10-15"
origin: "harvest 2026-10-15"
---

`src/cli/hook.rs` parses a Bash command and denies `gh` and `git push` by what runs. When the parse is unsure, such as a heredoc fed to an interpreter, it falls back to `mentions`: any whole word `gh` or `push` in the text denies the call. A Python heredoc that edits Rust source holding `line.push(` next to the word `git` is refused, and so is a doc edit that names the GitHub CLI. PRD-005, 006, 007 and 008 each paid for it, and so did a run in another repo: a round trip per refusal, and every multi-line edit routed through the Edit tool or a script file.

Shapes: (a) for a heredoc into an interpreter (`python3 -`, `node`, `ruby`), judge the script's own process calls (`subprocess`, `os.system`, `exec`), not every word in it; (b) deny on the word only where it sits in a command position of an embedded shell string. The credential is already withheld (no token, an empty `gh` config, a push-off env), so the word fallback is the second wall, not the only one.
