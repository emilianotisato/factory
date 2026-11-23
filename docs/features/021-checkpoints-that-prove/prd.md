---
number: 021
slug: checkpoints-that-prove
title: "Checkpoints that actually prove"
type: feature
created: 2026-11-05
author: Emiliano Tisato
summary: |
  A checkpoint closes only on what it can show: its record names the slices the driver handed it, every machine-provable Prove row it was handed left evidence on disk, and the last checkpoint re-proves every row the feature declared. A mismatch is sent back once, then blocks. Interviewed 2026-11-23 and corrected by an adversarial pass the same day; four FRs, no OPENs, built by an unattended run.
---

# PRD-021 — Checkpoints that actually prove

> **Interviewed 2026-11-23.** Every `file:line` below was verified against `main` at `8cbf7d2`, then **re-verified by an adversarial pass on 2026-11-23 which broke ten claims**. Each correction is inline and dated. The 2026-11-05 triage draft's candidate list is folded in; two of its claims were false.

## 1. Why now

A checkpoint is the run's only proof that a done slice works. Today it closes on the *existence* of a record, so three things have happened:

- **The record is never compared with the request.** The driver computes `covers` from `board.unverified` (`src/checkpoint.rs:66`), hands it to the worker, then closes on `after.checkpoints.iter().any(|c| c.n == n)` (`:296`) — the number alone. `land_here` only lands the commit; it never reads the record.
- **The `PROVE:` line is never read.** `assets/contract-checkpoint.md:25` asks the worker to return `{rows driven}/{rows named}` and an `unreached:` list. `grep -rn PROVE src/` returns nothing. In PRD-018, CP-1 drove **7 of 9** rows and CP-2 **3 of 4**; both records read "all run green", and FR-2, FR-4 and FR-6 reached polish unproven.
- **The last checkpoint proves only its own slices.** In a client feature a later slice put a session re-check in front of every acting user. Two earlier browser proofs went red and stayed red until polish, because the last checkpoint ran only its own new drive.

**This gives an existing rule its teeth.** `assets/autonomy.md:64` already puts "record a checkpoint, or a Prove row, without an artifact that exists on disk" under **Never**. Nothing checks it. FR-2 is that check.

**What this leaves alone.** The gate, its tiers and its cost. The checkpoint cadence (`checkpoint_every`). What a Prove row *says* — that is the slicer's job, and the rows' own wording is PRD-018's successor work.

## 2. Blast radius

- **Every repo on the framework.** A worker that used to close a checkpoint with a thin record now gets sent back once and then blocks the run. That is the point. Two classes of repo must keep closing as they do today, or their runs block on something no worker can satisfy: a PRD whose QA plan has no `| FR-` rows at all, and any row marked `Human? = yes` (§4).
- **`factory docs log add --kind checkpoint` gains a required `--covers`.** The owner-facing form comes from the death block at `src/checkpoint.rs:226`, which already passes it, so no instruction an owner copies goes stale. The refusal uses the machinery that already refuses a checkpoint with no `--slice` (`src/docs/log.rs:195-200`).
- **No archived history changes, and no existing test moves.** *Corrected 2026-11-23 (adversarial pass): an earlier draft of this PRD removed `covered()`'s high-water branch and claimed the only cost was 18 covers-less entries in archived 001 to 005. A live gate test pins that branch — `a_log_without_covers_keeps_the_high_water_rule` (`tests/docs_log.rs:289-308`) with its committed fixture `tests/fixture/log-without-covers/log.jsonl` — so removing it turns `cargo test` red. See §3.*
- **Evidence paths gain one directory level.** `src/live/evidence.rs`'s `walk` starts at depth 1 under `cp-{n}` and `DEPTH = 4` (`:18`) admits up to three more levels, so `live --json` needs no change and its schema stays 1.
- **No external contract.** No MCP tool, no public API, no URL.

## 3. FR-1 — A checkpoint's record names the slices it was handed

