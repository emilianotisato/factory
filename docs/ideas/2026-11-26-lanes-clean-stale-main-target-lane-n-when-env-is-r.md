---
title: "lanes clean: stale main/target/lane-N when env is relative target/lane-N"
state: "open"
added: "2026-11-26"
origin: "PRD-020 polish"
---

**Chose:** park for a follow-up
**Over:** changing slice-02 acceptance and this repo lane env in the same PR
**Because:** relative CARGO_TARGET_DIR resolves under the lane worktree, so main/target/lane-N leftovers may be stale; needs owner call on live-dir rules
**Undo:** delete the idea
