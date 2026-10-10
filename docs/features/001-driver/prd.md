---
number: 001
slug: driver
status: in-progress
owns_fr: [1-12]
retired_fr: []
---

# PRD-001 — Driver: the unattended deliver loop

**Type**: Feature — a new Rust binary, `factory`, that replaces the `/deliver` orchestration session. New repo, no migrations. It consumes the existing `docs.mjs` JSON contract and the existing feature-docs v2 files; it changes neither.
**Status**: **IN-PROGRESS** — unattended `/deliver` run started 2026-10-09, approved for delivery the same day. Interview 2026-10-09, four rounds. Adversarial pass 2026-10-09: Cursor (`grok-4.7-high`) timed out at 15 minutes with no result; a fresh Claude reviewer with no interview context found 19 items, 13 confirmed against the sources, every one corrected inline or pinned for slice entry (marked *Corrected 2026-10-09*). OPEN-16 closed 2026-10-09 by a research workflow over 55 of the owner's sessions and four surveys (memo: [`ctx-research.md`](ctx-research.md)); a thin `factory ctx` enters as FR-12. Cut 2026-10-09: the closeout worker folded into polish, the gate cache made path-aware; the other extras kept with their evidence (§15). Sliced 2026-10-09 into 13 slices (§17); one fact corrected at slicing (§4, permissions).
**Created**: 2026-10-09
**Author**: Emiliano (with Claude)

**Owns FR range**: **FR-1 … FR-12**.

**Split from / Related**: the 2026-10-09 brainstorm on rebuilding `~/Dropbox/dev/claude-code-skills` as a Rust CLI. Later PRDs recorded in [`ideas.md`](../../ideas.md): PRD-002 docs port (v3 format), learning loop, stack plugins, `factory init`.

---

## 1. Why now

The framework works and ships, and it is paid for three times over.

1. **Token hungry.** The orchestrator is a Claude Code session that re-reads the board, the skill and the log on every turn. The last three retros: 42M, 73M and 135M cache-read tokens for one feature each (§20). Peak context sits at the compaction ceiling (166–167k) in all three. On 2026-09-17 four runs consumed a week of Max quota in one day.
2. **Slow.** 017-arca-setup took 254 build minutes for its first 12 slices. Lanes exist in the design and have never been used: no project declares them, so every run is serial. Each gate, each `git` call and each log merge costs a model turn to decide and another to read the result.
3. **Messy.** 12 projects carry copies of the same skills. `docs.mjs` is 2,102 lines, half of it drift repair between files that duplicate state. The orchestrator's rules live in prose a model must re-derive every run.

This PRD builds the driver only: the loop that spawns workers, runs gates, commits, watches CI and opens the PR, with zero model tokens of its own. The docs format, the learning loop and stack plugins stay where they are and come later.

## 2. Blast radius

- **The twelve framework projects** — untouched until FR-11. `factory` reads them through `docs.mjs status --json` and `preflight --json`, writes what `/deliver` writes today (slice frontmatter via workers, log fragments, commits, `docs ship`). The old `/deliver` skill stays installed and unchanged until the first A/B run passes.
- **The Max subscription quota** — workers are `claude -p` on the same login (§20). Nothing here moves to API billing. FR-9 adds a pause and a budget because quota is the one resource a bug here can burn overnight.
- **Git and GitHub** — commits by pathspec on `feat/*`, draft PR, CI watch. Same Never list as today: no merge, no force, no push to default, no branch delete. The global `pre-push` hook and the `commit-msg` hook stay the enforcement, `factory` adds no attribution.
- **The phone workflow** — blocks and the closing review reach the owner as `AskUserQuestion` in a Claude Code session over Remote Control. FR-7 keeps that path; if it breaks, the owner notices on the first run.

## 3. FR-1 — Run lifecycle

Today `/deliver` §0 runs `docs preflight --json` and routes on `pass`, `ask[]`, `judge[]`, `refuse[]` (`deliver/SKILL.md:15-68`). Resume is a table of "what is true on disk" checks, re-derived by the model each time.

**Decisions taken 2026-10-09:**

- **`factory run NNN` is a detached process.** It owns the loop until the PR is open or the run parks. Run state lives in `~/.local/share/factory/runs/<id>/` (state file + `events.jsonl`), the slice board stays the source of truth for what is built. *(Accepted cost: two places describe progress; the board wins on any disagreement and `factory` re-reads it before every decision.)*
- **Preflight is `docs.mjs preflight --json`, routed by code, and it gates only the start of a new run.** `refuse[]` exits 1 with the tool's own text. `ask[]` goes to the cockpit (FR-7) as one question each; a run never starts on an unanswered one. `judge[]` is logged, one `judge_noted` event per line, shown by the cockpit and never waited on: the driver is the judge, as `/deliver` §0 is today. *(Corrected 2026-10-09 by the owner, BLOCK-201: `docs.mjs preflight` returns judge lines on every repo, so parking on them would have parked every run. "Deliver should be unattended.")* Branch creation follows `branch.feature` from the same JSON. A resume of a live run reads `status --json` directly and skips preflight. *(Corrected 2026-10-09 after adversarial review: an earlier draft routed every `factory run` through preflight. `docs.mjs:1703` refuses on any open BLOCK, which would have made resuming a run with a parked lane impossible.)* *(Rejected: reimplementing preflight in Rust. That is PRD-002's job; duplicating it now creates the drift this project exists to remove.)*
- **Resume is the default start.** `factory run NNN` on a feature with an existing run id continues from the board: done slices skip, an unverified checkpoint runs first, open blocks wait. A dead worker (process gone, no DONE/BLOCKED line) is treated as today: fresh worker, note in the log. `--resume` of the dead session is Not handled (§19).
- **Session death is not run death.** The driver is detached from the cockpit; killing the Claude Code session, closing the laptop lid excepted, leaves the run going. *(Accepted cost: a runaway run needs `factory stop <id>`, which must exist from the first slice.)*

**Acceptance**: in the fixture (FR-10), kill the driver mid-slice; `factory run NNN` continues from the board and no slice is built twice. A refused preflight prints the refusal and exits 1 with no worker spawned.

## 4. FR-2 — Workers

Today every slice is an `Agent` call from the orchestrator session with a ~70-line contract pasted in (`deliver/SKILL.md:75-164`). The orchestrator pays to compose it and to read the result.

**Decisions taken 2026-10-09:**

