# Slice 04 — Leaks at the checkpoint

**Source**: [`prd.md`](prd.md) → §5 FR-3, "Where it runs" item 1 (the driver's scan, the `# Leaks` section, the contract's Do step; its second acceptance bullet); §7 FR-5 (its run row, rerun on the tree that ships). **Depends on**: 01 (the driver calls the `scan` and the report rendering slice 01 wrote)
**Created**: 2026-10-14

**Progress**: built — the checkpoint scans the lane against the default branch once the gate is green (`leaks_section` in `src/checkpoint.rs`, `closeout::default_branch` now `pub(crate)`); a scan error reads `not run: …` and never reds the checkpoint. `Pack::for_checkpoint` takes the section body and writes `# Leaks` second, the report in a fence under `<!-- factory leaks --base <ref> -->`. The contract reads `# Leaks` third and does **Leaks** first (step 6 now lets the worker touch a hit's file). The stub's `leaks: "fix"` deletes each `path:line` hit bottom-up and each bare path, once each, and names them under **Fixed**/`FIXED:`. `the_checkpoint_worker_reads_the_leaks_and_its_fixes_land_with_the_checkpoint` (tests/checkpoints.rs) seeds the four leaks plus a why, a quirk and an allowlisted `BLOCK-1`, and checks the pack, the CP entry's **Fixed**, the checkpoint commit's deletions and `factory leaks --base main` = `clean` on the final tip. Inner loop: 1 run, 0 red, 1.6 min (plus one targeted run of the new test, green) — caught: nothing. Deviation: none. Prove rows (FR-3 stub row, FR-5 rerun) left for the checkpoint.
- **Checkpoint 2, FR-3 stub row:** task 4's test transcript; the checkpoint pack's `# Leaks` section (the four seeded leaks under `id-comment`, `id-name` and `history`, 5 fix hits; no why, quirk or `BLOCK-1`); CP-1's **Fixed** naming all four; the checkpoint commit's `M`/`D` lines; `factory leaks --base main` = `clean` on the post-polish tip — `docs/features/006-comment-discipline/evidence/cp-2-fr3-stub-checkpoint.txt`.
- **Checkpoint 2, FR-5:** `factory leaks --all` on the feature tip `daea025` with the lane build: `clean`, exit 0; `factory docs doctor`: `leaks          clean`; checkpoint 2's gate green (1 run, 0 red) — `docs/features/006-comment-discipline/evidence/cp-2-fr5-all-and-doctor.txt`.

**Slice**: After this slice, every checkpoint whose gate went green scans the feature's added lines against the default branch, and the checkpoint worker's pack carries the report as `# Leaks`, right after `# Gate transcript`. The worker's contract tells it to fix every `fix` hit in the file it names, verify with the inner loop, and name each one under **Fixed**; the driver commits the lane as it does today. A hit left unfixed shows again at the next checkpoint.

## Current code / environment

- **The gate loop**: `src/checkpoint.rs:104-158`. Each pass resets the lane (`let (lane_dir, lane_def) = scheduler::prepare_lane(run, lane, true, out)?;`, `:109`) and runs `gate_once` in it; green breaks out at `:111-114`, setting `green_log` and `green_cached`. `lane_dir` is local to the loop: scan inside the green branch, before the `break`, or call `prepare_lane` again after it. Lanes are detached worktrees of the main repo (`src/lanes.rs:1,223`), so `main` resolves inside one.
- **The worker loop**: `:240-300`. Each spawn builds its pack with `Pack::for_checkpoint(docs, board, n, &covers, &green_log, green_cached, &packs, spawn)` (`:249-258`), resets the lane again (`:261`), spawns, and after a dead worker sets `green_cached = true; // a respawn reads the same transcript` (`:285`) and loops. A respawn reads the same report: scan once per checkpoint, not once per spawn.
- **The base**: `src/closeout.rs:2235` `fn default_branch(run, docs)` reads the run's preflight `branch.default`, falling back to `"main"`. It is private to `closeout`; make it `pub(crate)` (or move it beside `scheduler`'s helpers) rather than writing a second copy.
- **The pack**: `src/pack.rs:343` `for_checkpoint` assembles its sections at `:408-425`: `# Gate transcript`, `# Covered slices`, `# QA rows`, the log section, `# Contract`, `# Autonomy`. `assemble` (`:68`) names each part from its heading, so `# Leaks` becomes the part `leaks`. Callers: `src/checkpoint.rs:249` and two tests, `src/pack.rs:963` and `:1135`; the second asserts the section list at `:1135-1146`. Slice 02 adds a separate test function in the same module: leave it alone.
- **The contract**: `assets/contract-checkpoint.md`. Read-first lists five items (`:3-8`): Autonomy, Gate transcript, Covered slices, QA rows, Log. Do has five steps (`:12-17`): Prove (which already says "A failing row is a defect: fix it … and name it under **Fixed** — the driver commits your files with the checkpoint and the next gate re-tests them"), Evidence, Record (`**Fixed:** {what, or none}`), no gate or commit, touch nothing else. The return block (`:21-25`) carries `FIXED:`.
- **The lane commit**: `src/land.rs:41,47` stages with `git add -A`, so a deleted line, a deleted file and a renamed folder all land in the checkpoint's commit.
- **The stub**: `tests/stubs/claude` reads a checkpoint pack by its `# Gate transcript` heading (`:141`). Its checkpoint branch (`:238-280`) writes the log entry with `'**Fixed:** none'` (`:273`) and returns `'FIXED: none'` (`:278`); it touches no file. The step keys are documented in the header (`:21-55`); the `sh` key runs a script in the worker's cwd for a slice, fix, polish or merge.
- **The stub's own files are id-shaped**: a slice step with no `touch` key writes `src/slice-NN.txt` (`tests/stubs/claude:445`), and `slice-01` is the `id-name` path shape (`prd-004`'s). Every stub run's scan lists those paths as hits, so a test that counts hits gives each slice step a `touch` that names a behavior (`"touch": ["src/parse.txt"]`), or `"touch": []` with an `sh`, as `tests/commits.rs:378` does. Renaming the stub's default is not this slice's: `tests/commits.rs:236-238,539-542` assert those paths.
- **The test file**: `tests/checkpoints.rs`; `the_driver_runs_the_gate_at_each_checkpoint_and_commits_the_cp_entries` (`:484`) is the pattern: `Fixture::new`, `fx.chain()`, `fx.write_plan(&[...])`, `start(&fx)`, `wait_phase(&fx, &dir, "finished", 60)`, then `pack_of(call)` (`:291`) to read a worker's pack. A fixture run builds on `feat/fixture` off `main` (`tests/closeout.rs:408`). `tests/checkpoints.rs` is an `all` entry on factory's allowlist (slice 03), so the seeded leaks in its strings are data.
- **Deliberate non-changes**: *(prd §11 "Not handled: refusing to run on a base with leaks.")* The scan never fails the checkpoint and never blocks the run. *(prd §5 "No new owner question anywhere.")* A hit is the worker's to fix, never a block.

