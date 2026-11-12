---
number: 017
slug: polish-reviews
title: "Polish reviews that don't waste passes"
type: enhancement
created: 2026-11-05
author: Emiliano Tisato
summary: |
  Polish's reviewers stop wasting passes and stop reporting what they did not do. The security pass reviews the run's own base range. A timed-out second-opinion pass counts as unreviewed. Collect skips tool-written and vendored files, splits a huge file by hunks, sends each pass the feature's settled decisions, and can review only what changed since the last polish. `factory stop` kills running reviewers, the UX pass runs before any test, and autonomy.md gains two rows. Interviewed 2026-11-12.
---

# PRD-017 — Polish reviews that don't waste passes

> Drafted from the 2026-11-05 idea triage, interviewed 2026-11-12 (two rounds plus one follow-up). Two ideas filed on 2026-11-10 and 2026-11-11 were folded in at the interview (§13). `file:line` facts are from `main` at `38f941b`.

## 1. Why now

Polish's reviewers spend hours on code nobody needs reviewed. They also report passes as done when nothing was reviewed. The evidence comes from field retros, mostly one client project's last seven features:

- **Re-argued decisions.** About a third of second-opinion findings re-argued choices already logged as ADs. Each one cost the polisher a `file:line` check to rebut.
- **Wasted passes.** Vendored files took 5 of 25 passes (~50 min) on one feature and 6 of 22 (~1 h) on another, with no finding. PRD-013's pass 1 got `log.jsonl` and `state.toml`, which only the binary writes.
- **Counted passes that reviewed nothing.** icg-cloud PRD-019: 2 of 4 passes hit the timeout, returned no findings and were still counted. The fallback rule never fired.
- **One file too big for a pass.** A ~2,000-line shell script got a pass of its own and returned nothing after its full timeout.
- **A whole-branch re-review.** A third polish on one long feature re-reviewed 187 files in 28 passes (~7.5 h). Two retros asked for this fix.
- **The security pass reviews the wrong range.** Reported in 3 of 7 retros in one project (prokeycloud PRD-024 is one). In each case the agent fell back to a manual review of `main...HEAD`.
- **The UX collector races the test runner.** It shares one lane database and checkout with them, reported in 3 of 7 retros. A test's database refresh wiped data under a live UX drive twice, and its screenshots predate the fixes.

Bugs the owner's runs hit: the counted timeout, the security pass, the UX/test race and the tool-written files. Improvements: settled context, the since-last-review round, the hunk split, stop, and the autonomy rows.

This deliberately leaves alone how many reviewers polish runs and the second opinion's vendor. The UX brief is unchanged too (§12).

## 2. Blast radius

- **`factory review second` exit codes.** A run whose every pass timed out *after streaming text* now exits `2` instead of `0`, so the fallback starts where it used to read findings (FR-2). A timeout with no text already exits `2` (`src/review.rs:554-556`). The `/polish` skill already treats `2` as "fallback owed" (`skills/polish/SKILL.md:68`), so no caller breaks.
- **The collected file's shape.** The driver and `factory live` read it through `collect::passes_and_log` and `collect::reusable` (`src/collect.rs:30-50`). `pack::collected` has its own parser, which keys on the same ` — counted` / ` — unreviewed` suffixes (`src/pack.rs:144`). The plan gains a `skipped:` line, and a later round's `changed:` line names a different range. Pass headings keep their suffix, so both parsers still count them.
- **The polish skill and autonomy.md** ship in the skills bundle. A skill edit needs `FACTORY_BUNDLE_WRITE=1 cargo test` to rewrite `skills/bundle.lock`. Other repos get it only through a release and `factory skills update`.
- **Reviewer cost.** It goes down: fewer passes, no tool-file passes, no re-review of settled code. The settled context makes every pass prompt longer, by the size of the feature's AD titles and its Non-goals section.

## 3. FR-1 — the security pass reviews the run's base range

**Today.** `/polish` §1 says to run `security-review` as its own subagent (`skills/polish/SKILL.md:55`). The built-in skill's prompt runs `git diff --name-only origin/HEAD...`, `git log --no-decorate origin/HEAD...` and `git diff origin/HEAD...` (Claude Code 2.1.316 binary, read 2026-11-12). It compares against the remote's default branch, not the polish base. Six repos on this machine have no `origin/HEAD` (`git rev-parse` fails), and there every one of those commands fails.

> **Corrected 2026-11-12.** The 2026-11-10 idea and prokeycloud PRD-024's retro say the skill "reviews pending (uncommitted) changes only". That is its one-line description, not what it runs. The prompt diffs `origin/HEAD...`, which includes commits. prokeycloud has `origin/HEAD` set, so the empty result there is not explained by recon. The fix below holds for either cause, because it stops depending on the built-in's range at all.

**Decided 2026-11-12.** Polish §1 briefs the security subagent with the range itself:

- The subagent reviews `git diff <base>...HEAD`, with the security lens the paragraph already lists.
- It does not rely on the built-in skill's own git commands.
- The prompt says which range it reviewed. A subagent that reviewed nothing says so, and polish reports the pass as not run.

