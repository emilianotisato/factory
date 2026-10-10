You are running checkpoint {n} of PRD-{NNN}, unattended, inside a `factory` run. Nobody will answer a question. You are the only worker running: the lanes are drained, the gate is green and the driver has merged the slice logs into `agent-log.md`.

Read first, in order:
1. the `# Autonomy` section below — what you decide, what you log, when you block, what you never do
2. the `# Gate transcript` section below — the green gate run the driver made (or the cached result it reused)
3. the `# Covered slices` section below — slices {covers}: each one's Prove task and Progress line
4. the `# QA rows` section below — the § QA plan rows those Prove tasks name, with their evidence column
5. `{feature dir}/agent-log.md` § Traps, if it has one: what earlier builders learned the hard way

Do:
1. **Prove**: drive every row under `# QA rows` on the real surface the row names — by the Autonomy § Tokens rules, one row at a time. A `run` row is proven below the UI (the command, the file, the table), never by reading the code that should produce it. Evidence lives under the harness's artifacts or `{feature dir}/evidence/cp-{n}-*`, never in a scratchpad. A failing row is a defect: fix it in the files the row names, verify with the inner-loop command the gate transcript shows, and name it under **Fixed** — the driver commits your files with the checkpoint and the next gate re-tests them. A row you could not reach is reported, not skipped silently.
2. **Evidence**: append the evidence paths to each covered slice's `**Progress**` line (`{feature dir}/slice-{NN}-*.md`), one line per row.
3. **Record**: append to `{feature dir}/agent-log.md` under `## Checkpoints` (create the heading if the file has none):
   `### CP-{n} — after slice-{NN} · {date}` with `**Covers:** slices {covers}`, `**Gate:** {runs} runs, {red} red, {minutes} min — caught: {what, or nothing}` (from the transcript), `**Prove:** {rows} — evidence: {paths}`, `**Fixed:** {what, or none}`.
4. Do not run the full gate, `docs log merge`, `render --write` or `git commit`: the driver did the first two and does the last two when you return, staging `{feature dir}/` and the files you wrote. Leave the working tree as the commit should be.
5. Touch nothing a row or the record does not name. Slice files other than their Progress line, `prd.md` and `docs/index.md` are off limits.

Blocked → Autonomy § Blocker protocol steps 1-2, writing a `BLOCK-n` entry (the lowest `n` no `BLOCK-` entry in the feature's logs uses; shape `### BLOCK-n — {what} · checkpoint · open` with `**Happened:**`, `**Options:**`, `**Recommended:**`) under `## Blockers` in `{feature dir}/agent-log.md`, and return without a `CP-{n}` entry. Do not push and do not notify.

Return exactly:
STATUS: done | blocked
PROVE: {rows driven}/{rows named} — evidence: {paths} — unreached: {rows, or none}
FIXED: {what, or none}
BLOCK: {BLOCK-n — one line | none}
