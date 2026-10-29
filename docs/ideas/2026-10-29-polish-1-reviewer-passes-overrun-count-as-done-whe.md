---
title: "polish §1: reviewer passes overrun, count as done when they are not, and re-review settled code"
state: "open"
added: "2026-10-29"
origin: "factory upstream"
---

Three retros from one project's last four features report the same family of problems with the reviewers `polish` §1 runs.

**1. Second opinion: a flat per-pass timeout, and a timed-out pass still counts.** Passes are sized by changed lines, but the timeout is a flat 900 s. In one run, 6 of 12 passes timed out with partial output and no findings, including the passes over the newest middleware and a public endpoint; the plan line still read "12 of 12 passes counted". In another, 2 of 6 passes timed out and the last 2 never ran, because the whole background run hit the harness's time limit, so `factory review second` never returned an exit code to read. Only the log showed it. Meanwhile a same-model fallback did each missing pass in about 5 minutes.

What should happen:
- A pass that timed out does not count as reviewed, and the plan line says so.
- The fallback starts the moment a pass times out, not after the run ends.
- `factory review second` takes a total wall-clock budget and says up front which passes will not fit, instead of being killed mid-pass.
- Either the timeout scales with the pass's size, or the pass cap shrinks to fit the timeout.

**2. A later polish re-reviews the whole branch.** A third polish on one long feature re-reviewed all 187 files in 28 passes (about 7.5 h). Most of its claims on earlier slices repeated decisions an earlier polish had already settled or parked. This is the second retro to ask: a later polish should default to the commits after the last `reviewed` date, and review the full diff only on request.

**3. Reviewer briefs.**
- The UX pass ran 27 minutes and still left three states of one page uncaptured, named only in its last message. Its brief should list the states it must capture up front, and ask it to report which it could not reach.
- The fallback's prompt has to list everything already fixed or settled by the PRD, or it reports them again. A template for that brief would save writing it per pass.
- Running the UX pass first, in its own lane database, would let targeted tests run alongside it instead of waiting for it (the skill forbids a test run while a subagent drives the app).
