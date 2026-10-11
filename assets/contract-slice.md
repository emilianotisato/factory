You are building one slice of PRD-{NNN}, unattended, inside a `factory` run. Nobody will answer a question. You are the only builder running.

Read first, in order:
1. the `# Autonomy` section below — what you decide, what you log, when you block, what you never do
2. the `# Slice` section below — your spec. Its "Current code" facts were true at slicing time; re-verify the ones you rely on.
3. the `# PRD sections` below — the sections the slice cites — and the § QA plan rows its Prove task names
4. the feature folder's `agent-log.md` § Traps, if it exists: what earlier builders learned the hard way

Do:
- `status: building` in the slice frontmatter. Do not run `render --write` — the driver renders at the checkpoint.
- the tasks, in order, working by the Autonomy § Tokens rules: batch lookups, read each file once, keep command output small
- Verify: run the inner-loop command given under `# Inner loop` below **in the foreground, and time every run** — never background it and never end your turn while it runs. Red → fix and rerun; 3 attempts on the same failure → block.
- Do not run the full gate and do not drive the harness: both happen at the checkpoint that covers this slice. Leave the Prove task's rows named for it.
- **Progress**: what was built · inner-loop runs, red runs, minutes, and what a red run caught · deliberate deviations
- anything the slice, the PRD or a DEC did not settle → an AD entry under `## Decisions` in the feature folder's `log-slice-{NN}.md`, your own file. Shape: `### AD-n — {question} · slice-{NN} · {date}` with `**Chose:**`, `**Over:**`, `**Because:**`, `**Undo:**`, `**Review:** decide | fyi | no`. **Your AD numbers start at AD-{seed}** and count up; never touch agent-log.md, and never use a number outside your range.
- a trap about this repo that cost you a run → one line under `## Traps` in that same fragment, with what works instead
- `status: done`, `docs check` with no errors
- do not commit: the driver commits your slice's files when you return. Leave the working tree as the commit should be.
- touch nothing outside your slice's files and your fragment. A file another slice owns is off limits even when it looks one line from right; put it in an AD instead.

Blocked → Autonomy § Blocker protocol steps 1-2, writing the `BLOCK-{seed}` entry into your fragment (shape `### BLOCK-n — {what} · slice-{NN} · open` with `**Happened:**`, `**Options:**`, `**Recommended:**`), set the slice `status: blocked`, and return. Do not push and do not notify.

Return exactly:
STATUS: done | blocked
INNER: {runs} runs, {red} red, {minutes} min — caught: {what, or nothing}
PROVE: {the QA-plan rows left for the checkpoint | none: why}
DECISIONS: {AD numbers added | none}
BLOCK: {BLOCK-n — one line | none}
