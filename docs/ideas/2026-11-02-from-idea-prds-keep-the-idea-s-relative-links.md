---
title: "From-idea PRDs keep the idea's relative links"
state: "open"
added: "2026-11-02"
origin: "owner"
retrigger: "the next factory docs new --from-idea on an idea that links other files"
---

`factory docs new <slug> --from-idea <id>` pastes the idea's body into the new PRD
unchanged. The idea lived in `docs/ideas/`, but the PRD lives in
`docs/features/{NNN}-{slug}/`, one folder deeper. So every relative link in the body
breaks: `../reference/x.md` now points at `docs/features/reference/x.md`.

The owner hit this on 8 PRDs in a downstream project and fixed every link by hand.

The copy happens in `with_idea()` in `src/docs/scaffold.rs`, which writes the body under
`## From idea {id}` as-is. Rewriting each relative markdown link target from the idea's
folder to the PRD's folder at that point would fix it. Absolute URLs, `#anchors` and
links inside code fences should stay as they are.

`factory docs check` could also flag broken relative links in PRDs, so the next case is
caught even if the rewrite misses one.
