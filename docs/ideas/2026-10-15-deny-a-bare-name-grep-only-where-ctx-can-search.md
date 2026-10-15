---
title: "Deny a bare-name grep only where ctx can search"
state: "open"
added: "2026-10-15"
origin: "PRD-009 polish"
---

After PRD-009, the hook denies a recursive grep for a bare identifier over any repo dir outside `VENDOR_DIRS` (`bare_name`, `vendored` in `src/cli/hook.rs`). ctx searches less than that: `walk()` (`src/ctx/walk.rs`) also skips `ROOT_DIRS` (storage, dist, build at the root), gitignored paths, and every file outside its five `Lang` types (`.md`, `.toml`, `.yaml`, `.py`, `.kt` and logs are not searched).

So `grep -rn checkpoint_every docs` or `grep -rn ERROR storage/logs` is denied, ctx finds nothing, and the reason is the hint alone ("a bare name goes to ctx, grep anything else"), which sends the worker to ctx for a word only grep can find. The worker gets out with `grep -rn 'checkpoint_every\b'` or a non-recursive grep, a turn later. On main every recursive grep over the repo was denied, so this is no regression.

FR-5 settled "no hit from either command leaves the hint alone". Options:
1. Deny only where ctx can search: share walk's skip rule and its `Lang` filter with the hook (a `docs/` path or `--include=*.md` passes).
2. Let a denial with no ctx hit become an allow, since nothing better was offered.

Recommended: 1, it keeps the denial a pointer to an answer. Count in the store's runs how many denials after PRD-009 carried the hint alone.