**Today.** `covers` is computed once from `board.unverified` (`src/checkpoint.rs:66`), travels into the pack (`Pack::for_checkpoint`, `src/pack.rs:413-417`) and into the `docs log add` line the contract gives the worker (`assets/contract-checkpoint.md:17`). Nothing compares the entry the worker wrote with that list.

**Corrected 2026-11-23 — the triage draft had this backwards.** It said a `CP-n` written without `--covers` "leaves slices unverified forever". The truth is worse: `covered()` (`src/docs/board.rs:338-346`) treats an empty `covers` as the high-water rule, so every done slice up to `--slice` counts as covered. An omitted flag does not stall the run, it **marks unproven slices verified**. The fallback is deliberate — `src/docs/log.rs:84` documents it for entries written before the field existed — which is why no test caught the hole.

**Decided 2026-11-23, then reversed the same day.** The first decision removed the fallback outright. The adversarial pass found the live test above, and `assets/autonomy.md:62` puts "skip, delete or weaken a test this run did not write" under **Never** with no "unless the PRD says so" escape, so an unattended run could not have built it. **The fallback stays and the refusal moves to the write side:** `factory docs log add --kind checkpoint` requires `--covers`, so a covers-less entry cannot be created again, while the read fallback and its test keep working for the 18 archived entries that need them. **Accepted cost:** two coverage rules live in `covered()`, one of them only for history nobody builds from. Removing the second is a follow-up once PRD-022 can authorize a test change — see `docs/ideas/2026-11-23-let-a-prd-authorize-a-test-change-provably-without.md`.

**Decided 2026-11-23.** A record that names fewer slices than the driver handed it is **sent back once, then blocks**, the rule `assets/autonomy.md:98` already states for a slice hand-back. **Accepted cost:** a second worker on every forgetful checkpoint. Blocking on the first mismatch loses because a forgotten flag then costs the owner an interruption.

**Constraint found 2026-11-23 (adversarial pass), which the send-back must respect.** A record cannot be withdrawn. `log::add` assigns the id itself as `{seq}-{last+1}` and appends (`src/docs/log.rs:217-237`), the CLI has only `Add` and `Show` (`src/cli/docs.rs:162-191`), and the contract forbids hand edits. So the thin `CP-n` survives the respawn, the respawned worker's record becomes `CP-(n+1)`, and the driver's `n` is frozen at `:66`. Two consequences the slicer must honour:

- The driver's acceptance test reads **the newest checkpoint entry**, not the entry numbered `n`.
- A rejected record must not count as coverage. Two shapes, both open to `/slice`: the driver intersects `board.unverified` with the checkpoints its own `CheckpointDone` events accepted (`src/events.rs:177-180`; no schema change, but `factory docs status` stays optimistic about a parked run), or a rejected entry is marked in the log so `covered()` skips it (an honest board, at the price of a new field — `Flag` is `Fyi | Decide` and `Verdict` is `Kept | Changed`, so neither fits as it stands).

**Acceptance.** `factory docs log add --kind checkpoint` with no `--covers` is refused, naming the flag, and exits non-zero. A worker whose record names fewer slices than the driver handed it is respawned once; a second short record writes a block naming the missing slices and the run parks, with the slices still reading as unverified. A record naming exactly the handed slices closes as it does today. `a_log_without_covers_keeps_the_high_water_rule` is untouched and green.

## 4. FR-2 — A Prove row is proven by its evidence, not by its claim

**Today.** The contract tells the worker to drive every row under `# QA rows`, to put evidence under `$FACTORY_EVIDENCE/cp-{n}/` (`assets/contract-checkpoint.md:15`), to return a `PROVE:` line (`:25`) and to record a `**Prove:**` body line (`:17`). The driver reads none of it. `$FACTORY_EVIDENCE` is the run dir's `evidence/` folder (`src/worker.rs:57-58`, `:261-263`), and names inside `cp-{n}/` are free-form, so nothing maps a file to a row.

