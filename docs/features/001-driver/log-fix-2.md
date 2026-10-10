## Decisions

### AD-20001 — Which number does the driver's death block on a slice take? · polish · 2026-10-10

**Chose:** an open block already titled "Two workers died on slice NN" if a kill left one written. Otherwise the lowest `BLOCK-NNkk` with `kk` from 02 that no log file of the feature uses (`scheduler::dead_block_id`).
**Over:** the lowest free number below 100, as the checkpoint's blocks take (AD-501); or `kk` from 01.
**Because:** the slice's range keeps the block attributable to the slice when the merge folds it. Starting at 02 keeps the id the driver has always written whenever nothing clashes, and skipping used numbers ends the clash with a builder's own `BLOCK-NN02`.
**Undo:** in `dead_block_id`, start the range at 1, or call `checkpoint::lowest_free_block`.
**Review:** no

### AD-20002 — What orders "since the last answer" for the death cap? · polish · 2026-10-10

**Chose:** `events.jsonl` order. A slice worker counts toward the cap unless its `slice_spawned` comes before the latest `block_answered` naming the slice. A checkpoint's workers are spent by the latest `block_answered` for one of that checkpoint's own `checkpoint_blocked` ids. A worker with no spawn event counts.
**Over:** comparing `spawned_at` with the answer file's `at`, which the checkpoint used. Both are whole seconds, so a respawn in the answer's second compares wrongly.
**Because:** `seq` is a total order (the emit is serialised under a lock, AD-20006), and `factory answer` now logs `block_answered` before the slice flips, so the answer always precedes the respawn it causes.
**Undo:** filter on `spawned_at > answer.at` in `scheduler::deaths_since_answer` and `checkpoint::spawned_before_answer`.
**Review:** no

### AD-20003 — Which answers reset a checkpoint's death cap? · polish · 2026-10-10

**Chose:** only the answers to that checkpoint's own blocks: the three-reds block, the two-deaths block, or the worker's own block, all emitted as `checkpoint_blocked {n}`.
**Over:** any answer naming no slice. That covers a refused merge, a closeout block or another checkpoint's block.
**Because:** an answer to a refused merge says the fragments are fixed. It says nothing about why the checkpoint workers died, so the once-only respawn should not come back with it.
**Undo:** treat `merge_refused` block ids as the checkpoint's own in `checkpoint::since_answer`.
**Review:** no

### AD-20004 — What does a sanitized BLOCK title look like? · polish · 2026-10-10

**Chose:** `answers::block_title`. Whitespace runs, newlines included, collapse to one space, every `·` becomes `-`, and the title is cut at 160 chars ending in `…`. The checkpoint's three-reds and refused-merge titles go through it.
**Over:** escaping the separator, or dropping the gate line from the title.
**Because:** `entries` and `resolve` split the heading on ` · `. A `·` in a gate's last line shifted the phase and `open` fields, so the block no longer parsed as open. A 2000-char compiler line in a heading also helps nobody. The full output stays in the body and the transcript.
**Undo:** use the raw `format!` titles in `checkpoint::run` and `block_on_refusal`.
**Review:** no

### AD-20005 — What does the driver do when HEAD leaves the run's branch? · polish · 2026-10-10

**Chose:** two guards. Each tick, before the reap, the driver checks HEAD against the run's branch. Off the branch, it parks in `waiting-owner` with a line naming `git switch <branch>`. In-flight workers finish but are not routed, nothing spawns, and the run goes on by itself once HEAD is back. Every `commit::commit` in the driver process is also held to the run's branch (`commit::hold_branch`, a process-wide table keyed by repo), so a switch between the check and a commit errors instead of landing a commit. That error fails the run. `factory run NNN` refuses a resume off the branch before it adopts anything. A run whose branch is the preflight's default branch refuses at creation, at resume and at every commit.
**Over:** a BLOCK-n the owner answers after switching back; failing the run on the first tick off the branch; or checking only in the four driver commit sites of my files.
**Because:** switching back is the whole fix, so a block and an answer would add a step without adding information. A park that clears itself keeps the run. The process-wide hold also covers the polish, ship and CI-fix commits in `closeout.rs`, which this fixer may not edit.
**Undo:** drop the `branch_refusal` block at the top of the `drive_inner` loop, and the `held` check in `commit::commit`.
**Review:** fyi

### AD-20006 — How are emitters in several processes serialised? · polish · 2026-10-10

**Chose:** `flock` on `events.jsonl` itself, through std `File::lock`. It is taken before the last `seq` is read and held until the `events` row is inserted. The line and its newline go out in one `write_all`.
**Over:** a separate lock file; an in-process mutex, which `factory stop` and `factory answer` never see; or the `fs2`/`fd-lock` crates, a new runtime dependency.
**Because:** the file is the thing being appended, so locking it needs no second path to create or clean up. Holding the lock through the insert keeps the table's rows in `seq` order with the file. AD-20002's "since the last answer" depends on that order being total. rustc 1.98 has `File::lock` in std.
**Undo:** drop the `f.lock()` call in `events::emit`.
**Review:** no

