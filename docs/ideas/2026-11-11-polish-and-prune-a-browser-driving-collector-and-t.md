---
title: "/polish and /prune: a browser-driving collector and test runs contend for one checkout"
state: "open"
added: "2026-11-11"
origin: "factory upstream"
---

**What happens.** Polish runs its collectors, then fixes. The UX collector drives the real app for about 15 minutes on the lane's database. Meanwhile any test run in the same checkout races it: the collector reports "the working tree changed under me", its screenshots predate the fixes, and tests can hit missing tables. Prune separately insists on one test process per checkout. The two skills are compatible only if you read both side by side. In one run the session polled `pgrep` in a loop to learn when the collector released the lane, and the pattern matched its own command line twice before it was bracketed.

**What it should do.**
- Polish §1 should say: start the UX collector first, and run no tests in that checkout until it returns. Or give factory a lane lock or a "subagent released the lane" wait primitive.
- Prune's PR mode reruns scoped files three times. It could exempt browser files already run green in the same polish, since about 3 × 70 s per browser file set on a flaky host buys little.

**Two smaller polish notes.**
- Polish has no way to tell whether a formatter call is in the project's declared format scope. A vendored `npx prettier --write` on an undeclared directory reformatted hundreds of lines before it was noticed.
- The UX row costs a full browser capture even for a copy-only diff. Its finds there came from reading the code beside the copy, so a "copy accuracy vs code" check would be cheaper for copy-only changes.