**Corrected 2026-11-23 — the driver does not hold the row list, so this FR has to build one.** An earlier draft claimed `qa_rows` gave the driver an exact, FR-keyed list of handed rows. Three things are wrong with that (`src/pack.rs:689-722`):

- `qa_rows` returns a **`String`** that `for_checkpoint` renders into the pack text and discards. `Pack` carries `path`, `prompt`, `files`, `missing_sections` and `parts` — no row list, so no caller holds one.
- It is not handed `covers`. It is handed `named`, the `FR-<digits>` ids scraped from each covered slice's `### Prove` section (`:452-461`).
- When `named` matches no row's first cell, `picked` is empty and **the whole table body is handed** (`:716-722`). That is the normal case for a slice with no user-visible FR, which the scaffold sanctions (`assets/docs/slice.scaffold.md:31`), and it also fires on a first cell like `| FR-1 after merge |`.

So `for_checkpoint` must return the keys of the rows it handed, and the driver must keep them.

**Decided 2026-11-23.** Evidence gets one directory per **row**, not per FR: `$FACTORY_EVIDENCE/cp-{n}/<key>/`, where the key is `FR-<n>` when the FR has one row in the table and `FR-<n>.<k>` for the k-th row when it has several. *Corrected 2026-11-23 (adversarial pass): an earlier draft keyed the directory on the FR id alone. A QA table may carry several rows for one FR, and four PRDs in this repo do — `docs/archive/004-skills/prd.md:223-225` has three `| FR-6` rows — so one `FR-6/` directory would have satisfied the check for three distinct rows.* **Accepted cost:** the contract line changes, the pack must print each row's key beside it, and a harness that saves elsewhere copies into the row's directory. A filename prefix loses because a harness writing several files per row would have to rename each one.

**Decided 2026-11-23.** The check is existence and non-emptiness, not content. **Accepted cost:** a worker could satisfy it with a near-empty file, so this catches the forgotten row, not the dishonest one. Reading the evidence loses because it puts the driver in the business of judging a screenshot.

**Decided 2026-11-23.** The driver checks evidence rather than parsing the worker's `PROVE:` line. **Accepted cost:** the contract's return line and `**Prove:**` body line stay advisory, so a worker's prose can still disagree with the disk; the disk wins and the block says so. Parsing the line loses because it trusts the claim this FR exists to stop trusting.

**Two exclusions, both found 2026-11-23 by the adversarial pass.**

- **A row marked `Human? = yes` is never checked.** Nothing in `src/` reads that column — `grep -rni human src/` finds only `src/closeout.rs:909`, which writes a PR-body pointer — and five such rows exist across this repo's own PRDs (`docs/archive/013-live-status/prd.md:159`, `docs/archive/004-skills/prd.md:219,225,228`, `docs/archive/003-observatory/prd.md:126`). Without this exclusion, FR-2 would block every run whose PRD has one on a row no unattended worker can ever prove. The handed list therefore reads the `Human?` column, and a `yes` row is listed in the record as deferred, not as evidence owed.
- **A PRD with no `| FR-` rows hands nothing,** and the record says `0 rows handed`. *The signal is an empty row body, not the sentinel string: `rows` in `qa_rows` collects lines beginning `| FR` **or `|---`** (`:690-693`), so any other markdown table in the PRD makes `rows` non-empty and `(the PRD has no QA plan table)` never prints.*

**Acceptance.** A checkpoint handed rows `FR-1` to `FR-9` that leaves `cp-1/FR-4/` missing or empty is respawned once, then blocks naming **every** missing row, not the first. A table with three `FR-6` rows needs `FR-6.1`, `FR-6.2` and `FR-6.3`, and two of three is a block. A row marked `Human? = yes` needs no directory and does not block. A PRD with no `| FR-` row closes with `0 rows handed` recorded, even when the PRD holds other markdown tables.

## 5. FR-3 — The last checkpoint re-proves every row the feature declared

