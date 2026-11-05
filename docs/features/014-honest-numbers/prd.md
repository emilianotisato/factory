---
number: 014
slug: honest-numbers
title: "Numbers count the CI rounds"
# type: feature | bugfix | enhancement | refactor
type: bugfix
created: 2026-11-05
author: Emiliano Tisato
summary: |
  A retro's `## Numbers` is written at ship, so CI fix workers that run after it are missing: 6 of 54 shipped runs under-report, one by 17%. Every push after ship now rewrites the section and, when it changed, commits it ahead of the push. Agreed 2026-11-05; one FR, no slices.
---

# PRD-014 — Numbers count the CI rounds

## 1. What happens vs what should

**Happens.** The driver writes the retro's `## Numbers` section once, in the ship step (`src/closeout.rs:936`), and commits it in the ship commit. CI fix workers spawn later, from `watch_ci` (`src/closeout.rs:1220`), so their tokens, minutes and turns never reach the section. Nothing rewrites it unless the owner runs `factory retro <run>` by hand (`src/cli/retro.rs`).

**Should.** The section a PR carries counts every worker the run spawned before that push.

**Impact, measured 2026-11-05** over the metrics store (64 runs, 584 workers): 6 of 54 shipped runs spent tokens after ship, all in CI fix workers. Five missed 2–6% of their total; `004-20261030-7551` missed 7.5M of 43.1M raw tokens, about 17%. The Numbers table is what knob experiments compare (one variable per PRD), so a missing CI round can fake a saving or hide a cost.

**Why it happens.** Not a slip: PRD-003 FR-25 settled "written at ship" and left the CI rounds open (idea of 2026-10-13, now this PRD).

## 2. FR-1 — Every push after ship carries current Numbers

**Today.** `fix_round` (`src/closeout.rs:1808`) lands the fix (`land_here`, called at `:1951`, defined at `:597`), then `settle` (`:2267`) writes the worker's usage and subagent counts to the store. The watch loop then calls `push` (`:1378`, defined `:1107`). `open_pr` calls the same `push` at `:1015`, *before* `ci::pr_create` (`:1079`), and it runs on every `closeout::run`, a resumed one included, even when `pr_url` is already set.

> *Corrected 2026-11-05 (adversarial pass): an earlier draft called `:1015` "the first push after the PR opens". It runs before the open; the PR opens on what it sent.* `retro::write` (`src/retro.rs:277`) rewrites only the text between the section's markers and returns whether anything changed; `retro::numbers` (`src/retro.rs:149`) computes the section from the store.

**Decision (2026-11-05, owner).** `push` rewrites `## Numbers` before it pushes, and when the section changed, commits the retro alone as `docs(NNN): refresh numbers` through `commit::commit`, the helper the ship commit uses (`src/closeout.rs:938-942`). The push then sends the fix and the docs commit together: one push, one CI run, no extra round. The push in `open_pr` right after ship finds the section unchanged and commits nothing (`commit::commit` returns `None` on an empty pathspec diff).

- **Why in `push`, not in `fix_round`:** a resumed driver lands an earlier driver's CI fix worker in `settle_orphan` (`src/closeout.rs:1971`, landing `:2060`, settle `:2075`, called from `src/scheduler.rs:1619`), which does not push; the branch reaches origin through `open_pr`'s `push` at `:1015` on the same resume. A refresh in `push` covers both roads; one in `fix_round` would miss the resume.
- **The subject names no round:** `push` does not know it, and on the resume road no round is in scope.

> *Corrected 2026-11-05 (adversarial pass): an earlier draft said "every road to the branch goes through `push`" and had `settle_orphan` push. It lands without pushing; the resume's `open_pr` push publishes it. An earlier subject, `numbers after CI round n`, needed a round `push` does not have.*
- **A failure to compute or write the section never fails the push.** It is said in one line, as the ship step says a write failure (`src/closeout.rs:936-937`: `ship: no ## Numbers written — …`), and the push goes ahead. The ship step itself still fails on a compute error (`retro::numbers(..)?`); this PRD leaves ship alone.
- **Accepted cost:** one more commit on the PR branch per CI fix round; the run's events show the fix commit is no longer `HEAD` when the push happens, and the existing closeout tests that pin `HEAD` to the fix commit move with it.
- **Lost alternative:** label the section "as of ship" and leave it. Cheaper, but every run with a CI round keeps reporting the wrong figure, and the figure is what experiments compare.

**Acceptance.**

1. A run with one red CI round ends with a retro whose Numbers `fix` row counts the CI fix worker, and the branch's last commit before the green CI run is the docs commit.
2. The PR opens on the ship commit: the push in `open_pr` adds no docs commit when no worker ran after ship.
3. A Numbers write that fails (the retro holds a second opening marker, so the markers are not one ordered pair, `src/retro.rs:294-299`) still pushes, and the driver log says why the section was not written.

> *Corrected 2026-11-05 (adversarial pass): criterion 3 first said "two section markers", which is one valid pair and the success arm (`src/retro.rs:283`).*

## 3. QA plan

