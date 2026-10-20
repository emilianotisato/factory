---
number: 009
slug: cheaper-builders
title: "Cheaper builders"
# type: feature | bugfix | enhancement | refactor
type: enhancement
created: 2026-10-15
author: "Emiliano Tisato (with Claude)"
summary: |
  A builder starts at about 25k tokens instead of 43–53k, and spends fewer turns on false hook denials, fmt reds and docs bookkeeping. Build, fix and merge workers get four tools and no MCP or skills; every worker gets the default output style; the hook judges what it denies and answers it; the driver owns a slice's done; the inner loop formats in write mode and tests quietly.
  Agreed 2026-10-15: FR-1 to FR-10, nothing cut. The first run after the merge is compared with the 002–008 baseline before this is archived.
from_idea: "2026-10-15-cut-the-builder-s-start-context-and-its-one-call-t"
---
> **History.** The code, `docs/reference` and `docs/decisions.md` win over this document.

# PRD-009 — Cheaper builders

## 1. Why now

After the first harvest (2026-10-15), the owner asked two things: is the builder's start context needed for quality, and how would a builder take fewer turns? Two research notes answer with the store's 14 runs: [builder-cost-research.md](../../reference/builder-cost-research.md) (91 builders, 74.9M weighted) and [ctx-replay-research.md](../../reference/ctx-replay-research.md).

**The cost.** Builders are 62.9M of the 96.9M weighted that workers spent over those runs (checkpoint 14.1M, polish 17.4M, fix 2.0M, merge 0.5M). A builder starts at 49.4k tokens on average, 43,334 to 53,232 in this repo's runs 002–008. About 28–30k of that is what a build needs. The rest is tool definitions, skill and agent listings and MCP servers no builder uses, plus the owner's output style and memory. 90% of a builder's turns make one tool call. The A/B rounds set a bar of 0.6× the manual run and missed it on arca-api, at 86.4% once polish subagents were counted ([runs.md](../../reference/ab/runs.md), arca-api § Numbers, both runs, and its correction).

**Bugs the owner's runs hit:**
- **False "workers never run gh or git push" denials.** 31 of them, 0.74M: python heredoc edits whose code held `push` or `gh`, such as PRD-002 slice 1 (session `5ac181c1-78db-48ca-b3b3-6fb083c934c3`, a heredoc holding `line.push(`). Cause in §3 FR-3.
- **The ctx deny misfires.** Of 175 denials in 61 workers, 62% blocked literal or regex greps, which the deny's own hint says to grep. 36 workers were denied more than once, up to 14 times.

**Improvements**, measured but not failures: the unused start context (FR-1, FR-2, FR-7), the denial that sends the worker to fetch an answer the hook could give (FR-5, FR-6), docs bookkeeping turns (FR-8), fmt reds (FR-9), test output (FR-10).

**Left alone:** checkpoint and polish tool sets, effort, the permission mode, and every lever no probe or run has measured (§7).

## 2. Blast radius

- **Every worker of every run, in every repo.** Build, fix and merge workers lose Agent, Monitor, ToolSearch, Skill, MCP and slash commands. Every worker loses the owner's output style; build, fix and merge also lose the owner's auto memory. A builder that used one of those tools now cannot: 15 calls in 4,407 did.
- **The hook judges every role**, polish review subagents included (68 of the ctx denials were theirs). Fewer greps are denied, and a denial now reads the tree, which ends the hook's "no I/O beyond stdin…" promise on that path.
- **`factory ctx def` output changes shape** when several definitions match. Its readers are workers and, after FR-5, the hook. `--in` gains a file form; nothing is removed.
- **The slice contract and the driver's done routing change together.** Compatible both ways: a builder on an old pack that sets done itself still routes as today.
- **The `/gate` skill text** moves the skill bundle to 0.7.0 on this branch. It reaches other repos only after the owner's release (tag, `factory skills update`) and a `/gate audit` there.
- **The polish worker's retro and PR body prose** stop following the owner's chat output style; the docs rules they follow are unchanged.
- **No external contract moves.** No GitHub call, PR body section, CLI flag or exit code is removed.

## 3. FR-1 — Build, fix and merge workers get four tools

