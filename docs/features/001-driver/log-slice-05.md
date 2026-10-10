## Traps

- **Inner-loop `cargo test` runs neither fmt nor clippy.** Slices 03/04 arrived with fmt red on 11 files and clippy red on 7 sites; run `cargo fmt` and `cargo clippy --all-targets -- -D warnings` before the commit, not after.
- **`lanes: 1` in the fixture index opens a preflight `ASK-1` and parks the run** before a worker spawns (`tests/run.rs` `run-ask` relies on it). A one-at-a-time build is `fx.chain()` on the default two-lane index.
- **`docs slice NNN a` titles the slice `A`**, so the driver's subjects are `feat: A`, not `feat: a`; read `slices[].title` from the board, never the slug.

## Decisions

### AD-501 — What id does the driver give a merge-refusal block? · slice-05 · 2026-10-10

**Chose:** `BLOCK-n` with the lowest `n ≥ 1` no `BLOCK-` entry in the feature's logs uses (`BLOCK-1` in the fixture), phase `checkpoint`, written to `agent-log.md`; a second refusal after an answer takes the next free number and parks the run anew.
**Over:** the slice's `BLOCK-<NNN>-merge`. Both `answers::block_heading` and `docs.mjs` `parseAgentLog` require `BLOCK-<digits>`: `factory answer BLOCK-001-merge` would find nothing to resolve and `docs status` would not list the block, so the owner could never clear it.
**Because:** FR-5 says the driver blocks; the id shape is the tool's, not the PRD's. Builders use `NN01`, `NN02`, … so the driver's own blocks stay below 100 without colliding.
**Undo:** one format string in `scheduler::merge_or_block` and the test's expected heading.
**Review:** decide

### AD-502 — Which events report the commit path? · slice-05 · 2026-10-10

**Chose:** `slice_committed {slice, sha, files}` and `untracked_leftovers {slice, files}` as the slice names them; `worker_wrote_protected_file {slice, path}` and `merge_refused {block_id, detail}` with these payloads; and a `log_merged {sha, fragments}` the slice does not name, so the cockpit sees the merge commit the way it sees a slice commit. The park on a refusal is the existing `run_waiting` with `blocked: []`, `asks: []` — the `merge_refused` right before it carries the id.
**Over:** folding the merge sha into `checkpoint_due`, which slice 06 owns; or a new `run_waiting` field for a driver-level block, which would change an event slice 04 already ships.
**Because:** `events.jsonl` is the cockpit's record (§9 FR-7) and FR-5 leaves the payloads open.
**Undo:** five variants in `events.rs` and the assertions in `tests/commits.rs`.
**Review:** decide

### AD-503 — Does the driver create `agent-log.md` when the feature has none? · slice-05 · 2026-10-10

**Chose:** yes — `# Agent log — PRD-{NNN}\n` written before `docs log merge`, which dies with `no agent-log.md in <dir> — nothing to merge into` otherwise (`docs new` does not create the file; the fixture has none).
**Over:** treating the missing file as a refusal, which would park every fresh feature on its first merge.
**Because:** `answer_ask` (slice 04) already creates it the same way; two writers of the header is one convention.
**Undo:** drop the `fs::write` in `commit::merge_logs`.
**Review:** fyi

### AD-504 — What does the merge commit stage? · slice-05 · 2026-10-10

**Chose:** `agent-log.md`, `prd.md` and `docs/index.md` as the slice says (only those `render --write` changed land — on the fixture the index is already current), plus the fragments `docs log merge` removed, so their deletions are in the commit.
**Over:** the three files alone, which leaves ` D log-slice-NN.md` in `git status` and a dirty tree for the next preflight — the slice's own acceptance wants the fragments gone and the tree clean.
**Because:** the fragments were committed with their slices (they are in every slice's pathspec); the merge is what removes them.
**Undo:** drop the fragments from the path list in `commit::merge_logs`.
**Review:** fyi

### AD-505 — How are a slice's leftovers told apart from another lane's half-written work? · slice-05 · 2026-10-10

**Chose:** leftovers = untracked files now − untracked at this worker's spawn (a snapshot kept on the running-table entry) − the slice's pathspec − every other slice's file and fragment − the paths the still-running workers' streams name so far. A worker settled after a driver death has no snapshot, so its baseline is empty.
**Over:** the slice's `git status --porcelain --untracked-files=all` minus the pathspec, which in a two-lane run lists lane 2's new files in lane 1's fragment and names a leftover twice when a later slice runs after it.
**Because:** FR-5 wants the list to be the worker's own unnamed files; the files themselves are never committed either way. Residual: a file lane 2 wrote whose stream line is not yet flushed can still be listed — cosmetic.
**Undo:** `Running.untracked_before` and `scheduler::leftovers`.
**Review:** fyi

### AD-506 — Does `commit_sha` bump `schema_version`? · slice-05 · 2026-10-10

**Chose:** no. The column is in `CREATE TABLE workers` for a new db and a guarded `ALTER TABLE workers ADD COLUMN commit_sha TEXT` (checked through `pragma_table_info`) for an existing one; `SCHEMA_VERSION` stays 1.
**Over:** version 2 with a migration table, which the slice's "idempotent migration" does not ask for and `tests/run.rs` (version 1) would have to follow.
**Because:** SQLite's `ALTER TABLE` has no `IF NOT EXISTS`; the guard is the idempotence.
**Undo:** the `has_column` check in `Db::migrate`.
**Review:** no
