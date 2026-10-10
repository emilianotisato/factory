# Autonomy — the contract for unattended runs

`/deliver` builds an approved PRD all the way to a green pull request with nobody watching. Every skill it calls was written for a session with the owner at the keyboard, so each place one of them would stop and ask carries a line marked **`Unattended (/deliver):`** that points here. **This file is the only place the rule lives.**

Read it when you are inside a `/deliver` run — the orchestrator, a slice subagent, or a skill it invoked. An attended run never reads it and never behaves differently because of it.

> **Changing a rule?** Change it here, never by adding a variant inside one skill. The reasoning behind each call is in [MAINTAINING.md](MAINTAINING.md).

## Who decides what

**Sub-skills classify; `/deliver` applies the policy.** A skill that hits a blocker records it and hands it back. It never decides on its own whether the run stops — that is the run's mode, and the mode lives only in `/deliver`.

Today there is one mode: **a blocker stops the run and notifies the owner.**

## The test — decide, log, or block

Ask it of every choice the work forces that nobody wrote down: **does the PRD, the slice, or a `DEC-n` already settle this?**

| Answer | Do | Examples |
|---|---|---|
| **Yes** | decide, no log | the pattern the slice names; the status code the FR states; the test an acceptance criterion describes |
| **No — and the choice is local and cheap to undo** | decide **and** write an `AD-n` | 404 vs 403 for a hidden row; extracting a helper; a test's shape; a retry count; a dev dependency |
| **No — and it changes what the PRD promises** (an FR's scope or acceptance, a non-goal, a settled `OPEN-n` or `DEC-n`), **or it cannot be undone or reaches outside the repo** (a destructive migration, an external contract, a paid service), **or it needs a secret**, **or a cap below is hit** | **block** | the PRD says soft-delete and a unique index makes it impossible; CI needs a secret this branch cannot add |

**Unsure between log and block → block.** A wrong `AD` costs the owner a revert. A silent scope change costs a feature.

**Every `AD` carries a `Review:` line with one of three values.** `decide` — the owner must answer before this ships: a deviation from what the slice or an acceptance criterion said, a schema or public interface shape the PRD did not specify, a new runtime dependency. `fyi` — nothing waits on it, but a reviewer reading the diff would miss it: a new dev dependency, a lint or type suppression, a behaviour chosen where the slice was silent. `no` — the rest. The PR body lists `decide` first and `fyi` after. A run that flags forty `fyi` entries has buried the two that mattered; two real runs marked 22 of 46 `decide` and 40 of 70 `fyi`. A helper extracted, a test's shape, a 404 over a 403 the slice hinted at are all visible in the diff, so they are `no`. When unsure between `no` and `fyi`, choose `no`.

## Mapping — the buckets skills already use

| Where | Attended | Unattended |
|---|---|---|
| `/polish` **Fix** | fix without asking | same |
| `/polish`, `/ux-review` **Ask** | one AskUserQuestion round | the test above: an `AD` when local and reversible — taste always lands here — otherwise **block** |
| `/polish`, `/ux-review` **Park** | an `ideas.md` line or a new slice | an `ideas.md` line only; a new slice changes the plan the owner approved |
| `test-cleanup`, "genuinely in doubt" | ask in-session | **keep the test** and write an `AD` — keeping never loses coverage |
| `/verify-app` confirm round | one AskUserQuestion round | take what the code shows; an `AD` for each finding it had to choose between |
| a `DEC-n` candidate | recommend, owner chooses | list it in the PR body; **never write `docs/decisions.md`** |
| a proposed rubric line | owner approves it | list it in the PR body; **never write `docs/ux-rubric.md`** |

## What an unattended run may write

**May:** application code and tests · the feature's own folder — `prd.md` Status prose and frontmatter status, slices, `agent-log.md` (or, when you were handed a `log-*.md` fragment, that file instead: siblings are running), `retro.md`, `qa-guide.md` · this feature's row and Notes cell in `docs/index.md` · the `inner:` key in `docs/index.md` frontmatter, at preflight, only when none is declared and only in the shape `/deliver` § 0 gives it, with an `AD` marked `fyi` · lines in `docs/ideas.md` · `.claude/skills/verify-*` whenever the app's surface moves under it: a harness slice, a feature-map entry, or a later slice that changes what the harness drives. Extend or repair a recipe, but never drop a step an earlier checkpoint drove.

**Never:** `docs/decisions.md` · `docs/ux-rubric.md` · CI workflow files · `CLAUDE.md`, `AGENTS.md`, `.ai/guidelines/` · another feature's folder · anything under `docs/archive/` · the PRD's FRs, acceptance criteria, non-goals or decisions log. A change the run believes one of these needs is a **block**, or a proposal in the PR body when nothing waits on it.

**The PRD outranks this list for a file it names.** An FR that orders a write here, such as installing a tool that rewrites `CLAUDE.md`, is done as written, with an `AD` marked `decide`. The owner approved that PRD; this list guards against what nobody approved.

## Never, to get green

An agent alone under pressure reaches for every one of these. All are forbidden. **The retro is the legitimate place to say a rule is too strict** — that is half of why it exists.

- lower a floor or a threshold, or add an ignore, exclusion, baseline or allowlist entry to a gate tool
- `--no-verify`, or any other way around a hook
- skip, delete or weaken a test this run did not write
- edit a CI workflow
- record a checkpoint, or a Prove row, without an artifact that exists on disk
- force-push, push to the default branch, merge, close the PR, delete a branch. In a `factory` run a worker runs no `git push` and no `gh` at all: the driver pushes and talks to GitHub, and the hook refuses both

**Two narrow exceptions, each with an `AD` marked `Review: fyi`:** an inline suppression where the real fix is impossible (a third-party type, a generated file), naming why; and extending gate configuration to *cover* new code the feature adds, such as a new source path. Extending, never loosening. When that extension makes CI need a toolchain it lacks, adding the setup step to the workflow is part of the same extension, with the `AD` marked `decide`. Never remove, skip or weaken a CI step.

## Tokens — every call re-reads everything before it

Two thirds of a run's cost is the model re-reading its own context on every call. At 300k of context, a `grep | head` costs about what reading a novel does. The call count is what you control:

- **Batch independent lookups into one message.** Several Read, Grep or Bash calls side by side are one model call. One after another, they are N calls.
- **Read a file once with Read**, whole when it fits and in a few large ranges when it does not, unless your pack already holds it: then read only the range you need, because a whole read of a file in your pack is refused and pointed at its section. Paging it through five `sed -n` calls pays for your context five times. Do not re-read a file only to confirm an edit landed; Edit fails loudly when it misses. The pre-commit re-read a skill requires is a different thing: keep it.
- **Keep command output to what you act on**: each failing test's own output (the assertion and its diff) and the summary line, not the passing transcript. A log you may need later goes to a file in your scratchpad, outside the repository, searched with `grep`.
- **Your map is the slice's Current code section plus `git log --stat` of the slices before yours**, which shows what moved since slicing. Re-verify the facts you rely on; do not re-explore the codebase to rebuild the map.

## Caps

Starting values. The retro records the numbers that will say whether they are right.

- **Gate:** per slice, the inner loop (`docs gate --inner`); the full gate at a checkpoint. 3 fix attempts on the same failure, then block. A red checkpoint is fixed in place; when the cause is unclear, bisect over the slice commits first.
- **Slice hand-back:** a subagent that reports done with a gap — no commit, inner loop not green, `docs check` errors — is sent back once, then blocked. A checkpoint that returns without evidence for a row it was asked to prove, same rule.
- **CI:** 3 fix rounds, then block.
- **Flaky:** a CI failure unrelated to the diff is re-run once (`gh run rerun <id> --failed`; in a `factory` run the driver does it, never a worker). Passes → name the job in the PR body's CI section, not in `agent-log.md`: a docs-only commit would start CI all over again. Fails again → block.
- **Not this branch's:** CI red on something the branch did not touch, or an infrastructure fault (a missing secret, runner or quota) → block at once.

## Blocker protocol

1. **Write a `BLOCK-n` entry**, shape in [templates/agent-log.md](templates/agent-log.md): what happened, with the `file:line` or command output that proves it; 2-4 options, each with its cost; the recommendation. It goes in the feature's `agent-log.md` — unless you were given a `log-*.md` fragment of your own, which means siblings are running and a direct append would lose somebody's work. Then it goes in your fragment, under `## Blockers`, and the orchestrator merges before it pushes.
2. **A slice's blocker → set that slice `status: blocked`**, then `docs render --write` — skip the render if you were given a fragment; whoever merges it also renders. A blocker outside a slice names its phase in the heading instead: `preflight`, `slicing`, `polish`, `pr`, `ci`.
3. **Commit it** — `docs(NNN): block <where>` — and push the branch (`git push -u origin feat/{slug}`), so the entry is readable away from this machine. In a `factory` run the driver commits and pushes; a worker stops after step 2.
4. **Notify** with the PushNotification tool. One line, under 200 characters, leading with what to act on: `PRD-013 blocked at slice-02: unique index prevents soft-delete. 3 options in agent-log BLOCK-1`.
5. **Stop.** The final message says the same and gives the entry's path. Start nothing else.

## The owner answers mid-run

An unattended run can have the owner in the chat anyway. Nothing waits for them. An answer that arrives is applied like a blocker's resolution: as the owner's correction, dated and cited, from the next step on. A question you want answered goes in the chat once, with your recommendation, and you continue on that recommendation until they reply.

## Resuming

The owner answers in the conversation or by editing the entry. Then:

1. Append `**Resolution ({date}):**` with the answer and change the heading's last field from `open` to `resolved`.
2. **If the answer changes the PRD, apply it as the owner's correction** — inline and visible, dated, citing `BLOCK-n`. This is the one time an unattended run edits FRs or non-goals, and only with the owner's words.
3. A blocked slice goes back to `building`; render; commit.
4. `/deliver NNN` again. It reads its position from `docs status NNN --json`, so nothing depends on the conversation that started the run.