**Today.** One `claude` command serves every role (`src/worker.rs:206-216`): `-p`, stream-json, `--permission-mode bypassPermissions`, the pack as an appended system prompt, `--model`, `--effort`, `--settings`. Nothing limits tools, MCP or skills. A probe in this repo (2026-10-15, one `true` call then "ok", opus high) started a worker-equivalent session at 25,944 tokens: 26 tools, 6 MCP servers, 54 skills, 92 slash commands. Adding `--strict-mcp-config --disable-slash-commands --tools Bash,Read,Edit,Write` started it at 9,300 with 4 tools, and the `--settings` hook still fired.

**Decision (2026-10-15).** The three flags go on the command when `Job::role()` (`src/worker.rs:119-127`) is `build`, `fix` (a gate fix and a CI fix) or `merge`. Checkpoint and polish keep today's command: polish runs as the `/polish` skill (`src/pack.rs:557`) and called Agent 71 times and Skill 37 times; checkpoints loaded a verify skill through Skill and called an MCP server 11 times.

**Accepted cost.** A builder cannot spawn a subagent, watch a process with Monitor, load a deferred tool, or call an MCP server. Over 91 builders that was 15 calls in 4,407 (Agent 5, ToolSearch 4, TaskStop 3, Monitor 2, ScheduleWakeup 1); fix workers made 0 of 147, merge workers 0 of 32. The Grep and Glob tools go too; builders grep through Bash, which the hook judges. Autonomy § Tokens names the Grep tool ("Several Read, Grep or Bash calls side by side", `assets/autonomy.md:80`); it becomes "Several tool calls side by side", for every role.

**Acceptance.** In the stub harness, the recorded argv of a build, a fix and a merge worker each holds `--tools Bash,Read,Edit,Write`, `--strict-mcp-config` and `--disable-slash-commands`; a checkpoint's and a polish worker's argv hold none of the three.

## 3. FR-2 — Each role gets its own settings

**Today.** `settings()` (`src/worker.rs:92-102`) writes one JSON for every role: `permissions.deny` and the PreToolUse hook. Each worker already gets its own file (`settings_path(run_dir, session_id)`). A probe adding `"outputStyle": "default"` and `"autoMemoryEnabled": false` to that file took the FR-1 session from 9,300 to 6,762 tokens: the default style, no MEMORY.md, the hook still firing.

**Decision (2026-10-15).** Every role's settings file holds `"outputStyle": "default"`. Build, fix and merge also hold `"autoMemoryEnabled": false`; checkpoint and polish keep the owner's memory, because they judge what a project's gotchas mean.

**Rejected.** `--setting-sources project,local`: in the probe it dropped only the output style, while the global `~/.claude/CLAUDE.md` and MEMORY.md still loaded (8,099).

**Accepted cost.** The global `~/.claude/CLAUDE.md` still loads into every worker: no flag drops it without dropping the project's own. Builders lose the owner's memory, whose facts are about the machine and the owner, not a slice.

**Acceptance.** In the stub harness, the settings file each worker's argv names holds `outputStyle` `default` for all five roles, and `autoMemoryEnabled` `false` for build, fix and merge only. The deny rules and the hook are unchanged in all five.

## 3. FR-3 — A script that only mentions gh or push runs

**Today.** `forbidden()` (`src/cli/hook.rs:173-187`) denies when any command is unsure and the text `mentions()` gh or push as a whole word anywhere (`:199-210`). An interpreter goes to `interpreter()` (`:316`, `:514-549`), which answers Unsure for a script on stdin (`python3 - <<EOF`, or no arguments) and for an inline flag (`-c`, `-e`, …), without reading the script. So a python heredoc that edits Rust holding `line.push(` is denied with `NEVER_HINT` (`:14`).

**Decision (2026-10-15).** When the doubt is an interpreter's script, its stdin or here-document or its inline-flag argument, the hook reads that script. It is Forbidden only when the script both makes a process call and mentions gh or push; otherwise it is Fine. A process call is one of: `subprocess`, `os.system`, `os.popen`, `os.exec…`, `os.spawn…`, `pty.spawn`, `child_process`, `execSync`, `spawnSync`, `IO.popen`, `Open3`, `shell_exec`, `passthru`, `proc_open`, a backtick, `%x`, `qx`, or a bare `system(`, `exec(`, `spawn(` or `popen(` not preceded by `.`. Every other doubt (unbalanced quoting, nesting past `MAX_DEPTH`) keeps today's rule. So does a script piped in (`printf '…' | python3`): the parser fills an interpreter's stdin only from a here-document or a here-string (`src/shell.rs:418`, `:455`), so there is no script to read.

