---
number: 018
slug: skill-wording
title: "Skill wording, contradictions and Prove guidance"
type: enhancement
created: 2026-11-05
author: Emiliano Tisato
summary: |
  The skills stop contradicting each other and stop leaving runs to guess. Polish's closeout names the summary, not a dead Notes cell. Every `{date}` is the UTC date. Prune counts polish's own edits as under review, lets a cited test name go, and reuses polish's gate. The PR body defines its gate runs. Polish says what to do on an API-only change. Autonomy names research notes, runbooks and sign-off items. Prove rows take their expected result from the spec and are seen red once. Slices that write one file are linked. A checkpoint repairs the harness drive its slices broke. Every skill-text test asserts a unique passage. Interviewed 2026-11-12.
---

# PRD-018 — Skill wording, contradictions and Prove guidance

## 1. Why now

Each item is a guess an unattended run had to log, or a defect that passed a green checkpoint. PRD-005's polish found 4 highs that four green checkpoints had passed (harvest 2026-10-15). Three skill-text tests passed after the rule they guard was deleted (PRD-012 retro). The rest are contradictions and gaps that cost a logged guess each, across arca-api, factory and other dogfood runs (§19).

Bugs the owner's runs hit:

- prune's own rule blocks every consolidation polish asks of it (§10);
- a Prove row took its expected result from the command's printed output, and specs passed with the feature removed (§9);
- a checkpoint deferred a harness repair to a polish that has no step to receive it (§12);
- two slices both rewrote four test files, were marked independent, and blocked four times (§11).

The rest are wording: a contradiction or a gap each.

**Deliberately left alone:**

- the bash hook's `.push(` false positive, which recurred in PRD-017's run (its log note), is PRD-019's (*catches every spelling of a process call*);
- the relock trip on a second skill edit is PRD-015 C-3's (§2).

## 2. Blast radius

- **Every repo's skills.** Every FR edits `skills/` or `assets/`. A skill edit needs a version bump past 0.13.3 (`bundle_lock_matches`), and the bump moves the cockpit mod's three version pins (`tests/cockpit_mod.rs`). Other repos receive the text through `factory skills update`.
- **Builders and checkpoints read this text unattended.** `assets/autonomy.md`, `assets/contract-slice.md` and `assets/contract-checkpoint.md` are embedded in the binary (`src/pack.rs:17`), so a lane running the installed binary reads the old text until `cargo install` after the merge. A builder sees autonomy.md minus its `<!-- builder: cut -->` blocks.
- **A checkpoint gains a write.** §12 lets the checkpoint edit `.claude/skills/verify-*`. The driver already commits everything not gitignored in the lane with the checkpoint (`assets/contract-checkpoint.md:18`; an earlier draft cited `:17`, corrected 2026-11-12), so no driver change is needed.
- **The relock trip.** A second skill edit under an already-bumped, uncommitted version makes `FACTORY_BUNDLE_WRITE=1` refuse (`src/skills.rs:165-182`). PRD-017's run hit it and logged the workaround. Decided 2026-11-12: 018 ships first and lives with it, and PRD-015 keeps the fix.
- **No external contract.** No CLI output, JSON or event changes. `assets/pr-body.md` is a template the polish worker fills (`src/pack.rs:557-562`), not code.

## 3. FR-1 — Polish's closeout names the summary, not a Notes cell

**Today.** `skills/polish/SKILL.md:214`, in §9's closeout list: "the index **Notes** cell — decisions taken, what remains, the PR number". v3's index has no Notes cell (`assets/docs/index.md`). `skills/prd/SKILL.md:129` says the frontmatter `summary` "replaces any notes cell". The draft's contradiction with `deliver` (PR number or not) is gone with the cell. One stale bullet remains.

**Decision (2026-11-12).** The bullet becomes: keep the PRD's `summary` frontmatter current, saying what landed and what remains. No PR number goes in it.

- **Accepted cost:** none.

**Acceptance.** Polish §9 names the `summary` frontmatter exactly once, and the polish skill no longer holds "the index **Notes** cell". (An earlier draft tested for the bare words "Notes cell", which the line never held, so the test could not fail before the change. Corrected 2026-11-12.)

## 4. FR-2 — Every `{date}` is the UTC date

**Today.** Six `{date}` placeholders name no clock:

- `skills/deliver/SKILL.md:45`
- `skills/polish/SKILL.md:163`
- `skills/harvest/SKILL.md:34`
- `skills/ux/references/rubric.md:69,70`
- `assets/contract-polish.md:18`

