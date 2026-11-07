---
title: "factory live: a read-only live-run contract for the herdr cockpit side panel"
state: "open"
added: "2026-11-07"
origin: "herdr-factory PRD-003"
retrigger: "herdr-factory PRD-003 slicing"
---

## What

A read-only verb, `factory live <id> --json`, that returns one run's live detail in a versioned contract. The herdr-app factory cockpit (herdr-factory PRD-003) draws a side panel from it. Today a panel would have to read `driver.log`, worker transcripts, gate logs and `evidence/` blind, and those are not a contract.

## Why: what the owner could not see, in 70 runs across 7 stacks (2026-11-06)

Ranked by how often an owner needed to know something live and had no signal, and how long the gap lasted.

1. **Why it waits on me.**
   - Parks lasted 6 to 9 h behind block text that said nothing: nine blocks read "gate red three times at checkpoint N: Script @ci:check was called via verify". The cause was a JSON `failures[]` line in the gate log.
   - A 9 h park on a push failure, while `factory report` said "blocks: 0".
   - 32 of the 33 `run_waiting` events on the machine have empty `asks` and `blocked`. Their reason ("… changed in the main tree") is in `driver.log` only.
   - Runs re-blocked 5 to 10 s after an answer (4 runs).
   - An answered block was followed by 56 min of nothing, until the owner stopped the run.
2. **What the run does now.**
   - The longest silences are the driver's own work. A 15 to 30 min review collect after every last checkpoint, with no event: 1023 to 1801 s in 7 runs.
   - Gates of 9 to 33 min.
   - CI waits of 8 to 31 min.
   - A driver dead for 379 min, seen only after the fact.
   - Stop lag of 3 to 13 min, behind a worker or a running gate.
3. **What each agent does.**
   - Slices of 30 to 106 min.
   - Silences of 10 min are normal inside one long Bash call.
   - Bash `input.description` is set on most calls (325 of 462), and builders narrate in short text blocks.
4. **The failing reason of a gate or CI round.**
   - The reason is often not at the tail. CRAP rows sit mid-file under about 480 warning rows, rustfmt puts it at the top, and composer chains end on a line with no information.
   - CI failure logs exist only inside `packs/ci-fix-*.md`.
   - Fix workers that "changed nothing to commit" re-serve a cached red, and that shows only in `driver.log`.
5. **Timeline.** The moments are sequences: 4 blocks in 9 min on one slice, then conflicts and red gates in a row; and conflict → merge worker pairs.
6. **Checkpoint evidence.**
   - PNGs land mostly in bursts near a checkpoint's end, so this is "proof just landed", not a live camera.
   - Many stacks make none at all, and some make PDFs, XML, zips or text.
7. **Decisions.** No owner moment needed them live, so a count is enough.

Dropped on evidence:
- **A transcript viewer.** Transcripts run 60 KB to 9.4 MB, with single lines up to 750 KB of base64, and they hold secrets: an API-key-shaped string, `PGPASSWORD`, `APP_KEY`, a reset token.
- **Review stream contents.**
- **A live cost meter.** `budget.used` moves only when a worker exits.
- **Screenshots in shared lane `artifacts/` or `/tmp/polish-*`.** They cannot be tied to a run.

## The verb

```
factory live <id> --json [--since <seq>] [--wait <secs>] [--evidence-limit <n>]
```

**Inputs:**
- An unknown id prints `no run <id>` on stderr and exits 1, as `status` does (`src/cli/status.rs:115-123`).
- One compact JSON object goes to stdout. Diagnostics go to stderr. There is no human mode in v1.
- `--since <seq>`: `timeline` holds only events with `seq > since`. Without it, the last 100. Every other section is always a full snapshot.
- `--wait <secs>` (0 to 25, default 0) is a long poll. It returns when `rev` changes, or after `secs`. It exits 0 either way, because a timeout is not an error here, unlike `wait`'s exit 3. It stays under herdr's 30 s `exec()` limit.
- `--evidence-limit <n>` defaults to 200, with a max of 300.
- It works on finished runs too, as a static snapshot.