**Rejected.** Today's rule (31 false denials). A parser per language: no evidence a list misses.

**Accepted cost.** A script that reaches gh through a call off the list runs. The backstop is [who talks to GitHub](../../decisions.md#who-talks-to-github): a worker has no GitHub token, an empty gh config dir and a push-off git env.

**Acceptance.** Hook tests: a python heredoc editing a file with `line.push(` and `git` in its text, and no process call, is allowed; `python3 -c "print('push')"` is allowed. A python heredoc calling `subprocess.run(["gh", "pr", "view"])`, `python3 -c "import os; os.system('git push')"` and `node -e "require('child_process').execSync('gh pr list')"` are denied with `NEVER_HINT`. Every hook test whose rule FR-4 and FR-5 leave alone stays green.

## 3. FR-4 — The grep deny judges the pattern

**Today.** `bash()` denies grep, egrep, fgrep and rg over a repo dir whatever the pattern (`src/cli/hook.rs:142-145`), with `grep_hint()` (`:16-21`): "use: factory ctx refs <word> (or ctx def <symbol>); grep only for literal strings and log output". Of 175 denials, 62% were literal or regex greps and 3% were vendor code.

**Decision (2026-10-15).** The hook denies only when the pattern is a single identifier, `[A-Za-z_][A-Za-z0-9_]*`, alone or as a whole word (`-w`, `\b…\b`, `\<…\>`), and no searched path lies under a `VENDOR_DIRS` dir (`src/ctx/walk.rs:8`: `vendor`, `node_modules`, `target`). Literals, regex, several patterns and vendor paths pass. The hint adds `--vendor` for vendor code.

The texts that state the old rule say the new one: a bare name goes to ctx, anything else may be grepped. They are `grep_hint()` ("grep only for literal strings and log output", `src/cli/hook.rs:18`) and the pack's `# Lookup` (`assets/lookup.md:7`, `:9`). The tests that pin the old rule change with it, and this PRD naming them is what makes the change no weakening (`assets/autonomy.md:54`):
- `recursive_grep_over_the_repo_is_denied_with_the_verb` (`src/cli/hook.rs:880-897`) expects `rg -n 'a b' src` and `egrep -r 'a|b' ./` denied; FR-4 allows both.
- `tests/cli.rs:100-107` pins the hint's exact text.
- `tests/loop.rs:412` pins the Lookup's "grep only for literal strings and log output".

**Rejected.** Forcing more lookups through ctx: the replay puts it at +17–34% turns for answers no smaller and empty 25% of the time when forced.

**Accepted cost.** A qualified name (`Foo::bar`, `obj.method`) is no identifier and passes as a literal, so some symbol lookups stay greps.

**Acceptance.** Hook tests in the fixture repo, which gains a `vendor/` dir (`repo_dir`, `src/cli/hook.rs:708-719`, matches real directories only, so without one the vendor case passes for nothing): `grep -rn "fn main" src`, `grep -rn 'foo\|bar' src` and `grep -rn Recon vendor/` are allowed; `grep -rn Recon src`, `grep -rnw Recon src` and `rg -n '\bRecon\b' src` are denied with the hint, which names `--vendor`. A slice pack's `# Lookup` states the new rule.

## 3. FR-5 — The denial carries the answer

**Today.** The denial is the hint alone, and 52% of denials were followed by exactly the ctx call it names. The hook's header promises "no I/O beyond stdin, the pack's first line and a `stat`" (`src/cli/hook.rs:1`).

**Decision (2026-10-15).** On an FR-4 denial, the hook runs ctx in-process from the worker's cwd: `refs <word>` with its default caps (10 hits, 3 per file; `src/ctx/mod.rs:47-57`) and `def <word> --no-body`. It puts both outputs under the hint in the denial's reason. A ctx error, or no hit from either command, leaves the hint alone. The rule counts hits, never the text: a miss still prints a line, "no definition of X found" or "no references to X found" (`src/ctx/print.rs:20`, `:65`). The header line says the deny path reads the tree.

**Rejected.** Rewriting the command through PreToolUse `updatedInput`: zero turns, but it changes a worker's command without telling it.

**Accepted cost.** About 40 ms per denial on the replayed repos, bounded by the hook's 5 s timeout (`src/worker.rs:98`); past it, Claude Code runs the grep. The reason grows by the answer the worker would have fetched next.

**Acceptance.** A hook test in a fixture with `alpha` defined once and called twice: `grep -rn alpha src` is denied, and the reason holds the hint, the definition's `file:line` and both call sites. For a word ctx finds nothing for, the reason is the hint alone.

## 3. FR-6 — ctx def prints one body, and `--in` takes a file

**Today.** `def` prints a body for every match (`print::defs` at `src/ctx/mod.rs:94`) and has no `--in` (`DefArgs`, `:31-43`). `refs --in` takes a directory only (`:50-51`). The replay: bodies for a single match only would cut def output by 67%, and a refs call limited to one file cost 1.9× the grep, because ctx cannot limit to a file.

**Decision (2026-10-15).** `def` prints a body only when exactly one definition matches. With several, it prints each one's location and signature, then one line naming `--kind` and `--in` to narrow. `--in` takes a file or a directory, for `def` and `refs`. The texts that promise a body say when one comes: the pack's `# Lookup` (`assets/lookup.md:3`) and the `def` help (`src/ctx/mod.rs:23`).

**Accepted cost.** An agent that wanted every body of an overloaded name makes a second call.

**Acceptance.** ctx tests in its fixture: `def` of a name defined twice prints two signatures, no body lines and the narrowing line; with `--in` naming the file that holds one, it prints that body. `refs X --in <file>` lists only that file's hits.

## 3. FR-7 — The builder's Autonomy is builder text

**Today.** Every pack carries `assets/autonomy.md` whole, 11,932 bytes: slice `src/pack.rs:229`, fix `:283`, merge `:362`, checkpoint `:466`, polish `:548`, CI fix `:595`.

**Decision (2026-10-15).** The slice pack (`for_slice_with`, `src/pack.rs:149`; the push is at `:229`) leaves out § Mapping (`assets/autonomy.md:29-39`), the Slice hand-back, CI, Flaky and Not-this-branch's bullets of § Caps (`:90-93`), steps 3 and 4 of § Blocker protocol (`:99-100`, the driver's in a run), § The owner answers mid-run (`:103-105`) and § Resuming (`:107` on). Step 5's Stop stays. Every other pack carries the file whole. Markers in `autonomy.md` delimit the cut, and no pack carries a marker.

