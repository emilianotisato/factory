# QA guide — PRD-008 A repo a team can share

Generated 2026-10-15 from the one Human row of [`prd.md`](prd.md) § QA plan: **FR-6**, your first `/harvest` in this repo. Every other row is proven by a test at the checkpoints; that evidence is in the slice files' Progress.

It runs **after the merge**, on `main`, with the 0.6.0 binary. Budget: **about 60 minutes**, most of it the routing interview. ★ marks the checks that matter most. All commands run in the factory checkout.

## FR-6 — The first `/harvest`, over 001 to 007 (≈60 min)

1. **Set up** *(3 min)*. Pull `main`, then `cargo install --path .`. **Pass:** `factory --version` says `0.6.0`, and `git status` is clean apart from the friction idea the 008 run left untracked (§6).
2. **The lessons** *(5 min)*. In Claude Code: `/harvest 001 002 003 004 005 006 007`. Add `008` to drop its committed `evidence/` now (§5 Build order); leave it out and the next harvest does it. **Pass:** each feature's retro sections print with no Numbers block, and the friction ideas print under 005, 006 and 007.
3. ★ **The routing** *(25 min)*. Answer each question. **Pass:** every lesson lands in one of the four places, with the recommendation first. A lesson for factory itself goes through `factory docs idea add`, because this checkout is factory. **Fail:** anything reaches GitHub. `gh issue list --repo emilianotisato/factory --author @me` shows nothing new.
4. ★ **`decisions.md`** *(8 min)*. Review the one diff the skill shows. **Pass:** three `## {Short name}` entries with **Chose**, **Over**, **Because** and **Applies when**, and no number, date or origin. `grep -n 'id="dec-' docs/decisions.md` prints `dec-1`, `dec-2` and `dec-3`, each just above its entry. The rules block no longer says the numbers are monotonic.
5. ★ **The first-harvest edits** *(5 min)*.
   - `grep -n 'are history' CLAUDE.md AGENTS.md` prints one line in each.
   - `docs/index.md` already carries the new archived definition from slice 05: the skill leaves it alone.
   - `factory docs check 2>&1 | grep W-HOME-PATH` prints nothing, except under `docs/features/008-team-and-harvest/` when 008 is not in the batch: its PRD and evidence quote `/home/x/…` fixtures. The four friction ideas' `From` lines are relative.
   - ★ `W-HOME-PATH` and the `--done` rewrite only know a home followed by `/`. Before the skill runs `--done`, `git grep -nE '(^|[^A-Za-z0-9._~-])/(home|Users)/[A-Za-z0-9._-]+([^/A-Za-z0-9._-]|$)' -- docs/features` lists the bare ones, such as the `find` over the home dir at `002-docs-port/prd.md:287`. Rewrite each by hand first, and commit it: once archived, the file is frozen, and `--done` refuses a folder with uncommitted changes.
6. ★ **The archive** *(5 min)*.
   - `git ls-files docs/archive | cut -d/ -f3 | sort | uniq -c` shows **4** for every archived folder.
   - Each archived `prd.md` has the `> **History.**` banner right after its frontmatter.
   - `git grep -nE '(^|[^A-Za-z0-9._~-])/(home|Users)/[A-Za-z0-9._-]+' -- docs/archive` prints nothing. A plain `/home/` grep also finds `/tmp/home/x/` and `/home/<name>/` in prose, which are not paths.
   - `docs/features/` holds only what you left out of the batch.
7. ★ **The commit** *(4 min)*. **Pass:** `git branch --show-current` says `harvest/<date>`, and the commit went through the docs hook, no `--no-verify`. `factory docs check` exits 0. Nothing is pushed.
8. **Record** *(5 min)*. The evidence is the branch: `git diff --stat main...harvest/<date>`. A step that failed becomes an idea (`factory docs idea add`), naming the step.