**Today.** The pack carries the rows named by the covered slices' `### Prove` sections (`src/pack.rs:452-461`, `:490-492`). So the final checkpoint of a 9-slice feature usually drives the last two slices' rows and nothing else. `board.all_done` (`src/docs/board.rs:326`, exposed at `src/docs/mod.rs:149`) is already on the `Status` the checkpoint reads, so "this is the last one" costs nothing to detect.

*Corrected 2026-11-23 (adversarial pass): an earlier draft said the pack carries **only** the covered slices' rows. That holds only while `picked` is non-empty; when the covered slices' Prove sections name no matching row, the whole table already goes (§4). So "an earlier checkpoint gets only its own rows" is a behaviour this FR establishes, not one it preserves.*

**Evidence.** Two browser proofs in a client feature went red under a later slice's session re-check and stayed red until polish.

**Decided 2026-11-23.** When `all_done` is true, the pack carries every `| FR-` row in the PRD and FR-2's evidence check applies to all of them, minus the `Human? = yes` rows. When it is false, the pack carries the covered slices' rows and nothing else, including when that set is empty. **Accepted cost:** the last checkpoint's Prove step grows from the covered slices' rows to the feature's whole table, and its record says so. On a `run`-row-only feature this largely repeats the gate, which is the price of catching the regression a later slice caused in an earlier slice's screen.

**Acceptance.** On a 6-slice feature with `checkpoint_every = 2`, CP-1 and CP-2 are handed only their covered slices' rows, and a covered slice whose Prove section names no row hands nothing rather than the whole table. CP-3, running with every slice done, is handed every `| FR-` row in the PRD's QA table and blocks on any one, bar a `Human?` row, with no evidence.

## 6. FR-4 — The contract asks for one mutation check per checkpoint

**Evidence.** Three features shipped tests that could not fail. PRD-017's `a_cancel_check_kills_every_running_pass` and `last_reviewed_commit_is_the_one_that_set_reviewed` both passed CP-1 and CP-2, and polish then found neither could fail on the defect it named. PRD-024's second opinion found another. PRD-018's fallback found five needle gaps the gate's own skill-text tests had passed.

**Decided 2026-11-23 (owner).** `assets/contract-checkpoint.md` asks the worker, at each checkpoint, to take the riskiest row it drove, break the one line that row's FR rests on, run its test, see it red, restore, and name the `file:line` and the red in the record. **Wording only: the driver does not check it.**

**Accepted cost, stated plainly.** PRD-018 is the proof that unenforced contract wording gets forgotten: CP-1 dropped two rows under a line that already said "a row you could not reach is reported, not skipped silently". So this FR may buy nothing. The signal that it bought nothing is a retro, after this ships, naming a test that could not fail. Enforcing it was weighed and left out as the owner's call, to keep the checkpoint's new strictness to things the driver can see on disk.

**Acceptance.** The contract names the check in one step, with the record field it writes, and the passage is unique in the bundle (PRD-018 FR-11), pinned by `assert_unique_passage` in `tests/asset_text.rs`.

## 7. Decisions log

No `OPEN-n`. Every fork was settled on 2026-11-23 and lives in the FR that owns it. Two were settled twice, and both reversals are recorded above:

- **The high-water fallback** (§3): removed, then kept with the refusal moved to the write side, because a live gate test pins the branch and `autonomy.md:62` forbids an unattended run from replacing it.
- **Evidence keyed per FR** (§4): changed to one key per table row, because an FR may own several rows.

Settled once: send-back-once over block-first (§3); evidence directories over a parsed return line (§4); existence over content (§4); `Human? = yes` and no-table repos excluded (§4); the whole table at the last checkpoint (§5); the mutation check left unenforced (§6).

## 8. QA plan

