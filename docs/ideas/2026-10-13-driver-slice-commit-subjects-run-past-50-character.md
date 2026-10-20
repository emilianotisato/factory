---
title: "Driver slice commit subjects run past 50 characters"
state: "open"
added: "2026-10-13"
---

`factory` writes `feat(NNN): slice-NN <title> (FR-..)`; on PRD-003, `bb933ab` is 63 characters. Trim the title or drop the FR list when the subject would pass 50. Found by the PRD-003 polish review, 2026-10-13.

**Harvest 2026-10-20: PRD-009 too.** `feat(009): slice-05 ctx answers: def trims and the denial's answer` is 66 characters: the subject is built from the slice title.
