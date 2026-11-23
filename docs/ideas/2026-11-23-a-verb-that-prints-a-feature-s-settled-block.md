---
title: "A verb that prints a feature's settled block"
state: "open"
added: "2026-11-23"
origin: "harvest 2026-11-23"
---

**What happened.** When the second opinion comes back unreviewed, the polish worker briefs fallback agents by hand, and each brief needs the feature's settled block: the decisions the owner already made, so a reviewer does not re-raise the side that lost.

- **PRD-017**: "A way to print a feature's settled block from the CLI (`factory review settled 017`). The fallback brief needs it, and I rebuilt it by hand from the log and the PRD. The binary has `settled::for_feature`, but no verb prints it."
- **PRD-015**: the settled block "had to be dug out of the pass's `.jsonl` by script".

The data is already computed and already sent to each vendor pass. Only the hand-briefed fallback cannot reach it.

**What it should do.** Either a read-only verb, `factory review settled NNN`, that prints the block, or `factory review collect` writing it to its own file in the run directory so a brief can point at the path. The second is cheaper and survives a collect that fails afterwards.

**Minimal shape.** `factory review settled 017` → the same block the vendor passes get, on stdout, ready to paste into a fallback brief.
