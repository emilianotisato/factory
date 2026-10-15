---
title: "Fix the ctx hook and trim ctx output instead of forcing ctx"
state: "open"
added: "2026-10-15"
origin: "harvest 2026-10-15"
---

A replay of 14 runs (2026-10-15, [ctx-replay-research.md](../reference/ctx-replay-research.md)) settles the old question of widening the hook's deny: forcing `ctx` would add 17–34% turns for answers no smaller, empty 25% of the time when forced, and no more correct. No defect reached a gate through a lookup miss. ctx is 2.8% of 13,797 lookups, but only about 19% of lookups ask what it answers. Shrinking to outline only keeps its weakest command (a follow-up read 71% of the time). What the data asks for, in order of value:

1. **The hook judges the pattern, not just `-r`** (`src/cli/hook.rs`): deny only identifier-shaped patterns outside `vendor/` and `node_modules/`; let literals, regex and vendor paths through; add `--vendor` to the hint. 62% of the 175 ctx denials blocked literal or regex greps the hint itself says to grep. About −2% turns.
2. **The denial carries the answer:** the hook runs `factory ctx refs|def <word>` (about 40 ms) and prints it in the denial. 52% of denials were followed by exactly that call.
3. **`ctx def` prints a body only for a single match;** for several, signatures and a `--kind`/`--in` hint. Definition output −67%, to 1.2× the grep.
4. **`--in` takes a file** for `refs` and `def`: one-file searches cost 1.9× the grep today.
5. **Index gaps:** PHP class constants, Go grouped `const ( )`, trait impls, Pest `test()`/`it()` in outline, extensionless scripts.
6. **Change the metric** in `factory report`: ctx share per question type (def, refs, outline) plus forced-empty and grepped-anyway rates, in place of the global `ctx_share threshold 10%` line and FR-12's outline-only trigger.

Leave `sed -n` ranges and `cat` alone: only 15% of ranges map to one definition, at higher cost. The bigger byte savings are outside ctx, in log reads through Read and whole-file code reads.
