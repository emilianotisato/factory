---
title: "Isolate the adversarial reviewer from repo and user config"
state: "open"
added: "2026-11-25"
origin: "PRD-026 polish"
---

The reviewer runs with its cwd at the branch under review, so the branch can loosen it:
- OpenCode: a committed `opencode.json` or `.opencode/agent/plan.md` can redefine the `plan` agent's permissions (src/review.rs, Opencode arm of `run_pass_resolved`). Pass an explicit permission config with edit and bash denied.
- Claude: project `.claude/settings.json` hooks still run, and `--tools Read,Grep,Glob` does not remove user MCP servers. Consider `--strict-mcp-config` and `--setting-sources user`. Note that the scrub keeps `CLAUDE_CONFIG_DIR` (src/worker.rs `scrubbed`).
- Codex: `--sandbox read-only` holds, but MCP servers from `~/.codex` run outside the sandbox.
- Only Claude gets the env scrub; the other reviewers inherit the full environment.
- A `--doc --prompt` text starting with `-` reaches claude, agy and opencode as a flag. A `--` before the prompt, or stdin as Codex does, fixes it, once each CLI's parser is checked.
Each one needs checking against a live binary. A review that writes into the tree it reviews is the failure to prevent.
