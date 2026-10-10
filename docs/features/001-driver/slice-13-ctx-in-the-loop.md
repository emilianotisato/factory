---
slice: 13
status: pending
depends_on: [10, 12]
---

# Slice 13 — ctx in the loop: pack sections, hook hints, follow-up labels (FR-12 slice 3, FR-2 pack-doc hint, FR-8 tail)

**Source**: [`prd.md`](prd.md) → §14 FR-12 decisions 3 and 4 (the pack's Outline and Lookup sections, the hook hints, the follow-up labels and report keys, the reversal rule) and decision 5 ("(3) pack sections, the two hook hints (lookup verb, pack doc), `tool_calls` table with the follow-up labels, and report keys"); §4 FR-2 permissions bullet as corrected 2026-10-09 (a new PreToolUse hook `factory hook` injected through `--settings`, carrying the two deny rules); §10 FR-8 (`followup_reads`, `pack_doc_read_chars`). **Depends on**: 10 (the whole driver loop and the report) and 12 (`outline` and `def` with bodies). The join of the spine and the branch.
**Created**: 2026-10-09 · **Status**: pending

**Progress**: _(written as it lands)_

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
