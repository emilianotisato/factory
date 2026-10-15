---
number: 007
slug: polish-in-the-binary
title: "Polish in the binary"
type: feature
created: 2026-10-14
author: Emiliano Tisato
summary: |
  The mechanical half of polish moves into the binary. Preflight checks the Cursor login before a run, `factory review collect` runs leaks, the reviewer plan and the second opinion (passes in parallel) into one file, and the driver runs it before the polish worker. Path patterns as a reviewer floor and the per-slice fix rule were cut at the review. Agreed 2026-10-14; sliced into five (§12), ready for `/deliver`.
from_idea: "2026-10-13-polish-collect-in-the-binary-pick-reviewers-from-t"
---
> **History.** The code, `docs/reference` and `docs/decisions.md` win over this document.

# PRD-007 — Polish in the binary

## 1. Why now

**The bug the owner hit, five runs in a row.** The cross-vendor second opinion never ran in a polish of this repo. 002, 004, 005 and 006 found Cursor logged out; 003 timed out at 900 s with no text. Each time the failure surfaced inside the polish worker, after the owner had left: "By the time polish probed, nobody was there to log in" (004 retro). Four retros asked for the same fix: a Cursor check at preflight (002, 004, 005, 006).

**What it cost.** The Claude fallback, one fresh agent per 600-line pass, was the largest line of every polish it ran in: 26% of 004's raw polish tokens, 70% of 005's (9 agents, 13.6M), 61% of 006's (3 agents, 2.8M). Polish was 35 to 52% of each run's weighted tokens. The fallback also earned its place: it found a high no other reviewer found in 002, 003 and 005. So the fix is to make Cursor run, not to drop the second opinion.

**Recon corrected the seed idea.** The idea claimed the collect half costs 30 to 60 minutes and 9 to 18 dollars of model time. Picking reviewers and running leaks cost the worker a few turns; the money went to the fallback and to the judgment reviewers. The improvement here is still worth making: the second opinion leaves the worker's context and its tool timeout, and the worker starts from one file.

**Improvement, not a bug.** Every polish worker peaked at 164k to 168k of context, the compaction ceiling. This PRD moves work out of that context; it does not split the worker (§13).

Left alone: the local, security and UX reviews stay model work, spawned by the skill. Triage, fixes and the closeout stay in the worker.

**Hypothesis for the observatory:** the next run's polish weighted tokens land under **1.9M**, half the 3.88M mean of 004 (3.66M), 005 (5.79M) and 006 (2.18M). It is stored with `factory run --hypothesis`, and the check is the `polish` entry of `weighted_by_role` in `factory report` on that run (`src/cli/report.rs:689-690`). *Corrected 2026-10-14: an earlier draft said `factory report` checks it; the report prints the number and the stored text, and a person compares them.*

## 2. Blast radius

- **The owner, at the start of a run.** Preflight gains one line. `factory run` prints preflight's judge lines as it starts (`src/cli/run.rs:132-136`), in the `/deliver` session, and does not wait on them. Logging in any time before polish is enough, because the collect probes again (FR-3). A failed probe is a judge line, never a refusal (FR-1). *Corrected 2026-10-14: an earlier draft said `/deliver` prints preflight before `factory run`; preflight runs inside `factory run` (`src/cli/run.rs:93`), and the judge line reaches neither `factory wait` nor the cockpit (`JudgeNoted` is not a cockpit event, `src/cockpit.rs:14-29`).*
- **The polish phase of every run.** The driver spends up to one Cursor pass (900 s cap) before the polish worker spawns (FR-5). The worker's pack gains a `# Collected` section. A run started on an older binary is unaffected: the driver is the binary.
- **`factory review second`.** Passes run at once instead of in turn (FR-2). Exit codes stay as documented at `src/cli/review.rs:1-2`, and the sections keep the pass order of today's loop (`:98-133`). Any script reading it sees the same text, sooner.
- **The installed skills.** The polish skill and its contract change, so the crate bumps to 0.5.0 and each repo runs `factory skills update` (FR-7).
- **Cursor quota.** Parallel passes spend the same calls as before, at once. No other cost surface moves.

