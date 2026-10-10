## Decisions

### AD-20001 — Which number does the driver's death block on a slice take? · polish · 2026-10-10

**Chose:** an open block already titled "Two workers died on slice NN" if a kill left one written. Otherwise the lowest `BLOCK-NNkk` with `kk` from 02 that no log file of the feature uses (`scheduler::dead_block_id`).
**Over:** the lowest free number below 100, as the checkpoint's blocks take (AD-501); or `kk` from 01.
**Because:** the slice's range keeps the block attributable to the slice when the merge folds it. Starting at 02 keeps the id the driver has always written whenever nothing clashes, and skipping used numbers ends the clash with a builder's own `BLOCK-NN02`.
**Undo:** in `dead_block_id`, start the range at 1, or call `checkpoint::lowest_free_block`.
**Review:** no

### AD-20002 — What orders "since the last answer" for the death cap? · polish · 2026-10-10

**Chose:** `events.jsonl` order. A slice worker counts toward the cap unless its `slice_spawned` comes before the latest `block_answered` naming the slice. A checkpoint's workers are spent by the latest `block_answered` for one of that checkpoint's own `checkpoint_blocked` ids. A worker with no spawn event counts.
**Over:** comparing `spawned_at` with the answer file's `at`, which the checkpoint used. Both are whole seconds, so a respawn in the answer's second compares wrongly.
**Because:** `seq` is a total order (the emit is serialised under a lock, AD-20006), and `factory answer` now logs `block_answered` before the slice flips, so the answer always precedes the respawn it causes.
**Undo:** filter on `spawned_at > answer.at` in `scheduler::deaths_since_answer` and `checkpoint::spawned_before_answer`.
**Review:** no

### AD-20003 — Which answers reset a checkpoint's death cap? · polish · 2026-10-10

**Chose:** only the answers to that checkpoint's own blocks: the three-reds block, the two-deaths block, or the worker's own block, all emitted as `checkpoint_blocked {n}`.
**Over:** any answer naming no slice. That covers a refused merge, a closeout block or another checkpoint's block.
**Because:** an answer to a refused merge says the fragments are fixed. It says nothing about why the checkpoint workers died, so the once-only respawn should not come back with it.
**Undo:** treat `merge_refused` block ids as the checkpoint's own in `checkpoint::since_answer`.
**Review:** no

### AD-20004 — What does a sanitized BLOCK title look like? · polish · 2026-10-10

**Chose:** `answers::block_title`. Whitespace runs, newlines included, collapse to one space, every `·` becomes `-`, and the title is cut at 160 chars ending in `…`. The checkpoint's three-reds and refused-merge titles go through it.
**Over:** escaping the separator, or dropping the gate line from the title.
**Because:** `entries` and `resolve` split the heading on ` · `. A `·` in a gate's last line shifted the phase and `open` fields, so the block no longer parsed as open. A 2000-char compiler line in a heading also helps nobody. The full output stays in the body and the transcript.
**Undo:** use the raw `format!` titles in `checkpoint::run` and `block_on_refusal`.
**Review:** no
