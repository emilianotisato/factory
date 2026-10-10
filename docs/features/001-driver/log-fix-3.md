# Fix log 3 — PRD-001

Polish fixer 3: the closeout's push, PR and CI watch (`src/closeout.rs`, `src/ci.rs`, `tests/closeout.rs`, `tests/stubs/gh`). The orchestrator merges these entries into `agent-log.md`.

## Traps

- `gh pr checks --json …` exits 1 when a row failed and 8 when one is pending, and prints the JSON on stdout either way. Parse stdout before reading the exit as an error. "no checks reported" comes on stderr with exit 1.
- `gh` puts `STARTUP_FAILURE` in its `pending` bucket. Classifying by `bucket` waits on a runner that never started, forever. `ci::class` reads `state` first.
- A `gh run rerun <id> --failed` keeps the run id and gives the jobs new ids. A check's job link (`…/runs/<run>/job/<job>`) tells the old attempt's row from the new one; the run id does not.
- `tests/closeout.rs::the_driver_never_merges_or_readies_a_pr` greps `src/` for the two `gh pr` subcommands the driver must never run. Do not write them in a comment or a message, even to say they are not used.
- The stub `gh` used to answer `pr checks` with one row and `run list` with every run on the branch. The wrong-run, wrong-workflow and `--limit` bugs passed every test until it printed real JSON, real job links and a run per workflow. Its header now documents `scenario/ci` (`<kind>:<check>[/<workflow>][@<sha>]`, `lag`, `stale`, `none`, `unreadable`) and `scenario/gh-fail`.

## Blockers

_(none)_

## Decisions

### AD-30001 — How long does "not registered yet" last after a push, a rerun or the watch's start? · polish · 2026-10-10

**Chose:** a grace window of `ci::GRACE_POLLS = 5` polls (`5 × FACTORY_CI_INTERVAL`, 5 min by default), counted from the watch's start, each fix push and each rerun. Inside it, each of these is a pending poll:
- a PR whose `headRefOid` is not the pushed head;
- an empty check table;
- a stale rerun row;
- a check name from the last red table that is missing from this one.

Past it:
- a PR head that is still behind blocks (`the PR's head X is not this branch's head Y · ci`);
- an empty table ends the watch as AD-907 says (`ci: no checks reported … nothing to watch`).

**Over:**
- judging the first poll after a push, which sees the previous head's suite or none at all;
- an unbounded wait for checks that never come.

**Because:** GitHub registers a pushed head's check suite seconds to minutes after the push. Five polls covers that without turning a repo with no workflow into a 6 h wait. The expected-names rule stops a red table that is still partly registered from reading as a smaller red.
**Undo:** set `GRACE_POLLS` to 0 in `src/ci.rs`; `judge` then decides on the first poll.
**Review:** fyi

### AD-30002 — When does a check that stays pending become a block? · polish · 2026-10-10

**Chose:**
- After `FACTORY_CI_TIMEOUT` seconds (default 6 h, the GitHub Actions job limit) since the watch's start, the last push or the last rerun, the watch blocks with `CI still pending after {span} · ci`. The block quotes the last table.
- Each poll is a single `gh pr checks --json`. Between polls the driver sleeps in one-second steps and checks `stop_requested` at each step and before each fix round's push. `factory stop` therefore ends a watch within about a second instead of at the next `--watch` exit.

**Over:** `gh pr checks --watch`, which never returns on a stuck check and cannot see a stop; a config key.
**Because:** a queued job with no runner and a check app that never reports are both real. Six hours is the longest a GitHub-hosted job can legitimately run, and the env var follows AD-908's knob.
**Undo:** raise `DEFAULT_TIMEOUT` in `src/ci.rs`.
**Review:** fyi

### AD-30003 — What do the CI caps count once the owner has answered a CI block? · polish · 2026-10-10

**Chose:** both caps count only events with a `seq` above the last answered `· ci ·` block's `closeout_blocked` event:
- fix rounds: distinct `ci_fix_spawned.round`;
- reruns: `ci_rerun` for the same check name at the same head.

An answer to the cap block therefore buys three fresh rounds, and the block text says so.

Round numbers keep counting from the run's highest round. The fourth round's pack is `ci-fix-4-*.md`, its log is `log-ci-4.md` and its AD seed is `8401`, so they never collide with round 1's. `ci_red.round` is the number of rounds since the answer.

**Over:** counting every round the run ever made. Under that rule, an answered cap block re-blocks on the next red and the owner's "fixed it by hand" buys nothing.
**Because:** `autonomy.md:77` caps the driver's own attempts between owner decisions. Once the owner has looked and answered, the driver gets another bounded try, never an unbounded one.
**Undo:** set `window` to 0 in `closeout::watch_ci`.
**Review:** fyi

### AD-30004 — What does each closeout block's `proceed` answer do? · polish · 2026-10-10

**Chose:** an answer is `proceed` when its first word is `proceed`, ignoring an option number (`2.`, `2)`), backticks and case. The driver reads it from the block entry's last `**Resolution (…):**` line. Each block that offers `proceed` gives it one effect:

| Block | Effect of `proceed` |
|---|---|
| `Two polish workers died · polish` | The driver ships what is committed, with no third polish worker. |
| `gh pr create refused · pr` | The PR is the owner's. The run finishes with no create and no CI watch. |
| `gh pr edit refused · pr` | The body is left as it is and CI is watched. |
| `gh pr checks unreadable`, `gh pr view unreadable`, `flaky check … red again after its rerun`, `CI did not run to a verdict`, `CI still pending after` (all `· ci ·`) | The checks are the owner's. The run finishes with no further watch and no `ci_green`. |

Any other answer, and any answer to a block with no `proceed` option, re-runs the step that blocked. For the CI cap block that means three fresh fix rounds (AD-30003).

