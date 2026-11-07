---
number: 024
slug: live-run
title: "factory live: the live-run contract for the herdr cockpit panel"
type: feature
created: 2026-11-07
author: Emiliano Tisato
summary: |
  A new read-only verb, `factory live <id> --json`, returns one run's live detail as a schema-1 contract: what it waits on and why, what each worker does, the running or red gate with its failing excerpt, CI, a timeline, and the evidence files. Driver phases that are silent today get events (gate start, review collect, CI waiting, an empty fix, a park's reason). The same excerpt becomes a red gate's block title in `status`. The consumer is herdr-factory PRD-003's Run side panel.
from_idea: "2026-11-07-factory-live-a-read-only-live-run-contract-for-the"
---

# PRD-024 — factory live: the live-run contract for the herdr cockpit panel

## 1. Why now

herdr-factory PRD-003 puts a Run side panel in the herdr-app factory layout. The owner picked its sections on 2026-11-07: Waiting on you and Now, Agents, Gate and CI with a timeline, and Screenshots. They also chose that factory exposes this through a contract, not the panel reading run files. Today nothing does: `status --json` (PRD-023) carries a run's board, not what it is doing.

A research pass on 2026-11-06 read 70 runs in `~/.local/share/factory/runs` across 7 stacks. What an owner watching live could not see, ranked:

1. **Why it waits.**
   - Parks lasted 6 to 9 h behind block titles that said nothing. Nine read "gate red three times at checkpoint N: Script @ci:check was called via verify". The title is the gate log's last non-empty line (`checkpoint.rs:125-135`), and the cause was a JSON `failures[]` line mid-log.
   - A 9 h park on a push failure, while `factory report` said "blocks: 0".
   - 32 of the 33 `run_waiting` events on the machine have empty `asks` and `blocked`. Their reason is in `driver.log` only.
   - A block answered, then 56 min of nothing until the owner stopped the run.
2. **What it does.** The longest silences are the driver's:
   - a 15 to 30 min review collect after the last checkpoint, with no event (1023 to 1801 s in 7 runs)
   - gates of 9 to 33 min, with no event until they end
   - CI waits of 8 to 31 min, visible in `driver.log` only
   - a driver dead for 379 min
3. **What each worker does.** Slices of 30 to 106 min, with normal 10 min silences inside one Bash call.
4. **Where a gate or CI round failed.**
   - The reason is mid-log, at the top, or in a JSON line, rarely at the tail.
   - A fix worker that "changed nothing to commit" re-serves a cached red. That shows in `driver.log` only (`checkpoint.rs:621-625`, `closeout.rs:643-648`).
5. **The sequence:** blocks, conflicts and red gates in a row.
6. **Evidence** lands in bursts near a checkpoint's end, and many stacks make none.

**Left alone:** `status --json` stays as PRD-023 shipped it, except two texts: FR-8's block title, and the board's `last:` line for a `run_waiting` event, which names its new `reason` (§2). `factory wait`, `report` and the cockpit Claude Code mod are unchanged.

## 2. Blast radius

- **New events in `events.jsonl`.** All are additive. Older binaries skip unknown kinds through `EventKind::Unknown` (`events.rs:257-259`). `run_waiting` gains a `reason` field. The plain `status` board's `short()` picks a field named `reason` as an event's subject (`cli/status.rs:700-718`), so the board's `last:` text changes for `run_waiting`.
- **Block titles change** (FR-8). A red gate's block question, and so `status --json` `questions[].text`, the herdr strip and its Answer dialog, show the failure excerpt in place of the last log line. The field's meaning is the same, so it stays schema 1. The 160-char cap stays (`answers.rs:131`).
- **`review` and `collect` gain an optional progress sink** (FR-5). The run-less `factory review` CLI passes none (`cli/review.rs:86`, `:136`).
- **A new run file**, `ci/<round>.log`.
- **`docs/project.toml`** gains an optional `gate_failure` array under `[docs]`.

## 3. FR-1 — the verb, read-only and cheap

**Today.** `status --json` builds each run through `run_json` (`cli/status.rs:191-241`). `Run::open` runs `Db::open`, which creates directories and migrates `factory.db` under an IMMEDIATE transaction on every call (`db.rs:348-378`). `factory wait` writes `cockpit.cursor` (`wait.rs:53-61`).

**Decided 2026-11-07.**

```
factory live <id> --json [--since <seq>] [--wait <secs>] [--evidence-limit <n>]
```

