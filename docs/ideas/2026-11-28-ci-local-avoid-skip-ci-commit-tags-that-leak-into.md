---
title: "ci = local: avoid [skip ci] commit tags that leak into squash-merges on main"
state: "open"
added: "2026-11-28"
origin: "harvest 2026-11-28"
---

In both PR #31 (PRD-016) and PR #32 (PRD-031), `ci = local` stamped `[skip ci]` on `docs(NNN): ship` and `docs(NNN): owner review` commits. GitHub default squash-merge concatenated every PR commit subject into the squash commit body on `main`, suppressing both `ci` and `Edge` workflows on push to `main`.

**Shape:** rethink how `ci = local` suppresses draft-PR CI without leaving `[skip ci]` inside commit messages that leak into squash-merges (e.g., rely on draft-silent PR workflows per `skills/gate/references/ci.md` instead of stamping `[skip ci]` on commits, or make `[skip ci]` opt-in).
