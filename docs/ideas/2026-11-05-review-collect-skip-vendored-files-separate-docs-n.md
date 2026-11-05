---
title: "review collect: skip vendored files, separate docs, never count an empty pass"
state: "open"
added: "2026-11-05"
origin: "factory upstream"
---

`factory review collect` (the `/polish` § 1 second opinion) splits a branch's diff into passes by line count alone. Across four features in one project this cost hours of reviewer time for no finding:

- **Vendored files get passes of their own.** A branch that adds third-party XML schemas byte for byte (`testdata/**/*.xsd`) or captured vendor manuals as text spent 5 of 25 passes (~50 min) and, on another feature, 6 of 22 passes (~1 h) on them. Every finding was about the vendor's own schema, which the repo must not change.
- **Docs passes return claims nobody verifies.** Passes over operations docs produced findings that could not be checked from the code, mixed into the same triage as code findings.
- **Size by line count misjudges dense files.** One pass got a single ~2,000-line shell script, ran its full 30-minute timeout, and returned nothing.
- **A timed-out pass with no text is counted.** The plan line said "11 of 11 passes counted" while pass 1 reviewed nothing, so the fallback rule for unreviewed code never fired.

Should:

1. Skip vendored paths by default (`**/testdata/**/*.xsd`, and similar), and honour an exclude list such as `[review] exclude = [...]` in `project.toml`.
2. Put docs-only passes in their own lower-priority section, or out by default.
3. Size passes by something closer to complexity than raw lines, or cap a single file's pass.
4. Mark a pass that times out without findings text `unreviewed`, not `counted`, so the fallback rule applies.

Seen again in PRD-013 (harvest 2026-11-05): pass 1 got `docs/features/013-live-status/log.jsonl` and `state.toml`, files only the binary writes, and spent most of its time on docs. Tool-written files (`log.jsonl`, `state.toml`, `events.jsonl`) belong in the default exclude.