**Accepted cost:** the pass loses the built-in prompt's exact wording. The lens list at `:55` is what decides the pass's focus, and it stays. **Over:** having collect write a security diff file, which adds Rust and a file for the same result.

**Acceptance.** The polish skill's security paragraph names `<base>...HEAD` as the range. It also tells the subagent not to take the range from the built-in skill.

## 4. FR-2 — a timed-out pass is unreviewed

**Today.** `body()` (`src/review.rs:540-558`) counts a pass killed at its timeout whenever it streamed any partial text. The doc comment reasons "it had the whole timeout". The test `a_timeout_keeps_the_partial_text` (`tests/review.rs:244`) pins exit `0` for it. The default timeout is 1,800 s (`src/config.rs:71`). The retros quote 900 s, which was a user `[review] timeout_secs` override.

**Decided 2026-11-12.**

- A pass that ended `Killed` never counts, with or without text.
- Its partial text stays in its section under `(timed out after N s, partial)`, so a real finding in it is still read and verified.
- In the collected file, its heading ends ` — unreviewed` (`src/collect.rs:190-192`). `review second` prints pass headings with no mark (`src/cli/review.rs:150`), and that stays.
- The plan's `reviewed, n of m passes counted` line counts it out.
- When no pass counted, collect writes `fallback owed` (`src/collect.rs:194-197`) and `review second` exits `2` (`src/cli/review.rs:154`), as today.

**Accepted cost:** a pass that found real defects and then ran out of time triggers a fallback over files it partly covered. That is about 5 minutes of a same-model agent per pass (the 2026-10-29 idea). **Over:** also scaling the timeout with pass size, which adds a formula for one observed case. FR-4 handles that case.

**Acceptance.**

- A stubbed reviewer that sleeps past the timeout after streaming text keeps its partial text, and `review second` exits `2` when that is the only pass.
- With a second pass that finishes, it exits `0`. The stub needs a per-pass "sleep after text" key for this, because `sleep_after_assistant` applies to every pass.
- In collect, the timed-out pass's heading ends ` — unreviewed`, and every pass timing out writes `fallback owed`.

## 5. FR-3 — collect skips tool-written and vendored files

**Today.** `collect()` takes every changed file from `review::numstat(root, base, &[])` (`src/collect.rs:70`) and groups them by line count alone (`src/review.rs:34-70`). `review second --diff` does the same.

> **Corrected 2026-11-12.** The draft listed `events.jsonl` as a tool-written repo file. It is not: it lives in the run directory (`src/events.rs:12`). The tool-written files a feature branch commits are `docs/features/*/log.jsonl` and `docs/features/*/state.toml`.

**Decided 2026-11-12.** Before grouping, both `review collect` and `review second --diff` drop:

1. **Tool-written files:** `docs/features/*/log.jsonl` and `docs/features/*/state.toml`.
2. **Vendored or generated files:** paths that `git check-attr linguist-vendored linguist-generated` reports as `set` or `true` in the checkout under review. The repo owns that list in `.gitattributes`, so factory adds no knob.

The size row (`needs_second_opinion`) counts only the files left after the skip. When none are left, the second opinion is `skipped by size`, and its text names the skipped files instead of "n files under 50 changed lines". The plan always lists skipped files under a `skipped:` line with the reason, so a reader sees what nobody reviewed. The leaks scan is unchanged and still reads every added line.

> **Corrected 2026-11-12.** The first draft said skipped files still count for the size row *and* that a branch with every file skipped gets `skipped by size`. Both cannot hold: two tool files make `needs_second_opinion` true (`src/collect.rs:55`). The size row now counts what is left. The draft also read the attributes "at HEAD". Plain `check-attr` reads the working tree, and `--source=HEAD` needs git 2.40 or later, so the checkout it is.

**Accepted cost:** a vendored file nobody marks in `.gitattributes` still gets passes, and a repo that hits that again adds one line. A branch can edit `.gitattributes` to hide its own files, just as it could edit an exclude list. The `skipped:` line keeps that visible, and PRD-022 owns reading guard files from the base. **Over:** a `[review] exclude` list in `docs/project.toml`, a new knob for what a standard file already says. Also over: tool files only, which leaves the measured vendored waste in place.

> **Corrected 2026-11-12.** The first draft rejected the exclude list because a branch could edit it. `.gitattributes` is just as editable, so that reason did not separate the two. The choice stands on "no new knob".

**Acceptance.**

- A branch changing `docs/features/001-x/log.jsonl`, `state.toml`, a `.gitattributes`-vendored `testdata/a.xsd` and `src/a.rs` sends only `src/a.rs` to the reviewer.
- The plan names the other three under `skipped:`.

## 6. FR-4 — a huge file is split by hunks

**Today.** A file over `pass_lines` (600) gets a pass of its own, marked `over` (`src/review.rs:54-64`), however large. One such pass (~2,000 lines) returned nothing after its full timeout. A `Pass` holds only `files`, `lines` and `over` (`src/review.rs:27-32`). `diff_pass_prompt` writes `git diff <base>...HEAD -- <files>` whole (`:523-534`).