- **Read-only, for real.** It loads `state.json` with `run::load_state` and events with `events::scan` (`events.rs:323-343`). It never calls `Run::open`, `Db::open` or `write_cursor`. Liveness uses `RunState::driver_alive` and `worker_alive` (`run.rs:129-135`, `:575-603`).
- **Output.** One compact JSON object on stdout. Diagnostics go to stderr. An unknown id prints `no run <id>` on stderr and exits 1, as `status --json` does (`cli/status.rs:116-125`). `--json` is required, and there is no human mode.
- **`--since <seq>`.** `timeline` holds only events with `seq > since`. Without it, the last 100. Every other section is a full snapshot.
- **`--wait <secs>`**, 0 to 25, default 0. It rereads every second and returns when `rev` changes, or after `secs`. It exits 0 on timeout, because a timeout is not an error here, unlike `wait`'s exit 3.
- **`--evidence-limit <n>`**, default 200, max 300.
- **`rev`** is a hash of the serialized snapshot minus its volatile fields: `at`, `now.idleSeconds`, `workers[].running.elapsedSeconds` and `workers[].lastWriteAt`. A hash over stream sizes would change every second while any worker writes.
- **Finished runs** answer with a static snapshot.
- **Budgets.**
  - Output is capped at 512 KiB, cut first in evidence, then in the timeline. `truncated` names the cut sections in order.
  - Per call it reads at most: `state.json`, `events.jsonl`, 1 MiB per live worker stream, the last 1 MiB of the current gate log and of the newest `ci/<round>.log` (the excerpt and the tail come from that read), one `lstat` walk, and `log.jsonl`.
  - It never calls status's `board()` or `last_rate_limit`.
- **Versioning.** Its own `schema` counter, starting at 1, under PRD-023's rules: an additive camelCase field keeps the number, and a rename, removal or change of meaning bumps it. The envelope carries `factory`, the crate version, as information.
- **Times.** Every time is UTC RFC3339, file mtimes included.
- **Old layouts.** A section whose files are missing is empty or null, never an error.

**Envelope:** `schema`, `factory`, `id`, `feature`, `phase`, `repo` (the path as `state.json` stores it, as `status --json` prints it: `cli/status.rs:218`), `branch`, `seq`, `rev`, `at`, `now`, `attention`, `workers`, `gate`, `ci`, `timeline`, `evidence`, `decisions`, `truncated`.

- `feature` is there because the herdr Answer dialog needs it (herdr-factory PRD-003 §6).
- `decisions` is `{ count }`, counted from the feature's `log.jsonl`. That file lives in the repo's feature folder, not the run dir (`docs/log.rs:127-128`), and is found the way `cli/status.rs:299-322` finds it.

**Accepted cost:** a second read path beside `Run::open`. A flag on `status` lost: status is polled every 10 s for every recent run, and this is one run and heavier. A `--follow` stream lost because herdr's `exec()` returns only at exit, within 30 s.

**Acceptance:**
- `live_answers_one_line_of_schema_1_json`.
- `live_writes_nothing`: the run dir, `factory.db` and its directory have the same mtimes and listing before and after a call.
- `live_since_returns_only_newer_events`.
- `live_wait_returns_when_something_visible_changes`: an event appended after 1 s ends a `--wait 5` call within 2 s.
- `live_wait_times_out_with_exit_0`.
- `live_rev_ignores_a_growing_stream`.
- `live_caps_output_and_names_what_it_cut`.
- `live_on_an_old_layout_run_is_empty_not_an_error`.
- `live_unknown_id_exits_1`.

## 4. FR-2 — now and attention

**Today.**
- Open questions come from `cockpit::questions` (`cockpit.rs:204-209`), and status renders them (`cli/status.rs:244-296`).
- The driver's parks go through `park_with` (`scheduler.rs:602-627`), directly or through `enter_park` (`:581-600`). It writes "parked (waiting-owner)" (`:616`) and emits `run_waiting {blocked, asks}` (`:621`), once per park.
- Preflight asks park differently: `cli/run.rs:114-128` emits `ask_opened`, sets `waiting-owner` and returns, with no `run_waiting`. Those parks always have a question, so they need no reason.
- Quota and budget parks are `run_paused` (`quota.rs:161`) and `run_parked` (`quota.rs:202`).
- `stop_requested` is in `state.json` (`run.rs:104`).

**Decided 2026-11-07.**
- **`now`:** `{ waitingOn, label, since, progress, resumesAt, idleSeconds, driverAlive, stopping }`.
  - `waitingOn` is `owner|worker|gate|review|ci|quota|budget|driver|nothing|done`. The first match in this order wins, so a quota pause with live workers is `quota`:
    1. `done`: the phase is over.
    2. `owner`: an open question, or a `park` entry (below).
    3. `budget`: the run is parked on a `run_parked` budget event (`cockpit::park`, `cockpit.rs:260-268`).
    4. `quota`: the phase is `paused`.
    5. `driver`: the driver is not alive and the run is not over. This includes an answer nobody picked up: the newest event is `block_answered` or `ask_answered`.
    6. `gate`: a `gate_started` with no later `gate_run`.
    7. `review`: a `collect_started` with no later `collect_done`.
    8. `ci`: the newest CI event is `ci_waiting` or `ci_no_checks`.
    9. `worker`: a worker is alive.
    10. `nothing`: none of the above.
  - `label` is factory's own sentence: "checkpoint 2 gate, attempt 2".
  - `progress` is `{done, of}` from FR-5's review events.
  - `resumesAt` is set for quota, from `run_paused.resets_at` (`quota.rs:161-164`). It is null for a budget park, which has no reset time (`quota.rs:202-206`).
  - `idleSeconds` is now minus the newest event, stream write or gate log write.
  - `stopping` is null or `{on: "worker|gate", what}`.
