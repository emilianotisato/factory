---
number: 005
slug: isolated-lanes
title: "Isolated lanes"
# type: feature | bugfix | enhancement | refactor
type: feature
created: 2026-10-14
author: Emiliano Tisato
summary: |
  Every worker runs in its own git worktree (a lane), seeded by clone; the driver commits the lane with `git add -A` and lands one commit per slice on the feature tip, a merge worker on conflict. Deletes the pathspec guessing. Folds in checkpoint coverage; adds the `/lanes` skill and `factory lanes up|prune`. **Agreed 2026-10-13** (three interview rounds; Cursor unauthenticated, so the adversarial pass was a fresh Claude agent: 2 high and 5 medium findings, all corrected inline; the cut kept FR-10 and retired FR-8, protecting `project.toml`). Proof: a stub harness slice first in the order, then live runs here and in a Laravel repo after release. **Sliced 2026-10-13, ten slices:** 01 harness, 02 board and transcripts from a lane, 03 lane worktrees and `lanes up`, 04 landing library (independent) ▲ · 05 checkpoint covers, 06 `/lanes`, `--prove`, `prune` and the bump to 0.3.0 (after 03), 07 build workers in lanes (spine, highest risk, after 01-04) ▲ · 08 merge worker → 09 checkpoint in a free lane ▲ · 10 closeout in a lane and the guessing deleted, alone ▲. Numbered for the installed v0.2.1 driver that builds it. No `qa-guide.md`: no Human rows.
from_idea: "2026-10-13-isolated-lanes-a-git-worktree-per-lane-a-cow-clone"
---

# PRD-005 — Isolated lanes

Graduated from idea `2026-10-13-isolated-lanes-a-git-worktree-per-lane-a-cow-clone` (its body, including the polish-review addendum, is folded into §1, §7 and §11; its open questions are settled in §13).

## 1. Why now

**Every factory run so far lost work to the shared working tree.** Lanes share one tree (PRD-001 FR-3, `docs/features/001-driver/prd.md:69`), so the driver has to guess which files a worker touched, and the guess misses:

- **PRD-001 run B**: unnamed files left out of slice commits.
- **PRD-002**: AD-305; AD-9901, the polish commit staged about 50 files of other slices' code (`docs/features/002-docs-port/retro.md:39`); 54 leftovers on slice 07.
- **PRD-004**: three slice commits (9eed991, 34d9bca, 393ddbd) left their source in the tree, so HEAD did not compile and CI went red (BLOCK-1, AD-11, AD-24). The checkpoint gate stayed green because it runs on the working tree, not on HEAD (`src/checkpoint.rs:507-508`). CP-1: "could not see that HEAD did not compile".
- **PRD-004 slice 03** finished after CP-1 and no checkpoint ever covered it (CP-1 covered 01, 02, 04, 05; CP-2 covered 06, 07). See FR-7.
- **PRD-001 polish** improvised its own worktree recipe (detached HEAD, own `CARGO_TARGET_DIR`, cherry-pick back, one merge gate); that merge gate caught a red no fixer's own gate could see. The next worker would improvise a different recipe.

**Root cause.** `Pathspec::from_stream` (`src/commit.rs:87-123`) reads Edit/Write/NotebookEdit inputs and parses heredocs and `git add` lines. Every other Bash edit is invisible: `git mv`, inline Python, `printf >`, rustfmt. A directory's new files are swept only while no sibling lane is live (`scheduler::unnamed`, `src/scheduler.rs:1061-1137`, its `alone` flag). The symlink case was patched in 4ece217; the class remains. The README names isolated lanes as the next driver PRD.

**Improvements folded in, owner's call 2026-10-13:** checkpoint coverage by slice set (FR-7), a lane cleanup verb (FR-10). Protecting `docs/project.toml` (FR-8) was cut at the final review.

**Left alone:** what the scheduler builds and when (`depends_on`, `lanes`, `checkpoint_every`), the run's autonomy rules, ship, PR and CI flow.

## 2. Blast radius

- **The owner's main tree.** It no longer receives worker edits mid-run. It receives one commit per slice when the driver lands it. An owner edit in a path a landing touches parks the run (FR-3).
- **Every repo on the framework.** Preflight refuses until `[lanes] seed` is declared (FR-1). This repo, icg-cloud and arca-api each run `/lanes` once before their next run.
- **Disk.** Lanes live under the data dir. A clone (btrfs, APFS) costs close to nothing; a plain copy costs the full seeded size per lane. `/lanes` picks the seed list per repo for that reason.
- **Worker contracts** (`assets/contract-*.md`, `assets/autonomy.md`): the commit rule and the lane prose change; a new merge contract (FR-4).
- **Skills bundle.** `/lanes` is new and `polish` §5 changes, so `skills/bundle.lock` forces a version bump; release with `scripts/release.sh` after the merge.
- **This repo's lane files** (`docs/reference/lanes/lane-N.md`, "What this lane does not isolate") and the `factory docs lanes` template (`src/docs/lanes.rs`) describe a shared tree; both change.
- **CLI.** New `factory lanes up [--prove]` and `factory lanes prune`; `factory docs log add` gains `--covers`. No external API, webhook or URL.
- **Dead after the live run:** `target/lane-1..4` here (43G of the 50G `target/`). Nothing in the binary deletes them; the owner does.

## 3. FR-1 — Lane worktrees, seeded by clone

**Today.** No code creates a worktree (`git worktree` appears only in `tests/docs_v3.rs:238-264`). A lane is an env list and a setup command (`src/lanes.rs:27-28`, `:46-87`, `FACTORY_LANE=N` first at `:92-95`). Setup runs `sh -c` in the main tree (`src/lanes.rs:105-109`) once per run (`src/scheduler.rs:687-757`, `lanes_set_up`; `tests/lanes.rs:1146`).