## 3. FR-1 — Preflight checks the second opinion

**Today.** `factory docs preflight` (`src/docs/preflight.rs:87-243`) checks the board, skills, leaks, the tree and the branch. Nothing checks Cursor. The probe exists: `review::probe()` (`src/review.rs:243`) runs `<agent> --list-models` under the timeout and fails on a nonzero exit; after exit 0, on text holding "login" or "Authentication required", or on the model missing from the list (`src/review.rs:263-277`). It runs only inside `factory review second` (`src/cli/review.rs:92-96`), which the worker reaches in polish §1, hours after preflight. The probe takes about 1.3 s on this machine.

**Decision (2026-10-14).** Preflight runs the same probe with the user's `[review]` config. Ready → an ok line naming the agent and the model. Not ready → a **judge** line: the reason, the login command, and that polish will fall back to one Claude agent per 600-line pass. The run still starts if the owner starts it, and polish keeps the fallback.

- Accepted cost: a run can still pay the fallback when the owner ignores the line. Refusing the run lost because a machine without Cursor could not run at all, and skipping the second opinion lost because the fallback found highs in 3 of 5 runs.
- Accepted cost: preflight takes about a second longer.

**Acceptance.** On the harness fixture (feature `001`), with the stub `cursor-agent` in its logged-out mode (`list_models = "auth"`: prints `Authentication required`, exits 1, `tests/stubs/cursor-agent:41-45`), `factory docs preflight 001` prints a `?` judge line holding the reason and the login command; with the stub listing the model, a `✓` line naming the agent and the model. `--json` carries the same text in `judge` or `ok`. The exit code is the same in both modes: the probe line never changes it.

## 4. FR-2 — The second opinion runs its passes at once

**Today.** `factory review second --diff` splits the diff into passes (`src/review.rs:33`) of about `pass_lines` changed lines, 600 by default, and runs them one after another (`src/cli/review.rs:98-133`), each under `timeout_secs`, 900 by default (`src/config.rs:71-72`). On 005's diff that is 9 passes, up to 135 minutes. Workers get no `BASH_MAX_TIMEOUT_MS`, so inside a worker a foreground call is capped by Claude Code's Bash limit, well under that.

**Decision (2026-10-14).** All passes start at once, each its own child process and log, as today. The command waits for every pass, then prints the sections in pass order. Exit codes are unchanged: 0 when any pass counted, 2 when none did.

- Accepted cost: Cursor's limit on parallel sessions is unknown. A pass it refuses prints `unreviewed` like any failed pass today, and the others keep their findings.
- Accepted cost: nothing prints until the slowest pass ends. Progress lines on stderr stay.

**Acceptance.** With a stub `cursor-agent` that sleeps 2 s per pass, a three-pass diff finishes in under 4 s of the test's wall clock. Its stdout holds the three `## Pass n` sections in pass order, each with the text the stub replayed for that pass, then the `log:` line.

## 5. FR-3 — `factory review collect` writes one findings file

**Today.** Polish §1 (`skills/polish/SKILL.md:29-62`) has the worker pick reviewers from the diff, run `factory leaks --base`, and run `factory review second`, reading each result into its own context.

**Decision (2026-10-14).** A new verb, `factory review collect --base <branch> [--out <file>]`, does the three mechanical steps and writes one markdown file:

1. **Plan.** The changed files and lines against `<base>...HEAD`, and the size row: one file and under 50 changed lines needs the local review only; anything larger adds the second opinion. The security and UX passes stay the skill's call (FR-7).
2. **Leaks.** The same scan as `factory leaks --base <base>`, with its verdict (clean, hits, or did not run) and its hits under their fix lines.
3. **Second opinion.** `factory review second --diff <base>`, only when the size row asks for it, with its exit code, each pass's status line, and the findings text. When it is unreviewed, the plan says `fallback owed`: the worker runs the Claude fallback.

