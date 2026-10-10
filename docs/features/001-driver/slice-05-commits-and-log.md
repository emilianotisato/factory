---
slice: 05
status: done
lane: 1
depends_on: [4]
---

# Slice 05 — Commits by pathspec and log merge (FR-5)

**Source**: [`prd.md`](prd.md) → §7 FR-5 (all bullets and acceptance); §4 FR-2 bullet 5 (the stream is the record; `tool_use` events carry the file paths). **Depends on**: 04 (two lanes must exist for the interleaving acceptance; and the commit path has to coexist with a parked lane's uncommitted log edit).
**Created**: 2026-10-09 · **Status**: pending

**Progress**: built — `src/commit.rs` (`Pathspec`, `untracked`, `commit`, `merge_logs`), `commit_sha` on `workers` (guarded `ALTER`, schema stays 1), five events, scheduler `commit_slice` / `leftovers` / `merge_or_block` / `park_on_merge`, stub keys `stray` and `ad`, `tests/commits.rs` (chain, two lanes, clash-and-answer) · inner loop: 5 runs, 4 red, ~2 min — red caught: `lanes: 1` opens a preflight ask so the sequential test uses `fx.chain()`; slice titles are `A`/`B`/`C`; `render --write` left `docs/index.md` unchanged so the merge commit holds `prd.md` instead; `tests/run.rs`'s hand-made "built slices" commit had nothing left to commit; fmt reflowed 3 files, clippy clean · deviations: block id `BLOCK-n`, not `BLOCK-<NNN>-merge` (AD-501); `Pathspec::from(summary, board, slice, repo)` takes the board and the repo root (title, feature dir, protected files) instead of `(summary, slice)`; the merge commit also stages the merged fragments' deletions (AD-504); `log_merged` event (AD-502); leftovers are measured against a spawn-time snapshot and the live lanes' streams (AD-505) · earlier-slice tests moved: `tests/loop.rs` (two exact sequences gain `slice_committed` / `log_merged`; two fragment reads after a checkpoint now read `agent-log.md`), `tests/run.rs` (event counts 10→12 and 12→14; the stop sequence gains `slice_committed`; the budget test asserts a clean tree instead of committing one), `tests/lanes.rs` (one fragment read after a checkpoint reads `agent-log.md`) · Prove (task 6, QA row FR-5) at the checkpoint.

**Slice**: After this slice the run leaves a readable history: one commit per slice, made by the driver from the files the worker's stream says it touched, plus that slice's file and log fragment. Two lanes interleave without stepping on each other because each commit is a pathspec, never `git add -A`. When all lanes are drained the driver runs `docs log merge NNN` and commits the merged `agent-log.md`; slice 06 moves that merge to every checkpoint. An AD-number clash is a block, not a silent renumber.

## Current code / environment

- **Today the builder commits** (`deliver/SKILL.md:100-135`: "one commit by pathspec: `feat({NNN}): slice-{NN} {subject}`") and the orchestrator merges at checkpoints (`:79`: "three files no builder may write": `prd.md`, `agent-log.md`, `docs/index.md`). The slice 03 contract removed the commit from the worker; this slice gives it to the driver. The three-files rule stays in the contract: a worker that writes `agent-log.md` races the merge.
- **The pathspec source**: slice 03's `Summary.tool_uses` holds every `tool_use` with its `input`. Paths come from `Edit`, `Write`, `NotebookEdit` (`input.file_path`); a `Bash` command that writes files (`sed -i`, heredocs, `cargo fmt`) leaves no path in the stream. §7: "Files the worker created but never named in a tool call (a `Bash` heredoc, a generator) are listed in the slice log as untracked leftovers, never swept into the commit." `git status --porcelain --untracked-files=all` after the worker exits, minus the pathspec, is that list.
- **The commit message**: §7 "message `feat: <slice title>`, no body, no trailers." The owner's global `commit-msg` hook strips AI trailers anyway, but the fixture runs with `GIT_CONFIG_GLOBAL` empty (slice 01), so the driver must simply never write one. `user.name`/`user.email` come from the repo's git config; the driver does not set an author.
- **`docs log merge NNN`**: `docs.mjs:2061`; `deliver/SKILL.md:145` "refuses on an AD clash" (two fragments claiming the same `AD-n`). The seed `{NN}01` in the pack (§4) is what keeps lanes apart; a worker that ignores it produces the clash. On refusal the driver blocks: a `BLOCK-n` entry in `agent-log.md` with phase `checkpoint`, run `waiting-owner`.
- **Two lanes, one index**: `git add` and `git commit` on the same repo from one driver are serialised by the driver's loop (single thread, one commit at a time), so there is no index lock race. The worker, however, may still be writing files in lane 2 while lane 1's commit runs; a pathspec commit only touches named paths, which is why the rule is pathspec and never `-A`.
- **Deliberate non-changes**: *(prd §7 "Rejected: one commit per checkpoint. Loses per-slice history."; the merge commit is extra, not a replacement.)* *(prd §19 sharing: no attribution lines, no agent names, in any commit.)*

## Tasks

### 1. Pathspec from the stream, plus the slice's own files — high — blocked by: —
**Source**: §7 FR-5: "The driver commits, one commit per slice, when the worker exits `done`: `git add -- <paths from the stream's tool_use events> <slice file> <log fragment>`, message `feat: <slice title>`, no body, no trailers. Never `git add -A`."

`src/commit.rs`: `Pathspec::from(summary, slice) -> Vec<PathBuf>`: `file_path` of every `Edit|Write|NotebookEdit` `tool_use`, made repo-relative and deduplicated, dropping paths outside the repo root and the three files no worker may write (`prd.md`, `agent-log.md`, `docs/index.md`, which are reported as a `worker_wrote_protected_file` event and left out), plus `docs/features/NNN-*/slice-NN-*.md` and `log-slice-NN.md`. Then `git add -- <paths>` and `git commit -m "feat: <title>"` run in the repo root, the title from the board's `slices[].title`. Record `commit_sha` on the `workers` row (new column, idempotent migration) and emit `slice_committed {slice, sha, files}`. If a listed path does not exist (the worker deleted it), `git add` still stages the deletion; a path that was never changed is harmless.

**Acceptance:** after the slice 03 fixture run, `git log --format=%s` shows `feat: a`, `feat: b`, `feat: c` on top of the fixture commit, each `--stat` naming exactly `src/lib.txt`, the slice file and its fragment; `git log -1 --format=%b` is empty; `git status --porcelain` is clean apart from `agent-log.md` if a merge has not run yet.

### 2. Untracked leftovers go in the log, not the commit — medium — blocked by: 1
**Source**: §7 FR-5: "Files the worker created but never named in a tool call (a `Bash` heredoc, a generator) are listed in the slice log as untracked leftovers, never swept into the commit."

Before the commit: `git status --porcelain --untracked-files=all`, minus the pathspec, minus ignored paths; non-empty → append `## Untracked leftovers` with one `- path` line per file to the slice's fragment (so the fragment, already in the pathspec, carries the list into the commit) and emit `untracked_leftovers {slice, files}`. The files themselves stay in the working tree for the owner.

**Acceptance:** stub step `done{touch:["src/lib.txt"], stray:["notes.txt"]}` (new stub key: written without a `tool_use`): the commit excludes `notes.txt`, the fragment lists it, and the file still exists after the run.

### 3. Two lanes interleave cleanly — high — blocked by: 1
**Source**: §7 acceptance: "two lanes interleave commits without touching each other's files."

No new mechanism; a test. Fixture `lanes: 2`, plan `done{delay_ms:800, touch:["src/a.txt"]}, done{delay_ms:200, touch:["src/b.txt"]}, done{touch:["src/c.txt"]}`. Assert three commits; the commit for slice 02 lands before slice 01's, and `git show --stat` of each names only its own touched file, slice file and fragment; `git status` clean except the merge target.

**Acceptance:** the test above; additionally a `git fsck` on the fixture passes (no half-staged index).

### 4. Log merge when the lanes drain — high — blocked by: 1
**Source**: §7 FR-5: "At each checkpoint, after the lanes drain, the driver runs `docs log merge NNN` and commits the merged `agent-log.md`. An AD-number clash makes the merge refuse; the driver blocks rather than renumbering."

Checkpoints are slice 06's; here the driver merges once, when `allDone` is true and no worker is running, before `finished`: `docs log merge NNN` (via `docs.rs`), `docs render --write`, then `git add -- docs/features/NNN-*/agent-log.md docs/features/NNN-*/prd.md docs/index.md` and commit `docs: merge slice logs`. Refusal (non-zero exit, stderr naming the clash) → write a `BLOCK-<NNN>-merge` entry in `agent-log.md` (phase `checkpoint`, the tool's text as the question, "renumber the later fragment" as Recommended), event `merge_refused`, phase `waiting-owner`; `factory answer` on it (slice 04) retries the merge. Slice 06 moves this call into the checkpoint and leaves the drained-at-end case as the final checkpoint.

**Acceptance:** after the fixture run the fragments are gone, `agent-log.md` holds `AD-101`, `AD-201`, `AD-301`, and `git log -1 --format=%s` is `docs: merge slice logs`; a stub step `done{ad:"AD-101"}` on slice 02 (duplicate seed) makes the merge refuse, leaves both fragments in place, writes the BLOCK entry, and the run parks.

### 5. Verify — medium — blocked by: 1–4
`cargo test`. The full gate runs at this slice's checkpoint.

**Acceptance:** inner loop green, one commit.

### 6. Prove — at the checkpoint — blocked by: 5
Rows from [`prd.md`](prd.md) § QA plan: **FR-5** ("`git log --stat` after a fixture run shows one commit per slice with only that slice's files; two lanes interleave", evidence: `git log` transcript). At the checkpoint: the task 3 run; paste `git log --stat --format='%h %s'` into this slice's **Progress**.

**Acceptance:** the transcript shows three `feat:` commits plus the merge commit, each `feat:` commit touching its own three paths only.
