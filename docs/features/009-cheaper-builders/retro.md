# Retro — PRD-009

Written by `/deliver` in the closeout commit, from the agent's side of the run.

**The point is cost next to catch.** An agent asked only about friction votes to cut every check, because every check costs it time. A verdict with an empty Caught column is a preference, not evidence.

## Cost vs catch

| Step | Cost | Caught | Verdict |
|---|---|---|---|
| Inner loop, per slice | 4 worker runs, none red, plus 2 green polish runs | nothing red; write-mode fmt meant no fmt red at all | keep |
| Checkpoints (full gate) | 2 runs, 3.3 min, none red | nothing | keep: cheap at this size, and the only place `fmt --check` runs |
| Prove, at checkpoints | 2 checkpoint workers | nothing. The FR-3 transcript proved the PRD's own cases, all green, while ten guard bypasses sat beside them | tune: a guard's Prove should also run what main denied |
| Polish — local review | one forked `code-review` | the high: the hook judged an inline script without the words around it (argv, env, stdin, `$S`, flag clusters, a second `-e`), so lines main denied now ran. Also the docs-check BLOCK that lands nothing, the walk from `/`, the unreadable root, stale comments | keep |
| Polish — security pass | one subagent, about 5.5 min | the same bypass, confirmed by building main and the branch and feeding both the same lines; the unbounded walk measured (39 s, 1.15 MB reason); no size cap on the reason | keep: the measured repro turned a claim into a fix with no doubt |
| Polish — UX pass | skipped: no screen, no verify harness | — | — |
| Polish — second opinion (grok-4.7-high) | 3 passes, 900 + 900 + 755 s | passes 1 and 2 timed out with no finding (assets, docs, hook, ctx, pack unreviewed). Pass 3 found the BLOCK recovery that lands nothing, on its own, agreeing with the local review | tune: two of three passes spent 15 min each and returned nothing |
| prune | 3 scoped runs | nothing to cut: the 12 new tests each pin a different acceptance criterion | keep |

## Where I guessed

- **FR-3's wording against its intent.** `prd.md` § FR-3 says a script is Forbidden "only when the script both makes a process call and mentions gh or push". Read literally, the argument or variable beside the script cannot count. I tightened it to judge every word from the flag on, and let a process call with no mention fall to the line's mention check. Every FR-3 acceptance case still passes. Logged as AD-6, `decide`.
- **Where Code-review fixes go.** `/polish` § 8 says the slice's `**Code-review fixes ({date}).**` section, near a Progress section. These slices have no Progress section, since Progress lives in the log now, so I appended the section at the end of each slice file.
- **The process-call list's gaps** (`__import__`, `Deno.Command`, `system "…"` with no parentheses). FR-3 accepted "a call off the list", so I parked them as an idea rather than widen the list.

## What I wished existed

- A hook regression corpus: every line main's hook denies, run against the branch's hook in one test. Each of the ten bypasses was a line main already denied.
- The second opinion returning partial findings when it times out. Two passes spent 30 minutes in total and left only their opening sentence.

## Project friction → `/gate audit`

None. The gate ran 2 times with none red in 3.3 min, and the write-mode inner loop removed the fmt reds this PRD was about.

## Framework friction → factory

- **`/prd` § acceptance, for a change that loosens a guard.** FR-3's acceptance listed what must now pass and three lines that must still be denied. It never said "everything main denied that this FR does not mean to allow stays denied". The builder, the checkpoint and its Prove all went green over ten bypasses. A guard-loosening FR should name that corpus.
- **`factory review second`, `pass_lines` and the timeout.** Passes of about 500 lines hit the 900 s timeout twice with grok-4.7-high. Either the passes are too large for that model or the timeout is too short. A timed-out pass should keep whatever findings it streamed.
- **The driver's slice commit subject.** `feat(009): slice-05 ctx answers: def trims and the denial's answer` is 66 chars, over the owner's 50-char rule, because the subject is built from the slice title.
- **The installed hook during polish.** The lane runs the installed binary's hook, so my own `grep -rn` for a literal was denied by the old rule this branch replaces. The log's Notes warn about it, but it still cost a turn.

Leaks: nothing the leaks check should have caught.

## Score

4 — every slice landed green with no red gate, and the run delivered what it promised; polish then found a real guard bypass that the PRD, the builder and the checkpoints had all missed.