**Read-only.** It writes nothing: not `cockpit.cursor` (`factory wait` moves it, `src/cli/wait.rs:53-61`), and no caches in the run dir. The `--since` cursor lives in the caller.

**Not folded into `status --json`.** Status is polled every 10 s for every recent run, so it must stay cheap. This verb is for one run and is heavier.

## Contract, schema 1

**Versioning:** its own `schema` counter, with PRD-023's rules: additive camelCase fields keep the number, and a rename, removal or change of meaning bumps it. The envelope carries `factory` (the crate version) as information only. Consumers pin `schema`, never `factory`. Every time is UTC RFC3339, including converted file mtimes. Every section is empty or null when its files are missing (old run layouts), never an error.

```jsonc
{
  "schema": 1, "factory": "0.14.0",
  "id": "…", "feature": "007", "phase": "checkpoint", "repo": "~/…", "branch": "feat/…",
  "seq": 61,
  "rev": "…",          // changes only when something visible changes (below)
  "at": "…",
  "now": {
    "waitingOn": "owner|worker|gate|review|ci|quota|budget|driver|nothing|done",
    "label": "checkpoint 2 gate, attempt 2",
    "since": "…",
    "progress": null,  // {done, of}, e.g. review passes
    "resumesAt": null, // quota and budget parks
    "idleSeconds": 12, "driverAlive": true,
    "stopping": null   // or {on: "worker|gate", what: "slice 01"}
  },
  "attention": [{
    "id": "BLOCK-3", "kind": "block|ask|park|warn", "openedAt": "…",
    "title": "…",      // same text as status questions[].text
    "body": null, "bodySource": "worker|driver|null",
    "reason": null,    // {source: "gate|ci|land|driver", log: "gates/2-3.log", excerpt: [..]}
    "answer": ["factory", "answer", "--feature", "007", "BLOCK-3"]  // exactly status's questions[].answer
  }],
  "workers": [{
    "sessionId": "…",
    "role": "build|checkpoint|polish|fix|merge|ciFix",
    "slice": 3, "checkpoint": null, "lane": 2, "spawnedAt": "…",
    "alive": true,
    "doing": null,     // {tool, label, at|null, inSubagent}
    "narration": null, // ≤ 160 chars
    "running": null,   // {tool, elapsedSeconds}, only while no tool_result follows it
    "lastWriteAt": "…",
    "outcome": null    // {status: "done|blocked", block: "BLOCK-n"|null}
  }],
  "gate": {
    "n": 2, "attempt": 2, "cmd": "…", "running": true, "startedAt": "…",
    "red": null, "minutes": null, "cached": false,
    "log": "gates/2-2.log", "bytes": 41233,
    "tail": ["…"],     // last 40 lines × 300 chars, ANSI stripped
    "excerpt": [],     // red only, see the extractor
    "fixEmpty": false
  },
  "ci": { "state": "none|waiting|red|green|noChecks", "round": null, "head": null, "checks": [], "excerpt": [] },
  "timeline": [{ "seq": 58, "ts": "…", "kind": "gate_run", "tone": "owner|red|green|info|noise",
                 "label": "checkpoint 2 gate red in 1.0 min", "data": {} }],
  "evidence": {
    "root": "/abs/…/runs/<id>/evidence",
    "checkpoints": [{ "n": 2, "state": "writing|done", "files": [{
      "path": "cp-2/settle-before-dark.png", // relative to root, never "..", never absolute
      "kind": "image|text|document|archive|other",
      "bytes": 104233, "mtime": "…",
      "stable": true,  // mtime older than 2 s
      "etag": "104233-1762459202000",
      "caption": "settle before dark",       // the file stem, humanized
      "citedBy": ["CP-2"],                   // log entries naming this file, by substring
      "sidecars": ["cp-2/settle-before-dark.aria.txt"]
    }]}],
    "skipped": 0, "truncated": false
  },
  "decisions": { "count": 14 },
  "truncated": []      // which sections were cut, in cut order
}
```

