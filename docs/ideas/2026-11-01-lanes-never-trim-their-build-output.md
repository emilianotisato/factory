---
title: "Lanes never trim their build output"
state: "open"
added: "2026-11-01"
origin: "owner"
retrigger: "a lane worktree passes 20 GB, or a second Rust repo runs lanes"
---

Lane worktrees keep their build output forever. On 2026-11-01 this repo's four
lanes held 148 GB of `target/` (20-55 GB each) plus 89 GB of stale `target/lane-*`
folders in the main checkout from before lanes moved to worktrees. Nothing in
factory trims either.

This repo now caps it per repo: each `docs/reference/lanes/lane-N.md` `setup:` wipes
`$CARGO_TARGET_DIR` at run start past 15 GB, and `[profile.dev] debug =
"line-tables-only"` halves a fresh build (7.3 GB to 3.2 GB). Every other Rust repo
with lanes still grows unbounded, and other stacks have their own caches.

Options for factory itself:
- `factory lanes clean`: report and trim build output across a repo's lanes, plus
  stale lane dirs in the main checkout.
- A size cap factory checks at run start, declared under `[lanes]`, instead of a
  hand-written shell line per lane file.
- Use `cargo sweep --time N` when installed: it drops only artifacts unused for N
  days, so a trim keeps the warm dependencies a full wipe throws away.

Rejected: one shared `CARGO_TARGET_DIR` across lanes or projects. Cargo's build
lock serializes every build that shares it, which undoes parallel lanes, and
same-named binaries from different checkouts overwrite each other.
