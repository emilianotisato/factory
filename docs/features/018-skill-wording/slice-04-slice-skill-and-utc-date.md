# Slice 04 — Slice skill and the UTC date

**Source**: [`prd.md`](prd.md) → §4 FR-2, §9 FR-7 (the `/slice` half), §11 FR-9, §13 FR-11 (the `/slice` §4 sentence), §15. **Depends on**: 02. Both edit `skills/polish/SKILL.md` (this slice adds its UTC sentence), both rewrite `skills/bundle.lock`, and both add tests to `tests/skills_bundle.rs`. Land 02's bump first, so this slice only relocks.
**Created**: 2026-11-12

**Progress**:

- **Built.** `/slice` §2 has a new bullet: two slices whose tasks write the same file are one slice, or the later one depends on the earlier, and reading the same file does not link them. The Prove bullet now says the expected result comes from the PRD, the spec or the input file, never from what the command printed. §4's Acceptance bullet gains two sub-bullets. One asks the slice to name how each test fails and to see it red once, worded like `assets/contract-slice.md`. The other asks every skill-text Acceptance to assert a passage that occurs exactly once. Deliver, polish, harvest, prd and ux each say once, at their first date, that it is the UTC date (`date -u +%F`). Four tests were added to `tests/skills_bundle.rs` through `assert_unique_passage`. The lock was restored from `main` and relocked once at 0.13.4.
- **Inner loop:** 1 run, 0 red, ~2 min. It caught nothing.
- **Seen red once** (removing the passage, then restoring it):
  - `prove_takes_its_expected_result_from_the_spec`: red with the §2 spec-not-output sentence removed ("occurs 0 times"), and again with the §4 name-the-failure bullet removed.
  - `slices_that_write_one_file_are_linked`: red with the §2 slices bullet removed.
  - `slice_asks_for_a_passage_that_occurs_once`: red with the §4 exactly-once bullet removed.
  - `every_skill_that_fills_a_date_names_utc`: red with the ux UTC sentence removed.
- **Checkpoint 2** FR-7 `prove_takes_its_expected_result_from_the_spec`, `checkpoint_takes_its_expected_result_from_the_spec`, `a_behavioural_test_is_seen_red_once`: `cp-2/fr-07-prove-from-spec.txt`
- **Checkpoint 2** FR-9 `slices_that_write_one_file_are_linked`: `cp-2/fr-09-slices-write-one-file.txt`
- **Checkpoint 2** FR-11 `a_passage_must_occur_exactly_once`, `slice_asks_for_a_passage_that_occurs_once`: `cp-2/fr-11-passage-once.txt`
- **Deviations:** none. The deliver sentence sits just before the `Owner review ({date})` line, not inside it, because that line is a template for `retro.md`.

**Slice**: After this slice:

- `/slice` says a Prove row's expected result comes from the spec, and that each behavioural test names how it fails without the feature and is seen red once;
- `/slice` links two slices whose tasks write one file;
- `/slice` asks every skill-text Acceptance to assert a passage that occurs exactly once;
- the five skills that ask an agent to fill a date each say once that it is the UTC date (`date -u +%F`).

## Current code / environment

- **FR-7, the `/slice` half**:
  - `skills/slice/SKILL.md:58` ("**Every slice ends in two tasks.** … **Prove** names this slice's rows…") says nothing on where the expected result comes from.
  - `:85` ("**`**Acceptance:**`** — a test that could fail…") does not ask how the test fails without the feature.
  - prd §9: name the failure for each behavioural Prove row and each Acceptance test. The builder sees red only for tests it runs (Acceptance tests and `run` rows). A harness-driven row only names its failure.
  - Slice 03 writes the builder and checkpoint contract halves. Keep the wording consistent with `assets/contract-slice.md` as 03 left it, if 03 has landed. Otherwise follow prd §9.
- **FR-9**: `skills/slice/SKILL.md:54`: "**A file two FRs both write is one slice's, or an order.**" It is FR-scoped. Add slices: two slices whose tasks write the same file are one slice, or the later depends on the earlier. Reading does not link. `:53` (dependencies explicit and minimal) and §5's "Every dependency you declare costs parallelism" stay. Say why the link is real.
- **FR-11 §4**: the sentence goes beside `:85`'s Acceptance bullet. The skill ships to every repo, so it names the rule (the quoted passage occurs exactly once), not factory's helper (prd §13).
- **FR-2, every site**: the sentence goes in each of these, at its first date use:
  - `skills/deliver/SKILL.md:45` (`**Owner review ({date}):**`);
  - `skills/polish/SKILL.md:163` (`**Code-review fixes ({date}).**`, first), with `:168` (`reviewed=<date>`) after it;
  - `skills/harvest/SKILL.md:34` (`--origin "harvest {date}"`);
  - `skills/prd/SKILL.md:42` (`` `FR-n — RETIRED (date)` ``). The prd skill also covers `assets/docs/index.md:43` and `assets/docs/prd.scaffold.md:56` (`Accepted _(date)_`);
  - `skills/ux/SKILL.md`, near `/ux rubric` (`:14`), for `skills/ux/references/rubric.md:69,70` (`added {date}`).
  - The code's clock is UTC: `src/docs/scaffold.rs:28`, `src/docs/log.rs:217`.
