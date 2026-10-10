## Traps

- 2026-10-10, run 1: `tests/closeout.rs::the_driver_never_merges_or_readies_a_pr` scans `src/` for `pr merge`, and the injected deny rule `"Bash(gh pr merge*)"` in `src/worker.rs` tripped it. The scan now strips that one literal before it looks.

## Blockers

_(none)_

## Decisions

### AD-1301 — `--settings` as inline JSON or as a file? · slice-13 · 2026-10-10

**Chose:** a file the driver writes per worker, `runs/<id>/workers/<session>.settings.json` (`worker::settings_path`), passed as `--settings <file>` right after `--effort`. It holds the three deny rules and the `PreToolUse` hook (`matcher: "Bash|Read"`, command `<current_exe> hook`, shell-quoted only when the path needs it, `timeout: 5`). `FACTORY_PACK=<absolute pack path>` goes in the worker env. Every role gets both.
**Over:** `--settings '<json>'` inline, as the slice's Current code and task 2 read.
**Because:** the run brief requires the settings to live in a file the driver owns under the run's directory. The file also leaves a record of what each worker ran under next to its stream, keeps argv short in `ps` and in `calls/N.json`, and `claude --help` 2.1.280 takes either form. Nothing is written under `~/.claude/` or a project's `.claude/`.
**Undo:** in `Worker::spawn`, pass `settings(&exe).to_string()` in place of the path and drop the `fs::write`. `check_worker_argv` in `tests/loop.rs` then reads the JSON from argv.
**Review:** decide — deviates from the slice's stated argv form

### AD-1302 — What is the shape of the new columns and report keys? · slice-13 · 2026-10-10

**Chose:**
- **Schema** (the PRD does not specify either column):
  - `workers.pack_files`: a JSON array of the pack header's files, set right after `insert_worker`.
  - `tool_calls.pack_doc INTEGER`: 1 on a `read`/`bash-cat` row whose target is held, 0 on the others.
