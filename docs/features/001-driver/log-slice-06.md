## Traps

- **`driver.log` holds the detached driver's lines only.** A `--foreground` run says the same lines on its stdout and appends nothing to `runs/<id>/driver.log`; a test that starts in the foreground and resumes detached finds only the resumer's "— resumed" line there (`tests/checkpoints.rs` captures the foreground stdout into `scenario/driver.out`).
- **Every spawn eats a plan step, respawns included.** A `dead` build worker's respawn takes the next step, so a plan that ends in `checkpoint` is one step short and the checkpoint step lands on a build worker, which the stub refuses (exit 65 → dead → respawn, and the count is off by one more). Count spawns, not slices: `[dead, done, done, done, done, checkpoint]` for three slices with one death; a kill-and-resume under a checkpoint worker spawns two checkpoint workers for one `CP-n`.
- **The stub records its call file after the driver has emitted the spawn event.** Waiting on `kinds().last() == "checkpoint_spawned"` and then reading `fx.calls()` races; wait on the call count too.
- **The agent-log Trap "a fixture run ends at phase `checkpoint`, never `finished`" is over as of this slice**: the end of the run is the last checkpoint, so a run with every slice done finishes (`finished (every slice done)`), and `factory run NNN` on a delivered feature finishes at once without a spawn.
- **`checkpoint_every: 10` in a fixture index now means "one checkpoint at the end", not "no checkpoint"**: every loop/lanes/commits plan needs a trailing `checkpoint` step and every exact event tail ends `checkpoint_started, gate_run, log_merged, checkpoint_spawned, checkpoint_done, run_finished`.

## Decisions

### AD-601 — What id do the driver's checkpoint blocks carry? · slice-06 · 2026-10-10

**Chose:** `BLOCK-n` with the lowest free `n` across the feature's logs (`BLOCK-1` in the fixture), phase `checkpoint`, in `agent-log.md` — for the three-reds block and the two-dead-checkpoint-workers block alike. The gate block is titled `gate red three times at checkpoint {n}: {last non-empty gate line}` with the 40-line tail of the last red log quoted under **Happened**.
**Over:** the slice's `BLOCK-<NNN>-cp<n>`, for AD-501's reason: `factory answer` and `docs.mjs` only know `BLOCK-<digits>`.
**Because:** the same answer as AD-501; whichever way that review goes, this follows it.
**Undo:** `checkpoint::lowest_free_block` and the ids in `tests/checkpoints.rs`.
**Review:** fyi

### AD-602 — When does the driver trust a cached red gate? · slice-06 · 2026-10-10

**Chose:** only right after a fix worker in the same checkpoint pass (`trust_red = reds > 0`): a fix that changed nothing is a cache hit, red again, and counts. On entering the checkpoint — fresh, resumed, or after `factory answer` on a gate block — a cached red is ignored and the gate reruns even when the tree is unchanged. A cached green is always reused.
**Over:** trusting every hit, which on a resume or an answer would replay three cached reds and re-block the run instantly with no fix worker between them; or never caching reds, which loses the "the fix touched nothing" signal.
**Because:** FR-4's cache exists to skip a *rerun* when nothing changed; a red the owner was just asked about is the one result worth rerunning, and the slice's own acceptance has the answer rerun the gate.
**Undo:** drop the `trust_red` filter in `checkpoint::gate_once`.
**Review:** fyi

### AD-603 — Which events report the checkpoint, and what became of `checkpoint_due` and `log_merged.sha`? · slice-06 · 2026-10-10

**Chose:** `checkpoint_started {n, covers}` / `checkpoint_resumed {n, covers}`, `gate_run {n, attempt, red, minutes, log}`, `gate_cached {n, attempt, red, log}`, `fix_spawned {n, attempt, session_id, pid}`, `fix_committed {n, attempt, sha, files}`, `checkpoint_spawned {n, attempt, session_id, pid}`, `checkpoint_blocked {n, block_id, question}`, `checkpoint_done {n, sha}` (`sha` null when render and the worker left nothing new to commit). Slice 03's `checkpoint_due` stub is gone; `log_merged {fragments}` drops AD-502's `sha` because the merge no longer has a commit of its own — it lands in `docs: checkpoint n`, the `checkpoint_done` sha.
**Over:** keeping `checkpoint_due` as a marker before `checkpoint_started` (two events for one fact), or a `log_merged.sha` that would always be the later `checkpoint_done.sha`.
**Because:** FR-4 names `checkpoint_started`, `gate_run`, `fix_spawned`, `checkpoint_done` and leaves the payloads open; `events.jsonl` is the cockpit's record (§9).
**Undo:** the variants in `events.rs` and the exact tails in `tests/loop.rs`, `tests/commits.rs`, `tests/lanes.rs`, `tests/run.rs`, `tests/checkpoints.rs`.
**Review:** fyi

