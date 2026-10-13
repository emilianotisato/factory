# Retro — PRD-004

Written by `/deliver` in the closeout commit, from the agent's side of the run.

**The point is cost next to catch.** An agent asked only about friction votes to cut every check, because every check costs it time. A verdict with an empty Caught column is a preference, not evidence.

## Cost vs catch

| Step | Cost | Caught | Verdict |
|---|---|---|---|
| Inner loop, per slice | 11 runs over 7 slices, 4 red, about 10 min | `cargo fmt --check` on new files (3 slices); an octal-looking `"\0010"` escape that did not compile (slice 04) | keep |
| Checkpoints (full gate) | 2 runs, 0 red, 2.7 min | nothing. It runs on the working tree, so it could not see that HEAD did not compile after slices 01, 04 and 05 | tune: also build the committed tree (`git stash -k` or a worktree of HEAD) |
| Prove, at checkpoints | 2 checkpoints, 7 machine rows | CP-1 found HEAD broken: 18 `skills/` files, `src/review.rs` and `Config.review` never committed. Prove's own HEAD build caught it, not the gate | keep |
| Polish — local review | one pass over the whole diff | renders that follow a symlink at `.claude/skills/<name>` and `.agents/skills` out of the repo; doctor blind to `[links]`; a per-pass error in `review second` that drops the passes already run; the nine source-checkout links never committed | keep |
| Polish — security pass | one subagent | the same symlink escape, extended to `.claude` and `.agents` themselves; `remove_dropped` following a symlinked parent; `convention()` appending through a link to outside the repo; a `-`-leading base reaching `git` as a flag | keep |
| Polish — UX pass | skipped | nothing to review: a CLI with no screens, and the output lines are pinned by tests | keep skipping for CLI-only diffs |
| Polish — second opinion (fresh Claude agents ×2, Cursor auth expired) | two scoped passes | a non-zero exit with partial text reported as findings; no signal forwarding to the reviewer's process group; `settle_orphan` never filing friction; a `[source]` without `path` failing every config load; the upgrade receipt in `CARGO_HOME` | keep, and restore Cursor: same-vendor agents still found 4 defects the others missed |
| test-cleanup | one `prune` pass in pr mode over 96 new tests, one read-only classifier | 4 duplicate verdict and stream unit tests; 2 update tests merged into one table; two friction tests that would fail across midnight UTC | keep |
| Polish — full gate after the fixes | 2 runs, 0 red, about 3 min | nothing; the three fixers had run their own tests in lanes 1-3 | keep |

## Where I guessed

- **`skills/polish/SKILL.md` § 1, the second opinion.** The probe failed (`cursor-agent` auth expired). The skill says to fall back to a fresh agent. It is silent on how many passes and how big. I ran two scoped passes (skills/init/doctor, then review/closeout/upgrade), on the same 600-line budget the Cursor rule uses.
- **The pack contract and the driver's commit.** The contract says a file I make with a command is mine if I `git add` it. It says nothing about files staged before polish began. The nine `.claude/skills` links were staged at CP-1 and still uncommitted. I left them staged and logged AD-11 (`decide`) for a hand commit, rather than blocking.
- **PRD-004 FR-2, "collisions are not special-cased".** That settles a real directory where a link goes. It is silent on a symlink at `.claude`, `.claude/skills`, `.agents` or `.agents/skills`, which redirects the whole write. I read the cut as being about collisions, not about writing outside the repo, and made render refuse those paths.

- **`/prune` on two test names a log entry quotes.** AD-17's Undo line names `a_non_zero_exit_marks_its_partial_and_does_not_count`. The skill protects names in evidence and Progress, not in the log. I kept both review tests rather than leave a stale name in an Undo line.

## What I wished existed

- **A gate that builds HEAD, not the tree.** Both checkpoint gates passed while HEAD did not compile.
- **A driver test with a symlinked dir in a slice's diff.** The commit sweep followed the links as dirs, and nothing in `tests/` covered it until this polish.
- **A `cursor-agent` auth check at preflight**, not at polish. By the time polish probed, nobody was there to log in.

## Project friction → `/guardrails audit`

None. The inner loop's four reds were real (fmt and a compile error), and the bundle lock cost one relock that FR-5 asked for. No floor or lint rule cost more than it caught here.

## Framework friction → skills repo

- **`factory` driver, slice and checkpoint commits** (`src/commit.rs` `Pathspec::from_stream`, `src/closeout.rs` `commit_polish`). A claimed path that is a symlink to a dir counted as a dir. Its contents were never swept, so three slice commits and two checkpoint commits left source behind. This cost a whole CP-1 repair and now a hand commit (AD-11). Fixed on this branch, but only a driver built from it benefits.
- **`factory` driver, checkpoint scope.** Slice 03 finished after CP-1, in a parallel lane, and CP-2's Covers line listed only 06 and 07. Its Prove rows (FR-3, FR-4) were never run until polish found the gap. A checkpoint should cover every `done` slice no earlier checkpoint covered, not only the slices since the last one in plan order.
- **`skills/polish/SKILL.md` § 1, the security-pass trigger.** "File paths built from input" did not obviously cover "a repo's own symlinks redirect a write". The local review found it anyway. The trigger list should name repo-controlled symlinks.
- **`skills/polish/SKILL.md` § 5, fixer decisions.** It says to fold the `log-fix-{k}.md` fragments in with `factory docs log merge`, which v3 removed ("the log is appended by `factory docs log add`"). I re-added the 11 entries by script, renumbered AD-12 to AD-22. Fixers should call `docs log add` themselves; the tool already numbers under a lock.
- **`skills/polish/SKILL.md` § 1, the fallback second opinion.** It names no pass size or count for the Claude fallback. It should say: the same 600-line scoped passes.

## Score

4 — every FR built and its machine rows proven, but the driver lost committed source twice, and the owner must commit nine links by hand.
