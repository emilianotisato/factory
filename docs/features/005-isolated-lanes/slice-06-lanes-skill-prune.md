# Slice 06 — `/lanes`, `--prove`, `prune`, version 0.3.0

**Source**: [`prd.md`](prd.md) → §11 FR-9 (the `/lanes` skill, `factory lanes up --prove`, the template), §12 FR-10 (`factory lanes prune`), §7 FR-5 (polish §5 fixers run one after another), §13 OPEN-5. **Depends on**: 03, which makes `src/lanes.rs`'s worktree functions and the `factory lanes` verb this slice extends. It holds every skill edit of this PRD, because the bundle lock refuses a second skill change under the same version: after this slice, no slice touches `skills/`.
**Created**: 2026-10-14

**Progress**:

- **Built.** `factory lanes up --prove` (`src/cli/lanes.rs` `prove`): `up`, then `sh -c <inner>` in every lane from one thread each, cwd the lane, the lane's env; `lane-N  inner  exit <code>  <secs>s` per lane, a red lane's last 20 output lines under its line, `✗ inner red in lane-2` last. `factory lanes prune [--all]` (`prune`, with `lanes::on_disk`, `remove_lane`, `prune` in `src/lanes.rs`): refuses on `run::list_open`, one `run <id>  feature <f>  <phase>` line per open run. `skills/lanes/SKILL.md` and its `.claude/skills/lanes` link, in `NAMES`/`FILES`; polish §5 one fixer at a time; the template's shared-tree paragraph gone; 0.3.0 and `skills/bundle.lock` rewritten.
- **Tests.** `tests/lanes.rs`: `lanes_up_prove_runs_the_inner_loop_in_every_lane_at_once_and_names_a_red_lane` (overlap at the first run: lane-1 `1791945476.40..1791945477.40`, lane-2 `1791945476.40..1791945477.40`), `lanes_up_prove_stops_at_a_failed_up`, `lanes_prune_removes_the_lanes_above_the_count_and_all_with_all`, `lanes_prune_refuses_while_a_run_is_unfinished_and_removes_nothing` (`✗ prune refused: this repo has an unfinished run, which holds its lanes` / `run 001-20261014-2107  feature 001  waiting-owner`). `tests/skills_bundle.rs` `polish_fixers_run_one_after_another_with_no_lane_rule`; `tests/docs_lifecycle.rs` asserts no `does not isolate`; `tests/init.rs` 20 files, 9 links, `lanes/SKILL.md` written (`:391` held the same 19/8 and moved too).
- **Inner loop.** 2 runs (15 s, then 88 s with `--no-fail-fast`), both red on files this slice does not own: `cargo fmt --check` on `src/scheduler.rs`, and `cargo test` in `checkpoints`, `commits`, `docs_log`, `loop` and `lanes::a_fresh_run_makes_its_lanes_and_workers_still_run_in_the_main_tree`, all slices 05 and 07 mid-edit (workers moving into lanes, `lane_bases`). Clippy green; this slice's files pass `rustfmt --check`; every test this slice wrote or touched is green, unit tests 268/268. One build wait on a sibling's half-edit of `Worker::spawn`.
- **Deviations.** AD on the extra shared-tree prose removed (the `docs lanes set` closing hint, polish §5's "fixers running at once"); AD on claiming `src/cli/mod.rs`'s `pub mod lanes`, which slice 03's commit left out.
- **Prove (CP-2), FR-9.** `lanes up --prove` overlap: lane-1 `1791946731.853..1791946732.859`, lane-2 `1791946731.853..1791946732.859`; the red-lane and failed-up cases green; `factory init` in a fresh repo writes 20 files, 9 links and `lanes/SKILL.md` — evidence: `evidence/cp-2-fr-9-prove-and-init.txt`, `evidence/cp-2-fr-9-init-drive.txt`.
- **Prove (CP-2), FR-10.** `prune` after `lanes = 2` removes lane-3 only, `--all` the rest; with an open run: `✗ prune refused: this repo has an unfinished run, which holds its lanes` / `run 001-20261014-9c13  feature 001  waiting-owner`, every lane kept — evidence: `evidence/cp-2-fr-10-prune.txt`.
- **Prove (CP-2), FR-5 text.** polish §5 holds no `lane` or `sibling`, still names `factory docs log add` and `factory docs gate` — evidence: `evidence/cp-2-fr-5-polish-text.txt`.

**Slice**: After this slice `factory lanes up --prove` runs the project's inner loop in every lane at once and says which lane is red, and `factory lanes prune` removes lanes above the declared count (`--all`: every lane), refusing while a run of this repo is unfinished. A new `/lanes` skill interviews a repo into its seed list and lane files and does not finish until `--prove` is green. Polish §5 loses its shared-tree fixer rules: fixers run one after another. The `factory docs lanes` template drops the shared-tree paragraph. The crate goes to 0.3.0 and the bundle lock is rewritten, so `factory init` ships nine skills.

## Current code / environment

- **The verb**: slice 03 adds `factory lanes up` (`src/cli/lanes.rs`, `Lanes { verb }`) and `src/lanes.rs`'s `main_tree`, `dir`, `ensure`, `reset`. A lane's env is `Lane::env()` (`src/lanes.rs:91-95`), `FACTORY_LANE=N` first; the lane file is `lanes::load(repo, n)` (`:32`).
- **The inner loop**: `Docs::gate_inner` (`src/docs/mod.rs:107`) is what `factory docs gate --inner` prints: the declared `inner`, else the full gate.
- **Open runs**: `run::list_open(data_dir, repo)` (`src/run.rs:599-605`) is every run of the repo whose phase is not over. A run whose driver died, or that parked on an ask, has a phase that is not over, so it counts, which FR-10 wants ("unfinished run (live or parked)"). `tests/run.rs:705-711` makes one: `Fixture::with_index(…, &[("lanes", "1")])`, then `factory run 001` parks on the preflight ask with no driver alive.
- **The bundle**: `src/skills.rs` lists the skills by hand: `NAMES` (`:26-30`, "The eight skills"), `FILES` (`:33-92`, sorted by path), and "eight" again in the module doc (`:2`) and at `:345`. `skills/bundle.lock` holds `version = "0.2.1"` and one hash. `relock` (`:176-200`) refuses a changed bundle unless `Cargo.toml`'s version is above the lock's. `tests/skills_bundle.rs:56-70` requires a `.claude/skills/<name>` link to `../../skills/<name>` for every name, git-tracked. `tests/init.rs:136` counts 19 files, `:141` 8 links, `:215` holds the literal `(19 files, factory.toml)`.
- **Docs that list the skills**: `skills/README.md:7-16` (the table), `README.md:36` ("and `/lanes` (planned)").
- **Polish §5**: `skills/polish/SKILL.md:101-119`. `:105` splits a long Fix list across subagents when file sets do not overlap; `:107` is the lanes rule ("Tests in a fixer depend on lanes … With `lanes: 1`, no test suite runs in a fixer"); `:109` is the sibling-wait rule ("A red result may be a sibling's half-finished edit. Wait and re-run"). `:108` (static analysis on the whole project), `:110` (report what you could not reach) and `:111` (each fixer logs its own decisions) stay.
- **The template**: `src/docs/lanes.rs:14-32`. The "What this lane does not isolate" paragraph (`:26-28`) is the shared-tree prose. Only the v2 fixture (`tests/fixture/v2/.docs/bin/docs.mjs:1498`) holds the same words, and it is a frozen v2 copy: leave it.
- **This repo's own lane files are slice 10's**: `docs/reference/lanes/lane-1..4.md:12-18` say "All lanes edit the same working tree … the driver commits by pathspec". That is true for the v0.2.1 driver building this PRD, and slice 07 may still be building beside this one. Slice 10 runs alone, last, and edits them.
- **Deliberate non-changes**: *(prd §16 "Parallel polish fixers. Idea `2026-10-14-parallel-polish-fixers-each-in-its-own-lane`." Do not give fixers lanes.)* *(prd §16 "Machine-specific lane values and several developers' lanes" is an idea. The skill writes committed literal values only.)* *(prd §16 "Not handled: disk-growth alerts". `prune` removes; it never measures.)* `skills/gate/SKILL.md:61` is stale too, but no FR names it; leave it.

## Tasks

### 1. `factory lanes up --prove` — high — blocked by: —
**Where**: `src/cli/lanes.rs`, `src/lanes.rs`.

**Source**: §11 FR-9: "`factory lanes up --prove` runs `up`, then the project's `inner` command in every lane at once, cwd the lane, with the lane's env. It prints each lane's exit and duration and exits nonzero when any lane is red."

After `up` succeeds, start `sh -c <gate_inner>` in every lane `1..=lanes` at once (one child each, then wait on all), cwd the lane, env the lane's. One line per lane in lane order: `lane-N  inner  exit <code>  <secs>s`. Any nonzero → exit 1 and a last line naming the red lanes. A failed `up` stops before the inner loop.

**Acceptance:** in `tests/lanes.rs`, a fixture with `lanes = 2`, `seed = []` and an `inner` that appends its start and end times (`date +%s.%N`) to `<tempdir>/times-$FACTORY_LANE` around a `sleep 1`: `--prove` exits 0, prints one `exit 0` line per lane, and the two lanes' runs overlap (each start is before the other's end). With an `inner` that exits 1 when `$FACTORY_LANE` is 2: exit 1, lane 2's line says `exit 1`, the last line names lane 2 and not lane 1.

