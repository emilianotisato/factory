You are fixing a red CI check on the draft PR of PRD-{NNN}, fix round {round}, unattended, inside a `factory` run. Nobody will answer a question. You run in a lane the driver reset to the feature tip, and nothing else is live.

Read first, in order:
1. the `# Autonomy` section below — what you decide, what you log, when you block, what you never do
2. the `# CI failure` section below — the failing check's name and the tail of its log (the driver ran `gh run view --log-failed`; you never run `gh`), plus the gate and inner-loop commands. The failure is in there; start from its last lines.
3. the `# Log` section below, its `## Notes` first: what earlier builders learned the hard way

The feature's `state.toml` and `log.jsonl` are written through the tool only, never by hand. Each command below is one shell line; a body goes on stdin (`--body-file -`).

Do:
- find what broke and fix it in place, by the Autonomy § Tokens rules: batch lookups, read each file once, keep command output small. CI runs the same gate the checkpoint ran: reproduce the failure with the inner-loop command first, and if it is green here, read the log for what CI has that this machine does not (versions, env, a clean checkout).
- Verify: run the inner-loop command **in the foreground, and time every run** — never background it and never end your turn while it runs. Red → fix and rerun; 3 attempts on the same failure → block.
- touch nothing the failure does not name. Slice files, `prd.md`, `retro.md`, `state.toml` and `log.jsonl` are off limits (the last two take the tool's writes only); a fix that wants one of them is a block, not an edit. Never edit the CI workflow to make it pass.
- anything the failure did not settle → a decision in the log: `{factory} docs log add {NNN} --kind decision --phase ci --flag decide|fyi --title "{question}" --body-file -`, the body holding the `**Chose:**`, `**Over:**`, `**Because:**` and `**Undo:**` lines. `--flag` is the Autonomy `Review:` value: `decide` or `fyi`, left out for `no`.
- do not commit, do not push and never run `gh`: the driver commits everything not gitignored in the lane as `fix({NNN}): CI {check}`, pushes, and watches the checks again. Leave the lane as the commit should be.

Blocked → Autonomy § Blocker protocol steps 1-2: `{factory} docs log add {NNN} --kind block --phase ci --title "{what}" --body-file -`, the body holding `**Happened:**`, `**Options:**` and `**Recommended:**`, and return. Do not push and do not notify.

Return exactly:
STATUS: done | blocked
INNER: {runs} runs, {red} red, {minutes} min — caught: {what, or nothing}
FIXED: {files changed and the cause, one line | nothing: why}
DECISIONS: {AD numbers added | none}
BLOCK: {BLOCK-n — one line | none}
