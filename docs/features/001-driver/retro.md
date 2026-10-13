---
score: 4
gate_runs: 29
gate_red: 6
gate_minutes: 16
ci_cycles: 1
slices: 13
checkpoints: 7
build_minutes: 627
polish_minutes: 157
out_tokens: 3772425
cache_read_tokens: 229975490
start_context: 55259
peak_context: 182520
---

# Retro — PRD-001

Written by `/deliver` in the closeout commit, from the agent's side of the run.

Gate numbers come from the `CP-1`..`CP-7` entries in `agent-log.md` (17 runs, 5 red, about 5 min) and from polish (12 runs, 1 red, about 11 min). The inner loop is counted separately below. Measured keys come from `measure.py --retro` over session `f2d0ea47`. The slicing ran earlier, in session `9b8e157d`, and is not in these numbers.

## Cost vs catch

| Step | Cost | Caught | Verdict |
|---|---|---|---|
| Inner loop, per slice (`cargo test`) | 83 runs, 46 red, about 27 min | Real bugs: zero-padded AD/BLOCK seeds (slice-03); the stub read a quoted Resolution as an answer (slice-04); an orphaned checkpoint worker got no Traps line (slice-06); the `scheduler::finish` event-vs-phase race (slice-10); the harness `Fixture::calls()` race (slice-08/09); the no-merge scan tripping on a deny literal (slice-13). Most other reds were the builder's own test expectations. | tune: add `cargo fmt --check` and clippy. The inner loop missed both, and CP-2 paid 4 red runs for it. Here the full gate takes about 35-60 s, so the inner loop can simply be the gate. |
| Checkpoints (full gate) | 17 runs, 5 red, about 5 min | CP-2: fmt on 11 files plus 7 clippy sites, which builders had not run. CP-5: its own fmt. No application defect. | keep: about 20 s a run; the fmt/clippy catch moves to the inner loop. |
| Prove, at checkpoints | real Haiku workers twice (about $0.12), a clean checkout (12.7 s), `ctx` timing on two owner repos | CP-6: `def` signature on PHP attributes (AD-9003). CP-7: the hook hint and pack name a bare `factory`, which exits 127 from an uninstalled build (K1). | keep: both were real and invisible to the stub harness. |
| Polish — local review | 2 opus agents, about 24 agent-min | 24 findings and 18 notes. 2 HIGH: `factory answer` bound a BLOCK id to the wrong feature, and the death cap counted the whole run. Also: a resume never checked the branch, so commits could land on `main` (L12); a bare `git commit` swept the whole index (L3). | keep |
| Polish — security pass | 1 opus agent, 8 min | S1, the most important finding of the run. Workers could `gh pr ready/close/merge` through REST or GraphQL with the owner's own `gh` credentials, because the deny list covered only `gh pr merge*`. | keep |
| Polish — UX pass | skipped | no screens, and no `verify-*` harness | n/a |
| Polish — second opinion (Cursor `grok-4.7-high`) | 9 passes; 3 hit the 900 s timeout (about 45 min lost) and were re-scoped | About 55 claims, about 40 beyond the locals. Highlights: the CI watch read a stale suite after a fix push; `timed_out` and `startup_failure` were treated as fix rounds; answers could not be retried; a guessed quota pause outlived a reported reset. Independently agreed with security on the missing default-branch push guard. | tune: every pass that timed out covered 1000+ diff lines across two modules. Cap a pass at about 600 lines or one module, up front. |
| Polish — fixes (5 fixers, 3 in detached worktrees) | 12 gate runs, 1 red, about 11 min | The merge gate caught one interaction: the driver-core branch guard parked silently before the closeout's push guard could block. Each fixer was green alone. | keep: detached worktrees with their own `CARGO_TARGET_DIR` made 4 parallel fixers safe under `lanes: 1`. |
| test-cleanup (pr mode) | 3 gate runs, 3 min | Removed 2 duplicate unit tests and consolidated 1 (30 hook cases moved into the `tests/hook.rs` tables); 256 → 253 tests | keep |
| Builders pinned to `fable` | slices 01-09 and CP-1..4 ran on Fable after the session had moved to Sonnet, then Opus | nothing: it only burned the owner's Fable weekly quota | cut: fixed for this owner by a memory entry; see Framework friction. |

