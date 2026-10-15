# Builder cost: what is in the start context, where turns go, what to change

Research run 2026-10-15, during the harvest of 001 to 008, to answer the owner: is the builder's 45k start context needed for quality, and how would a builder take fewer turns? The scripts (`turns.py`, `agg.py`, `levers.py`, `sim.py`, `runa.py`, `probes.sh` and the rest) stayed in the session scratchpad; the method below is enough to redo them. Read-only on repos, on a copy of `factory.db`.

## Summary

1. **Data:** 91 builder sessions from 14 factory runs, 3,871 API turns, 74.9M weighted (W = input + cache write + 0.1 × cache read + 5 × output). For comparison, the run A (`/deliver`) builder subagents for arca 024 and icg 018.
2. **Start context:** it averages 49k. About 27–30k is fixed Claude Code overhead and about 20k is the pack. About 17k of the overhead is tooling a builder never uses. Four probes measured it: 24.9k down to 7.8k.
3. **Turns:** 90% of turns make a single tool call (only 9.5% make several). 49% are pure exploration (read, grep, ctx, git). Context grows from 49k to 92k in the first 10 turns, and the first edit comes at turn 9.6 on average. Verification is 17.5% of turns, with 179 red runs (94 test, 56 compile, 14 fmt).
4. **Cost split:** re-reading the start context is 23.8M (32%). Carrying what earlier turns added is 27.4M (37%). Output is 17.6M (24%), about a third of it thinking. Compaction is 2.2M (3%).
5. **Orchestration is not the cause.** On the same PRDs, run A subagents and run B workers cost the same per turn (18.0k vs 18.1k). Single-call share was 96% vs 94%, and the first edit came at turn 10.7 vs 11.9.
6. **Lever 1 (low risk, one change):** add `--strict-mcp-config --disable-slash-commands --tools Bash,Read,Edit,Write` to the worker command, about 8.0M (10.7%). Builders called the dropped tools 9 times in 3,871 turns.
7. **Lever 2 (batching):** 480 of 1,154 back-to-back exploration pairs were independent, so about 4.8M (6.4%). The pack's Tokens rule already asks for batching and is ignored. Claude Code's bypass-mode reminder pushes one-off `sed`/`cat` calls instead, so prompt text alone is weak.
8. **Smaller levers:** hook denies 2.1M (31 were false "gh/git push" hits on python heredocs); driver owns docs bookkeeping about 1.1M; fmt in write mode up to 1.3M; quieter test output about 0.9M; builder-only Autonomy text about 0.9M; pack pre-load nets 1–4M, uncertain, overlaps batching.
9. **Not levers:** smaller slices or a fresh session per task (simulated −0.8M to +0.9M: each new session pays the 49k start plus about 10 orientation turns again); a lower compaction threshold (140k: +0.3M, 120k: −1.5M).
10. **Answer:** a good build needs about 30k of start context, not 45k. Fewer turns come from session flags and builder tooling (batched or pre-loaded reads, fewer denies), not from orchestration, and only weakly from more contract text. Effort high → medium (thinking is 7% of W) is untested.

## Sources and method

