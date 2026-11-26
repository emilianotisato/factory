---
title: "providers: live codex/opencode JSONL recordings, review --dry-run, and docs log add --title-file"
state: "open"
added: "2026-11-26"
origin: "harvest 2026-11-26"
---

From PRD-026 and PRD-027 retros:
- Capture real JSONL stream recordings from live `codex exec --json` and `opencode run --format json` runs (today non-Claude stream tests translate the Claude stub).
- Add `factory review second --dry-run` to print each provider's exact argv without invoking the CLI.
- Add `--title-file` on `factory docs log add` so titles containing backticks are not mangled by shell substitution.