**Decisions, 2026-10-13:**

- **Lane N is a detached git worktree at `<data dir>/lanes/<repo key>/lane-N`.** Data dir as today (`src/paths.rs:20-26`). The repo key is new and path-safe: the repo directory's name plus a short hash of its canonical path. *Corrected 2026-10-13, adversarial pass:* an earlier draft said "repo key as runs use it (`src/run.rs:354-375`)"; runs sit flat under `runs/<id>` and are matched by comparing the canonical path (`src/run.rs:608-612`), so no such key exists. Detached, so no branch is locked and the feature branch stays checked out in the main tree. *Cost:* the lanes sit outside the repo; `git worktree list` shows them. *Lost:* a path inside the repo or `.git/` gains nothing, and a clone needs the same filesystem, which the data dir is here (btrfs `/home`) and on a default macOS volume.
- **`[lanes] seed = [...]` in `docs/project.toml`** lists repo-relative paths of ignored state to copy into each lane (`target`, `vendor`, `node_modules`, `.env`). A path is copied only when the lane lacks it, so a lane keeps its own build state across runs. Clone first: `cp --reflink=always` on Linux, `cp -c` (clonefile) on macOS; a failed clone falls back to a plain copy and says so. A seed path missing in the main tree is skipped with a line. *Cost:* on ext4 or across filesystems, each lane holds a full copy (this repo's `target/` four times would be 200G).
- **A slice taking a free lane resets it** to the feature tip: detached checkout, forced, and untracked files that are not ignored removed. Ignored seeded state stays. A respawn, or a resume after an answer, keeps the lane as the worker left it: a `building` or `blocked` slice holds its lane (`src/docs/board.rs:322-326`). *Cost:* see §16, an abandoned slice's unlanded edits.
- **Setup runs inside the lane**, cwd the lane, with the lane's env. In a run, once per lane, at that lane's first spawn (today's rule). `factory lanes up` runs it in every lane each time it is called.
- **`factory lanes up`** creates the missing worktrees `1..=lanes`, seeds them, runs setup, and prints one line per lane: its path, `clone` or `copy` per seed path, the setup exit. Nonzero on any failure. The driver's preflight runs the create and seed half only, so a fresh run does not run setup twice.
- **Preflight refuses** when `docs/project.toml` has no `[lanes] seed` key, and names `/lanes`, as it does for a missing gate. `seed = []` is a valid declaration. *Cost:* every repo runs `/lanes` once.
- **No shared-tree fallback.** The old path is deleted, not kept behind a flag (v3 only, no v2 fallbacks).

**Acceptance:**

- A fixture with `lanes = 2`, `seed = ["big", ".env"]` and a setup that writes `pwd` into a file: `factory lanes up` exits 0; `git worktree list` shows two detached worktrees under `$FACTORY_DATA_DIR/lanes/`; each holds `big/` and `.env` byte-equal to the main tree's; each setup file holds its own lane's path.
- When a clone fails, `up` prints `copy` for that path and the lane content is still byte-equal.
- `factory run` in a fixture without `[lanes] seed` exits nonzero before any spawn, and the message names `/lanes`.

## 4. FR-2 — Every worker runs in its lane

**Today.** Every role's cwd is the main tree (`src/worker.rs:280,294`) and `FACTORY_REPO` is the main tree (`:322`). Only build workers get the lane env: polish (`src/closeout.rs:245`), checkpoint (`src/checkpoint.rs:279`) and fix (`:600`) spawn with `&[]`. Workers write the board with `factory docs set` and `factory docs log add` (`assets/contract-slice.md:12,17-19,23`, `contract-fix.md:15,18`, `contract-checkpoint.md:15,19`, `contract-polish.md:17,18,21`, `contract-ci-fix.md:14,17`), and polish writes its score to the metrics store with `factory docs retro --set score` (`contract-polish.md:15`), keyed by the canonical repo root (`src/docs/retro.rs:3,21-31`). All of them find the repo from cwd (`src/cli/docs.rs:220`). The lock is per feature dir (`src/docs/state.rs:124-133`). The transcript slug comes from the main tree's path (`src/measure.rs:282-291`). The hook takes the repo from `FACTORY_REPO` (`src/cli/hook.rs:893-898`) and keys its guards to it (`:104-112`, `:797-802`).

**Decisions, 2026-10-13:**

- **Every role runs in a lane**: build, fix, checkpoint, polish, CI fix and merge (FR-4). Cwd is the lane, the env is the lane's (`FACTORY_LANE` first), and `FACTORY_REPO` is the lane path, so the hook's guards hold there.
- **Board and metrics writes go to the main tree.** The driver sets an env var (its name is a slice detail) that points `factory docs set`, `factory docs log add`, `factory docs retro --set` and `factory docs status` at the main tree, under the existing lock. Every other file a worker writes is a lane file and lands with the commit: a slice file, `retro.md`, an idea from `factory docs idea add`. *Corrected 2026-10-13, adversarial pass:* an earlier draft left out `retro --set`, which would have filed polish's score under the lane's path. *Cost:* two roots in one process. A board write that misses the env lands in the lane's copy, which landing drops (FR-3). *Lost:* "the driver writes the board from worker reports" changes every contract and the exit routing (`src/scheduler.rs:789-864`), which reads the board today.
- **Transcripts are found by the worker's cwd** (`src/measure.rs:282-291`; callers `src/metrics.rs:515`, `src/cli/report.rs:929`; stub mirror `tests/stubs/claude:603,674`).
- **`factory ctx` needs nothing**: its base is cwd (`src/main.rs:78`) and it keeps no index (`src/ctx/mod.rs:4-5`).

**Acceptance:**

- In the harness, each stub call's recorded cwd (`calls/N.json`) is its lane path; its env holds `FACTORY_LANE`, the lane's declared env, and `FACTORY_REPO` set to the lane path.
- A stub's `docs set` and `docs log add` from a lane show in the main tree's `state.toml` and `log.jsonl`; the lane's copies are unchanged.
- A polish stub with a `subagents:` step, run in a lane, shows its subagent's tokens in `factory report`. The transcript is read only for subagent usage (`src/metrics.rs:515`, `src/cli/report.rs:929`), so this is the line that moves when the slug is wrong.
- `factory hook`, fed a whole-file Read of `<lane>/<pack-held file>` with `FACTORY_REPO` set to the lane, denies it, as it does for the main tree today (`src/cli/hook.rs:104-112`).

*Corrected 2026-10-13, adversarial pass:* two earlier bullets could not move. "Nonzero tokens for a lane worker" passes with a wrong slug, because a worker's own tokens come from its stream (`src/cli/report.rs:938-941`). "A stub `git push` is denied in a lane" never runs the hook (the stub ignores `--settings`), and the push guard does not read `FACTORY_REPO` (`src/cli/hook.rs:207`).

## 5. FR-3 — The driver lands each slice on the feature tip

**Today.** The driver commits by pathspec and never runs `git add -A` (`src/commit.rs:1-17`; `commit` at `:379-423`; `commit_slice` at `src/scheduler.rs:958-1056`). With two or more lanes, a slice commit carries the whole `log.jsonl` and `state.toml`.

**Decisions, 2026-10-13:**

- **The lane is committed whole.** When a worker exits and the board says its slice is `done` (routing unchanged, `src/scheduler.rs:789-864`), the driver runs `git add -A` in the lane. It drops the feature's `state.toml` and `log.jsonl` and the protected files from the stage, per role as today (§10): every role drops `docs/index.md`, and every role but polish drops the feature's `prd.md`. A slice landing emits `WorkerWroteProtectedFile` and the others print a line, as today. then makes one commit on the lane's base with today's subject. A commit the worker made itself is squashed into it. *Cost:* every file in the lane that is not gitignored lands, scratch files too. The contracts say so: `assets/contract-slice.md:20` and its polish, fix and CI-fix twins become "everything not gitignored in your lane is committed".
- **The driver lands it by cherry-pick** onto the feature tip in the main tree, and adds the main tree's board files to the same commit. One commit per slice, linear history, today's subjects. *Lost:* a merge commit per slice (non-linear history); a rebase (the lane holds one commit, so it is the same pick).
- **A dirty main tree parks the run.** When a path the pick touches is dirty in the main tree (board files aside), the run parks with a line naming the paths, as the off-branch refusal does (`src/commit.rs:435-451`). The owner's edit stays untouched.
- **The lane commit stays reachable until it lands**, through a ref under `refs/factory/` (name: slice detail), so no reset loses it.
- **A slice counts as done for its dependents only once it has landed.** A build worker sets its slice `done` before it exits (`assets/contract-slice.md:19`), and `ready` follows `done` (`src/docs/board.rs:306`). So the driver does not spawn a ready slice while one of its dependencies is still in its table, and a slice whose landing turns into a merge goes back to `building` (FR-4). Otherwise a dependent's lane resets to a tip without its dependency's code. *Corrected 2026-10-13, adversarial pass:* an earlier draft let `done` stand from the worker's own write.
- **Driver work lands the same way**: a fix, checkpoint, polish or CI-fix result, with its subject from today (`docs: checkpoint n`, `fix({NNN}): polish review fixes`, …).

**Acceptance:**

- A stub slice that edits only through Bash (`git mv`, `printf >`, an inline script rewriting a file), adds a directory, and adds a symlink to a directory: after it lands, `git show --stat HEAD` lists every one of those paths, and `git status --porcelain` in the main tree is empty.
- That landed commit, checked out alone in a fresh worktree, holds every file the stub created, byte-equal to the lane's. *Corrected 2026-10-13, adversarial pass:* an earlier draft said "passes the fixture's `./gate.sh`", which is green unless the gitignored `scenario/` says red (`tests/fixture/gate.sh:12-23`), so it passed for a commit missing its source.
- A dependent slice whose dependency's worker has written `done` but not yet exited is not spawned until that dependency lands: the dependent's first stub call records a HEAD that holds its dependency's commit.
- Two lanes landing in turn give two commits, one parent each, each with its slice's subject and the board files.
- A main-tree file dirty in a path the pick touches parks the run, the park line names it, and the file keeps the owner's content.

## 6. FR-4 — A merge worker resolves a conflicted landing

**Today.** Lanes cannot conflict: they write the same tree, and a sibling's half-written file shows up as a red in someone else's build (`docs/reference/lanes/lane-2.md`, "What this lane does not isolate").

**Decisions, 2026-10-13:**

- **A conflicting pick spawns a merge worker in the slice's lane.** The driver aborts the pick in the main tree, which stays untouched. It redoes the pick in the slice's lane on the new tip, leaving the conflict in that worktree. It spawns a merge worker there: a new job kind and contract, the pack holding the slice file, the slice commit's message and diff stat, and the conflicted paths. The driver sets the slice back to `building`, so it keeps its lane, its dependents wait, and no checkpoint counts it. The driver sets it `done` again once the merge lands; the merge worker does not touch the slice's status. *Corrected 2026-10-13, adversarial pass:* an earlier draft said "the slice stays `building`"; the build worker had already set it `done` (`assets/contract-slice.md:19`), which frees its lane (`src/docs/board.rs:322-326`) and its dependents (`:306`).
- **It may change anything in its lane** (owner's call). *Cost:* it can rework a sibling slice's landed code; the checkpoint gate and the checkpoint worker see that, nothing earlier does. *Lost:* conflicted files only (a call site in the slice's own code could not be fixed, so it blocks); conflicted plus the slice's own files.
- **It runs the inner loop until green.** It is a BLOCK when a conflict marker remains in a file that was conflicted (the driver checks), when the worker logs a BLOCK after three attempts on the same failure (today's worker rule, `assets/contract-slice.md:14`), or when it dies twice. A BLOCK sets the slice `blocked`, answered like any other.
- **A clean exit lands** as FR-3. If that landing conflicts again because the tip moved, one more merge worker runs; the third conflict for one slice is a BLOCK.

**Acceptance:**

- Slices 1 and 2 in two lanes both change line 3 of `shared.txt`. Slice 1 lands; slice 2's pick conflicts. A stub call is recorded with role `merge` and cwd slice 2's lane. After it writes a resolution, HEAD holds both slices' commits in order and `shared.txt` holds the resolved text.
- The same, with the merge worker exiting with markers left: the run parks on a BLOCK for slice 2, `factory status` shows slice 2 `blocked`, HEAD is slice 1's commit, and the main tree holds no conflict marker.

## 7. FR-5 — Driver gates and closeout work run in a free lane at HEAD

**Today.** The checkpoint gate runs `sh -c` in the main tree with `FACTORY_LANE` removed (`src/checkpoint.rs:507-508`). The gate cache key is the main tree's tree id, built through a temp index (`src/gatecache.rs:82-110`), and a dirty tree outside `docs/` pins it (`src/checkpoint.rs:536`). A checkpoint starts only once no worker is live (`src/scheduler.rs:440-443`), but a `blocked` slice still holds its lane (`src/docs/board.rs:322-326`). Polish is one worker in the main tree (`src/closeout.rs:164`, `:245`), and `skills/polish/SKILL.md` §5 (`:101-113`) carries the fixer rules: a lane per fixer when lanes are declared, no suite in a fixer with `lanes: 1`, wait out a sibling's half-edit. No driver gate runs after polish.

**Decisions, 2026-10-13:**

- **The driver works in the lowest-numbered free lane**, reset to HEAD. The interview chose "lane 1 reset to HEAD" to avoid an extra worktree. Refined at drafting the same day: a blocked slice keeps its lane and its unlanded edits, so the driver takes the lowest lane that no `building` or `blocked` slice holds, lane 1 when free. No free lane: the checkpoint waits, and the run is already parked on those blocks. *Cost:* the gate runs with that lane's env (its database, its ports), not the owner's dev setup.
- **The gate runs in that lane**, cwd the lane, with the lane's env. The cache key is the tree the gate ran on: HEAD's tree without `docs/`. A dirty main tree no longer pins it.
- **Fix, checkpoint, polish and CI-fix workers run in that lane**, and their result lands as FR-3. After a fix lands, the gate runs again in the lane reset to the new HEAD.
- **Polish fixers run one after another** inside the polish worker's lane. `skills/polish/SKILL.md` §5 drops the lane, `lanes: 1` and sibling-wait rules and keeps the rest: one fixer at a time, each runs the tests it touches, the full gate once at the end, each fixer logs its own decisions. *Cost:* a long Fix list takes longer than a parallel split. Parallel fixers are idea `2026-10-14-parallel-polish-fixers-each-in-its-own-lane`.
- **No driver gate after polish.** Nothing else is live, so the pick lands the tree the polish worker's last gate ran on, less any protected path it wrote (FR-3). CI stays the outside gate.

**Acceptance:**

- The fixture's `gate.sh` records `pwd` and `git rev-parse HEAD`: at every checkpoint they are a lane path and the feature tip.
- An untracked file only in the main tree, which `gate.sh` needs to pass, no longer turns the checkpoint gate green: the gate goes red, because it ran on HEAD.
- With lane 1 held by a blocked slice, the checkpoint gate's recorded `pwd` is lane 2.
- The polish stub's recorded cwd is a lane, and its edits land as one `fix({NNN}): polish review fixes` commit.
- `skills/polish/SKILL.md` §5 holds no rule naming lanes or `lanes: 1`.

## 8. FR-6 — The guessing is deleted

**Today.** `Pathspec::from_stream` (`src/commit.rs:87-123`, with `heredoc_paths` `:151`, `added_paths` `:178`, `real_dir` `:232`); `scheduler::unnamed` (`src/scheduler.rs:1061-1137`) and `leftovers` (`:1142-1164`); spawn baselines in `runs/<id>/untracked-before/<key>.json` (`:871-919`); polish and CI-fix baselines (`src/closeout.rs:239`, `:1646`, `:1852`, `:1885`) and their commits (`:398`, `:447`, `:1736`); `note_leftovers` (`src/checkpoint.rs:735`).

**Decision, 2026-10-13:** all of it goes, with its tests (the pathspec and leftover tests in `tests/commits.rs`, and the unnamed-file test at `tests/closeout.rs:1801`), replaced by FR-3. The contract lines about claiming a file by `git add` (`assets/contract-slice.md:20`, `assets/contract-polish.md:19`, and the fix and CI-fix twins) go with it. *Cost:* none beyond FR-3's. Idea `2026-10-13-slice-commits-drop-files-the-worker-claimed-by-dir` is resolved by this PRD and dropped with it.

**Acceptance:** `grep -rnE "from_stream|fn unnamed|fn leftovers|untracked-before" src` finds nothing, and the gate passes.

## 9. FR-7 — A checkpoint covers exactly what it gated

**Today.** `unverified` is every done slice numbered above the highest `after` of any checkpoint entry (`src/docs/board.rs:332-346`): a high-water mark by slice number. PRD-004 CP-1 ran "after slice-05" and covered 01, 02, 04, 05; slice 03 finished later, 3 is below 5, so no checkpoint ever covered it. The checkpoint worker writes the entry (`assets/contract-checkpoint.md:15`, `--slice {NN}`, the covered list only in the body's prose).

> *Corrected 2026-10-13.* The brief named `src/checkpoint.rs:87` as the cause. That line only copies `board.unverified` (`let covers = board.unverified.clone();`); the high-water rule in `board.rs` is the bug.

**Decision, 2026-10-13:** a checkpoint entry records the slices it covered, in a `covers` field written by `factory docs log add --kind checkpoint --covers 1,2,4,5` (the contract's step 3 passes `{covers}`). `unverified` becomes every done slice that no entry covers. An entry without `covers`, which is every log written before this ships, falls back to its `after` high-water mark. *Cost:* one log field and one CLI flag; old logs keep the old rule.

**Acceptance:** a board test with CP-1 covering 1, 2, 4, 5 and slice 3 done afterwards: `factory docs status --json` lists 3 as unverified, and `checkpoint_due` is true once every slice is done. PRD-004's own `log.jsonl`, read as a fixture, computes the same board as before.

## 10. FR-8 — RETIRED (2026-10-13): `docs/project.toml` is protected

**Today.** `protected_files` (`src/commit.rs:33-35`) names the feature's `prd.md` and `docs/index.md`. Slice, fix, checkpoint and CI-fix commits strip both (`src/scheduler.rs:970`, `src/checkpoint.rs:688`, `:787`, `src/closeout.rs:1748`). The polish commit strips `docs/index.md` only and keeps `prd.md` (`src/closeout.rs:436-441`), which autonomy § May allows ("`prd.md` Status prose", `assets/autonomy.md:43`). Only a slice emits `WorkerWroteProtectedFile` (`src/scheduler.rs:971`; the event carries a `slice`, `src/events.rs:159-162`); the others print a line. Under `git add -A`, a worker that edits `docs/project.toml` would land a changed gate, inner loop or lane count.

> *Corrected 2026-10-13, adversarial pass.* An earlier draft said every commit path strips both files and emits the event. Polish keeps `prd.md`, and only slices emit.

> *Corrected 2026-10-13.* Idea `2026-10-13-protect-docs-project-toml-from-slice-workers-in-v3` assumed a preflight worker that writes `inner`, needing an exception. v3 has no such worker (job kinds at `src/worker.rs:192-210`), and autonomy § May (`assets/autonomy.md:43`) does not mention `inner`. No worker needs an exception.

**Retired 2026-10-13, owner's call at the final cut:** no incident has shown a worker editing `docs/project.toml`, only the idea. The drafted shape was one more entry in `protected_files`, dropped at landing per role like the others. The recon above went into idea `2026-10-13-protect-docs-project-toml-from-slice-workers-in-v3`, which stays open. See §16.

## 11. FR-9 — The `/lanes` skill and `factory lanes up --prove`

**Today.** `factory docs lanes [set N] [--env] [--setup]` writes lane files from a template (`src/docs/lanes.rs`), and nothing interviews a repo into them. This repo's four files hold only `CARGO_TARGET_DIR=target/lane-N` and shared-tree prose.

**Decisions, 2026-10-13:**

- **A new `/lanes` skill** (`skills/lanes/SKILL.md`, in the bundle) picks each repo's lane strategy. It reads the stack (database, ports, cache, queue, Redis DB, scratch paths, heavy ignored state) and writes `[lanes] seed` and `docs/reference/lanes/lane-N.md` files with literal values (`DB_DATABASE=app_lane_2`) and a `setup:` that creates and migrates the lane's database. Secrets stay in `.env`, which is seeded. It ends with `factory lanes up --prove` and does not finish on red. *Cost:* one recipe per repo, written once; a version bump and release. *Lost:* templated values derived from the lane number (more machinery for four files), a gitignored `lane-N.local.md` (left to the multi-dev idea).
- **`factory lanes up --prove`** runs `up`, then the project's `inner` command in every lane at once, cwd the lane, with the lane's env. It prints each lane's exit and duration and exits nonzero when any lane is red. The skill, the Laravel live proof and the owner all use the same command. *Lost:* a prose recipe in the skill, which the next agent would improvise differently (the polish lesson in §1).
- **The `factory docs lanes` template** and this repo's lane files drop the shared-tree prose.

**Acceptance:**

- A fixture whose `inner` records its start and end times and sleeps: `factory lanes up --prove` exits 0, prints one line per lane, and the lanes' runs overlap in time. With lane 2's `inner` red, it exits 1 and names lane 2.
- `factory init` in a fresh repo writes `.claude/skills/lanes/SKILL.md`, and the bundle test (`tests/skills_bundle.rs`) passes after the version bump. *Corrected 2026-10-13, adversarial pass:* an earlier draft said the skill "is in `skills/bundle.lock`", which holds only a version and one hash (`skills/bundle.lock:2-3`).
- The Laravel proof is a live run after release (§14), not this FR's acceptance: `/lanes` ships only once the release is out.

## 12. FR-10 — `factory lanes prune`

**Today.** No lane worktree exists, so nothing removes one. The closest thing, `target/lane-1..4` here, has grown to 43G unnoticed.

**Decision, 2026-10-13 (owner's call, no incident behind it):** `factory lanes prune` removes the lane worktrees numbered above the declared `lanes`, worktree and directory. `--all` removes every lane of this repo. Both run `git worktree prune`. It refuses while this repo has an unfinished run (live or parked), naming the run, because a parked run's blocked slice holds its lane. *Cost:* a verb and its tests; a pruned lane loses its seeded state, and the next `up` seeds it again.

**Acceptance:** a fixture with `lanes = 3`: `up`, then `lanes = 2`, then `prune` removes `lane-3`'s directory and its `git worktree list` row and leaves lanes 1 and 2 untouched. With an unfinished run, `prune` exits nonzero, names the run and removes nothing.

## 13. Decisions log

No question is open. The idea's and the brief's questions, settled at the interview:

- ~~**OPEN-1** — Who writes slice status from a lane?~~ **Settled 2026-10-13:** workers keep calling `factory docs set` and `log add`; an env var points them at the main tree's board (FR-2).
- ~~**OPEN-2** — Rebase, cherry-pick or merge, and when is a conflict a BLOCK?~~ **Settled 2026-10-13:** cherry-pick, one commit per slice; conflict → merge worker in the lane, which may touch anything; BLOCK on markers left, three attempts on the same failure, two deaths, or a third conflict (FR-3, FR-4).
- ~~**OPEN-3** — The non-clone fallback, disk growth, cleanup?~~ **Settled 2026-10-13:** reflink on Linux, clonefile on macOS, plain copy otherwise (FR-1); `factory lanes prune` (FR-10).
- ~~**OPEN-4** — What does a lane file commit?~~ **Settled 2026-10-13:** literal per-lane values, committed; secrets stay in the seeded `.env`; machine-specific values go to the multi-dev idea (FR-9, §16).
- ~~**OPEN-5** — Is the `/lanes` skill in?~~ **Settled 2026-10-13:** in, with `factory lanes up --prove` (FR-9).
- ~~**OPEN-6** — Fold in checkpoint scope?~~ **Settled 2026-10-13:** in (FR-7).
- ~~**OPEN-7** — Fold in `docs/project.toml` protection?~~ **Settled 2026-10-13:** drafted as FR-8, cut at the final review; it stays an idea (§10, §16).
- ~~**OPEN-8** — Knock-ons: transcript slug, gate cache key, `factory ctx`, hook paths?~~ **Settled 2026-10-13:** slug by worker cwd, cache key by the gated tree, ctx unchanged, `FACTORY_REPO` set to the lane (FR-2, FR-5).
- ~~**OPEN-9** — Where do driver gates and closeout workers run?~~ **Settled 2026-10-13:** the lowest free lane reset to HEAD; polish fixers one after another there (FR-5).

## 14. QA plan

**Gate**: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` *(declared in project.toml)*
**Surface**: CLI (`factory run`, `factory lanes up|prune`, `factory status`, `factory report`) and the git history the run writes.
**Harness**: the CLI test harness, `tests/harness/mod.rs` (fixture repo, `lanes = 2`, `./gate.sh`) and the stub worker `tests/stubs/claude` (records each call's cwd and env to `calls/N.json`). **It needs a harness slice, first in the order.** The fixture steers `gate.sh` and the lane setups through `scenario/` (`tests/fixture/gate.sh:8-22`, `tests/lanes.rs:210,1136`), which is gitignored (`tests/harness/mod.rs:209`). So it is missing from every lane worktree: a scripted red gate goes green and a lane setup fails. Seeding it does not help, because a seed path is copied only once (FR-1). The harness slice reaches `scenario/` by an absolute path, adds a merge role and a conflict scenario, and makes `gate.sh` record `pwd` and HEAD. *Cost:* one slice before any FR slice. *Corrected 2026-10-13, adversarial pass:* an earlier draft said "no new harness slice".
**Seed state**: a fixture repo per test.

| FR | Proof on the real surface | Evidence | Human? |
|---|---|---|---|
| FR-1 | run `factory lanes up` on a fixture with two lanes and a seed list; then `factory run` without `[lanes] seed` | `cargo test` transcript; `git worktree list`; the refusal line | no |
| FR-2 | run a two-lane harness run; read each `calls/N.json` and the main tree's board; run a polish stub with `subagents:` and read `factory report`; feed `factory hook` a Read of a pack-held file in a lane | `cargo test` transcript; hook output | no |
| FR-3 | run a harness slice that edits only through Bash; check out its landed commit alone and compare its files with the lane's; run a dependent slice | `cargo test` transcript; `git show --stat` | no |
| FR-4 | run the two-lane conflict scenario, once resolved and once with markers left | `cargo test` transcript; `factory status` | no |
| FR-5 | run a harness checkpoint with `gate.sh` recording `pwd`, a main-tree-only file, and lane 1 blocked | `cargo test` transcript | no |
| FR-6 | run the grep in §8 and the gate | command output | no |
| FR-7 | run the board test and the PRD-004 log fixture | `cargo test` transcript | no |
| FR-8 | retired (§10) | none | no |
| FR-9 | run `factory lanes up --prove` on the overlap fixture; run `factory init` in a fresh repo | `cargo test` transcript | no |
| FR-10 | run `prune` after shrinking `lanes`, and with an unfinished run | `cargo test` transcript | no |

**Live runs, after release, owner-started.** The PR is done when the harness proves FR-1 to FR-7, FR-9 and FR-10. The live runs are the retro's evidence, not the run's acceptance, because `/lanes` ships only with the release and a worker may not write `[lanes] seed` (`docs/project.toml` is not in autonomy § May, `assets/autonomy.md:43`).

- **Here.** Run `/lanes`. Seeding `target/` into four lanes grows `df` used space on `/home` by under 1 GB, measured right after `factory lanes up`, before any build. Then the next `/deliver` with four lanes: every slice commit, checked out alone, passes the gate; the main tree is clean at the end; no line about unnamed or leftover files.
- **Laravel** (icg-cloud or arca-api, database server running). Run `/lanes`. `factory lanes up --prove` runs the suite in two lanes at once, both green, each lane file naming its own `DB_DATABASE`. Then a real `/deliver` with two lanes: no cross-lane database failure in any transcript.

**Who builds it.** A `factory` run builds every FR unattended; no FR needs the owner mid-run. The owner reviews the `/lanes` skill text and the live runs afterwards.

## 15. Slices

Ten slices, built on `feat/isolated-lanes`: one branch, one PR, one commit per slice, Prove at the checkpoints. `factory docs status` is the board.

- **01, the harness for lanes** (§14 Harness). `gate.sh` and the lane setups reach the gitignored `scenario/` from a worktree, `gate.log` records the gate's directory and HEAD, and the stub gains a `merge` role and an `sh` step. Every later slice's acceptance runs on it, so it goes first.
- **02, the board and transcripts from a lane** (FR-2: the board redirect, the transcript slug, the hook). `factory docs set`, `log add`, `retro` and `status` write the main tree's board from a linked worktree, and the metrics read the worker's cwd. Nothing spawns in a lane yet.
- **03, lane worktrees and `factory lanes up`** (FR-1). `src/lanes.rs` owns where a lane lives, seeding it, resetting it and its setup; a fresh run refuses a repo with no `[lanes] seed`.
- **04, the landing library** (FR-3's mechanics). `commit_lane` and `land` turn a lane into one commit on the feature tip, or report a conflict or the owner's dirty paths with the main tree untouched. Nothing calls it yet.
- **05, checkpoint covers** (FR-7). A checkpoint entry names the slices it covered, so a slice that finishes after a higher-numbered one is still checked. Independent of the rest.
- **06, `/lanes`, `--prove`, `prune` and 0.3.0** (FR-9, FR-10, FR-5's polish §5). The new skill, the proving and pruning verbs, the template edit, and the one version bump. It reads 03's lane functions.
- **07, ⚠ the highest risk: build workers in lanes** (FR-2, FR-3). Every build and fix worker spawns in its own lane and lands through 04; the pathspec commit leaves the build path. The spine: it needs 01 to 04, and 08 to 10 build on it.
- **08, the merge worker** (FR-4). A conflicting landing spawns a merge worker in the slice's lane instead of blocking at once.
- **09, the checkpoint in a free lane** (FR-5). Driver gates and the checkpoint worker run in the lowest free lane, reset to HEAD.
- **10, closeout in a lane, and the guessing deleted** (FR-5's polish and CI fix, FR-6, FR-2's polish subagents, FR-9's lane files). Polish and CI fix land from a lane; then the baselines, the unnamed-file handling, the leftovers notes and the `Pathspec` commit are deleted.

**Order.** The installed v0.2.1 driver builds this run: one shared tree, pathspec commits, and the slice-number high-water checkpoint rule that 05 replaces. The numbers follow the expected finish order, so no checkpoint skips a slice. 01 to 04 are independent and fill the four lanes; 05 waits for a free lane. 06 holds every skill edit under one bump, because the bundle lock refuses a second skill change at the same version (`src/skills.rs:176-200`); after it, 07 to 10 edit `assets/` only, which is outside the bundle. 08, 09 and 10 are chained because they edit the same `scheduler.rs`, `worker.rs` and `closeout.rs`, and the old driver commits whole files. 10 runs alone and last: it rewrites this repo's lane files, which the running driver reads.

**Checkpoints** (`checkpoint_every = 2`). The first covers 01 to 04: it falls due after two, and waits for the live workers. The second covers 05, 06 and 07, which build together. The third covers 08 and 09; the last covers 10. 07, the spine, is proven before 08 builds on it.

**No `qa-guide.md`**: §14 has no Human row. The two live runs in §14 come after the merge, when the owner releases 0.3.0 and runs `/lanes` here and in a Laravel repo.

## 16. Non-goals

- **Parallel polish fixers.** Idea `2026-10-14-parallel-polish-fixers-each-in-its-own-lane`.
- **Machine-specific lane values and several developers' lanes.** Idea `2026-10-13-multi-dev-coherence-prove-several-developers-can-s`.
- **Polish decomposed into driver steps, polish collect in the binary.** Ideas `2026-10-09-decompose-polish-into-driver-run-steps`, `2026-10-13-polish-collect-in-the-binary-pick-reviewers-from-t`.
- **Windows.** Out.
- **A shared-tree mode.** Deleted, with no flag to bring it back.
- **`target/lane-1..4` here.** The owner deletes them after the live run; nothing in the binary touches them.
- **Not handled: re-seeding after a lockfile change mid-run.** Accepted 2026-10-13: a slice that changes `composer.lock` or `package-lock.json` runs the install in its lane itself; no incident. Revisit if a run blocks on a stale `vendor/` or `node_modules/` in a lane.
- **Not handled: an abandoned slice's unlanded edits.** Accepted 2026-10-13: a slice holds its lane while `building` or `blocked`, so only an owner's decision frees it, and the next reset discards what never landed. Revisit if an owner loses work this way.
- **Not handled: disk-growth alerts.** Accepted 2026-10-13: `/lanes` picks the seed per repo and `prune` cleans up; no incident. Revisit if a run fails on a full disk.
- **Not handled: a worker landing a change to `docs/project.toml`.** Accepted 2026-10-13, owner's cut: `git add -A` makes it possible, but no run has shown it; autonomy § May does not allow it. Revisit if a landed commit changes the gate, the inner loop or the lane count. Idea `2026-10-13-protect-docs-project-toml-from-slice-workers-in-v3`.
- **Not handled: a driver gate after polish.** Accepted 2026-10-13: the polish worker's last gate ran on the tree that lands (FR-5). Revisit if CI goes red on a polish commit whose lane gate was green.

## 17. Verification notes

Checked against `main` at 7459404, 2026-10-13. **Re-verify at slice entry.**

- `src/commit.rs:1-17` — module doc: commit by pathspec, never `git add -A`; with two or more lanes the slice commit stages the whole `log.jsonl` and `state.toml`.
- `src/commit.rs:33-35` — `protected_files`: `{dir}/prd.md`, `docs/index.md`.
- `src/commit.rs:87-123` — `Pathspec::from_stream`; `heredoc_paths` `:151`, `added_paths` `:178`, `real_dir` `:232` (the 4ece217 symlink fix, `:111-116`).
- `src/commit.rs:379-423` — `commit`: `ls-files` filter, `git add --`, `git commit -- paths`. `:435-451` `branch_refusal`.
- `src/scheduler.rs:420-426` — the driver writes `status=building` and the lane for a ready slice; `:386-388` a respawn keeps its lane.
- `src/scheduler.rs:440-443` — a checkpoint runs only once no worker is live.
- `src/scheduler.rs:687-757` — `spawn_on`: lane setup once per run, spawn baseline at `:732`, `Worker::spawn` with the lane env.
- `src/scheduler.rs:789-864` — `route_exit` routes on the board's slice status.
- `src/scheduler.rs:871-919` — spawn baselines; `:958-1056` `commit_slice`; `:1061-1137` `unnamed`; `:1142-1164` `leftovers`.
- `src/docs/board.rs:306` — `ready` follows the dependencies' `done`. `:322-326` — a `building` or `blocked` slice holds its lane.
- `assets/contract-slice.md:19` — the build worker sets its slice `done` before it exits.
- `src/docs/board.rs:332-346` — `unverified` is a high-water mark over checkpoint `after`.
- `src/checkpoint.rs:87` — `covers` copies `board.unverified`; not the bug (§9).
- `src/checkpoint.rs:507-508` — the gate runs `sh -c` in the main tree with `FACTORY_LANE` removed; `:536` the dirty-tree cache pin; checkpoint worker `:279` and fix worker `:600` spawn with `&[]`.
- `src/gatecache.rs:82-110` — cache key `"{tree_id} {gate}"`, tree id from a temp index over the main tree excluding `docs`.
- `src/closeout.rs:436-441` — the polish commit keeps `prd.md`, strips `docs/index.md`. `src/scheduler.rs:971` — only a slice emits `WorkerWroteProtectedFile`.
- `src/docs/retro.rs:3,21-31` — `docs retro --set` keys the metrics row by the canonical repo root.
- `tests/fixture/gate.sh:8-23`, `tests/harness/mod.rs:209` — the fixture gate reads the gitignored `scenario/`.
- `src/closeout.rs:164`, `:239`, `:245`, `:398`, `:447` — polish: one worker, baseline, `&[]` env, `commit_polish`, `unnamed(.., true)`. CI fix `:1579`, `:1646`, `:1736`, `:1756`; orphan baselines `:1852`, `:1885`.
- `src/worker.rs:192-210` — job kinds: Build, Fix, CiFix, Checkpoint, Polish; no preflight worker.
- `src/worker.rs:280,294,298-322` — cwd is the main tree for every role; env scrub, lane env, `FACTORY_PACK`, `FACTORY_REPO`.
- `src/lanes.rs:27-28,46-87,92-95,105-109` — lane file path, `env:`/`setup:` parse, `FACTORY_LANE` first, setup in the main tree.
- `src/measure.rs:282-291` — transcript slug from the main tree's path; callers `src/metrics.rs:515`, `src/cli/report.rs:929`; stub `tests/stubs/claude:603,674`.
- `src/cli/hook.rs:893-898` — repo from `FACTORY_REPO`; guards at `:104-112`, `:207`, `:457-500`, `:797-802`.
- `src/cli/docs.rs:220` — `factory docs` finds the repo from cwd.
- `src/docs/state.rs:124-133` — `docs.lock` per feature dir; `src/docs/scaffold.rs:219-246` number allocation through the common git dir.
- `src/paths.rs:20-26` — data dir. `src/run.rs:608-612` — runs sit flat under `runs/<id>` and are found by comparing the canonical repo path; no path-safe repo key exists.
- `src/ctx/mod.rs:4-5`, `src/main.rs:78` — `ctx` keeps no index; base is cwd.
- `assets/contract-slice.md:14` — the inner loop is the worker's own command (pack `# Inner loop`, `src/pack.rs:236`). *Corrected 2026-10-13:* the brief put "the inner loop" in a gate worktree of HEAD; it runs in the worker's lane, which is already the tip plus its own changes. Only driver gates move to a lane at HEAD.
- `assets/contract-slice.md:20`, `assets/contract-polish.md:19` — today's "claim a file with `git add`" rule.
- `assets/contract-checkpoint.md:15` — the checkpoint entry: `--slice {NN}`, covers in prose.
- `assets/autonomy.md:43` — § May: no `inner`, no `project.toml`.
- `skills/polish/SKILL.md:101-113` — §5 fixer rules.
- `docs/features/001-driver/prd.md:69` — "in the same working tree" (PRD-001 FR-3).
- `tests/harness/mod.rs:127-225`, `tests/stubs/claude:15-58` — fixture and stub steps. Tests asserting cwd is the fixture: `tests/loop.rs:228-230`, `tests/worker.rs:156`, `tests/stubs.rs:101`. `tests/commits.rs` holds the pathspec and leftover tests FR-6 removes.
- Environment: git 2.55.0; `/home` is btrfs; `target/` 50G, of which `target/lane-1..4` 43G. *Corrected 2026-10-13:* the idea said 17G.