The file starts with `<!-- head: <sha> -->`, the HEAD it reviewed. Exit 0 when the file was written, whatever each part found; exit 1 on a usage error. Standalone `/polish` calls the same verb.

- Accepted cost: the second opinion no longer gets `--others` inside a run, because it runs before the local review exists. No run here has used it, since Cursor never ran. `factory review second --others` stays for attended use.
- Accepted cost: the 50-line threshold is a number, not the table's judgment ("a few lines"). A small change on the table's fourth row (auth, money, a public API) gets no Cursor pass from the collect; the worker can still run `factory review second` itself.

**Acceptance.** On the fixture repo with a two-file branch and the stub `cursor-agent`, `factory review collect --base main --out f.md` exits 0 and `f.md` holds the head line, the plan with its size row and changed files, the leaks verdict and one second-opinion section per pass. With the stub logged out, the file says unreviewed and the plan says `fallback owed`. On a one-file branch under 50 changed lines, the stub `cursor-agent` is never called and the file says the second opinion was skipped by size.

## 6. FR-4 — RETIRED (2026-10-14)

**Was:** project path patterns. An optional `[polish]` table of `security` and `ui` globs made those passes required, a `polish` line in `factory docs doctor` pointed at it, and this PRD seeded the table for this repo.

**Owner's call, at the cut (2026-10-14).** No polish on record skipped a pass it needed, so the floor had no evidence behind it. The security and UX passes stay the skill's two override rules (`skills/polish/SKILL.md:40`). The shape the interview settled (the table, the match with the `ignore` crate, the doctor line, the discovery paths) is in idea `2026-10-14-polish-path-patterns-as-a-reviewer-floor`, so a future PRD does not redo it.

*Corrected 2026-10-14, kept because the owner heard it in the interview: `/prd` preflight does not work through every doctor line that is not ok. `skills/prd/SKILL.md:22-28` routes on four framework states and moves on. Only `docs/reference/adopt-a-repo.md:34` walks the doctor lines.*

## 7. FR-5 — The driver collects before the polish worker

**Today.** `polish()` (`src/closeout.rs:134-318`) builds the pack (`Pack::for_polish`, `src/pack.rs:445-519`) and spawns the worker in the closeout lane. Respawns after a death or a rate limit rebuild the pack on each attempt (`src/closeout.rs:191-204`).

**Decision (2026-10-14).** Before the first polish spawn, the driver prepares the polish lane and runs the collect in it against the default branch, writing `<run dir>/collect.md`. A respawn reuses the file when its head line matches the lane's HEAD, and collects again when it does not. The pack gains a `# Collected` section: the plan, the leaks verdict, each second-opinion pass's status line, and the file's absolute path. The findings text stays in the file; the worker reads it at §2, so it does not anchor the local review.

A collect that fails (a git error) does not stop the run: the section says `collect failed: <reason>`, and the worker takes the skill's own path for that case (FR-7). The driver prints one line with the collect's duration.

- Accepted cost: up to one pass (900 s) of wall clock before the worker spawns. Today the worker waits the same time or longer, sequentially, inside its own context.

**Acceptance.** In the stub harness, a run reaching polish writes `collect.md` before the polish worker's spawn, and the polish pack holds `# Collected` with the file's path. A rate-limited polish worker's respawn reuses the file without a second stub `cursor-agent` call.

## 8. FR-6 — RETIRED (2026-10-14)

**Was:** a `REVIEWERS` return line in the polish contract, and a driver check that wrote one `decide` decision per required pass the worker did not run.

**Owner's call, at the cut (2026-10-14).** It checked the floor FR-4 declared, and FR-4 is retired. Its recon (the return block at `assets/contract-polish.md:25-32`, `log::add` at `src/docs/log.rs:195`) is in the same idea.

## 9. FR-7 — The skill and the contract follow the binary

**Today.** Polish §1 picks reviewers from a size table and two override rules (`skills/polish/SKILL.md:29-40`), then runs leaks and the second opinion itself (`:48-62`).