- **A worker is `claude -p` with a context pack.** Flags: `--output-format stream-json --verbose --permission-mode bypassPermissions --append-system-prompt-file <pack> --session-id <uuid> --model <m> --effort <e>` (the file form of the prompt flag is confirmed at slice entry, §20). The pack holds: the slice file, the PRD sections the slice cites, the slice contract, the autonomy rules (decide/log/block, Never list), the gate inner command, the AD numbering seed `{NN}01`. Nothing else. *(Accepted cost: a worker that needs more context greps for it, as today. The pack is tuned from the metrics store, not guessed in this PRD.)*
- **A pack doc is not read twice.** The guard hook (the one that gives the lookup hint, §14) answers a whole-file Read or `cat` of a file the pack carries in full (the slice file, the plan) with a one-line hint naming the pack section; partial reads and reads of `prd.md` go through. FR-8 counts every read of a pack-held file as `pack_doc_read_chars`. Evidence (owner transcripts, 2026-10-09): 631 such reads, 10.6M chars, 40.3% of whole-file read bytes ([`ctx-jev-research.md`](ctx-jev-research.md) §3). Built in FR-12 slice 3 with the lookup hint. *(Accepted cost: a worker that wants a PRD section the pack cut reads `prd.md` as today; the figure says whether the pack's excerpt is too thin.)*
- **Permissions as today: bypass plus deny rules plus the guard hook.** `bypassPermissions` is the only mode that runs unattended; the project's `permissions.deny` and the global hooks keep push, merge and force off limits. Re-verify at slice entry that deny rules hold under bypass (§20). *(Corrected 2026-10-09 at slicing: an earlier draft said "plus the guard hook, as today". No Claude Code hook exists today: neither dogfood project has a `.claude/settings.json`, the deny rules (`gh pr merge`, `git push --force*`) live in the owner's user settings, and the "global hooks" are the git `pre-push` and `commit-msg` hooks under `~/.config/git/hooks/`. The hints of this section and §14 are therefore a new PreToolUse hook, `factory hook`, that the driver injects per worker through `--settings <json>` (in the 2.1.280 `--help`); the same settings carry the two deny rules, so a worker is guarded on any project. Built in slice 13.)*
- **Claude builds, Cursor reviews.** Workers are Claude Code; the adversarial review in polish stays on `cursor-agent` as the `second opinion` role in `models.md`. *(Rejected: Codex workers. Different flags, different stream, no evidence they are cheaper for this work. Left as an idea.)*
- **The stream is the record.** `factory` captures stream-json to the run directory, parses the final `result` event for `usage`, `modelUsage`, `num_turns`, `duration_ms`, `permission_denials`, `terminal_reason`, and writes a worker row (FR-8). The DONE/BLOCKED/FAILED line at the end of `result` is a hint; the slice frontmatter after `docs status --json` is the truth.
- **Slash commands inside `-p` are verified at slice entry, not assumed.** The polish worker needs `/polish`; if slash commands do not resolve in headless mode, the pack carries the skill body instead.

**Acceptance**: in the fixture, the stub agent receives a pack containing exactly the slice, its cited PRD sections and the contract; the parsed `result` lands as one worker row with all usage fields non-null.

## 5. FR-3 — Lanes and parking

`docs status --json` already computes `ready[]`, `freeLanes[]`, `spawnable` and `checkpointDue` (`docs.mjs:1381-1416`, `:1432`). Nothing has ever consumed `freeLanes` because the orchestrator cannot run two workers and stay cheap.

**Decisions taken 2026-10-09:**

- **The scheduler is a loop over `status --json`.** Spawn `spawnable` workers for the first `spawnable` entries in `ready[]`, one per free lane, in the same working tree. Re-read the board after every worker exit. *(Accepted cost: lanes share a tree, so two slices that touch the same file race; keeping them apart is the slicer's job through `depends_on`, and `factory` adds no file-level check.)* Lanes above one are used only when `index.md` declares `gate_parallel: true`, the same ask `/deliver` §0 puts to the owner today. *(Corrected 2026-10-09 after adversarial review: an earlier draft claimed slice frontmatter marks `worker-safe`. No such key exists; slice keys are `slice`, `status`, `depends_on` (`docs.mjs:914`) plus `lane` (`:1380`). "Worker-safe" is a property of the gate, stored as `gate_parallel` in `index.md` (`docs.mjs:317`).)*
- **A block parks its lane, the run continues.** A worker's BLOCK-n marks the slice blocked on the board. `factory` keeps spawning slices whose dependencies do not include the blocked one. When nothing is spawnable and a block is open, the run enters `waiting-owner`, emits one event, and the cockpit pushes a notification. *(Rejected: stopping the whole run on the first block, as today. Owner's call, 2026-10-09: "park the lane and try continuing if parallel work is possible".)*
- **Answers arrive through `factory answer <BLOCK-n> "<text>"`.** `factory` writes the `**Resolution ({date}):**` line under the BLOCK entry as `autonomy.md:96` prescribes, sets the slice back to `building` (`autonomy.md:98`; slice statuses are `pending`, `building`, `blocked`, `done`, `docs.mjs:22`), and spawns a fresh worker whose pack carries the block and the answer. No worker is kept alive waiting. *(Corrected 2026-10-09 after adversarial review: an earlier draft cited `autonomy.md:71-110` and said the slice returns to `ready`; the file has 100 lines and the status is `building`.)*

**Acceptance**: fixture with three slices, the second blocks and the third is independent; the third is built while the block is open, the run reaches `waiting-owner`, `factory answer` resumes it and the second slice completes.

## 6. FR-4 — Checkpoints, gate and Prove

`checkpoint_every` defaults to 2 (`docs.mjs:318`). Today the checkpoint worker runs the gate itself and the orchestrator often runs it again to be sure.

**Decisions taken 2026-10-09:**

- **`factory` runs the gate; a worker never does at a checkpoint.** On `checkpointDue`, drain the lanes, run `docs gate` output as a command, record green/red and minutes. Green: spawn the checkpoint worker with the gate transcript attached and the covered QA-plan rows to Prove; it writes CP-n. Red: spawn a fix worker with the failing output; rerun the gate; three red runs block the checkpoint (cap from `autonomy.md`). *(Accepted cost: a gate that needs a running app or a browser still needs the worker to drive it; the gate command itself is still run by `factory`, the Prove is not.)*
- **"Do not pay for the same gate twice" is code, and it knows what changed.** A gate result is cached under the hash of the tracked tree *minus* the framework's own namespace (`docs/**`), plus the gate command string. A worker that finishes green, then writes its slice notes and log fragment, hands the driver a tree whose code hash is unchanged, so the checkpoint gate is served from the cache. Green and red are both cached; the cache lives in the run directory, so a resumed run reuses it. Only driver-run gates populate it: a worker's own runs are counted (FR-8) but never trusted as entries, because the driver cannot pin the tree they ran against. *(Owner's call at the cut 2026-10-09: a plain tree hash would re-run the gate after every notes edit. Accepted cost: the hash does not know the gate's inputs, so a README line still re-runs it; a stage-aware gate is left out, §19.)*
- **Slice Verify stays inside the worker.** The inner loop (`docs gate --inner`) runs where the edits are, as today. Only checkpoints and closeout move to the driver.

**Acceptance**: fixture scenario "red, red, green": the gate command runs exactly three times, two fix workers spawn, CP-1 is written once. Scenario "red ×3": the checkpoint blocks and no fourth gate runs.

## 7. FR-5 — Commits and the log

Today the builder commits its own slice by pathspec from inside the slice contract (`deliver/SKILL.md:122`) and the orchestrator only checks that a new commit exists (`:85`). Fragments are folded into `agent-log.md` with `docs log merge NNN` (`docs.mjs:2061`) at each checkpoint and on a block (`deliver/SKILL.md:79`, `:92`, `:145`), not after every slice. *(Corrected 2026-10-09 after adversarial review: an earlier draft said the orchestrator commits and merges after each slice. Both were wrong; see the lines above.)*

**Decisions taken 2026-10-09:**

