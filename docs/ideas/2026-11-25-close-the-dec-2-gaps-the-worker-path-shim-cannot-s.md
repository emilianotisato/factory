---
title: "Close the dec-2 gaps the worker PATH shim cannot see"
state: "open"
added: "2026-11-25"
origin: "PRD-027 polish"
---

The worker `git` shim (`src/worker.rs`, `GIT_SHIM`) refuses `push`/`send-pack` only when the worker runs git through `PATH`. Found at the PRD-027 polish (security pass and cross-vendor second opinion, independently):

- **git's own children skip the shim.** git prepends its exec path (`/usr/lib/git-core`, which holds a real `git`) to `PATH` for helpers and child shells: `git subtree push`, `git rebase -x '<cmd>'`, `git submodule foreach`, `git bisect run`, hooks, `git lfs push`. Verified: `git rebase -x 'command -v git'` prints `/usr/lib/git-core/git`. With a remote that has an explicit `pushurl` (which `pushInsteadOf` ignores) or a local path, the push lands. Same class as the absolute-path bypass the PRD accepted, but reachable without the agent naming a path.
- **An interactive shell can rebuild `PATH`.** On the owner's machine `bash -ic 'command -v git'` resolves `/usr/bin/git` ahead of the shim dir (`.bashrc` reorders PATH); `bash -lc` keeps it. Unchecked which of codex, cursor-agent, agy and opencode run tools through an interactive shell.
- **`help.autocorrect=immediate`** turns a typo into a real push (config class, like aliases).
- `GH_TOKEN_VARS` lacks `GITHUB_PERSONAL_ACCESS_TOKEN` (the GitHub MCP server reads it); non-Claude workers load the owner's MCP setup unnarrowed.

Recommended fix: rewrite explicit pushurls in the environment rather than through PATH. `url.<base>.insteadOf` does apply to an explicit `pushurl` (verified: `git remote get-url --push` returned `factory-no-push://…`), so at spawn read `remote.*.pushurl` in the worktree and add one `GIT_CONFIG_*` insteadOf entry per value. Caveat: when `url == pushurl` the fetch URL is rewritten too. Add a live per-provider smoke check that `command -v git` inside each CLI's tool shell is `run_dir/bin/git`.