### 2. `factory lanes prune [--all]` — high — blocked by: —
**Where**: `src/cli/lanes.rs`, `src/lanes.rs`.

**Source**: §12 FR-10: "`factory lanes prune` removes the lane worktrees numbered above the declared `lanes`, worktree and directory. `--all` removes every lane of this repo. Both run `git worktree prune`. It refuses while this repo has an unfinished run (live or parked), naming the run".

First, `run::list_open` for the main tree: non-empty → exit 1, a line per open run with its id and feature, nothing removed. Else, for every `lane-N` under this repo's lane dir with `N > lanes` (every one with `--all`): `git worktree remove --force <dir>` from the main tree, then remove the dir if it is still there. Then `git worktree prune`. One line per removed lane; none → say so, exit 0.

**Acceptance:** in `tests/lanes.rs`: `lanes = 3`, `up`, a marker file written into each lane, then `lanes = 2` and `prune`: `lane-3`'s dir is gone and `git worktree list --porcelain` has no row for it; lanes 1 and 2 keep their markers. `--all` then removes both. With an open run (made as `tests/run.rs:705-711` does), `prune` exits nonzero, its output names that run's id, and every lane dir is still there.

### 3. The `/lanes` skill — high — blocked by: —
**Where**: `skills/lanes/SKILL.md` (new), `src/skills.rs` (`NAMES`, `FILES`, the three "eight"s), `.claude/skills/lanes` (a link to `../../skills/lanes`, committed), `skills/README.md` (a table row), `README.md:36`, `tests/init.rs:136`, `:141`, `:215`.