- **`attention[]`:** `{ id, kind: block|ask|park|warn, openedAt, title, body, reason, answer }`.
  - `id`, `title` and `answer` come from **one function shared with status**, so `answer` is exactly status's `questions[].answer` argv, `--feature` included.
  - `body` is the body of the `log.jsonl` entry whose `id` is the question's id, else null. A log entry has no author field (`docs/log.rs:67-91`), so `live` does not say who wrote it.
  - `reason` is `{source: gate|ci|land|driver, log, excerpt}`.
- **`run_waiting` gains `reason`**, a short string (`#[serde(default)]`), set from `park_with`'s `what`. With no question, it becomes a `kind: park` entry.
- **`kind: warn`** carries `polish_findings_missing` and `pack_section_missing`.

**Accepted cost:** `cockpit.rs:531` and `cli/status.rs:1069` build `RunWaiting` literals in tests and must be updated. A park whose `what` changes mid-park still emits once, so `reason` is the park's first reason.

**Acceptance:**
- `live_names_what_the_run_waits_on`: one fixture per `waitingOn` value.
- `live_attention_answer_is_status_answer`.
- `a_park_with_no_question_carries_its_reason`: an end-to-end dirty-landing park, as in `tests/commits.rs:549`, has `reason` in `run_waiting` and a `park` entry in `live`.
- `live_says_driver_after_an_answer_nobody_picked_up`.

## 5. FR-3 — workers

**Today.**
- `state.json` `workers[]` holds `WorkerRef` (`run.rs:138-153`). The stream is claude's stdout at `runs/<id>/workers/<session>.jsonl` (`worker.rs:18`, `:177-179`, `:234`).
- Build, fix and merge workers run with `--tools Bash,Read,Edit,Write` (`worker.rs:93-95`, `:226-229`).
- There is no CI-fix role: `Job::CiFix` maps to `"fix"` (`worker.rs:128-136`). It is told apart only through `CiFixSpawned` (`worker.rs:378`).
- `stream.rs` has no `tool_progress` variant (`stream.rs:10-35`).

**Decided 2026-11-07.**
- **Fields:** `{ sessionId, role, job, slice, checkpoint, lane, spawnedAt, alive, doing, narration, running, lastWriteAt, outcome }`.
  - `job` is `gateFix|ciFix|null`, matched by session_id against the spawn events.
  - `outcome` is `{status: done|blocked, block}` from the stream's last `result` line. Its `STATUS:` header is not always on line 1.
- **The tail reader:**
  - Reads backwards in 64 KiB blocks, at most 1 MiB per live worker.
  - Skips lines that do not start with `{` (the mise banner), lines over 64 KiB unparsed, and a partial last line.
  - `doing` is `{tool, label, at, inSubagent}` from the newest assistant `tool_use`. `label` is Bash `input.description`, else `Skill <name>` or `Agent <description>`, else the tool name plus a lane-relative path. **Never `input.command` and never tool output.**
  - `at` comes from the newest line with a `timestamp`, of any type, or null.
  - `narration` is the newest assistant text block, at most 160 chars.
  - `running` is `{tool, elapsedSeconds}` from the newest `tool_progress` whose tool_use has no later `tool_result`.
  - Every derived field is nullable. Claude Code's stream is not a contract.

  - **The heartbeat trap.** A `tool_progress` line's `tool_use_id` is `<id>-heartbeat-N`. The tool it belongs to is `parent_tool_use_id`, and that is what a later `tool_result` names.

**Fixtures, committed with this PRD on 2026-11-07.** The old recording, `claude-p-oneshot-2.1.280.jsonl`, holds no `tool_progress`. Two cuts of factory's own worker streams from Claude Code 2.1.316 sit beside it in `tests/fixture/recordings/`. Strings over 300 chars are cut and home paths read `/home/user`.
- `claude-stream-build-mid-bash-2.1.316.jsonl`: a build worker, 125 lines, ending on two Bash heartbeats with no `tool_result` yet.
- `claude-stream-polish-skill-2.1.316.jsonl`: a polish worker, 60 lines, starting with the mise banner line and calling `Skill`.

**Accepted cost:** a new reader beside `stream.rs`, which parses whole files at worker exit.

**Acceptance:**
- `live_worker_says_what_it_is_doing`
- `live_worker_label_never_carries_the_command`: a fixture whose `input.command` holds a fake token, and no output contains it
- `live_worker_running_clears_once_the_tool_returns`
- `live_worker_on_an_unknown_stream_shape_is_null_not_an_error`
- `live_skips_an_oversized_line`
- `live_tells_a_ci_fix_from_a_gate_fix`