The code's clock is UTC:

- `scaffold::today()` is `clock::now_rfc3339()[..10]` (`src/docs/scaffold.rs:28`), used by the scaffold, `docs ship` and the polish contract (`src/pack.rs:542`);
- `log add` stamps `ts` in UTC (`src/docs/log.rs:217`).

A checkpoint subagent once stamped UTC while its orchestrator wrote the local date, so one heading was a day off.

**Corrected 2026-11-12 (adversarial pass).** The list above was incomplete in two ways.

- The `{date}` in `assets/contract-polish.md:18` is filled by the binary (`src/pack.rs:542`, `.replace("{date}", …)`), so no agent reads it. A sentence there holding `{date}` would itself be substituted.
- It missed unbraced date placeholders an agent fills:
  - `reviewed=<date>` (`skills/polish/SKILL.md:168`);
  - `RETIRED (date)` (`skills/prd/SKILL.md:42`, mirrored in `assets/docs/index.md:43`);
  - `Accepted _(date)_` (`assets/docs/prd.scaffold.md:56`).

**Decision (2026-11-12).** Every skill that asks an agent to fill a date says once, at its first use, that it is the UTC date (`date -u +%F`). The skills are deliver, polish, harvest, prd and ux. The ux skill covers its rubric template, and prd covers the scaffold and the index line. Templates that become project files (`assets/docs/*`, `skills/ux/references/rubric.md`) and `assets/contract-polish.md` get no sentence. No code change.

- **Accepted cost:** a run late in the owner's evening stamps tomorrow's date.
- **Rejected:** the local date. It needs `scaffold::today()` to change, and the log's `ts` would stay UTC, leaving two clocks.

**Acceptance.** Each of the five `SKILL.md` files holds the UTC sentence exactly once.

## 5. FR-3 — Prune counts polish's own edits as under review

**Today.** `skills/prune/SKILL.md:24`: "Working tree must be clean, or dirty only with the changes under review… Inside a `factory` run the uncommitted polish fixes are the changes under review." Polish calls prune at §7 (`skills/polish/SKILL.md:157`). Before that, §5 fixes code and writes log entries through the tool, and §6 writes `docs/ideas/` files. Neither the docs and idea edits nor an attended polish are named. A run read this as a conflict and logged a guess.

**Decision (2026-11-12).** Precondition 1 says that when polish invokes prune, attended or not, polish's own uncommitted edits count as changes under review: fixes, slice docs, `docs/ideas/` files and the feature's log.

- **Accepted cost:** none.

**Acceptance.** Prune's precondition 1 names polish's own docs and idea edits, and it is not limited to a `factory` run.

## 6. FR-4 — The PR body defines its gate runs

**Today.** `assets/pr-body.md:43`: "Gate `{command}`: {runs} runs, {red} red, {minutes} min in total". Nothing defines `{runs}`. The polish worker fills it (`src/pack.rs:557-562`, pasted raw). The retro's `gate_runs` counts the driver's full-gate runs, cached hits included at 0 minutes (`src/cli/report.rs:553,572`). Inner loops are `worker_gate_runs` (`:554,620`). Line 30's per-slice cell already says `inner {runs}`.

**Corrected 2026-11-12 (adversarial pass).** An earlier draft matched line 43 to the retro's numbers. The polish worker cannot see those: the polish pack carries no gate counts (`src/pack.rs:537-572`). `worker_gate_runs` also counts every worker's gate calls, polish's own included, and only once each worker has exited (`src/metrics.rs:536-550`). So it is not "the builders' inner loops".

**Decision (2026-11-12).** Line 43 is filled from what the polish worker can read.

- **`{runs}`** is the full-gate runs: the sum of each checkpoint entry's `**Gate:** n runs` in the feature's log, plus polish's own gate runs. `{red}` and `{minutes}` come the same way.
- **A second clause, `inner loops: {n} runs, {red} red`,** is the sum of the slices' Progress inner-loop lines.

- **Accepted cost:** the PR body's count can differ from the retro's `gate_runs`, which counts cached hits from the metrics store.

**Acceptance.** The PR body template's Gate line says where its numbers come from and has its own inner-loop clause.

## 7. FR-5 — Polish on an API-only change

**Today.** `skills/polish/SKILL.md:45`: "**Anything user-visible gets the UX pass**". `:61`: "Skip it only when nothing user-visible changed". A change whose only user surface is an API (a third-party API-docs renderer, a curl-only harness) fits neither. Workers in four consecutive runs in one repo skipped `/ux` and explained why each time.

