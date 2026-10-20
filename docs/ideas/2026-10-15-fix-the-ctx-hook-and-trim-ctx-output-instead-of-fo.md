---
title: "Fix the ctx hook and trim ctx output instead of forcing ctx"
state: "open"
added: "2026-10-15"
origin: "harvest 2026-10-15"
---

A replay of 14 runs (2026-10-15, [ctx-replay-research.md](../reference/ctx-replay-research.md)) settles the old question of widening the hook's deny: forcing `ctx` would add 17–34% turns for answers no smaller, empty 25% of the time when forced, and no more correct. No defect reached a gate through a lookup miss. ctx is 2.8% of 13,797 lookups, but only about 19% of lookups ask what it answers. Shrinking to outline only keeps its weakest command (a follow-up read 71% of the time). What the data asks for, in order of value:

Items 1–4 (the hook judges the pattern, the denial carries the answer, `def` bodies for a single match, `--in` takes a file) went to [PRD-009](../archive/009-cheaper-builders/prd.md) FR-4 to FR-6 on 2026-10-15. What is left:

5. **Index gaps:** PHP class constants, Go grouped `const ( )`, trait impls, Pest `test()`/`it()` in outline, extensionless scripts.
6. **Change the metric** in `factory report`: ctx share per question type (def, refs, outline) plus forced-empty and grepped-anyway rates, in place of the global `ctx_share threshold 10%` line and FR-12's outline-only trigger.

Leave `sed -n` ranges and `cat` alone: only 15% of ranges map to one definition, at higher cost. The bigger byte savings are outside ctx, in log reads through Read and whole-file code reads.
