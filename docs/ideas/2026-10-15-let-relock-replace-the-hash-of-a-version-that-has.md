---
title: "Let relock replace the hash of a version that has no tag"
state: "open"
added: "2026-10-15"
origin: "harvest 2026-10-15"
---

`skills/bundle.lock` pins one hash per version, and `relock` (`src/skills.rs`) refuses a second hash at the same version. On a feature branch a slice bumps and locks the version, and the tag comes only after the merge. A stale skill line polish finds then cannot be fixed on the branch: bumping again breaks the FR that pinned the version. PRD-005 and PRD-007 parked their fixes, and PRD-008 relocked by hand from main's lock. Releasing on time does not help, because the clash happens before any tag exists.

Fix: `relock` replaces the hash while no `vX.Y.Z` tag exists for that version, locally or on origin, and keeps refusing once one does, since that hash is what users installed.