## Where I guessed

- **PRD FR-1 vs `/deliver` (BLOCK-201).** "`judge[]` parks the run" parked every run, the fixture's included, which contradicts an unattended delivery. I blocked. The owner answered "Deliver should be unattended", and FR-1/FR-7 were corrected inline.
- **`polish` § 5 with `lanes: 1`.** It says no test suite runs in a fixer. The suite isolates every fixture with `FACTORY_DATA_DIR`, so I gave each fixer a detached worktree and its own target dir, and let it run the full gate. I guessed that a worktree counts as isolation.
- **AD grading (`autonomy.md` § The test).** "A behaviour chosen where the slice was silent" (fyi) and "a public interface shape the PRD did not specify" (decide) overlap for a CLI, where every flag and exit code is public. I regraded AD-604, AD-607 and AD-1101 up to `decide`. 31 of 145 are `decide`, which is still more than the owner can comfortably review.
- **Fragment shape.** The builder contract and the polish fixer prompt never say a fragment must start with a `## ` heading. `docs log merge` refused a title line at CP-4, and again refused two fixer fragments' prose preambles at the polish merge. Both times I stripped the lines by hand.

## What I wished existed

- `docs log merge` that tolerates a `#` title line and prose above the first section, or a fragment template the contracts point to.
- A heartbeat in the Cursor stream-json for "still reading" vs "stuck", so a 900 s timeout is not the first signal.
- `measure.py` finding polish's end without depending on the literal `docs.mjs ship` string in the transcript.

## Project friction → `/guardrails audit`

- **The inner loop is `cargo test` only.** CP-2's 4 red runs were all fmt and clippy, which no builder had run. From slice-05 on, builders ran fmt and clippy by hand, a Trap, and every later checkpoint was green on the first run. Declare the inner loop as the full gate, since it takes under a minute here.
- **`gate_parallel` was never measured** (a preflight `judge` line). With `lanes: 1` it cost nothing this run. It matters before the first `lanes: 2` run.

## Framework friction → skills repo

- **`feature-docs/models.md` § The four roles, and `deliver` § 2.** "`fable` when it is the session model" was read once at preflight, and the pin outlived the session model. It should be re-checked at every spawn. The owner noticed it in the quota meter.
- **`deliver` § 2 contracts and `polish` § 5.** Say "your fragment starts with `## Decisions`", or have `docs log merge` accept a title.
- **`polish` § 1 and `models.md` § Second opinion.** "~30 files or ~2000 changed lines" is too loose for `grok-4.7-high` at 900 s. Three passes of 1000-1400 lines timed out with nothing. Use about 600 lines or one module per pass.
- **`polish` § 5.** Write down the worktree recipe for parallel fixers when `lanes: 1` (detached HEAD, own target dir, cherry-pick, then one merge gate). The merge gate is not optional: it caught a red that each fixer's own green gate could not see.

## Score

4. The run built 13 slices and got through polish with one owner answer. The review layers caught about 100 real findings that seven green checkpoints had not, including a worker sandbox hole. Lost: about 45 min of Cursor timeouts, and a model pin that burned the wrong quota.

**Owner review (2026-10-11):** 73 kept, 5 changed (AD-401, AD-503, AD-904, AD-10001, AD-40008). DEC candidates: 3 of 4 recorded, as DEC-1 to DEC-3.

## FR-11 — A/B, icg-cloud PRD-018 · 2026-10-11

