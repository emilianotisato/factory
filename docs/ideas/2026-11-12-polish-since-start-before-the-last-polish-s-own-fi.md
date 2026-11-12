---
title: "polish --since: start before the last polish's own fixes, and ignore merged main"
state: "open"
added: "2026-11-12"
origin: "PRD-017 polish"
---

`settled::last_reviewed` (src/settled.rs) returns R, the newest commit that changed `reviewed` in the feature's state.toml, and a later collect reviews `R..HEAD` (`Range::Since`, src/review.rs). Four of the PRD-017 polish reviewers (security pass, three fallback passes) raised the same gaps:

- **The previous polish's own fixes are never second-opinion reviewed.** The polish worker sets `reviewed`; its fixes land in the same commit or an earlier one (`land::commit_pick`, src/land.rs; the ship commit, src/closeout.rs). Either way they sit at or before R, so `R..HEAD` leaves them out, and round 1's collect ran before they existed. The local review still sees the whole branch.
- **Merging main after R puts main's changes in the since passes.** `R..HEAD` is a tree diff, so a branch that merges or rebases onto main after R sends every file main changed to the reviewer, under "(since the last review)". If R is gone after a rewrite, numstat fails and the collect fails.
- **Two polishes on one day** leave the `reviewed` line unchanged, so a third round starts from the first. It only over-reviews.

FR-6 fixed the anchor as "the commit that changed `reviewed`", so changing it is the owner's call. Options: anchor at the head the previous collect reviewed (the collected file's `<!-- head: … -->` line, recorded on the board); use `R^`; or intersect the since paths with `<base>...HEAD` and diff from `merge-base`. Recommended: record the collected head in state.toml beside `reviewed` and diff from it, intersected with the branch's own paths.
