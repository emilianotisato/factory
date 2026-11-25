# Retro — PRD-027

Written by `/deliver` in the closeout commit, from the agent's side of the run.

**The point is cost next to catch.** An agent asked only about friction votes to cut every check, because every check costs it time. A verdict with an empty Caught column is a preference, not evidence.

## Cost vs catch

| Step | Cost | Caught | Verdict |
|---|---|---|---|
| Inner loop, per slice | 6 runs, about 10 min in all | slice 04: the per-worker line pinned in `tests/metrics.rs` (AD-11). Slice 01's red was the known `discover_models` flake, not the slice | keep |
| Checkpoints (full gate) | 2 runs, 5 min | nothing | keep: cheap, and the only full-gate runs before polish |
| Prove, at checkpoints | 12 test rows, 2 mutation checks | nothing new. Both mutations went red as they should. But polish showed two Prove tests were weaker than they looked: `non_claude_worker_skips_claude_subagent_lookup_without_warning`'s integration twin asserted an absence that a wrong build also gives, and the mixed run never spawned a fix or merge worker | tune: a Prove row whose assertion is "nothing happened" needs a mutation that makes it happen |
| Polish — local review | 1 forked review | 6 real defects: killed workers read as finished, `--recount` wiping non-Claude metrics, the worker answer skipping the CLI check, a legacy model overwritten, a recovered Codex retry left errored, `codex:high` read as a model | keep |
| Polish — security pass | 1 subagent, about 4 min | 2 medium: git's own child processes and an interactive shell both get past the PATH shim. Both parked as an idea (a real fix changes how dec-2 is enforced) | keep |
| Polish — UX pass | skipped | the only surface is CLI output, there is no `verify-*` harness and no rubric; every FR is proven by tests | — |
| Polish — second opinion (grok-4.7-high) | 10 passes, 8501 s of pass time | most of the stream bugs: a `duration_ms` of 429 and `lib.rs:429:10` read as rate limits, a failure losing to earlier narration, Agy's reset time dropped, OpenCode's input update ignored, live reading an earlier `STATUS:` as the outcome, a pre-upgrade run running Claude with a Codex model. It agreed independently with the local review on 3 findings. About a third of its claims were stub-fidelity or latent and got no fix | keep |
| prune | 1 pass | 1 duplicate polish test merged into its existing neighbour | keep |
| Verifying a reviewer's claim by experiment | 1 full suite run with a push-blocking `git` first on `PATH` (about 2 min) | rebutted the local review's "the shim breaks this repo's own tests inside a worker", which would otherwise have been a harness change | keep |

## Where I guessed

- **`prd.md` §3, "Persist resolved run workers"**: when `worker_config` is absent, the PRD says to fall back to "`Provider::Claude` + loaded `config.workers` (model/effort)". Taken literally, that feeds a Codex slug to `claude` once a role was moved in config. I followed AD-3's provider-scoped rule instead (AD-14).
- **Real terminal events of `codex exec --json` and `opencode run --format json`**: neither CLI was run live. I guessed that a Codex `error` followed by `turn.completed` is a recovered retry, and that an OpenCode `step_finish` with `reason: "tool-calls"` is not the end of the run (AD-12). The stub fixtures cannot confirm either.
- **How much of a provider-less role table survives a provider switch**: AD-3 and AD-6 disagree here, and no FR settles it. Parked as an idea, not decided.

## What I wished existed

- Real JSONL recordings from one live run of each CLI. Every non-Claude stream in the tests is the Claude stub's output translated by `provider-worker.js`, so a test proves the translator agrees with the parser, not that the parser reads the CLI.
- A `--title-file` on `factory docs log add`: one AD title lost its backtick examples to shell substitution and had to be superseded (AD-15 → AD-16).

## Project friction → `/gate audit`

- `claude plugin validate mods/cockpit` prints a page of TypeScript `TS2345` errors from its directory checks on every gate run, while still exiting 0. The noise sits next to real failures in every gate log. Either fix the cockpit types or pass a profile that stays quiet.
- `provider::tests::discover_models_parses_live_outputs_and_kills_hung_probe` flaked red in slice 01's inner loop (an ETXTBSY-style race, per the log's Notes). It costs a rerun whenever it fires.

## Framework friction → factory

- **The worker `PreToolUse` hook (`src/cli/hook.rs`, dec-2 enforcement)** refuses any Bash command whose text contains a push-like token. That includes a Python heredoc that edits Rust (`refused.push(`) and a `factory docs idea add` body describing the bypass. Two calls were refused, and both had to go through files. The hook should judge the command a shell would run, not heredoc contents.
- **`/polish` §2 with a 10-pass second opinion**: about 40 claims arrive as prose, one section per pass, with no ids or severities. Triaging them in one context is the most expensive step of the ritual. A collect that emitted one line per claim (file:line, severity, one sentence) would make dedupe mechanical.
- **`factory docs log add` run from a lane writes the main checkout's `log.jsonl`** (`/home/emiliano/Projects/factory/docs/…`). That is right for the driver, but nothing says so at the call site, and a worker that expects its lane to change sees no diff.

A comment or name the leaks check should have caught: none. The one wrong comment (`src/worker.rs`, "the `git` shim covers that one") was a false security claim, not a plan reference, and the reviewers caught it.

## Score

4/5. Every FR shipped with green proofs, but polish found five high-severity defects the checkpoints had passed, so the Prove rows mostly exercised the happy path.