## Where each part comes from, and what is new

**Already in factory:**
- `id`, `feature`, `phase`, `repo`, `branch`: `state.json` (`src/run.rs:357-433`). The panel's Answer dialog needs `feature`, and the same `answer` argv status gives: `--feature` goes before the id, because the answer text is a trailing var-arg.
- `seq` and `timeline`: `events.jsonl`, with the tolerant scan (`src/events.rs:284-343`).
- Questions, title and answer argv: shared with status (`status.rs:77-84, 244-296`). Move them into one function, so the two cannot disagree.
- Workers' role, slice, lane, spawn time and alive: `state.json` `workers[]` (`src/run.rs:138-153, 579-587`), plus `worker_dead`.
- `driverAlive`: the `/proc` check (`src/cockpit.rs:272-280`).
- `stopping`: `stop_requested`.
- Quota and budget: `run_paused` and `run_parked`, with status's `pausedUntil` and `parked`.
- `decisions.count`: `log.jsonl`.

**New events** (all additive; older readers skip them through `Unknown`, `src/events.rs:257-259`):
- `gate_started {n, attempt, cmd, log}`, emitted before the log is created (`src/checkpoint.rs:496-501`).
- `collect_started {passes, dir, reused}`, `review_pass_done {k, of, seconds}` and `collect_done {seconds}`.
  - **This is a refactor, not one append.** `review::run_passes` prints every progress line before it spawns anything, and joins in pass order (`src/review.rs:405-440`). `collect` gets only a `&mut dyn Write`.
  - Pass an emitter down, and emit from inside each pass thread. Emit `collect_started` on the reuse path too (`closeout.rs:342-351`).
- `ci_waiting {checks[]}`, emitted only when the set of running checks changes, and `ci_no_checks {minutes}`.
- `fix_empty {n, attempt, session_id}`. Today it appears in `driver.log` only.
- **`reason` on `run_waiting`**, at every site that writes "parked (waiting-owner)" or "cannot land: … changed in the main tree" (scheduler, closeout, `land.rs`). Recon lists them all, and a test covers each.
- `waitingOn: driver` when the newest owner answer has no later event, no worker is alive and no gate runs.

**New files:** `ci/<round>.log`, written where the ci-fix pack already fetches `gh run view --log-failed` (`src/ci.rs:392`). Strip the BOM and ANSI.

**`rev`:** a hash of the serialized snapshot minus the volatile fields (`at`, `idleSeconds`, `running.elapsedSeconds`, `lastWriteAt`). Hashing stream sizes would change it every second while any worker writes thinking or progress lines, and `--wait` would never wait.

## Rules the verb must keep

**Redaction.**
- One pass over every string in the output at serialization time, not a list of fields. `timeline[].data` (`run_failed.error`, raw gh stderr), titles, `ci.checks` and CI excerpts all carried secrets in the evidence.
- Token shapes:
  - `sk-ant-`, `ghp_`, `github_pat_`, `AKIA…`, `xox?-`, JWT and `Bearer \S+`
  - case-insensitive env-style `(KEY|SECRET|TOKEN|PASSWORD|PASS|APP_KEY)=…`
  - JSON `"(password|secret|token|key)": "…"`
  - `X-API-Key:` headers
  - URL path tokens of 41 or more hex or base62 characters, which keeps 40-hex git shas
- Home paths become `~`.
- Best effort only: screenshots can show tokens, and nothing scrubs pixels.