**Gate**: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test && claude plugin validate mods/cockpit && claude plugin test mods/cockpit`.
**Surface**: the driver and the `factory docs log add` CLI. There is no UI: every FR is proven below it.
**Harness**: `tests/checkpoints.rs`, `tests/docs_log.rs` and `tests/harness/mod.rs` (`Fixture` at `:128`, `script_gate` at `:472`). `tests/checkpoints.rs` is in the leaks allowlist (`docs/project.toml`, scope `all`), so fixtures may narrate by slice number.
**Seed state**: a fixture feature with 6 slices whose QA table has 6 `| FR-` rows, one of them `Human? = yes` and one FR owning three rows; a fixture PRD with no `| FR-` row but another markdown table; a covered slice whose `### Prove` section names no row; evidence trees with a row directory missing, present-but-empty and populated.

| FR | Proof | Evidence | Human? |
|---|---|---|---|
| FR-1 | run `log_add_checkpoint_refuses_a_missing_covers`, `a_record_naming_fewer_slices_is_sent_back_once`, `a_second_short_record_blocks_and_leaves_the_slices_unverified`, `a_log_without_covers_keeps_the_high_water_rule` | test transcript | no |
| FR-2 | run `a_row_with_no_evidence_directory_is_sent_back`, `an_empty_evidence_directory_counts_as_missing`, `a_block_names_every_missing_row`, `one_fr_with_three_rows_needs_three_directories`, `a_human_row_needs_no_evidence`, `a_prd_with_no_fr_rows_hands_nothing` | test transcript | no |
| FR-3 | run `the_last_checkpoint_is_handed_every_row_in_the_table`, `an_earlier_checkpoint_is_handed_only_its_covered_rows`, `a_covered_slice_naming_no_row_hands_nothing` | test transcript | no |
| FR-4 | run `the_checkpoint_contract_asks_for_one_mutation_check` | test transcript | no |

No row needs a human, so no `qa-guide.md` is generated.

## 9. Slices

_`/slice` fills this._

## 10. Non-goals

- **Parsing a gate's mutation output.** The triage draft's third candidate wanted surviving mutants recorded by `file:line` and handed to the second opinion and to `prune`. Cut 2026-11-23: the **binary** parses no mutation output of any tool, and this repo's own gate declares no mutation tier, so it would ship against a fixture of another tool's format; a 2026-11-20 field report measured the tier at 400-550s per checkpoint catching nothing across 8 runs. *Corrected 2026-11-23 (adversarial pass): an earlier draft justified this with "factory has no mutation concept". It has one, in the skills bundle — `skills/gate/SKILL.md`, `skills/gate/references/tiers.md` and `stacks.md`, plus `skills/prune/` and `skills/harness/`. What is missing is binary support, not the concept.* **Reopens when** a project's gate runs mutation at a cost it is willing to pay and a run shows survivors reaching polish.
- **Authorizing a test change.** Removing `covered()`'s high-water branch needs a test replaced, which no PRD can order today (§3). The mechanism — a base-read list of changeable tests, plus the replacement proven red against the base — belongs to PRD-022 and is `docs/ideas/2026-11-23-let-a-prd-authorize-a-test-change-provably-without.md`.
- **Per-slice gate numbers in `state.toml`.** Cut 2026-11-23: `GateRun { n, attempt, red, minutes, log }` (`src/events.rs:132-138`, emitted `src/checkpoint.rs:554`) already records them, and joining `GateRun.n` with `CheckpointStarted.covers` gives them per slice. A second copy in a file only `factory docs` may write would be two stores for one truth. The real gap is the **builder's inner loop**, which emits nothing: `docs/ideas/2026-11-23-an-inner-run-event-so-a-retro-can-cost-the-inner-l.md`.
- **Not handled: a worker that writes an evidence file to satisfy the check.** FR-2 catches the forgotten row, not a dishonest one. **Reopens when** a retro shows evidence written to pass rather than to prove.
- **What a Prove row says.** Hostile cases, empty submits, narrow-screen drives and option seeds are the row's own wording, in `/slice` and the QA plan. They stay in `docs/ideas/2026-11-20-slice-deliver-three-rules-for-prove-rows-and-the-l.md` (rules 2 and 3); rule 1 is FR-3 here.
- **The gate's cost and its tiers.** A tier that runs only at the last checkpoint is `docs/ideas/2026-11-20-gate-allow-a-tier-that-runs-only-at-the-last-check.md`.
- **Dropped at the 2026-11-05 triage, unchanged:** the `docs check` output redirect in worker packs and the merge pack reading the slice from its commit; the gate's pid, which is PRD-020's.

