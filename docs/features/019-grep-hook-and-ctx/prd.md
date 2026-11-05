---
number: 019
slug: grep-hook-and-ctx
title: "The grep hook and ctx answer what they're asked"
# type: feature | bugfix | enhancement | refactor
type: feature
created: 2026-11-05
author: Emiliano Tisato
summary: |
  The PreToolUse hook denies a grep only when `factory ctx` can answer it, caps what its denial carries, catches every spelling of a process call, and replays a corpus of what main denied before any loosening ships; ctx indexes Go consts, Pest tests and extensionless scripts. Draft from the 2026-11-05 idea triage, not interviewed.
---

# PRD-019 — The grep hook and ctx answer what they're asked

> **Draft from the 2026-11-05 idea triage.** The owner chose what is in and out, group by group; nothing here is interviewed or verified yet. `/prd` continues this draft: interview, recon, FRs, QA plan. `file:line` facts are from `main` at `ca128f7`; re-verify them.

## 1. Why now

Each wrong denial costs a worker a round trip; the latest report is from 2026-11-05 (piped greps, `--include`, string literals, docs paths). `def` output has no cap (`src/ctx/print.rs:1`). A loosening FR without a corpus can re-allow what main denied (harvest 2026-10-20).

## 2. Candidate FRs

Kept by the owner at the triage. `/prd` turns each into an FR or cuts it.

- **C-1 — Deny only what ctx can answer.** `bare_name` checks only `VENDOR_DIRS` (`src/cli/hook.rs:670-683`) while `walk()` also skips `ROOT_DIRS`; `--include` is ignored (`:723`); the first command is judged regardless of a pipe (`:131-138`); string-literal identifiers; the hint (`:20`) gives string-search advice.
- **C-2 — Cap ctx output:** line width and `def` location count (`src/ctx/print.rs`; `refs` is capped at 10, `src/ctx/refs.rs:18`).
- **C-3 — Index gaps:** Go `const_spec`, Pest `test()`/`it()`, extensionless scripts (`src/ctx/lang.rs:22-52`, `src/ctx/walk.rs:25-31`); PHP class constants unverified.
- **C-4 — Widen the process-call list** (17 `CALLS`, the dot-skip, `src/cli/hook.rs:591-617`).
- **C-5 — A hook regression corpus**: every command main denies, replayed by a test (fixed lists today, `tests/hook.rs:45-57`), and a `/prd` acceptance line for any guard-loosening FR.

## 3. Not in scope (decided 2026-11-05)

- **Per-question-type ctx metric** (item 6 of the fix-the-ctx-hook idea) — dropped; items 1-4 shipped in PRD-009.
- **Jev picker, LSP/SCIP lookup layer** — dropped: measured and rejected, reasons in `docs/reference/ctx-research.md` and `ctx-jev-research.md`.
- **Batched `def` lookups in the pack's recon** — dropped: no measured need.

## 4. Source ideas

The full text of each idea this draft absorbed; the idea files are deleted.

### Deny a bare-name grep only where ctx can search

_added 2026-10-15, origin: PRD-009 polish_

After PRD-009, the hook denies a recursive grep for a bare identifier over any repo dir outside `VENDOR_DIRS` (`bare_name`, `vendored` in `src/cli/hook.rs`). ctx searches less than that: `walk()` (`src/ctx/walk.rs`) also skips `ROOT_DIRS` (storage, dist, build at the root), gitignored paths, and every file outside its five `Lang` types (`.md`, `.toml`, `.yaml`, `.py`, `.kt` and logs are not searched).

So `grep -rn checkpoint_every docs` or `grep -rn ERROR storage/logs` is denied, ctx finds nothing, and the reason is the hint alone ("a bare name goes to ctx, grep anything else"), which sends the worker to ctx for a word only grep can find. The worker gets out with `grep -rn 'checkpoint_every\b'` or a non-recursive grep, a turn later. On main every recursive grep over the repo was denied, so this is no regression.

FR-5 settled "no hit from either command leaves the hint alone". Options:
1. Deny only where ctx can search: share walk's skip rule and its `Lang` filter with the hook (a `docs/` path or `--include=*.md` passes).
2. Let a denial with no ctx hit become an allow, since nothing better was offered.

Recommended: 1, it keeps the denial a pointer to an answer. Count in the store's runs how many denials after PRD-009 carried the hint alone.

### The grep → `factory ctx refs` hook refuses greps ctx cannot answer

_added 2026-11-05, origin: factory upstream_

The PreToolUse hook that redirects a bare-word `grep` to `factory ctx refs` costs a round trip in two cases where ctx is the wrong tool:

- **Identifiers that exist only inside string literals.** An XML element name used as `"DirRecep"` in code: `ctx refs` found nothing, and the hook's message did not say a quoted grep was the way.
- **Filtered pipelines.** `grep -n "Name" … | grep -v _test` was refused. The ctx answer was useful, but the refusal cost a turn.