- **Not these files** (prd §4):
  - the templates that become project files (`assets/docs/*`, `skills/ux/references/rubric.md`);
  - `assets/contract-polish.md:18`, whose `{date}` the binary fills (`src/pack.rs:542`). A sentence holding `{date}` there would be substituted.
- **The relock trap** (`src/skills.rs:165-182`): 02 committed the lock at 0.13.4. A skill edit here under the same version refuses ("bump Cargo.toml past 0.13.4 first"). Restore the base lock first, with `git show main:skills/bundle.lock > skills/bundle.lock`, then run `FACTORY_BUNDLE_WRITE=1 cargo test -q --test skills_bundle` once. Do not bump again. prd §2 keeps the real fix in PRD-015.
- **Existing pins**: `the_slice_skill_skips_the_write_question_only_for_deliver` (`tests/skills_bundle.rs:241`) pins slice §5. `the_prd_skill_offers_the_build_and_drops_the_clear_advice` (`:227`) pins prd text. Slice 01 converted both to the helper, so keep their passages intact.
- **Trap**: no line may contain the uppercase decision prefix `no_skill_or_asset_asks_for_a_numbered_decision` rejects (`tests/skills_bundle.rs:76`).

## Tasks

### 1. `/slice`: Prove from the spec, red once, shared files, unique passages — high — blocked by: —
**Source**:

- §9 FR-7: "`/slice`'s Prove guidance and the checkpoint contract say the expected result comes from the PRD, the spec or the input file, never from what the command printed"; "Each behavioural Prove row, and each test a task's Acceptance names, says how it fails if the feature is removed."
- §11 FR-9: "Two slices whose tasks write the same file are one slice, or the later one depends on the earlier. Reading the same file does not link them."
- §13 FR-11: "`/slice` §4 asks every skill-text Acceptance to assert that its quoted passage occurs exactly once."

Edit `skills/slice/SKILL.md:54`, `:58` and `:85`.

**Acceptance**, in `tests/skills_bundle.rs` through the helper:

- `prove_takes_its_expected_result_from_the_spec`: §2's Prove bullet holds the spec-not-output passage once, and §4 holds the name-the-failure passage once.
- `slices_that_write_one_file_are_linked`: §2 holds the slices passage once.
- `slice_asks_for_a_passage_that_occurs_once`: §4 holds the exactly-once passage once.

### 2. Every date is the UTC date — high — blocked by: —
**Source**: §4 FR-2: "Every skill that asks an agent to fill a date says once, at its first use, that it is the UTC date (`date -u +%F`). The skills are deliver, polish, harvest, prd and ux."

Edit the five `SKILL.md` files listed under Current code.

**Acceptance:** `every_skill_that_fills_a_date_names_utc`: each of the five files holds the UTC sentence (e.g. "the UTC date, `date -u +%F`") exactly once.

### 3. Relock — high — blocked by: 1, 2
**Source**: §2: "A skill edit needs a version bump past 0.13.3". Slice 02 already made it.

Restore the base lock, then relock as described under Current code.

**Acceptance:** `bundle_lock_matches` passes at 0.13.4.

### 4. Verify — medium — blocked by: 3
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q`

**Acceptance:** inner loop green, one commit.

### 5. Prove — at the checkpoint — blocked by: 4
prd §15 rows:

- **FR-2:** `every_skill_that_fills_a_date_names_utc`.
- **FR-7:** the `/slice` half, `prove_takes_its_expected_result_from_the_spec`.
- **FR-9:** `slices_that_write_one_file_are_linked`.
- **FR-11:** `slice_asks_for_a_passage_that_occurs_once`.

They are proven below the UI: the checkpoint runs them and keeps the transcript.

**Acceptance:** after the checkpoint, the transcripts are in Progress.

**Code-review fixes (2026-11-13).**

- `/slice` §4's "breaks the feature once (a revert or a stub)" now says by hand, never through git, which can lose uncommitted work. This is the same defect as the builder contract's.
- `/slice` §2 held two near-identical bullets, one for FRs and one for slices, writing one file. They are merged into the slice bullet, which keeps both incidents.
- Harvest's UTC sentence covered only `{date}`, not its `harvest/<yyyy-mm-dd>` branch, and prune's `prune/<yyyy-mm-dd>` named no clock. Both now name UTC, and `every_skill_that_fills_a_date_names_utc` lists prune (AD-1).
- `prove_takes_its_expected_result_from_the_spec` also asserted §4's break-it-once rule. That half moved to its own test, `slice_asks_each_test_to_fail_once_without_the_feature`.
- `tests/skills_bundle.rs` had three copies of the section helper and three inline copies of the same bounds logic. They are folded into one `section(rel, heading, next)`.
