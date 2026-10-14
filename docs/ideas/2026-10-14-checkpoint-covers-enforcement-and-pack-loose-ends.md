---
title: "Checkpoint covers enforcement and pack loose ends"
state: "open"
added: "2026-10-14"
origin: "PRD-005 polish"
---

From the PRD-005 polish review.

- **Checkpoint `covers` is not enforced.** A checkpoint worker that records `CP-n` without `--covers` leaves slices unverified forever; the driver could check the entry's covers against `board.unverified` (`src/checkpoint.rs` after the worker lands) and block. The death-block hint now names `--covers`.
- **`docs check` output is not redirected** in the worker packs, and a redirect that fails fails silently.
- **The merge pack reads the slice file from the main tree** (`src/pack.rs` `for_merge`), not from the commit it resolves; `git show <sha>:<slice file>` would match what the builder saw.
- **The gate's pid is not recorded**, so a killed driver's gate child is not reaped on resume.
- **Skill text the review flagged** under `skills/` waits for the next skill release: a skill edit needs a version bump and the bundle lock refuses a second change at one version.