- **Touched paths come from the stream.** Every `Edit`, `Write`, `NotebookEdit` tool_use target in the worker's stream, plus the slice file and the log fragment, is the pathspec. Paths the worker names in its report but did not edit through a tool are listed, not committed. *(Accepted cost: a file created through `Bash` is missed and shows up as untracked. `factory` lists untracked leftovers in the slice's log entry and in the final report; it never sweeps them in. A resume inside the run reads the board, not preflight (§3), so it does not care; the next new run on that repo is refused by preflight until the owner commits or removes them (`docs.mjs:1726-1732`), which is the right moment to look.)*
- **The driver commits, one commit per slice**, subject from the slice title, Conventional Commits, no trailers. The global `commit-msg` hook strips AI trailers anyway; `factory` adds none. *(Rejected: keeping the commit inside the worker, as today. It costs worker turns, the pathspec is the model's memory of what it touched, and a worker that falls back to `git add .` takes the other lane's half-written files with it.)*
- **Log merge at checkpoints and closeout, as today.** `docs log merge NNN` runs as a plain command when the lanes are drained, so no fragment is folded while a worker is still writing it; its output is logged, never read by a model. *(Accepted cost: `agent-log.md` lags the fragments by up to `checkpoint_every` slices, exactly as now.)*

**Acceptance**: two lanes in the fixture edit disjoint files; `git log --stat` shows each commit holding only its slice's paths, and `agent-log.md` holds both slices' entries after the checkpoint merge.

## 8. FR-6 — Polish, closeout, PR and CI

Today: `/polish` in Unattended mode (`polish/SKILL.md:23,94,129,166,183,199`), `docs ship`, `retro.md` by hand plus `measure.py --retro`, `gh pr create --draft --body-file`, `gh pr checks --watch --interval 60` in a background Bash, three CI fix rounds, one flaky rerun (`deliver/SKILL.md` §3–§6).

**Decisions taken 2026-10-09:**

- **Polish is one worker.** Full `/polish` Unattended as today, with the Cursor review inside it. *(Rejected: decomposing polish into driver-run steps. It is the largest token line after the build and the owner wants the whole loop first; decomposition is an idea for later.)*
- **Closeout splits by who can do it.** `factory` runs `docs ship NNN` and fills every numeric key of `retro.md` from the metrics store (FR-8). The polish worker writes the retro prose and the PR body in its last turn, before it exits; the driver then opens the PR. *(Cut 2026-10-09: an earlier draft spawned a separate closeout worker for the prose. No evidence asked for a clean context there, and every worker pays ≈24k tokens of system context before its pack (§20).)*
- **PR and CI are driver work.** `gh pr create --draft --body-file`, then `gh pr checks --watch --interval 60`. Red: a fix worker with the failing check's log, up to three rounds. One failed check with no diff since the last green: rerun once. Then block. The caps are the same numbers as `autonomy.md`; they move from prose to constants.
- **The PR is never merged, never marked ready without the owner.** Same as today.

**Acceptance**: fixture scenario with stubbed `gh`: polish worker runs, `docs ship` runs, `retro.md` has every numeric key filled, a draft PR call happens once, one red check spawns one fix worker, a flaky check is rerun once and only once.

## 9. FR-7 — Cockpit

The owner's workflow: start a run from Claude Code, monitor and unblock from the phone, review every decision after polish through `AskUserQuestion`, four per round (`deliver/SKILL.md` §8). That stays.

**Decisions taken 2026-10-09:**

- **Three layers.** `factory` (detached, zero tokens) → `claude -p` workers → one thin Claude Code "cockpit" session. The cockpit starts the run, waits on it, relays, and runs the closing review. It never reads the board or the log to decide anything.
- **`factory wait <run> [--timeout S]`** blocks until the next cockpit-relevant event (block; ask, which carries preflight `ask[]` items *(not `judge[]` — BLOCK-201, 2026-10-09)*; quota pause; cap reached; finished; failed) or prints `still running` with a one-line status at the timeout. The cockpit calls it in a background Bash with a long timeout; whether a background Bash can sleep for hours and wake the session is tested at slice entry, with a 25-minute poll as the fallback.
- **`factory answer <id> "<text>"`** closes a block or an ask. **`factory status <run>`** prints the board in ten lines. **`factory stop <run>`** ends it cleanly after the in-flight workers exit.
- **The cockpit skill lives in this repo** (`skills/factory-deliver/SKILL.md`), installed into Claude Code by the owner. It is under 60 lines: start, loop on `wait`, relay as `AskUserQuestion`, `PushNotification` on every event the owner must see, then the review interview exactly as `/deliver` §8 does it. *(Rejected: putting the review interview inside `factory`. It is judgment over the agent-log and belongs to a model with the owner on the other end.)*
- **A dead cockpit reattaches.** A new session calls `factory wait <run>` and continues; events it missed are replayed from `events.jsonl`.

**Acceptance**: a real run started from the laptop, the laptop locked, one block answered from the phone over Remote Control, the review interview completed on the phone. Human row in §16.

## 10. FR-8 — Metrics store

Today `measure.py --retro` reads the session JSONL after the fact and prints six of the twelve `retro.md` numeric keys: `build_minutes`, `polish_minutes`, `out_tokens`, `cache_read_tokens`, `start_context`, `peak_context` (`measure.py:167-168`). The model fills the other six from the log and the slices' Progress notes (`deliver/SKILL.md:178`). Weights for the headline figure come from `feature-docs/MAINTAINING.md:298`: a cache read counts 0.1 of an input token, an output token 5. *(Corrected 2026-10-09 after adversarial review: an earlier draft said `measure.py` prints all the keys.)*

**Decisions taken 2026-10-09:**

- **SQLite at `~/.local/share/factory/factory.db`.** Tables: `runs`, `workers` (role, slice, session_id, model, effort, input, cache_write, cache_read, output, thinking, cost_usd, num_turns, duration_ms, terminal_reason, outcome, peak_context), `gates` (source: driver or worker, checkpoint or slice, cmd, red, minutes), `events`. Driver runs are recorded directly; a worker's inner-loop runs are counted from its stream, as a `Bash` tool_use whose command contains the inner or gate command, red taken from the tool_result. Written as events happen, not reconstructed. *(Accepted cost: a worker that runs the gate through a wrapper script is missed. Today's `gate_runs` comes from the worker's own Progress notes (`deliver/SKILL.md:178`), which has the same hole the other way.)* *(Rejected: JSONL only. The learning loop and the A/B tables need queries; SQLite is one file and one crate.)*
- **`factory report <run>` prints the retro keys** (`gate_runs`, `gate_red`, `gate_minutes`, `ci_cycles`, `slices`, `checkpoints`, `build_minutes`, `polish_minutes`, `out_tokens`, `cache_read_tokens`, `start_context`, `peak_context`) plus the new ones: `input_tokens`, `cache_write_tokens`, `weighted_tokens`, `cockpit_tokens`, `workers`. The weighted figure uses the `MAINTAINING.md` weights so A/B numbers compare with the 42 existing retros.
- **A `tool_calls` table measures lookups** (FR-12, §14): per worker, `kind` (`ctx-def`, `ctx-refs`, `ctx-outline`, `bash-grep`, `bash-sed`, `bash-cat`, `read`, `run`, `edit`), `result_chars`, `target_hash` (normalised path or pattern), `repeat_in_worker`, `repeat_in_run`, and on `ctx-*` rows `followup_read_within_2` and `expanded_within_5` (§14). `factory report` adds `lookup_chars`, `ctx_share`, `repeat_rate`, `followup_reads` (count and chars: reads of a file a `ctx` answer named, within the next two tool calls) and `pack_doc_read_chars` (reads of files the pack carries, §4). The follow-up detector takes the path from Read, `cat` and `sed -n` alike. Baselines from the owner's transcripts: strict lookup share 59.5% in build workers, re-hit rate 26.7%, lookup result mean 4,519 chars, Read mean 12,373 chars (§20). *(Accepted cost: one more table written from the stream. A Bash call is classified by its first command, as the research did, so the numbers compare.)*
- **Cockpit tokens are measured from the cockpit session's JSONL** with a port of `measure.py`'s reading logic (`measure.py:144-158`), because the cockpit is the one layer `factory` cannot see from the inside. `cockpit_tokens` is a weighted figure, same weights, so the §13 ratio compares like with like.
- **`measure.py` is retired after FR-11 proves the numbers match**, within 2% on the same session JSONL.

