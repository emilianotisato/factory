---
title: "settled block: include answered asks beside decisions"
state: "open"
added: "2026-11-12"
origin: "PRD-017 polish"
---

`settled::decisions` (src/settled.rs) lists only `Kind::Decision` entries. An Ask entry shares the AD sequence (src/docs/log.rs) and the owner's `answer --ref AD-n` settles it too, yet it never reaches the reviewer's settled block, so a reviewer can re-raise the side the owner rejected. FR-5 said "each decision", so this was left out on purpose for now. Shape: for each Ask with an Answer referring to it, list `- AD-n — title: answered — <answer text>`. About 15 lines plus a unit test beside `a_decision_the_owner_changed_shows_the_change`.