**Decided 2026-11-12.**

- A file with more than 2 × `pass_lines` changed lines is cut at hunk boundaries (`@@` headers of `git diff`) into parts of about `pass_lines` each, one pass per part.
- Each part's diff file holds the file header plus its hunks.
- Its pass heading reads `path (part k of n)`.
- A single hunk larger than the cap is cut at line boundaries into parts of about `pass_lines`, each with its own recomputed `@@` header. A newly added file is one hunk, so this is the case that covers it.
- Files up to 2 × `pass_lines` keep today's behavior.
- `Pass` gains the part it covers, and `diff_pass_prompt` writes only that part's slice of the file's diff.

> **Corrected 2026-11-12.** The first draft kept a hunk over the cap whole. An added file's diff is a single hunk, so a new 2,000-line script, possibly the very case observed, would still have gone in one pass. The owner's choice was "split so the whole file gets reviewed", and cutting oversized hunks at lines is what delivers it.

**Accepted cost:** about 120 lines plus tests, since `Pass` and the diff writer both change. Reviewers lose sight of the other hunks of the same file, but they may still read the whole file from the checkout, as the prompt already allows (`src/review.rs:594`). **Over:** skipping the file and owing the fallback, which leaves it with no cross-vendor review.

**Acceptance.**

- A file on `main` edited in ten places, 2,000 changed lines in all, becomes 4 passes. Each holds whole hunks and about 600 changed lines at most. The `review-n.diff` files show it.
- A 900-line file stays one pass.
- A newly added 1,500-line file becomes 3 parts of at most about 600 lines.

## 7. FR-5 — reviewers know what is settled

**Today.** A pass prompt is the diff path and one instruction (`diff_prompt`, `src/review.rs:592-598`). It carries no decision, slice or non-goal. Polish's triage counts only `OPEN-n` and `docs/decisions.md` as settled (`skills/polish/SKILL.md:89`). The fallback brief is improvised per pass (`:70`).

**Decided 2026-11-12.**

