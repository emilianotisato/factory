---
title: "One base-ref resolver for pack, closeout and leaks"
state: "open"
added: "2026-10-14"
origin: "PRD-006 polish"
---

Three callers each work out "the base branch" their own way: `pack.rs` (around line 459), `closeout.rs` (around line 485, `default_branch`) and the `changed_lines` helper. PRD-006 added `leaks::scan(Input::Base(..))`, called from the checkpoint driver through `closeout::default_branch`. None handles a repo whose default branch exists only as `origin/<name>` with no local branch: the scan errors and the checkpoint pack says `not run`.

Shape: one `base_ref(repo) -> Result<String>` that tries the local default branch, then `origin/<default>`, then `merge-base` against it, used by all four. Found in the PRD-006 polish review; pre-existing, so not fixed in that PR.