**Decision (2026-10-14).**

- **§1 starts from the collected file.** In a run, the pack names it. Attended, or when the pack says `collect failed`, the skill runs `factory review collect --base <base> --out <scratchpad>/collect.md` in the background, since it can outlast a Bash call. If that fails too, §1 keeps today's two commands as the last resort: `factory leaks --base` and `factory review second --diff`. The size table and the two override rules still pick the security and UX passes. The fallback text is unchanged: one fresh agent per pass, when the file says `fallback owed`. *Corrected 2026-10-14: an earlier draft had FR-5 send a failed collect to "§1 as today" while this FR replaced §1; the last-resort line keeps both true.*
- **The contract** gains one line: the `# Collected` section holds the plan, the leaks verdict and the second opinion, and §1 starts from it.
- The skill edit bumps the crate to **0.5.0** and rewrites `skills/bundle.lock` (`FACTORY_BUNDLE_WRITE=1 cargo test`).

- Accepted cost: every repo needs `factory skills update` after the release, and the doctor says so until it runs.

**Acceptance.** The bundle-lock test (`tests/skills_bundle.rs`) is green with `skills/bundle.lock` at 0.5.0. The polish pack the driver builds holds, in its `# Contract` section, the line that starts §1 from `# Collected`. `skills/polish/SKILL.md` names `factory review collect` and the last-resort commands.

## 10. Decisions log

None open. The interview settled every fork on 2026-10-14, and each FR records its decision and accepted cost:

- ~~Scope~~ — probe plus a driver-run collect; the worker is not split (§13 Non-goals).
- ~~Cursor out~~ — warn at preflight, keep the fallback.
- ~~Timing~~ — the driver runs the second opinion first, passes in parallel.
- ~~Picking~~ — the interview chose project path patterns, written by hand, security and UI classes, enforced by contract plus a driver check. The cut removed them (FR-4 and FR-6 retired): the skill's table and override rules keep picking.
- ~~Discovery~~ — a doctor line and the worker's proposal in the retro; cut with the patterns.
- ~~§8~~ — the interview folded the per-slice rule in; the cut removed it.
- ~~Proof~~ — stub tests, then the next live run.
- ~~The cut~~ — kept: parallel passes (FR-2), the respawn reusing the collect (FR-5), and the size row skipping Cursor on a small diff (FR-3). Everything else listed at the cut went to §13.

## 11. QA plan

**Gate**: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` *(declared in project.toml)*
**Surface**: CLI. `factory docs preflight`, `factory review second`, `factory review collect`, and the driver's polish phase.
**Harness**: the existing test harness: the fixture repo, the stub `claude` replaying recorded streams, and the stub `cursor-agent` (`tests/stubs/cursor-agent`, `tests/review.rs`). Nothing user-visible beyond the CLI.
**Seed state**: the harness fixture; no seeder.

| FR | Proof on the real surface | Evidence | Human? |
|---|---|---|---|
| FR-1 | run `factory docs preflight 001` against the stub logged out, then logged in: a judge line with the login command, then an ok line with the model, same exit code | test transcript | no |
| FR-2 | run `factory review second --diff` over three passes with a slow stub: the passes overlap in time, the output order holds | test transcript | no |
| FR-3 | run `factory review collect` on a fixture branch: one file with the head line, plan, leaks and second opinion; logged out, `fallback owed`; a small one-file branch never calls the stub | test transcript, the written file | no |
| FR-4 | retired | — | — |
| FR-5 | run a stubbed feature to polish: `collect.md` exists before the polish spawn, the pack names it, a respawn reuses it | test transcript, the pack file | no |
| FR-6 | retired | — | — |
| FR-7 | run the gate: bundle lock green at 0.5.0, the polish pack's contract starts §1 from `# Collected` | gate transcript | no |

**Live proof.** The next `/deliver` on any repo: `factory run` prints the Cursor line as it starts, the driver log shows the collect and its duration, and the `polish` entry of `weighted_by_role` in `factory report` is read against the hypothesis (§1).

