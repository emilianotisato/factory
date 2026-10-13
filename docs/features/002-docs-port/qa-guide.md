# QA guide — PRD-002 Docs port

Generated 2026-10-13 from [`prd.md`](prd.md) § QA plan: the three Human rows, **FR-16** (read the migration plan and the migrated PRD-001 folder), **FR-17** (run three skills in a v2 repo, commit the skills repo) and **FR-20** (read the guess rows of the ideas migration on arca-api). Every other row is proven under `cargo test` at the checkpoints; the evidence is in the slice files' Progress. ★ marks the checks that matter most.

Budget: about 40 minutes in all. The FR-17 part runs after slice 11, before the merge. The FR-16 and FR-20 parts run after the merge, before the real migration of each repo. **No step here migrates a repo for real**: every write happens in a throwaway worktree.

## FR-17 — The skills on `factory docs` (≈15 min, after slice 11)

Needs: slice 11 done, so `cargo install --path .` has put the branch binary on `PATH` and the nine skill files under `~/.claude/skills/` say `factory docs`. This repo is still v2.

1. Confirm the binary and the route. *(1 min)*
   ```
   factory docs --help
   factory docs doctor
   factory docs check; echo "exit $?"
   ```
   **Pass:** `--help` lists eighteen verbs; `doctor` says `v2 (run factory docs migrate --dry)`; `check` exits 0 (it went through the shim to `docs.mjs`).
2. ★ Run each skill's docs step once in this repo, in a fresh session. *(10 min)* `/prd` on a throwaway idea up to its first docs call (`factory docs new …`), then delete the folder; `/prd-to-tasks 002` up to its preflight (`factory docs doctor`, `factory docs gate`), then stop; `/polish` up to its `factory docs check`, then stop. **Pass:** each docs step prints what `docs.mjs` printed before, no `unrecognized subcommand`, no `docs.mjs` command in the transcript. **Fail:** any skill still runs `node …/docs.mjs` (the grep in slice 11's Progress missed a line; note the file and line).
3. ★ Read and commit the skills repo. *(4 min)*
   ```
   git -C ~/Dropbox/dev/claude-code-skills diff --stat
   git -C ~/Dropbox/dev/claude-code-skills diff
   ```
   **Pass:** only the nine files of §7 plus the ideas-wording files change, every hunk is a `docs.mjs` → `factory docs` swap or the FR-20 wording, and `feature-docs/bin/docs.mjs` is untouched. Then commit it yourself, one commit, in that repo.

Evidence: the three outputs of step 1 and one line per skill in slice 11's Progress.

## FR-16 — The migration plan and the migrated PRD-001 folder (≈15 min, after the merge)

Needs: the merged binary installed from `main`. Run on a throwaway worktree so the real tree is never touched.

1. Make the worktree. *(1 min)*
   ```
   git worktree add ../factory-mig main
   cd ../factory-mig
   ```
2. ★ Read the dry run top to bottom. *(5 min)*
   ```
   factory docs migrate --dry
   git status --porcelain
   ```
   **Pass:** `git status` is empty; the plan names `docs/project.toml` (with `[docs]` and, if present, the untouched `[run]` and `[workers]`), a `state.toml` and a `log.jsonl` per live feature, the `index.md` trim, the slice frontmatter removals, the `ideas.md` rows, and one `retros` row per `retro.md` found. **Fail:** anything under `docs/archive/` listed as a write, or a `retro.md` listed as rewritten.
3. Migrate the worktree and read the PRD-001 folder. *(8 min)*
   ```
   factory docs migrate
   factory docs check; echo "exit $?"
   git diff --stat -- docs/
   git diff -- docs/features/001-driver/
   factory docs log show 001 | less
   ```
   **Pass:** `check` exits 0; `prd.md` keeps every section and loses only the prose header block; `retro.md` is not in the diff; `log show 001` reads like the old `agent-log.md`, with 145 decisions, 7 checkpoints, 1 block and each `**Owner (date):**` line as a review under its AD. **Fail:** a decision body cut short (the parser ended an entry at a heading inside it, `docs.mjs:287`), or text from `agent-log.md` missing from both the entries and the `## Notes` section.
4. Clean up. *(1 min)*
   ```
   cd - && git worktree remove --force ../factory-mig
   ```

Evidence: one paragraph in slice 07's Progress: go or no-go for migrating this repo, and anything to fix first. The real migration of this repo is then its own commit, `docs: migrate to v3` (§14).

## FR-20 — The guess rows on arca-api (≈10 min, before arca-api migrates)

Needs: the merged binary. `/home/emiliano/Projects/Prokey/arca-api`, whose `docs/ideas.md` holds 15 `### Parked from …` headings below `## Graduated`. `--dry` writes nothing, so this runs on the real checkout.

1. ★ Read the guesses first. *(8 min)*
   ```
   cd /home/emiliano/Projects/Prokey/arca-api
   factory docs migrate --dry
   ```
   The ideas rows print guesses first. For each guess row, decide: is that idea still open? **Pass:** every Parked bullet below `## Graduated` you consider open is listed as `open`, and every one you consider done is one you will drop after the migration with `factory docs idea drop <id> --because "…"`. **Fail:** a bullet under a real `## Graduated` line listed as migrating, or an open bullet listed as "not migrated".
2. Check the dates. *(2 min)* **Pass:** each guess row's `added` comes from its `### Parked … (date)` heading; a `git blame` date shows only where no date is written.

Evidence: the list of ids to drop, one line in slice 08's Progress, then the real migration of arca-api on your go.