**Source**: §11 FR-9: "A new `/lanes` skill (`skills/lanes/SKILL.md`, in the bundle) picks each repo's lane strategy. It reads the stack (database, ports, cache, queue, Redis DB, scratch paths, heavy ignored state) and writes `[lanes] seed` and `docs/reference/lanes/lane-N.md` files with literal values (`DB_DATABASE=app_lane_2`) and a `setup:` that creates and migrates the lane's database. Secrets stay in `.env`, which is seeded. It ends with `factory lanes up --prove` and does not finish on red." §3 FR-1: preflight "refuses … and names `/lanes`".

Frontmatter like the other skills (`name`, `description`). The body, in the family's numbered-section style: preflight (`factory docs doctor`, `factory docs gate --inner`); read the stack and list what two processes would share; propose the seed list and each lane's literal values with AskUserQuestion, recommendation first; write `[lanes] seed` into `docs/project.toml` and each lane file with `factory docs lanes set N --env … --setup …`; run `factory lanes up --prove`; on red, fix the lane files and run it again; commit. It is attended work: the preflight refusal says "an interview, never unattended", and a `factory` run's workers may not write `docs/project.toml`.

**Acceptance:** `tests/init.rs`: `factory init` in a fresh repo writes `.claude/skills/lanes/SKILL.md`, the manifest has 20 files and 9 links, and the init line says `(20 files, factory.toml)`. `tests/skills_bundle.rs` passes with the new link (after task 6).

