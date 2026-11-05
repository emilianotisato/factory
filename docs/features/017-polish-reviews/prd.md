---
number: 017
slug: polish-reviews
title: "Polish reviews that don't waste passes"
# type: feature | bugfix | enhancement | refactor
type: feature
created: 2026-11-05
author: Emiliano Tisato
summary: |
  The second opinion reviews the code that matters with what is settled in hand: passes skip vendored and tool-written files, a timed-out pass counts as unreviewed, a later round reviews only what changed, `factory stop` stops reviewers, and the UX pass gets its own checkout. Draft from the 2026-11-05 idea triage, not interviewed.
---

# PRD-017 — Polish reviews that don't waste passes

> **Draft from the 2026-11-05 idea triage.** The owner chose what is in and out, group by group; nothing here is interviewed or verified yet. `/prd` continues this draft: interview, recon, FRs, QA plan. `file:line` facts are from `main` at `ca128f7`; re-verify them.

## 1. Why now

Across one project's last four features, about a third of second-opinion findings re-argued logged ADs; vendored files took 5 of 25 passes (~50 min) and 6 of 22 (~1 h) for no finding; a pass that timed out was counted, so the fallback never fired. Three retros and four upstream ideas name the same family.

## 2. Candidate FRs

Kept by the owner at the triage. `/prd` turns each into an FR or cuts it.

- **C-1 — Reviewers know what is settled.** The decisions log, slice text, non-goals and answered OPENs go with every pass (`src/review.rs` and `src/collect.rs` send none today) and into a fallback brief template (`skills/polish/SKILL.md:70`); triage's settled rule widens past OPEN-n and `decisions.md` (`:89`). Autonomy rows for an AD that overturns an AD, and for a safe contract-changing Ask (`assets/autonomy.md:17-23`).
- **C-2 — Honest pass accounting.** A kill with partial text counts today (`src/review.rs:516-520`): a timeout becomes unreviewed, the fallback starts on it (`skills/polish/SKILL.md:63,70`), the timeout scales with pass size (flat 1800 s, `src/config.rs:71`), and the budget is printed up front.
- **C-3 — A pass planner.** Vendored paths and tool-written files (`log.jsonl`, `state.toml`, `events.jsonl`) are skipped by default, with `[review] exclude` in `project.toml` (`numstat(root, base, &[])`, `src/collect.rs:45`); docs-only passes go in their own section; one huge file is capped (`src/review.rs:477`).
- **C-4 — A later round reviews since the last reviewed commit**, not `base...HEAD` again.
- **C-5 — `factory stop` kills a running collect's reviewers** (`src/review.rs:224-259` polls with no cancel; `src/closeout.rs:203-205`).
- **C-6 — The UX pass gets its own checkout** and lane database so the fixer cannot move under it (`skills/polish/SKILL.md:47`), starts from checkpoint evidence, and gets the state list, slices and ADs up front (`skills/ux/SKILL.md:41-50`).

## 3. Not in scope (decided 2026-11-05)

- **Polish path patterns as a reviewer floor** — dropped: no polish on record skipped a pass it needed.
- **Split the polish worker into driver steps** — dropped: `review collect` took the mechanical half.
- **Parallel polish fixers, each in its own lane** — stays an idea.

## 4. Source ideas

The full text of each idea this draft absorbed; the idea files are deleted.

### Polish treats only OPEN-n and decisions as settled, so reviewers re-argue slice text and earlier ADs

_added 2026-10-19, origin: factory upstream_

##### What happened

Across three shipped features, `/polish` reviewers and the `/ux` collector raised findings that the feature's own docs had already settled:

- **Slice text.** A UX pass asked about two strings that a slice's tasks wrote out verbatim. `/polish` § 2 counts only `OPEN-n` and `DEC-n` as settled, so the worker had no rule for it. It treated the slice text as settled anyway and wrote no AD.
- **Earlier ADs.** Five of seven UX asks re-argued choices that the PRD, a slice or an earlier AD had pinned. The UX collector does not read the slice files or the AD list, while `/polish` § 0 hands both to triage.
- **PRD non-goals.** A second-opinion pass re-raised a race the PRD's non-goals section had already accepted.

Two cases have no row in `autonomy.md` at all:

1. **A later phase overturning an earlier AD.** A polish finding asked for exactly what an implementation AD had rejected (a 500 for a malformed id). `autonomy.md` covers an AD against the PRD or a DEC, not an AD against an earlier phase's AD. The worker wrote a new fyi AD citing the old one and left the old one standing.
2. **An Ask whose fix changes the contract while today's behaviour is safe to ship.** Refusing a disabled record needed a new error reason in the public code table. Under the autonomy test that is a block, but nothing waited on it. The worker parked it as an idea. The Mapping table has no row for this.

##### What it should do

