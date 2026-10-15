---
title: "Widen the hook's process-call list"
state: "open"
added: "2026-10-15"
origin: "PRD-009 polish"
---

The hook's script reading (`process_call` in `src/cli/hook.rs`) matches the list PRD-009 FR-3 fixed, and FR-3 accepted that a call off the list runs. The polish review of PRD-009 found these off-list spellings, each denied on main (any inline script with a mention was unsure) and allowed now:

- `ruby -e 'system "gh pr list"'`, `perl -e 'exec "git", "push"'`: a call with no parentheses.
- `python3 -c '__import__("os").system("gh pr list")'`: the `.system(` follows a `.`, so the bare-call rule skips it.
- `bun -e 'Bun.spawn(["git","push"])'`, `lua -e 'io.popen("git push")'`, `ruby -e 'Kernel.system("gh")'`: a call behind a dot.
- `deno eval 'new Deno.Command("gh",{args:["pr","view"]}).outputSync()'`: Deno's process API is not on the list.
- `osascript -e 'do shell script "git push"'`, `tclsh` `exec`.

Options: add `__import__`, `Deno.Command`, `Bun.spawn`, `io.popen`, `Kernel.`, `do shell script` to the list, and a paren-less `system ` / `exec ` / `spawn ` at a word start. Each addition can only add denials; measure the false-denial count against the store's runs first (the FR-3 research counted 31).

The backstop stays: a worker has no GitHub token, an empty gh config dir and a push-off git env (`docs/decisions.md#who-talks-to-github`).