**Decision (2026-11-12).** Both `:45` and `:61` name the case. `:45`'s override rule (an earlier draft edited only `:61`, which `:45` would still override; corrected 2026-11-12) excepts a change whose only surface is an API. `:61` says what to do then: skip `/ux`, say so, and point at the QA plan's Human row if there is one.

- **Accepted cost:** none.

**Acceptance.** Polish's override rule and its UX skip paragraph each name an API-only change.

## 8. FR-6 — What an unattended run may write

**Today.** `assets/autonomy.md:46-52`:

- **May:** app code and tests, the feature's folder, `idea add`, `verify-*`.
- **Never:** `docs/decisions.md`, the UX rubric, CI workflows, `CLAUDE.md` and the like, another feature's folder, the archive, the PRD's FRs.
- "**The PRD outranks this list for a file it names**… done as written, with an `AD` marked `decide`."

Four cases are on neither list, and runs guessed:

- a research note the run wrote on its own branch and then repaired;
- an operations runbook the feature made stale;
- an open sign-off checklist row;
- a later polish fix to a CI workflow the PRD ordered.

**Decision (2026-11-12).** The section adds:

- **May:**
  - repair anything the run itself wrote on this branch;
  - update project docs (research notes, runbooks, operations procedures) that the run's FRs make factually wrong, text only;
  - fix a defect a review found in a file the PRD ordered, later, with an `AD` marked `decide` as the creation had.
- **Never:** close or tick a human sign-off item (a checklist row, a signature, an approval) in any doc. That change is a proposal in the PR body.
- **Precedence**, stated in the section (added 2026-11-12 after the adversarial pass):
  - The **Never** list and "Never, to get green" (`assets/autonomy.md:61`, "edit a CI workflow") win over every new **May**. A PRD-ordered CI workflow may get a defect fix a review found, never an edit that makes a red check pass.
  - `CLAUDE.md` and `docs/decisions.md` stay **Never**, even when the FRs make them wrong. That is a proposal in the PR body.
  - A builder's own contract still wins: "touch nothing outside your slice's files" (`assets/contract-slice.md:19`). For a builder, "anything the run wrote" means its slice's files.

The section sits outside the builder cut, so builders read it too.

- **Accepted cost:** a run may edit a project doc outside `docs/features`. The PR diff shows it.
- **Rejected:** a stricter rule that parks every such doc edit as an idea. It leaves a runbook the feature made wrong merged as wrong.

**Acceptance.** The section names the four cases, the sign-off rule sits under **Never**, and the precedence sentence exists.

## 9. FR-7 — Prove rows check the spec, and are seen red once

**Today.**

- `skills/slice/SKILL.md:58` (Prove) and `:85` (Acceptance: "a test that could fail") say nothing on where the expected result comes from.
- `assets/contract-checkpoint.md:15` drives rows but does not say where the expected result comes from.
- `assets/contract-slice.md:13-14` never asks a builder to see a test fail.

Two defects passed green checkpoints:

- a Prove row recorded a command's printed summary as the expected result, while the input spec named a different set;
- two "submit once" browser specs passed with the feature's script removed.

**Decisions (2026-11-12).**

- **Expected result from the spec.** `/slice`'s Prove guidance and the checkpoint contract say the expected result comes from the PRD, the spec or the input file, never from what the command printed.
- **Name the failure, see it once.** Each behavioural Prove row, and each test a task's Acceptance names, says how it fails if the feature is removed.
  - The builder sees it red only for tests it runs: the tests its tasks' Acceptance names, and `run` rows. It breaks the feature once (a revert or a stub), sees the test go red, then restores the feature. Progress gets one line per test.
  - These deliberate reds do not count in the `INNER: … red` return line (`assets/contract-slice.md:25`).
  - A harness-driven row only names its failure. The builder never drives the harness (`assets/contract-slice.md:14`).
  - An earlier draft asked the builder to see every Prove row red, including the ones only the checkpoint drives. Corrected 2026-11-12.
  - This goes in `/slice`'s guidance and in `assets/contract-slice.md`.
- **Accepted cost:** one extra targeted test run per behavioural test, in the builder's inner loop.
- **Rejected:** naming only, with no red run, which would still pass the "submit once" specs. Also rejected: a checkpoint that reverts per row, which costs a gate-sized run per row, needs driver work and overlaps PRD-021.

**Acceptance.**

