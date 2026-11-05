---
title: "Prove guidance: check against the source's rule, and prove the negative"
state: "open"
added: "2026-11-05"
origin: "factory upstream"
---

Two Prove-task weaknesses let defects through green checkpoints:

- **Expected output taken from the command itself.** A checkpoint's Prove row recorded a command's printed summary as the expected result. The input specification said, in its own words, which documents that report must contain; the output held a different set. All three checkpoints passed, and only polish caught it.
- **Specs that pass without the feature.** Two browser specs for a "submit once" behaviour would have passed with the feature's script removed: nothing held the request long enough for a second click to matter. No checkpoint asked whether the spec could fail.

Should: `/slice`'s Prove guidance (and the Prove task template) says (1) state the expected result from the spec, PRD or input file, never from what the command printed, and (2) for each behavioural spec, name how it would fail if the feature were removed, and check that at least once.
