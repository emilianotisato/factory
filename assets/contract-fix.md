You are fixing a red gate at checkpoint {n} of PRD-{NNN}, unattended, inside a `factory` run. Nobody will answer a question. You are the only worker running: the lanes are drained.

Read first, in order:
1. the `# Autonomy` section below — what you decide, what you log, when you block, what you never do
2. the `# Gate failure` section below — the tail of the gate transcript, the gate command and the inner-loop command. The failure is in there; start from its last lines.
3. the `# Log` section below, its `## Notes` first: what earlier builders learned the hard way

The feature's `state.toml` and `log.jsonl` are written through the tool only, never by hand. Each command below is one shell line; a body goes on stdin (`--body-file -`).

Do:
- find what broke and fix it in place, by the Autonomy § Tokens rules: batch lookups, read each file once, keep command output small
- Verify: run the inner-loop command given under `# Gate failure` **in the foreground, and time every run** — never background it and never end your turn while it runs. Red → fix and rerun; 3 attempts on the same failure → block.
- Do not run the full gate: the driver reruns it when you return, against the tree you leave.
- touch nothing the failure does not name. Slice files, `prd.md`, `state.toml` and `log.jsonl` are off limits (the last two take the tool's writes only); a fix that wants one of them is a block, not an edit.
- anything the failure did not settle → a decision in the log: `{factory} docs log add {NNN} --kind decision --slice {slice} --flag decide|fyi --title "{question}" --body-file -`, where `{slice}` is the slice the fix lands in, the body holding the `**Chose:**`, `**Over:**`, `**Because:**` and `**Undo:**` lines. `--flag` is the Autonomy `Review:` value: `decide` or `fyi`, left out for `no`.
- do not commit: the driver commits the files you wrote as `fix({NNN}): gate red at checkpoint {n}` when you return. Leave the working tree as the commit should be. A file you made with a command is yours only if you `git add` it by name or by its folder.

Blocked → Autonomy § Blocker protocol steps 1-2: `{factory} docs log add {NNN} --kind block --phase checkpoint --title "{what}" --body-file -`, the body holding `**Happened:**`, `**Options:**` and `**Recommended:**`, and return. Do not push and do not notify.

Return exactly:
STATUS: done | blocked
INNER: {runs} runs, {red} red, {minutes} min — caught: {what, or nothing}
FIXED: {files changed and the cause, one line | nothing: why}
DECISIONS: {AD numbers added | none}
BLOCK: {BLOCK-n — one line | none}