Should: let a grep through when it is piped or path-filtered, or when the pattern is quoted with surrounding quotes; and have the refusal message say "string literals: grep with the quotes".

### Cap ctx output line width and def location count

_added 2026-10-15, origin: PRD-009 polish_

A grep denial's reason carries `ctx refs` and `ctx def --no-body` output (PRD-009 FR-5, `answered()` in `src/cli/hook.rs`). `refs` is capped at 10 hits; `def` prints every definition it finds, and neither caps a line's width (files up to 2 MB are scanned). The polish security pass showed a minified one-line `.js` file can put a megabyte line in each ref, and a name defined hundreds of times prints every location. Run from `/` it measured a 1.15 MB reason; that path now walks the repo only, but the in-repo case stands.

The same output reaches a worker that runs `ctx` itself, so the fix belongs in `src/ctx/print.rs`: truncate a printed line at about 200 chars, cap def's location list (say 20, then "n more: narrow with --kind or --in"), and maybe cap the denial reason at a few KB.

### Fix the ctx hook and trim ctx output instead of forcing ctx

_added 2026-10-15, origin: harvest 2026-10-15_

A replay of 14 runs (2026-10-15, [ctx-replay-research.md](../../reference/ctx-replay-research.md)) settles the old question of widening the hook's deny: forcing `ctx` would add 17–34% turns for answers no smaller, empty 25% of the time when forced, and no more correct. No defect reached a gate through a lookup miss. ctx is 2.8% of 13,797 lookups, but only about 19% of lookups ask what it answers. Shrinking to outline only keeps its weakest command (a follow-up read 71% of the time). What the data asks for, in order of value:

Items 1–4 (the hook judges the pattern, the denial carries the answer, `def` bodies for a single match, `--in` takes a file) went to [PRD-009](../../archive/009-cheaper-builders/prd.md) FR-4 to FR-6 on 2026-10-15. What is left:

5. **Index gaps:** PHP class constants, Go grouped `const ( )`, trait impls, Pest `test()`/`it()` in outline, extensionless scripts.
6. **Change the metric** in `factory report`: ctx share per question type (def, refs, outline) plus forced-empty and grepped-anyway rates, in place of the global `ctx_share threshold 10%` line and FR-12's outline-only trigger.

Leave `sed -n` ranges and `cat` alone: only 15% of ranges map to one definition, at higher cost. The bigger byte savings are outside ctx, in log reads through Read and whole-file code reads.

### Widen the hook's process-call list

_added 2026-10-15, origin: PRD-009 polish_

The hook's script reading (`process_call` in `src/cli/hook.rs`) matches the list PRD-009 FR-3 fixed, and FR-3 accepted that a call off the list runs. The polish review of PRD-009 found these off-list spellings, each denied on main (any inline script with a mention was unsure) and allowed now:

- `ruby -e 'system "gh pr list"'`, `perl -e 'exec "git", "push"'`: a call with no parentheses.
- `python3 -c '__import__("os").system("gh pr list")'`: the `.system(` follows a `.`, so the bare-call rule skips it.
- `bun -e 'Bun.spawn(["git","push"])'`, `lua -e 'io.popen("git push")'`, `ruby -e 'Kernel.system("gh")'`: a call behind a dot.
- `deno eval 'new Deno.Command("gh",{args:["pr","view"]}).outputSync()'`: Deno's process API is not on the list.
- `osascript -e 'do shell script "git push"'`, `tclsh` `exec`.

Options: add `__import__`, `Deno.Command`, `Bun.spawn`, `io.popen`, `Kernel.`, `do shell script` to the list, and a paren-less `system ` / `exec ` / `spawn ` at a word start. Each addition can only add denials; measure the false-denial count against the store's runs first (the FR-3 research counted 31).

The backstop stays: a worker has no GitHub token, an empty gh config dir and a push-off git env (`docs/decisions.md#who-talks-to-github`).

### A guard-loosening FR replays everything main denied

_added 2026-10-20, origin: harvest 2026-10-20_

##### What happened

PRD-009 FR-3 loosened the hook so a script is judged by what it runs, not by the words it holds. FR-3's acceptance listed the lines that must now pass and three lines that must still be denied. The builder, the checkpoint and its Prove all went green. Polish then found ten bypasses: lines main's hook denied that the branch's hook let through (argv, env, stdin, `$S`, flag clusters, a second `-e`). Every one was a line main already denied, and nothing ran them.

##### What it should do

- **A hook regression corpus in the tests.** Every line the current hook denies, kept as a fixture, run against the hook in one test. A change that lets one through fails unless the corpus entry is moved to an "allowed on purpose" list in the same diff.
- **A `/prd` acceptance line for any FR that loosens a guard** (a hook, a deny rule, a validator): "everything the old guard denied that this FR does not mean to allow stays denied", with the corpus as its proof. `/slice` carries it into the Prove task.