## 6. FR-4 — gate, CI, and the failure excerpt

**Today.**
- The gate log is created in `gate_once` (`checkpoint.rs:495-501`) and written live. `gate_run` is emitted only after it ends (`:547`), and `gate_cached` at `:474`.
- The CI failure log is assembled in `on_red` (`closeout.rs:1681-1708`). It reaches the pack cut to 200 lines (`pack.rs:79`, `:620`), and is never saved whole.
- CI waiting is `driver.log` only, in `watch_ci` (`closeout.rs:1251-1455`).
- `docs/project.toml` keys have int, float, string and boolean readers (`gate_parallel`, `docs/project.rs:69-73`), and no array reader.

**Decided 2026-11-07.**
- **`gate`:** `{ n, attempt, cmd, running, startedAt, red, minutes, cached, log, bytes, tail, excerpt, fixEmpty }`.
  - `tail` is the last 40 lines × 300 chars, ANSI stripped.
  - `excerpt` is set when red.
  - `running` and `startedAt` come from FR-5's `gate_started`.
  - `fixEmpty` comes from `fix_empty`.
- **`ci`:** `{ state: none|waiting|red|green|noChecks, round, head, checks, excerpt }`.
  - `excerpt` comes from `ci/<round>.log`. `on_red` writes it with the BOM and ANSI stripped, numbered by the pack round, `round` at `closeout.rs:1507` (`ever.last() + 1`), so it matches `packs/ci-fix-<round>-*.md`. Not `rounds_done` at `:1506`, which `ci_red.round` stores: after an answered CI block the two differ.
- **The excerpt extractor**, one stack-neutral function that searches the whole log:
  1. Before stripping ANSI, mark red lines (SGR 31 or 91) as candidates. Then strip ANSI and CR.
  2. Structured lines first: split on `}{`, parse each JSON object, take `failures[].{test,file,line,message}`, and cap `message` at 300 chars.
  3. Then a generic set:
     - `^error`, `error[:\[]`, `\berror TS\d+`, `:\s*error\b`
     - `--- FAIL`, `^FAIL`, `✘`, `\[FAIL\]`, `Diff in `
     - `\bfailed\b`, but not `\b0 failed\b`
     - `timed out after`, `affected by \d+ vulnerabilit`
  4. Always add the line before the last `returned with error code` or `make: *** … Error`, and the last `=== N.` tier header.
  5. At most 20 lines with 2 of context, preferring the last matches.
- **`gate_failure`**: an optional array of regexes under `[docs]` in `docs/project.toml`, tried before the generic set.
  - It needs an array reader in `project.rs`, a `check.rs` arm (unknown keys pass today, `check.rs:218-229`), and a line in the `/gate` skill.
  - A bad regex is a `docs check` error, not a crash.

**Accepted cost:** a heuristic reader that will miss some stack's format. That is what `gate_failure` is for (owner's pick, 2026-11-07).

**Acceptance:** `excerpt_finds` tests, one per real format. Each fixture is a cut of a real log, scrubbed:
- `excerpt_finds_a_pest_json_failure`
- `excerpt_finds_a_tsc_error`
- `excerpt_finds_a_red_crap_row`
- `excerpt_finds_a_rustfmt_diff_at_the_top`
- `excerpt_ignores_zero_failed`
- `excerpt_uses_project_gate_failure_first`
- `docs_check_rejects_a_bad_gate_failure_regex`
- `live_shows_a_running_gate_tail`
- `ci_red_writes_the_whole_failure_log`

## 7. FR-5 — timeline and the new events

**Decided 2026-11-07.**
- **`timeline[]`:** `{ seq, ts, kind, tone: owner|red|green|info|noise, label, data }`.
  - `label` is factory's short sentence, extending the board's shortener (`cli/status.rs:680-719`).
  - `data` is a whitelisted subset per kind. Lists and blobs become counts.
  - `tone`, one table over every `EventKind`:
    - `owner`: `ask_opened`, `run_waiting`, `run_parked`, `slice_blocked`, `checkpoint_blocked`, `closeout_blocked`, `block_answered`, `ask_answered`.
    - `red`: `gate_run` red, `ci_red`, `worker_dead`, `merge_refused`, `landing_conflicted`, `fix_empty`, `run_failed`.
    - `green`: `gate_run` green, `ci_green`, `slice_done`, `checkpoint_done`, `polish_done`, `pr_opened`, `shipped`, `run_finished`.
    - `noise`: `ci_waiting` and `review_pass_done`.
    - `info`: every other kind, `unknown` included.
