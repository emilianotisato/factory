---
title: "A guard-loosening FR replays everything main denied"
state: "open"
added: "2026-10-20"
origin: "harvest 2026-10-20"
---

## What happened

PRD-009 FR-3 loosened the hook so a script is judged by what it runs, not by the words it holds. FR-3's acceptance listed the lines that must now pass and three lines that must still be denied. The builder, the checkpoint and its Prove all went green. Polish then found ten bypasses: lines main's hook denied that the branch's hook let through (argv, env, stdin, `$S`, flag clusters, a second `-e`). Every one was a line main already denied, and nothing ran them.

## What it should do

- **A hook regression corpus in the tests.** Every line the current hook denies, kept as a fixture, run against the hook in one test. A change that lets one through fails unless the corpus entry is moved to an "allowed on purpose" list in the same diff.
- **A `/prd` acceptance line for any FR that loosens a guard** (a hook, a deny rule, a validator): "everything the old guard denied that this FR does not mean to allow stays denied", with the corpus as its proof. `/slice` carries it into the Prove task.