## Tasks

### 1. The driver scans after a green gate — high — blocked by: —
**Where**: `src/checkpoint.rs:104-158`, `src/closeout.rs:2235`.

**Source**: §5 FR-3, "Where it runs" item 1: "After the gate goes green (`src/checkpoint.rs:125-147`), the driver runs `factory leaks --base <feature base>` and puts the report into the checkpoint worker's pack as a `# Leaks` section, after `# Gate transcript`."

When the gate goes green, call slice 01's `scan` with the lane as the repo and the default branch as the base, and keep the rendered report for every spawn of this checkpoint. The library call is the verb's work without a process: same report, same allowlist. A scan error is not a red: the section then reads `not run: <the error>`, and the checkpoint goes on.

**Acceptance:** task 4's test sees the report in the pack; a unit test renders a scan error as a section reading `not run: …`.

### 2. `# Leaks` in the pack — high — blocked by: 1
**Where**: `src/pack.rs:343-440` and its tests at `:963`, `:1135-1146`.

**Source**: as task 1: "as a `# Leaks` section, after `# Gate transcript`".

`for_checkpoint` takes the report text and pushes `# Leaks` right after `# Gate transcript`, every time, so the pack's shape does not depend on the tree: a clean report is short. Update both test callers and the section list at `:1135-1146` to `gate_transcript, leaks, covered_slices, qa_rows, log, contract, autonomy`.

**Acceptance:** the updated pack test passes and lists `leaks` second.

### 3. The contract's Do step — high — blocked by: —
**Where**: `assets/contract-checkpoint.md`.

**Source**: §5 FR-3: "The contract gains one Do step: fix every `fix` hit in the file it names, verify with the inner-loop command, and list them under **Fixed**. The driver commits the lane as it does today. A hit the worker could not fix is named under **Fixed** as unfixed, and the next checkpoint shows it again." And: "Mechanical hits are fixed by the worker; judgment stays with a reader."

