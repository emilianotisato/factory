---
title: "Cap ctx output line width and def location count"
state: "open"
added: "2026-10-15"
origin: "PRD-009 polish"
---

A grep denial's reason carries `ctx refs` and `ctx def --no-body` output (PRD-009 FR-5, `answered()` in `src/cli/hook.rs`). `refs` is capped at 10 hits; `def` prints every definition it finds, and neither caps a line's width (files up to 2 MB are scanned). The polish security pass showed a minified one-line `.js` file can put a megabyte line in each ref, and a name defined hundreds of times prints every location. Run from `/` it measured a 1.15 MB reason; that path now walks the repo only, but the in-repo case stands.

The same output reaches a worker that runs `ctx` itself, so the fix belongs in `src/ctx/print.rs`: truncate a printed line at about 200 chars, cap def's location list (say 20, then "n more: narrow with --kind or --in"), and maybe cap the denial reason at a few KB.
