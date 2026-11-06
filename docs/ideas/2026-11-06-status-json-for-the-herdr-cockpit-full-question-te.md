---
title: "Status JSON for the herdr cockpit: full question text and a change marker that covers non-event changes"
state: "open"
added: "2026-11-06"
origin: "PRD-023 polish"
---

Two gaps in `factory status --json` that the herdr-app cockpit (herdr-factory PRD-001) will hit, found by the cross-vendor second opinion in PRD-023's polish and left alone because each changes what FR-1/FR-3 decided.

**`questions[].text` is the block title, not the question.** Every real block event stores `answers::block_title(..)` (capped at 160 chars) as its `question`: `scheduler::emit_blocked` via `open_block` (`e.title`), `checkpoint.rs:135` and `:932`, `closeout.rs:2140`. The options and the recommendation live only in the block's `log.jsonl` body (`**Options:**` / `**Recommended:**`). An Answer dialog that shows `text` shows a one-line title and no options. Fix options: read the body from the feature's `log.jsonl` by block id in `run_json` (one more file read per open block), or add a `body`/`options` field. `CloseoutBlocked.phase` (pr vs ci) is also dropped: `at` is `closeout` for both.

**`seq` misses changes that write no event.** `budget.used` moves as workers write usage (no event); `set_pr_url` on an already-open PR (`closeout.rs:1034`) and `set_pause` extending a future pause (`quota.rs:155`) change `state.json` only. A reader that skips a poll when `seq` is unchanged keeps stale `pr`, `pausedUntil` and `budget`. Options: document that `seq` covers events only (herdr diffs the payload), or add an `updatedAt`-based or hashed `rev` field.

**Low, from the security pass: a cloned repo can stall the poll.** `feature_folder` and `prd_title` (`src/cli/status.rs`) follow symlinks: `docs/archive/NNN-x/prd.md` pointing at a FIFO blocks the call, `/dev/zero` grows memory. Only a `title:` value ever reaches stdout, and `src/docs/board.rs:170` already reads `docs/features/*/prd.md` the same way; what is new is the archive read and a 10 s poll. Fix, if wanted: require a regular file through `symlink_metadata` and read with a size cap, in both places.