**Acceptance**: after the first A/B run, `factory report` and `measure.py --retro` agree within 2% on `out_tokens` and `cache_read_tokens` for the cockpit session; every worker row has non-null usage.

## 11. FR-9 — Quota

Evidence: 2026-09-17, four runs used a week of quota in one day. The worker stream emits `rate_limit_event` (§20).

**Decisions taken 2026-10-09:**

- **Pause on rate limit.** A `rate_limit_event` that reports the limit reached, or a `result` error naming the usage limit, pauses every lane: in-flight workers finish or die, no new spawn. Resume at the reset time when the event carries one, else retry every 15 minutes. Attempts during a pause do not count against the three-attempt caps. The cockpit gets one event. *(Accepted cost: the exact field names of `rate_limit_event` are confirmed at slice entry, not here.)*
- **Per-run budget in weighted tokens.** `budget_weighted_tokens` in `~/.config/factory/config.toml`, overridable with `--budget`. Default 30M weighted, a guess: the largest of the last three retros is 18.9M weighted (§20). Past the budget the run finishes in-flight workers, parks, and asks; `factory run NNN --budget <n>` continues. *(Accepted cost: the default will be wrong for some feature sizes until retros replace it; the owner raises it in one command.)*

**Acceptance**: fixture scenario "rate limit": the stub emits the event, the run pauses, no spawn happens until the reset, then it continues. Scenario "budget 1": the run parks after the in-flight worker exits and `--budget` resumes it.

## 12. FR-10 — Stub harness

There is no harness; this FR is the decided prerequisite and the first slice.

**Decisions taken 2026-10-09:**

- **A fixture repo built by the tests.** A git repo on the v2 framework, created in a temp dir per test: `docs.mjs` vendored; `docs/index.md` declaring `gate: ./gate.sh`, `inner: ./gate.sh --inner`, `lanes: 2` and `gate_parallel: true`, so preflight has no ask to raise (`docs.mjs:1708`, `:1713`); one PRD with three slices, two independent and one dependent. `gate.sh` reads a scenario file and exits as the test scripts it (for example red, red, green), so gate outcomes belong to the test, not to the stub agent. *(Corrected 2026-10-09 after adversarial review: an earlier draft declared gate `true`, which can never go red, and no inner loop or lanes, which would have stopped the fixture on two preflight asks.)*
- **A stub `claude` and a stub `gh` on PATH.** The stub replays canned stream-json for the scenario under test: done, blocked, failed-then-fixed gate, rate limit, dead worker, red CI then green. The canned streams are cut from real recordings, including a real `result` event with usage fields.
- **`cargo test` runs the whole loop.** Start to draft PR, no network, under 60 seconds. *(Accepted cost: the stub cannot catch prompt problems; only the A/B runs can. The harness proves the loop, not the workers.)*

**Acceptance**: `cargo test` green on a clean checkout with no `claude` and no `gh` installed, in under 60 seconds.

## 13. FR-11 — A/B Prove

The target is tokens −40% end to end, orchestrator near zero, same PRD, same project. Counts are information; the owner judges quality.

**Decisions taken 2026-10-09:**

- **Two runs, sequential, baseline first: icg-cloud, then arca-api.** One PRD per project, written and sliced once with `/prd` and `/prd-to-tasks`. Two worktrees from the same base commit, neutral names (`wt-a`, `wt-b`). Run A: the current `/deliver`. Run B: `factory`. Then the second project. *(Rejected: parallel runs. They share the quota and the machine, and the comparison would carry the noise.)*
- **Same ruler for both.** Baseline tokens from `measure.py` over the deliver session and its subagents; `factory` tokens from the metrics store plus the cockpit session. Both weighted with the `MAINTAINING.md` weights. Wall-clock as build minutes from first spawn to draft PR.
- **Quality is the owner's call, through a fixed questionnaire**, answered once per run, before the numbers are shown:
  1. Does the result do what the PRD asked? (yes / partly / no)
  2. Would you merge this PR after your normal review, with at most small fixes? (yes / no)
  3. Decisions in the agent-log: how many would you overturn? (count)
  4. Code quality against the project's own standard (1–5).
  5. Tests: do they prove the FRs or only pass? (prove / pass / neither)
  6. Anything built that the PRD did not ask for? (list)
  7. Anything the PRD asked for that is missing? (list)
  8. Overall: ship as is / ship after fixes / redo (one).
  Pass means run B scores at least as well as run A on 1, 2 and 8, lists no more items than run A on 6 and 7, and is no worse by more than one point on 4.
- **Pass criteria for the PRD**: on both projects, `weighted_tokens(B) ≤ 0.6 × weighted_tokens(A)`, `cockpit_tokens(B) ≤ 5% of weighted_tokens(B)`, a green draft PR, and the questionnaire pass above. One project failing is a `BLOCK` for the owner, not a quiet retry.

**Acceptance**: the two comparison tables and the eight answers for each run, recorded in this feature's `retro.md`.

## 14. FR-12 — `factory ctx`

Today every worker lookup is Bash `grep`, `sed -n`, `cat`, `ls` or Read: 30,267 of 40,748 tool calls across 55 owner sessions (74% literal, 56% strict), and 136.8M of 148.6M tool-result characters (92%). Grep, Glob and LSP tool calls: zero; the bypass-mode system prompt tells the model to prefer the shell commands. File reads are 77% of lookup bytes, grep output 17%. Symbol questions (definition, callers, outline) are 23% of lookups by count and 8% by bytes. 26.7% of lookups re-hit a target the same worker already saw (36.5M chars). The common bundle is "grep the def, `sed` 30 lines, grep the callers", mean 3.5 commands per call. Sources and the full tables: [`ctx-research.md`](ctx-research.md) §2.

**Decisions taken 2026-10-09:**