Run A was `/deliver 018`, PR [#24](https://github.com/ICG-Argentina/icg-cloud/pull/24). Run B was `/factory-deliver 018`, run `018-20261011-6490`, PR [#25](https://github.com/ICG-Argentina/icg-cloud/pull/25). Both started from base `cde31ac` on Opus 5.5. The setup, the owner's answers and the timeline are in [`ab/runs.md`](ab/runs.md). This section holds the results the QA guide asks for. arca-api, the second project, ran on 2026-10-12; its results are in the next section.

### The sixteen answers

The owner answered before seeing any number: run A at 16:04, run B at 19:31.

| # | question | run A | run B |
|---|---|---|---|
| 1 | Does the result do what the PRD asked? | yes | yes |
| 2 | Merge after normal review, at most small fixes? | yes | yes |
| 3 | Decisions you would overturn | 2–3 | 0 |
| 4 | Code quality against the project's standard (1–5) | 5 | 5 |
| 5 | Tests prove the FRs, or only pass? | prove | prove |
| 6 | Built but not asked for | none | none |
| 7 | Asked for but missing | none | none |
| 8 | Overall | ship after minor fixes | ship as is |

After the questionnaire, the owner's hands-on QA rated B slightly better: it "added some small conveniences that A missed". A blind review of both PRs then went the other way, 32 to 25 for A over 8 dimensions (details in `ab/runs.md` § Blind PR review). B won only frontend and UX. B's confirmed defects:

- It quotes an uncapped exception line with its file path on failed-job rows. B followed its uncorrected PRD; A's polish caught the same leak.
- AD-101's fixture deletes broke two browser tests that CI does not run.

The owner's grades stand as given.

### Numbers

Weighted as `quota.rs` does: input + cache write + 0.1 × cache read + 5 × output. Run A comes from `measure.py` over 22 transcripts. Run B comes from `factory report --cockpit`.

| | run A | run B |
|---|---|---|
| input | 1,888 | 684 (workers) |
| cache write | 3,759,673 | 1,313,783 + cockpit 74,504 |
| cache read | 90,308,357 | 32,168,158 + cockpit 4,553,507 |
| output | 671,386 | 309,481 + cockpit 17,844 |
| **weighted** | **16,149,327** | **6,697,917** (workers 6,078,688, cockpit 619,229) |
| orchestrator or cockpit | 2,178,976 (13%) | 619,229 (9.2%) |
| launch to draft PR | 3h 07m (12:09–15:16) | 3h 01m (16:06–19:07) |
| full-gate runs, red, minutes | 6, 0, 42 | 6, 0, 42.7 (3 of them repeats; see below) |
| inner-loop runs, red | not counted | 11, 0 |
| CI cycles | 0 | 0, but its first green was on an incomplete branch |
| owner interrupts after launch | BLOCK-101, plus one unprompted steer | BLOCK-1 and BLOCK-2, plus two calls on the leftovers. The cockpit answered BLOCK-3 itself. |
| `ctx_share` · `repeat_rate` · `followup_reads` | n/a | 0.073 · 0.003 · 7 of 21 (33%) |

Each tool reports build minutes on its own ruler: 115 for A from `measure.py`, 44.4 worker-minutes for B from `factory report`. They are not comparable, so the table uses launch to draft PR instead.

### Pass criteria (QA guide step 7)

| criterion | result |
|---|---|
| `weighted_tokens(B) ≤ 0.6 × weighted_tokens(A)` | **pass**: 41.5%, 2.99M under the 9,689,596 bar |
| `cockpit_tokens(B) ≤ 5%` of `weighted_tokens(B)` | **fail**: 10.2% of the workers' figure, 9.2% of the total |
| run B's draft PR has CI green | **pass**, twice. The first green (7m53s) covered a branch missing four polish files; the second, on `806992c` after the cockpit's repair, covered all of it. |
| questionnaire (step 5) | **pass**: B matches or beats A on every question |

**The cockpit share fails, so this project is a `BLOCK` for the owner to judge** (step 7: "not a reason to rerun quietly"). My read: most of the cockpit's work was repair the driver should not have needed. It found the uncommitted leftovers twice, made three repair commits, and relayed three blocks, all caused by the fragment-title slip. Those causes are fixed below. My recommendation: record icg-cloud as a fail on this criterion and re-measure on arca-api with the fixed binary. Do not change the 5% bar. **Owner's call (2026-10-11): accepted.** icg-cloud is recorded as a fail on the cockpit share; arca-api re-measures it with the fixed binary, under the same 5% bar.

**The shipped branch is not the driver's alone.** Three repair commits made PR #25 whole: `37c30c0` and `6d0f223` by the cockpit, and `615fda5` by the cockpit after `run_finished`. Without the first, the branch would not have built. The first CI green came before the third, so it covered a branch without two review fixes its PR body claimed.

### Side checks

- **FR-8 cross-check: pass.** `measure.py` on the cockpit session gives 619,075 weighted against the report's 619,229, a gap of 0.025%, and the peak context matches exactly. One gap: `factory report` prints only `cockpit_tokens` and `cockpit_peak_context`, so the four figures the guide names cannot be compared one by one.
- **Slice-13 ranged-read check (AD-1304): pass.** No "file has not been read" error and no retry loop. Three of six builders wrote their slice file through a `python3` heredoc and never tried `Read`. That habit is the same one that lost slice 02's files.
- **FR-12 reversal rule: both triggers fired, on one project and 21 `ctx` calls.** `followup_reads` is 33%, over 30%, so the rule says to widen the answers "before anything else". `ctx_share` is 7.3%, under 10%, which would shrink `ctx` to `outline`. `repeat_rate` fell from 26.7% to 0.3%, so that trigger did not fire. Following the rule's own order, the first step is widening. The shrink waits for arca-api's numbers, because 21 calls are too few to cut a verb on. **Owner's call (2026-10-11): accepted.** Widen first; the shrink is decided on arca-api's numbers. **Reversed (2026-10-12):** no widening. The 7 follow-ups were reads of neighbouring methods and callees, which wider answers would not save. `factory report` splits `followup_reads` by worker role instead, and arca-api decides on the builders' number.

### What the round found in factory

All fixes are on `fix/001-ab-findings`: 268 tests before the claim change, 272 after. Gate green, binary reinstalled, not pushed. The PRD's FR-4 and FR-5 carry dated correction notes.

| finding (`ab/runs.md`) | severity | fix |
|---|---|---|
| A `Bash` heredoc (`cat > f <<'EOF'`) wrote 11 of slice 02's files, and no tool call named them, so they were never committed. CI on the pushed branch would have failed on missing classes. | major | `ea6866f`: a heredoc into `cat` or `tee` names its target like a `Write`. |
| A file listed once was never committed again, even after it was tracked and changed. Slice 06's and polish's edits to those files vanished silently. | major | `ea6866f`: a listed file stays skipped only while its blob is unchanged. |
| Four of six workers wrote a `# Log fragment` title. `docs log merge` refused it, the driver parked, and the owner was asked three times. Builders copied the title from a sibling's fragment. | major | `e75076b`: the driver drops a title above the first `## `, and the slice contract says so. |
| The refusal came after a 7-minute green gate, and each resume reran that gate on an unchanged tree. That cost 3 extra runs, 2 at CP-2 and 1 at CP-3, about 21 minutes. | medium | `afe8904`: `docs log merge --dry` runs before the gate. A green on a dirty tree is kept for its own checkpoint. |
| Commit subjects carried no feature scope (`feat: Core sync kinds`, `docs: checkpoint 1`). | minor | `b713aa8`, `b395562`: `feat(018): slice-02 …`, `docs(018): checkpoint 1`, and the same in the worker contracts. |
| The owner asked why a worker cannot claim a file it generated. | design | `c911674`: `git add <file>` claims it. `git add <dir>` claims new files under it while no other worker is live. `git add -A` and `.` claim nothing. A staged-new file no longer counts as tracked, so a lone worker's `git add -A` cannot sweep a secret into its commit. |

**The three minor ones, fixed 2026-10-12 on `main`** (gate green at 276 tests):

- `factory answer` noted the preflight ask as `- **DEC (date)**`, neither an `AD-n` nor a `DEC-n`. `bc89299`: it writes a `### AD-n — Preflight ASK-n: … · preflight · date` entry, numbered after the highest AD below 100. Slicing owns that range and is finished before preflight.
- The cockpit's BLOCK-1 question named one of the two fragments, because `factory wait` printed only the first line of the merge output. Its options were written for an AD clash, not a format slip. `67137fb`: the wait line and the block title join every refusal, and a format slip gets format options. Either way the block says the driver edits no fragment, so the fix is the owner's.
- `factory report` lacked the cockpit's own output, cache-read and start-context figures. `9442a04`: `cockpit_out_tokens`, `cockpit_cache_read_tokens` and `cockpit_start_context`. On run B's cockpit session they print 17,844, 4,553,507 and 49,700, the same figures as `measure.py`, so the FR-8 cross-check now compares all four.

**Not factory's, carried to the project:** icg-cloud's CI runs no browser suite, so the two browser tests B broke reached a green PR. That is a `/guardrails` question for icg-cloud.

### Next

- **arca-api round: done 2026-10-12**, see the next section. Setup as planned: the repo is `~/Projects/Prokey/arca-api` (`~/Projects/ICG/arca-api` is a copy with no git). Branch from `main`, since its PR #63 is open. It has no draft PRD: the owner writes PRD-023 with `/prd` in a fresh arca-api session, then runs A (`/deliver`) and B (`/factory-deliver`). Run B uses the binary from `fix/001-ab-findings`, so the round also tests these fixes on a real project.
- **Per-role split, decided 2026-10-12, built and merged 2026-10-12** (`d414119`): `factory report` prints `followup_reads_by_role` in text and JSON. On run `018-20261011-6490` it prints `build 2 of 7 (29%), polish 5 of 14 (36%)`. Each follow-up was a 25–80 line read of a neighbouring method or a callee. Also seen: the polish worker ran `factory ctx --help` twice.
- **Branches (2026-10-12):** the owner dropped the PR ceremony for this repo while it is early: work merges into `main` locally and `main` is pushed. `main` holds the six A/B fixes (PR #3, `81ba7ee`), the per-role split, the A/B records, the PRD-002 draft and the three minor fixes above. Gate green at 276 tests. The installed binary (`~/.cargo/bin/factory`) is built from that `main`, so arca-api's run B uses it. Every other branch is deleted; `main` is the only one.
- **PR pick for icg-cloud, owner's call (2026-10-12): #24.** Done as [#26](https://github.com/ICG-Argentina/icg-cloud/pull/26) (2026-10-12): #24's head plus the badge fix and B's extras (the inbox link above the company search, the count beside the subject, year-first times, the failing source named in the log, and its isolation tests), without B's defects. Close #24 and #25 when it merges; the merge stays the owner's. #24 won the blind review, and its polish caught the leak #25 still has.

## FR-11 — A/B, arca-api PRD-024 · 2026-10-12

Run A was `/deliver 024`, PR [#64](https://github.com/prokeycloud/arca/pull/64). Run B was `/factory-deliver 024`, run `024-20261012-e894`, PR [#65](https://github.com/prokeycloud/arca/pull/65), on the binary from `main` with the icg fixes. Both started from base `91b4c59` on Opus 5.5, serial, one after the other. Setup, timeline, the owner's answers and the blind review are in [`ab/runs.md`](ab/runs.md) § arca-api.

### Answers and review

| # | question | run A | run B |
|---|---|---|---|
| 1–2 | Does what the PRD asked · merge after normal review | yes · yes | yes · yes |
| 3 | Decisions you would overturn | 1 | 2–3 |
| 4–5 | Code quality · tests prove the FRs | 5 · prove | 5 · prove |
| 6–7 | Built but not asked · asked but missing | none · none | none · none |
| 8 | Overall | ship as is | ship after minor fixes |

The owner found B's UI better and was unsure B's code matched A's. A blind review (X and Y from `git archive` exports, no history or PR metadata) put **B ahead, 32 to 30**, no merge blocker on either side. A's weak spot was systematic: about ten record loads turn any database error into an unlogged 404, against slice 09. B's worst defects were local: one missing audit label, and "Todas" not clearing a stale taxpayer filter (FR-84). Both are minor fixes. Run B had no decision to make (18 fyi, all kept in review), yet the owner would overturn 2–3: a judgment formed after QA, not voiced in the review.

### Numbers

Weighted as in the icg round. Run A from its 25 transcripts, run B from `factory report --cockpit`.

| | run A | run B | icg A → B, for scale |
|---|---|---|---|
| build | 9.86M | 9.95M | 4.92M → 3.63M |
| checkpoints | 4.15M | 3.94M | 2.93M → 1.27M |
| fixes | inside the above | 0.34M | — |
| polish and closeout | 6.36M | 1.95M | 6.13M → 1.19M |
| orchestrator or cockpit | 2.13M | 0.97M | 2.18M → 0.62M |
| **total** | **22,488,877** | **17,144,058** (76.2%) | 41.5% |
| launch to draft PR | 3h 56m | 2h 58m | 3h 07m → 3h 01m |

### Pass criteria (QA guide step 7)

| criterion | result |
|---|---|
| `weighted_tokens(B) ≤ 0.6 × weighted_tokens(A)` | **fail**: 76.2%, 3.65M over the 13,493,326 bar |
| `cockpit_tokens(B) ≤ 5%` of `weighted_tokens(B)` | **fail, narrowly**: 6.0% of the workers' figure, 5.7% of the total. It includes the 18-decision review and three hours of 25-minute check-ins. |
| run B's draft PR has CI green | **pass**: 4 of 4 first time, then again after the review commit. The PR stays a draft by design (FR-6). |
| questionnaire (step 5) | **pass** on the guide's rule (yes on 1–2, none on 6–7, 5 on 4, "after fixes" on 8), though one notch under A on 3 and 8 |

**Both token criteria fail, so this project is a `BLOCK` for the owner to judge** (step 7). The cockpit miss is small and mostly the owner's own review time. The token miss is not noise; it has a cause.

### Why the build did not get cheaper

**The driver saves a fixed amount, and the bar is a ratio.** Across both projects the saving is polish (−4.9M icg, −4.4M arca) and orchestration (−1.6M, −1.2M): about 5.5–6.5M either way. Build plus checkpoints barely moved here (14.0M → 13.9M; icg 7.9M → 4.9M). PRD-024 is a build twice the size of PRD-018, so the same fixed saving is a smaller share, and 0.6× fails.

**Builders cost the same because they behave the same.** Run B's 9 builders took 610 turns against run A's 547 calls, read 59.9M cache against 58.5M, wrote 462k output against 472k. `factory ctx` was not the lever: 11 calls by builders (`ctx_share` 0.025); they read with `sed -n` (153 calls) and `grep` (84). Lookups are cheap anyway: every builder tool result together is about 1.5M characters, roughly 0.4M tokens.

**The cost is turns × context.** Cache reads are 6.0M of the builders' 9.95M weighted. The starting context alone (about 45k, mostly Claude Code's system prompt plus the pack) re-read on every turn is about 27.8M of the 59.9M cache read, 46%. The rest is each builder's own growing session, peaking near 150k on average.

**FR-12's reversal rule:** builders' `followup_reads` is 1 of 11 (9%), under 30%, so no widening. `ctx_share` is under 10% on both projects now (7.3%, 2.5%), so the shrink trigger has fired twice. Owner's call.

### What the round found in factory

No repair commits this time: no BLOCK, no leftovers, no owner question during the build. The icg fixes held on a real Go project. Two red gates (CP-4, CP-5) were fixed by the driver's own fix workers.

- **The QA stack has fixed ports.** Run A's harness stack had to be stopped before run B could run its own. A/B setup, not factory.
- **The owner's message leaked into the blind review's agents again**, through the workflow harness. Labels kept it harmless this time.

### Next

- **Owner's calls:** the two failed criteria (BLOCK under step 7), and the FR-12 shrink.
- **The lever is builder turns and starting context, not lookups.** Options to weigh, cheapest first: a smaller fixed context per worker (trim the pack, check what Claude Code's own system prompt costs under `-p`); fewer turns per slice (batch edits, run the inner gate less often); a cheaper builder model or an advisor arm (`docs/ideas.md`). Measure each on one PRD before the next A/B.
- **Merge picks for arca-api** are the owner's: #64 (A) or #65 (B) plus B's two minor fixes, or a mix as icg did with #26.

## FR-7 — Cockpit from the phone

Not run yet. The guide's steps (start on the laptop, answer a block from the phone, `factory wait --timeout 7200` in a background Bash) are still open.
