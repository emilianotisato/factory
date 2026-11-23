---
title: "A checkpoint send-back tells the respawn why, and does not keep stale evidence"
state: "open"
added: "2026-11-23"
origin: "PRD-021 polish"
---

Found by the local review and the second-opinion fallback in PRD-021 polish; today's behaviour is safe to ship (a second short record blocks), so it was parked, not fixed.

- **The respawn is not told why.** `src/checkpoint.rs`, the send-back `continue` in the checkpoint loop, rebuilds the pack with `Pack::for_checkpoint` and the same inputs, so the second worker gets the same pack and prompt (only the spawn number changes). Nothing names the slices the first record left out or the rows with no evidence. Two workers doing the same thing is what turns a send-back into a block, and the block text then blames "the pack or the contract".
- **The first worker's fixes are lost and its evidence is kept.** The loop calls `scheduler::prepare_lane(run, lane, true, …)`, which resets the lane to the feature tip, throwing away the first worker's leak and defect fixes. Its `$FACTORY_EVIDENCE/cp-{n}/<key>/` dirs stay, and `unproven_rows` counts them for the second worker. That evidence was taken on a tree that no longer exists.
- **Shape:** add a `# Sent back` section to the respawn's pack (`Pack::for_checkpoint` takes the missing slices and unproven keys), plus a contract line saying what it means. Then either move `cp-{n}/` to `cp-{n}.sent-back/` before respawning, or keep the first worker's lane. Test: `short(&[1])` followed by a worker whose prompt must name slice 02.