*Corrected 2026-10-15 (review): an earlier draft placed the push in `for_slice` (`:139-230`). `for_slice` only delegates; the push is in `for_slice_with`. The same review found the Slice hand-back bullet (`:90`) promising a builder "sent back once" on `docs check` errors, which FR-8 does not do; it is the orchestrator's rule, so it joined the cut.*

**Accepted cost.** Two Autonomy texts to keep coherent, one file holding both. The cut is 3,273 chars, about 800 tokens per builder start.

**Acceptance.** A pack test on the fixture: the slice pack's Autonomy holds none of "## Mapping", "**Slice hand-back:**", "**CI:**", "**Flaky:**", "**Not this branch's:**", "**Notify**", "## The owner answers mid-run", "## Resuming", and still holds § The plan stays in docs/ (pinned by `every_role_reads_the_plan_stays_in_docs`, `src/pack.rs:1278`) and the Stop step. It is at least 2,500 chars shorter than the checkpoint pack's Autonomy. No pack holds a marker.

## 3. FR-8 — The driver owns a slice's done

**Today.** The slice contract has the builder set its slice building (`assets/contract-slice.md:12`) and, at the end, set it done and run `docs check` with no errors (`:19`). The driver already sets building and the lane on a fresh spawn (`src/scheduler.rs:674-681`). On a builder's exit it routes on the board's slice status: done lands and settles, blocked parks, anything else is dead or limited (`:838-890`). A restarted driver settles a dead builder the same way, from its stream file (`settle_orphans`, `:1560`, calling `summary_from_disk` at `:1600`; defined at `src/worker.rs:406`). The `STATUS:` line becomes a hint (`src/stream.rs:117-131`) the row keeps and nothing routes on (`src/scheduler.rs:1`, `src/stream.rs:99`). No driver code runs `docs check` on a slice's exit. Builder turns on this bookkeeping cost about 1.1M.

*Corrected 2026-10-15 (review): an earlier draft said no driver code runs `docs check` at all. The preflight does: `src/docs/preflight.rs:97` calls `check::run` (`src/docs/check.rs:82`), reached from `src/scheduler.rs:620`, `src/cli/run.rs:93` and `src/closeout.rs:2288`. None of them runs on a slice's exit.*