**Over:**
- free-text intent parsing;
- keeping the texts' `proceed` option while no code read it, which is what the review found: every answer re-ran the step.

**Because:** each block now promises an effect the code delivers, and `tests/closeout.rs` answers four of them and checks what follows. A first-word rule is something the owner can type without guessing.
**Undo:** drop the `is_proceed` checks in `closeout::run`, `open_pr` and `watch_ci`, and the `proceed` options from the block texts.
**Review:** decide

### AD-30005 — What makes a red check flaky, and how often is it rerun? · polish · 2026-10-10

**Chose:**
- Each failed row's run is the run id in its job link, read with `gh run view <id> --json`.
- The red is flaky when every failed row has a run and, for each of those runs, `gh run list --commit <headSha> --workflow <workflowName> --limit 100` holds another run that concluded `success`: the same workflow passed at the same head.
- Each such run is rerun once with `--failed`, and a `ci_rerun` is emitted per check name.
- A check name red again at the same head after its rerun blocks (`flaky check … red again after its rerun`). A different check that turns flaky at that head still gets its own rerun.
- Several red workflows in one poll make one fix round. The check is the joined names (`test, lint`), and the log is one `--- run <id> (`<workflow>`), failed: …` section per run.

**Over:** AD-902's rule, which is any `success` in `gh run list --branch <b> --limit 20` at the red's head. It counted another workflow's green as this check passing and lost runs past 20. It also took the first `failure` in the list as the run to rerun or log, even when that run belonged to the other red workflow.
**Because:** "flaky" means the same check passed on the same tree. Only the same workflow at the same sha says that, and only the check's own link names its run.
**Undo:** `ci::passed_once` and `ci::runs_at` in `src/ci.rs`.
**Review:** fyi

### AD-30006 — What happens when `gh` refuses a call the red verdict needs? · polish · 2026-10-10

**Chose:** a refused `gh run rerun` blocks (`gh run rerun <id> refused: … · ci`), and so does a refused `gh run view` or `gh run list` (`gh run list refused: …`). None of them falls into a fix round, and no failure is swallowed as an empty list.
**Over:** AD-902's "a refused rerun is treated as a fix round", and the old `runs().unwrap_or_default()`.
**Because:** a fix worker given a flaky check, or a check whose run it cannot name, changes code that is not broken. A `gh` that refuses is a login or permission problem for the owner, as the other `gh` refusals already are.
**Undo:** map the `Err` arms in `closeout::on_red` back to `Next::Fix`.
**Review:** fyi

### AD-30007 — Which check states are infrastructure, and do they wait for the rest of the table? · polish · 2026-10-10

**Chose:**
- `TIMED_OUT`, `STARTUP_FAILURE`, `CANCELLED`, `ACTION_REQUIRED` and `STALE` (or the `cancel` bucket) are infrastructure. Any one of them blocks at once with `CI did not run to a verdict: <checks> · ci`, even while other checks are still running, and spawns no fix worker.
- The one exception is a run that is queued or in progress again (the owner reran it). The watch then waits for it.

**Over:**
- skipping these rows, as `failed_run` did by matching only `failure`;
- waiting for the whole table to settle first.

**Because:** `autonomy.md:79` says infrastructure blocks at once. A timeout or a runner that never started is not something a fix worker can see in a diff.
**Undo:** move the states to `Class::Fail` in `ci::class`.
**Review:** fyi

### AD-30008 — How does the watch tell a rerun's old rows from its new ones? · polish · 2026-10-10

**Chose:** the job links the driver just reran are kept in memory (`Watch::stale`). A row with one of those links reads as "the rerun of `x` has not registered". After a driver restart that memory is gone, and the run's own `status` (`queued` or `in_progress`, read with `gh run view`) makes the watch wait instead.
**Over:** a `links` field on `ci_rerun`, which would make the set survive a restart. `src/events.rs` is outside this fix's files, so that field is listed for the orchestrator.
**Because:** the in-flight status covers the restart case for everything except a rerun that has not yet left `completed`, a window of seconds.
**Undo:** drop `Watch::stale`; `ci::judge` then reads the old attempt's rows as a fresh red, and the rerun counter blocks.
**Review:** no

### AD-30009 — Where do ship and push check the branch? · polish · 2026-10-10

**Chose:** `closeout::branch_guard` runs before `docs ship` (phase `polish`) and before every `git push` (phase `pr`), including each fix round's push. It blocks with `{ship|push} stopped: {problem}` when any of these holds:
- the run names no feature branch;
- the run's branch is the default branch;
- HEAD is detached;
- HEAD is on another branch.

`gh pr create` gains `--head <branch>`. HEAD is read with `git symbolic-ref --quiet --short HEAD`.

**Over:** checking once at the start of the closeout, which misses a HEAD that moves while the run is parked on a block (the regression test does exactly that).
**Because:** the driver must never push the default branch or one the run does not own. A guard at each push is the only place the check cannot go stale.
**Undo:** remove the two `branch_guard` calls.
**Review:** fyi

### AD-30010 — When is the ship done on a re-entry? · polish · 2026-10-10

**Chose:** the ship is done when HEAD's `prd.md` says `status: shipped` (`git show HEAD:<dir>/prd.md`) or a `shipped` event exists. A board that says `shipped` while HEAD does not is a ship killed before its commit. The ship runs again: `docs ship` is a no-op on a shipped PRD, and the retro, render and `docs: ship PRD-NNN` commit follow.
**Over:** the board's `status` alone (`already_shipped`), which skipped the ship commit and left `prd.md` staged.
**Because:** the commit is the half that can be lost, so HEAD is the source of truth for it.
**Undo:** set `ship_committed` back to `board_shipped` in `closeout::run`.
**Review:** no