### AD-20007 — Whose are the tracked files a worker changed without naming them? · polish · 2026-10-10

**Chose:** the worker's, when nothing else ran beside it. A file changed since the worker's spawn baseline that no tool call named goes in the worker's commit if it is tracked, not protected, and no other worker was live. That applies to a slice worker with no sibling, and to every fix and checkpoint worker, since the lanes drain before a checkpoint. With a sibling live, the file is listed (an `untracked_leftovers` event and the fragment's list for a slice, `checkpoint_leftovers` at a checkpoint) and left in the tree. A new, untracked file that no tool call named is always listed, never committed, as before. A file once listed is never swept later. Each checkpoint worker's baseline is saved under `runs/<id>/untracked-before/checkpoint-n-<role>.json`, as a slice's is, so a resumed driver judges an orphan the same way.
**Over:** listing every tracked change as well (it would leave a `sed -i` or a formatter's edit uncommitted for good); or sweeping with siblings live (whose change it is cannot be told).
**Because:** with one worker running, a tracked file that changed under it is its work. The commit message already names the worker, so the sweep loses nothing, and `git diff` of the commit shows it. An untracked file can be a scratch file, a build output that `.gitignore` misses, or a secret, so it stays a human call. The cost is that an owner editing a tracked file by hand during a lone worker's run sees the edit in that worker's commit.
**Undo:** pass `alone: false` to `scheduler::unnamed` everywhere, so every unnamed change is listed.
**Review:** fyi

### AD-20008 — Does a slice whose worker died wait for a due checkpoint? · polish · 2026-10-10

**Chose:** yes, on resume and in an unbroken run alike. With `checkpointDue` set, a `building` slice with no live worker (a death below the cap, or an answered block) is not respawned. Workers in flight finish, the checkpoint runs, and the respawn follows it. A death-cap block still happens at once, since it spawns nothing.
**Over:** respawning first and counting the respawn as part of draining the lanes.
**Because:** PRD §5 FR-4 says to drain the lanes on `checkpointDue`, and the board reports `spawnable: 0`. A respawn is a spawn. Respawning first also lets a checkpoint cover more than `checkpoint_every` slices. The cost is that the gate runs over the dead worker's half-written changes. The checkpoint commit leaves them out, because they are in its spawn baseline, so the respawned worker still owns them.
**Undo:** drop the `board.checkpoint_due` skip in the building-respawn loop of `drive_inner`.
**Review:** fyi

### AD-20009 — When `factory answer BLOCK-n` fails partway, how does a retry finish it? · polish · 2026-10-10

**Chose:** two changes. First, everything that can refuse runs before anything is written; a block naming a slice with no file is refused and nothing is written. Second, the steps run in this order: answer file, `block_answered`, then the heading and its `**Resolution**`, then the slice flip, then the wake. A retry that finds every `BLOCK-n` resolved finishes the answer when the feature's resumable run has it on file and is still stuck: either the slice is `blocked`, or the run is `waiting-owner` with no driver and is not parked on its budget. Otherwise it is refused as "already resolved", as before.
**Over:** writing the heading last, after the flip.
**Because:** a respawned worker's pack quotes the entry as the log holds it (`block_section`). If the flip happens before the heading, a live driver can respawn the slice in between, and the worker gets the open block without its resolution.
**Undo:** in `cli/answer.rs`, call `answers::resolve` first again and drop `answer_resolved`.
**Review:** no

### AD-20010 — Two open runs wait on the same `ASK-n`: which does `factory answer` take? · polish · 2026-10-10

**Chose:** neither. It refuses and names each run, with its feature and the ask's text. A new `--run <id>` option on `factory answer` picks one.
**Over:** the newest run (what it did, silently); refusing with no way to answer; or matching on the feature number.
**Because:** ask numbers start at 1 in every run, so two features parked at preflight in one repo both have an ASK-1. A refusal with no option would leave both runs unanswerable. The run id is what the refusal prints, so it is what the owner copies.
**Undo:** remove `AnswerArgs::run`, and take the first run that waits.
**Review:** fyi

### AD-20011 — Does a run parked on its budget get its own phase? · polish · 2026-10-10

**Chose:** no. It stays `waiting-owner`. `factory answer` does not start a driver for a run that `quota::hold` says is over budget. It prints the weighted tokens, the budget, and `factory run NNN --budget <higher>`.
**Over:** a `parked-budget` phase.
**Because:** the budget is read from the `workers` rows each time, so the hold check already tells the two parks apart. A new phase would touch the cockpit, `status`, the `runs` table and every match on `Phase`, and those files belong to other fixers.
**Undo:** drop the `Hold::Budget` arm in `cli/answer.rs::wake`.
**Review:** no