**Decision (2026-10-15).** The contract drops the building line and the done-and-check line. When a build worker exits with the hint `done` and the board still says building, the driver runs `check::run` on the worker's lane tree:
- **No errors:** it sets the slice done and routes it as today's done.
- **Errors:** it writes a BLOCK entry for the slice whose body holds the check's errors, sets the slice blocked, and routes it as today's blocked. The worker is not resumed.

A restarted driver applies the same rule to a dead builder, from the hint in its stream file. A board that already says done (a builder on an old pack) routes as today. No hint, or any other, routes as today. The contract change and the driver change land in one slice, so no build ever runs where neither side sets done.

**Rejected.** Resuming the builder with the check's errors: a fresh turn at full context, for a failure no run has shown to be common. Autonomy's § Slice hand-back ("sent back once, then blocked", `assets/autonomy.md:90`) is the attended `/deliver`'s rule; FR-7 takes it out of the builder's pack.

**Accepted cost.** For a build worker, the hint plus a clean check now flips the board, so the rule at `src/scheduler.rs:1` and `src/stream.rs:99` changes, and both say so. A builder that claims done with work missing lands; today the same builder would have set done itself.

**Acceptance.** Stub-run tests. The stub builder sets building and done on every call today (`tests/stubs/claude:453`, `:524`), so it gains a step key that leaves the board alone.
- A builder that returns `STATUS: done` without touching the board lands its slice done, and its pack holds no `status=done` line.
- A builder whose lane holds a `docs check` error blocks its slice, with the error code in the BLOCK body.
- A builder that sets done itself lands as today.
- A driver killed while a done builder lingers (the kill pattern at `tests/loop.rs:1178`) lands the slice on restart, through `settle_orphans`.

## 3. FR-9 — The inner loop formats in write mode

**Today.** This repo's inner loop is the gate, `cargo fmt --check && …` (`docs/project.toml:4`). `/gate` round 3 puts format and lint in the inner loop and names no mode (`skills/gate/SKILL.md:80`). It also says "when the full gate runs in about a minute, the inner loop is the gate: declare the same string for both", and the audit repeats it (`:148`). The stack table gives a write form for PHP only (`pint --parallel` / `pint --parallel --test`, `skills/gate/references/stacks.md:16`) and check forms elsewhere, such as Rust's `cargo fmt --check` (`:123`). Builders had 14 fmt reds and 71 fmt turns, 1.0M.

*Corrected 2026-10-15 (review): an earlier draft said the stack table lists check forms only; the PHP row has both.*

**Decision (2026-10-15).** The inner loop runs the formatter in write mode, and the gate keeps check mode. The same-string rule becomes: when the full gate runs in about a minute, the inner loop is the gate's own tiers, with the formatter in write mode and the tests quiet (FR-10). Round 3 and the audit (`:80`, `:148`) say so, and the stack table gives each stack's write form beside its check form. This repo's `inner` becomes `cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q`, through `factory docs gate set --inner`. Its `gate` stays byte-identical.

**The bundle.** `skills/gate/SKILL.md` and `references/stacks.md` are in the skill bundle (`src/skills.rs:34-41`), so the edit changes its hash, and `bundle_lock_matches` (`tests/skills_bundle.rs:179-184`) fails until the lock moves. The slice that edits the skill bumps `Cargo.toml` from 0.6.0 to 0.7.0 and relocks (`FACTORY_BUNDLE_WRITE=1 cargo test`), as PRDs 004 to 008 did in their own branches. Every `skills/` edit of this PRD (FR-9 and FR-10) lands in that one slice: `relock` refuses a second hash at the same version (`src/skills.rs:160-176`).

**Accepted cost.** An inner run can rewrite a file the builder never opened, only where it was unformatted. The gate's `--check` still catches what an inner run missed. A stale skill line polish finds after the relock cannot be fixed on this branch; it goes to an idea (see [the relock idea](../../ideas/2026-10-15-let-relock-replace-the-hash-of-a-version-that-has.md)).

**Acceptance.** `factory docs gate --inner` prints the new string and `factory docs gate` prints the old one, read as a command transcript in this repo (a test never reads the live `docs/`). A bundle test pins that `/gate` round 3 names write mode for the inner loop and check mode for the gate, and `bundle_lock_matches` passes at 0.7.0.

## 3. FR-10 — Quiet test output

