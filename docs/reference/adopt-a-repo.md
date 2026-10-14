# Adopt a repo

Paste the prompt below into a fresh Claude Code session in the target repo. It moves the repo onto factory v3, whether it starts on v2, on v3 or bare, and stops for the owner at every judgment call.

```text
Move this repo onto factory v3: the docs board, the skill family, the gate and the lanes. Work on a branch, commit as each phase completes, and stop for me wherever a step needs my judgment. Never push, merge or delete a branch: those are mine.

## 0. Preconditions
- `factory --version` must say 0.4.0 or later. If not, stop and tell me.
- Find the default branch (`git symbolic-ref refs/remotes/origin/HEAD`). If I'm on another branch, check whether it is already merged, squash merges included (its changes already on the default branch). Uncommitted work: stop and ask. Otherwise switch to the default branch and `git pull --ff-only`.
- Create `chore/factory-v3` from there.
- Run `git shortlog -sne --since=60.days`. If anyone besides me commits here, tell me before phase 2: the migration moves the docs layout under everyone.

## 1. Which state is this repo in
- `docs/project.toml` exists: already v3. Go to phase 3.
- A v2 board (`docs/index.md`, `docs/features/`, `docs/ideas.md`, `agent-log.md` files) and no `docs/project.toml`: phase 2.
- No docs board at all: go to phase 3; `factory init` creates it.

## 2. Migrate the v2 docs
1. If a feature is mid-delivery (a v2 run in flight, a slice in progress), stop and ask.
2. `factory docs migrate --dry`. Read all of it. Report in a few lines: the counts, every `warn`, and anything marked "not migrated" or dropped that carries real information. If something would be lost, ask before going on.
3. `factory docs migrate`. Review `git diff --stat`, then spot-read `docs/index.md`, one shipped feature's `state.toml` and `log.jsonl`, and `docs/ideas/`.
4. `factory docs check` must pass.
5. Commit: `chore(docs): migrate the board to factory v3`.

## 3. Install the skill family
1. `factory init`. Follow the commit line it prints.
2. Grep `CLAUDE.md`, `AGENTS.md` and `.claude/` for retired v2 names: `feature-docs`, `prd-to-tasks`, `docs.mjs`, `agent-log.md`, `factory-deliver`. Point them at the v3 equivalents (`/slice`, `/deliver`, `factory docs …`, `log.jsonl`). Touch nothing else in those files. Commit.

## 4. Doctor, then the attended setups
Run `factory docs doctor` and work through every line that is not green, in this order. /gate, /lanes and /harness are interviews: run them with me, never guess my answers.
- No gate: `/gate`.
- No lanes, or preflight refuses: `/lanes`. Done only when `factory lanes up --prove` is green on every lane.
- No harness: ask me whether to run `/harness` now. It pays off when the app has a UI or an API to drive.
- No UX rubric: mention it only. `/ux` builds it later.
- Leaks: report the count and the top rules from `factory leaks --all`, then ask me about phase 5.
Re-run `factory docs doctor` at the end and show me the output.

## 5. Comment cleanup (only if I say yes)
Its own branch, `chore/comment-cleanup`, from `chore/factory-v3` (or from the default branch if that one is merged), so its diff is the cleanup alone.

The rule:
- Never name a plan item in code: not in file or folder names, classes, functions, tests, fixtures, strings or comments. No PRD-n, FR-n, AD-n, slice n, checkpoint n. Name the behavior.
- Comments: default to none. One short line only for what the code cannot show: an invariant no name or test expresses, a quirk of something we cannot change, or why a choice is deliberate. No history, no narration, no TODOs.
- A library's public API, an HTTP or CLI contract, and type annotations a checker reads keep their doc comment.

Steps:
1. `factory leaks --all` for the hit list. Split the source and test trees into module groups.
2. One subagent per group, in parallel, no test runs between them. Each gets the rule and this order per comment:
   plan cite: drop it, keep the fact as one line if the code cannot show it;
   restates the code or the name: delete;
   history: delete;
   argues its own correctness: delete;
   TODO: delete, and list it for me;
   says what the code cannot show: keep, one line, about 20 words;
   a test or fixture named after the plan: rename it to the behavior;
   a string citing the plan: rewrite it only after grepping that nothing compares against it;
   a wrong comment: delete, never patch;
   a section banner: keep it, minus any plan id.
   When shortening a long why, keep its contract: exit codes, who owns what, which return value is which. Two lines beat one that drops it.
   Never rename migration files or anything a database or outside system records by name.
3. Recorded data that must keep its ids (fixtures, captured tool output) goes in `[leaks] allow` in `docs/project.toml`: `{ path = "…" }` for strings only, `{ path = "…", scope = "all" }` for everything. Plain paths, no globs.
4. Run the gate once at the end and fix until green. `factory leaks --all` must print clean.
5. Commit, then give me a sample read to do: 20 random deleted comment lines, 20 random kept ones, and every renamed test and file, each with the git command that lists it. I judge those and merge.

## 6. Report
Branches and commits, the final doctor output, anything you skipped and why, and my next step: usually I merge, then `/prd` or `/deliver NNN` on the first feature.
```

**Two things it cannot do alone:**

- **`/gate` and `/lanes` are interviews.** Plan to be at the keyboard for phase 4.
- **Merging is the owner's,** so phase 5 waits for a yes and runs on its own branch.
