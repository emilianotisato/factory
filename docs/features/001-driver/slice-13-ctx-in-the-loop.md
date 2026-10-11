---
slice: 13
status: done
lane: 1
depends_on: [10, 12]
---

# Slice 13 — ctx in the loop: pack sections, hook hints, follow-up labels (FR-12 slice 3, FR-2 pack-doc hint, FR-8 tail)

**Source**: [`prd.md`](prd.md) → §14 FR-12 decisions 3 and 4 (the pack's Outline and Lookup sections, the hook hints, the follow-up labels and report keys, the reversal rule) and decision 5 ("(3) pack sections, the two hook hints (lookup verb, pack doc), `tool_calls` table with the follow-up labels, and report keys"); §4 FR-2 permissions bullet as corrected 2026-10-09 (a new PreToolUse hook `factory hook` injected through `--settings`, carrying the two deny rules); §10 FR-8 (`followup_reads`, `pack_doc_read_chars`). **Depends on**: 10 (the whole driver loop and the report) and 12 (`outline` and `def` with bodies). The join of the spine and the branch.
**Created**: 2026-10-09 · **Status**: pending

**Progress** (2026-10-10, lane 1):
- **Built.**
  - **`factory hook`** (`src/cli/hook.rs`, on a new `src/shell.rs` tokenizer):
    - It reads the PreToolUse JSON and the first line of `$FACTORY_PACK`, decides, prints `hookSpecificOutput` and exits 0.
    - Malformed input is allowed, with a line on stderr.
    - It denies:
      - a recursive `grep`/`egrep`/`fgrep`, or any `rg`, over no path or over a directory under `cwd`, with `use: factory ctx refs <word> (or ctx def <symbol>); grep only for literal strings and log output` (AD-1305);
      - in a slice pack only, a whole `Read` or an unpiped `cat` of a held file, with `that file is in your pack under "# Slice"` or `"# Outline"` (AD-1304). `prd.md` and block logs pass.
    - There is no fallback deny for merge and force-push (AD-1303).
    - The `Hook` subcommand lost its `Passthrough`, and `not_built` is gone from `main.rs`.
  - **Settings injection** (`src/worker.rs`): every role writes `runs/<id>/workers/<session>.settings.json` (AD-1301), holding:
    - the deny rules `Bash(gh pr merge*)`, `Bash(git push --force*)` and `Bash(git push -f*)`;
    - the PreToolUse hook: `matcher: "Bash|Read"`, `<current_exe> hook`, `timeout: 5`.

    `spawn` passes `--settings <file>`, sets `FACTORY_PACK=<absolute pack>`, and stores the pack's held files in `workers.pack_files`.
  - **The pack** (`src/pack.rs`):
    - `Recon::of` reads the paths and symbols the `## Current code / environment` section cites.
    - It runs `ctx::def` and `ctx::outline` in-process, outlining the cited files and the `def` hit files (AD-1306).
    - The `# Outline` section is capped at 8,000 chars for outlines and 6,000 for defs, cut at a line with a trailer naming the verb.
    - The `# Lookup` section comes from the new `assets/lookup.md`.
    - Both sections sit between `# PRD sections` and `# Contract`, and the header's `files:` gains the outlined files.
  - **`tool_calls`** (`src/metrics.rs`, `src/stream.rs`):
    - New kinds: `ctx-def`, `ctx-refs` and `ctx-outline`.
    - `label_followup` is set on `ctx-*` rows: a `read`, `bash-cat` or `bash-sed` of a path the answer named, within 2 calls.
    - `label_expanded` is set on `read` and `ctx-refs` rows: a wider `limit` or a larger `--max` within 5 calls (AD-1308).
    - New column `pack_doc` (AD-1302).
    - A ctx answer's result head is kept to 65,536 chars (AD-1309).
  - **The report** (`src/cli/report.rs`):
    - `ctx_share` is now ctx rows among lookup rows; slice 08's byte share prints as `lookup_share_chars`.
    - New keys `followup_reads: N of M ctx calls (P%)` and `pack_doc_read_chars` (AD-1307).
    - Three status lines, with the §14 constants 30%, 10% and 26.7% cited in a comment.
    - `--per-worker`.
    - `--json` carries all of them (AD-1302).
- **Tests.**
  - `tests/cli.rs::hook_denies_repo_greps_and_whole_reads_of_held_files` covers task 1's acceptance list through the binary.
  - `tests/loop.rs`:
    - `check_worker_argv` now asserts on each slice worker that:
      - the stub received `--settings`;
      - the path is under `runs/<id>/workers/`;
      - the file parses with both task 2 deny rules and a single `Bash|Read` PreToolUse command ending in `factory hook`, with `timeout: 5`;
      - `FACTORY_PACK` is the pack.
    - New `pack_outlines_the_cited_code_and_carries_the_lookup_verbs`:
      - the planted `src/helper.rs` outline and `def helper` body;
      - `# Lookup` with the three verbs and the rule;
      - the header gains `src/helper.rs`;
      - output is byte-identical across two spawns;
      - a 60-file recon is cut at 8,000 chars with its trailer, and the held list matches the kept headers.
  - `tests/metrics.rs`:
    - New `ctx_calls_are_labelled_and_the_report_reads_the_reversal_rule`. The stub's new `ctx: true` option streams a `ctx refs`, a follow-up `Read` and a `ctx outline`. It checks:
      - the report shows `followup_reads: 1 of 2 ctx calls (50%)`, `followup threshold 30%: over` and `ctx_share` 0.200;
      - `pack_doc_read_chars` is the sum of the held reads;
      - `--per-worker` text and JSON;
      - the labels row by row.
    - The first test also checks `pack_doc` and `pack_files`, and `--settings` on every role.
  - Unit tests:
    - `shell.rs`: one;
    - `hook.rs`: three;
    - `metrics.rs`: four (kinds, followup, expansion, pack_doc);
    - `pack.rs`: two (citations, cap);
    - `report.rs`: one (the threshold lines).
- **Inner loop:** 3 runs of `cargo test`, 1 red, about 2 min (40 s, 36 s, 38 s).
  - Run 1 was red. `tests/closeout.rs::the_driver_never_merges_or_readies_a_pr` scans `src/` for `pr merge`, and it caught the new deny-rule string in `worker.rs` (Traps). Everything else passed on its first run.
  - Run 2 was green.
  - Run 3 was green after `cargo fmt`: 155 tests, 0 ignored.
  - `cargo fmt` reflowed `tests/loop.rs`, `tests/metrics.rs`, `src/metrics.rs` and `src/pack.rs`.
  - `cargo clippy --all-targets -- -D warnings` caught nothing.
- **Hook timing and a hand transcript** (debug build, this repo's checkout as `cwd`, a slice pack):
  - Timing over 50 runs: median 2.9 ms, max 4.1 ms. The target is under 20 ms.
  - `grep -rn foo src` → `deny` with the `factory ctx refs` hint, exit 0.
  - a whole `Read` of a held `src/pack.rs` → `deny` naming `# Outline`.
  - `cat src/x.php | grep foo` → `allow`.
- **Earlier-slice expectation shifts:**
  - `tests/cli.rs`: removed `every_subcommand_says_not_built_yet_and_exits_2`, because `hook` was the last unbuilt subcommand.
  - `tests/loop.rs`: the `check_worker_argv` length is 15 → 17 (`--settings <file>`).
  - `tests/closeout.rs`: the no-merge source scan now strips the deny-rule literal first.
  - `tests/metrics.rs`:
    - `KEYS` 21 → 23 (`followup_reads` and `pack_doc_read_chars` after `repeat_rate`);
    - `EXTRA_KEYS` gains `lookup_share_chars`;
    - the byte-share assertion moved from `ctx_share` to `lookup_share_chars`;
    - `ctx_share` is now 0.000 on a run with no `ctx` calls;
    - the threshold lines follow the keys.
  - `src/cli/report.rs`'s keys unit test was updated the same way.
- **Deviations:**
  - `decide`:
    - AD-1301: `--settings` takes a driver-owned file, not inline JSON.
    - AD-1302: the new columns, the report JSON shape and `--per-worker`.
  - `fyi`:
    - AD-1303: no fallback deny.
    - AD-1304: whole-read denies in slice packs only.
    - AD-1305: a lexical "over the repo".
    - AD-1306: `def` hit files are outlined too.
    - AD-1307: `followup_reads` is a count, not count and chars.
- **Prove (task 7), CP-7 (2026-10-10):**
  - **FR-12 tail and FR-8 tail: `factory report --per-worker` on the fixture run.**
    - The fixture was kept with `FACTORY_KEEP_FIXTURE=1 cargo test --test metrics ctx_calls_are_labelled_and_the_report_reads_the_reversal_rule`: three builds, one checkpoint and a polish. Slice 01's stub stream makes two `factory ctx` calls and one follow-up `Read`.
    - `pack_doc_read_chars` 14124 is the sum of the seven `pack_doc = 1` rows.
    - The report as printed:
      ```
      lookup_chars: 14273
      ctx_share: 0.200
      repeat_rate: 0.000
      followup_reads: 1 of 2 ctx calls (50%)
      pack_doc_read_chars: 14124
      worker_gate_runs: 4
      worker_gate_red: 0
      cockpit_peak_context: 0 (no --cockpit)
      lookup_share_chars: 0.923
      followup threshold 30%: over
      ctx_share threshold 10%: ok
      repeat_rate vs 26.7%: lower
      worker 1 build slice 01 (03533997): lookups 5, ctx_share 0.400, repeat_rate 0.000
      worker 2 build slice 02 (e9d98042): lookups 2, ctx_share 0.000, repeat_rate 0.000
      worker 3 build slice 03 (c2714c6b): lookups 2, ctx_share 0.000, repeat_rate 0.000
      worker 4 checkpoint (90b2ca84): lookups 0, ctx_share 0.000, repeat_rate 0.000
      worker 5 polish (29047a3d): lookups 1, ctx_share 0.000, repeat_rate 0.000
      ```
      The 19 keys before `lookup_chars` are omitted here; the full text is in the evidence file.
    - The evidence file also holds the full text, `--json` (`followup_reads {count, ctx_calls, share}`, `thresholds`, `per_worker`), the `tool_calls` rows with their labels, `workers.pack_files`, the settings file the driver wrote, and the stub's `--settings` and `FACTORY_PACK`.
    - Evidence: [`evidence/cp-7-fr12-fr8-report-per-worker.txt`](evidence/cp-7-fr12-fr8-report-per-worker.txt).
  - **Task 1 hook transcript.**
    - `factory hook` was fed Claude Code's PreToolUse JSON, with `FACTORY_PACK` set to the kept run's slice-01 pack and `cwd` set to the fixture repo.
    - Every exit was 0, at about 5 ms each with process start.
    - Denied with `use: factory ctx refs <word> (or ctx def <symbol>); grep only for literal strings and log output`:
      - `grep -rn foo src`;
      - `grep -rn foo .`;
      - `rg foo`;
      - `cd src && LC_ALL=C grep -R foo .`.
    - Denied with `that file is in your pack under "# Slice"; read a range if you need more`:
      - a whole `Read` of the held `slice-01-a.md`;
      - `cat` of it.
    - Allowed:
      - the same `Read` with `offset: 10, limit: 40`;
      - a whole `Read` of `prd.md`;
      - `grep -n foo src/lib.txt`;
      - `cat src/lib.txt | grep foo`;
      - `cargo test`;
      - malformed stdin, with `factory hook: cannot parse stdin (…); allowed` on stderr.
    - Evidence: [`evidence/cp-7-fr2-fr12-hook-transcript.txt`](evidence/cp-7-fr2-fr12-hook-transcript.txt).
  - **The real one-turn worker.**
    - It ran once with the slice's own prompt, `claude -p "run: grep -rn foo ."`, on Claude Code 2.1.280 with `--model haiku` (`claude-haiku-4-5-20251001`).
    - Its flags were the driver's: `--output-format stream-json --verbose --permission-mode bypassPermissions --session-id <uuid> --settings <the driver-written settings file>`, plus `--max-budget-usd 0.25`. The 2.1.280 `--help` lists no `--max-turns`, so that flag was not passed.
    - The env matched the driver's: `CLAUDECODE` and `CLAUDE_*` removed, and `FACTORY_PACK` set.
    - It ran in a `mktemp -d` copy of `tests/fixture/` with a fresh `git init`. That directory was deleted afterwards.
    - The result: 4 turns, $0.0621.
    - **`permission_denials` holds the grep** (`toolu_01WT2ccPSw38sozFUfj7tEzd`, `grep -rn foo .`) and the model's retry, `grep -rn "foo" . 2>/dev/null | head -20`. Both tool results carry the hook's reason, verbatim and prefixed `PreToolUse:Bash hook error:`.
    - No PreToolUse `hook_response` line is streamed; 2.1.280 streams those for SessionStart only.
    - The model followed the hint: `factory ctx refs foo` exited 127 with `factory: command not found`.
      - `factory` is not installed on this machine. The hook ran because the settings name it by absolute path (AD-1309).
      - qa-guide.md requires `factory` to be installed for real runs.
      - It is recorded, not fixed.
    - Stream: [`recordings/claude-p-hook-grep-deny-haiku-2.1.280.jsonl`](recordings/claude-p-hook-grep-deny-haiku-2.1.280.jsonl).
    - Evidence: [`evidence/cp-7-fr2-fr12-real-worker.txt`](evidence/cp-7-fr2-fr12-real-worker.txt).
  - **Unreached:**
    - **AD-1304's ranged read of the slice before an `Edit`.** This real worker's fixed prompt makes no `Edit`, and a second real run with another prompt was out of bounds. It is now a check in [`qa-guide.md`](qa-guide.md), FR-11 step 6.
    - **The A/B figures** (`ctx_share` and `repeat_rate` on real runs). They are the owner's rows.
  - Gate: [`evidence/cp-7-gate.txt`](evidence/cp-7-gate.txt).

**Code-review fixes (2026-10-10).**
- The worker deny rules were three prefixes (`gh pr merge*`, `git push --force*`, `git push -f*`) and the hook passed everything else, so a worker holding the owner's `gh` login could still run `gh pr ready`, `gh api …/merge`, `true && gh pr merge`, `git -c … push` or `git push --no-verify --force`, against the never-list and §4's "guarded on any project" (S1, S-n1, Xb-1, Xa-shell). Any `gh` and any `git push` is now denied by rule and by the hook's guard, which reads every simple command through lists, substitutions, nested shells and wrappers and denies when it is unsure (input that does not parse included, where this Progress says malformed input is allowed), and workers get no GitHub token and an empty `GH_CONFIG_DIR` (AD-10001, `decide`; real check in `evidence/polish-f1-deny-real.txt`).
- The hook answered `allow` for every call it did not deny, which could override the project's own settings deny rules (Ln7). It now returns no decision, and the real check showed a project deny rule still applies.
- The `# Lookup` verbs, the grep hint and the pack's cap notes named a bare `factory`, so CP-7's real worker on an uninstalled build got exit 127 (K1). They now name `paths::factory_command()`, the running binary's absolute path (AD-10002).
- `Held::section` matched a held path by its suffix, so another file with the same tail, such as a `registry/src/lib.rs`, was refused as held (C3). It now matches `<repo>/<held path>` exactly, with the repo from `FACTORY_REPO`, which the driver sets for every worker (AD-10003).
- AD-1305's lexical rule took a trailing `/` or an extensionless last segment for a directory, so `grep -rn x Makefile` was denied, `grep -rn x src.d` passed, and `cd X && grep` was judged from the original cwd (Cn3, Cn4). **AD-10005 replaces that text-only grep-operand rule**: an operand is a directory when `Path::is_dir` says so under the repo or it is a glob ending `*` or `**`, and each leading `cd X &&` moves the directory later operands resolve against.
- `pack_doc_read_chars` matched held docs by suffix, so another file with the same tail inflated it (C5). It now matches the held path at the repo root.
- With no lookups, or no ctx call, the threshold lines still said `ok` or `under` over 0 of 0, which read as a pass for the reversal rule (Cn7). They now say `n/a`, a string in the JSON too (AD-40007).

**Slice**: After this slice the worker meets `factory ctx` three ways: the pack opens with an Outline of every recon file and the `def` of every cited symbol, plus a Lookup section with the three verbs; a PreToolUse hook injected through `--settings` denies `grep -r` and `rg` over the repo with a one-line hint naming the verb, and denies whole-file reads of files the pack already holds with a hint naming the pack section; and the same settings carry the two permission deny rules so a worker is guarded on any project. The `tool_calls` table gains the `ctx-*` kinds and the two follow-up labels, and `factory report` prints `followup_reads` and `pack_doc_read_chars` with the three threshold lines the reversal rule reads. The A/B numbers themselves are the owner's (qa-guide.md).

## Current code / environment

- **The correction** (§4, 2026-10-09): no Claude Code hook exists today; the deny rules `gh pr merge` and `git push --force*` live in the owner's user settings (`~/.claude/settings.json:51`), and the "global hooks" are git hooks. Neither dogfood project has a `.claude/settings.json`. So the driver injects per worker: `--settings '<json>'` (Claude Code 2.1.280 `--help` line 224: a file or inline JSON) with `{"permissions":{"deny":["Bash(gh pr merge*)","Bash(git push --force*)","Bash(git push -f*)"]},"hooks":{"PreToolUse":[{"matcher":"Bash|Read","hooks":[{"type":"command","command":"factory hook","timeout":5}]}]}}`. Slice 03 task 1 recorded whether deny rules hold under `bypassPermissions`; read that answer at entry. If they do not, the hook itself returns `deny` for those two commands as well, which is the fallback §20 asks for.
- **The hook protocol**: a PreToolUse command hook receives JSON on stdin (`session_id, tool_name, tool_input, cwd, …`) and answers on stdout with `{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"allow|deny|ask","permissionDecisionReason":"…"}}` (or exit 2 with the reason on stderr = deny); `deny` beats `ask` beats `allow` when several hooks answer. The reason text is what the model reads, so the hint goes there. Must answer in well under the timeout: no `docs.mjs`, no SQLite; the held-file list comes from the pack header (`<!-- factory-pack files: … -->`, slice 03 task 2) read through an env var the driver sets, `FACTORY_PACK=<path>`.
- **What the hook denies** (§14 decision 3 and §4): `Bash` whose command (after `cd … &&` and env prefixes) starts with `grep` with `-r`/`-R`, or `rg`, over a repo path (the cwd, `.`, `src`, `app`, a directory) → deny, reason `use: factory ctx refs <word> (or ctx def <symbol>); grep only for literal strings and log output`. A grep over a single file or a pipe (`cmd | grep`) passes. `Read` of a file the pack holds with no `offset`/`limit`, or `Bash cat <held file>` → deny, reason `that file is in your pack under "# <section>"; read a range if you need more`. A partial `Read` passes. `prd.md` always passes (§4: the PRD sections in the pack are cuts, the file is longer).
- **Pack additions** (§14 decision 3): `# Outline`: `factory ctx outline <path>` for each path the slice's `Current code / environment` cites (parse `file:line` and bare paths from that section; distinct files only; cap the section at 8k chars with a trailer) and `factory ctx def <symbol>` for each backticked identifier of the form `name()` or `Type::name` in the slice (cap 6k chars); `# Lookup`: the three verbs, three one-line examples, the one rule. Both go after `# PRD sections` and before `# Contract`. The header's `files:` list gains the outlined files so the hook knows them.
- **`tool_calls` additions** (§14 decision 4, §10): kinds `ctx-def`, `ctx-refs`, `ctx-outline` for `Bash` commands starting with `factory ctx <verb>`; labels `label_followup` (`followup_read_within_2`: a `read`/`bash-cat` of a file named in the ctx output within the next two tool calls of the same worker) and `label_expanded` (`expanded_within_5`: a `read` with a larger `limit` or a `refs` with a larger `--max` on the same target within five). Both columns exist since slice 08 (created null).
- **Report additions** (§14 decision 4): `followup_reads: N of M ctx calls (P%)` and `pack_doc_read_chars` (sum of `result_chars` of reads on held files, the hint's own metric from the Jev-free graft) plus three status lines after the keys: `followup threshold 30%: ok|over`, `ctx_share threshold 10%: ok|under`, `repeat_rate vs 26.7%: lower|not lower`. The reversal rule is the owner's decision after the A/B; the report only prints the comparison.
- **The stub** needs `settings` recorded: `tests/stubs/claude` already records argv; a test parses the `--settings` JSON out of `calls/N.json`. The hook is tested directly by piping PreToolUse JSON into `factory hook` with `FACTORY_PACK` set.
- **Deliberate non-changes**: *(prd §14 "Accepted cost: uptake is unproven … FR-8 measures the uptake; the reversal rule is next"; no forced verb, the hint is a deny with a reason, nothing rewrites the command.)* *(prd §19 "Not handled: magic dispatch" of lookups; the model chooses.)* *(prd §19 "Not handled: a sha256 seen-before stub"; cut 2026-10-09, the pack-doc hint replaces it.)* *(prd §4: the worker runs with `bypassPermissions`; the two deny rules are the only permission rules injected.)*

## Tasks

### 1. `factory hook`: the PreToolUse command — high — blocked by: —
**Source**: §14 FR-12: "The guard hook (FR-2) also answers `grep -r` and `rg` over the repo with a one-line hint naming the verb." §4 FR-2 (pack-doc hint, kept 2026-10-09): a whole-file read of a file the pack holds is answered with the section name. §4 correction: "a new PreToolUse hook, `factory hook`, that the driver injects per worker through `--settings <json>`".

`src/cli/hook.rs`: read stdin JSON, decide as the Current code entry describes, write the `hookSpecificOutput` JSON, exit 0; any parse failure → allow (never block a worker on a hook bug) and a line to stderr. Held files from `FACTORY_PACK`'s header line; sections mapped file → section name (`# Slice`, `# Outline`, `# PRD sections` for `prd.md` cuts, which are never denied). Under 20 ms, no I/O beyond reading the pack's first line. Fallback deny of `gh pr merge` and `git push --force|-f` when slice 03 recorded that `permissions.deny` does not hold under bypass.

**Acceptance:** piping `{"tool_name":"Bash","tool_input":{"command":"grep -rn foo src"}}` → `deny` with a reason containing `factory ctx refs`; `grep -n foo src/one.php` → `allow`; `rg foo` → deny; `cat src/x.php | grep foo` → allow; `{"tool_name":"Read","tool_input":{"file_path":"<held slice file>"}}` → deny naming `# Slice`; the same with `"offset":10,"limit":40` → allow; `Read` of `prd.md` → allow; malformed stdin → allow and exit 0.

### 2. Inject settings per worker — high — blocked by: 1
**Source**: §4 FR-2 (corrected 2026-10-09): "the same settings carry the two deny rules, so a worker is guarded on any project"; "`--settings <json>` (in the 2.1.280 `--help`)".

`worker.rs`: add `--settings <json>` with the deny list and the PreToolUse hook (`matcher: "Bash|Read"`, command `factory hook`, the absolute path of the running binary from `std::env::current_exe()` so an uninstalled build works in tests), and `FACTORY_PACK=<pack path>` in the worker env. Every role gets it (build, fix, checkpoint, polish).

**Acceptance:** the stub's `calls/1.json` argv holds `--settings` whose JSON parses, lists both deny rules and a `PreToolUse` entry whose command ends in `factory hook`; the env has `FACTORY_PACK` pointing at the pack file.

### 3. Pack: Outline and Lookup sections — high — blocked by: —
**Source**: §14 FR-12: "The pack (FR-2) gains an 'Outline' section, the `outline` of each recon file and the `def` of each cited symbol … and a 'Lookup' section: three verbs, three examples, one rule: grep only for literal strings and log output."

`pack.rs`: collect paths and symbols from the slice's `## Current code / environment` section (regex for `` `path/with.ext:123` `` and `` `path/with.ext` ``, and `` `name()` ``/`` `Type::name` ``), run `ctx::outline` and `ctx::def` in-process (library calls, not a subprocess) with the caps above, write `# Outline` and `# Lookup` (`assets/lookup.md`), add outlined files to the header list. A file that does not exist (a planned file) is skipped silently; a `def` with no hit is skipped.

**Acceptance:** on a fixture slice whose Current code cites `src/lib.txt:1` and `` `helper()` `` with a planted `src/helper.rs`: the pack has `# Outline` with the `helper.rs` outline, the `def helper` body, `# Lookup` with the three verbs, and the header `files:` names `src/helper.rs`; the section is cut at 8k chars with a trailer on a fixture with a large recon list.

### 4. `tool_calls`: ctx kinds and follow-up labels — high — blocked by: —
**Source**: §14 FR-12: "for each `ctx` call whether the worker read a named file within the next two tool calls (`followup_read_within_2`) or widened the window within five (`expanded_within_5`)". §10 FR-8: `followup_reads`, `pack_doc_read_chars`.

`metrics.rs`: classify `Bash factory ctx def|refs|outline …` as `ctx-def|ctx-refs|ctx-outline`; after classifying a worker's calls, a second pass sets `label_followup` on each ctx row when any of the next two calls is a `read`/`bash-cat`/`bash-sed` whose target path appears in the ctx call's `tool_result` text, and `label_expanded` when within five calls the same target is read with a wider `limit` or searched with a larger `--max`. `pack_doc_read_chars` = `result_chars` of `read`/`bash-cat` rows whose target is in the pack's held list (stored per worker in the `workers` row as JSON at spawn).

**Acceptance:** a synthetic stream `ctx-refs → Read <file from output> → …` sets `label_followup = 1` on the ctx row; `ctx-refs → Read other → Read other → Read <file from output>` leaves it 0; `Read f limit 40 → … → Read f limit 200` within five sets `label_expanded` on the first; a `Read` of a held file adds its chars to `pack_doc_read_chars`.

### 5. Report: the two keys and the three threshold lines — medium — blocked by: 4
**Source**: §14 FR-12: "`factory report` prints `lookup_chars`, `ctx_share`, `repeat_rate` and `followup_reads`. `followup_reads` above 30% of `ctx` calls after slice 3 means the answers are too thin … If, after slice 3, `ctx_share` stays under 10% of worker lookups or `repeat_rate` does not fall from 26.7%, `ctx` shrinks to `outline` only". §10: `pack_doc_read_chars`. Acceptance: "After the first A/B run, `factory report` prints `ctx_share` and `repeat_rate` for every worker."

`report.rs`: append `followup_reads` and `pack_doc_read_chars` to the run keys; `--per-worker` prints `ctx_share` and `repeat_rate` per worker row; after the keys, the three status lines with the constants `30%`, `10%`, `26.7%` and their §14 citation in a comment. `ctx_share` here counts `ctx-*` rows among lookup rows (the decision 4 meaning); slice 08's byte-share stays as `lookup_share_chars` so both readings print.

**Acceptance:** on a fixture run whose stub stream includes two `factory ctx` calls and one follow-up read, the report prints `followup_reads: 1 of 2 ctx calls (50%)`, `followup threshold 30%: over`, `ctx_share` per worker with `--per-worker`, and `pack_doc_read_chars` as a number; `--json` includes the new keys.

### 6. Verify — medium — blocked by: 1–5
`cargo test`. The full gate runs at this slice's checkpoint.

**Acceptance:** inner loop green, one commit.

### 7. Prove — at the checkpoint — blocked by: 6
Rows from [`prd.md`](prd.md) § QA plan: the last part of **FR-12** ("after the A/B run `ctx_share` and `repeat_rate` print per worker", evidence: `factory report`) and the tail of **FR-8** (`followup_reads`, `pack_doc_read_chars` in the report). At the checkpoint: `factory report --per-worker` on the fixture run and the task 1 hook transcript, pasted into this slice's **Progress**; the hook's deny under `bypassPermissions` on a real one-turn worker (`claude -p "run: grep -rn foo ."` with the injected settings) is run once here, Haiku-sized, and its stream kept under `recordings/`. The A/B figures are the owner's rows in [`qa-guide.md`](qa-guide.md).

**Acceptance:** the report and hook transcripts are attached; the real-worker recording shows a `permission_denials` entry or a `hook_response` deny for the grep.
