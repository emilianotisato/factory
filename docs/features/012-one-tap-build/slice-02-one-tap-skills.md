# Slice 02 — Skills: the one-tap flow, 0.11.0

**Source**: [`prd.md`](prd.md) → §3 FR-1, §4 FR-2, §5 FR-3, §6 FR-4, §7 FR-5, §8 FR-6 (the skill half), §11 QA plan. **Depends on**: — (independent of slice 01, which touches only Rust, the scaffold and tests. The `not sliced — /slice NNN` line this slice's text relays is slice 01's, quoted from the PRD, so no build order is needed)
**Created**: 2026-10-30

**Progress**:
- **Built (2026-10-30).** `skills/_shared/stale-tooling.md`: the four skills-line bullets, hand edit first, in `FILES`. `/prd` §0 links it, §2 and §6 drop Blast radius when nothing outside the FRs can break, and §8's agreed bullet is one Build it now (Recommended) / Slice only / Stop here question with the `/clear` advice gone. `/slice`: `NNN from /deliver` skips `Write these?` only; `/factory-deliver` → `/deliver`. `/deliver` (50 lines): step 0 runs doctor and the rule, then slices on `not sliced`; step 3 clears the mechanical `✗` lines and asks on the rest; a 55-minute `--timeout 3300` poll; §3 retitled; §4 also covers a compaction. `/lanes` §0 links the rule. 0.11.0, lock relocked.
- **Tests.** New: `the_prd_skill_offers_the_build_and_never_says_clear`, `the_slice_skill_skips_the_write_question_only_for_deliver`, `every_doctor_step_links_the_stale_tooling_rule`. `the_cockpit_skill_…` now needs `--timeout 3300`, `not sliced` and `from /deliver`, and refuses `--timeout 1500`. `tests/init.rs`'s bundle count goes 21 → 22 files for the new shared file.
- **Inner loop:** 3 full runs, 2 red, ~4 min. Red 1 was a flake in `tests/closeout.rs` (passed 3/3 alone, and green on the next full run). Red 2 caught the hard-coded `21 files` in `tests/init.rs`.
- **Left for the checkpoint:** the full gate and task 7's FR quotes. The session rows FR-1, FR-2 (the `/deliver` half), FR-3, FR-4 and FR-5 are proven on the owner's next feature after the merge.
- **Checkpoint 1 (2026-10-30).** Gate: 1 full run, 0 red, 1.9 min, green. After the leaks fix the inner loop is green again (`cp-1/inner-loop.log`). The quotes are in `cp-1/skill-passages.txt`:
  - FR-1: `skills/prd/SKILL.md:165` "the last act is one `AskUserQuestion`: **Build it now (Recommended)** / **Slice only** / **Stop here**", and `:166` "**Build it now** → invoke `/deliver NNN` with the Skill tool, in this session".
  - FR-2 (`/deliver` half): `skills/deliver/SKILL.md:12` "`not sliced` → invoke `/slice NNN from /deliver` with the Skill tool first", and `skills/slice/SKILL.md:114` "**From `/deliver`** → print the board without the `Write these?` line and create the slices straight away".
  - FR-3: `skills/deliver/SKILL.md:16` "`working tree not clean` with every dirty path under `docs/features/NNN-*` → commit them by pathspec, `docs(NNN): …`. Any other dirty path → ask."
  - FR-4: `skills/_shared/stale-tooling.md` "**`behind`, `differs` or `missing`**, no hand edit → `factory skills update`, then the `commit:` line it prints, on the current branch". The `hand-edited:` line still asks first. `/prd`, `/slice`, `/deliver` and `/lanes` all link the file.
  - FR-5: `skills/deliver/SKILL.md:23` "Run `factory wait <id> --timeout 3300` in a background Bash whose own `timeout` is above 3,300,000 ms", and `:48` "## 4. A dead cockpit, a compaction, a lost context".
  - Post-merge: a checkpoint cannot drive a live `/prd` or `/deliver` session. So the session rows FR-1 (Human), FR-2's `/deliver` half, FR-3, FR-4 and FR-5 are proven on the owner's next feature after the merge. The feature ships as `shipped`, not archived.

