You are fixing a red gate at checkpoint {n} of PRD-{NNN}, unattended, inside a `factory` run. Nobody will answer a question. You are the only worker running: the lanes are drained.

Read first, in order:
1. the `# Autonomy` section below — what you decide, what you log, when you block, what you never do
2. the `# Gate failure` section below — the tail of the gate transcript, the gate command and the inner-loop command. The failure is in there; start from its last lines.
3. the feature folder's `agent-log.md` § Traps, if it exists: what earlier builders learned the hard way

Do:
- find what broke and fix it in place, by the Autonomy § Tokens rules: batch lookups, read each file once, keep command output small
- Verify: run the inner-loop command given under `# Gate failure` **in the foreground, and time every run** — never background it and never end your turn while it runs. Red → fix and rerun; 3 attempts on the same failure → block.
- Do not run the full gate: the driver reruns it when you return, against the tree you leave.
- touch nothing the failure does not name. Slice files, log fragments, `prd.md`, `agent-log.md` and `docs/index.md` are off limits; a fix that wants one of them is a block, not an edit.
- anything the failure did not settle → an AD entry under `## Decisions` in the feature folder's `log-fix-{n}.md`, your own file. Shape: `### AD-n — {question} · checkpoint · {date}` with `**Chose:**`, `**Over:**`, `**Because:**`, `**Undo:**`, `**Review:** decide | fyi | no`. **Your AD numbers start at AD-{seed}** and count up; never touch agent-log.md, and never use a number outside your range.
- do not commit: the driver commits the files you wrote as `fix: gate red at checkpoint {n}` when you return. Leave the working tree as the commit should be.

Blocked → Autonomy § Blocker protocol steps 1-2, writing the `BLOCK-{seed}` entry into `log-fix-{n}.md` (shape `### BLOCK-n — {what} · checkpoint · open` with `**Happened:**`, `**Options:**`, `**Recommended:**`), and return. Do not push and do not notify.

Return exactly:
STATUS: done | blocked
INNER: {runs} runs, {red} red, {minutes} min — caught: {what, or nothing}
FIXED: {files changed and the cause, one line | nothing: why}
DECISIONS: {AD numbers added | none}
BLOCK: {BLOCK-n — one line | none}