## 12. Slices

Five slices on `feat/polish-in-the-binary`: one branch, one PR, one commit per slice, Prove at the checkpoints. `factory docs status` is the board.

- **01, passes at once** (FR-2). `factory review second --diff` starts every pass in its own child, prints the sections in pass order, and every reviewer dies with factory. The pass runner moves into the `review` library. ⚠ The highest risk: the signal handler tracks one child today (`src/review.rs:162-171`), and a leaked reviewer runs on for up to 900 s.
- **02, the Cursor line at preflight** (FR-1). `factory docs preflight` and `factory run` probe with the user's `[review]` config and print an ok or a judge line. The probe sits in those two paths, not in `preflight::run`, which the scheduler, the closeout and the tests call in-process.
- **03, `factory review collect`** (FR-3). The plan, the leaks scan and the second opinion in one file, built in the library behind a new verb. Its fixed headings are what 04 lifts into the pack.
- **04, the driver collects at polish** (FR-5). The polish loop prepares the lane, collects or reuses `collect.md`, then builds the pack with `# Collected`. A rate-limited respawn reuses the file.
- **05, the skill, the contract and 0.5.0** (FR-7). Polish §1 starts from the file, with the attended path and the last resort. The contract gains its line, the crate goes to 0.5.0, and the lock is rewritten.

**Order.** 01 and 02 start together: they share no file. 03 waits for 01, because the collect runs the second opinion through 01's runner and both edit `src/cli/review.rs` and `tests/review.rs`. 04 and 05 each wait for 03 only and run two wide after it: 04 edits the driver and the pack builder, 05 the skill, the contract asset and the version. Their one shared file is `tests/closeout.rs`, in different hunks. Width 2, then 1, then 2, within `lanes = 4`.

**Checkpoints** (`checkpoint_every = 2`). The first covers the first two done, 01 and 02 unless 02 runs long, so the parallel runner and its signal handling are proven before the collect builds on them. The second covers the next two, 03 among them; the last covers the fifth. 03's file format is what 04 reads, so a checkpoint proves it together with a reader.

**The installed 0.4.0 driver builds this run**, and it predates every part of this feature. Its preflight prints no Cursor line, and its polish pack has no `# Collected` section. This repo's `.claude/skills` link into `skills/`, so the polish worker reads slice 05's §1, takes the attended path, and calls `factory review collect` on the installed binary, which has no such verb. It lands on the last resort: this feature's own polish is that line's first live test. The checkpoints prove each row from the lane's build. 0.5.0 is tagged after the merge, not in the run; every repo then needs `factory skills update` (§9's accepted cost).

**No `qa-guide.md`**: no row in §11 needs a human.

## 13. Non-goals

- **The local, security and UX reviews stay model work.** They read content and judge it; the skill picks them from its size table and override rules, as today.
- **The worker is not split.** Triage and fixes keep sharing one context. Every polish worker peaked at 164k to 168k; that is the decompose-polish idea (2026-10-09), still open.
- **Not handled: parallel polish fixers, each in its own lane.** Accepted 2026-10-14: its retrigger, a Fix list past an hour with disjoint file sets, has not happened. Revisit when `factory report` shows it (idea 2026-10-14).
- **Not handled: refusing a run while Cursor is logged out.** Accepted 2026-10-14: a machine without Cursor could not run, and the fallback found highs in 3 of 5 runs. Revisit if the owner keeps starting runs past the preflight line.
- **Not handled: `--others` inside a run.** Accepted 2026-10-14: the driver runs the second opinion before the local review exists. Revisit if a live Cursor pass misses a high the local review found and `--others` would have pointed at.
- **Not handled: path patterns as a reviewer floor**, with the doctor line, the worker's proposal, the adoption-doc bullet and the driver's check (FR-4, FR-6). Cut 2026-10-14 by the owner: no polish skipped a pass it needed. Revisit when one does (idea `2026-10-14-polish-path-patterns-as-a-reviewer-floor`).
- **Not handled: the per-slice rule for polish fixes (§8 of the skill).** Cut 2026-10-14 by the owner at the cut. It stays in idea `2026-10-14-friction-factory-prd-006`. Revisit if a polish has to guess a fix's slice again.
- **Not handled: a configurable pass parallelism.** Accepted 2026-10-14: no evidence Cursor refuses parallel passes. Revisit if a run's passes come back `unreviewed` together.
- **Not handled: the driver running the fallback as its own workers.** Accepted 2026-10-14: the fallback stays the worker's subagents. Revisit if the fallback still dominates polish after Cursor is checked at preflight.
- **The prune pass's cost** (1.5M to 4.6M raw per polish) is not touched here.

