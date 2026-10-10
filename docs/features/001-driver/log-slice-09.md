## Traps

- `docs ship NNN` refuses with exit **2** (`refusing: N slice(s) not done (02). Set them done first…`), not 1; a block body or a test that spells `(exit 1)` is wrong on the first red.
- A worker pack with no files has the header `<!-- factory-pack files: -->` (one space, from `files.join(",")` on an empty list); the CI fix pack is one, and a test spelling two spaces fails.
- `cargo test` stops at the first failing test *binary*: a change every finished run feels (here: the polish worker and two closeout commits at the end of each fixture run) surfaces one binary's failures per run. `cargo test --no-fail-fast` shows them all in one go — three runs here were spent discovering the next binary's shifts.
- The stub `claude` claims `calls/N.json` empty (`openSync … 'wx'`) and writes the JSON a moment later; `Fixture::calls()` polled from a `wait_until` closure panicked on the empty file (`EOF while parsing`) once in seven runs. `calls()` now skips empty files; do not read the calls dir any other way from a poll.
- A finished fixture run now ends with a `polish` worker and the commits `polish: review fixes` and `docs: ship PRD-001`: `fx.calls().len()`, `workers.len()`, `subjects()` and anything reading `HEAD` as the checkpoint commit must count them (`sha_of("docs: checkpoint 1")` instead of `HEAD`).

## Blockers

## Decisions

### AD-901 — What id does a closeout block get? · slice-09 · 2026-10-10

**Chose:** `BLOCK-n` with the lowest free number across `agent-log.md` and the fragments (`checkpoint::lowest_free_block`, slice 06), the phase in the heading — `### BLOCK-n — {title} · polish|pr|ci · open` — and in the event (`closeout_blocked {phase, block_id, question}`). Task 4's "`BLOCK-<NNN>-ci`" is not used.
**Over:** a second id shape for CI (`BLOCK-001-ci`).
**Because:** `factory answer BLOCK-n` (slice 04) and `docs log merge` parse one id shape; a second would need a second parser and a second numbering, and the phase is already in the heading where the owner reads it.
**Undo:** format the id in `closeout::block`; `tests/closeout.rs::block_heading` pins the shape.
**Review:** decide

### AD-902 — When is a red check "flaky" (one rerun) rather than a fix round? · slice-09 · 2026-10-10

**Chose:** when `gh run list --branch <b> --json …` shows a `success` run at the red run's `headSha` (`ci::green_at`): the same tree passed once, so the red is the check's. One rerun per head (`ci_rerun {head}` events at that head counted against `CI_FLAKY_RERUNS = 1`); a second red at that head blocks (`flaky check <c> red again after its rerun · ci`). A refused `gh run rerun` is treated as a fix round. A failed run whose `headSha` is not this branch's head blocks at once with no fix worker (`autonomy.md:79`).
**Over:** the slice's "HEAD equals the HEAD recorded at the last green or at PR creation".
**Because:** that comparison never matches on the first red after a push — no green exists at the new head yet — so a flaky first check would always cost a fix round, and after any fix commit the heads always differ. The run list says whether the same sha passed, which is what "no diff since the last green" means.
**Undo:** replace `green_at` with a lookup of `ci_green {head}` events in `closeout::watch_ci`.
**Review:** decide

### AD-903 — What does a new `factory run NNN` do on a PRD already `shipped`? · slice-09 · 2026-10-10

