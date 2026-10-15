---
title: "Cut the builder's start context and its one-call turns"
state: "open"
added: "2026-10-15"
origin: "harvest 2026-10-15"
---

Factory's builder cost reached 86% of manual against a 60% target. A builder starts at 49k tokens and 90% of its turns make one tool call. Measured over 91 builders and 74.9M weighted ([builder-cost-research.md](../reference/builder-cost-research.md)): about 30k of the start is what a build needs, and orchestration is not the cause (a `/deliver` subagent and a factory worker cost 18.0k against 18.1k per turn on the same PRDs). The levers, cheapest and surest first:

1. **Three flags on the worker command** (`src/worker.rs`): `--strict-mcp-config --disable-slash-commands --tools Bash,Read,Edit,Write`. Drops 17.1k of unused tool definitions, skill and agent listings and MCP; about 8.0M (10.7%). Builders used the dropped tools 9 times in 3,871 turns. Check the same for checkpoint and polish workers.
2. **Batch reads by mechanism, not by asking.** The pack already says "batch independent lookups"; builders do not, and Claude Code's bypass-mode reminder tells them to read with `cat` and `sed -n` (912 Bash reads against 145 Read). About 4.8M (6.4%). Shapes: a multi-range read (`factory ctx show f:a-b g:c-d`), a first turn that reads every file the slice's Current code names in one message, or a permission mode without that reminder.
3. **The driver owns docs bookkeeping**: set building before the spawn, done and `docs check` after. About 1.1M.
4. **`fmt` in write mode before the checks**, the gate keeping `--check`. Up to 1.3M.
5. **Quieter test output** (failures and the summary line only), about 0.9M; **a builder-only Autonomy cut**, about 0.9M.
6. **Untested, one probe or run each:** `--setting-sources project,local` (0.6–0.9M), effort high to medium (up to 2.6M), a pack that pre-loads the Current code ranges (1–4M net, overlaps 2).

Not levers: smaller slices or a session per task (each pays the 49k start and about 10 orientation turns again; −0.8M to +0.9M), and a lower compaction threshold.
