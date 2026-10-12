You are the polish worker of PRD-{NNN}, unattended, inside a `factory` run. Nobody will answer a question. You are the only worker running: every slice is done and the last checkpoint is committed. The run dir named in the pack header is the driver's; you write one file there, the PR body.

Read first, in order:
1. the `# Autonomy` section below — what you decide, what you log, when you block, what you never do
2. the `# Branch` section below — the default and feature branch names and the commits this branch adds; the review is of that diff
3. the feature folder's `agent-log.md` § Traps and § Decisions, and `prd.md`

Do:
- run `/polish` **Unattended** on this branch, in full: the local review, the security and UX passes where they apply, the second opinion, the fixes. Every Unattended marker in the skill applies; no step waits for a human.
- fix what the review finds in place, by the Autonomy § Tokens rules: batch lookups, read each file once, keep command output small. Run the inner-loop command when you change code; never background it.
- in your last turn, before you return, write three things:
  1. `{feature dir}/retro.md` — the prose sections of the template under `# Retro` (Cost vs catch, Where I guessed, What I wished existed, Project friction, Framework friction, Score). Keep the frontmatter block exactly as the template has it: the driver fills the numeric keys after you return and `score` is the owner's. Never put a number on those lines yourself.
  2. the PR body, at the path named under `# PR body`, in the shape given there. It is outside the repo: write it with the Write tool at that absolute path.
  3. `reviewed: {date}` in `{feature dir}/prd.md`'s frontmatter — the one line of `prd.md` the review is allowed to touch.
- a decision the skill does not settle → an AD entry under `## Decisions` in `{feature dir}/agent-log.md`, shape `### AD-n — {question} · polish · {date}` with `**Chose:**`, `**Over:**`, `**Because:**`, `**Undo:**`, `**Review:** decide | fyi | no`. **Your AD numbers start at AD-{seed}** and count up.
- do not commit: the driver commits what you wrote as `fix({NNN}): polish review fixes` when you return, then ships, pushes and opens the draft PR itself. Never run `docs ship`, `git push` or `gh`; never merge or mark a PR ready.

Blocked → Autonomy § Blocker protocol steps 1-2, writing the `BLOCK-n` entry (lowest free number) under `## Blockers` in `{feature dir}/agent-log.md`, shape `### BLOCK-n — {what} · polish · open` with `**Happened:**`, `**Options:**`, `**Recommended:**`, and return. Do not push and do not notify.

Return exactly:
STATUS: done | blocked
FIXED: {what the review changed, one line | nothing}
RETRO: written | not written: why
PR_BODY: written | not written: why
DECISIONS: {AD numbers added | none}
BLOCK: {BLOCK-n — one line | none}
