---
title: "slice/deliver: three rules for Prove rows and the last checkpoint"
state: "open"
added: "2026-11-20"
origin: "factory upstream"
---

**1. The last checkpoint re-runs every browser proof the feature wrote.** A later slice put a session re-check in front of every acting user. Two earlier proofs went red and stayed red until polish, because the last checkpoint ran only its own new drive. A per-feature list of proofs, run in full at the last checkpoint, catches that.

**2. Each form a slice adds gets one empty submit and one narrow-screen drive.** Happy-path drives missed the browser's own `required` bubbles blocking the app's Spanish errors, and a primary button off screen at 375 px. Both showed up only in the UX pass.

**3. A Prove row that picks between options seeds a state where the options give different results.** One drive could not tell two choices apart because both produced the same number on the seed, so the proof passed with the wiring swapped.