### AD-604 — On a resume, where does the killed checkpoint worker's death land? · slice-06 · 2026-10-10

**Chose:** `worker_dead {slice: null, session_id, exit_code}` right after `run_resumed`, before `checkpoint_resumed` and `gate_cached` — the scheduler settles every orphan by role before it reads the board, the way slices 03/04 settle a build worker's, and writes the `Dead checkpoint worker` Traps line. The death counts toward the two-deaths cap (AD-609).
**Over:** the slice's "the first events after `run_resumed` are `checkpoint_resumed` and `gate_cached`", which would mean deferring the checkpoint orphan into `checkpoint::run` — two places that settle orphans.
**Because:** one orphan path; and the cockpit sees the death before the retry, the order it happened in.
**Undo:** move `scheduler::settle_checkpoint_orphan` behind `checkpoint::run`'s resume branch; the `k[16..]` tail in `tests/run.rs`.
**Review:** fyi

### AD-605 — What does the checkpoint commit stage? · slice-06 · 2026-10-10

**Chose:** the feature folder, `docs/index.md`, and whatever else the checkpoint worker's stream named (`Pathspec::from_stream` → `commit::expand`), as one `docs: checkpoint n` after `docs log merge` and `render --write`. The merged fragments' deletions, the worker's `CP-n` and pasted evidence, and the rendered PRD table land together.
**Over:** the slice's `docs/features/NNN-*/ docs/index.md` alone, which drops any evidence the worker wrote outside the folder; or per-file staging, which misses the fragments' deletions (AD-504).
**Because:** the folder is the common case; the stream is the record (§4) when the worker steps outside it.
**Undo:** drop `spec.paths` from `want` in `checkpoint::commit_checkpoint`.
**Review:** no

### AD-606 — Where does a fix worker write its decisions? · slice-06 · 2026-10-10

**Chose:** its own `log-fix-{n}.md` (`n` the checkpoint), AD numbers from `AD-9{n}01`, blocks from `BLOCK-9{n}01`; `docs log merge` already folds `log-fix-N.md` after the slice fragments, so the file goes with the checkpoint commit and never races a slice worker (the lanes are drained).
**Over:** letting a fix worker write `agent-log.md` (the file the slice contract forbids) or a slice's fragment (whose numbers are the slice's).
**Because:** the seed keeps a fix worker's numbers out of every slice's `NN01` range below slice 91.
**Undo:** the `{seed}` string in `Pack::for_fix` and `assets/contract-fix.md`.
**Review:** no

### AD-607 — How is the gate's cache key built? · slice-06 · 2026-10-10

**Chose:** `git read-tree --empty` + `git add -A -- . ':(exclude)docs'` + `git write-tree` on a throwaway index at `runs/<id>/gate-index`, plus the trimmed gate command; stored in `runs/<id>/gate-cache.json`. Modified and untracked (not ignored) code counts; a committed `docs/` change does not.
**Over:** the slice's `read-tree HEAD` first, which keeps HEAD's `docs/` blobs in the index, so every checkpoint commit (docs only) would change the key and defeat the cache at the next checkpoint.
**Because:** PRD §6: "hash of the tree minus `docs/**`".
**Undo:** `gatecache::tree_id`.
**Review:** no

### AD-608 — How do the checkpoint's workers fit the schema? · slice-06 · 2026-10-10

**Chose:** `WorkerRef.slice: Option<u32>` plus `checkpoint: Option<u32>` in `state.json`; `workers.checkpoint`, `gates.cached`, `gates.checkpoint`, `gates.attempt` added by guarded `ALTER TABLE` (slice 05's `has_column`), schema version unchanged; `workers.role` tells `build` / `fix` / `checkpoint` apart (`worker::Job`).
**Over:** a schema bump, or a second table for the checkpoint's workers.
**Because:** the rows are the same thing — a `claude -p` with a pack and an exit — and §10's report reads one table.
**Undo:** the four `ALTER`s in `db::open`.
**Review:** no

### AD-609 — How many checkpoint-worker deaths before a block, counted since when? · slice-06 · 2026-10-10

**Chose:** two (slice 04's `DEATH_CAP`), counted among this checkpoint's workers spawned after the latest `factory answer` on a checkpoint block, so an answer buys two fresh attempts; the block is `Two checkpoint workers died at checkpoint {n}`. A dead fix worker is settled and the gate reruns — it counts as a red, never as a death.
**Over:** counting across the whole run (an answered block would re-block on the next death) or resetting on every spawn (no cap).
**Because:** the slice: "treat it as slice 04 treats a slice worker: once more, then block"; a dead fix worker is FR-4's "counts as a red attempt".
**Undo:** `checkpoint::last_checkpoint_answer`.
**Review:** no
