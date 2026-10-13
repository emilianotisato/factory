# QA guide — PRD-003 Observatory

Generated 2026-10-13 from [`prd.md`](prd.md) § QA plan: the one Human row, **FR-25** (does the `## Numbers` section read well beside the polish prose), and the manual replay on the two stored A/B runs that the FR-21 and FR-25 rows name. Every other row is proven under `cargo test` at the checkpoints; the evidence is in the slice files' Progress. Both steps run on this machine, against the real store, after slice 02 (the replay) and slice 06 (the section). ★ marks the checks that matter most.

Budget: about 15 minutes in all.

## Replay on the stored runs (≈10 min) — FR-21 second half, FR-25

Needs: the `factory` binary from the branch at or after slice 02 (slice 06 for step 5), the store at `~/.local/share/factory/factory.db` with runs `024-20261012-e894` (arca-api, repo `/home/emiliano/Projects/Prokey/wt-b`) and `018-20261011-6490` (icg-cloud), their streams under `~/.local/share/factory/runs/`, and the Claude Code transcripts under `~/.claude/projects/`. If a worktree's transcript dir has been removed since, the recount says `transcript missing, own tokens only` per worker and the target cannot be met; record that instead.

1. Back up the store first. The recount rewrites rows in place. *(1 min)*
   ```
   cp ~/.local/share/factory/factory.db ~/.local/share/factory/factory.db.pre-003
   ```
2. ★ Recount the arca run and read the total. *(2 min)*
   ```
   factory report 024-20261012-e894 --recount
   ```
   **Pass:** `weighted_tokens` within 1% of **18,465,008**; the `recount:` line of `ddbb0927…` shows the polish split, about 1.95M own and 2.29M subagents, `subagents 4`; `cockpit_tokens` unchanged from the pre-recount report (it comes from the cockpit transcript, not the worker rows). **Fail:** a total near the old 16.17M (the transcript dir was not found: check `ls ~/.claude/projects/-home-emiliano-Projects-Prokey-wt-b/`), or any `subagents unreadable` line.
3. ★ Recount the icg run. *(1 min)*
   ```
   factory report 018-20261011-6490 --recount
   ```
   **Pass:** `weighted_tokens` within 1% of **8,131,594**.
4. Read the per-worker lines on the arca run. *(1 min)*
   ```
   factory report 024-20261012-e894 --per-worker
   ```
   **Pass:** every row prints `own`, `sub`, `subagents` and `model_id`; the build rows have `sub 0 subagents 0`; the polish row's `model_id` is a `claude-opus-…` string.
5. ★ Regenerate the arca retro's numbers. *(2 min; needs slice 06)*
   ```
   factory retro 024-20261012-e894
   git -C /home/emiliano/Projects/Prokey/wt-b diff docs/features/
   ```
   **Pass:** the `## Numbers` section's per-role rows match `001-driver/ab/runs.md:223-233` within 1% (build 9 workers about 9.95M, checkpoints 5 about 3.94M, fixes 2 about 0.34M, polish about 1.95M own + 2.29M subagents), the total is 18,465,008, and the comparison block says there is no earlier finished run in this repo. Run it a second time: **Pass:** `unchanged`, and the diff does not grow.

Evidence: paste the two `--recount` outputs into slice 02's Progress and the `retro.md` diff into slice 06's Progress. Then decide whether to keep the recounted store or restore the backup; the PRD's intent is to keep it.

## FR-25 — Does the section read well beside the polish prose? (≈5 min)

Open the `retro.md` that step 5 changed, the arca-api feature's retro, and read it top to bottom once.

1. ★ The judgment stays in the prose and the figures stay in the section. **Pass:** nothing in `## Numbers` argues or recommends; nothing in the prose above restates a figure the section prints better. **Fail:** the two disagree on a number (the prose was written from the under-counted report; note which figure wins and whether the prose needs a correction).
2. The section is readable without the report. **Pass:** the per-role table, the knobs line, the quality line, the pack composition and the comparison each make sense with the column headers alone; dashes mean "not stored for this run", and the knobs and quality lines of this pre-003 run are all dashes, which should read as expected, not as an error.
3. Position. **Pass:** `## Numbers` sits after the `## Cost vs catch` table, at the end, and does not split the polish worker's sections.

Record the answer as one paragraph in slice 06's Progress: keep as is, or what to change in the section's shape before PRD-002 builds on it.