- **Data:** a read-only copy of `factory.db`; worker streams `runs/<id>/workers/<sid>.jsonl`; transcripts `~/.claude/projects/*/<sid>.jsonl` (all 91 builders have one); packs `runs/<id>/packs/slice-*.md`.
- **Trusted numbers:** output tokens from the transcripts (the stream's per-message `output_tokens` is partial); totals from the stream's final `result.modelUsage`, which includes compaction calls. The DB sums to 62.9M for the same builders: it under-records 002-20261013-e729 slices 2, 7 and 9, and 019-20261015-516b has empty totals.
- **Scripts** (scratchpad, not kept): `turns.py` classifies each turn; `agg.py` per-turn cost attribution (`agg.json`); `report.py`, `levers.py`, `levers2.py`, `turncost.py`, `preload.py` the lever arithmetic; `sim.py`, `sim2.py` compaction and session-split simulations; `runa.py` run A vs run B; `attach.py`, `probes.sh` start-context composition.
- **Attribution:** each turn costs 0.1 × its context + cache write + 5 × output. A turn's context splits into its share of the start (written once, read at 0.1 every turn) and the carry of what earlier turns added. What a turn adds is charged to its kind as size × (1 + 0.1 × later turns before the next compaction). This accounts for 68.8M of the 70.1M message-level W.
- **Rule of thumb:** each 1k tokens removed from the start saves about 0.47M W across these builders.

## 1. What the start context is made of

Example: arca 024 slice-05 builder, session `042634d8-375b-4bb2-a3c8-2401cfcdb7f3`, first call 48.4k (37.9k cache write + 10.5k cache read).

**The worker command** (`src/worker.rs:206-217`) restricts nothing: no MCP, tool, skill or setting-source limits. Every user setting loads: the output style, the codex, caveman and pyright-lsp plugins, SessionStart hooks (`bd prime` fails with "command not found" on every builder), `autoCompactWindow: 200000`.

**Probes** (each adds its flag to the one before; empty temp dir, `-p "reply ok" --model opus --effort high --permission-mode bypassPermissions`):

| Probe | Prompt tokens | What it removed |
|---|---|---|
| P1, same as a worker | 24,932 | — |
| P2, `--strict-mcp-config` | 22,852 | Claude Docs MCP (3 tools, instructions, deferred names), about 2.1k |
| P3, `--disable-slash-commands` | 22,108 | only 0.7k: the Workflow tool's definition grew from 9.0k to 43.8k chars and absorbed the 19.2k-char skill listing |
| P4, `--tools Bash,Read,Edit,Write` | 7,844 | about 14.3k: unused tool definitions plus the skill, agent and deferred-tool listings |

**P1 → P4 removes 17.1k tokens.** Tool definitions in chars: Workflow 9.0k, ScheduleWakeup 8.1k, Agent 5.5k, Skill 3.4k, ReportFindings 2.9k, ToolSearch 2.6k, ListAgents 2.1k. Listings: skills 19.2k (23.4k in arca), agents 3.2k, deferred tools 3.6k, MCP instructions 2.2k.

**Stays in P4, needed:** the system prompt (about 6.7k chars), Bash, Read, Edit and Write definitions (9.6k), environment, date, session context.

**Stays in P4, questionable** (would need `--setting-sources project,local`, untested; about 1.2–2k tokens): the output style (3.9k chars, written for a human chat while the driver parses the final message); the global `~/.claude/CLAUDE.md` (2.2k); in real repos the project CLAUDE.md (1.9k) and auto-memory MEMORY.md (2.4k in arca); the bypass-mode reminder (0.5k). That reminder tells the model to read files with cat, head or sed -n instead of Read, Edit and Write, which contradicts the pack's Tokens rule ("Read a file once with Read"). Builders follow the reminder: 912 turns read files through Bash, 145 used Read.

**Real builders:** start context regressed on pack size gives about 0.39 tokens per char. The non-pack part is 26k (factory), 29k (arca), 34k (icg), 37k (cody).

**The pack** averages 51.8k chars, about 20k tokens, over 70 builders: slice 13.1k, autonomy 11.4k, outline 10.5k, PRD sections 8.7k, log 4.4k, contract 3.2k, lookup 0.9k. About 5k chars of Autonomy is not builder work: the Mapping table (polish/ux/harness buckets), CI and flaky caps, blocker steps 3–5 (the driver's job), "The owner answers mid-run", "Resuming".

**Verdict:** of the 49k average, about 17–19k (flags) + 2k (Autonomy) + 1–2k (setting sources) is not builder work. About 28–30k is what a builder needs.

## 2. Where the turns go

| Run | Builders | Turns | Avg turns | Start | Peak | Compactions | W | Reds (fmt/compile/test) |
|---|---|---|---|---|---|---|---|---|
| 002-e729 | 11 | 691 | 63 | 53k | 170k | 10 | 16.5M | 0/20/14 |
| 005-d1ad | 10 | 534 | 53 | 49k | 166k | 6 | 10.0M | 7/12/19 |
| 024-e894 | 9 | 565 | 63 | 45k | 166k | 5 | 10.2M | 0/6/13 |
| 025-a235 | 10 | 436 | 44 | 52k | 168k | 7 | 8.9M | 0/9/14 |
| 019-48af | 6 | 324 | 54 | 55k | 169k | 3 | 6.4M | 1/2/0 |
| 008-b926 | 8 | 148 | 18 | 43k | 92k | 0 | 2.1M | 2/1/4 |
| 007-c8fa | 5 | 93 | 19 | 44k | 104k | 0 | 1.5M | 2/0/3 |

- **Turns per builder:** median 38, p90 77, max 145 (002 slice 9, `8ed703e0-1528-46fa-bf1d-ae6e8bea30c8`).
- **Context growth:** 49k at turn 0, 76k at 5, 92k at 10, 113k at 20, 129k at 40, then capped by compaction. Each turn adds a median 1.5k (mean 2.4k, p90 5.5k).
- **Single vs parallel:** 3,412 turns made one call, 360 made several (9.5%), 99 none. 1.10 calls per turn.
- **Reads:** 1,988 file reads; 790 (40%) went back to a file already read, almost always a different range; 24 exact repeats. 568 reads were of files the pack's Outline lists.
- **Before the first edit:** 9.6 turns on average. Excluding turn 0, 651 exploration-only turns cost 7.9M (10.6%) and grew context 34k per builder. 80% of those reads (915 of 1,147) were of files the pack text already names.
- **Red runs:** 179 (test 94, compile 56, fmt 14, other 13, lint 2). The two turns after each red cost 4.8M. Most reds are the loop doing its job.
- **Inner loop vs targeted tests:** inner-loop runs per slice fell from 5.7 (002) to about 1 (024, 025, 008). Targeted test runs are now the bigger share: 337 turns against 186.
- **Compaction:** 36 auto-compactions in 28 of 91 sessions, each at about 170k, about 68 s and 64k W, 2.2M total. In the 12 turns after a compaction, 184 of 223 reads were of files read before it.

**Cost by turn kind** (70.1M message-level W; turn 0 carries the start write):

| Kind | Turns | % turns | Start share | Carry | Output | Total | Per turn |
|---|---|---|---|---|---|---|---|
| Bash read (sed -n/cat/head) | 912 | 23.6 | 4.67M | 8.80M | 2.78M | 16.3M | 17.8k |
| Edit/Write | 339 | 8.8 | 1.70M | 2.22M | 4.33M | 8.3M | 24.4k |
| Bash write (heredoc/python/sed -i) | 413 | 10.7 | 2.11M | 2.19M | 2.80M | 7.1M | 17.2k |
| Search (grep/find/ls) | 505 | 13.0 | 2.64M | 2.73M | 1.62M | 7.0M | 13.8k |
| `factory docs` | 182 | 4.7 | 3.52M | 1.58M | 0.69M | 5.8M | 31.8k |
| Targeted tests | 337 | 8.7 | 1.70M | 1.19M | 1.70M | 4.6M | 13.6k |
| git | 197 | 5.1 | 2.28M | 1.49M | 0.44M | 4.2M | 21.4k |
| Read tool | 145 | 3.7 | 0.75M | 3.00M | 0.25M | 4.0M | 27.5k |
| ctx | 149 | 3.8 | 0.78M | 1.15M | 0.57M | 2.5M | 16.8k |
| Denied by hook | 126 | 3.3 | 0.73M | 0.97M | 0.69M | 2.4M | 19.0k |
| Inner loop | 186 | 4.8 | 0.94M | 0.36M | 0.48M | 1.8M | 9.6k |
| Build | 82 | 2.1 | 0.43M | 0.27M | 0.35M | 1.1M | 12.8k |
| fmt | 71 | 1.8 | 0.35M | 0.29M | 0.33M | 1.0M | 13.8k |

- Turn 0 is a `docs set … building` call in 51 builders and git in 29, inflating those rows. Turn 0 alone: 3.8M (5.1%).
- Output: thinking is 1.05M of 3.18M output tokens (33%), about 5.2M W (7%). All 91 builders ran opus at high effort.

**Hook denies** (110 fully denied turns, 2.1M): 71 "use ctx refs" (1.2M; the bypass-mode reminder tells them to "search with grep and find"); 31 false "workers never run gh or git push" (0.74M; python heredoc edits whose code holds `push` or `gh`, e.g. 002 slice 1 `5ac181c1-78db-48ca-b3b3-6fb083c934c3`; cause: `forbidden()` in `src/cli/hook.rs` falls back to `mentions()` on an unparseable command); 8 "file is in your pack under # Outline" (0.1M).

**Bookkeeping:** turns that only ran `docs set`, `docs check` or `docs log add`: 86 turns, 1.5M. 024 slice 5 lost turns to "unrecognized subcommand" from a stale installed binary.

**Run A vs run B** (same PRDs; run A builders are `/deliver` subagents with no pack):

| | Builders | Turns | Start | W | W/turn | 1-call | Exploration | First edit | Reads (repeat) | Inner/test runs | Denies |
|---|---|---|---|---|---|---|---|---|---|---|---|
| arca A 024 | 9 | 547 | 34k | 9.86M | 18.0k | 96% | 56% | 10.7 | 304 (86) | 15/92 | 0 |
| arca B 024 | 9 | 565 | 45k | 10.24M | 18.1k | 94% | 58% | 11.9 | 298 (96) | 11/115 | 7 |
| icg A 018 | 6 | 271 | 37k | 4.92M | 18.1k | 94% | 56% | 10.7 | 175 (36) | 11/49 | 0 |
| icg B 018 | 6 | 195 | 54k | 3.69M | 18.9k | 89% | 54% | 12.3 | 87 (10) | 9/33 | 10 |

Run A's W misses subagent compaction calls. The builder behaves the same under both orchestrations; B's pack, Outline included, did not shorten orientation.

## 3. Levers, ranked

Savings over the 91 builders (74.9M W), turn counts held fixed unless stated.

| # | Lever | Saving | Risk | What it takes |
|---|---|---|---|---|
| 1 | Drop unused start context: `--strict-mcp-config --disable-slash-commands --tools Bash,Read,Edit,Write` | 8.0M (10.7%), measured at 17.1k per builder; likely 8.9M in real repos | Low: dropped tools used 9 times in 3,871 turns; the hook on Bash and Read still applies | Three flags in `worker.rs`; probably right for checkpoint and polish workers too (unmeasured) |
| 2 | Batch independent reads | 4.8M (6.4%); bounds 7.0M (pairs) to 11.5M (whole runs) | Low | Prompt alone fails. Options: a first tool turn that reads every file Current code names in one message; `factory ctx show f:a-b g:c-d …`; drop the bypass-mode reminder (untested) |
| 3 | Pack pre-loads the first reads | 1–4M net, uncertain | Medium: today's Outline shows no gain | Include Current code ranges within 10–15k tokens; drop Outline entries they cover. Overlaps lever 2 |
| 4 | Fewer hook denies | up to 2.1M (2.8%) | None for false positives; low for ctx | Judge an unparseable command by its first word (idea filed); one contract line on `factory ctx refs` |
| 5 | Quieter verification output | about 0.9M (1.2%) | Low | Test commands that print failures and the summary only |
| 6 | Driver owns docs bookkeeping | about 1.1M (1.5%) | Low | Driver sets building and done and runs `docs check`; keep `docs log add` for the builder; preflight that `factory docs` works |
| 7 | Format in write mode before checks | 0.8–1.3M (1.1–1.7%) | Low; the gate keeps `fmt --check` | `cargo fmt && clippy && test`, or a formatter hook after Edit/Write |
| 8 | Trim the pack to builder text | about 0.9M (1.3%) | Low | A builder-specific Autonomy cut; test the Outline with lever 3 |
| 9 | Drop user setting sources | 0.6–0.9M, estimated | Unknown | `--setting-sources project,local`; one probe settles it |
| 10 | Effort high → medium | up to 2.6M (3.5%) | Unknown | Controlled A/B on one PRD |
| – | Smaller slices or fresh session per task | −0.8M to +0.9M | — | Not worth it on cost |
| – | Lower compaction threshold | 140k +0.3M, 120k −1.5M, 100k −11.6M | More re-reads | Not a lever |

**Combined:** levers 1, 2, 4, 6, 7, 8 add up to about 18M (24%) before overlap. Lever 1 is measured, certain and one code change.

**Orchestration or builder prompt?** Neither as framed. Not orchestration: run A vs B is the same per turn. Not prompt text: the pack already asks for batching and one Read per file, and the bypass-mode reminder says the opposite. What works: session configuration (levers 1, 9, 10) and mechanisms that don't depend on compliance (a multi-range read, a pre-loaded pack, a hook without false positives, a driver doing the bookkeeping).

## Gaps

- Probes ran in an empty dir; real repos add a project CLAUDE.md, auto-memory and a larger skill listing, so lever 1 is probably understated.
- 0.39 tokens per char is a regression, not a tokenizer count.
- "Independent" in lever 2 is a heuristic.
- Levers 3, 9, 10 are estimates with no run behind them.
- 002-20261013-e729 slice 9 transcript is partial (145 of 308 turns). 019-20261015-516b has no DB totals.
- Run A subagents report no `modelUsage`, so their compaction calls are missing from run A's W.
