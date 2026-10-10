# QA guide — PRD-001 Driver

Generated 2026-10-09 from the Human rows of [`prd.md`](prd.md) § QA plan: **FR-7** (cockpit from the phone) and **FR-11** (the A/B against `/deliver`). Every other FR is proven under `cargo test` at the checkpoints; look in the slice files' Progress for that evidence. Both checks here run **after the branch ships**, on real projects, and their results go into `retro.md`.

Budget: FR-7 about 40 minutes of attention spread over a run; FR-11 two working days of wall-clock, mostly unattended, about 3 hours of your attention. ★ marks the checks that matter most.

## FR-7 — Cockpit: start on the laptop, answer from the phone (≈40 min attention)

Needs: `factory` installed, the cockpit skill installed (`skills/factory-deliver/SKILL.md` into `~/.claude/skills/factory-deliver/`), a real project on the v2 framework with a sliced PRD, Remote Control paired with the phone.

1. On the laptop, in Claude Code: `/factory-deliver NNN`. Note the run id it prints. *(1 min)*
2. ★ Lock the laptop. Leave it for at least 30 minutes. The run must keep going: on the phone, open the same session over Remote Control and read what `factory wait` relayed. **Pass:** events arrive while the laptop is locked. **Fail:** nothing arrives, or the session says the Bash call ended early. If it failed, note whether the skill was on the long wait or the 25-minute poll (slice 10 recorded which). *(30 min, idle)*
3. ★ Wait for a block, or force one: `factory answer` is only testable against a real `BLOCK-n`. If the run reaches the PR without blocking, mark this step "no block in this run" and plan a second run on a PRD with a known open question. On the phone, answer the `AskUserQuestion` the cockpit relays. **Pass:** `factory status <run>` on the laptop later shows the slice back in `building` and the log holds `**Resolution (date)**`. *(5 min)*
4. Finish the review interview on the phone when the run reports `finished`: the decide lines, four questions a round, as `/deliver` §8 does it. **Pass:** the interview completes; overturned ADs appear in `agent-log.md`; the PR URL is shown and the PR is still a draft. *(10–20 min)*
5. Kill the cockpit on purpose at least once during the run (close the session) and start a new session with `/factory-deliver NNN` or `factory wait <run>`. **Pass:** the events you missed are replayed. *(3 min)*

Evidence: screenshots of steps 2, 3 and 4 on the phone, pasted or linked in `retro.md` under FR-7.

## FR-11 — A/B Prove: `/deliver` against `factory`, two projects (≈2 days wall-clock, ≈3 h attention)

Order is fixed by the PRD: **icg-cloud first, then arca-api; within each project, baseline A first, then B.** Never run A and B at the same time (shared quota, shared machine).

Per project:

1. Write and slice one PRD once, with `/prd` and `/prd-to-tasks`, on the project's default branch. Commit it. *(your usual time)*
2. Two worktrees from the same base commit, neutral names: `git worktree add ../wt-a` and `../wt-b`. Each gets a feature branch. *(5 min)*
3. ★ **Run A, baseline:** in `wt-a`, `/clear`, then `/deliver NNN` as today. Let it finish to a draft PR. Save the cockpit session id. *(unattended; hours)*
4. ★ **Run B:** in `wt-b`, `/clear`, then `/factory-deliver NNN`. Let it finish to a draft PR. Save the run id and the cockpit session id. *(unattended; hours)*
5. ★ **Answer the questionnaire for each run before looking at any number.** Open the PR and the agent-log, then answer these eight, once per run, and write the answers into `retro.md` first:
   1. Does the result do what the PRD asked? (yes / partly / no)
   2. Would you merge this PR after your normal review, with at most small fixes? (yes / no)
   3. Decisions in the agent-log: how many would you overturn? (count)
   4. Code quality against the project's own standard (1–5).
   5. Tests: do they prove the FRs or only pass? (prove / pass / neither)
   6. Anything built that the PRD did not ask for? (list)
   7. Anything the PRD asked for that is missing? (list)
   8. Overall: ship as is / ship after fixes / redo (one).
   Questionnaire pass: B scores at least as well as A on 1, 2 and 8, lists no more items than A on 6 and 7, and is no worse by more than one point on 4.
6. Numbers, same ruler for both. Run A: `measure.py --retro` over the deliver session and its subagents. Run B: `factory report <run> --cockpit <cockpit session.jsonl>`. Both weighted with the `MAINTAINING.md` weights (cache read 0.1, output 5, input and cache write 1). Wall-clock as build minutes from first spawn to draft PR. *(15 min)*
   - **FR-8 cross-check, same step:** run `measure.py --retro` on run B's cockpit session too and compare its `out_tokens`, `cache_read_tokens`, `start_context`, `peak_context` with the `--cockpit` figures `factory report` printed. **Pass:** within 2%. This is the FR-8 QA row; it needs a real cockpit transcript, which is why it lives here.
7. ★ Pass criteria per project: `weighted_tokens(B) ≤ 0.6 × weighted_tokens(A)`; `cockpit_tokens(B) ≤ 5% of weighted_tokens(B)`; run B's draft PR has CI green; the questionnaire pass from step 5. **One project failing is a `BLOCK` for you to judge, not a reason to rerun quietly.**
8. Write both comparison tables (tokens by kind, build minutes, gate runs and reds, CI cycles, cockpit tokens, `ctx_share`, `repeat_rate`, `followup_reads`) and the eight answers for each run into this feature's `retro.md`. Then the FR-12 reversal rule reads them: `followup_reads` over 30% means widen `ctx` output; `ctx_share` under 10% or `repeat_rate` not below 26.7% means `ctx` shrinks to `outline` only.

Evidence: the two tables and the sixteen answers in `retro.md`; the two PR URLs.

## Not in this guide

FR-1 to FR-6, FR-8 (except the 2% cross-check above), FR-9, FR-10 and FR-12 are machine rows, proven by the checkpoint on the fixture; their transcripts are in the slice files' Progress paragraphs.
