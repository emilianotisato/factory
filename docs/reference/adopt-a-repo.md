# Adopt a repo

The prompt below moves a repo onto factory v3, whether it starts on v2, on v3 or bare, and stops for the owner at every judgment call. A fresh Claude Code session in the target repo needs one line:

- **Full adoption:** `Read ~/Projects/factory/docs/reference/adopt-a-repo.md and run its prompt in this repo.`
- **Comment cleanup only**, on a repo already on v3: `Read ~/Projects/factory/docs/reference/adopt-a-repo.md and run phase 5 of its prompt in this repo, as if I said yes.`

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
Its own branch, `chore/comment-cleanup`, from an up-to-date default branch, or from `chore/factory-v3` while that one is not merged, so its diff is the cleanup alone. Never push, merge or delete a branch.

The rule:
- Never name a plan item in code: not in file or folder names, classes, functions, tests, fixtures, strings or comments. No PRD-n, FR-n, AD-n, slice n, checkpoint n. Name the behavior.
- Comments: default to none. One short line only for what the code cannot show: an invariant no name or test expresses, a quirk of something we cannot change, or why a choice is deliberate. No history, no narration, no TODOs.
- The test for a comment that stays: would someone write it fresh today, in a codebase with no plan docs? If not, it goes.

What stays untouched:
- Annotations a checker reads: PHPDoc and JSDoc tags (`@param`, `@return`, `@var`, `@template`, `@property`…), Python type comments, Rust doc comments on a library's public API. Keep every tag, even one that repeats the native type: that is a separate change. Delete only the prose around the tags. A docblock with no tag left goes whole.
- Lint and analyser directives (`@phpstan-ignore`, `@ts-expect-error`, `eslint-disable`, `#[allow]`, `noqa`), each with its reason. An HTTP or CLI contract's doc comment.
- What a framework or tool still owns: vendored code, generated files, stock comment blocks in published config. Comments the team added there follow the rule.
- Recorded data: fixtures, captured tool output.
- Strings: API descriptions, translations, anything the app shows or sends. They are not comments.

**The work list is every comment, not the leaks hits.** `factory leaks` finds only comments with a marker: a plan id or a history word. A comment that restates the code has no marker, so no hit list holds it. A pass driven by the hits strips the ids and keeps every comment around them.

Steps:
1. Run `factory leaks --all` and note the hits and the `comment share` line: the baseline.
2. List every file that holds a comment, across the source, test, view, frontend, migration and config trees. Split them into groups of at most 40 files or 1,500 comment lines, along module lines. Print the group list with counts.
3. One subagent per group, in waves of up to 10, no test runs while they work. Each gets the rule, the untouched list and the order below. It reads **every comment in every file** of its group, edits comment lines only, never code, and batches the edits to one file into as few calls as it can. It returns the files it visited and its comment lines before and after. For each comment, the first match decides:
   plan cite: drop it, keep the fact as one line if the code cannot show it;
   restates the code, the function name or the class name: delete;
   says what the next lines do: delete;
   history ("now", "used to", "was moved", "previously"): delete;
   argues its own correctness about code a test covers: delete;
   TODO or FIXME: delete, and list it with file:line for me;
   empty placeholder (a bare `//` in a framework stub's empty method): delete;
   commented-out code: delete;
   says what the code cannot show: keep, as short as it can be without losing its contract;
   a test or fixture named after the plan: rename it to the behavior;
   a string citing the plan: rewrite it only after grepping that nothing compares against it;
   a section banner in a long file: keep it, minus any plan id;
   a wrong comment: delete, never patch.
   **Delete is the default.** Rewording a comment that restates or narrates is not a fix: it clears the leaks hit and keeps the noise.
   When shortening a long why, keep its contract: exit codes, who owns what, which value is which, why the order matters. Two lines beat one that drops it.
   Never rename migration files or anything a database or outside system records by name.
4. Coverage: each group's visited count must equal its file count. Send back once any group that skipped files.
5. No code changed: list the changed lines that are not comments or blank (`git diff -U0 <base> | grep '^[-+]' | grep -vE '^[-+]{3}|^[-+]\s*(//|\*|/\*|#|\{\{--|--\}\}|<!--|-->|$)'`). Review every line it prints. Renames and plan-citing strings belong there; revert anything else.
6. Recorded data that must keep its ids goes in `[leaks] allow` in `docs/project.toml`: `{ path = "…" }` for strings only, `{ path = "…", scope = "all" }` for everything. Plain paths, no globs.
7. Run the gate once and fix until green. Then check what the gate does not run, such as a browser suite: a syntax check, the formatter, and the test runner's list mode on every changed file there. `factory leaks --all` must print clean.
8. Commit one per top-level area: `chore: drop redundant comments in <area>`. Report the comment share before and after, per group and in total, the TODO list, and any comment you kept that looks borderline, with why. Give me a sample read: 20 random deleted comment lines, 20 random kept ones, and every renamed test and file, each with the git command that lists it. I judge those and merge.

## 6. Report
Branches and commits, the final doctor output, anything you skipped and why, and my next step: usually I merge, then `/prd` or `/deliver NNN` on the first feature.
```

**Two things it cannot do alone:**

- **`/gate` and `/lanes` are interviews.** Plan to be at the keyboard for phase 4.
- **Merging is the owner's,** so phase 5 waits for a yes and runs on its own branch.
