# Slice 04 — Landing library

**Source**: [`prd.md`](prd.md) → §5 FR-3 (the lane committed whole, the cherry-pick onto the feature tip, a dirty main tree, the `refs/factory/` ref), §10's per-role protected files as FR-3 quotes them, §13 OPEN-2. **Depends on**: nothing. It is a library nothing calls yet; slice 07 wires it into the driver for build workers, slices 08 to 10 for the other roles.
**Created**: 2026-10-14

**Progress**: _(written as it lands — what was built, the gate numbers, what a review caught)_

**Slice**: After this slice `src/land.rs` can take a lane worktree and put its work on the feature tip as one commit. `commit_lane` stages everything in the lane with `git add -A`, unstages the board files and the role's protected files, squashes any commit the worker made itself onto the lane's base, makes one commit with the given message, and keeps it reachable under `refs/factory/`. `land` cherry-picks that commit onto the main tree's HEAD and adds the main tree's board files to the same commit. It returns `Landed`, `Conflict` with the conflicted paths and the main tree untouched, or `Dirty` with the owner's paths it would have overwritten. Nothing in the driver calls it yet, so every run behaves as today.

## Current code / environment

- **Today's commit is by pathspec**: `commit::commit` (`src/commit.rs:379-423`) filters the paths through `ls-files`, `git add --` them, and commits `-- paths`, "so nothing else staged rides along" (`:369-371`): an owner's `git add` stays staged and out of the driver's commit. The module doc (`:1-17`) says "Never `git add -A`". That rule holds for the main tree still; the lane is the new place where `-A` is right, because nobody else writes there.
- **The branch hold**: `commit` refuses while HEAD is off the run's branch, through `held(repo)` and `branch_refusal` (`:383-387`, `:435-451`, `hold_branch` `:455+`). The hold is keyed by the main tree's path. A landing writes the main tree, so it must refuse the same way; a lane is detached by design, so the hold must not be checked there.
- **The board files and the protected files**: `commit::tool_files(board)` (`:39-45`) is the feature's `state.toml` and `log.jsonl`; `commit::protected_files(board)` (`:33-35`) is the feature's `prd.md` and `docs/index.md`. Every role but polish strips both protected files; polish keeps `prd.md` (`src/closeout.rs:436-441`; prd §10). Only a slice emits `WorkerWroteProtectedFile` (`src/scheduler.rs:971`, the event at `src/events.rs:159-162`); the others print a line. So the library takes the drop list as an argument and reports which dropped paths the worker had changed; the caller emits or prints.
- **Subjects**: `commit::subject(board, kind, text)` (`:507-509`) gives `feat(005): slice-02 <title>`. The library takes the full message; callers keep building it as today.
- **The keys for the ref**: today's baseline keys are `slice-NN` (`src/scheduler.rs:876-878`) and `checkpoint-{n}-{role}` (`:881-883`); polish and CI fix have their own in `src/closeout.rs`. The ref is `refs/factory/<NNN>/<key>` with the same keys.
- **git 2.55.0 here** (prd §17): `git merge-tree --write-tree --merge-base <base> <ours> <theirs>` reports a three-way result and its conflicted paths without touching the index or the tree.
- **Deliberate non-change**: *(prd §16 "Not handled: a worker landing a change to `docs/project.toml`. Accepted 2026-10-13, owner's cut: `git add -A` makes it possible, but no run has shown it". Do not add `docs/project.toml` to any drop list.)*

## Tasks

### 1. `commit_lane` — high — blocked by: —
**Where**: `src/land.rs` (new), `src/lib.rs` (`pub mod land;`).

**Source**: §5 FR-3: "The lane is committed whole. When a worker exits and the board says its slice is `done` …, the driver runs `git add -A` in the lane. It drops the feature's `state.toml` and `log.jsonl` and the protected files from the stage, per role as today (§10): every role drops `docs/index.md`, and every role but polish drops the feature's `prd.md`. … then makes one commit on the lane's base with today's subject. A commit the worker made itself is squashed into it." "The lane commit stays reachable until it lands, through a ref under `refs/factory/` (name: slice detail)".

`commit_lane(lane, base, drop: &[String], message, ref_name) -> Result<Option<LaneCommit>>`:

