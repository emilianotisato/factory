# Slice 05 — Skill, contract, 0.5.0

**Source**: [`prd.md`](prd.md) → §9 FR-7. **Depends on**: 03 (the skill describes the file `factory review collect` writes and the verb's flags). Independent of 04: this slice edits the skill, the contract asset and the version, and 04 edits the driver and the pack builder.
**Created**: 2026-10-15

**Progress**: Polish §1 now starts from the collected file: the pack's `# Collected` section in a run; otherwise `factory review collect --base … --out <scratchpad>/collect.md` in the background; a collect that leaves no file → the last resort (`factory leaks --base`, `factory review second --diff`, with their exit codes and `--others`). §1 reads the plan's `size:` and `second opinion:` rows, the leaks `verdict:` and each `### Pass` status; the findings text waits for §2, which gains one line pointing at it. The table drops the second opinion (the size row decides it) and keeps picking security and UX; the fallback paragraph keeps its words, triggered by `fallback owed` or a failed last resort. The polish contract's Read-first list gains the `# Collected` item between `# Branch` and `# Log`. Crate 0.5.0, `skills/bundle.lock` rewritten. New test `polish_collect_starts_from_the_collected_file_with_the_last_resort_beside_it` (checked red with `collect` renamed in §1, then restored). Inner loop: 1 run, 0 red, 2.0 min (plus the 1.6 min `FACTORY_BUNDLE_WRITE=1 cargo test` lock rewrite). Deviation: one sentence lets the worker run `factory review second --diff` itself when the size row skipped it on a blast-radius change, per prd §5's accepted cost.
Checkpoint 2 (FR-7): gate green at 0.5.0 with `bundle_lock_matches`, `polish_collect_starts_from_the_collected_file_with_the_last_resort_beside_it` and `a_finished_board_polishes_ships_pushes_and_opens_the_draft_pr` passing; the polish pack's contract lists `# Collected` before `# Log`: `docs/features/007-polish-in-the-binary/evidence/cp-2-gate-0.5.0.txt`, transcript `docs/features/007-polish-in-the-binary/evidence/cp-2-gate.log`.

**Slice**: After this slice, polish §1 starts from the collected file. In a run, the pack's `# Collected` section names it. Attended, or when the pack says `collect failed`, the skill runs `factory review collect` in the background. When that fails too, §1 falls back to today's `factory leaks --base` and `factory review second --diff`. The size table and the two override rules still pick the security and UX passes. The polish contract gains one line pointing §1 at `# Collected`, and the crate goes to 0.5.0 with a rewritten `skills/bundle.lock`. Releasing the tag stays the owner's.

## Current code / environment

- **Polish §1 today**: `skills/polish/SKILL.md:29-62`. The size table (`:33-38`) and the two override rules (`:40`) pick every reviewer, the second opinion included. The read-only rule for reviewers (`:42`). "Local review first, always … running it first also stops the second opinion's noise from anchoring the local pass" (`:44-46`). Leaks, with its exit-code reading (`:48`). The security pass (`:50`) and the UX pass (`:52-54`). The second opinion, its exit codes and the `--others` sentence (`:56-60`). The fallback, one fresh agent per pass (`:62`). The working file `findings.md` lives in the scratchpad (`:27`), so `<scratchpad>/collect.md` sits beside it.
- **The second opinion now runs before the local review.** The driver's collect runs it before the worker spawns. The skill's anchoring rule still holds through FR-5: "The findings text stays in the file; the worker reads it at §2, so it does not anchor the local review." Attended, the collect can run in the background while the local review runs.
- **The collect's exit code says only that the file was written** (prd §5: "Exit 0 when the file was written, whatever each part found"). The verdicts are in the file: the leaks part's clean, hits or did not run, and `fallback owed` in the plan. Today's rules for reading `factory leaks` and `factory review second` exit codes (`:48`, `:60`) become rules for reading those lines. "Did not run" is never "no hits", and an `unreviewed` or `timed out … partial` pass is never "no findings".
- **The size table no longer decides the second opinion.** The collect's size row does (prd §5: "one file and under 50 changed lines needs the local review only; anything larger adds the second opinion"). Its accepted cost is the owner's: *(prd §5 "Accepted cost: the 50-line threshold is a number, not the table's judgment.")* A table row that still lists the second opinion, like "all of them" (`:38`), would have the worker run it again on a small diff.
- **This feature's own polish runs under the 0.4.0 driver.** This repo's `.claude/skills` link into `skills/` (`tests/skills_bundle.rs:50-60`), so the run building PRD-007 reads the new §1. The installed driver builds a pack with no `# Collected` section, and the installed binary has no `collect` verb (clap refuses it). A pack with no `# Collected` section has to take the attended path, and the unknown verb has to land on the last resort. That is why prd §9 keeps the last-resort line.
- **The contract**: `assets/contract-polish.md`, embedded with `include_str!` as `CONTRACT_POLISH` (`src/pack.rs:13`) and dropped into the pack's `# Contract` (`:468`). Its Read-first list (`:3-6`) names `# Autonomy`, `# Branch`, `# Log`. The contract's text must not start a line with a heading the stub `claude` routes on (`# Branch`, `# Conflict`, `# Gate failure`, `# CI failure`, `# Gate transcript`; `tests/stubs/claude:138-147`), and `tests/closeout.rs:430` asserts the pack holds no `# Gate transcript`.
- **Tests that read these files**: `a_finished_board_polishes_ships_pushes_and_opens_the_draft_pr` lists strings the polish pack must hold (`tests/closeout.rs:407-424`), its contract lines among them. `models_md_holds_no_invocation_and_the_skills_call_review_second` asserts `skills/polish/SKILL.md` holds `factory review second` (`tests/skills_bundle.rs:68-93`); the last resort keeps it. `polish_fixers_run_one_after_another_with_no_lane_rule` reads §5 only (`:95-107`). `bundle_lock_matches` (`:133`) goes red on any skill edit until the lock is rewritten.
- **The version**: `Cargo.toml:3` is `version = "0.4.0"`; `skills/bundle.lock` records the version and the bundle's sha256. README §Releasing (`README.md:99-107`) is the procedure: the bump goes in the feature's change, `FACTORY_BUNDLE_WRITE=1 cargo test` rewrites the lock, and `scripts/release.sh` tags later on `main`.
- **Deliberate non-changes**: *(prd §9 "The fallback text is unchanged: one fresh agent per pass, when the file says `fallback owed`.")* Keep `:62`'s words; only its trigger moves to the file. *(prd §13 "The local, security and UX reviews stay model work.")* The security and UX paragraphs (`:50-54`) keep their lists. *(prd §13 "Not handled: `--others` inside a run.")* The `--others` sentence stays with `factory review second`, where it already is; add no `--others` step after the collect.

## Tasks

### 1. §1 starts from the collected file — high — blocked by: —
**Where**: `skills/polish/SKILL.md:29-62`.

**Source**: §9 FR-7: "§1 starts from the collected file. In a run, the pack names it. Attended, or when the pack says `collect failed`, the skill runs `factory review collect --base <base> --out <scratchpad>/collect.md` in the background, since it can outlast a Bash call. If that fails too, §1 keeps today's two commands as the last resort: `factory leaks --base` and `factory review second --diff`. The size table and the two override rules still pick the security and UX passes. The fallback text is unchanged: one fresh agent per pass, when the file says `fallback owed`."

Rewrite §1's opening and its leaks and second-opinion paragraphs around the file:

- **Where the file comes from.** A pack's `# Collected` section names it. With no `# Collected` section, attended, or `collect failed`: run the collect in the background and read the file when it exits. A collect that does not write the file (an error, or a binary without the verb) → the last resort, said out loud.
- **What §1 reads from it.** The plan's size row, the leaks verdict, and each pass's status line. The findings text waits for §2, so the local review still runs first, unanchored.
- **Reading the verdicts.** Leaks hits are Fix-bucket findings, as today. "Did not run" is said out loud and never reported as no hits. `fallback owed` starts the fallback, said out loud; an `unreviewed` or `timed out … partial` pass names the files it did not finish.
- **The table.** The size row decides the second opinion. The table and the two override rules pick the security and UX passes, so no row lists the second opinion as the table's choice.

The last resort keeps today's two paragraphs for `factory leaks --base` and `factory review second --diff`, with their exit codes and the `--others` sentence.

**Acceptance:** `skills/polish/SKILL.md` names `factory review collect`, `factory leaks --base` and `factory review second --diff`; `models_md_holds_no_invocation_and_the_skills_call_review_second` stays green; task 3 pins the first two.

### 2. The contract's line — high — blocked by: —
**Where**: `assets/contract-polish.md:3-6`.

**Source**: §9 FR-7: "The contract gains one line: the `# Collected` section holds the plan, the leaks verdict and the second opinion, and §1 starts from it."

One item in the Read-first list, between `# Branch` and `# Log`, the pack's order once slice 04 lands. The line mentions `# Collected` inside backticks, never at the start of a line.

**Acceptance:** `a_finished_board_polishes_ships_pushes_and_opens_the_draft_pr` (`tests/closeout.rs:407-424`) lists the line among the strings the polish pack must hold, and the test is green.

### 3. 0.5.0 and the lock — high — blocked by: 1, 2
**Where**: `Cargo.toml:3`, `skills/bundle.lock`, `tests/skills_bundle.rs`.

**Source**: §9 FR-7: "The skill edit bumps the crate to **0.5.0** and rewrites `skills/bundle.lock` (`FACTORY_BUNDLE_WRITE=1 cargo test`)." And its acceptance: "The bundle-lock test (`tests/skills_bundle.rs`) is green with `skills/bundle.lock` at 0.5.0 … `skills/polish/SKILL.md` names `factory review collect` and the last-resort commands."

Bump the version, then run `FACTORY_BUNDLE_WRITE=1 cargo test` once, after the skill text is final. Add one test beside `models_md_holds_no_invocation_and_the_skills_call_review_second` that reads polish §1 (from `## 1.` to `## 2.`) and asserts it holds `factory review collect`, `factory leaks --base` and `factory review second --diff`.

**Acceptance:** `skills/bundle.lock` reads `version = "0.5.0"`; `bundle_lock_matches` is green; the new test fails when any of the three commands leaves §1.

### 4. Verify — medium — blocked by: 1, 2, 3
`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 5. Prove — at the checkpoint — blocked by: 4
Rows from [`prd.md`](prd.md) §11 this slice proves, below the UI:

- **FR-7**: "run the gate: bundle lock green at 0.5.0, the polish pack's contract starts §1 from `# Collected`" — the checkpoint's gate transcript, with `bundle_lock_matches`, the new §1 test and `a_finished_board_polishes_ships_pushes_and_opens_the_draft_pr` named in it.

**Acceptance:** after the checkpoint, Progress holds the gate transcript.

**Code-review fixes (2026-10-15).**

- The contract line said `factory review collect` wrote the `# Collected` section, which reads as a command the worker runs. It now says the driver collected it before the spawn.