**Today.** This repo's inner loop runs `cargo test`, one line per test. The contract's Verify line (`assets/contract-slice.md:14`) says nothing about output, and Autonomy § Tokens asks for "each failing test's own output … and the summary line" (`assets/autonomy.md:82`). Quieter output is worth about 0.9M.

**Decision (2026-10-15).** This repo's inner loop runs `cargo test -q` (the FR-9 string). `/gate` round 3 names each stack's quiet test flag for the inner loop, where one exists. The contract's Verify line says a targeted test run uses the quiet flag too.

**Accepted cost.** Targeted runs depend on the prompt. A builder that needs a passing test's output reruns it without the flag.

**Acceptance.** `factory docs gate --inner` ends in `cargo test -q`. A pack test finds the quiet-output rule in a slice pack's Contract.

## 4. Decisions log

No OPENs. Settled at the interview, 2026-10-15:
- ~~Which roles lose tools?~~ Build, fix and merge. Checkpoint and polish keep theirs (FR-1).
- ~~Which settings?~~ Default output style for all roles, memory off for build, fix and merge (FR-2).
- ~~Which hook fixes?~~ All four: false script denials, identifier-only denies, the answer in the denial, the ctx trims (FR-3 to FR-6).
- ~~Which small levers?~~ All four: the builder Autonomy, write-mode fmt, the driver's done, quiet tests (FR-7 to FR-10).
- ~~How is quality checked?~~ The first run after the merge against the 002–008 baseline (§5).
- ~~What when the driver's check fails?~~ The slice blocks with the errors; no resume (FR-8).
- ~~Who builds it?~~ A factory run (§5).
- ~~The cut: keep which?~~ All five mechanisms no Why-now bug demands stay: FR-5, FR-6, FR-7, FR-8 and FR-10. Nothing is retired.

## 5. QA plan