- **New events:**
  - `gate_started {n, attempt, cmd, log}`, in `gate_once` after the cache miss and before `File::create` (`checkpoint.rs:495-501`).
  - `collect_started {passes, dir, reused}`, on both the fresh path (`closeout.rs:352-354`) and the reuse path (`:342-350`). `collected()` (`:335`) gains a `run` parameter, or the emits move to its caller at `:196`. A small diff that skips the second opinion has `passes: 0` (`collect.rs:49-59`).
    - Corrected 2026-11-07 at slicing: at `closeout.rs:352-354` neither `passes` nor `dir` is known yet. `collect` groups the passes and makes the review dir inside `second_opinion` (`collect.rs:124`, `:127`), after the leaks scan (`:47`). So the fresh path's `collect_started` is the sink's first message. The reuse path reads both from the file (`log:` at `collect.rs:153`, the `### Pass` headings at `:146`). The reuse path also emits `collect_done {seconds: 0, ok: true}`, or §4 rule 7 reads a reused collect as a review that never ends.
  - `review_pass_done {k, of, seconds}`, emitted as each pass ends. `run_passes` prints every line up front and joins in order (`review.rs:405-443`). `Db` is not Sync (`db.rs:172-174`), so the pass threads send on a channel, and the caller emits through `run`.
    - `collect` and `run_passes` take an `Option` sink.
    - The run-less `factory review` CLI passes `None` (`cli/review.rs:86`, `:136`).
  - `collect_done {seconds, ok}` at `closeout.rs:359-378`.
  - `ci_waiting {checks}`, in `watch_ci` beside the waiting line (`closeout.rs:1442-1444`), emitted only when the set of running check names changes. Today's line dedups on the whole `why` text, so the event needs its own dedup. The set is rebuilt from `rows` (declared `:1307`, filled `:1364`) with `ci::class` (`ci.rs:196`). Pending reasons that are not checks are not in `checks`: head lag, rerun, grace.
  - `ci_no_checks {minutes}` at `closeout.rs:1375-1384`.
  - `fix_empty {n, attempt, session_id}` in `fix_once` (`checkpoint.rs:621-625`), and `fix_empty {round}` for CI in `nothing()` (`closeout.rs:643-648`).
  - `run_waiting.reason`, FR-2.

**Accepted cost:** `review.rs` and `collect.rs` change shape to carry the sink. That is the refactor behind the longest silence in the research.

**Acceptance:**
- `review_passes_report_as_they_finish`: a two-pass collect where pass 2 ends first emits `review_pass_done` k=2 before k=1.
- `gate_started_comes_before_gate_run`.
- `ci_waiting_emits_on_a_change_of_checks_only`.
- `an_empty_fix_is_an_event`, for both a gate fix and a CI fix.
- `live_timeline_tones_follow_the_table`: one event of each tone gets that tone.
- `old_binary_skips_new_events`, through `EventKind::Unknown`.

## 8. FR-6 — evidence

**Today.** `$FACTORY_EVIDENCE` is `runs/<id>/evidence` (`worker.rs:58`, `:261-263`). The `cp-N/` layout is a convention of `assets/contract-checkpoint.md:15-17`, and no code enforces it.

**Decided 2026-11-07.**
- **`evidence`:** `{ root, checkpoints: [{ n, state: writing|done, files }], skipped, truncated }`.
  - Each file is `{ path, kind: image|text|document|archive|other, bytes, mtime, stable, etag, caption, citedBy, sidecars }`.
  - `path` is relative to `root`, never absolute and never containing `..`.
  - `stable` means the mtime is older than 2 s.
  - `etag` is `<bytes>-<mtimeMs>`.
  - `caption` is the file stem, humanized.
  - `citedBy` lists the log entries that name the file, by substring.
  - `.aria.txt` files attach as `sidecars`. Every other file is its own row.
- **Listing:**
  - `lstat` only: regular files, no symlinks, FIFOs or devices (counted in `skipped`), no link following, depth ≤ 4.
  - Only this run's `cp-N` directories, by the `n` in its events. Numbers continue across runs of a feature.
  - Checkpoints newest first, then files by mtime descending, then name.
  - File contents are never read, except for `citedBy`, which reads `log.jsonl` only.

**Accepted cost:** captions are file names, which are only as good as what the checkpoint worker typed. A caption manifest lost: it changes every repo's checkpoint contract, and no owner moment needed captions.

**Acceptance:**
- `live_lists_evidence_newest_first`
- `live_skips_a_symlink_in_evidence`
- `live_evidence_paths_stay_under_root`
- `live_marks_a_file_written_this_second_unstable`
- `live_evidence_respects_the_limit`

## 9. FR-7 — redaction

**Decided 2026-11-07.**
- One pass over **every string** in the output, at serialization time, not over a list of fields. In the research, secrets sat in `run_failed.error`, raw gh stderr, CI excerpts and gate logs.
- Token shapes:
  - `sk-ant-`, `ghp_`, `github_pat_`, `AKIA…`, `xox?-`, JWT and `Bearer \S+`
  - case-insensitive env-style `(KEY|SECRET|TOKEN|PASSWORD|PASS|APP_KEY)=…`
  - JSON `"(password|secret|token|key)": "…"`
  - `X-API-Key:` headers
  - URL path tokens of 41 or more hex or base62 characters, which keeps 40-hex git shas
