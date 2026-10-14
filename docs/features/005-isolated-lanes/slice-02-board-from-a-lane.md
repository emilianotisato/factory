# Slice 02 — Board and transcripts from a lane

**Source**: [`prd.md`](prd.md) → §4 FR-2 (board and metrics writes go to the main tree; transcripts found by the worker's cwd; the hook row), §13 OPEN-1 and OPEN-8. **Depends on**: nothing. It is plumbing no worker uses yet: slice 07 is the first to set the env var in a worker and to spawn in a lane.
**Created**: 2026-10-14

**Progress**: _(written as it lands — what was built, the gate numbers, what a review caught)_

**Slice**: After this slice a `factory docs` call made from a linked worktree can write the main tree's board. When `FACTORY_BOARD` names the main tree and the caller's cwd belongs to the same repo, four verbs resolve their root there instead of in cwd: `set`, `log add`, `retro` and `status`. Every other verb still works on cwd. The run's metrics stop assuming a worker's cwd is the repo: the stream's `init` line carries the worker's cwd, and the subagent transcript and the `pack_doc` read flag are both computed from it, falling back to the run's repo when the line has none. The hook already keys on `FACTORY_REPO`; a test pins that a lane path works there too. Nothing spawns in a lane yet, so a run behaves exactly as today.

## Current code / environment

- **One root for every verb**: `src/cli/docs.rs:220` `route` computes `root` once from cwd through `paths::repo_root` (`src/paths.rs:114-133`, `git rev-parse --show-toplevel`, canonical), then hands it to `native` (`:249`) and `dispatch` (`:343`). From a lane, that is the lane. Only four verbs may move: `set` (`:365`, `fn set` `:682-694`), `log add` (`:382-410`, through `live_dir` `:670-680`), `retro` (`:274`, `fn retro` `:542-564`) and `status` (`:364`, `fn status` `:570-667`). `docs check`, `docs idea add` and `docs slice` must keep cwd: an idea is a lane file that lands with the commit (prd §4), and `contract-slice.md:19`'s `docs check` checks the lane's own files.
- **The retro key is the canonical root**: `src/docs/retro.rs:21-31` keys a metrics row by `root.canonicalize()`. From a lane it would file polish's score under the lane path (prd §4, corrected by the adversarial pass). Redirect the whole `retro` verb, not only `--set`: a read keyed by the lane would miss the score the same worker just wrote.
- **The lock is per feature dir**: `src/docs/state.rs:124-133` locks `<feature dir>/docs.lock`. Redirected writes take the main tree's lock, which is "under the existing lock" (prd §4). `state::set` validates against the slice files in the target dir (`:140-145`, `slice_files(dir)`), which is fine: slice files are committed in the main tree.
- **Tests inherit the env**: the gate is `cargo test`. `tests/docs_v3.rs:13-22` `docs_in` and `tests/harness/mod.rs:310-312` `Fixture::command` build commands without `env_clear`. From slice 07 on, a build worker of this repo runs with `FACTORY_BOARD` set to this repo's main tree. Honored unconditionally, every fixture's `factory docs set` inside `cargo test` would write this repo's board.
- **The stream has no cwd**: `stream::Summary` (`src/stream.rs:244-260`) holds usage, turns, errors and tool uses. Claude Code's `init` line carries `cwd`, and so does the stub's (`tests/stubs/claude:158`). The `workers` table has no cwd column (`src/db.rs:33-56`), and none is needed: a recount reparses the stream.
- **The transcript slug**: `measure::transcript_path(config_dir, repo, session_id)` (`src/measure.rs:278-292`) slugs `repo`. Callers: `metrics::record_subagents` (`src/metrics.rs:514-515`, with `run.state().repo`), called from `src/scheduler.rs:1423`, `src/closeout.rs:2154` and `src/checkpoint.rs:889`; and `report --recount` (`src/cli/report.rs:929`). The stub writes its transcript under the slug of its own cwd (`tests/stubs/claude:603`, `:674`).
- **`pack_doc` matches Read paths against the repo**: `metrics::pack_doc` (`src/metrics.rs:440-455`) builds `<repo>/<held>` and compares. `insert_calls` passes `&run.state().repo` (`:541`, `:586`) and so does the recount (`src/cli/report.rs:920`). A lane worker reads `<lane>/<held>`, so `pack_doc_read_chars` would read zero for every lane worker with no error. Same cause as the slug: the worker's cwd is not the repo.
- **Tests that mirror today's rules**: `tests/metrics.rs:684-689` `subagents_dir` builds the slug from the fixture root (still right when cwd is the fixture), used at `:713`, `:816`, `:907`, `:999`. The `measure.rs` unit tests at `:400`, `:435`, `:477`, `:503` call `transcript_path` directly.
- **The hook**: `factory hook` takes the repo from `FACTORY_REPO` (`src/cli/hook.rs:893-898`) and denies a whole-file Read of `<repo>/<pack-held file>` (`:104-112`). Nothing ties it to the main tree, so a lane path should already work; FR-2 asks for the test.
- **`tests/docs_v3.rs:238-264` already makes a linked worktree** with `git worktree add`; follow it.
- **Deliberate non-changes**: `factory ctx` needs nothing (prd §4: "its base is cwd … and it keeps no index"). The env var is not set by any spawn here; slice 07 sets it.

## Tasks

### 1. `FACTORY_BOARD` redirects four verbs, same repo only — high — blocked by: —
**Where**: `src/cli/docs.rs` (`route`), `src/paths.rs` (a common-dir helper), `src/worker.rs` (the name as a `pub const`, next to `REPO_ENV`).

**Source**: §4 FR-2: "The driver sets an env var (its name is a slice detail) that points `factory docs set`, `factory docs log add`, `factory docs retro --set` and `factory docs status` at the main tree, under the existing lock. Every other file a worker writes is a lane file and lands with the commit". §13 OPEN-1: "workers keep calling `factory docs set` and `log add`; an env var points them at the main tree's board".

The name is `FACTORY_BOARD`, its value the main tree's path. In `route`, after `root` is known: for `set`, `log add`, `retro` and `status` only, when `FACTORY_BOARD` is set and non-empty, and `git rev-parse --path-format=absolute --git-common-dir` canonicalizes to the same path from `root` and from the var's path, use the var's path (canonical) as the root. Otherwise keep cwd's root and say nothing. `log show` stays on cwd: no contract reads it.

**The trap**: the same-repo check is what keeps `cargo test` safe inside a live worker (see Current code). Without it, a fixture's `docs set` writes this repo's `state.toml`. Do not replace it with an `env_remove` in the harness: an agent's own scratch repo would hit the same thing.

**Acceptance:** in `tests/docs_v3.rs`, on a fixture with a linked worktree `wt` made by `git worktree add --detach`, run from `wt` with `FACTORY_BOARD=<fixture root>`: `docs set 001 slices.1.status=building` changes the main tree's `state.toml` and leaves `wt`'s byte-equal; `docs log add 001 --kind note --slice 1 --title t --body-file -` appends to the main tree's `log.jsonl` only; `docs status 001 --json` reports slice 1 `building`; `docs retro 001 --set score=4` then `docs retro 001` from the main tree shows the score. With `FACTORY_BOARD` pointing at a second, unrelated fixture, `docs set` from `wt` changes `wt`'s own `state.toml` and the unrelated fixture is untouched.

### 2. The worker's cwd from the stream — high — blocked by: —
**Where**: `src/stream.rs` (`Summary`, the `init` parse).

**Source**: §4 FR-2: "Transcripts are found by the worker's cwd (`src/measure.rs:282-291`; callers `src/metrics.rs:515`, `src/cli/report.rs:929`; stub mirror `tests/stubs/claude:603,674`)."

`Summary` gains `cwd: Option<PathBuf>`, taken from the first `system`/`init` line's `cwd`. No DB column.

**Acceptance:** a unit test in `src/stream.rs`: a stream whose `init` carries `"cwd": "/x/lane-2"` gives `Some("/x/lane-2")`; one with no `cwd` gives `None`.

### 3. Subagents and `pack_doc` read from that cwd — high — blocked by: 2
**Where**: `src/metrics.rs` (`record_subagents` `:514-515`, `insert_calls` `:541`, `:586`), its three callers (`src/scheduler.rs:1423`, `src/closeout.rs:2154`, `src/checkpoint.rs:889`), `src/cli/report.rs:920`, `:929`.

**Source**: §4 FR-2 (above) and its acceptance: "A polish stub with a `subagents:` step, run in a lane, shows its subagent's tokens in `factory report`. The transcript is read only for subagent usage …, so this is the line that moves when the slug is wrong."

Wherever a worker's transcript path or its `pack_doc` base is built, use the worker's `Summary::cwd` when present, else the run's repo as today. Keep `transcript_path`'s signature; pass it the right dir.

**The trap**: `pack_doc` is the same bug with no visible symptom. It is not named in the PRD's list of callers, but its cause is the one FR-2 fixes: a lane worker reads `<lane>/<held>`, and `pack_doc_read_chars` reads zero for every lane worker.

**Acceptance:** unit tests in `src/metrics.rs`: `record_subagents` for a summary whose `cwd` is `W` reads `<config>/projects/<slug of W>/<session>/subagents` and fills the columns; with `cwd` `None` it reads the repo's slug, as `tests/metrics.rs:713` asserts today. `pack_doc` on a `Read` of `<W>/src/lib.txt`, with `src/lib.txt` held and base `W`, is `true`; with base the repo it is `false`. `report --recount` uses the same rule (one test through the binary or a unit test on the recount loop). `tests/metrics.rs` stays green unchanged.

### 4. The hook guards a lane path — medium — blocked by: —
**Where**: `tests/hook.rs`.

**Source**: §4 FR-2 acceptance: "`factory hook`, fed a whole-file Read of `<lane>/<pack-held file>` with `FACTORY_REPO` set to the lane, denies it, as it does for the main tree today (`src/cli/hook.rs:104-112`)."

A test through the binary: a pack holding `src/lib.txt`, `FACTORY_REPO` set to a linked worktree's path, a `PreToolUse` Read of `<worktree>/src/lib.txt` with no offset is denied with today's message. Change `src/cli/hook.rs` only if it fails.

**Acceptance:** that test passes, and the same input with `FACTORY_REPO` set to the main tree is allowed (the path is outside it).

### 5. Verify — medium — blocked by: 1, 2, 3, 4
`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 6. Prove — at the checkpoint — blocked by: 5
Rows from [`prd.md`](prd.md) §14 this slice proves, below the UI. **FR-2**, the parts that need no spawn in a lane:

- run the task 1 test: `docs set` and `docs log add` from a worktree land in the main tree's `state.toml` and `log.jsonl`, the worktree's copies unchanged, and an unrelated `FACTORY_BOARD` is ignored.
- run the task 4 test: "feed `factory hook` a Read of a pack-held file in a lane".

The rest of FR-2's row is slice 07's (calls' cwd and env, board writes in a run) and slice 10's (polish subagents in `factory report`).

**Acceptance:** after the checkpoint, Progress holds both test transcripts and the hook's deny output.