- Treat slice task text, earlier ADs and the PRD's non-goals as settled, beside `OPEN-n` and `DEC-n`. A finding against one of them is rebutted with a citation, not re-litigated.
- Give the `/ux` collector the same context `/polish` § 0 gives triage: the slice contracts and the AD list.
- Add an `autonomy.md` row for a later AD that overturns an earlier one: allowed, cite the overturned AD, and mark the old one superseded.
- Add a Mapping row for a contract-changing Ask when current behaviour is safe: park it as an idea for the owner, and do not block the run.

### /polish second opinion: send the feature's decisions log with each pass

_added 2026-11-05, origin: factory upstream_

`/polish` § 1 sends the second-opinion reviewer the diff without the feature's decisions log. In one project, about a third of the reviewer's findings across several features re-argued choices already logged as `AD` entries, and each one cost the polisher a verification at `file:line` to rebut. § 2 already names this as the most common false-positive class.

Should: pass `factory log show --decisions` (or the decisions section of the plan) with each pass, with an instruction that a finding contradicting a logged decision must name the decision and say why it is wrong.

### friction: cody-agent PRD-026

_added 2026-10-15, origin: cody-agent PRD-026 run 026-20261015-da46_

- `polish` § 1, second opinion: the skill says to read the exit code of `factory review second`, but a run longer than the harness's background limit never returns one. Passes 5 and 6 were simply never reviewed, and only reading the log showed it. The step needs a per-pass budget, or a rule to launch the fallback the moment a pass times out instead of waiting for the run.
- `polish` § 1: the fallback prompt has to list everything already fixed or settled by the PRD, or the fallback re-reports it. A template for that brief would save rewriting it per pass.

A comment or name the leaks check should have caught: none found.

From cody-agent `docs/features/026-product-pulse/retro.md`

### polish §1: reviewer passes overrun, count as done when they are not, and re-review settled code

_added 2026-10-29, origin: factory upstream_

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

### review collect: skip vendored files, separate docs, never count an empty pass

_added 2026-11-05, origin: factory upstream_

`factory review collect` (the `/polish` § 1 second opinion) splits a branch's diff into passes by line count alone. Across four features in one project this cost hours of reviewer time for no finding:

- **Vendored files get passes of their own.** A branch that adds third-party XML schemas byte for byte (`testdata/**/*.xsd`) or captured vendor manuals as text spent 5 of 25 passes (~50 min) and, on another feature, 6 of 22 passes (~1 h) on them. Every finding was about the vendor's own schema, which the repo must not change.
- **Docs passes return claims nobody verifies.** Passes over operations docs produced findings that could not be checked from the code, mixed into the same triage as code findings.
- **Size by line count misjudges dense files.** One pass got a single ~2,000-line shell script, ran its full 30-minute timeout, and returned nothing.
- **A timed-out pass with no text is counted.** The plan line said "11 of 11 passes counted" while pass 1 reviewed nothing, so the fallback rule for unreviewed code never fired.

Should:

1. Skip vendored paths by default (`**/testdata/**/*.xsd`, and similar), and honour an exclude list such as `[review] exclude = [...]` in `project.toml`.
2. Put docs-only passes in their own lower-priority section, or out by default.
3. Size passes by something closer to complexity than raw lines, or cap a single file's pass.
4. Mark a pass that times out without findings text `unreviewed`, not `counted`, so the fallback rule applies.

Seen again in PRD-013 (harvest 2026-11-05): pass 1 got `docs/features/013-live-status/log.jsonl` and `state.toml`, files only the binary writes, and spent most of its time on docs. Tool-written files (`log.jsonl`, `state.toml`, `events.jsonl`) belong in the default exclude.

### factory stop should kill a polish collect's reviewers

_added 2026-10-15, origin: PRD-007 polish_

The driver runs `collect::collect` in-process before the polish worker spawns (`collected` in `src/closeout.rs`). PRD-007's polish added a stop check after it, so no worker spawns after a `factory stop`, but the collect itself keeps going: its `cursor-agent` passes are not workers, `factory stop` does not see them, and they run until `[review] timeout_secs` (900 s) ends them.

Shape: `run_passes` (`src/review.rs`) takes a cancel check (the run's `stop_requested`, reloaded) and its `run_logged` poll loop kills the pass's group when it fires, the pass reading `unreviewed (stopped)`. Cost: a callback through `collect`, `review::run_passes` and `run_logged`. Worth it once a stop during a long collect is seen in a real run.

### /polish UX pass: no shared checkout with the fixer, and start from checkpoint evidence

_added 2026-11-05, origin: factory upstream_

Two problems with the UX collector in `/polish` § 1:

- **It drives the app from the same checkout the fixer edits.** The skill forbids reviewers from moving HEAD but says nothing about the fixer editing files a reviewer is building from. In one run the UX collector rebuilt the stack mid-pass and reported the polisher's own in-progress edits as "someone else's changes".
- **It re-captures states the checkpoints already captured.** The checkpoints' Prove tasks held screenshots of every state. Telling the UX pass to start from the evidence directory and drive only the missing states saved most of its time (~8 min instead of much longer); the skill does not say to.

Should: give the UX collector its own worktree (or make § 5 fixes wait until it finishes), and tell it to start from the checkpoint evidence directory and drive only what is missing.