- **`--feature NNN`** on `review collect` and `review second --diff` builds a settled block. It holds each decision in the feature's `log.jsonl` (id, title and its `**Chose:**` line), plus the PRD's Decisions log and Non-goals sections.
- **Every pass prompt** gets that block, with one instruction: a finding that contradicts an entry must name it and say why the entry is wrong.
- **The driver** passes the run's feature (`closeout.rs:197`). Attended polish passes it when the branch is a feature's.
- **No feature, or a section missing**, means no block for that part, never an error.
- **The block is capped at 32 KiB.** The prompt goes to the reviewer as one argv string (`src/review.rs:378`), and Linux limits one argument to 128 KiB. A cut says what it dropped.
- **A superseded AD is left out** (FR-8's `**Supersedes:**` line), so the block never offers two contradicting choices as settled.

The skill half changes two places in `/polish`:

- **§2** treats slice task text, earlier ADs and the PRD's Non-goals as settled, beside `OPEN-n` and `docs/decisions.md`.
- **§1** gains a fallback brief template: the pass's files, the range and the same settled block.

**Accepted cost:** about 60 lines plus skill text, and a longer prompt on every pass. **Over:** skill text only, which leaves the reviewer blind and keeps the rebuttal cost on the polisher.

**Acceptance.**

- With `--feature`, each stubbed pass's recorded prompt contains a fixture AD's title and Chose line, and the fixture PRD's Non-goals text.
- Without `--feature`, the prompt is today's.
- Polish §2 lists the five settled sources.

## 8. FR-6 — a later round reviews what changed since the last one

**Today.** Collect always reviews `<base>...HEAD` (`src/collect.rs:70`, `src/review.rs:85,98`). `reviewed` in a feature's `state.toml` is a date, not a commit.

**Decided 2026-11-12.**

- **`review collect --since <commit>`** reviews `<commit>..HEAD` for the second opinion. The plan's `changed:` line names that range and says it is since the last review.
- **Leaks still scans `<base>...HEAD`**, because it is cheap and its verdict covers the branch.
- **The size row** applies to the since diff.
- **The last-reviewed commit** is the newest one that changed the `reviewed` line of the feature's `state.toml`. One Rust function finds it.
- **The driver** passes it as `--since` when it exists (`src/closeout.rs:197`).
- **Polish §1** does the same attended, and the skill names the git command.
- **The full diff** is reviewed when no such commit exists, or when the owner asks.

> **Corrected 2026-11-12.** The first draft kept the driver out, claiming a resume skips polish once `reviewed` is set and citing `src/closeout.rs:178`. That line is block text. The driver never reads `reviewed`. It skips polish only when the board says shipped, a polish block was answered `proceed`, or this run logged `PolishDone` (`src/closeout.rs:78-82`, `:118`). So a new run on a feature that was already polished would re-review the whole branch, the 7.5 h case. The driver now passes `--since` too.

**Accepted cost:** about 60 lines plus skill text. Code reviewed in an earlier round is not re-reviewed, even where a later fix touched its callers. **Over:** leaving it out, which costs hours on a long feature's third polish.

**Acceptance.**

- On a fixture branch with a commit `R` and two later files, `collect --since R` sends only the two later files to the reviewer.
- The leaks verdict still covers the whole branch.
- With a fixture feature whose `state.toml` gained `reviewed` in commit `R`, the last-reviewed lookup returns `R`, and none returns nothing.

## 9. FR-7 — the UX pass runs before any test

**Today.** `/polish` §1 runs the local review, leaks, security, `/ux` and the second opinion in that order (`skills/polish/SKILL.md:49-63`). It never orders test runs against the UX drive. In a lane, the app and the test suite share the lane's database (`docs/reference/lanes/` pattern, and the 2026-11-10 idea §2).

**Decided 2026-11-12.** Polish §1 says:

1. Start `/ux` first, before anything that runs tests.
2. Run no test and no fix in that checkout until the UX pass returns. The local review, security pass and second opinion are read-only, so they run meanwhile.
3. Fixes (§5) start after the UX pass returns.

**Accepted cost:** fixes wait for the UX pass, about 15 minutes on a UI feature. **Over:** its own worktree and database, a new lane resource with seeding and teardown, a slice or two.

**Acceptance.** The polish skill orders the UX pass first. It forbids test runs in the checkout until the pass returns.

## 10. FR-8 — two autonomy rows

**Today.** `assets/autonomy.md`'s test (`:17-25`) and Mapping (`:30-40`) have no row for two cases:

- A later phase's AD that overturns an earlier AD. A worker wrote a new fyi AD and left the old one standing.
- An Ask whose fix changes a contract while today's behavior is safe to ship. It reads as a block, though nothing waits on it.

**Decided 2026-11-12.**

- **An AD that overturns an earlier AD** is allowed. Its body carries a `**Supersedes:** AD-n` line, which FR-5's settled block reads to drop the old one. The log stays append-only.
- **A contract-changing Ask whose current behavior is safe** is parked with `factory docs idea add` for the owner, and never blocks the run.

**Accepted cost:** about 4 lines. **Over:** leaving workers to guess.

**Acceptance.** `assets/autonomy.md` holds both rows, and the overturn row names the `**Supersedes:**` line.

## 11. FR-9 — `factory stop` kills a running collect's reviewers

**Today.** The driver runs `collect::collect` in-process before the polish worker (`src/closeout.rs:197-209`). It checks `stop_requested` only after the collect returns. `run_logged` (`src/review.rs:224-259`) polls each reviewer with no cancel, so reviewers run up to `timeout_secs` (1,800 s) after a stop. No run on record has stopped during a collect. The owner kept this on 2026-11-12 regardless.

**Decided 2026-11-12.**

- `run_passes` takes a cancel check. The driver supplies one that reloads the run's `stop_requested`. The CLI verbs pass none.
- `run_logged`'s poll loop kills the pass's process group when the check fires.
- That pass reads `unreviewed (stopped)`, and the driver returns `Stopped` as today.
- The check is read at most once a second, not every 50 ms poll.

**Accepted cost:** a callback through `collect`, `review::run_passes` and `run_logged`, plus a test. **Over:** leaving reviewers to run out their timeout.

**Acceptance.** In a fixture run, `factory stop` during a collect whose stubbed reviewer sleeps leaves no reviewer process alive within a few seconds. The collected file names the pass `unreviewed (stopped)`.

## 12. Decisions log

- ~~OPEN — security pass shape~~ → **brief it with the range** (2026-11-12, FR-1).
- ~~OPEN — timed-out pass~~ → **unreviewed, partial kept** (2026-11-12, FR-2).
- ~~OPEN — what collect skips~~ → **tool files plus `.gitattributes` vendored/generated** (2026-11-12, FR-3).
- ~~OPEN — UX vs tests~~ → **ordered in the skill** (2026-11-12, FR-7).
- ~~OPEN — settled context~~ → **prompt plus skill** (2026-11-12, FR-5).
- ~~OPEN — later rounds~~ → **`--since` flag** (2026-11-12, FR-6).
- ~~OPEN — autonomy rows~~ → **both added** (2026-11-12, FR-8).
- ~~OPEN — huge file~~ → **split by hunks** (2026-11-12, FR-4).
- ~~OPEN — extras~~ → **kept the huge-file cap and stop. Cut the UX brief and docs-only passes** (2026-11-12, §15).
- ~~OPEN — the cut~~ → **kept all four post-review additions**: FR-9 stop, the driver's `--since`, splitting an added file by lines, and the `Supersedes` line (2026-11-12).

## 13. QA plan

**Gate:** `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test && claude plugin validate mods/cockpit && claude plugin test mods/cockpit`.

**Surface:** the `factory review second` and `factory review collect` CLIs, the driver's polish phase, and the text of `skills/polish/SKILL.md` and `assets/autonomy.md`. There is no UI, so every FR is proven below the UI.

**Harness:** `tests/review.rs`, with its stub `cursor-agent` (`tests/stubs/cursor-agent`, scenario JSON via `scenario()`), and its `fixture`, `branch`, `collect` and `collected` helpers. FR-9 also uses `tests/harness/mod.rs`'s run fixture.

**Seed state:**

- Fixture git branches built by `branch()`. It creates no parent directories (`tests/review.rs:30`), so nested paths need a helper change.
- A `.gitattributes` marking a vendored path, committed on `main` so it is not itself a changed file.
- A fixture feature folder with a `log.jsonl` holding two decisions, one superseding the other, and a `prd.md` with Decisions log and Non-goals sections.
- A file committed on `main`, then edited in ten places on the branch for 2,000 changed lines. A newly added 1,500-line file.
- A per-pass "sleep after text" key in the stub `cursor-agent` scenario.

Tests read fixtures, never this repo's `docs/`.

| FR | Proof | Evidence | Human? |
|---|---|---|---|
| FR-1 | run `polish_security_pass_reviews_the_base_range`: a skill-text test that the security paragraph names `<base>...HEAD` and tells the subagent not to take the built-in's range | test transcript | no |
| FR-2 | run `a_timeout_is_unreviewed_and_keeps_its_partial_text` (replaces `a_timeout_keeps_the_partial_text`), `a_timeout_beside_a_counted_pass_exits_0`, `collect_owes_the_fallback_when_every_pass_timed_out` | test transcript | no |
| FR-3 | run `collect_skips_tool_written_and_vendored_files_and_lists_them` | test transcript | no |
| FR-4 | run `a_huge_file_splits_at_hunks`, `a_file_under_twice_the_cap_stays_whole`, `an_added_huge_file_splits_at_lines` | test transcript | no |
| FR-5 | run `feature_puts_settled_decisions_in_every_pass_prompt`, `a_superseded_decision_is_left_out`, `no_feature_keeps_the_prompt_as_today`, `polish_lists_five_settled_sources` | test transcript | no |
| FR-6 | run `collect_since_reviews_only_later_commits`, `collect_since_still_scans_leaks_on_the_whole_branch`, `last_reviewed_commit_is_the_one_that_set_reviewed` | test transcript | no |
| FR-7 | run `polish_starts_ux_first_and_holds_tests` | test transcript | no |
| FR-8 | run `autonomy_has_the_overturn_and_safe_contract_rows` | test transcript | no |
| FR-9 | run `stop_kills_a_running_collects_reviewers` | test transcript | no |

No row needs a human, so there is no `qa-guide.md`. Whether the settled block actually lowers re-argued findings is measured on the next harvest's retros, not here.

## 14. Slices

Five slices, sliced 2026-11-12.

- **01, honest passes (FR-2, FR-3).** A timed-out pass is unreviewed, and tool-written and `.gitattributes`-vendored files are skipped and listed. It goes first: it fixes the two field bugs and every later Rust slice builds on its file list.
- **02, split a huge file (FR-4).** This is the riskiest slice, because it parses diffs. It depends on 01, since both change what reaches `group` and the `second()` flow.
- **03, settled decisions and since-last-review (FR-5, FR-6, Rust and driver).** It depends on 02, because it threads a range through the `diff` and part fetch 02 just changed. It also introduces the collect options struct.
- **04, stop kills reviewers (FR-9).** It depends on 03, because it adds the cancel check to that struct.
- **05, polish skill and autonomy text (FR-1, FR-7, FR-8, and the skill halves of FR-5 and FR-6).** It is independent, since text compiles against nothing.

01 and 05 build in parallel, then the 02 → 03 → 04 chain. Checkpoints fall every 2 done slices: after 01 and 05, after 02 and 03, and a final one after 04. Each Prove task is proven below the UI by running its named tests.

Workflow: one branch, `feat/polish-reviews`, one PR, one commit per slice. No QA row needs a human, so there is no `qa-guide.md`.

## 15. Non-goals

- **The grep hook refusing a whole batched command** (idea 2026-11-10) is PRD-019's (`grep-hook-and-ctx`), not polish's.
- **Parallel polish fixers, each in its own lane**, stays an idea.
- **Polish path patterns as a reviewer floor.** Dropped 2026-11-05: no polish on record skipped a pass it needed.
- **Splitting the polish worker into driver steps.** Dropped 2026-11-05: `review collect` took the mechanical half.
- **Not handled: a total wall-clock budget for `review second`** (2026-11-12). Passes already run at once (`src/review.rs:416-475`), and the driver's collect runs in-process, so no harness time limit kills it. Reopen if an attended background run is cut short again.
- **Not handled: a timeout that scales with pass size** (2026-11-12). FR-2 makes a timeout honest and FR-4 shrinks the one case seen. Reopen if passes under 2 × `pass_lines` keep timing out.
- **Not handled: a `[review] exclude` list in `docs/project.toml`** (2026-11-12). It is a lever a branch could use to hide its own files, and `.gitattributes` covers the cases seen.
- **Not handled: the UX pass's own worktree and database** (2026-11-12). FR-7 orders it instead. Reopen if the 15-minute wait for fixes shows up in retros.
- **Not handled: a UX brief that starts from checkpoint evidence and lists states up front** (cut by the owner 2026-11-12). Evidence: one 27-minute pass missed three states, and one run saved time from evidence. Reopen on a second retro.
- **Not handled: docs-only passes in their own section** (cut by the owner 2026-11-12). Evidence: one project. Reopen if doc findings again crowd code triage.
- **Not handled: `/prune` exempting browser files already run green in the same polish, a formatter-scope check, and a copy-only UX check** (idea 2026-11-11, side notes, 2026-11-12). Each was seen once, with no evidence of recurrence.

## 16. Verification notes

Checked against `main` at `38f941b` on 2026-11-12. Re-verify at slice entry.

- `src/collect.rs:70`: `review::numstat(root, base, &[])`, every changed file, no filter.
- `src/collect.rs:187-201`: one pass heading per pass, ` — counted` / ` — unreviewed`, and `fallback owed` when `counted == 0`.
- `src/collect.rs:30-50`: `reusable` and `passes_and_log` parse the head line, the state lines and the pass headings.
- `src/review.rs:27-32`: `Pass` holds `files`, `lines` and `over`. `:34-70`: `group` by line count, with an oversized file alone (`:54-64`).
- `src/review.rs:84-107`: `numstat` and `diff` hard-code `{base}...HEAD`.
- `src/review.rs:224-259`: `run_logged` polls every 50 ms and kills the process group on timeout only. No cancel.
- `src/review.rs:416-475`: `run_passes` starts every pass at once. `:523-534`: `diff_pass_prompt` writes each pass's whole-file diff. `:378`: the prompt is one argv string.
- `src/review.rs:540-558`: `body`, where a `Killed` pass with partial text counts.
- `src/review.rs:592-598`: `diff_prompt`, with no settled context.
- `src/config.rs:32-37,71-72`: `[review]` has `agent`, `model`, `timeout_secs` (1800) and `pass_lines` (600). It is user config only, and `docs/project.toml`'s top keys exclude `review` (`:126`).
- `src/closeout.rs:197-209`: the driver's collect, then a stop check after it returns. `:78-82`, `:118`: polish is skipped only when shipped, `proceed` answered, or `PolishDone` logged. `reviewed` is never read.
- `src/cli/review.rs:150`: `review second`'s pass headings carry no counted mark. `:154`: exit `0` if any pass counted, else `2`.
- `src/pack.rs:144`: `pack::collected` parses pass headings by the same suffixes.
- `src/worker.rs:93-95`: only `build`, `fix` and `merge` workers lose skills (`--disable-slash-commands`). The polish worker can invoke `security-review` and `code-review`.
- `src/events.rs:12`: `events.jsonl` is in the run dir, not the repo.
- `skills/polish/SKILL.md:55`: the security paragraph. `:57-59`: `/ux`. `:63`: plan line reading. `:70`: fallback. `:89`: the settled rule.
- `assets/autonomy.md:17-25`: the test. `:30-40`: the Mapping.
- Claude Code 2.1.316's `security-review` prompt runs `git diff origin/HEAD...`. Six local repos lack `origin/HEAD`, and prokeycloud has it.
- `tests/review.rs:244`: `a_timeout_keeps_the_partial_text` pins today's counted timeout, and FR-2 replaces it.

## 17. Source ideas

The full text of each idea this PRD absorbed. The idea files are deleted.

### Polish treats only OPEN-n and decisions as settled, so reviewers re-argue slice text and earlier ADs

_added 2026-10-19, origin: factory upstream_

##### What happened

Across three shipped features, `/polish` reviewers and the `/ux` collector raised findings that the feature's own docs had already settled:

- **Slice text.** A UX pass asked about two strings that a slice's tasks wrote out verbatim. `/polish` § 2 counts only `OPEN-n` and `DEC-n` as settled, so the worker had no rule for it. It treated the slice text as settled anyway and wrote no AD.
- **Earlier ADs.** Five of seven UX asks re-argued choices that the PRD, a slice or an earlier AD had pinned. The UX collector does not read the slice files or the AD list, while `/polish` § 0 hands both to triage.
- **PRD non-goals.** A second-opinion pass re-raised a race the PRD's non-goals section had already accepted.

Two cases have no row in `autonomy.md` at all:

1. **A later phase overturning an earlier AD.** A polish finding asked for exactly what an implementation AD had rejected (a 500 for a malformed id). `autonomy.md` covers an AD against the PRD or a DEC, not an AD against an earlier phase's AD. The worker wrote a new fyi AD citing the old one and left the old one standing.
2. **An Ask whose fix changes the contract while today's behaviour is safe to ship.** Refusing a disabled record needed a new error reason in the public code table. Under the autonomy test that is a block, but nothing waited on it. The worker parked it as an idea. The Mapping table has no row for this.

##### What it should do

- Treat slice task text, earlier ADs and the PRD's non-goals as settled, beside `OPEN-n` and `DEC-n`. A finding against one of them is rebutted with a citation, not re-litigated.
- Give the `/ux` collector the same context `/polish` § 0 gives triage: the slice contracts and the AD list.
- Add an `autonomy.md` row for a later AD that overturns an earlier one: allowed, cite the overturned AD, and mark the old one superseded.
- Add a Mapping row for a contract-changing Ask when current behaviour is safe: park it as an idea for the owner, and do not block the run.

### /polish second opinion: send the feature's decisions log with each pass

_added 2026-11-05, origin: factory upstream_

`/polish` § 1 sends the second-opinion reviewer the diff without the feature's decisions log. In one project, about a third of the reviewer's findings across several features re-argued choices already logged as `AD` entries, and each one cost the polisher a verification at `file:line` to rebut. § 2 already names this as the most common false-positive class.

Should: pass `factory log show --decisions` (or the decisions section of the plan) with each pass, with an instruction that a finding contradicting a logged decision must name the decision and say why it is wrong.

### friction: cody-agent PRD-026

_added 2026-10-15, origin: cody-agent PRD-026 run 026-20261015-da46_

- `polish` § 1, second opinion: the skill says to read the exit code of `factory review second`, but a run longer than the harness's background limit never returns one. Passes 5 and 6 were simply never reviewed, and only reading the log showed it. The step needs a per-pass budget, or a rule to launch the fallback the moment a pass times out instead of waiting for the run.
- `polish` § 1: the fallback prompt has to list everything already fixed or settled by the PRD, or the fallback re-reports it. A template for that brief would save rewriting it per pass.

A comment or name the leaks check should have caught: none found.

From cody-agent `docs/features/026-product-pulse/retro.md`

### polish §1: reviewer passes overrun, count as done when they are not, and re-review settled code

_added 2026-10-29, origin: factory upstream_

Three retros from one project's last four features report the same family of problems with the reviewers `polish` §1 runs.

**1. Second opinion: a flat per-pass timeout, and a timed-out pass still counts.** Passes are sized by changed lines, but the timeout is a flat 900 s. In one run, 6 of 12 passes timed out with partial output and no findings, including the passes over the newest middleware and a public endpoint; the plan line still read "12 of 12 passes counted". In another, 2 of 6 passes timed out and the last 2 never ran, because the whole background run hit the harness's time limit, so `factory review second` never returned an exit code to read. Only the log showed it. Meanwhile a same-model fallback did each missing pass in about 5 minutes.

What should happen:
- A pass that timed out does not count as reviewed, and the plan line says so.
- The fallback starts the moment a pass times out, not after the run ends.
- `factory review second` takes a total wall-clock budget and says up front which passes will not fit, instead of being killed mid-pass.
- Either the timeout scales with the pass's size, or the pass cap shrinks to fit the timeout.

**2. A later polish re-reviews the whole branch.** A third polish on one long feature re-reviewed all 187 files in 28 passes (about 7.5 h). Most of its claims on earlier slices repeated decisions an earlier polish had already settled or parked. This is the second retro to ask: a later polish should default to the commits after the last `reviewed` date, and review the full diff only on request.

**3. Reviewer briefs.**
- The UX pass ran 27 minutes and still left three states of one page uncaptured, named only in its last message. Its brief should list the states it must capture up front, and ask it to report which it could not reach.
- The fallback's prompt has to list everything already fixed or settled by the PRD, or it reports them again. A template for that brief would save writing it per pass.
- Running the UX pass first, in its own lane database, would let targeted tests run alongside it instead of waiting for it (the skill forbids a test run while a subagent drives the app).

### review collect: skip vendored files, separate docs, never count an empty pass

_added 2026-11-05, origin: factory upstream_

`factory review collect` (the `/polish` § 1 second opinion) splits a branch's diff into passes by line count alone. Across four features in one project this cost hours of reviewer time for no finding:

- **Vendored files get passes of their own.** A branch that adds third-party XML schemas byte for byte (`testdata/**/*.xsd`) or captured vendor manuals as text spent 5 of 25 passes (~50 min) and, on another feature, 6 of 22 passes (~1 h) on them. Every finding was about the vendor's own schema, which the repo must not change.
- **Docs passes return claims nobody verifies.** Passes over operations docs produced findings that could not be checked from the code, mixed into the same triage as code findings.
- **Size by line count misjudges dense files.** One pass got a single ~2,000-line shell script, ran its full 30-minute timeout, and returned nothing.
- **A timed-out pass with no text is counted.** The plan line said "11 of 11 passes counted" while pass 1 reviewed nothing, so the fallback rule for unreviewed code never fired.

Should:

1. Skip vendored paths by default (`**/testdata/**/*.xsd`, and similar), and honour an exclude list such as `[review] exclude = [...]` in `project.toml`.
2. Put docs-only passes in their own lower-priority section, or out by default.
3. Size passes by something closer to complexity than raw lines, or cap a single file's pass.
4. Mark a pass that times out without findings text `unreviewed`, not `counted`, so the fallback rule applies.

Seen again in PRD-013 (harvest 2026-11-05): pass 1 got `docs/features/013-live-status/log.jsonl` and `state.toml`, files only the binary writes, and spent most of its time on docs. Tool-written files (`log.jsonl`, `state.toml`, `events.jsonl`) belong in the default exclude.

### factory stop should kill a polish collect's reviewers

_added 2026-10-15, origin: PRD-007 polish_

The driver runs `collect::collect` in-process before the polish worker spawns (`collected` in `src/closeout.rs`). PRD-007's polish added a stop check after it, so no worker spawns after a `factory stop`, but the collect itself keeps going: its `cursor-agent` passes are not workers, `factory stop` does not see them, and they run until `[review] timeout_secs` (900 s) ends them.

Shape: `run_passes` (`src/review.rs`) takes a cancel check (the run's `stop_requested`, reloaded) and its `run_logged` poll loop kills the pass's group when it fires, the pass reading `unreviewed (stopped)`. Cost: a callback through `collect`, `review::run_passes` and `run_logged`. Worth it once a stop during a long collect is seen in a real run.

### /polish UX pass: no shared checkout with the fixer, and start from checkpoint evidence

_added 2026-11-05, origin: factory upstream_

Two problems with the UX collector in `/polish` § 1:

- **It drives the app from the same checkout the fixer edits.** The skill forbids reviewers from moving HEAD but says nothing about the fixer editing files a reviewer is building from. In one run the UX collector rebuilt the stack mid-pass and reported the polisher's own in-progress edits as "someone else's changes".
- **It re-captures states the checkpoints already captured.** The checkpoints' Prove tasks held screenshots of every state. Telling the UX pass to start from the evidence directory and drive only the missing states saved most of its time (~8 min instead of much longer); the skill does not say to.

Should: give the UX collector its own worktree (or make § 5 fixes wait until it finishes), and tell it to start from the checkpoint evidence directory and drive only what is missing.

### /polish: the security pass cannot run on a committed branch, and the UX pass shares the test database

_added 2026-11-10, origin: factory upstream_


Two steps of `/polish` §1, as written, do not work in a `factory` run's lane. Both were reported in three of seven feature retros in one project.

##### 1. The security pass names a skill that only reads uncommitted changes

`/polish` asks for the security pass by running the `security-review` skill as a subagent. That skill reviews pending (uncommitted) changes. By polish time every slice is committed on the feature branch, so the skill finds nothing to review. In each case the agent noticed and fell back to a manual review of `main...HEAD`, but the skill text never says to.

**Should.** Tell the security pass to review the branch diff (`git diff main...HEAD`), or pass that range to the security skill explicitly. Say what to do when the skill cannot take a range.

##### 2. The UX pass and the test runner share one database in a lane

A lane worktree points its app and its test suite at the same database (the lane's `DB_DATABASE`; the project's separate test database file is not seeded into lanes). The UX pass drives the served app while the polish agent runs scoped tests. A test's database refresh wiped the data under a live UX drive twice; once the UX agent moved to a scratch database to finish, which other rules forbid. Another time, polish simply waited for the UX pass before testing.

**Should.** Order the steps: run the UX pass before any fix that needs tests, and run no tests while it drives. Or give the UX pass its own lane or database, and say which one the served app reads. The lanes skill could record whether a lane's app and tests share a database, so polish knows which ordering applies.

##### Minimal shape

```
polish §1: security → security-review skill → "no pending changes" → manual fallback (undocumented)
polish §1: UX drive on lane DB ‖ pest --filter … (RefreshDatabase on lane DB) → drive data gone
```

### /polish and /prune: a browser-driving collector and test runs contend for one checkout

_added 2026-11-11, origin: factory upstream_


**What happens.** Polish runs its collectors, then fixes. The UX collector drives the real app for about 15 minutes on the lane's database. Meanwhile any test run in the same checkout races it: the collector reports "the working tree changed under me", its screenshots predate the fixes, and tests can hit missing tables. Prune separately insists on one test process per checkout. The two skills are compatible only if you read both side by side. In one run the session polled `pgrep` in a loop to learn when the collector released the lane, and the pattern matched its own command line twice before it was bracketed.

**What it should do.**
- Polish §1 should say: start the UX collector first, and run no tests in that checkout until it returns. Or give factory a lane lock or a "subagent released the lane" wait primitive.
- Prune's PR mode reruns scoped files three times. It could exempt browser files already run green in the same polish, since about 3 × 70 s per browser file set on a flaky host buys little.

**Two smaller polish notes.**
- Polish has no way to tell whether a formatter call is in the project's declared format scope. A vendored `npx prettier --write` on an undeclared directory reformatted hundreds of lines before it was noticed.
- The UX row costs a full browser capture even for a copy-only diff. Its finds there came from reading the code beside the copy, so a "copy accuracy vs code" check would be cheaper for copy-only changes.
