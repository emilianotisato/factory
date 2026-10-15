---
title: "W-HOME-PATH and the archive rewrite miss a home with no trailing slash"
state: "open"
added: "2026-10-15"
origin: "PRD-008 polish"
---

`find_home_path` and `home_paths_to_tilde` (src/paths.rs) match `/home/<name>/` and `/Users/<name>/` only, as FR-3 and FR-5 of PRD-008 specified, and a unit test pins that `/home/x and` is no hit. A home at the end of a word slips through both: `docs/features/002-docs-port/prd.md:287` quotes a `find` over the owner's home dir with a space after the name, so `docs check` never warns, `harvest --done` keeps it, and once archived the line is frozen.

The cross-vendor review in PRD-008 polish found it. The qa-guide for the first harvest now has the owner grep for these by hand before `--done`.

Fix: also match `/(home|Users)/<name>` followed by a byte that is neither `/` nor a name byte, or the end of the text, and rewrite it to `~`. Touches `W-HOME-PATH`, the archive rewrite and the upstream scrub at once; flip the `/home/x and` assertion in `paths.rs` tests.
