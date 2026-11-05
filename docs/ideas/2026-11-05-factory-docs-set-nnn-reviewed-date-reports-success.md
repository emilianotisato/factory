---
title: "`factory docs set NNN reviewed=<date>` reports success but writes nothing"
state: "open"
added: "2026-11-05"
origin: "factory upstream"
---

`factory docs set 011 reviewed=2026-11-05` printed "state.toml updated", but no `reviewed` key appeared in the lane's `state.toml`, and `factory docs check` kept warning `W-NOT-REVIEWED`. Either the key goes to the driver's store while the check reads the lane, or the set is a no-op.

Related: `/polish` § 8 says to "set `reviewed: {date}` in the PRD's frontmatter", while the contract says `factory docs set NNN reviewed=…`. A run followed the contract and left the frontmatter alone. The two should name one place.

Should: `docs set reviewed=` writes where `docs check` reads (or refuses loudly), and § 8 points at the same mechanism.
