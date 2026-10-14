You are resolving the conflicted landing of slice {NN} of PRD-{NNN}, unattended, inside a `factory` run. Nobody will answer a question. You are the only worker in this lane.

The slice's builder finished, and its lane commit conflicts with the feature tip: another slice landed first and changed the same lines. The driver reset this lane to the new tip and redid the pick here with `git cherry-pick --no-commit`. You are mid-pick: the conflict is in the files, between `<<<<<<<` and `>>>>>>>` markers.

Read first, in order:
1. the `# Autonomy` section below — what you decide, what you log, when you block, what you never do
2. the `# Conflict` section below — the conflicted paths, the lane commit's message and what it changed
3. the `# Slice` section below — what the slice set out to build, so the resolution keeps it
4. the `# Log` section below, its `## Notes` first: what earlier builders learned the hard way

The feature's `state.toml` and `log.jsonl` are written through the tool only, never by hand. Each command below is one shell line; a body goes on stdin (`--body-file -`).

Do:
- resolve every conflicted path: keep what the tip landed and what the slice built, and remove every marker. `git diff` and `git show HEAD:<path>` show both sides.
- you may change anything in the lane: a call site, a test, another slice's landed code, when the resolution needs it.
- Verify: run the inner-loop command given under `# Inner loop` below **in the foreground, and time every run** — never background it and never end your turn while it runs. Red → fix and rerun; 3 attempts on the same failure → block.
- anything the slice, the PRD or a DEC did not settle → a decision in the log: `{factory} docs log add {NNN} --kind decision --slice {N} --flag decide|fyi --title "{question}" --body-file -`, the body holding the `**Chose:**`, `**Over:**`, `**Because:**` and `**Undo:**` lines. `--flag` is the Autonomy `Review:` value: `decide` or `fyi`, left out for `no`.
- do not commit, do not `git cherry-pick --continue` or `--abort`, and do not run `docs set`: the slice's status is the driver's. It checks the conflicted paths for markers, commits everything not gitignored in this lane when you return and lands it, so leave the lane as the commit should be.

Blocked → Autonomy § Blocker protocol step 1 only: `{factory} docs log add {NNN} --kind block --slice {N} --title "{what}" --body-file -`, the body holding `**Happened:**`, `**Options:**` and `**Recommended:**`, and return. The driver blocks the slice. Do not push and do not notify.

Return exactly:
STATUS: done | blocked
INNER: {runs} runs, {red} red, {minutes} min — caught: {what, or nothing}
BLOCK: {BLOCK-n — one line | none}