**Gate**: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` *(declared in project.toml)*
**Surface**: CLI and the worker spawn: the `claude` command line and settings file a worker gets, `factory hook`, `factory ctx`, the packs, the driver's routing of a build exit, `factory docs gate --inner`, the skill texts.
**Harness**: the stub harness in `cargo test` (`tests/stubs/claude` records each call's argv, cwd and env; `tests/harness/mod.rs` holds the fixture repo). No UI and no verify skill.
**Seed state**: the test fixtures; none beyond them.

| FR | Proof on the real surface | Evidence | Human? |
|---|---|---|---|
| FR-1 | run: a stub run with a build, fix, merge, checkpoint and polish worker; read each recorded argv | transcript | no |
| FR-2 | run: the same stub run; read the settings file each argv names | transcript | no |
| FR-3 | run: `factory hook` fed the heredoc, inline and subprocess scripts of §3 FR-3 | transcript | no |
| FR-4 | run: `factory hook` fed the greps of §3 FR-4 in the fixture repo | transcript | no |
| FR-5 | run: `factory hook` fed `grep -rn alpha src` in the fixture; read the reason | transcript | no |
| FR-6 | run: `factory ctx def` and `refs --in <file>` on the ctx fixture | transcript | no |
| FR-7 | run: build a slice pack and a checkpoint pack from the fixture; diff their Autonomy sections | transcript | no |
| FR-8 | run: stub runs whose builder returns done without the board, with a check error, with the board set, and across a driver kill | transcript | no |
| FR-9 | run: `factory docs gate --inner` and `factory docs gate` in this repo; the bundle test | transcript | no |
| FR-10 | run: `factory docs gate --inner`; a slice pack's Contract | transcript | no |
| After merge | The first `factory run` after `cargo install --path . --locked`: `factory report <run>` shows a build `start_context` of at most 30,000 in this repo (002–008: 43,334–53,232), or at least 18,000 below that repo's previous run elsewhere; the builder stream's init lists 4 tools and `output_style` `default`; the `quality:` line stays within 002–008: gate_red ≤ 2, fix_commits ≤ 2, ci_red_rounds ≤ 3, blocks 0 | `factory report` | **yes**: the owner reads the comparison |

**Who builds it.** A `factory` run builds every FR, and nothing needs a human mid-run. That run uses the binary installed before the merge, so it is the last "before" sample. The feature stays shipped, not archived, until the After-merge row is read.

**Build order.** Nothing else is live.

## 6. Slices

Six slices, sliced 2026-10-15. `factory docs status 009` is the board.

- **01, Hook judges scripts and greps** (FR-3, FR-4). The hook reads an interpreter's script, and a grep is denied only for a bare identifier outside vendor code. The hint and the pack's Lookup state the new rule.
- **02, Driver owns done** (FR-8, FR-10's contract line). The contract drops the builder's building and done lines. On a build worker's `done` hint, the driver checks the lane and sets done, or blocks with the errors, live and after a restart. **The highest risk**: it changes how every build exit routes.
- **03, Worker spawn per role** (FR-1, FR-2). Build, fix and merge workers get four tools, no MCP and no slash commands; every role gets its own settings file. The biggest saving, about 26k to 7k start tokens in the probe.
- **04, Builder Autonomy** (FR-7, FR-1's wording). The slice pack carries Autonomy without the orchestrator's and the driver's parts, cut by markers.
- **05, ctx answers** (FR-5, FR-6). `def` prints one body, `--in` takes a file, and a denied grep carries ctx's answer.
- **06, Write-mode fmt and quiet tests** (FR-9, FR-10's inner loop and `/gate` halves). `/gate` and the stack table, the 0.7.0 bump and relock, and this repo's `inner`.

**Order.** 01 goes first because it ends both denials the owner hit, and 05 waits on it. 01, 02, 03, 04 and 06 are independent: each owns its files, so four lanes start at once and the fifth slice takes the first free lane. 05 depends on 01 alone: it adds the answer to the same deny arms of `bash()` and edits the same `assets/lookup.md`. 02 is the riskiest, and as an independent slice it lands early, where the first checkpoints prove it.

**Checkpoints** fall after every second done slice (`checkpoint_every = 2`): after the 2nd, the 4th and the 6th. Each runs the full gate and drives the Prove rows of the slices it covers, below the UI.

**Workflow.** One branch, `feat/cheaper-builders`, one PR, one commit per slice, Prove at the checkpoints. Every `skills/` edit sits in 06, so one bump and one relock cover them (§3 FR-9).

**No `qa-guide.md`.** The one Human row is After merge: the owner reads `factory report` after the first run on the new binary (§5).

## 7. Non-goals

- **Checkpoint and polish tool sets.** They use Skill, Agent and an MCP server (§3 FR-1).
- **The global `~/.claude/CLAUDE.md`** still loads into every worker (§3 FR-2).
- **`git grep` and a grep that is not the first command** stay unjudged (935 `git grep` uses). Catching them means forcing ctx, which the replay rejects.
- **ctx index gaps and a report metric per question type** stay in [the ctx idea](../../ideas/2026-10-15-fix-the-ctx-hook-and-trim-ctx-output-instead-of-fo.md), items 5 and 6.
- **Smaller slices, a session per task, a lower compaction threshold** are no levers: each pays the start and about 10 orientation turns again.
- **Releasing the `/gate` edit.** The bump and the relock ride the branch (§3 FR-9). The tag and `factory skills update` are the owner's, after the merge.
- **Not handled: the `dontAsk` permission mode.** Accepted 2026-10-15: it drops the bypass-mode reminder that tells workers to read with `cat` and `sed -n`, but a probe in a temp git repo showed it denies a Write into `.claude/skills/verify-x/`, a Bash append into `.claude/` and a Write into `.git/`. Builders may edit `.claude/skills/verify-*` (`assets/autonomy.md:43`). Revisit if Claude Code lets an allow rule cover those paths.
- **Not handled: effort medium, a multi-range read, a pack that pre-loads Current code, a partial tool cut for checkpoints.** Accepted 2026-10-15: untested. They wait on [the experiment idea](../../ideas/2026-10-15-experiment-on-the-builder-levers-prd-009-left-unte.md). Revisit when it reports.
- **Not handled: resuming a builder when the driver's check fails.** Accepted 2026-10-15: the slice blocks with the errors. Revisit if check blocks show up in more than one run.

## 8. Verification notes

Checked against `main` at `20c9c45`, 2026-10-15. **Re-verify at slice entry.**

- `src/worker.rs:206-216`: one `claude` command for all roles; no tool, MCP or skill limits.
- `src/worker.rs:92-102`: `settings()`, the deny rules and the `Bash|Read` hook with `"timeout": 5` at `:98`; one file per session through `settings_path`.
- `src/worker.rs:119-127`: `Job::role()`; Fix and CiFix both map to `fix`.
- `src/worker.rs:55`, `:246`: `FACTORY_BOARD` points a lane worker's `docs set` and `docs log add` at the main tree (`board_root`, `src/cli/docs.rs:253-265`); `docs check` is not redirected, so a builder checks its lane tree.
- `src/cli/hook.rs:1`: the no-I/O header. `:14` `NEVER_HINT`, `:16-21` `grep_hint()`, `:121` `bash()`, `:142-145` the grep and rg denies, `:173-187` `forbidden()`, `:199-210` `mentions()`, `:316` the interpreter branch, `:514-549` `interpreter()`.
- `src/ctx/mod.rs:31-43` `DefArgs` (no `--in`), `:46-57` `RefsArgs` (`--in` a DIR, `--max 10`, `--per-file 3`), `:94` `print::defs` with a body per match. `src/ctx/walk.rs:8` `VENDOR_DIRS`.
- `src/pack.rs:229`, `:283`, `:362`, `:466`, `:548`, `:595`: the six packs that push `AUTONOMY` whole; `:557` the polish prompt `/polish`.
- `assets/autonomy.md`: 11,932 bytes; Mapping `:29`, may-write `:41-47`, Tokens `:76`, Caps `:85`, Blocker protocol `:95`, owner mid-run `:103`, Resuming `:107`. The cut's parts count 3,273 chars, where builder-cost-research.md estimated about 5k (corrected 2026-10-15, counted by section). `:54` never weaken a test this run did not write; `:80` names the Grep tool; `:90` Slice hand-back.
- `assets/contract-slice.md:12` building, `:14` Verify, `:19` done then `docs check`, `:26` the STATUS line.
- `src/scheduler.rs:1` the board-is-truth rule; `:674-681` the driver sets building; `:838-890` routing a slice exit; `:1560` `settle_orphans`, `:1600` its `summary_from_disk` call (defined `src/worker.rs:406`); `:2118` `slice_status`. Builders always run in a lane worktree (`prepare_lane`, `:763`).
- `src/stream.rs:99` the hint's own doc comment, `:117-131` `StatusHint::from_result`. `src/docs/check.rs:82` `check::run`, called by the preflight only (`src/docs/preflight.rs:97`).
- `src/skills.rs:34-41` the gate skill and its stack table in the bundle; `:141-158` `check_lock`; `:160-176` `relock` refuses a second hash at one version. `tests/skills_bundle.rs:179-184` `bundle_lock_matches`. `Cargo.toml:3` and `skills/bundle.lock` at 0.6.0.
- `assets/lookup.md:3` def "with its body", `:7` the one rule, `:9` the recursive-grep hint; `src/ctx/mod.rs:23` the def help. `src/ctx/print.rs:20`, `:65` the miss lines.
- `src/shell.rs:418`, `:455`: an interpreter's stdin comes only from a here-document or a here-string.
- Tests pinning the old hook rule: `src/cli/hook.rs:880-897`, `tests/cli.rs:100-107`, `tests/loop.rs:412`. `src/cli/hook.rs:708-719` `repo_dir` matches real dirs only.
- Stub harness: `tests/stubs/claude:453`, `:524` set building and done on every build call; `tests/loop.rs:1178` the driver-kill pattern.
- `src/commit.rs:11-12`: `protected_files` is the PRD and `docs/index.md`; `docs/project.toml` is not protected, so a slice's `inner` change lands.
- `docs/project.toml:3-4`: `gate` and `inner` are the same string.
- `skills/gate/SKILL.md:80` round 3, `:132`, `:148` the audit; `skills/gate/references/stacks.md:123` Rust's format row.
- Probe sessions, 2026-10-15, in this repo: w0 `39c5f634-8833-448a-8c5d-a12997a77820` (25,944), w1 `bf8998e4-75f0-4c44-9c11-63a28c2eb6a7` (9,300), w2 `93318b76-3720-408a-8f73-554b837becc6` (8,099), w3 `dfa26913-f274-4507-bb3e-70a1c9e172b7` (dontAsk, 9,247), w4 `726c8ec1-3466-48bf-b4ed-2c0adb90c230` (6,762).
- Tool calls by role, from the store's 14 runs: build 15 non-core of 4,407; fix 0 of 147; merge 0 of 32; checkpoint Skill 3 and MCP 11 of 1,030; polish Agent 71 and Skill 37 of 3,388.
- Build `start_context` from `factory report`'s `pack_by_role`, this repo: 002 53,232; 004 47,268; 005 48,692; 006 49,332; 007 44,046; 008 43,334.
