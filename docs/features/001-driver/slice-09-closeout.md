---
slice: 09
status: pending
depends_on: [8]
---

# Slice 09 — Closeout: polish worker, ship, retro keys, PR, CI (FR-6)

**Source**: [`prd.md`](prd.md) → §8 FR-6 (all four decisions and acceptance); §4 FR-2 bullet 6 (slash commands inside `-p` verified at slice entry); §10 FR-8 (the retro keys come from the store). **Depends on**: 08 (the retro's numeric keys are `factory report`'s keys; `ci_cycles` and `polish_minutes` are rows this slice writes).
**Created**: 2026-10-09 · **Status**: pending

**Progress**: _(written as it lands)_

**Slice**: After this slice a run ends in a draft PR with CI green, or parks with a block that says why. When the last checkpoint is done the driver spawns the polish worker (`/polish` Unattended, which writes the retro prose and the PR body in its last turn), runs `docs ship NNN`, fills every numeric retro key from the store, commits the closeout, pushes the feature branch, opens the draft PR with `gh`, watches the checks, and handles red CI with the same three-round cap and one flaky rerun today's `/deliver` prose prescribes. It never merges, never marks the PR ready.

## Current code / environment

- **Today's prose**: `deliver/SKILL.md` §3 Polish `:165` (`/polish` Unattended), §4 Closeout `:173-179` (`:175` "ship, never archive", `:178` retro keys from `measure.py --retro`), §5 PR `:181-188` (`:185` `gh pr create --draft --base {default} --title "PRD-{NNN}: {title}" --body-file …`, `:188` the body file, resume → `gh pr edit`), §6 CI `:190-198` (`:193` `gh pr checks <n> --watch --interval 60`), `autonomy.md:77` three CI rounds, `:78` flaky rerun once with `gh run rerun <id> --failed`, `:79` a failing check that is not this branch's → block. `polish/SKILL.md` Unattended markers at `:23,94,129,166,183,199`.
- **Does `/polish` resolve inside `-p`?** Slice 03 task 1 read `init.slash_commands` from a real stream and recorded the answer in its Progress and in `prd.md` §20. Listed → the prompt is `/polish` and the pack carries the Unattended flag; not listed → the pack carries `polish/SKILL.md` verbatim as `# Skill` and the prompt says to follow it. Read slice 03's Progress at entry; do not re-test.
- **What the polish worker writes** (§8): "the retro prose and the PR body in its last turn, before it exits". Contract `assets/contract-polish.md`: run `/polish` Unattended on the branch, then write `docs/features/NNN-*/retro.md` prose sections (leaving the numeric keys for the driver, marked `<!-- factory fills -->`), write `runs/<id>/pr-body.md` in the shape of `deliver/SKILL.md:188`, set `reviewed:` in the PRD frontmatter (that is `/polish`'s job, `prd-to-tasks` §9), return STATUS. The driver's run-dir path is given to the worker in the pack header since the worker must write there.
- **`docs ship NNN`** (`docs.mjs:2084`) refuses while any slice is not `done`; it sets `status: shipped` and stages. `retro.md` keys (`templates/retro.md:2-14`): `score` stays empty (owner's), the other twelve come from `factory report` (slice 08); the driver fills them in place by key name.
- **Push and `gh`**: the owner's `pre-push` hook refuses the default branch and non-fast-forward pushes under Claude Code; the driver runs outside Claude Code and pushes only `feat/*`, fast-forward, so the hook is not in play, and the fixture runs with `GIT_CONFIG_GLOBAL` empty anyway. The fixture needs an `origin`: a bare repo in the temp dir (`git init --bare`, `git remote add origin`), so `git push -u origin <branch>` is real and `gh` is the stub. The stub `gh` (slice 01) gets scenario files here: `scenario/ci` with one line per `pr checks --watch` call (`green`, `red:<check>`, `flaky:<check>`), `pr checks` printing the check table and exiting 1 on red, `run view --log-failed` printing a canned log, `run rerun` logging the call.
- **Default branch**: from the board's `branch.default` (slice 02's `preflight` struct; the resume path has it in `state.json`).
- **"No diff since the last green"** (flaky rule): compare `git rev-parse HEAD` at the red check with the HEAD recorded at the last green or at PR creation; equal → flaky, rerun once; a second red at the same HEAD → block.
- **Deliberate non-changes**: *(prd §8 "Rejected: decomposing polish into driver-run steps."; one worker, one pack.)* *(prd §8 "Cut 2026-10-09: an earlier draft spawned a separate closeout worker"; the polish worker writes the prose.)* *(prd §8 "The PR is never merged, never marked ready without the owner."; no `gh pr merge`, no `gh pr ready` anywhere in the binary, and a test greps the source for both.)* *(`deliver/SKILL.md:175` "ship, never archive".)*

## Tasks

### 1. Polish worker — high — blocked by: —
**Source**: §8 FR-6: "Polish is one worker. Full `/polish` Unattended as today, with the Cursor review inside it." "The polish worker writes the retro prose and the PR body in its last turn, before it exits".

`src/closeout.rs`: when the last checkpoint is done (`allDone` and `unverified` empty) → phase `closeout`, event `polish_spawned`; worker role `polish`, pack per the Current code entry (`/polish` prompt or skill body; `# Branch` with the default and feature branch names and `git log --oneline <default>..HEAD`; `# Retro` with the `retro.md` template and which keys the driver fills; `# PR body` with the shape and the path to write; `assets/contract-polish.md`; `assets/autonomy.md`). On exit: commit its pathspec with `polish: review fixes` (slice 05's `commit.rs`) when it touched anything; `polish_minutes` is its `duration_ms`. Dead or `blocked` → the slice 04 treatment (once, then block with phase `polish`).

**Acceptance:** stub scenario `polish` (writes `retro.md` prose, `reviewed: true` in the PRD, `runs/<id>/pr-body.md`, touches `src/lib.txt`): one `workers` row with `role='polish'`, a `polish:` commit touching `src/lib.txt`, `retro.md` and `prd.md`, and `pr-body.md` present; the recorded pack contains either `/polish` as the prompt or the skill text, matching slice 03's recorded answer.

### 2. Ship, retro keys, closeout commit — high — blocked by: 1
**Source**: §8 FR-6: "`factory` runs `docs ship NNN` and fills every numeric key of `retro.md` from the metrics store (FR-8)." Acceptance: "`docs ship` runs, `retro.md` has every numeric key filled".

After the polish commit: `docs ship NNN` through `docs.rs` (refusal → block with phase `polish` and the tool's text); fill `retro.md`'s twelve numeric keys from `report::keys(run)` by key name, leaving `score` empty; `docs render --write`; `git add -- docs/features/NNN-*/ docs/index.md` and commit `docs: ship PRD-NNN`. Event `shipped`.

**Acceptance:** after the fixture closeout, the PRD frontmatter reads `status: shipped`, `retro.md` has a value on each of the twelve numeric lines and an empty `score:`, `git log -1 --format=%s` is `docs: ship PRD-001`; a fixture where one slice is forced back to `building` makes `docs ship` refuse and the run block with phase `polish`.

### 3. Push and the draft PR — high — blocked by: 2
**Source**: §8 FR-6: "PR and CI are driver work. `gh pr create --draft --body-file`". `deliver/SKILL.md:185` (`--base {default} --title "PRD-{NNN}: {title}"`), `:188` (`pr-body.md`), resume → `gh pr edit`. Acceptance: "a draft PR call happens once".

`git push -u origin <feature branch>` (fast-forward only; a non-fast-forward failure is a block with phase `pr`, never a force); `gh pr create --draft --base <default> --title "PRD-NNN: <title>" --body-file runs/<id>/pr-body.md`, capture the URL, store `pr_url` in `state.json` and the `runs` row, event `pr_opened {url}`. On resume with `pr_url` set: `gh pr edit <n> --body-file …` instead of create. The fixture's `origin` is a bare repo in the temp dir.

**Acceptance:** `gh.log` holds exactly one `pr create` line with `--draft`, `--base main`, `--title PRD-001: Fixture`, `--body-file`; the bare `origin` has `feat/fixture` at the closeout commit; a resume after `pr_opened` adds one `pr edit` line and no second `pr create`; `grep -r "pr merge\|pr ready" src/` finds nothing.

### 4. CI watch: fix rounds, flaky rerun, block — high — blocked by: 3
**Source**: §8 FR-6: "then `gh pr checks --watch --interval 60`. Red: a fix worker with the failing check's log, up to three rounds. One failed check with no diff since the last green: rerun once. Then block. The caps are the same numbers as `autonomy.md`; they move from prose to constants." Acceptance: "one red check spawns one fix worker, a flaky check is rerun once and only once."

`src/ci.rs`: `gh pr checks <n> --watch --interval 60` (interval injectable for tests); green → `ci_green`, phase `finished`, event `run_finished {pr_url}`; red → `ci_red {check, round}`, `ci_cycles += 1`; if HEAD equals the last-green HEAD → `gh run rerun <id> --failed` once (`ci_rerun` event), watch again; a second red at the same HEAD → block; else fix worker (`autonomy.md:77` constant `CI_FIX_ROUNDS = 3`, `CI_FLAKY_RERUNS = 1`, with the citation in a comment) with `gh run view <id> --log-failed` output as `# CI failure`, commit `fix: CI <check>`, push, watch again. A failing check that is not this branch's (the check's `head_sha` differs) → block with phase `ci` (`autonomy.md:79`). Round 4 → `BLOCK-<NNN>-ci` in `agent-log.md`, phase `waiting-owner`. Rate-limited fix workers do not count (slice 07 task 2).

**Acceptance:** `scenario/ci` = `red:test, green` with plan `…, fix` → one `fix` worker, one `fix: CI test` commit, two `pr checks` lines in `gh.log`, `ci_cycles: 1` in the report, `run_finished`; `scenario/ci` = `flaky:test, green` → one `run rerun` line, no fix worker, finished; `flaky:test, flaky:test` → one rerun, then `BLOCK-001-ci`; `red, red, red, red` → three fix workers, then block, no fourth fix.

### 5. The run's final report event — medium — blocked by: 4
**Source**: §9 FR-7: `factory wait` returns on `finished` with the summary. `deliver/SKILL.md:200` §7 Report (what the owner reads first).

On `finished`: event `run_finished {pr_url, slices, checkpoints, ci_cycles, weighted_tokens, duration_minutes}` from `report::keys`; `state.json.phase = finished`; the `runs` row gets `finished_at`. On a block in closeout: `run_waiting` as in slice 04 so `factory wait` has one event kind to watch for both.

**Acceptance:** the last line of `events.jsonl` after a green fixture closeout is `run_finished` with the PR URL and `slices: 3`.

### 6. Verify — medium — blocked by: 1–5
`cargo test`. The full gate runs at this slice's checkpoint.

**Acceptance:** inner loop green, one commit.

### 7. Prove — at the checkpoint — blocked by: 6
Rows from [`prd.md`](prd.md) § QA plan: **FR-6** ("run the fixture closeout → `docs ship`, retro keys filled, one draft PR call, one fix worker on red CI, one flaky rerun", evidence: stubbed `gh` call log). At the checkpoint: the task 4 `red:test, green` and `flaky:test, green` runs; paste `gh.log` and the filled `retro.md` keys into this slice's **Progress**. The CI run on this repo's own PR (FR-10's second evidence) is noted here once the PR exists.

**Acceptance:** `gh.log` shows one `pr create`, the `pr checks` lines, one `run rerun`; `retro.md` keys are attached.