**Gate**: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test && claude plugin validate mods/cockpit && claude plugin test mods/cockpit` *(declared in project.toml)*
**Surface**: CLI — the driver's closeout and the `retro.md` a PR carries.
**Harness**: gate-only: the driver's closeout tests run it end to end against the stub `claude` and `gh` in `tests/closeout.rs`.
**Seed state**: none needed; the stub plan scripts the red CI round.

| FR | Proof on the real surface | Evidence | Human? |
|---|---|---|---|
| FR-1 | `run cargo test --test closeout`: `a_red_check_gets_one_fix_round_then_green` finds the CI fix worker in `retro.md`'s `fix` row and the docs commit as the tip `ci_green.head` names; `a_finished_board_polishes_ships_pushes_and_opens_the_draft_pr` and `a_flaky_check_is_rerun_once_then_green` still end on `docs(001): ship PRD-001` with no docs commit; a new test with a second opening marker in the retro sees `pr: pushed` and the skipped-write line in the driver log | test transcript | no |

## 4. Slices

No slices — build straight from this document.

## 5. Non-goals

- **Not handled: a missing transcript stores zeros.** Accepted 2026-11-05: the zeros are deliberate for a worker killed before its first turn (`src/metrics.rs:448`), and the feared case, a wrong path rule hiding real subagents, has not happened: all 513 workers stored with zero subagents after real turns have their transcript on disk, and none of them had subagents. Revisit if `factory report --recount` ever finds subagents behind a stored zero.
- **Not handled: partial sums in the Numbers table.** Accepted 2026-11-05: the only 11 null-count workers are PRD-003's own build runs, and each null covers a whole role, which prints `—`, never a partial sum. No run mixes null and counted workers in one role. Revisit if one does.
- **Not handled: tokens after the last push.** No worker runs after it; the owner's own review sessions are not the run's.
- **`factory retro <run>`** stays as it is, the owner's manual rewrite (e.g. after `--recount`).

## 6. Verification notes

Checked against `main` at `c4ced84`, 2026-11-05. **Re-verify at slice entry.**

- `src/closeout.rs:936` — ship writes Numbers once; the only other writer is `factory retro` (`src/cli/retro.rs:50`).
- `src/closeout.rs:1370-1380` — `fix_round` then `run.reload()` then `push`; `fix_round` settles the worker (store written) before it returns `Ok(None)` (`:1963`).
- `src/closeout.rs:2267-2280` — `settle` writes usage, subagents and exit, so Numbers computed after it count the worker.
- `src/closeout.rs:1015` — `open_pr` pushes before `ci::pr_create` (`:1079`), on every `closeout::run`, resume included.
- `src/closeout.rs:1971-2075` — `settle_orphan` lands (`:2060`) and settles (`:2075`) an earlier driver's worker, and does not push.
- `src/closeout.rs:1951` — `land_here` call in `fix_round` (defined `:597`); `ci_fix_committed` is emitted at land time, before `push`, so its `sha` stays the fix commit.
- `src/worker.rs:133` — CI fix workers are role `fix`, so they land in the `fix` row with checkpoint fix workers.
- `src/retro.rs:277-299` — `write` replaces between one ordered pair of markers (`:283`), appends when there are none, and refuses any other marker count (`:294-299`).
- `src/metrics.rs:448-466` — `TranscriptMissing` stores zeros on purpose (doc comment).
- Metrics store, 2026-11-05: 584 workers, 64 runs; 9 `ci_fix_spawned` events in 6 runs; post-ship tokens per run 0.7M–7.5M (2–17% of the run's total).
- `tests/closeout.rs` `a_red_check_gets_one_fix_round_then_green` — pins the event order (`:1127-1148`), `ci_fix_committed.sha` (`:1149`), the tip subject `fix(001): CI test` (`:1082-1088`) and the working-tree retro equal to the ship commit's (`:1094-1097`); the last two move with this FR.
- `tests/closeout.rs:449-464` — the PR-open test pins `git log -1` to `docs(001): ship PRD-001`; it must not move.

## 7. Source ideas

The three ideas this PRD absorbed at the 2026-11-05 triage; two were cut on the evidence above.

### A missing transcript in a live run stores zeros

_added 2026-10-13_

FR-21 treats "no transcript" like "no `subagents/` dir", which is right for a worker killed before its first turn but hides a wrong path rule. Store nulls plus `subagents_unread` only when the worker ran turns. PRD-003 polish review, 2026-10-13 (`tests/checkpoints.rs` kill-and-resume relies on today's zeros).

### Mark partial figures in the retro's Numbers table

_added 2026-10-13_

A role with one worker whose `sub_*` are null sums the others' and prints a figure that looks whole; pre-v2 rows in "Compared with" are own tokens only. Print a `*` and a footnote in both cases. PRD-003 polish review (second opinion), 2026-10-13.

### Refresh `## Numbers` after the CI rounds

_added 2026-10-13_

`factory retro` runs at ship, so CI fix rounds and later polish tokens are missing from the section until the owner reruns it. Either the driver reruns it after CI green, or the section says "as of ship". PRD-003 polish review (second opinion), 2026-10-13; FR-25 settles today's behaviour.
