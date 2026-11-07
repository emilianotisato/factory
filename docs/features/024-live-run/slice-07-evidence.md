# Slice 07 — Evidence files per checkpoint

**Source**: [`prd.md`](prd.md) → §8 FR-6, §3 (`--evidence-limit`, the cap order). **Depends on**: 01 (`src/live/evidence.rs`, `tests/live/evidence.rs`, the parsed `--evidence-limit` and the cap that cuts evidence first). Independent of 02 to 06.
**Created**: 2026-11-07

**Progress**:
- **Built:** `src/live/evidence.rs` lists `evidence` as `{root: "evidence", checkpoints, skipped, truncated}`. This run's checkpoint numbers come from `checkpoint_started`, `checkpoint_resumed` and `checkpoint_done` events; `state` is `done` once `checkpoint_done` exists. Each existing `cp-N` dir is walked with `symlink_metadata` only, to depth 4 below the root. Symlinks, FIFOs and devices count in `skipped`, and a symlinked dir is never entered. Printed paths are built from the walk's relative names, never from an absolute path. Files sort by mtime descending, then path. `x.aria.txt` attaches to the first row stemmed `x` in its dir (AD-9: `sidecars` is a list of paths); one with no such row is its own row. Each row has `kind` by extension, RFC3339 `mtime`, `stable` (older than 2 s), `etag` `<bytes>-<mtimeMs>`, a humanized `caption`, and `citedBy`: the ids of the feature log entries whose body contains the path. `--evidence-limit` counts rows, newest checkpoint first, and sets `evidence.truncated` when it cuts. The choices the slice left open are AD-10.
- **Tests:** `tests/live/evidence.rs` has the five task-1 acceptance tests and `live_evidence_names_the_entries_citing_a_file_and_its_sidecars`. A linked dir is also checked in `live_evidence_paths_stay_under_root`. A unit test in `src/live/evidence.rs` covers kind, stem and caption.
- **Inner loop:** 1 run, 0 red, ~1.7 min (plus one targeted `--test live` run).
- **Prove (FR-6):** left for the checkpoint: `live_skips_a_symlink_in_evidence` and `live_evidence_paths_stay_under_root`.

**Slice**: After this slice, `factory live` lists the run's evidence: per checkpoint of this run, newest first, each file's path under the evidence root, kind, size, mtime, whether it is still being written, an etag, a caption from its name, the log entries that cite it, and its `.aria.txt` sidecars. It only `lstat`s: no symlink is followed and no file is read. Captions are file names; there is no manifest.

## Current code / environment

- **The root**: `$FACTORY_EVIDENCE` is `runs/<id>/evidence`, created at every worker spawn (`src/worker.rs:58`, `:261-263`). A run that never spawned has none: an empty `checkpoints` list, not an error (prd §3 "Old layouts").
- **`cp-N` is a convention, not code**: `assets/contract-checkpoint.md:15-17` tells the checkpoint worker to save under `$FACTORY_EVIDENCE/cp-{n}/` and cite `cp-{n}/<file>` in Progress and the log body. Nothing enforces it, so expect stray files at the root and deeper dirs.
- **Which checkpoints are this run's**: `checkpoint_started {n, covers}` and `checkpoint_resumed {n, covers}` (`src/events.rs:120-127`); `checkpoint_done {n, sha}` (`:167-170`) ends one. prd §8: "Only this run's `cp-N` directories, by the `n` in its events. Numbers continue across runs of a feature." A `cp-N` dir with no event of this run is not listed.
- **What real runs hold**: images, `.txt` transcripts, and `.aria.txt` accessibility dumps beside screenshots (e.g. `runs/010-20261016-61d1/evidence/cp-1/case-statuses-before.aria.txt`). Tests build their own trees; never read the machine's runs folder.
- **`citedBy`**: the feature's `log.jsonl` (`src/docs/log.rs:127-128`, `entries` `:154`) through the feature-folder lookup slice 01 shared; an entry cites a file when its `body` contains the file's root-relative path.
- **The cap**: slice 01 cuts evidence first, then the timeline, to keep the output ≤ 512 KiB, and names the cut in `truncated`. `--evidence-limit` (default 200, max 300) is parsed there.
- **Deliberate non-change**: prd §11 "Image dimensions and `dupOf` hashing: the consumer learns them, or dedupes by etag", "A caption manifest", "Review contents". prd §14 "Not handled: screenshots outside the run dir (lane `artifacts/`, `/tmp/polish-*`)." prd §8 "File contents are never read, except for `citedBy`, which reads `log.jsonl` only."