1. `git reset --soft <base>` in the lane: a commit the worker made itself folds back into the index.
2. `git add -A`.
3. Unstage every path in `drop` (`git reset -q -- <paths>`); record which of them were staged. A dropped path stays in the lane's tree as the worker left it.
4. Nothing staged → `Ok(None)`. Else `git commit -q -m <message>` in the lane, no `--author`, hooks on, as `commit::commit` does.
5. `git update-ref <ref_name> <sha>`.

`LaneCommit { sha, files, dropped }`: `files` from `git diff-tree --no-commit-id --name-only -r -z <sha>`, `dropped` the drop paths the worker had changed.

**The trap**: `git add -A` stages a deletion, a rename, a symlink and a new directory, which is the point; but run step 1 before step 2, or a self-made commit's files show as unchanged against HEAD and the squash commit misses them.

**Acceptance:** unit tests in `src/land.rs` on a temp repo with a linked worktree as the lane, `base` its HEAD: (a) a lane with an edit, a `git mv`, a new dir with a file, and a symlink to a dir yields one commit whose parent is `base` and whose `files` list every one of those paths; (b) a lane where the worker committed one file itself and left another uncommitted yields one commit on `base` holding both; (c) a lane that changed `state.toml`, `log.jsonl`, `prd.md` and `docs/index.md` beside one code file, with all four in `drop`, yields a commit holding only the code file and `dropped` naming the two protected files it changed (board files are dropped silently); (d) a lane with nothing changed yields `None` and no ref; (e) after (a), `git rev-parse <ref_name>` is the commit's sha.

### 2. `land` — high — blocked by: 1
**Where**: `src/land.rs`.

**Source**: §5 FR-3: "The driver lands it by cherry-pick onto the feature tip in the main tree, and adds the main tree's board files to the same commit. One commit per slice, linear history, today's subjects." "A dirty main tree parks the run. When a path the pick touches is dirty in the main tree (board files aside), the run parks with a line naming the paths, as the off-branch refusal does (`src/commit.rs:435-451`). The owner's edit stays untouched." §6 FR-4: "The driver aborts the pick in the main tree, which stays untouched."

`land(main, sha, board_files: &[String]) -> Result<Landing>`:

- The branch hold first: a held main tree off its branch is an `Err`, with `branch_refusal`'s text, before anything is read.
- The paths the pick touches: `git diff --name-only -z <sha>^ <sha>`. Any of them dirty or untracked in the main tree (`git status --porcelain -z -- <paths>`), board files aside → `Dirty { paths }`, nothing written.
- A conflict check that does not touch the tree, e.g. `git merge-tree --write-tree --merge-base <sha>^ HEAD <sha>` → conflicted paths → `Conflict { paths }`, nothing written.
- Else apply the pick to the main tree (`git cherry-pick --no-commit <sha>`), `git add --` the board files that exist, and commit by pathspec: the picked paths plus the board files, with the lane commit's message (`git commit -q -C <sha> -- <paths>`), so an owner's unrelated staged change stays staged and out, as `commit::commit` keeps it. Delete the `refs/factory/` ref that pointed at `sha`.

`Landing::Landed { sha }` is the new HEAD.

**The trap**: the main tree's board files are dirty by design (workers write them there through `FACTORY_BOARD`, slice 02), so they must be excluded from the dirty check and included in the commit. An owner's staged file in a path the pick does not touch must survive: commit by pathspec, never a bare `git commit`.

**Acceptance:** unit tests on a temp repo with a lane: (a) `Landed`: HEAD's parent is the old HEAD, HEAD holds the lane commit's files byte-equal plus the main tree's modified `state.toml`, its message equals the lane commit's, `git status --porcelain` is empty, and the ref is gone; (b) two lane commits from the same base touching different files land in turn as two commits, one parent each; (c) `Conflict`: a main-tree commit changed line 3 of `shared.txt` after the lane's base and the lane commit changes line 3 too → `Conflict { paths: ["shared.txt"] }`, HEAD and `git status --porcelain` unchanged, the ref still resolves; (d) `Dirty`: the owner's uncommitted edit to a path the pick touches → `Dirty` naming it, the file keeps the owner's bytes; (e) an owner's staged edit to an unrelated file stays staged and is not in the landed commit; (f) a main tree held to a branch and switched to another → `Err` naming the branch.

### 3. Verify — medium — blocked by: 1, 2
`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 4. Prove — none
No row of [`prd.md`](prd.md) §14 is this slice's: it is a library nothing calls yet. FR-3's rows are proven through the driver by slice 07, FR-4's by slice 08. The checkpoint runs the gate over it.