- Read first gains `# Leaks` as item 3, after the gate transcript: the leaks report the driver made, each rule's fix line above its hits.
- Do gains the step, first in the list, so the rows are proven on the cleaned tree: fix every `fix` hit by its rule's fix line, in the file it names, run the inner-loop command, and list each hit under **Fixed** as `leaks: path:line`, or as unfixed with one line on why. The Record step's `**Fixed:**` and the return block's `FIXED:` carry them.

**Acceptance:** read back; task 4's stub run follows it.

### 4. The stub fixes the hits, and a run proves it — high — blocked by: 2, 3
**Where**: `tests/stubs/claude` (header `:21-55`, checkpoint branch `:238-280`), `tests/checkpoints.rs`.

**Source**: §5 FR-3 acceptance: "A stub run with those leaks seeded in a slice: the checkpoint pack holds `# Leaks` with the hits, the stub worker's scenario fixes them and names them under **Fixed**, and polish's run reports zero hits." §9 seed state: "an id comment, a history note, a real why, a real foreign quirk, a test named after an FR, a fixture folder named `prd-9`, and an allowlisted `BLOCK-1` string."

- **A stub key** on a checkpoint step, `leaks: "fix"`: read the `path:line` and bare `path` hits under the pack's `# Leaks` heading; delete each hit line from its file in cwd, bottom-up per file so line numbers hold; delete each hit path; then write `**Fixed:**` and `FIXED:` naming every hit instead of `none`. Document the key in the header beside the polish keys.
- **The test**: a plan whose first slice step carries an `sh` that writes the seeded files (slice 01's task 5 lists them: the id comment, "until slice 4", the FR-named test, `tests/fixture/prd-9/`, a why, a quirk, an allowlisted `BLOCK-1`), and whose checkpoint step is `{"outcome": "checkpoint", "leaks": "fix"}`. Every slice step names its `touch` (see Current code), or the stub's `src/slice-NN.txt` paths join the hits and the last slice's file outlives the last fix. The `[leaks]` entry for `BLOCK-1`'s file goes into the fixture's `docs/project.toml` and is committed on `main` before the run starts, the way the fixture's `[lanes]` table is (`tests/harness/mod.rs:112`). Run to `finished`.

"Polish's run reports zero hits": the stub polish runs no command, so the test runs `factory leaks --base main` itself in the fixture on the run's final tip. That is what a real polish runs (slice 02's § 1 Collect), on the same tree.

**Acceptance:** the checkpoint pack holds `# Leaks` with the four planted hits under their rules and not the why, the quirk or `BLOCK-1`; the checkpoint log entry's `**Fixed:**` names the four; the checkpoint's commit deletes them; `factory leaks --base main` on the final tip exits 0 and prints `clean`.

### 5. Verify — medium — blocked by: 1, 2, 3, 4
`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 6. Prove — at the checkpoint — blocked by: 5
Rows from [`prd.md`](prd.md) §9 this slice proves, below the UI:

- **FR-3**, the stub part of row b: "run a stub run with the seeded leaks in a slice: the checkpoint pack holds `# Leaks` with the hits, the stub worker's **Fixed** names them, polish's run reports zero" — task 4's test transcript. The doctor and preflight part of the same row is slice 03's.
- **FR-5**, on the tree this checkpoint covers, once slice 03 is done: "run `factory leaks --all` on the feature branch, which holds the merged cleanup: zero `fix` hits outside the allowlist, and `factory docs doctor` prints `leaks    clean`; the gate is green". Run both with the lane's build, as slice 03's Prove does. Slice 03 owns this row; it is named here so the final checkpoint reruns it on the tree that ships, whichever slice lands last. If slice 03 is not done when this checkpoint runs, Progress says so and slice 03's own checkpoint runs it.

**Acceptance:** after the checkpoint, Progress holds task 4's test transcript and the `# Leaks` section of its checkpoint pack, and the `--all` and `doctor` output or the line saying slice 03 runs it.

**Code-review fixes (2026-10-14).**

- A leaks scan that failed at a checkpoint showed only in the worker's pack; the driver now also prints `checkpoint N: leaks not run — …`.
- The `# Leaks` fence was a fixed three backticks, so a hit holding ` ``` ` closed it early. The fence is one longer than the longest backtick run in the report; a unit test seeds one.
- The stub-run test checked `# Leaks` only at CP-1; it now also checks CP-2's section reads `clean` after the worker's fixes. The contract's **Fixed** line says a path hit is listed as a bare path.
