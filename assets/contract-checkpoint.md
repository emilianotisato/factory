You are running checkpoint {n} of PRD-{NNN}, unattended, inside a `factory` run. Nobody will answer a question. You run in a lane the driver reset to the feature tip, and nothing else is live: the gate is green there.

Read first, in order:
1. the `# Autonomy` section below — what you decide, what you log, when you block, what you never do
2. the `# Gate transcript` section below — the green gate run the driver made (or the cached result it reused)
3. the `# Leaks` section below — the leaks report the driver made over what the feature added: each rule's fix line above its hits
4. the `# Covered slices` section below — slices {covers}: each one's Prove task and Progress line
5. the `# QA rows` section below — the § QA plan rows those Prove tasks name, with their evidence column
6. the `# Log` section below, its `## Notes` first: what earlier builders learned the hard way

The feature's `state.toml` and `log.jsonl` are written through the tool only, never by hand. Each command below is one shell line; a body goes on stdin (`--body-file -`).

Do:
1. **Leaks**: fix every `fix` hit under `# Leaks` by its rule's fix line, in the file it names, then run the inner-loop command the gate transcript shows. List each hit under **Fixed** as `leaks: path:line` (a bare path for a path hit), or as unfixed with one line on why: the next checkpoint shows it again. Fixing first means the rows below are proven on the cleaned tree.
2. **Prove**: drive every row under `# QA rows` on the real surface the row names — by the Autonomy § Tokens rules, one row at a time. A `run` row is proven below the UI (the command, the file, the table), never by reading the code that should produce it. The expected result comes from the PRD, the spec or the input file, never from what the command printed. Each row's evidence lives in its own directory, `$FACTORY_EVIDENCE/cp-{n}/<key>/`, where `<key>` is the `key:` the row carries under `# QA rows` (`FR-6`, or `FR-6.2` for the second of several `FR-6` rows); never in a scratchpad and never under `docs/`, and a harness that saves elsewhere copies what it wrote into the row's directory. The driver checks that every row not marked `(deferred: Human? = yes)` left a non-empty directory there, a real directory and not a link, and sends the checkpoint back when one did not; a deferred row needs none. A failing row is a defect: fix it in the files the row names, verify with the inner-loop command the gate transcript shows, and name it under **Fixed** — the driver commits your files with the checkpoint and the next gate re-tests them. A row you could not reach is reported, not skipped silently. Then one mutation check per checkpoint: take the riskiest row you drove, the one whose FR rests on a single line, and break that line by hand, never through git: your work is uncommitted, so a `git checkout`, `restore` or `stash` loses it. Run that row's test and see it red, then restore the line and see the test green again before you record: a line left broken turns the next gate red for a reason nothing explains.
3. **Evidence**: append the evidence files to each covered slice's `**Progress**` line (`{feature dir}/slice-{NN}-*.md`), one line per row, each named `cp-{n}/<key>/<file>`, never an absolute path.
4. **Record**: `{factory} docs log add {NNN} --kind checkpoint --phase checkpoint --slice {NN} --covers {covers csv} --title "after slice-{NN}" --body-file -`, the body holding `**Covers:** slices {covers}`, `**Gate:** {runs} runs, {red} red, {minutes} min — caught: {what, or nothing}` (from the transcript), `**Prove:** {rows}`, which lists each handed row by its key with its evidence (`FR-6.1: cp-{n}/FR-6.1/<file>`), a deferred row as `FR-7: deferred (Human? = yes)`, and reads `0 rows handed` when the pack handed none, `**Fixed:** {what, or none}`, which names any `verify-*` repair, and `**Mutation:** {file:line} broken — {test} red: {how it failed} — restored, green`, which names the line the mutation check broke, the red it saw and the restore.
5. Do not run the full gate or `git commit`: the driver ran the first and does the second when you return, committing everything not gitignored in the lane except `state.toml` and `log.jsonl`, which the tool writes. Leave the lane as the commit should be.
6. Touch nothing a row, a leaks hit or the record does not name. One exception: when a covered slice moved the surface a `.claude/skills/verify-*` recipe drives, repair that recipe, then drive the row and name the repair under **Fixed**. Never drop a step an earlier checkpoint drove. The driver commits the repair with the checkpoint. A repair changes how the recipe drives the app, never what it expects: a drive that fails on the expected result is a defect, fixed as step 2 says. Slice files other than their Progress line, `prd.md`, `state.toml` and `log.jsonl` are off limits.

Blocked → Autonomy § Blocker protocol steps 1-2: `{factory} docs log add {NNN} --kind block --phase checkpoint --title "{what}" --body-file -`, the body holding `**Happened:**`, `**Options:**` and `**Recommended:**`, and return without a checkpoint entry. Do not push and do not notify.

Return exactly:
STATUS: done | blocked
PROVE: {rows driven}/{rows named} — evidence: {cp-{n}/<file>, …} — unreached: {rows, or none}
FIXED: {what, or none}
BLOCK: {BLOCK-n — one line | none}