**Slice**: After this slice, an agreed PRD ends with one Build / Slice / Stop question, and Build runs `/deliver NNN` in the same session. `/deliver` updates stale tooling, slices an unsliced PRD with no "Write these?", fixes the preflight refusals it can fix alone, starts the run, and waits with a 55-minute poll. `/prd` treats Blast radius as optional. **All skill text of PRD-012 is in this slice**, together with the one version bump to 0.11.0: once the bundle changes under a version, `relock` refuses a second change until the version moves again (`src/skills.rs:161-178`). **Highest risk:** `/deliver` must stay under 60 lines (task 3's trap).

## Current code / environment

- **`/prd` §8 hand-off**: `skills/prd/SKILL.md:158-170`. The "built without them" bullet at `:165` says `/slice`, then `/clear`, then `/deliver NNN`, and "Say the `/clear` part". §7 sets `ready` and commits at `:156`.
- **`/prd` §0 doctor routing**: `skills/prd/SKILL.md:20-31` (Current / Not on the framework / On v2 / Drifted). It has no skills-verdict branch today.
- **Blast radius in `/prd`**: the interview bullet at `skills/prd/SKILL.md:59` and the §6 bullet at `:131`.
- **`/slice`**: "Slicing is attended work." `skills/slice/SKILL.md:13`. §0 routes "Not on the framework, behind, or drifted" to `/prd` (`:23`). The target comes from `$ARGUMENTS` (`:25`). The small-PRD line names `/factory-deliver` (`:37`). The unsourced-task question is `:87`. The board ends "Write these? [Y/n]" (`:107`), and "On approval" (`:114`).
- **`/deliver`**: 44 lines (`skills/deliver/SKILL.md`). §1 runs `factory run NNN` (`:12`) and drives a live or resumed run's id (`:13`). §2 waits with `--timeout 1500` and says a long Bash "was not verified" (`:17`). §3's title says "as `/deliver` §8" (`:33`). §4 is the dead-cockpit re-entry (`:42-44`).
- **`/lanes` §0** runs doctor at `skills/lanes/SKILL.md:15-19` and routes at `:21-24`.
- **The skills verdict**: a version line (`behind (repo X, binary Y) → factory skills update`, or `ahead (…) → <hint>`), then `hand-edited: …` and `missing: …`, printed together when several apply (`src/skills.rs:474-491`). The ahead hint is `factory upgrade` only for a release install; from source it is the `cargo install` line (`src/cli/upgrade.rs:40-47`). `factory skills update` refuses while any file is hand-edited and prints one `git checkout -- …` per file (`src/cli/skills.rs:55-65`). On success it prints the exact commit line (`:91-96`). In this repo doctor says `skills source checkout`.
- **Preflight from `factory run`**: a refusal prints `  ✗ <line>` and exits 1 (`src/cli/run.rs:93-99`). Asks already become `ASK-n` events that `/deliver` §2 relays (`:106-127`). `ensure_branch` makes the feat branch from the default branch (`:131`, `:289`). A dirty tree refuses as `working tree not clean (n path(s)) — commit or stash first` (`src/docs/preflight.rs:172-175`).
- **`factory docs status NNN`** prints `slices    not sliced` for a feature with none (`src/cli/docs.rs:638-644`).
- **`factory wait`** returns on cockpit events only; progress shows in `factory status` (`src/cli/wait.rs:54-57`, `src/cockpit.rs:13-14`). On 2026-10-30 a background Bash ran a 15-minute `sleep` to the end in this repo's sessions; Claude Code docs give a 2 h maximum when the call's `timeout` asks for it.
- **Not handled** (prd §13): no skill triggers `/compact` or `/clear`; no subagent cockpit; no new terminal session; `/gate` and `/lanes` are never run unasked. Do not add any of them here.

## Tasks

### 1. `/prd` offers the build — high — blocked by: —
**Source**: §3 FR-1: "When §7 has set the PRD `ready` and committed it, §8 asks one `AskUserQuestion`: **Build it now (Recommended)** / **Slice only** / **Stop here**." "The `/clear` sentence goes." And §8 FR-6: "`/prd` §2 and §6 say the same."

`skills/prd/SKILL.md`.
- Rewrite the §8 "built without them" bullet (`:165`) into the question. Build → invoke `/deliver NNN` with the Skill tool, in this session. Slice only → `/slice NNN`. Stop → the hand-off line. Keep "Running it is the approval … It never merges." Keep the commit on `feat/{slug}`. Drop "Say the `/clear` part" and the context sentence after it. Add one line: the owner may still `/clear` and type `/deliver NNN` at any time.
- A PRD still `draft` gets no build offer: the "Still open" bullet stands.
- In the §2 interview bullet (`:59`) and the §6 bullet (`:131`), say Blast radius is dropped, and the sections renumbered, when nothing outside the FRs can break.

**Acceptance:** `the_prd_skill_offers_the_build_and_never_says_clear` in `tests/skills_bundle.rs`: `prd/SKILL.md` holds `Build it now (Recommended)`, `Slice only`, `Stop here` and `/deliver NNN`, and does not hold `Say the \`/clear\` part`.

### 2. `/slice` reports and writes when `/deliver` calls it — high — blocked by: —
**Source**: §4 FR-2: "`/slice`, when `/deliver` called it, **reports the plan and writes it** with no "Write these?". It still asks only where it would stop today: a task with no source in the PRD, a load-bearing `OPEN-n`, no gate, no QA plan." "That line's `/factory-deliver` becomes `/deliver`." "`/slice` typed by the owner keeps "Write these? [Y/n]"."

`skills/slice/SKILL.md`.
- `/deliver` passes `NNN from /deliver` as the arguments. In §0 (`:25`) the number still resolves the target.
- In §5 (`:107`, `:114`): with `from /deliver`, print the board and go straight to "create each slice". Without it, ask as today.
- At `:13`, keep "the run builds what you wrote here and never re-slices". Say slicing is attended unless `/deliver` called it.
- At `:37`, replace `/factory-deliver` with `/deliver`.
- The questions at `:87` (unsourced task), §1 (load-bearing OPENs, no QA plan) and §0 (no gate) stay in both modes.

**Acceptance:** `the_slice_skill_skips_the_write_question_only_for_deliver` in `tests/skills_bundle.rs`: `slice/SKILL.md` holds `Write these? [Y/n]` and `from /deliver`, and does not hold `/factory-deliver`.

### 3. `/deliver` slices, clears refusals and waits 55 minutes — high — blocked by: —
**Source**: §4 FR-2: "`/deliver` §1 checks `factory docs status NNN`. No slices → it invokes `/slice NNN` first." §5 FR-3: "On a refusal, `/deliver` fixes the mechanical ones and runs `factory run NNN` again", with its three bullets, and "Everything else is relayed as one `AskUserQuestion` each, with the fix it needs as option 1". §7 FR-5: "`factory wait <id> --timeout 3300` in a background Bash whose own `timeout` is above 3,300,000 ms"; "after a compaction or a lost context, `/deliver` resumes at §4"; "Fix §3's title."

`skills/deliver/SKILL.md`.
- §1, before `factory run NNN`: step 0 (task 4's link). Then, when `factory docs status NNN` prints `not sliced`, invoke `/slice NNN from /deliver`.
- §1, after `factory run NNN`: on exit 1, read each `✗` line.
  - A dirty tree whose paths are all under `docs/features/NNN-*` → commit them by pathspec with `docs(NNN): …`.
  - `docs check` errors in that feature's files → fix them and commit.
  - `not sliced — /slice NNN` → slice as above.
  - Then run `factory run NNN` again.
  - Every other `✗` line gets one `AskUserQuestion`, with its fix as option 1. Never run `/gate` or `/lanes` unasked.
- §1: one line saying the owner may `/clear` and type `/deliver NNN` at any time, and §1's live-run line picks the run up.
- §2: `--timeout 3300`, and the Bash call's own `timeout` above 3,300,000 ms. Replace the "was not verified" sentence with the reason: a 55-minute poll stays inside the 1-hour prompt cache.
- §3 title: "Review interview", without "as `/deliver` §8".
- §4: also the place to resume after a compaction or a lost context. `factory status` lists live runs when the id is gone.

**The trap:** `the_cockpit_skill_is_under_sixty_lines_and_names_the_five_commands` (`tests/cockpit.rs:501-535`) caps the file at 59 lines and pins `--timeout 1500`. Change the needle to `--timeout 3300`, and keep the 60-line cap: the cockpit is thin on purpose. That is why the step-0 rule lives in a shared file (task 4), and `/deliver` links to it in one line. The same test forbids `emiliano`, `/home/` and `Dropbox` in the file.

**Acceptance:** `the_cockpit_skill_is_under_sixty_lines_and_names_the_five_commands`, updated: under 60 lines, `--timeout 3300` and `not sliced` and `from /deliver` present, `--timeout 1500` absent.

### 4. Stale tooling updates itself, in one shared file — high — blocked by: —
**Source**: §6 FR-4: "In `/prd`, `/slice`, `/deliver` and `/lanes`, step 0 acts on the skills line without asking", with its four bullets: a hand edit asks first and wins, ahead runs the command after the `→`, behind / differs / missing runs `factory skills update` and then the commit line it prints, and a source checkout does nothing.

- New `skills/_shared/stale-tooling.md`, holding the four bullets and the reason a hand edit wins (`factory skills update` refuses while one stands, `src/cli/skills.rs:55-65`). The commit goes on the current branch, the line `factory skills update` prints.
- Add it to `FILES` in `src/skills.rs:25-29`, beside `_shared/models.md`, or `every_file_under_skills_is_in_the_bundle_byte_for_byte` (`tests/skills_bundle.rs:27`) fails.
- `/prd` §0 (`:20-31`), `/slice` §0 (`:23`), `/lanes` §0 (`:15-24`) and `/deliver` §1: one line each that applies it after `factory docs doctor` (in `/deliver`, run doctor first). In `/slice`, "behind" in `:23` keeps meaning the docs framework. Route the skills line to the shared file.

**Acceptance:** `every_doctor_step_links_the_stale_tooling_rule` in `tests/skills_bundle.rs`: `prd`, `slice`, `deliver` and `lanes` `SKILL.md` each link `_shared/stale-tooling.md`, and the shared file holds `hand-edited`, `factory skills update` and `source checkout`.

### 5. Bump to 0.11.0 and relock — high — blocked by: 1, 2, 3, 4
**Source**: the repo's release rule (`README.md:103`): a skill edit forces a bump in the PR.

`Cargo.toml` `version = "0.11.0"`, `Cargo.lock` follows, then `FACTORY_BUNDLE_WRITE=1 cargo test` rewrites `skills/bundle.lock`. Check `assets/lookup.md` and any other file the 0.10.0 bump touched (`git show --stat 15d038e`) for a version string.

**Acceptance:** `bundle_lock_matches` (`tests/skills_bundle.rs:203`) is green, with the lock at `0.11.0`.

### 6. Verify — medium — blocked by: 5
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 7. Prove — at the checkpoint — blocked by: 6
§11 QA plan rows **FR-1**, **FR-2** (the `/deliver` half), **FR-3**, **FR-4** and **FR-5** need a live Claude Code session driving `/prd` and `/deliver`. A checkpoint inside a run cannot drive one. At the checkpoint:
- Run the full gate.
- Read each rewritten passage against its FR's **Acceptance**, quoting the line that meets it.
- Record in **Progress** that the session rows are proven on the owner's next feature after the merge. FR-1 is the one Human row. The feature ships as `shipped`, not archived, because these proofs outlive the merge.

**Acceptance:** Progress holds the gate result, one quoted line per FR, and the post-merge line.