## Tasks

### 1. The listing — high — blocked by: —
**Source**: §8 FR-6: "`evidence`: `{ root, checkpoints: [{ n, state: writing|done, files }], skipped, truncated }`", "Each file is `{ path, kind: image|text|document|archive|other, bytes, mtime, stable, etag, caption, citedBy, sidecars }`", "`path` is relative to `root`, never absolute and never containing `..`", "`stable` means the mtime is older than 2 s", "`etag` is `<bytes>-<mtimeMs>`", "`caption` is the file stem, humanized", "`.aria.txt` files attach as `sidecars`. Every other file is its own row", "`lstat` only: regular files, no symlinks, FIFOs or devices (counted in `skipped`), no link following, depth ≤ 4", "Checkpoints newest first, then files by mtime descending, then name"; §3 "`--evidence-limit <n>`, default 200, max 300."

`src/live/evidence.rs`. `root` is the run-relative `evidence` (never absolute: slice 09's redaction is not a reason to print a home path). `state` is `done` once a `checkpoint_done` for `n` exists, else `writing`. `kind` by extension (png/jpg/jpeg/gif/webp/svg → image; txt/log/md/json/jsonl/csv → text; pdf/html → document; zip/tar/gz/tgz → archive; else other). A sidecar `x.aria.txt` attaches to the row whose stem is `x` in the same dir, and is its own row when there is none. `mtime` RFC3339 UTC. The limit counts files across checkpoints, newest first; `truncated` (inside `evidence`) is true when it cut.

**The trap:** `fs::metadata` and `DirEntry::metadata` differ: use `symlink_metadata` (lstat) for every entry, and never descend into a symlinked dir. A path built by `strip_prefix(root)` is the only path printed.

**Acceptance:** `live_lists_evidence_newest_first` (cp-1 done and cp-2 writing with three files of set mtimes: checkpoint 2 first, files by mtime descending, ties by name; a `cp-9` dir with no event is absent); `live_skips_a_symlink_in_evidence` (a symlink to `/etc/passwd` and a FIFO under `cp-1`: neither listed, `skipped == 2`); `live_evidence_paths_stay_under_root` (every `path` is relative and has no `..` component, including a file at depth 4; one at depth 5 is not listed); `live_marks_a_file_written_this_second_unstable` (a file just written has `stable: false`, one with mtime 10 s ago `true`); `live_evidence_respects_the_limit` (`--evidence-limit 2` over 5 files lists the 2 newest and `evidence.truncated == true`).

### 2. `citedBy` and sidecars — medium — blocked by: 1
**Source**: §8 FR-6: "`citedBy` lists the log entries that name the file, by substring", "`.aria.txt` files attach as `sidecars`."

`src/live/evidence.rs`. `citedBy` is the citing entries' ids (skip entries without one).

**Acceptance:** a fixture feature log whose `CP-1` entry body names `cp-1/home.png` gives that file `citedBy: ["CP-1"]` and another file `[]`; `cp-1/home.aria.txt` appears in `home.png`'s `sidecars` and not as a row.

### 3. Verify — medium — blocked by: 2
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 4. Prove — at the checkpoint — blocked by: 3
prd §12 row **FR-6**: run `live_skips_a_symlink_in_evidence` and `live_evidence_paths_stay_under_root`, and keep the transcript.

**Acceptance:** after the checkpoint, this slice's Progress names that transcript.
