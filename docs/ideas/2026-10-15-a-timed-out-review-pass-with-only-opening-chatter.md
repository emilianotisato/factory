---
title: "A timed-out review pass with only opening chatter should not count"
state: "open"
added: "2026-10-15"
origin: "PRD-007 polish"
---

`review::body` (`src/review.rs`) counts a killed pass whenever it left any partial text ("a kill's partial counts: it had the whole timeout"). PRD-007's own polish hit the failure: Cursor pass 2 of 3 (pack.rs, review.rs and three test files, 572 lines) ran the full 900 s and left only "I'll read the review diff…" twice, yet came back `— counted`. Pass 3 (348 lines of tests and the stub) did the same: 900 s, two sentences of intent, counted. Two of three passes were empty and the run still read as reviewed; a fallback agent per pass found 6 and 4 findings in those files. `collect` then writes `reviewed, n of m passes counted`, the pack lifts only the `— counted` heading, and no fallback is owed: those files go unreviewed unless the worker reads the pass body and notices.

Shapes: (a) a third mark, `— partial`, which `pack::collected` lifts and the skill treats as owing the fallback for that pass's files; (b) count a kill's partial only past a length or when it names a `file:line`. (a) keeps the text and needs a skill line, so it waits for a skill bump. Test: the stub `cursor-agent` sleeping past a 1 s timeout after one assistant line.

**Harvest 2026-10-15: the same failure in four runs.** PRD-001: 3 of 9 Cursor passes hit 900 s on 1000+ lines and returned nothing (about 45 min). PRD-003: one 15-minute pass, killed with only two preamble messages. PRD-007: the two passes above. PRD-008: 4 of 7 passes timed out with only opening chatter, and a fallback agent then found real defects in 3 of those diffs in under 4 minutes each (about 100 min in all). Friction from other repos' runs says the same. `pass_lines = 600` already caps a pass; the time is lost on passes that never start reviewing.

Shape (c): stop or split a pass early when it has shown only opening chatter after a few minutes, or stream findings so a kill keeps what it found. Either way, the skill says a pass that timed out owes the fallback for its files.
