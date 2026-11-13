---
title: "Rebase reference-style link definitions in from-idea PRDs"
state: "open"
added: "2026-11-13"
origin: "PRD-015 polish"
---

`docs new --from-idea` rebases inline links and images (`migrate::rebase_links`, `src/docs/migrate.rs`), but not reference-style definitions such as `[x]: ../reference/x.md`. `edit_links` and `check`'s `link_at` only match `](`. So a definition copied from `docs/ideas/` into `docs/features/NNN-*/` points one folder too shallow. `docs check` does not flag it either, since it doesn't resolve definitions.

Fixing it means teaching both `edit_links` and `check`'s href scan the `[label]: target` line form, outside fences. Found by the PRD-015 local review. No idea seen so far uses one.
