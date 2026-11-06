---
title: "Harvest feedback: ten frictions from five delivered features"
state: "open"
added: "2026-11-06"
origin: "factory upstream"
---

Collected while harvesting five features built with `/deliver` on one project (skills 0.12.0, binary 0.13.0). Each section is independent: what happened, what would have helped. Some may already be fixed in 0.13.0.

## 1. Progress has no machine-readable inner-loop line

Slices recorded inner-loop runs as "about 3 min", a list of seconds, or nothing. The retro then adds them up by hand, and one slice's time was simply missing. A required one-line shape in Progress (for example `inner: runs=3 red=1 minutes=7`) would make the retro's totals mechanical.

## 2. A drive-only fix re-runs the whole gate

When a checkpoint's Prove drive fails and the fix touches only the drive (no app code), the checkpoint re-runs the full gate. In one run this was 3 of 11 full-gate runs. Re-running only the drive tier after a drive-only commit would save that time.

## 3. Prove does not notice a QA-plan test name that matches no test

The PRD's QA table named three tests that did not exist. Prove ran the real tests and reported green without flagging the drift; only the polish review caught it. Prove could fail a row whose named test matches no test in the suite.

## 4. Review collect counts a preamble-only pass as reviewed

Four second-opinion passes each timed out after returning only their opening sentence. `review collect` reported them as counted, and `/polish` had to override its own collected verdict to run the fallback. A pass with no findings section should be classed as "fallback owed", not counted.

## 5. Fallback reviewers can collide on the test database

Fallback reviewer agents are told to stay read-only on git, but not to stay off the test database. One ran the suite while the polish worker ran its own, and the shared lane database produced spurious failures. The "one test process per checkout" rule from `/prune` belongs in the fallback reviewer's prompt.

## 6. `docs set NNN reviewed=…` and `docs check` disagree

`docs set NNN reviewed=<date>` printed "state.toml updated" and `docs status --json` showed the date, but no `reviewed` line appeared in `state.toml`, and `docs check` kept warning `W-NOT-REVIEWED`.

## 7. Second-opinion passes are split by size, not by risk

One pass mixed operational scripts with a dozen test files and timed out on the scripts, the part that most needed a cross-vendor read. Splitting so high-risk files (scripts, migrations, auth) get their own pass would protect the reads that matter.

## 8. A tiny remainder pass costs a full pass

`[review] pass_lines` produced a final pass of about 50 lines that still took about 15 minutes of wall time. Folding a remainder under, say, a quarter of `pass_lines` into the previous pass would avoid that.

## 9. `docs check` takes no feature

A feature-scoped `factory docs check NNN` would make a closeout check cheaper to read than the whole-board output.

## 10. The push-guard hook matches prose inside a heredoc

The `PreToolUse` hook that refuses push commands matched the words "Push-to-deploy" inside a heredoc of documentation text, so a plain doc edit had to move into a script file. The hook should match the command, not the heredoc body.

## Also: Prove calling an overlap "cosmetic"

A checkpoint drive saw a fixed banner covering part of the layout and deferred it as cosmetic; the banner was in fact swallowing clicks. Before deferring an overlap, the drive could try a click on what sits under it.
