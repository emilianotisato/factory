---
title: "Checkpoints should record surviving mutants by file:line"
state: "open"
added: "2026-11-05"
origin: "factory upstream"
---

A checkpoint's full gate runs the mutation tier and records only the efficacy number. In one feature, 22 mutants lived at the second checkpoint, all in two slices' tests. The second-opinion reviewer later spent about 20 minutes per pass rediscovering exactly those weak assertions, and PR-mode `prune` re-read every test instead of starting from them.

Should:

1. The checkpoint record lists lived mutants by `file:line` (and the mutated operator).
2. The second opinion and `prune` receive that list as a starting point.
3. Optionally, gate numbers per slice in `state.toml` (runs, red runs, minutes), so the retro's Cost column is filled from disk rather than guessed.