### 4. Polish §5: fixers one after another — medium — blocked by: —
**Where**: `skills/polish/SKILL.md:101-119`, `tests/skills_bundle.rs` (a text test).

**Source**: §7 FR-5: "Polish fixers run one after another inside the polish worker's lane. `skills/polish/SKILL.md` §5 drops the lane, `lanes: 1` and sibling-wait rules and keeps the rest: one fixer at a time, each runs the tests it touches, the full gate once at the end, each fixer logs its own decisions."

Rewrite `:105` to: a long Fix list may go to subagents, one at a time, never two at once. Replace `:107` with one line: each fixer runs the tests it touches. Delete `:109`. Keep `:108`, `:110`, `:111` and the gate paragraph.

**Acceptance:** a test in `tests/skills_bundle.rs` reads `skills/polish/SKILL.md` between `## 5. Fix` and `## 6.` and asserts it contains neither `lane` nor `sibling`, and still contains `factory docs log add` and `factory docs gate`.

### 5. The template drops the shared-tree paragraph — medium — blocked by: —
**Where**: `src/docs/lanes.rs:14-32`.

**Source**: §11 FR-9: "The `factory docs lanes` template and this repo's lane files drop the shared-tree prose."

Delete the "What this lane does not isolate" paragraph from `template`. Keep the frontmatter and the two lines before it.

**Acceptance:** a test (unit or `tests/docs_v3.rs`): `factory docs lanes set 2` writes a file that does not contain `does not isolate`.

### 6. Version 0.3.0 and the bundle lock — high — blocked by: 3, 4
**Where**: `Cargo.toml:3`, `Cargo.lock`, `skills/bundle.lock`.

**Source**: §11 FR-9: "*Cost:* … a version bump and release." Acceptance: "the bundle test (`tests/skills_bundle.rs`) passes after the version bump".

`0.2.1` → `0.3.0`: a new skill and a new verb. Then `FACTORY_BUNDLE_WRITE=1 cargo test` rewrites `skills/bundle.lock`; commit it with the rest.

**The trap**: `relock` refuses a changed bundle at the same version (`src/skills.rs:176-200`), so any `skills/` edit after this slice turns the gate red until the next bump. Slices 07 to 10 edit `assets/` only, which is outside the bundle. Do not tag or release: the owner releases after the merge.

**Acceptance:** `skills/bundle.lock` says `version = "0.3.0"`, and `cargo test --test skills_bundle --test init` is green without the env var.

### 7. Verify — medium — blocked by: 1, 2, 3, 4, 5, 6
`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 8. Prove — at the checkpoint — blocked by: 7
Rows from [`prd.md`](prd.md) §14 this slice proves, below the UI:

- **FR-9**: "run `factory lanes up --prove` on the overlap fixture; run `factory init` in a fresh repo" — the task 1 and task 3 tests.
- **FR-10**: "run `prune` after shrinking `lanes`, and with an unfinished run" — the task 2 test.
- **FR-5**, its text row: "`skills/polish/SKILL.md` §5 holds no rule naming lanes or `lanes: 1`" — the task 4 test.

**Acceptance:** after the checkpoint, Progress holds the transcripts of the four tests, with the overlap times and the refusal line naming the run.