- `/slice` §2 and the checkpoint contract each carry the spec-not-output rule.
- `/slice` and the builder contract each carry the break-it-once rule.

## 10. FR-8 — Prune may rename a cited test, and reuses polish's gate

**Today.**

- `skills/prune/SKILL.md:28`, precondition 5: names cited by evidence, Progress and log lines stay as written. "Prefer keeping the name." Prune runs last in polish (`skills/polish/SKILL.md:157-159`), after every checkpoint has cited the names, so that rule argues against every merge. In one run, two same-setup pairs stayed separate for exactly this reason.
- PR mode (`:71-79`) runs the scoped files 3 times (step 3). On a branch that adds most of the suite, that repeats a gate's worth of time right after polish's own gate run. It also never notes that such a branch is really full mode without the baseline coverage step. Four consecutive runs in one repo hit this.

**Decisions (2026-11-12).**

- **Precondition 5.** A name cited only in frozen records (a checkpoint transcript, a Progress line, a log entry) does not bind. Prune may rename or merge freely. It leaves the records as history and names old → new in the commit body, or in its report inside a `factory` run, where the driver commits (`assets/contract-polish.md:20`, `skills/prune/SKILL.md:61`). An earlier draft said the commit body only. Corrected 2026-11-12.
  - **Accepted cost:** an old Progress line names a test that no longer exists. The commit body or the report maps it.
  - **Rejected:** a prune pass per checkpoint, which costs a prune worker per checkpoint and a driver change.
