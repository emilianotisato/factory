---
title: "friction: icg-cloud PRD-019"
state: "open"
added: "2026-10-15"
origin: "icg-cloud PRD-019 run 019-20261015-48af"
---

- **`polish` §1, the second opinion**: the passes run serially at ~12 min each with a 900 s timeout. A feature-sized diff (17 passes) has no budget, and there is no rule for when slow counts as failed. Run the passes in parallel, or give the step a wall-clock cap that hands the remaining passes to the fallback.
- **`factory docs log`**: `log add` in a lane writes the main checkout's log, while `log show` in the lane reads the lane's copy. That is how AD-14 duplicated AD-13, and an append-only log cannot take it back.
- **Leaks check**: it missed a plan number inside a string. `tests/Browser/GuideTest.php:217` had `guideShot($page, 'fr123-7-guide-staff-new-slot', narrow: true);`, a screenshot name carrying `fr123-7`. The local review caught it, and it is now `guide-staff-new-slot`.

From ~/Projects/ICG/icg-cloud/docs/features/019-arca-setup-papercuts/retro.md