## 11. Verification notes

Checked against `main` at `8cbf7d2`, 2026-11-23, then re-verified the same day by an adversarial pass (a fresh agent; `factory review second --doc` exited 2 on the vendor's usage limit, which resets 2026-11-24). The pass verified 35 claims and broke 10; every break is corrected inline above. **Re-verify at slice entry.**

- `src/checkpoint.rs:66` — `let covers = board.unverified.clone();` and the point where `n` is frozen.
- `src/checkpoint.rs:296` — `if after.checkpoints.iter().any(|c| c.n == n)`, the whole close condition. *Corrected 2026-11-23: an earlier draft cited `:299`, which is the `Err(o) =>` arm of the `land_here` match below it.*
- `src/checkpoint.rs:190-215` — the respawn loop and `DEATH_CAP`; `:226` the death block's hand-run `docs log add` line, already passing `--covers`; `:263` `drop_ref`; `:554` `GateRun`.
- `src/docs/board.rs:326` — `all_done`; `:338-346` — `covered()` and its high-water branch; `:348` — `unverified`.
- `src/docs/mod.rs:149` — `all_done` on `Status`.
- `src/docs/log.rs:84-86` — the `covers` field and the doc comment making the empty-list fallback deliberate; `:55-65` — `Flag` is `Fyi | Decide` and `Verdict` is `Kept | Changed`; `:195-200` — the existing refusal machinery; `:217-237` — `log::add` assigning `{seq}-{last+1}` and appending.
- `src/cli/docs.rs:162-191` — the log verbs are `Add` and `Show` only.
- `src/pack.rs:413-417` — `for_checkpoint`; `:452-461` — `named` from each covered slice's `### Prove`; `:490-492` — the `# QA rows` section; `:689-722` — `qa_rows`, its `|---` term, its `picked.is_empty()` fallback to the whole body, and the sentinel that only prints when the PRD holds no markdown table at all.
- `src/worker.rs:57-58`, `:261-263` — `FACTORY_EVIDENCE` and the run dir's `evidence/`.
- `src/live/evidence.rs:15`, `:18` — the evidence root and `DEPTH = 4`; `walk` starts at depth 1 under `cp-{n}` and admits three more levels.
- `src/events.rs:132-138` — `GateRun`; `:177-180` — `CheckpointDone { n, sha }`.
- `tests/docs_log.rs:289-308` — `a_log_without_covers_keeps_the_high_water_rule`, with `tests/fixture/log-without-covers/log.jsonl` (2 covers-less entries), both allowlisted in `docs/project.toml`.
- `assets/contract-checkpoint.md:7-8`, `:15`, `:17`, `:25` — the covered-slices and QA-rows sections, the Prove step, the record body, the return line.
- `assets/autonomy.md:48` — the May clause for project docs is text only; `:62` — Never "skip, delete or weaken a test this run did not write"; `:64` — Never record a checkpoint or Prove row without an artifact on disk; `:98` — "sent back once, then blocked".
- `grep -rni human src/` — only `src/closeout.rs:909`, a PR-body pointer; nothing filters the `Human?` column.
- `grep -rn PROVE src/` — empty.
- Counted 2026-11-23: 18 covers-less checkpoint entries across archived 001 to 005; every feature from 006 on carries `covers`. Three `| FR-6` rows in `docs/archive/004-skills/prd.md:223-225`; five `Human? = yes` rows across this repo's PRDs.
- **Dead ends checked:** none. Every path above is live. `land_here` only lands a commit and never reads the record.
