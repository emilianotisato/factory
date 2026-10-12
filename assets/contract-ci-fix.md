You are fixing a red CI check on the draft PR of PRD-{NNN}, fix round {round}, unattended, inside a `factory` run. Nobody will answer a question. You are the only worker running.

Read first, in order:
1. the `# Autonomy` section below — what you decide, what you log, when you block, what you never do
2. the `# CI failure` section below — the failing check's name and the tail of its log (the driver ran `gh run view --log-failed`; you never run `gh`), plus the gate and inner-loop commands. The failure is in there; start from its last lines.
3. the feature folder's `agent-log.md` § Traps, if it exists: what earlier builders learned the hard way

Do:
- find what broke and fix it in place, by the Autonomy § Tokens rules: batch lookups, read each file once, keep command output small. CI runs the same gate the checkpoint ran: reproduce the failure with the inner-loop command first, and if it is green here, read the log for what CI has that this machine does not (versions, env, a clean checkout).
- Verify: run the inner-loop command **in the foreground, and time every run** — never background it and never end your turn while it runs. Red → fix and rerun; 3 attempts on the same failure → block.
- touch nothing the failure does not name. Slice files, log fragments, `prd.md`, `agent-log.md`, `retro.md` and `docs/index.md` are off limits; a fix that wants one of them is a block, not an edit. Never edit the CI workflow to make it pass.
- anything the failure did not settle → an AD entry under `## Decisions` in the feature folder's `log-ci-{round}.md`, your own file. Shape: `### AD-n — {question} · ci · {date}` with `**Chose:**`, `**Over:**`, `**Because:**`, `**Undo:**`, `**Review:** decide | fyi | no`. **Your AD numbers start at AD-{seed}** and count up.
- do not commit, do not push and never run `gh`: the driver commits the files you wrote as `fix({NNN}): CI {check}`, pushes, and watches the checks again. Leave the working tree as the commit should be.

Blocked → Autonomy § Blocker protocol steps 1-2, writing the `BLOCK-{seed}` entry into `log-ci-{round}.md` (shape `### BLOCK-n — {what} · ci · open` with `**Happened:**`, `**Options:**`, `**Recommended:**`), and return. Do not push and do not notify.

Return exactly:
STATUS: done | blocked
INNER: {runs} runs, {red} red, {minutes} min — caught: {what, or nothing}
FIXED: {files changed and the cause, one line | nothing: why}
DECISIONS: {AD numbers added | none}
BLOCK: {BLOCK-n — one line | none}