## 14. Verification notes

Checked against `main` at `09b1078`, 2026-10-14. **Re-verify at slice entry.**

- Polish second opinion per retro: 002 Cursor "Authentication required" ×2, then two Opus agents (`docs/features/002-docs-port/retro.md:31`); 003 Cursor killed at 900 s with no result, then Opus (`003-observatory/retro.md:41`); 004 auth expired, two Claude agents (`004-skills/retro.md:17,33`); 005 unauthenticated, 9 agents (`005-isolated-lanes/retro.md:17`); 006 unreviewed, 3 agents (`006-comment-discipline/retro.md:17,30`).
- Polish workers from `~/.local/share/factory/factory.db` (`workers`, role `polish`), weighted per `src/quota.rs:22-27` (input 1, cache write 1, cache read 0.1, output 5): own / subagents = 004 0.35M / 3.31M, 005 1.57M / 4.21M, 006 0.88M / 1.30M; peak context 164,075 to 168,595 on all seven polish workers on record.
- Raw subagent tokens by role, from the polish streams: 005's 9 fallback passes 13.6M of 19.5M; 006's 3 passes 2.83M of 4.6M; 004's 2 passes 4.26M of 16.3M.
- `src/review.rs:243` `probe()`; `src/cli/review.rs:92-96` probe call; `:98-133` sequential pass loop and exit; `:1-2` exit codes; `src/review.rs:33` `group()`.
- `src/config.rs:66-75` `[review]` defaults: `cursor-agent`, `grok-4.7-high`, 900 s (`:71`), 600 lines (`:72`). `~/.config/factory/config.toml` sets only `[source] path`.
- `cursor-agent --list-models` lists `grok-4.7-high` on 2026-10-14 in 1.3 s: Cursor is logged in today.
- `src/cli/run.rs:93` runs preflight; `:132-136` prints judge lines and emits `JudgeNoted`, which the cockpit does not relay (`src/cockpit.rs:14-29`).
- `src/docs/preflight.rs:87-243` preflight; `ok`/`judge`/`ask`/`refuse` lists; exit 0 pass, 1 refused, 2 asks (`src/docs/mod.rs:71`).
- `src/closeout.rs:134-318` `polish()`; pack build per attempt at `:191-204`; `FINDINGS` parse at `:285-297`. `src/pack.rs:445-519` `for_polish`. `assets/contract-polish.md:25-32` return block.
- `src/leaks.rs:748` `scan`.
- `Cargo.toml`: crate at 0.4.0.
- No `BASH_MAX_TIMEOUT_MS` anywhere in `src/`, `assets/` or `skills/`.
- `skills/polish/SKILL.md:29-62` §1, its override rules at `:40`.
- The `[review]` probe and `review second` are tested against `tests/stubs/cursor-agent` (`tests/review.rs:1,87`).
- Adversarial pass, 2026-10-14: `factory review second --doc` (Cursor, `grok-4.7-high`, 878 s, log `~/.local/share/factory/reviews/20261014-225505`). Thirteen corrections taken, marked above where a claim changed meaning; line cites fixed in place.
- The cut, 2026-10-14: FR-4 and FR-6 retired; their cites (`src/docs/doctor.rs:65-75`, `src/leaks.rs:408-430`, `src/docs/log.rs:195`, the `ignore` crate) moved to the idea.
