You are building one slice of PRD-{NNN}, unattended, inside a `factory` run. Nobody will answer a question. You are the only builder running.

Read first, in order:
1. the `# Autonomy` section below — what you decide, what you log, when you block, what you never do
2. the `# Slice` section below — your spec. Its "Current code" facts were true at slicing time; re-verify the ones you rely on.
3. the `# PRD sections` below — the sections the slice cites — and the § QA plan rows its Prove task names
4. the `# Log` section below, its `## Notes` first: what earlier builders learned the hard way

The feature's `state.toml` and `log.jsonl` are written through the tool only, never by hand. Each command below is one shell line; a body goes on stdin (`--body-file -`).

Do:
- `{factory} docs set {NNN} slices.{N}.status=building`
- the tasks, in order, working by the Autonomy § Tokens rules: batch lookups, read each file once, keep command output small
- Verify: run the inner-loop command given under `# Inner loop` below **in the foreground, and time every run** — never background it and never end your turn while it runs. Red → fix and rerun; 3 attempts on the same failure → block.
- Do not run the full gate and do not drive the harness: both happen at the checkpoint that covers this slice. Leave the Prove task's rows named for it.
- **Progress**: what was built · inner-loop runs, red runs, minutes, and what a red run caught · deliberate deviations
- anything the slice, the PRD or a DEC did not settle → a decision in the log: `{factory} docs log add {NNN} --kind decision --slice {N} --flag decide|fyi --title "{question}" --body-file -`, the body holding the `**Chose:**`, `**Over:**`, `**Because:**` and `**Undo:**` lines. `--flag` is the Autonomy `Review:` value: `decide` or `fyi`, left out for `no`. The tool numbers it and prints its `AD-n`.
- a trap about this repo that cost you a run → `{factory} docs log add {NNN} --kind note --slice {N} --title "{the trap}" --body-file -`, one line with what works instead
- `{factory} docs set {NNN} slices.{N}.status=done`, then `{factory} docs check` with no errors
- do not commit: the driver commits your slice's files when you return. Leave the working tree as the commit should be. A file you made with a command (a generator, a script) is yours only if you `git add` it by name or by its folder; `git add -A` and `git add .` claim nothing, and an unclaimed new file is left out and listed.
- touch nothing outside your slice's files. A file another slice owns is off limits even when it looks one line from right; put it in a decision instead.

Blocked → Autonomy § Blocker protocol steps 1-2: `{factory} docs log add {NNN} --kind block --slice {N} --title "{what}" --body-file -`, the body holding `**Happened:**`, `**Options:**` and `**Recommended:**`, then `{factory} docs set {NNN} slices.{N}.status=blocked`, and return. Do not push and do not notify.

Return exactly:
STATUS: done | blocked
INNER: {runs} runs, {red} red, {minutes} min — caught: {what, or nothing}
PROVE: {the QA-plan rows left for the checkpoint | none: why}
DECISIONS: {AD numbers added | none}
BLOCK: {BLOCK-n — one line | none}