- Home paths become `~`, except the envelope's `repo`, which §3 keeps as stored (corrected 2026-11-07 at slicing). Token shapes still apply to it.

**Accepted cost:** best effort only, and screenshots can still show tokens. The consumer keeps images local (herdr-factory PRD-003 §9).

**Acceptance:**
- `live_redacts_every_string`: a fixture plants one of each shape in an event payload, a gate log, a CI log, a block body and a stream narration, and none comes out.
- `live_keeps_a_git_sha`.

## 10. FR-8 — a red gate's block title is its failure excerpt

**Today.** A red gate's block question is the gate log's last non-empty line (`checkpoint.rs:125-135`), through `answers::block_title` (`answers.rs:134-147`, capped at 160). Other callers are `checkpoint.rs:932`, `closeout.rs:2140` and `add_block` (`answers.rs:231`).

**Decided 2026-11-07** (owner's pick: in this PRD, as its own slice).
- The red-gate title takes the excerpt's first line, behind the existing "gate red … at checkpoint N:" prefix. It still goes through `block_title` and its 160-char cap, and falls back to the last line when the excerpt is empty.
- The release notes call out the change of text. It is not a schema change.

**Accepted cost:** the herdr strip and dialog, and any reader of `questions[].text`, see different words for the same question.

**Acceptance:** `a_red_gate_block_names_the_failure`: an end-to-end red gate, run like `three_reds_block_the_run_and_the_answer_reruns_the_gate` (`tests/checkpoints.rs:1071`; corrected 2026-11-07 at slicing, this said `:883`, which is red-red-green and never blocks). The fixture gate `tests/fixture/gate.sh:27-29` writes one line, so this one needs a new fixture gate that prints a pest failure mid-log and ends in a useless line. The block's title names the failing test.

## 11. Decisions log

- ~~Source~~: decided 2026-11-07, a factory contract (owner's pick), over the panel reading run files.
- ~~Block title~~: decided 2026-11-07, in this PRD as its own slice (owner). Gate blocks only: corrected 2026-11-07, see §14.
- ~~`gate_failure`~~: decided 2026-11-07, kept in v1 (owner).
- ~~The cut~~: decided 2026-11-07, the owner kept every extra listed: `stopping`, `decisions.count`, worker `narration`, `warn` entries, `citedBy`, `.aria.txt` sidecars and `ci_no_checks`.
- ~~Dropped on evidence~~: decided 2026-11-07.
  - A transcript viewer: streams run to 9.4 MB and hold secrets.
  - Review contents.
  - A cost meter: `budget.used` moves only at worker exit.
  - Image dimensions and `dupOf` hashing: the consumer learns them, or dedupes by etag.
  - A caption manifest.
- **Review corrections, 2026-11-07.** A grok-4.7-high pass (`~/.local/share/factory/reviews/20261107-004638`) found these in the first draft, and the sections now carry the fix:
  - The CI excerpt file was numbered from `:1506` (`rounds_done`). That is not the pack's round, `:1507`.
  - `resumesAt` was promised for a budget park, which has no reset time.
  - `body` was split by author (`bodySource`). Log entries have no author.
  - The CI block title was to come from `ci/<round>.log`. CI blocks open at `closeout.rs:1535`, `:1595`, `:1642` and `:1672`, before `on_red` fetches the log, so FR-8 covers gate blocks only.
  - `repo` was called `~`-relative. Status prints the stored path.
  - `nothing` had no rule and tones had no table. Both are now listed.
  - Rebutted: `scheduler.rs:205` and `:455` call `enter_park`, which itself calls `park_with` (`:600`).
- **Slice recon corrections, 2026-11-07.** Re-verifying §15 at slicing found these, and the sections now carry the fix:
  - §10 modelled the FR-8 test on `tests/checkpoints.rs:883`. That test never blocks; the red-gate block test is `:1071`, whose title assert (`:1106`) stays green because the generic `^FAIL` matches its one line.
  - §7 put `collect_started` at `closeout.rs:352-354`, where `passes` and `dir` are not known yet. It rides the progress sink instead.
  - §7 gave the reuse path a `collect_started` and no `collect_done`, so §4 rule 7 would read it as a review forever. The reuse path emits both.
  - §9's home rewrite over every string contradicted §3's stored `repo`. `repo` is exempt from the home rewrite only.
- **Recon corrections, 2026-11-07.** The idea's draft cited these wrong, and §3 to §10 carry the corrected facts:
  - `state.json` fields at `run.rs:357-433`: they are setters, and `RunState` is `:93-126`.
  - A `ciFix` role, which does not exist.
  - The `/proc` check at `cockpit.rs:272-280`: that is `driver_dead`, which is false on a budget park and on preflight asks. A `run_waiting` park with a dead driver is `true`.
  - `ci.rs:392` as where the pack fetches the CI log: it is `on_red`, `closeout.rs:1681-1708`.
  - The CI loop in `ci.rs`: it is `watch_ci` in `closeout.rs`.
  - `land.rs` writing "cannot land …": it returns `Landing::Dirty`, and three callers write the text.
  - `Run::open` treated as read-only: it migrates `factory.db`.

## 12. QA plan

**Gate**: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test && claude plugin validate mods/cockpit && claude plugin test mods/cockpit`.
**Surface**: the `factory live` CLI and `events.jsonl`. There is no UI: every FR is proven below the UI.
**Harness**: the integration tests' fixture helpers.
- `tests/cockpit.rs`: `write_run` (`:809-840`), `write_live_run` (`:427`), `status_json` (`:256`), `factory_in` (`:24`).
- `tests/harness/mod.rs`: `Fixture`, `script_gate` (`:472`), `script_ci` (`:481`).
- The `claude` and `gh` stubs in `tests/stubs/`.

New helpers write `workers/`, `gates/`, `ci/` and `evidence/`. Tests read fixtures, never the machine's runs folder or this repo's `docs/`.

**Seed state**:
- Fixture runs per `waitingOn` value.
- Scrubbed cuts of real gate logs, one per excerpt format.
- The two stream cuts from Claude Code 2.1.316, already committed (§5).
- A planted-secrets fixture.

| FR | Proof | Evidence | Human? |
|---|---|---|---|
| FR-1 | run `live_answers_one_line_of_schema_1_json`, `live_writes_nothing`, `live_wait_returns_when_something_visible_changes`, `live_rev_ignores_a_growing_stream`, `live_caps_output_and_names_what_it_cut` | test transcript | no |
| FR-2 | run `live_names_what_the_run_waits_on`, `a_park_with_no_question_carries_its_reason`, `live_attention_answer_is_status_answer` | test transcript | no |
| FR-3 | run `live_worker_says_what_it_is_doing`, `live_worker_label_never_carries_the_command`, `live_worker_on_an_unknown_stream_shape_is_null_not_an_error` | test transcript | no |
| FR-4 | run the `excerpt_finds_*` tests, `ci_red_writes_the_whole_failure_log`, `docs_check_rejects_a_bad_gate_failure_regex` | test transcript | no |
| FR-5 | run `review_passes_report_as_they_finish`, `ci_waiting_emits_on_a_change_of_checks_only`, `an_empty_fix_is_an_event` | test transcript | no |
| FR-6 | run `live_skips_a_symlink_in_evidence`, `live_evidence_paths_stay_under_root` | test transcript | no |
| FR-7 | run `live_redacts_every_string`, `live_keeps_a_git_sha` | test transcript | no |
| FR-8 | run `a_red_gate_block_names_the_failure` | test transcript | no |

No row needs a human, so no `qa-guide.md`. The panel's look is herdr-factory PRD-003's human check.

## 13. Slices

Nine slices on one branch, `feat/live-run`, one PR, one commit per slice. Prove rows are driven at the checkpoints with the full gate (`checkpoint_every = 2`): after the 2nd, 4th, 6th and 8th done slice, and once at the end. Every row is a `run` row, proven below the UI.

- **01, the spine** (FR-1, the FR-5 event kinds and `timeline[]`, FR-2's `run_waiting.reason` field): the read-only verb, the full envelope, `rev`, `--wait`, the cap, the timeline and its tone table, and a minimal `workers[]`. It also lays out one source file and one test file per section (`src/live/<section>.rs`, `tests/live/<section>.rs` under `tests/live.rs`), each a stub, so the branch slices never edit the same file. It declares every new event kind in `events.rs`, so the emitting and the reading slices run in parallel. It goes first and alone: everything depends on it.
- **02, review progress** (FR-5: `collect_started`, `review_pass_done`, `collect_done`): the progress sink through `review.rs` and `collect.rs`. **Highest risk:** it reshapes the review library the run-less CLI shares. It is numbered first among the branches so it starts early.
- **03, gate, CI and the excerpt** (FR-4): the stack-neutral extractor, `gate_failure` with its `docs check` arm and `/gate` line (a skill edit, so a patch version bump and a relock), the saved `ci/<round>.log`, and the `gate`/`ci` sections. It precedes 05, 08 and 09, which reuse the extractor or the logs it reads.
- **04, driver phase events** (FR-5: `gate_started`, `ci_waiting`, `ci_no_checks`, `fix_empty`): emit-only, in `checkpoint.rs` and `closeout.rs`.
- **05, now and attention** (FR-2): `waitingOn` and the rest of `now`, `attention[]` with status's own answer argv, the park entry and its reason. It depends on 03 because `attention[].reason.excerpt` comes from the extractor.
- **06, workers** (FR-3): the bounded tail reader over the two committed 2.1.316 streams, `job` and `outcome`.
- **07, evidence** (FR-6): the `lstat` listing, `citedBy`, sidecars, the limit.
- **08, the red-gate block title** (FR-8, the owner's own slice): the first excerpt match in place of the last log line. It waits on 03 only.
- **09, redaction** (FR-7): one pass over every string. It waits on 03, 05 and 06 because its acceptance plants secrets in the gate and CI logs, a block body and a worker narration.

**Build shape.** 01 is the spine. 02, 03, 04, 06 and 07 branch off it and are mutually independent, which is five ready slices for 4 lanes. 05 and 08 branch off 03. 09 joins 03, 05 and 06. The reader slices (03, 05, 06, 07, 09) work from fixture events, so none waits on the emitting slices (02, 04). The only file two branches both edit is `src/closeout.rs`, in separate functions: `collected()` (02), `on_red` (03), `watch_ci` and `nothing()` (04). `src/checkpoint.rs` is shared the same way: `gate_once` and `fix_once` (04), the red title at `:124-135` (08).

No QA plan row says Human, so there is no `qa-guide.md`.

## 14. Non-goals

- **No writes of any kind** from `live`: not `cockpit.cursor`, not `factory.db`, not caches.
- **No `--follow` stream, no ETA, no cost meter, no cross-run gallery.**
- **No transcript, pack, collect.md or review viewer.**
- **Not handled: screenshots outside the run dir** (lane `artifacts/`, `/tmp/polish-*`). They cannot be tied to a run. Reopen through the checkpoint contract, copying into `$FACTORY_EVIDENCE`, if the owner asks for builder shots.
- **Not handled: a "stuck" verdict on a worker.** Long silences are normal.
- **Not handled: a CI block's title** (2026-11-07). CI blocks open before the CI log is fetched. Reopen if CI parks keep reading as unclear once `live` shows `ci.excerpt`.

## 15. Verification notes

Checked 2026-11-07 on `feat/live-run` (`src/` identical to `main`, 0.13.0) by a recon pass over every claim. **Re-verify at slice entry.**

Re-verified 2026-11-07 at slicing, still with `src/` identical to `main`. Every line below held, within a line or two. The corrections are in §11.


- `run.rs:93-126` is `RunState`: `stop_requested :104`, `stopped :106`, `paused_until :109`. `WorkerRef` is `:138-153`. `driver_alive :129-131` and `worker_alive :133-135` work over `/proc` (`:575-603`).
- `db.rs:348-378`: `Db::open` creates dirs and migrates under IMMEDIATE. `Db` holds a non-Sync `rusqlite::Connection` (`:172-174`).
- `events.rs:284-318` `emit` rereads the file for the last seq (`tail :363-381`) and inserts an SQLite row (`:314`). `scan` is `:323-343`, and `Unknown` is `:257-259`. `RunWaiting {blocked, asks}` is `:57-60`.
- `cli/status.rs:77-84` `QuestionJson`, `:191-241` `run_json`, `:244-296` questions, `:299-322` the feature folder scan, `:700-718` `short()` subject pick, `:116-125` the unknown id.
- `cockpit.rs:204-209` `questions`, `:247-253` `in_flight`, `:272-280` `driver_dead`, `:297-300` `write_cursor`.
- `worker.rs:18`, `:58`, `:93-95`, `:128-136`, `:177-179`, `:226-229`, `:234`, `:261-263`, `:378`.
- `stream.rs:10-35`: no `tool_progress`. Timestamps are read at `:336`, `:369` and `:434-439`.
- `checkpoint.rs:125-135` red title, `:474` `GateCached`, `:495-501` log create, `:547` `GateRun`, `:621-625` empty fix.
- `review.rs:405-443` passes. `collect.rs:43` writer, `:47` leaks, `:49-59` small diff, `:107`, `:128`.
- `closeout.rs:196`, `:335`, `:342-354`, `:359-378`, `:643-648`, `:1251-1455` `watch_ci`, `:1375-1384`, `:1506-1511` rounds, `:1681-1708` `on_red`.
- `scheduler.rs:602-627` `park_with`, and its callers `:181`, `:205`, `:275`, `:438`, `:450`, `:455`, `:467`, plus `checkpoint.rs:829` and `closeout.rs:622`.
- `pack.rs:79`, `:594`, `:620`, `:627`: the CI log is cut to 200 lines.
- `answers.rs:131`, `:134-147`, `:231`.
- `docs/project.rs:10-78`, `docs/check.rs:218-246`: int, float, string and bool readers (`:69-73`), no array reader, and unknown keys pass.
- `tests/cli.rs:8-11` `SUBCOMMANDS` needs `live`.
- `tests/fixture/recordings/claude-p-oneshot-2.1.280.jsonl` has no `tool_progress`. The two 2.1.316 cuts beside it do.
- `closeout.rs:1442-1444` is the CI waiting line; `:1506` is `rounds_done`, `:1507` the pack `round`; `rows` is `:1307` and `:1364`.
- `cli/run.rs:114-128`: preflight asks park with no `run_waiting`.
- `quota.rs:161-164` `run_paused.resets_at`; `quota.rs:202-206` `run_parked` has none.
- `docs/log.rs:67-91` `Entry` has no author; `:127-128` the log path.
- `cockpit.rs:260-268` `park` matches `RunParked` only.
