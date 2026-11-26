# Retro — PRD-020

Written by `/deliver` in the closeout commit, from the agent's side of the run.

**The point is cost next to catch.** An agent asked only about friction votes to cut every check, because every check costs it time. A verdict with an empty Caught column is a preference, not evidence.

## Cost vs catch

| Step | Cost | Caught | Verdict |
|---|---|---|---|
| Inner loop, per slice | — | — | keep |
| Checkpoints (full gate) | — | — | keep |
| Prove, at checkpoints | — | — | keep |
| Polish — local review | — | scheduler `Dirty` dropped protected-file warnings; `default_memory_max` unit bug | keep |
| Polish — security pass | — | cache trim via canonicalize + `remove_dir_all` outside lane | keep |
| Polish — UX pass | skipped — CLI-only | nothing | cut |
| Polish — second opinion (claude:opus) | — | memcap cap math, systemctl `.scope`, reap/OOM false positives | keep |
| prune | — | nothing (no redundant tests removed; only test tightening) | tune |

## Where I guessed

- Whether to treat `main/target/lane-N` as live when lane env uses relative `target/lane-N`: slice-02 acceptance and §11 decisions say keep; second opinion argued they are always stale here. Parked as an idea rather than reversing a documented acceptance test in polish.
- `reset-failed` before reusing a gate systemd unit name: slice silent; assumed checkpoint reap + `.scope` stop is enough for this PR and parked fuller unit lifecycle work.

## What I wished existed

- A single helper tested once for “systemd scope unit string” so second-opinion passes would not each re-find the bare-name `.service` mismatch.
- Collect output indexed by severity with file:line anchors pre-verified, to shrink the polish triage read of seven passes.

## Project friction → `/gate audit`

Nothing in this branch argues for lowering an existing gate tier. The memcap default-size bug slipped through because the unit test duplicated the wrong formula instead of calling `default_memory_max()` — a small mutation or property test on the formatted string would catch that class cheaply.

## Framework friction → factory

- `/polish` §1: second-opinion volume on a 3k-line diff is high-signal but expensive to triage; a `--since-reviewed` polish round on fixes-only would help large features.
- Leaks check: clean verdict; no missed plan-number comments in this pass.

## Score

4/5 — checkpoints and mutation catches were strong; polish still had to fix a shipping memcap math bug the gate green-lit because the test mirrored the mistake.