**Chose:** the preflight lets a shipped PRD through (`docs.mjs:1690`, "resumes at the PR, CI or owner review"), so `closeout::run` treats `board.status == shipped` as polished and shipped (no second polish worker, no second `docs ship`), and `open_pr` with no `pr_url` in state first asks `gh pr view <feature branch> --json url,state` (`ci::pr_view`): an open PR is reused (`set_pr_url`, `pr edit` refreshes the body, no `pr_opened`), none → `pr create`. The watch then runs and the run finishes. `tests/run.rs::budget_comes_from_the_flag_then_the_config_then_the_default` runs a finished fixture twice more: no worker spawned, one `pr create` and two `pr edit` in `gh.log`.
**Over:** polishing and shipping again (a second `polish:` commit and a second draft PR on the same branch); or refusing the run (that is `docs preflight`'s, outside this repo).
**Because:** nothing in §8 or the slice says what a run on a delivered feature does; the preflight's own line says "resumes at the PR".
**Undo:** drop `already_shipped` in `closeout::run` and the `pr_view` call in `open_pr`.
**Review:** decide

### AD-904 — What does the driver fill in when the polish worker leaves gaps? · slice-09 · 2026-10-10

**Chose:** the polish commit's pathspec is the stream's paths plus `{feature}/prd.md` and `{feature}/retro.md` always (the polish worker is the one worker allowed to write those two; `docs/index.md` stays the ship's); a missing `retro.md` is written from `templates/retro.md` beside `docs.mjs` (else a bare frontmatter with the twelve keys), a missing `runs/<id>/pr-body.md` from `assets/pr-body.md` (the PRD title and pointers to the log and the retro) — each said on stdout — so the ship and the PR never wait on a worker that skipped its last turn. The PR title is the PRD's h1 (`# PRD-001 — Fixture` → `PRD-001: Fixture`); the URL is `gh pr create`'s last non-empty stdout line.
**Over:** blocking when the worker wrote no retro or body.
**Because:** the slice says the worker writes them "in its last turn" and the acceptance checks they exist; it does not say what happens when they do not. A block would cost the owner a round-trip for a file the driver can write.
**Undo:** replace the two `fs::write` fallbacks in `commit_polish` with a `block(.., "polish", ..)`.
**Review:** decide

### AD-905 — What do `run_finished`, `pr_url` and `ci_cycles` carry? · slice-09 · 2026-10-10

**Chose:** `run_finished {pr_url, slices, checkpoints, ci_cycles, weighted_tokens, duration_minutes}` from `report::compute`, every field `#[serde(default)]` so a bare `run_finished` from before this slice still parses; `pr_url` in `state.json` and a `runs.pr_url TEXT` column by guarded `ALTER` (`SCHEMA_VERSION` unchanged); the report's `ci_cycles` is the count of `ci_fix_committed` events, and each `fix: CI <check>` commit bumps `ci_cycles` in `retro.md` in the same commit, so the retro the ship wrote stays in step with the report. `factory report` is unchanged otherwise.
**Over:** a second `runs` column for the cycles; a `run_finished` with only `pr_url`.
**Because:** task 5 names the fields and task 3 names the two homes of `pr_url`; the defaults, the `ALTER` and the retro bump are what a diff reviewer would miss.
**Undo:** the column is one line in `db.rs`'s `ALTER` list; the bump is `commit_ci_fix`.
**Review:** fyi

### AD-906 — How do the two new workers appear in the tables and the packs? · slice-09 · 2026-10-10

**Chose:** `Job::Polish {attempt}` → role `polish`, `Job::CiFix {round, attempt}` → role `fix` with `slice` and `checkpoint` NULL (slice 08's report counts both under `fix`, `build_minutes` includes them, `polish_minutes` is the polish worker's `duration_ms`); the polish worker's inner runs are `gates` rows with `source = worker`, `kind = slice` — slice 08's label for every worker run, left alone. The polish pack header carries `<!-- run dir: … -->` after the files line (the worker writes `pr-body.md` there), then `# Branch` (default and feature branch, `git log --oneline <default>..HEAD`), the PRD, `# Retro` (the template in a fenced block), `# PR body` (`<!-- write: <path> -->` and `assets/pr-body.md`'s shape), the contract with `AD-{seed}` = `{NNN}01`-style `AD-9901`, the autonomy text; the prompt is `/polish` (slice 03: `/polish` is among `init.slash_commands`). The CI fix pack is `<!-- factory-pack files: -->`, `# CI failure` with `<!-- round r, attempt a, check: name -->` and `gh run view <id> --log-failed`'s text, `assets/contract-ci-fix.md`, the autonomy text.
**Over:** a `role = ci-fix`; a `gates.kind = polish`.
**Because:** the slice names the roles (`polish`, fix) and the pack sections; the column values and the header line are the parts a reader of the tables would otherwise guess.
**Undo:** `Job::role` in `worker.rs`; `Pack::for_polish` / `for_ci_fix` in `pack.rs`.
**Review:** fyi

### AD-907 — What happens when `gh pr checks` reports no checks at all? · slice-09 · 2026-10-10

**Chose:** `Checks::None` ("no checks reported" in `gh`'s text) ends the watch as if green, without a `ci_green` event: the run finishes with the PR open — a repo with no workflow gets its draft PR and the owner reads it. A non-zero exit with no `fail` row and no such text (`Checks::Unreadable`) blocks in phase `ci` with `gh`'s text.
**Over:** blocking on "no checks" too.
**Because:** §8 says CI red is handled and green finishes; a repo without CI is neither, and the dogfood repo has a workflow so the case is only the fixture's and other repos'.
**Undo:** turn the `Checks::None` arm in `watch_ci` into a `block`.
**Review:** decide

### AD-908 — Where does the watch interval come from? · slice-09 · 2026-10-10

**Chose:** `FACTORY_CI_INTERVAL` (seconds, `> 0`) read at each watch, default `60` (`deliver/SKILL.md:193`); no config key.
**Over:** a `[ci] interval` key in `factory.toml`.
**Because:** task 4 says "interval injectable for tests" and nothing more; an env var is the smallest knob that a test harness and an owner can both turn, and the stub ignores it anyway.
**Undo:** read it from `Config` in `ci::interval`.
**Review:** fyi

### AD-909 — Where does the closeout read `branch.default` from on a fresh run? · slice-09 · 2026-10-10

**Chose:** `cli/run.rs` now calls `save_preflight` on the fresh-run path too (it already did on the resume path), so `closeout::default_branch` reads `preflight.json` from the run dir and falls back to `docs status`'s `branch.default`, then `main`.
**Over:** re-running `docs preflight` at the closeout.
**Because:** the slice says the default branch comes from the board's `branch.default` and that "the resume path has it in `state.json`"; the fresh path did not persist it anywhere.
**Undo:** delete the one `save_preflight` call; the fallback chain still works.
**Review:** fyi

### AD-910 — How is a dead, rate-limited or blocked polish worker handled? · slice-09 · 2026-10-10

**Chose:** slice 04's treatment, phase `polish`: a death is a `worker_dead {slice: null}` event, a Traps line `- **Dead polish worker (date)**: …` and one respawn (`polish_spawned {attempt: 2}`); a second death blocks (`two polish workers died · polish`), and an answer to that block spawns a third (deaths before the answer do not count). A usage limit waits for the reset (slice 07's `await_quota`) and respawns without counting as a death. `STATUS: blocked` parks on the worker's own `BLOCK-n · polish` entry.
**Over:** nothing new.
**Because:** task 1 says "the slice 04 treatment (once, then block with phase `polish`)".
**Undo:** —
**Review:** no

### AD-911 — How do the stubs script the closeout? · slice-09 · 2026-10-10

**Chose:** stub `gh`: `scenario/ci` in the repo root, one line per `pr checks` call — `green` (exit 0), `red:<check>[@<sha>]` (a `fail` row, exit 1; `run list` then shows that run as `failure` at `<sha>` or HEAD), `flaky:<check>` (as red plus a `success` run at the same head); a missing line is `green`; state in `$FACTORY_STUB_DIR/ci-state.json` (`pr`, `checks`, `reds`); `pr view … --json` answers only after `pr create` ran (`exit 1` before, as the real `gh` does for a branch with no PR); `run view <id> --log-failed` prints `FAIL <check>: expected 2 got 3`; `pr edit`, `run rerun`, `auth status` exit 0 silently; every call is one line in `gh.log`. Stub `claude`: the polish pack is recognised by `# Branch`, the CI fix pack by `# CI failure`; plan outcomes for polish are `polish|done` (writes `retro.md`, `reviewed:` in `prd.md`, `<run dir>/pr-body.md`, touches `touch ?? ["src/lib.txt"]`, forces slice statuses from `slices: {"02": "building"}`), `dead` (exit 137), `blocked` (its own `BLOCK-n · polish`), `limited`. `Fixture::script_ci` and `Fixture::gh_log` read and write them.
**Over:** a `gh` stub answering from a fixed JSON.
**Because:** task 4's acceptance scripts four CI shapes; a line-per-call file is the smallest thing that does.
**Undo:** both stubs document their formats at the top.
**Review:** fyi