- **PR mode.**
  - When the scoped tests are most of the suite, prune takes full mode's coverage baseline.
  - When polish invokes prune right after a green gate, with no code or test change since (polish §6's `docs/ideas/` files do not count), step 3's three repeat runs are replaced by that gate result, cited in the report.
  - **Accepted cost:** flakiness is checked once, by the gate, instead of three times.

**Acceptance.** Prune's precondition 5 lets a cited name go and drops "Prefer keeping the name". PR mode names the whole-suite case and the reuse of polish's gate.

## 11. FR-9 — Slices that write one file are linked

**Today.** `skills/slice/SKILL.md:54`: "**A file two FRs both write is one slice's, or an order.**" It covers FRs, not slices. In arca-api PRD-025, slice 02 was marked independent of 01 while both rewrote the same four test files and the stub, and four blocks followed.

**Decision (2026-11-12).** Two slices whose tasks write the same file are one slice, or the later one depends on the earlier. Reading the same file does not link them.

- **Accepted cost:** less parallelism where two slices share a test file.

**Acceptance.** `/slice` §2 names slices whose tasks write one file.

## 12. FR-10 — A checkpoint repairs the harness drive its slices broke

**Today.** `assets/contract-checkpoint.md:19`: "Touch nothing a row, a leaks hit or the record does not name". A `verify-*` recipe that a covered slice's change broke is not named by a row. So the row goes `unreached`, and the checkpoint "defers" the repair. `/polish` and `/deliver` have no step that receives it (none mentions "harness" with "repair"). `assets/autonomy.md:48` already lets a run "Extend or repair a recipe".

**Decision (2026-11-12).** The checkpoint contract lets the checkpoint repair `.claude/skills/verify-*` when a covered slice moved the surface it drives. It then drives the row and names the repair under **Fixed**. It never drops a step an earlier checkpoint drove (`autonomy.md:48`).

- **Accepted cost:** a longer checkpoint when the drive broke.
- **Rejected:** blocking the slice for the owner, which stops the run for a fix a worker can do.

**Acceptance.** The checkpoint contract's touch rule names the `verify-*` repair, and **Fixed** lists it.

## 13. FR-11 — Every skill-text test asserts a unique passage

**Today.** About 24 skill and asset passage assertions use plain `contains`, and none checks uniqueness:

- `tests/skills_bundle.rs`: 17 assert lines across 11 functions, plus positional `find`s;
- `tests/cockpit.rs:792`: about 20 needles in one loop;
- `tests/init.rs:285-288`;
- `tests/loop.rs:273,277`;
- `src/pack.rs:1072`;
- `src/pack.rs:1377-1395` (cut markers).

Three such tests passed after the rule they guard was deleted, because another line held the same words (PRD-012 retro). The `count()==1` the draft cited (`tests/init.rs:263`) counts a heading in a rendered `CLAUDE.md`. It is not a passage check.

**Decisions (2026-11-12).**

- **The helper.** `assert_unique_passage(label, text, needle)` fails when the needle occurs zero times or more than once. Its message names `label`: the file, or the section it was cut from.
- **Where it lives.** In a file under `tests/` (say `tests/common/passage.rs`, with `#![allow(dead_code)]` for clippy's `-D warnings`). Integration tests pull it in with `#[path]` the way `tests/harness/mod.rs` is, and so does `src/pack.rs`'s test module. Nothing ships in the binary.
  - Corrected 2026-11-12: an earlier draft left the place open. A `#[cfg(test)]` item in the lib cannot be reached from `tests/`, and `pack.rs`'s `AUTONOMY` and `CUT` are private, so those tests stay where they are.
- **What moves.** A *passage assertion* is a positive check that skill or asset text holds a passage. Every one listed above moves to the helper.
  - A needle that occurs twice is lengthened until it quotes the one passage it guards.
  - Absence checks stay `!contains`, for example `src/pack.rs:1144,1361,1384`, `tests/skills_bundle.rs:112,122,149,165,237,251`, `tests/loop.rs:274` and the forbidden loop in `tests/cockpit.rs`.
  - Line selectors (`.find(|l| l.contains(…))`) are not assertions.
  - The cut markers occur four times by design (`assets/autonomy.md:31,95,106,112`) and keep their own count checks.
- **`/slice` §4** asks every skill-text Acceptance to assert that its quoted passage occurs exactly once, never a word it shares with others. The skill ships to every repo, so it names the rule, not factory's helper.
- **Accepted cost:** some needles grow to a full clause.

**Acceptance.**

- The helper's own test fails on zero and on two occurrences.
- No positive passage assertion in `tests/` or `src/pack.rs` uses bare `contains`. Polish's local review checks this; no test can.
- `/slice` §4 carries the exactly-once rule.

## 14. Decisions log

- ~~**Prune vs cited names**~~: frozen citations do not bind (2026-11-12), §10.
- ~~**Prove negative case**~~: name it and see it red once, in the builder (2026-11-12), §9.
- ~~**Autonomy writes**~~: the proposed default (2026-11-12), §8.
- ~~**Order vs PRD-015**~~: 018 first, living with the relock trip (2026-11-12), §2.
- ~~**Clock**~~: UTC, matching the code (2026-11-12), §4.
- ~~**Reversing a proven AD**~~: left out (2026-11-12), §17.
- ~~**Harness repair owner**~~: the checkpoint (2026-11-12), §12.
- ~~**Unique-passage reach**~~: every existing site (2026-11-12), §13.

## 15. QA plan

- **Gate:** `factory docs gate`.
- **Surface:** skill and asset text, which agents read. No UI, no harness. Every row is proven below the UI by a skill-text test using `assert_unique_passage`.
- **Seed:** none.
- **Human rows:** none, so there is no `qa-guide.md`.

| FR | Proof | Human? |
|---|---|---|
| FR-1 | run `polish_closeout_keeps_the_summary_current` | no |
| FR-2 | run `every_skill_that_fills_a_date_names_utc` | no |
| FR-3 | run `prune_counts_polish_own_edits_as_under_review` | no |
| FR-4 | run `pr_body_names_full_gate_runs_and_inner_loops` | no |
| FR-5 | run `polish_skips_ux_on_an_api_only_change` (both passages) | no |
| FR-6 | run `autonomy_names_research_notes_runbooks_and_sign_offs` | no |
| FR-7 | run `prove_takes_its_expected_result_from_the_spec`, `checkpoint_takes_its_expected_result_from_the_spec` and `a_behavioural_test_is_seen_red_once` | no |
| FR-8 | run `prune_lets_a_cited_name_go` and `prune_pr_mode_reuses_the_polish_gate` | no |
| FR-9 | run `slices_that_write_one_file_are_linked` | no |
| FR-10 | run `checkpoint_repairs_the_drive_a_covered_slice_broke` | no |
| FR-11 | run `a_passage_must_occur_exactly_once` and `slice_asks_for_a_passage_that_occurs_once` | no |

## 16. Slices

Four slices, one branch (`feat/skill-wording`), one PR, one commit per slice, and Prove at the checkpoints. Every Prove row is a skill-text test run below the UI. No row is Human, so there is no `qa-guide.md`.

- **01, unique-passage helper** (FR-11, the helper and the conversion). The spine: every later slice's tests call the helper. It is also the riskiest slice, since every existing passage assertion moves to the helper without being weakened. It edits no skill text, so it needs no bump.
- **02, polish and prune text** (FR-1, FR-3, FR-5, FR-8). It depends on 01, because it adds tests to `tests/skills_bundle.rs`, which 01 rewrites. As the first skill edit, it bumps to 0.13.4 with the cockpit pins.
- **03, autonomy, contracts and the PR body** (FR-4, FR-6, FR-7's contract half, FR-10). It depends on 01 for the helper only. It edits `assets/`, which is outside the skills bundle, and puts its tests in a new `tests/asset_text.rs`, so it runs beside 02.
- **04, slice skill and the UTC date** (FR-2, FR-7's `/slice` half, FR-9, FR-11's `/slice` §4 sentence). It depends on 02, because both edit the polish skill, `skills/bundle.lock` and `tests/skills_bundle.rs`. It only relocks: it restores the base lock, then rewrites it once under 0.13.4.

Checkpoints fall every 2 done slices: after the second of 01/02/03 to land, and once at the end. The board is `factory docs status 018`.

## 17. Non-goals

- **Not handled: an Ask that reverses an AD a checkpoint already proved** (2026-11-12, owner cut). PRD-017's overturn rule (`assets/autonomy.md:27`) covers it: overturning a `decide` AD is `decide`. A proven `fyi` AD overturned in polish reaches the owner only through the PR review. Reopen if one ships wrong.
- **Not handled: one hostile case per FR** in the checkpoint contract (harvest 2026-10-15, PRD-005). §9's red run covers the "spec passes without the feature" half. Reopen with PRD-021.
- **The `.push(` hook false positive**: PRD-019.
- **The relock of an untagged version**: PRD-015 C-3.
- **Wording gap 5** (a PRD dictating a plan-id comment): done in PRD-006.
- **Polish §8 names the `docs set reviewed=` verb**: done in PRD-017 (`skills/polish/SKILL.md:168`).

## 18. Verification notes

Checked 2026-11-12 on `feat/skill-wording` at `865a943` (main `37f88b3` plus a leaks chore). Re-verify at slice entry.

- `skills/polish/SKILL.md:214` holds the Notes bullet. `assets/docs/index.md` has no Notes cell.
- The `{date}` sites are as listed in §4. `src/docs/scaffold.rs:28` and `src/docs/log.rs:217` stamp UTC. `src/docs/state.rs:166-173` accepts any `YYYY-MM-DD`.
- Prune's precondition 1 is at `skills/prune/SKILL.md:24`, precondition 5 at `:28`, PR mode at `:71-79`. Polish §7 is at `skills/polish/SKILL.md:155-159`.
- `assets/pr-body.md:30,43`. Nothing in `src/` substitutes `{runs}`. The retro's `gate_runs` and `worker_gate_runs` are at `src/cli/report.rs:553-554`.
- Polish's UX passages are at `:45,59,61`. `polish_starts_ux_first_and_holds_tests` (`tests/skills_bundle.rs:300`) pins `:47` only.
- `assets/autonomy.md`:
  - `:46-52` is "What an unattended run may write", outside all four cut blocks (`:31-45`, `:95-100`, `:106-109`, `:112-125`; an earlier draft said two, corrected 2026-11-12);
  - `:27` is the overturn rule, pinned at `tests/skills_bundle.rs:340-357`.
- `skills/slice/SKILL.md:53,54,58,85`. `assets/contract-slice.md:13-14`. `assets/contract-checkpoint.md:15,19`.
- `bundle_lock_matches` is `tests/skills_bundle.rs:180-207`, and `relock` is `src/skills.rs:165-182`. The lock and `Cargo.toml` are at 0.13.3, tagged `v0.13.3`, so the first skill edit needs a bump to 0.13.4 and its cockpit pins.
- No skill or asset passage test checks uniqueness (§13 lists the sites).
- **Adversarial pass (2026-11-12):** a fresh Opus agent, since Cursor's quota is out until 2026-11-24. It made 12 findings. Eleven are applied above as dated corrections (§2, §3, §4, §6, §7, §8, §9, §10, §13, §18). One is rebutted: every acceptance moves only a skill-text test. That is the nature of a wording PRD, and no run-behaviour test is in scope.

## 19. Source ideas

The full text of the ideas this PRD absorbed is in the 2026-11-05 triage draft, `git show 865a943:docs/features/018-skill-wording/prd.md` §4:

- "Five small contradictions between factory skills";
- "Five skill wording gaps";
- "autonomy.md: research notes and operations docs";
- "Prove guidance";
- "prune vs cited test names";
- "Skill-text tests: assert the quoted passage is unique";
- "friction: arca-api PRD-025".