- **Report text:**
  - `followup_reads: N of M ctx calls (P%)` and `pack_doc_read_chars: N` follow `repeat_rate` (`KEYS` 21 → 23).
  - `lookup_share_chars` (slice 08's byte share) joins `EXTRA_KEYS`.
  - The three status lines follow the keys.
  - `--per-worker` adds one line per worker: `worker <id> <role> slice NN (<sid8>): lookups N, ctx_share X.XXX, repeat_rate X.XXX`.
- **Report JSON:**
  - `followup_reads` is `{count, ctx_calls, share}`.
  - `thresholds` is `{followup, ctx_share, repeat_rate}`, each `ok|over`, `ok|under` or `lower|not lower`.
  - `per_worker` is an array, present only with `--per-worker`.
**Over:** recomputing the held list from the pack file at report time; a flat JSON shape.
**Because:** the pack file can be pruned with the run, while the row cannot. The flag is stored per row so the report needs no path matching. The text lines are the slice's own wording.
**Undo:** the two columns in `src/db.rs` (the schema and the add-if-missing list), and `Report`/`PerWorker`/`Thresholds` in `src/cli/report.rs`.
**Review:** decide — new schema columns and a public report and JSON surface that the PRD does not specify

### AD-1303 — Does the hook also deny `gh pr merge` and `git push --force`? · slice-13 · 2026-10-10

**Chose:** no. The deny rules ride in `permissions.deny` alone, and the hook answers an explicit `allow` for everything it does not refuse.
**Over:** the slice's fallback, where the hook denies those commands itself.
**Because:**
- The fallback applies only when slice 03 recorded that deny rules do not hold under `bypassPermissions`. Slice 03 recorded the opposite: Progress, "`--settings` deny holds under `bypassPermissions`", and AD-308's third Haiku probe denied `Bash(git status:*)` under bypass.
- Claude Code 2.1.280 applies a deny rule over a hook's `allow`, so the explicit `allow` lets nothing through that the rules refuse.

The checkpoint's real-worker probe confirms the hook half.
**Undo:** two match arms in `cli::hook::bash` returning `Verdict::Deny`.
**Review:** fyi — a fallback named in the task is deliberately absent

### AD-1304 — Which whole-file reads does the hook refuse? · slice-13 · 2026-10-10

**Chose:**
- Whole-file reads are refused only under a build worker's slice pack (pack file name `slice-*`). Fix, checkpoint and polish packs get the grep hint only.
- In a slice pack:
  - the slice file (header index 0) → `# Slice`;
  - every outlined file → `# Outline`;
  - `prd.md` always passes;
  - so do `agent-log.md` and a `log-*.md` named for an answered block.
- A `Read` with an `offset` or a `limit` passes.
- `cat <held>` is refused unless its first command pipes on (`cat f | head`).
**Over:** refusing every header file in every pack.
**Because:**
- A checkpoint pack lists each slice's file but carries only its Prove task.
- The block log is in the pack as one entry.
- The PRD is in the pack as cuts.

In all three cases, refusing would point the worker at text it does not have.
**Undo:** `Held::section` in `src/cli/hook.rs`.
**Review:** fyi — a worker must now `Read` its own slice by range before its first `Edit`. Watch for that at the checkpoint's real-worker run.

### AD-1305 — What counts as a grep "over the repo"? · slice-13 · 2026-10-10

**Chose:**
- **Lexical only, on the first simple command** (after `cd …`, `env`, `time` and `VAR=` prefixes):
  - `grep`/`egrep`/`fgrep` with `-r`/`-R`/`--recursive`/`--dereference-recursive` (not `-d recurse`), or any `rg` other than `--files`/`--type-list`;
  - with no path, or a path under the hook's `cwd` that reads as a directory: `.`, a trailing `/`, or a last segment with no extension.
- **No `stat`.**
- **These pass:**
  - a single file (`grep -n foo src/one.php`, `rg foo src/x.rs`);
  - a path outside `cwd`;
  - `~` or `$VAR` paths;
  - `git grep`;
  - a grep in a later pipeline stage (`cat f | grep foo`) or after `&&`.
**Over:** stat'ing each path, and scanning every command in the line.
**Because:** the hook must answer in well under 20 ms with no I/O beyond the pack's first line (task 1). It measured a 2.9 ms median and 4.1 ms max over 50 runs. The deny is a hint, not a guard.
**Undo:** `over_repo`/`repo_dir` in `src/cli/hook.rs`.
**Review:** fyi — `ls && grep -rn foo .` is not refused

### AD-1306 — Where do the pack's Outline paths and `def` symbols come from? · slice-13 · 2026-10-10

**Chose:** paths and symbols only from the slice's `## Current code / environment` section, as task 3 says:
- **Paths:**
  - a backticked repo-relative path with an extension, with an optional `:N` or `:N-M`;
  - no `/`, `~`, `$`, `..` or empty segment;
  - existing, in a language `ctx outline` knows.
- **Symbols:** a backticked `name()` or `A::b`, last segment.
- **`def`:**
  - runs in-process for each symbol;
  - a symbol with no hit is skipped;
  - it stops adding once 6,000 chars are written.
- **Outline:** covers the cited files plus every file a `def` hit landed in (depth 2).
- **Caps:**
  - each section is cut at its last whole line within the cap (8,000 and 6,000 chars);
  - a trailer follows the cut: `[cut at N chars: run \`factory ctx outline <path>\` for the rest]`;
  - the header's `files:` gains only the outlined files whose `== path ==` header survived the cut.
- **No match:** the section says so in an HTML comment.
**Over:** outlining only the cited paths; reading symbols from the whole slice, as the slice's Current code bullet reads.
**Because:**
- Task 3's acceptance wants `src/helper.rs` outlined and in `files:` when only `helper()` names it.
- Task text from the whole slice cites symbols the worker is meant to create.
- A cut mid-line would hand the hook a file the pack does not hold.
**Undo:** `Recon::of` in `src/pack.rs`.
**Review:** fyi — the outlined set is larger than the cited paths

### AD-1307 — What do `followup_reads` and `pack_doc_read_chars` count? · slice-13 · 2026-10-10

**Chose:**
- `followup_reads` prints a count and a share of `ctx` calls (`1 of 2 ctx calls (50%)`), with no chars.
- `pack_doc_read_chars` sums `result_chars` over every `read`/`bash-cat` of a held file: ranged reads and `prd.md` included.
- The `ctx-*` kinds are lookups, so `lookup_chars` and `repeat_rate` now include them.
**Over:**
- §10's "count and chars" for `followup_reads`;
- a `pack_doc_read_chars` that counts only the reads the hook refuses.
**Because:**
- The slice's task 5 gives the line format, and the 30% rule reads a share of calls.
- Task 4 defines `pack_doc` by the held list, not by the hook's refusal.
- `ctx_share` is ctx rows among lookup rows (§14 decision 4), which needs them counted as lookups.
**Undo:** `Followups` and the `pack_doc` sum in `src/cli/report.rs`; `LOOKUP_KINDS` in `src/metrics.rs`.
**Review:** fyi — the report's `followup_reads` follows the slice, not §10's wording

### AD-1308 — When is a call "expanded"? · slice-13 · 2026-10-10

**Chose:**
- `label_expanded` is set on `read` and `ctx-refs` rows. Task 4's acceptance puts it on a `read`, while §10 names `ctx-*` rows only.
- A `read` is expanded when a later `read` of the same file within five calls has a larger `limit` or none. The first read must have had a `limit`.
- A `ctx-refs` is expanded when a later one has the same word and `--in` and a larger `--max`, where the default is 10.
- Rows of other kinds stay null.
**Over:** setting the label on whole-file reads.
**Because:** a whole read cannot be widened.
**Undo:** `metrics::labels`.
**Review:** no

### AD-1309 — How much of a `ctx` answer is kept, and from which binary does the hook run? · slice-13 · 2026-10-10

**Chose:**
- A `ctx` call's `tool_result` head is kept up to 65,536 chars (`stream::CTX_RESULT_HEAD`), so the follow-up detector sees every named path. Other calls keep slice 08's 4,096 chars.
- The hook command is `std::env::current_exe()`, as task 2 says, which is the test binary in in-process tests.
- `ctx_kind` reads the first command through `shell::first_command`, so `cd x && factory ctx refs y` counts. The other kinds keep slice 08's whitespace reading.
**Over:** one head size for all calls.
**Because:** a `refs` answer can run to several thousand chars and the detector needs its paths.
**Undo:** `CTX_RESULT_HEAD` in `src/stream.rs`.
**Review:** no