**Worker tail reader.** Claude Code's stream-json is not a contract, so every derived field is nullable.
- Read the stream backwards in 64 KiB blocks, at most 1 MiB per live worker.
- Skip lines that do not start with `{` (the mise banner), lines over 64 KiB unparsed (base64 tool results), and a partial last line.
- `doing.label` comes from Bash `input.description`, else `Skill <name>` or `Agent <description>`, else the tool name plus a lane-relative path. **Never `input.command`, never tool output.**
- `narration` is the newest assistant text block.
- `doing.at` comes from the newest line that has a `timestamp`, of any type. Assistant and user lines carry it differently across Claude Code 2.1.296 to 2.1.316.
- `running` comes from the newest `tool_progress` whose tool_use has no later `tool_result`.
- Keep fixture streams from two Claude Code versions, and one unknown-shape test that asserts nulls, not errors.

**Failure excerpt extractor.** Stack-neutral, and it searches the whole log, not the tail.
1. Before stripping ANSI, mark red lines (SGR 31 or 91) as candidates. Then strip ANSI and CR.
2. Structured lines first: split on `}{`, parse each JSON object, take `failures[].{test,file,line,message}`, and cap `message` at 300 chars.
3. Then a generic set:
   - `^error`, `error[:\[]`, `\berror TS\d+`, `:\s*error\b`
   - `--- FAIL`, `^FAIL`, `✘`, `\[FAIL\]`, `Diff in `
   - `\bfailed\b`, but not `\b0 failed\b`
   - `timed out after`, `affected by \d+ vulnerabilit`
4. Always add the line before the last `returned with error code` or `make: *** … Error`, and the last tier header (`=== N.`).
5. Keep at most 20 lines with 2 lines of context, preferring the last matches.
6. A project override, `gate_failure = ["<regex>", …]` under `[docs]` in `docs/project.toml`, ships in v1 and is documented in `/gate`.
7. Using the same extractor for the block question's title changes `status` `questions[].text` for existing consumers. Do it in its own slice, with a change note. It stays schema 1, because the meaning is the same, and the 160-char cap stays.

**Evidence listing.**
- Use `lstat` only: regular files, no symlinks, FIFOs or devices (counted in `skipped`), no link following, depth ≤ 4.
- Only this run's `cp-N` directories, by the `n` in its events. Numbers continue across runs of a feature, so never assume cp-1.
- Checkpoints come newest first, and files by mtime descending, then name.
- `.aria.txt` files attach as sidecars. Every other file, including a same-stem `.txt`, is its own row.
- Never inline file contents.
- **Out of v1:** an agent-written caption manifest, `dupOf` hashing, and image dimensions. No owner moment needed them, and each costs on every poll.

**Budgets.**
- Output is capped at 512 KiB, cutting evidence first, then the timeline, and naming what was cut in `truncated`.
- Per call it reads `state.json`, `events.jsonl`, each live stream's tail, the current gate log's last 16 KiB (up to 64 KiB for a red excerpt), one `lstat` walk, and `log.jsonl`.
- It never calls status's `board()` or `last_rate_limit`.
- Tests pin bytes read and files opened on fixture runs, never the machine's runs folder. Wall time is a bench, not a gate: load average reached 14 to 20 during gates.

## Non-goals

- No transcript, pack, collect.md or review viewer.
- No writes of any kind.
- No `--follow` stream.
- No ETA, no cost meter, no cross-run gallery.
- No crawling of shared lane folders. A later fix is contractual: checkpoint or polish workers copy chosen shots into `$FACTORY_EVIDENCE`.

## Consumer

herdr-factory PRD-003 is a Run side panel. It polls `live <id> --since <seq> --wait 20` while visible, and copies stable images with `cp -P` into `<HERDR_APP_HOME>/data/factory-run/shots/<run>/<etag>.png`. herdr's `image()` loads only paths relative to its home: `data/…` works and an absolute path does not (checked 2026-11-06, herdr-app 0.1.13). It never copies under `mods/`, which can be a Dropbox symlink.

## Evidence

The research ran on 2026-11-06 over 70 runs in `~/.local/share/factory/runs` and the factory source at 0.13.0: six run readers by stack, one source reader, a draft and an adversarial critique (39 findings, all folded in above). The full draft and the critique are available on request from the herdr-factory session.