- **Three verbs, plain text, exit 0 on empty.** `factory ctx def <symbol> [--kind class|fn|method|type] [--no-body]`, `factory ctx refs <symbol> [--in <dir>] [--max 10] [--per-file 3] [--tests]`, `factory ctx outline <path>... [--depth 1]`. Output is path:line, kind, signature, one or two context lines, then a trailer saying what was cut. `def` prints the body by default and `refs` stops at 10: in the owner's transcripts 86.9% of definition greps asked for the body (`-A`, median 25 lines) and 10 was the commonest cap on caller lists ([`ctx-jev-research.md`](ctx-jev-research.md)). *(Why context lines: bare locations tripled follow-up reads in the one LSP-versus-grep study, 15.2 against 3.2 per task. Why caps: capped, summarised search beat iterative search by 6 points in the SWE-agent ablation, and iterative search scored below having no search tool.)*
- **Engine: the ripgrep crates plus tree-sitter tags, no language server.** `def` runs a per-language definition regex through `grep-searcher`, `grep-regex` and `ignore`, then parses each candidate file with tree-sitter and keeps the hits inside a definition node of the right kind, outside comments and strings; the body, printed by default (`--no-body` for the location alone), is the node's exact range, replacing the "`sed` 30 lines" guess. `refs` is a word-boundary search with comments and strings dropped, collapsed per file. `outline` prints the grammar's `tags.scm` definitions with their signatures, the Agentless skeleton. Grammars: PHP 0.25.1, TypeScript, Go 0.25.0, Rust 0.24.2, all MIT with an upstream `tags.scm`; Vue has none, so `outline` locates the `<script>` block and parses it with the TypeScript grammar at offset. Vendor and gitignored paths are skipped unless `--vendor`. Every call re-reads the tree, so results are fresh after any edit with no daemon and no warm-up: rg-style search measured 7–44 ms warm on 59k–3.45M-line corpora, whole-tree tagging 0.47 s for 263 Rust files, 7 ms per file re-parse. *(Rejected: an LSP proxy. Typed `def` and `refs` cost 6–69 s cold and 0.3–3 GB per server, intelephense's licence names an individual with an editor, Vue needs a tsserver-forwarding client, and the one controlled study found LSP cost +6% tokens on Opus and +118% on Sonnet at equal success. Rejected: in-process ast-grep as the search engine, 0.54 s on 237k lines single-threaded against 13–21 ms for the grep crates. Accepted cost: callers through Laravel facades, Eloquent magic, scopes and route strings stay textual; a Rust function made by a macro is found by regex only.)*
- **Workers learn it from the pack, backed by a hint.** The pack (FR-2) gains an "Outline" section, the `outline` of each recon file and the `def` of each cited symbol, aimed at the first read of every recon file and at the 39.7% of Reads that repeat a file across the run; and a "Lookup" section: three verbs, three examples, one rule: grep only for literal strings and log output. The guard hook (FR-2) also answers `grep -r` and `rg` over the repo with a one-line hint naming the verb. *(Accepted cost: uptake is unproven. The built-in bypass instruction still says cat, grep, sed, and whether the appended pack overrides it is unknown. With free choice, models picked semantic lookup 0–6% of the time for localisation in the one study that measured it. FR-8 measures the uptake; the reversal rule is next.)*
- **Measured, with a reversal rule.** The FR-8 `tool_calls` table records every lookup's kind, bytes and repeat flags, and for each `ctx` call whether the worker read a named file within the next two tool calls (`followup_read_within_2`) or widened the window within five (`expanded_within_5`); `factory report` prints `lookup_chars`, `ctx_share`, `repeat_rate` and `followup_reads`. `followup_reads` above 30% of `ctx` calls after slice 3 means the answers are too thin: widen the context lines or the body before anything else. If, after slice 3, `ctx_share` stays under 10% of worker lookups or `repeat_rate` does not fall from 26.7%, `ctx` shrinks to `outline` only and the pack lever is pushed instead. A typed layer (LSP, SCIP) gets its own PRD only if the data then shows `def` and `refs` still heavy; the recon for it is in `ctx-research.md` §3.
- **Three slices**: (1) grep-crate walker, per-language definition regexes, compact capped printer; (2) tree-sitter tags for the four grammars, `outline`, the kind filter, the Vue script shim; (3) pack sections, the two hook hints (lookup verb, pack doc), `tool_calls` table with the follow-up labels, and report keys. Slices 1 and 2 touch no driver code and can run beside FR-1/FR-2; slice 3 waits on FR-2 and FR-8.

**Acceptance**: fixture files for PHP, Vue, Go and Rust under `cargo test` pin each verb's output: `def` returns exactly the definition with kind and signature and drops the docblock mention; `refs` on a symbol with 300 planted hits prints at most `--max` lines plus a trailer; `outline` prints every top-level definition with its signature and nothing else. On the icg-cloud and arca-api checkouts each verb answers in under 1 s warm. After the first A/B run, `factory report` prints `ctx_share` and `repeat_rate` for every worker.

## 15. Decisions log *(16 settled 2026-10-09, 0 open)*

- ~~**OPEN-1** — name?~~ **Decided 2026-10-09: `factory`.**
- ~~**OPEN-2** — driver first, or port the docs tool first?~~ **Decided 2026-10-09: driver first; the port is PRD-002.** The driver calls `docs.mjs` as a subprocess. §3
- ~~**OPEN-3** — who builds, who reviews?~~ **Decided 2026-10-09: Claude builds, Cursor reviews.** §4
- ~~**OPEN-4** — docs format?~~ **Decided 2026-10-09: keep v2, extend only.** v3 ideas recorded in `ideas.md`.
- ~~**OPEN-5** — on a block, stop or continue?~~ **Decided 2026-10-09: park the lane, continue independent work.** §5
- ~~**OPEN-6** — the target?~~ **Decided 2026-10-09: tokens −40% end to end, orchestrator ≈0, green draft PR.** §13
- ~~**OPEN-7** — loop scope?~~ **Decided 2026-10-09: full loop, polish as one worker.** §8
- ~~**OPEN-8** — proof?~~ **Decided 2026-10-09: stub gate plus A/B Prove.** §12, §13
- ~~**OPEN-9** — who judges quality?~~ **Decided 2026-10-09: the owner, through the fixed questionnaire; counts are informational.** §13
- ~~**OPEN-10** — how many A/B runs, in what order?~~ **Decided 2026-10-09: two, icg-cloud then arca-api, sequential, baseline first.** §13
- ~~**OPEN-11** — quota?~~ **Decided 2026-10-09: pause on rate limit plus a per-run budget.** §11
- ~~**OPEN-12** — where do config and metrics live?~~ **Decided 2026-10-09: `~/.config/factory/`, `~/.local/share/factory/`.** §10, §11
- ~~**OPEN-13** — worker permissions?~~ **Decided 2026-10-09: bypass plus deny rules plus guard hook, as today.** §4 *(Corrected 2026-10-09 at slicing: the hook is new, not "as today"; see §4.)*
- ~~**OPEN-14** — where does the cockpit skill live?~~ **Decided 2026-10-09: in this repo.** §9
- ~~**OPEN-15** — retire `measure.py`?~~ **Decided 2026-10-09: after FR-11 shows the numbers match.** §10
- ~~**OPEN-16** — `factory ctx`, a worker-side lookup command (`def`, `refs`, `outline`): leave out, ripgrep-based, tree-sitter tags, LSP proxy, or hybrid?~~ **Decided 2026-10-09: thin hybrid, the ripgrep crates plus tree-sitter tags, as FR-12; no language server in this PRD.** A research workflow over 55 owner sessions and four fact-checked surveys closed it ([`ctx-research.md`](ctx-research.md)). The "evidence against" held for docs reads (29% of read bytes, pack material) and not for code reads. *(Corrected 2026-10-09: an earlier draft said lookups were 44% of worker tool calls, method unrecorded; the transcripts give 56% strict and 74% literal. FR-11 baselines on the strict figure.)* §14

## 16. QA plan

**Gate**: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` *(declared in `index.md` 2026-10-09 — `docs gate` prints it; the first slice creates the crate it runs against)*
**Inner**: `cargo test`
**Surface**: CLI (`factory run|wait|answer|status|stop|report`); a Claude Code cockpit session on the phone over Remote Control; a GitHub draft PR.
**Harness**: none — FR-10 builds it as the first slice: fixture repo plus stub `claude` and `gh`. Cost: one slice.
**Seed state**: the fixture repo, generated per test.

| FR | Proof on the real surface | Evidence | Human? |
|---|---|---|---|
| FR-1 | run the fixture, kill the driver mid-slice, `factory run` again → continues from the board, no slice built twice; refused preflight exits 1 with no spawn | test transcript | no |
| FR-2 | run the fixture → the stub receives a pack holding exactly the slice, its cited sections and the contract; one worker row with all usage fields | test + pack snapshot | no |
| FR-3 | run the fixture "slice 2 blocks" → slice 3 builds, run waits, `factory answer` resumes, slice 2 completes | `events.jsonl` | no |
| FR-4 | run the fixture "red, red, green" → gate runs 3 times, 2 fix workers, one CP-1; "red ×3" → blocked, no 4th gate | test | no |
| FR-5 | run the fixture with two lanes → `git log --stat` shows one commit per slice with only its paths; merged log holds both | `git log --stat` | no |
| FR-6 | run the fixture closeout → `docs ship`, retro keys filled, one draft PR call, one fix worker on red CI, one flaky rerun | stubbed `gh` call log | no |
| FR-7 | start a real run on the laptop, lock it, answer a block from the phone, finish the review interview on the phone | screenshots | yes → `qa-guide.md` |
| FR-8 | after the first A/B run, `factory report` vs `measure.py --retro` on the cockpit session | comparison table | no |
| FR-9 | run the fixture "rate limit" → pause, no spawn, resume at reset; "budget 1" → parks, `--budget` resumes | `events.jsonl` | no |
| FR-10 | `cargo test` on a clean checkout, no `claude`, no `gh` → green in under 60 s | CI run | no |
| FR-11 | the two A/B runs, questionnaire answered before numbers are shown | tables + answers in `retro.md` | yes → `qa-guide.md` |
| FR-12 | fixture files per stack → each verb's output pinned; `def`, `refs`, `outline` under 1 s warm on icg-cloud and arca-api; after the A/B run `ctx_share` and `repeat_rate` print per worker | test + timing table in slice Progress + `factory report` | no |

## 17. Slices

Sliced 2026-10-09 into 13 slices; the table below is generated by `docs slice` and `docs render`, the prose here is the argument. **The harness is FR-10 itself**, not a `/verify-app` skill: slice 01 builds the crate, the fixture repo and the stubs, and every later slice adds the stub scenario it consumes, so `cargo test` grows with the loop and no slice waits on a harness it does not need. Slices are named by what they make true; the FR each one builds is in its header.

**Spine.** 01 → 02 → … → 10 is one serial chain: every driver slice edits the same scheduler loop, so each depends on the one before it. **Branch.** 11 → 12 (`factory ctx` search, then tags) touch no driver code and hang off 01 alone, so a second lane builds them beside the spine. 13 joins both at the end. Checkpoints fall every two done slices (`checkpoint_every` 2): after 02, 04, 06, 08, 10 and 12, and once more at 13. The schema, the pack and the scheduler land right before a checkpoint on purpose.

**Where the order differs from the note the slicer was given, and why.** (1) FR-3 lanes (04) land before FR-5 commits (05): FR-5's acceptance is "two lanes edit disjoint files", unprovable until lanes exist. (2) FR-11 has no slice. The A/B needs the polished binary and the owner, and a slice no worker may build would park every `/deliver` run on it. It is the post-ship owner measurement: `docs ship` at closeout, the two runs, results into `retro.md`, `docs archive` when it passes; the procedure is in [`qa-guide.md`](qa-guide.md) next to FR-7's phone run. (3) FR-12's `tool_calls` table is created in 08 with the rest of FR-8, where `repeat_rate` and `lookup_chars` already need it; 13 adds the `ctx-*` kinds, the two follow-up labels and the report keys that depend on them. (4) FR-10's scenarios are spread: 01 ships the mechanism and `done`; `dead` comes with 03, `blocked` with 04, `red, red, green` with 06, `rate limit` with 07, `red CI then green` with 09.

**The highest-risk slice is 03**, one worker end to end: the `claude -p` flags, the stream shape, the detach model and resume all meet there, and three of them are marked "verify at slice entry" in §20. Its slice file says so at the top.

**Workflow.** One branch (`feat/driver`), one PR, one commit per slice by pathspec, a checkpoint commit after every second slice with the full gate and the Prove rows, `/polish` then `docs ship` in the last commit. No row of §16 is Human except FR-7 and FR-11, so `qa-guide.md` holds exactly those two.

**A fact corrected at slicing (2026-10-09).** FR-2 said "bypass plus deny rules plus the guard hook, as today". No Claude Code hook exists today; the correction is inline in §4 and OPEN-13, and the hook is slice 13's work.

<!-- docs:slices -->
| Slice | Status | Depends on |
|---|---|---|
| [01 — Harness: crate, fixture repo, stub claude and gh (FR-10)](slice-01-harness.md) | ✅ done | — |
| [02 — Contract and state: docs.mjs wrapper, run dir, config, schema (FR-1, FR-8)](slice-02-contract-and-state.md) | ✅ done | 01 |
| [03 — One worker end to end (FR-1, FR-2)](slice-03-one-worker.md) | ✅ done | 02 |
| [04 — Lanes and parking (FR-3)](slice-04-lanes-and-parking.md) | ✅ done | 03 |
| [05 — Commits by pathspec and log merge (FR-5)](slice-05-commits-and-log.md) | ✅ done | 04 |
| [06 — Checkpoints: driver-run gate, cache, fix workers (FR-4)](slice-06-checkpoints.md) | ✅ done | 05 |
| [07 — Quota: rate-limit pause and weighted budget (FR-9)](slice-07-quota.md) | ✅ done | 06 |
| [08 — Metrics: tool_calls, report, cockpit tokens (FR-8)](slice-08-metrics.md) | ✅ done | 07 |
| [09 — Closeout: polish worker, ship, retro keys, PR, CI (FR-6)](slice-09-closeout.md) | pending | 08 |
| [10 — Cockpit: wait, replay, status, skill file (FR-7)](slice-10-cockpit.md) | pending | 09 |
| [11 — ctx search: grep crates, definition regexes, printer (FR-12 slice 1)](slice-11-ctx-search.md) | pending | 01 |
| [12 — ctx tags: tree-sitter, outline, kind filter, Vue shim (FR-12 slice 2)](slice-12-ctx-tags.md) | pending | 11 |
| [13 — ctx in the loop: pack sections, hook hints, follow-up labels (FR-12 slice 3, FR-2 pack-doc hint, FR-8 tail)](slice-13-ctx-in-the-loop.md) | pending | 10, 12 |
<!-- /docs:slices -->

## 18. Sequencing risk

None inside this repo. For FR-11: the A/B PRD in icg-cloud must not touch files owned by 017-arca-setup (in progress, `feat/arca-setup`), and in arca-api not files owned by 020 (in progress). Pick the PRD after both have shipped, or pick one in a disjoint area, and record the choice in that project's `index.md` build order.

## 19. Non-goals

- **Porting `docs.mjs` to Rust, or any docs format change** — PRD-002, recorded in `ideas.md`. `factory` shells out to `docs.mjs`.
- **The learning loop** (retro ranking, verdicts, promotion between scopes) — its own PRD. FR-8 only makes sure the data exists.
- **Stack plugins, `factory init`, rendered project skills, project profiles** — its own PRD. Models and effort come from `~/.config/factory/config.toml` defaults for now.
- **Replacing `/prd` and `/prd-to-tasks`** — they stay interactive Claude Code skills.
- **Codex or Agent SDK workers, API-key billing** — no evidence they are cheaper; the subscription path is verified and used.
- **Sharing, packaging, a release** — planned later; nothing here may contain owner-specific facts outside `config.toml`.
- **Decomposing polish into driver steps** — idea for after the first A/B.
- **A typed lookup layer (LSP, SCIP) for `factory ctx`** — its own PRD, opened only if FR-8 shows `def` and `refs` still heavy after FR-12 (§14). Recon kept in [`ctx-research.md`](ctx-research.md) and a line in `ideas.md`.
- **A hosted decision model (TypeSafe Jev) inside `factory ctx` or the driver** — assessed 2026-10-09 by a research workflow and three replays on the owner's transcripts: `def` has nothing to disambiguate (91.2% single hit), Jev-scored read compaction missed the edited lines in 30% of cases while keeping 64% of the bytes, and block auto-answer lost to "pick the recommended option" (85.0% against 88.2%). Parked in `ideas.md`; memo [`ctx-jev-research.md`](ctx-jev-research.md).
- **Not handled: `--resume` of a dead worker's session.** Accepted 2026-10-09: today's behaviour (fresh worker plus a log note) has been enough; a resumed session would carry the state that killed it. Revisit if dead workers show up in the metrics more than once a run.
- **Not handled: two runs on the same repo at once.** Accepted 2026-10-09: never needed; `factory run` refuses when a run is live for that repo.
- **Not handled: macOS or Windows.** Accepted 2026-10-09: the owner's machines are Linux; nothing is designed against it, nothing is tested on it.
- **Not handled: the closing review interview inside `factory`.** Accepted 2026-10-09: judgment over the agent-log with the owner present belongs in the cockpit (§9).
- **Not handled: cross-repo features.** Accepted 2026-10-09: never seen.
- **Not handled: a stage-aware gate** that re-runs only the fmt, lint or test stage whose inputs changed. Accepted 2026-10-09 at the cut: the gate is one declared command and the driver does not know its inputs; the path-aware cache (§6) covers the case seen, notes edited after a green gate. Revisit if FR-8 shows `gate_minutes` dominating a run.
- **Not handled: a separate closeout worker.** Cut 2026-10-09: the polish worker writes the retro prose and the PR body (§8). Revisit if that prose degrades in the owner's review.
- **Not handled: a tags cache or `factory ctx index`.** Accepted 2026-10-09: app trees tag in under 0.5 s and vendor is skipped, so every call re-reads the tree. Revisit if FR-8 shows `ctx` calls above 1 s.
- **Not handled: callers through magic dispatch** (Laravel facades, Eloquent `__call`, scopes, route strings). Accepted 2026-10-09: textual `refs`, as grep gives today; resolving them is the typed layer's job.
- **Not handled: a seen-before stub for repeated tool results.** Cut 2026-10-09 at the Jev cut: the owner's transcripts show 239 whole re-reads of persisted oversized tool outputs, 9.77M chars, but a "seen, unchanged" stub can hide a read the worker needed and nothing local can shrink those files. Revisit if `repeat_rate` (§10) does not fall after FR-12 slice 3.

## 20. Verification notes

Checked against the current skills repo and the toolchain 2026-10-09. **Re-verify at slice entry.**

- **Preflight routing**: `~/Dropbox/dev/claude-code-skills/deliver/SKILL.md:15-68` — §0 reads `preflight --json` and routes on `pass`, `ask[]`, `judge[]`, `refuse[]`; resume table and dead-builder handling are prose the model re-derives.
- **Build loop and contracts**: `deliver/SKILL.md:75-164` — orchestrator only spawns, checks and commits; the slice contract and the checkpoint contract are pasted per worker; AD numbering `{NN}01`; "do not pay for the same gate twice" is a rule, not code.
- **Caps and blocker protocol**: `feature-docs/autonomy.md:71-110` — 3 gate attempts, 3 CI rounds, 1 flaky rerun, blocker protocol steps 1–5, owner answers mid-run, resume.
- **Board JSON**: `feature-docs/bin/docs.mjs:318` default `checkpoint_every` 2; `:1381-1416` `freeLanes`, `spawnable`, `checkpointDue`; `:1432` `checkpointDueFor`; commands at `:2055` (`status`), `:2061` (`log`), `:2066` (`preflight`), `:2081` (`gate`), `:2084` (`ship`). File is 2,102 lines.
- **`status --json` keys**: `number, slug, dir, kind, status, reviewed, gate{declared,status,detail,candidates}, qaPlan, harnessLine, harness, rubric, sliced, allDone, slices[], checkpointEvery, checkpoints[], unverified[], checkpointDue, next, ready[], building[], lanes, freeLanes[], spawnable, blocked[], openBlocks[], openOpens[], log{decisions,flagged,decide,blocks}, retro, branch`. **`preflight --json` keys**: `feature, pass, refuse[], ask[], judge[], ok[], branch{current,default,feature,alt,featureExists}`; exit 1 refused, 2 asks pending.
- **Retro numbers**: `deliver/bin/measure.py --retro` prints the six token and time keys; the six count keys are read off disk by the model (`deliver/SKILL.md:178`). Weights in `feature-docs/MAINTAINING.md:298`: cache read 0.1, output 5. Last three retros by file date:

  | retro | slices | build min | out tokens | cache read | peak ctx | weighted |
  |---|---|---|---|---|---|---|
  | icg-cloud 017 † | 12 | 254 | 1,080,973 | 134,911,332 | 167,040 | 18.9M |
  | nycase 008 | 5 | 72 | 567,030 | 73,173,357 | 166,439 | 10.2M |
  | arca-api 021 | 7 | 151 | 375,659 | 41,561,609 | 166,717 | 6.0M |

  † The 017 retro frontmatter was written for the first 12 slices. The feature grew to 21 slice files and shipped 2026-10-09 without the numbers being refreshed, so its row covers those 12 slices only. *(Corrected 2026-10-09: an earlier draft rounded nycase to 10.1M; 7,317,336 + 2,835,150 = 10,152,486.)*

- **Lanes never used**: no project under `~/Projects` has `docs/reference/lanes`; `lanes` is 1 everywhere. 12 projects on the framework.
- **Polish Unattended mode**: `polish/SKILL.md:23,94,129,166,183,199`.
- **Second opinion**: `feature-docs/models.md` — `cursor-agent -p --trust --mode ask --output-format stream-json --model grok-4.7-high`, 900 s timeout, empty `.result` means unreviewed.
- **Headless Claude Code 2.1.280**: `claude -p --help` lists `--permission-mode {acceptEdits,auto,bypassPermissions,manual,dontAsk,plan}`, `--allowedTools`, `--disallowedTools`, `--max-budget-usd`, `--model`, `--effort`, `--resume`, `--session-id`, `--append-system-prompt`, `--agents`, `--add-dir`, `--no-session-persistence`, `--output-format stream-json` (needs `--verbose`). `--append-system-prompt-file` appears only in passing in the `--bare` description as `--append-system-prompt[-file]`; confirm at slice entry, else the pack goes inline through `--append-system-prompt`. *(Confirmed 2026-10-09 at slice 03 entry: `--append-system-prompt-file <path>` is accepted by 2.1.280 — `recordings/claude-p-append-file-haiku-2.1.280.jsonl`; the pack goes by file.)* *(Corrected 2026-10-09: an earlier draft listed `--max-turns`; it is not in the 2.1.280 help.)* The `result` event carries `session_id, usage{input_tokens,cache_creation_input_tokens,cache_read_input_tokens,output_tokens,output_tokens_details.thinking_tokens}, modelUsage, total_cost_usd, num_turns, duration_ms, permission_denials, terminal_reason, result`. Stream event types seen in the probe: `system`, `assistant`, `user`, `rate_limit_event`, `result` (18 events, scratchpad `probe/stream.jsonl`). Every worker starts with ≈24k tokens of Claude Code system context before its pack: a one-word, one-turn Haiku call on 2026-10-09 read 17,693 cache tokens and wrote 6,280 (`scratchpad/probe/haiku-probe.json`, list price $0.0146). *(Corrected 2026-10-09: an earlier draft called the 24k a cache write; most of it is a cache read when the prefix is warm.)*
- **Billing path**: no `ANTHROPIC_API_KEY` in the environment; `claude -p` runs on the subscription login. `--bare` would read only an API key, so it is not used.
- **Dogfood boards**: icg-cloud on `feat/arca-setup`, 017 shipped 2026-10-09 with 21 slices, pending owner review, next PRD 018, gate `composer test`. arca-api on `feat/certificate-lifecycle`, 020 in progress, 017/018/019/021 shipped, gate `make verify`.
- **Toolchain**: cargo and rustc 1.98.1; `gh auth status` OK; `cursor-agent` installed.
- **Lookup habits** (research workflow 2026-10-09, [`ctx-research.md`](ctx-research.md)): 55 main sessions and 563 subagent transcripts under `~/.claude/projects/` for icg-cloud and arca-api, Claude Code 2.1.259–2.1.280, all in bypass mode. 40,748 tool calls; lookups 30,267 literal (74.3%) and 22,846 strict (56.1%), 59.5% strict in build and verify workers. By kind: Bash grep 7,745, `sed -n` 6,456, `cat` 3,276, Read 2,911, `ls` 1,256; Grep, Glob and LSP tools 0. Tool-result chars 148.6M, lookups 136.8M (92.1%); reads 77% of lookup chars, grep output 17%; results over 10k chars are 12.2% of lookups and 53.4% of bytes. Re-hits: 26.7% of lookups in the same worker (36.5M chars), 37.6% across the run; 29% of read bytes are prd, slice or SKILL.md files. *(Corrected 2026-10-09: an earlier draft's 44% has no recorded method.)*
- **`ctx` engine facts**: grep 0.4.1, grep-printer 0.3.1, grep-searcher 0.1.17, grep-regex 0.1.14, grep-matcher 0.1.9, ignore 0.4.33, all Unlicense OR MIT; a 60-line binary built in 6.2 s, 3.0 MB stripped, 7–44 ms warm per definition regex on 59k–3.45M-line corpora. tree-sitter 0.27.1 and tree-sitter-tags 0.27.1, MIT; grammars PHP 0.25.1, Go 0.25.0, Rust 0.24.2 and TypeScript, all MIT with `tags.scm`; the helix tree (263 files) tagged in 467 ms, a 1,816-line file re-parsed in 7 ms. tree-sitter-vue has no `tags.scm` and is not on crates.io; `tree-sitter-vue-sqry` 33.0.1 (MIT) exists, `tags.scm` unknown. Definition regexes: Go 255 = AST; PHP methods 2 false positives in 136; Rust finds 185 macro-made functions the AST misses. Cold page cache latency not measured.
- **Prior art used**: SWE-agent ACI ablation (arXiv 2405.15793): summarised capped search 18.0% against iterative 12.0% and no search 15.7%; the paper says a 50-result cap, the shipped code caps at 100. Agentless (arXiv 2407.01489): skeleton localisation 58.33% at $0.02 against full file 53.67% at $0.15 *(corrected 2026-10-09 by the Jev research: these are "contains ground truth" localisation coverage figures, not resolve rates; arXiv 2607.09691, Claude Sonnet 4.6, N=70, found keep/drop pruning against the full file not significant at p=0.07 and disclaims a deterministic-beats-model reading)*. LSP-versus-grep (arXiv 2608.13568, 6 tasks, Python and TypeScript): tokens +6% Opus, +118% Sonnet, −26% Haiku at equal success; refs +19% tokens for +0.07 F1; bare locations 15.2 follow-up reads against 3.2 with two context lines. No study covers Claude on PHP, Vue, Go or Rust.
- **Jev replays** (2026-10-09, the owner's key, transcript text sent with the owner's consent): latency from this machine median 257–315 ms, p90 under 450 ms; block auto-answer 85.0% right against the 88.2% "pick the recommended option" baseline over 472 questions, 1 of 51 owner deviations caught; read compaction by Jev-scored windows kept 63.7% of bytes and covered the edited lines in 70.0% of 120 read-then-edit cases; a named class or function line was found with confidence 1.00 on a public file. Graft evidence from the same mining: 86.9% of definition greps carry `-A` (median 25 lines) and the commonest cap on caller lists is 10; 631 reads of pack-held docs, 10.6M chars, 40.3% of whole-file read bytes; 239 whole re-reads of persisted tool outputs, 9.77M chars. Scripts in [`ctx-jev-replay/`](ctx-jev-replay/), memo [`ctx-jev-research.md`](ctx-jev-research.md).
- **Unverified, to check at slice entry**: deny rules under `bypassPermissions`; slash commands inside `-p`; background Bash sleeping for hours in the cockpit; the field shape of `rate_limit_event` and of a usage-limit `result` error; whether the appended pack overrides the built-in cat/grep/sed instruction; whether `tree-sitter-vue-sqry` ships a `tags.scm`; which model workers run, since the LSP token effect flips by model. *(Checked 2026-10-09 at slice 03 entry, Haiku on 2.1.280: deny rules hold under `bypassPermissions` — a `--settings '{"permissions":{"deny":["Bash(git status:*)"]}}'` denial lands in `permission_denials[0]{tool_name,tool_use_id,tool_input}` even against a user-settings allow, `recordings/claude-p-deny-settings-bypass-haiku-2.1.280.jsonl`; the literal `git push --force --dry-run` probe was inconclusive, the model asked instead of calling the tool, `recordings/claude-p-deny-forcepush-haiku-2.1.280.jsonl`. Slash commands resolve inside `-p`: `init.slash_commands` lists 96 entries including `polish`, `recordings/claude-p-append-file-haiku-2.1.280.jsonl`, so a pack need not carry a skill body. Still unverified: background Bash, the `rate_limit_event` shape, the cat/grep/sed override, `tags.scm`, the worker model's LSP effect.)* *(Slice 10, 2026-10-10: background Bash was not tested at entry — the slice was built by an unattended builder, which cannot drive a Claude Code session. The cockpit skill uses the 25-minute poll (`factory wait <id> --timeout 1500`), which works whichever way the answer goes; the live check is FR-7 step 2 of [`qa-guide.md`](qa-guide.md). `log-slice-10.md` AD-1007.)*
