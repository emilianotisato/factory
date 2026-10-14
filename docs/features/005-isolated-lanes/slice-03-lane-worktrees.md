# Slice 03 — Lane worktrees and `lanes up`

**Source**: [`prd.md`](prd.md) → §3 FR-1 (lane worktrees, `[lanes] seed`, reset, setup in the lane, `factory lanes up`, the preflight refusal), §13 OPEN-3. **Depends on**: nothing. It builds the lanes; slice 07 is the first to spawn a worker in one, and slice 06 adds `--prove` and `prune` to the verb made here.
**Created**: 2026-10-14

**Progress**:

- **Built.** `src/lanes.rs`: `main_tree` (parent of the absolute git common dir, canonical), `key` (`<name>-<8 hex of sha256(path)>`), `dir(data, main, n)`, `ensure` (detached `git worktree add` from the main tree's HEAD when the lane is not a registered worktree, `-f` when git still lists a vanished one; then each seed path as `clone`, `copy`, `kept` or `skipped (not in the main tree)`, a failed clone's partial target removed before the copy), `reset` (forced detached checkout, `reset --hard` to drop `CHERRY_PICK_HEAD`, `clean -fd`), `Lane::run_setup` (setup's raw output; `setup` now wraps it), `NO_SEED`. `src/docs/project.rs::seed` reads `[lanes] seed` as `Option<Vec<String>>`; `"lanes"` joins `PROJECT_TOP_KEYS`. `src/cli/lanes.rs`: `factory lanes up` and `ensure_all`, which a fresh `factory run` calls after the refusals and before any run exists (the ask path included). Preflight refuses with `NO_SEED` next to the gate refusal; the lanes ask names `/lanes`. Harness: `build_v3` writes `[lanes]\nseed = []`; `set_seed` and `drop_lanes_table` rewrite and commit the table.
- **Tests.** Unit: same lane dir from the main tree and a linked worktree, twin `app` repos differ; `ensure` twice → one detached lane, seeds byte-equal, labels match a `cp --reflink=always` probe, `kept` the second time with a lane-grown file left alone, `missing` skipped, `../out` refused; `reset` over an edit, a staged file, an untracked file and a conflicted cherry-pick keeps the ignored `.env`. Config: `[lanes]` loads with no warning. Project: absent table, absent key, `[]`, `["target"]`. `tests/lanes.rs`: `lanes up` on two lanes (detached, byte-equal seeds, each `setup-pwd` its own canonical path, labels per the probe, `kept` and setup again on a rerun); `lanes up` and `run` without a seed exit 1 naming `/lanes`, no call, no run dir, no lane; a fresh `--foreground` run leaves both lanes and every worker's cwd is the main tree. `"lanes"` in `SUBCOMMANDS`.
- **Inner loop.** 1 run, 0 red, 1.2 min, 545 tests passed. Run in a tree other slices were building in at the same time.
- **Deviations.** `dir` and `ensure` take the data dir as an argument (unit tests can't set `FACTORY_DATA_DIR` safely); the CLI passes `paths::data_dir()`. `main_tree` runs its own `git rev-parse --git-common-dir`: `paths::git_common_dir` was in the tree but uncommitted (another slice's in-flight work), AD logged. A seed path that is absolute or climbs out with `..` is an error naming it.
- **Prove (checkpoint).** FR-1 rows left for the checkpoint: the `lanes up` transcript, `git worktree list` with two detached lanes, and the refusal line with its exit code.
- **Prove (CP-1), FR-1 tests.** The task 5 and task 6 tests green: `evidence/cp-1-fr-1-lanes-tests.txt`.
- **Prove (CP-1), FR-1 drive.** `lanes up` prints one line per lane (`big copy · .env copy  setup 0` on tmpfs, `kept` on rerun). `git worktree list` shows two detached lanes. With no `[lanes] seed`, `lanes up` and `run 001` print `✗ no lanes declared — /lanes …` and exit 1, with no run dir: `evidence/cp-1-fr-1-lanes-up-drive.txt`.

**Slice**: After this slice a repo has real lanes. Lane N is a detached git worktree at `<data dir>/lanes/<repo key>/lane-N`, and the ignored state `[lanes] seed` names is cloned into it once, or copied when a clone fails. `src/lanes.rs` owns the whole lane: where it lives, creating and seeding it, resetting it to a commit, and running its setup there. `factory lanes up` creates and seeds every lane, runs each setup inside its lane, and prints one line per lane. A fresh `factory run` refuses a repo with no `[lanes] seed` and names `/lanes`, and creates and seeds the lanes before it starts. Workers still run in the main tree: slice 07 moves them, and the reset is first called there.

## Current code / environment

- **No code creates a worktree** (prd §3): `git worktree` appears only in `tests/docs_v3.rs:238-264`. A lane today is an env list and a setup command: `src/lanes.rs:19-24` `Lane { n, env, setup }`, `file` `:27`, `load` `:32`, `parse` `:46`, `env()` with `FACTORY_LANE` first `:92-96`, and `setup(&self, repo)` `:100-109`, which runs `sh -c` with `current_dir(repo)`. Setup already takes a directory; a lane path is a valid one. The module doc (`:1-7`) still says the count lives in `docs/index.md`: it is `[docs] lanes` in `docs/project.toml` now. The unit test at `:183` covers `parse`.
- **The data dir**: `paths::data_dir()` (`src/paths.rs:20-26`) honors `FACTORY_DATA_DIR`, which the harness sets per fixture (`tests/harness/mod.rs:555`). So test lanes land under the fixture's tempdir.
- **No repo key exists**: runs sit flat under `runs/<id>` and are matched by comparing the canonical path (`src/run.rs:608-612`; prd §3, corrected by the adversarial pass). The key is new: the main tree dir's name, `-`, and the first 8 hex chars of the SHA-256 of its canonical path (`sha2` is already a dependency).
- **Trap T2, `repo_root` inside a lane is the lane**: `paths::repo_root` (`src/paths.rs:114-133`) runs `--show-toplevel`. Run from a lane, every key and lane path computed from it would be the lane's own. The main tree is the parent of `git rev-parse --path-format=absolute --git-common-dir`; slice 02 may already add a helper for the common dir, use it if so.
- **Trap T1, `[lanes]` is an unknown key today**: `PROJECT_TOP_KEYS` (`src/config.rs:141`) is `["docs", "run", "workers"]`; any other table prints `unknown key `lanes` ignored` (`:207`, `warn` `:345`). The test at `:443` covers the warning. `src/docs/project.rs` reads `[docs]` only (`:43-54`), and `docs set`/`edit` touch `[docs]` only (`:93`, `:110-155`).
- **The preflight refusals**: `src/docs/preflight.rs:302-308` refuses with no gate and names `/gate`; any refusal makes the exit 1 (`:232-238`). The lanes ask at `:348-358` names `factory docs lanes`; `tests/docs_lifecycle.rs:201-204` asserts it contains `lanes = 1`. `factory run` prints the refusals and exits 1 at `src/cli/run.rs:109-116`, before `Run::create` (`:118`).
- **Trap T4, a resume skips preflight**: `src/cli/run.rs:79-107` resumes a run without calling `docs.preflight`. Lanes made here can vanish before a resume (a `prune`, a deleted data dir). Slice 07 ensures the lane at each spawn; this slice only does the fresh path.
- **The CLI pattern**: top-level verbs with subcommands follow `Skills { verb }` (`src/main.rs:48-52`, dispatch `:54-66`), each in `src/cli/<name>.rs` listed in `src/cli/mod.rs:4-15`. `tests/cli.rs:12-14` `SUBCOMMANDS` lists every verb `--help` must show.
- **Trap T5, every run test goes red without a seed**: `build_v3` (`tests/harness/mod.rs:188-225`) writes `[docs]` only, from `extra` pairs (`with_index` `:85` passes `[docs]` keys). Once preflight refuses without `[lanes] seed`, 69 `["run"…` call sites in 12 test files refuse. `build_v3` must write `[lanes]\nseed = []`, and the harness needs a helper to replace the `[lanes]` table and commit it.
- **Test tempdirs cannot clone here or on CI**: `/tmp` is tmpfs on this machine, CI runners use ext4. `cp --reflink=always` fails on both, so a test that expects `clone` is red on every machine but a btrfs or APFS tempdir. No env knob to fake it: the test probes.
- **Deliberate non-changes**: *(prd §16 "Not handled: re-seeding after a lockfile change mid-run. Accepted 2026-10-13: a slice that changes `composer.lock` or `package-lock.json` runs the install in its lane itself; no incident." Do not re-seed a path the lane already has.)* *(prd §16 "Not handled: an abandoned slice's unlanded edits. … the next reset discards what never landed." The reset does not save anything first.)* *(prd §16 "Not handled: disk-growth alerts." No size check.)* *(prd §3 "No shared-tree fallback.")* The lane count stays `[docs] lanes`.

## Tasks

### 1. Where a lane lives, and making it — high — blocked by: —
**Where**: `src/lanes.rs` (new functions; the module doc), `src/paths.rs` if the common-dir helper is not there yet.

**Source**: §3 FR-1: "Lane N is a detached git worktree at `<data dir>/lanes/<repo key>/lane-N`. … The repo key is new and path-safe: the repo directory's name plus a short hash of its canonical path." "Detached, so no branch is locked and the feature branch stays checked out in the main tree."

- `main_tree(from) -> PathBuf`: the parent of the absolute git common dir, canonical. Right from the main tree and from any lane.
- `dir(main, n) -> PathBuf`: `<data dir>/lanes/<name>-<8 hex>/lane-<n>`.
- `ensure(main, n, seed) -> Report`: `git worktree add --detach <dir> HEAD` from the main tree when the dir is not a worktree of this repo, then the seed (task 3). An existing lane is kept as it is.

**Acceptance:** unit tests in `src/lanes.rs` on a temp repo: `dir` called from the main tree and from a linked worktree gives the same path; two repos named `app` in different parents get different keys; `ensure` twice leaves one worktree in `git worktree list --porcelain`, `detached`.

### 2. `[lanes] seed` in `docs/project.toml` — high — blocked by: —
**Where**: `src/config.rs:141` (`PROJECT_TOP_KEYS`), `src/docs/project.rs` (a reader), `tests/harness/mod.rs` (`build_v3`, a helper).

**Source**: §3 FR-1: "`[lanes] seed = [...]` in `docs/project.toml` lists repo-relative paths of ignored state to copy into each lane (`target`, `vendor`, `node_modules`, `.env`)." "`seed = []` is a valid declaration."

Add `"lanes"` to `PROJECT_TOP_KEYS`. In `src/docs/project.rs`, read `[lanes] seed` as `Option<Vec<String>>`: `None` when the table or the key is absent, `Some(vec![])` for `seed = []`. `build_v3` appends `[lanes]\nseed = []\n` after the `[docs]` table. A harness helper, e.g. `fx.set_seed(&["big", ".env"])`, rewrites the `[lanes]` table and commits `docs/project.toml`.

**Acceptance:** a unit test in `src/config.rs`: a project file with `[lanes]\nseed = []` loads with no warning (next to the test at `:443`). `src/docs/project.rs`: absent table, absent key, `[]` and `["target"]` read as `None`, `None`, empty, one path.

### 3. Seed by clone, fall back to copy — high — blocked by: 1
**Where**: `src/lanes.rs`.

**Source**: §3 FR-1: "A path is copied only when the lane lacks it, so a lane keeps its own build state across runs. Clone first: `cp --reflink=always` on Linux, `cp -c` (clonefile) on macOS; a failed clone falls back to a plain copy and says so. A seed path missing in the main tree is skipped with a line." §13 OPEN-3.

For each seed path: missing in the main tree → `skipped (not in the main tree)`. Present in the lane → `kept`. Else the clone command for the OS (`cp --reflink=always -a` on Linux, `cp -c -R -p` on macOS) into the lane at the same relative path, creating parents. A nonzero clone removes any partial target and runs a plain recursive copy (`cp -a` / `cp -R -p`); its label is `copy`. A failed copy is an error naming the path. Return each path's label for the caller to print.

**The trap**: a partial clone left behind would make the next call print `kept` for half a directory. Remove the target before falling back.

**Acceptance:** in a test, a main tree with an ignored `big/` dir (two files, one nested) and an ignored `.env`: after `ensure` both are byte-equal in the lane, and each label is `clone` when a probe `cp --reflink=always` of a scratch file in the same tempdir succeeds, else `copy`. A second `ensure` labels both `kept` and leaves a file added inside the lane's `big/` alone. A seed `missing` prints the skip line and no error.

### 4. Reset a lane to a commit — high — blocked by: 1
**Where**: `src/lanes.rs`.

**Source**: §3 FR-1: "A slice taking a free lane resets it to the feature tip: detached checkout, forced, and untracked files that are not ignored removed. Ignored seeded state stays."

`reset(lane_dir, rev)`: a forced detached checkout of `rev`, then removing untracked files and dirs that are not ignored (`git clean -fd`, never `-x`). It must also work on a lane left mid-cherry-pick or with staged changes: no `CHERRY_PICK_HEAD` survives it. Slice 07 calls it; nothing here does.

**Acceptance:** a unit test: a lane with a tracked file edited, a staged new file, an untracked file, an ignored seeded file and an in-progress conflicted `git cherry-pick`; after `reset(lane, main HEAD)` the lane's HEAD is that commit, detached, `git status --porcelain` is empty, `git rev-parse -q --verify CHERRY_PICK_HEAD` fails, and the ignored file is still there.

### 5. `factory lanes up` — high — blocked by: 1, 2, 3
**Where**: `src/cli/lanes.rs` (new), `src/cli/mod.rs`, `src/main.rs` (a `Lanes { verb }` command like `Skills`), `tests/cli.rs:12-14`, `src/lanes.rs` (setup).

**Source**: §3 FR-1: "`factory lanes up` creates the missing worktrees `1..=lanes`, seeds them, runs setup, and prints one line per lane: its path, `clone` or `copy` per seed path, the setup exit. Nonzero on any failure." "Setup runs inside the lane, cwd the lane, with the lane's env. … `factory lanes up` runs it in every lane each time it is called."

`/// Create, seed and set up this repo's lanes` `Lanes { verb: LanesVerb }` with `Up`. It reads `[docs] lanes` (default 1) and `[lanes] seed` from the main tree, refusing with exit 1 and the `/lanes` line when `seed` is absent. Per lane: `ensure`, then the lane file loaded from the main tree (`lanes::load(main, n)`) and its `setup` run with cwd the lane. One line per lane: `lane-N  <path>  big clone · .env copy  setup 0` (or `setup -` when none is declared). Any failure still prints the other lanes, then exits 1.

**Acceptance:** `tests/lanes.rs`, through the binary: the fixture with `lanes = 2`, `fx.set_seed(&["big", ".env"])` with both ignored and present, and lane files whose `setup:` is `pwd -P > setup-pwd`: `factory lanes up` exits 0; `git worktree list --porcelain` shows two `detached` worktrees under `$FACTORY_DATA_DIR/lanes/`; each holds `big/` and `.env` byte-equal to the main tree's; each lane's `setup-pwd` holds its own canonical path; each line says `clone` or `copy` as the probe predicts. `"lanes"` is in `SUBCOMMANDS`.

### 6. The run refuses without a seed, and makes the lanes — high — blocked by: 2, 3
**Where**: `src/docs/preflight.rs` (`:302-308`, `:348-358`), `src/cli/run.rs` (after `:109-116`), `tests/docs_lifecycle.rs`.

**Source**: §3 FR-1: "Preflight refuses when `docs/project.toml` has no `[lanes] seed` key, and names `/lanes`, as it does for a missing gate." "The driver's preflight runs the create and seed half only, so a fresh run does not run setup twice." Acceptance: "`factory run` in a fixture without `[lanes] seed` exits nonzero before any spawn, and the message names `/lanes`."

In `preflight::run`, v3 arm, next to the gate refusal: no `[lanes] seed` → refuse `no lanes declared — /lanes picks the seed list and writes the lane files (an interview, never unattended)`. The lanes ask at `:351` names `/lanes` in place of `factory docs lanes` and keeps `lanes = {lanes}` in its text. In `src/cli/run.rs`, on a fresh run, after the refusals and before `Run::create`: `ensure` every lane `1..=lanes`, printing one line each; a failure prints and exits 1 with no run created. `factory docs preflight` itself creates nothing: it is read-only, like doctor.

**Acceptance:** `tests/lanes.rs`: a fixture whose `[lanes]` table is removed and committed: `factory run 001` exits 1, stdout names `/lanes`, no `calls/` file exists, and no run dir is created. `tests/docs_lifecycle.rs` preflight tests stay green with the wording change. After a default-fixture run, `git worktree list` holds both lanes, and the run's workers still record the main tree as cwd (slice 07 moves them).

### 7. Verify — medium — blocked by: 1-6
`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 8. Prove — at the checkpoint — blocked by: 7
Rows from [`prd.md`](prd.md) §14 this slice proves, below the UI:

- **FR-1**: "run `factory lanes up` on a fixture with two lanes and a seed list; then `factory run` without `[lanes] seed`". Run the task 5 and task 6 tests and keep their transcripts, plus the `git worktree list` output from task 5's fixture and the refusal line from task 6.

**Acceptance:** after the checkpoint, Progress holds the `lanes up` output (one line per lane, `clone` or `copy` per path), `git worktree list` with two detached lanes, and the refusal line naming `/lanes` with its exit code.
