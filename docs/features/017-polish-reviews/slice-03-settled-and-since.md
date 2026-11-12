# Slice 03 — Settled decisions and since-last-review

**Source**: [`prd.md`](prd.md) → §7 FR-5 (the Rust half), §8 FR-6 (the Rust half and the driver), §10 FR-8 (the `**Supersedes:**` line it reads), §13. **Depends on**: 02 (this slice changes the range `numstat`, `diff` and slice 02's part fetch use, and the prompt `diff_pass_prompt` builds. Both are functions 02 just changed.)
**Created**: 2026-11-12

**Progress**: built 2026-11-12.

- **Built.** `src/settled.rs`: `block(dir)` (live decisions, the PRD's Decisions log and Non-goals by title through the new `pack::cut_titled`, capped at 32 KiB: Non-goals first, then the oldest decisions, then the end of the Decisions log, with a `Cut to fit 32 KiB: …` line), `for_feature(checkout, NNN)` and `last_reviewed(checkout, NNN)` (`git log -1 --format=%H -G ^reviewed -- <state.toml>`). `review::Range` (`Base` → `b...HEAD`, `Since` → `c..HEAD`) now goes through `numstat`, `diff`, `plan` and `diff_pass_prompt`; `review::with_settled` appends the block. `collect::collect(root, &Options { base, since, feature }, …)`: leaks stays on `base`, the plan line reads `against <c>..HEAD (since the last review)`. CLI: `--feature` on `second` (diff mode; doc mode ignores it) and `collect`, `--since` on `collect` (a leading `-` is a usage error). The driver's `collected` passes `run.state().feature` and `last_reviewed` of the lane.
- **Tests.** Unit: `a_superseded_decision_is_left_out`, `the_block_names_what_the_cap_cut`, `no_log_or_section_is_no_block_part`, `last_reviewed_is_none_outside_a_feature`. `tests/review.rs`: `feature_puts_settled_decisions_in_every_pass_prompt` (collect and second), `no_feature_keeps_the_prompt_as_today`, `collect_since_reviews_only_later_commits`, `collect_since_still_scans_leaks_on_the_whole_branch`, `collect_since_starting_with_a_dash_is_a_usage_error`, `last_reviewed_commit_is_the_one_that_set_reviewed`. `tests/closeout.rs`: `the_polish_collect_tells_its_reviewers_what_the_feature_settled` (a full run; every recorded reviewer prompt holds the fixture decision).
- **Inner loop:** 2 runs, 1 red, about 4 min. The red run caught the collect usage test, which matches `--base <branch> [--out <file>]` as a substring; the new flags now follow `[--out <file>]` in `USAGE`, so the existing test is unchanged.
- **Deviations:** none from the tasks. AD-5 (fyi) records the prompt wording, the order beside `--others`, and the third cut step.
- **Prove:** left for the checkpoint (FR-5's three review tests, FR-6's three).

**Slice**: After this slice, `review collect` and `review second --diff` take `--feature NNN`. Every pass prompt then carries the feature's settled block: its decisions (id, title, `**Chose:**` line, superseded ones left out) and its PRD's Decisions log and Non-goals sections, capped at 32 KiB. `review collect` also takes `--since <commit>` and reviews `<commit>..HEAD` for the second opinion, while leaks still scans the whole branch. The driver passes the run's feature and, when the feature was reviewed before, the commit that last set `reviewed`. The skill text that tells attended polish to pass these flags is slice 05's.

## Current code / environment

- **The prompt**: `diff_prompt(files, base, diff_file)` (`src/review.rs:592-598`) is one fixed instruction. `with_others(prompt, others)` (`:608`) shows how a block is appended to every pass's prompt. The test `others_go_into_every_pass_prompt` (`tests/review.rs:286`) shows how to assert on the prompt the stub received: it is the last argv item of each `review_calls` entry.
- **The argv ceiling**: the prompt is passed as one argument (`src/review.rs:378`), and Linux caps one argument at 128 KiB. prd §7 caps the block at 32 KiB, and a cut names what it dropped.
- **The range is hard-coded twice**: `numstat` (`src/review.rs:84-96`) and `diff` (`:97-107`) both build `format!("{base}...HEAD")`. Slice 02's part fetch goes through `diff`. Give both a range argument (or a small enum: `Base(b)` → `b...HEAD`, `Since(c)` → `c..HEAD`) so `collect` picks one and every caller agrees.
- **Leaks keeps the base**: `leaks_part(root, base)` (`src/collect.rs:72`, `:123-134`). prd §8: "Leaks still scans `<base>...HEAD`". Do not route it through the since range.
- **The plan line**: `changed: n files, m lines against {base}...HEAD` (`src/collect.rs:97-101`). With `--since`, it names `<commit>..HEAD (since the last review)`. The size row (`needs_second_opinion`) applies to the since diff (prd §8).
- **The CLI**: `CollectArgs` (`src/cli/review.rs:42-49`) has `base` and `out`. `SecondArgs` sits beside it. `USAGE` (`:51`) lists every form, and a usage test (`a_bad_combination_is_a_usage_error_with_exit_1_and_no_call`, `tests/review.rs:311`) checks it. `--feature` goes on both verbs, and `--since` on `collect` only (prd §8 names collect). A `--since` value starting with `-` is a usage error, as `--base` is (`src/cli/review.rs:79-80`).
- **The collect entry point**: `collect(root, base, cfg, progress, sink)` (`src/collect.rs:62-68`), called by the CLI (`src/cli/review.rs:85`) and the driver (`src/closeout.rs:381-387`). Slice 04 adds a cancel check to this same call. Gather the new inputs in one options struct now, so 04 adds a field rather than a parameter.
- **Finding the feature**: `retro::feature_dir(repo, feature)` (`src/retro.rs:308`) resolves `docs/features/NNN-*` through `Docs`. Use it on the checkout under review: the lane in a run, the repo root attended.
- **The log**: `docs::log::entries(dir)` (`src/docs/log.rs:154`) returns `Entry { id, kind, title, body, … }` (`:68-90`). Decisions are `Kind::Decision`, and the `**Chose:**` line is in `body`. prd §10: an overturning AD carries `**Supersedes:** AD-n` in its body. The block drops every AD some later entry supersedes.
- **PRD sections by name**: `pack::cut_section(prd, n)` (`src/pack.rs:948-965`) cuts by number. The block needs the sections headed `Decisions log` and `Non-goals` whatever their number (in this PRD, §12 and §15). Add a by-title cut beside it rather than guessing numbers.
- **No feature folder, no log, or a missing section** means no block for that part, never an error (prd §7).
- **The driver**: `polish()` (`src/closeout.rs:135`) calls `collected(run, &lane_dir, &default_branch, &cfg.review, …)` (`:197-204`). The run's feature is `run.state().feature` (`src/run.rs:97`).
- **The driver does not read `reviewed` today.** It skips polish only when shipped, `proceed` answered, or `PolishDone` logged (`src/closeout.rs:78-82`, `:118`). prd §8 (corrected 2026-11-12): it now passes `--since` when a last-reviewed commit exists.
- **The last-reviewed commit**: `state.toml` holds `reviewed = "<date>"` (`src/docs/state.rs:29`, written by `factory docs set`). The commit is the newest one that changed that line: `git log -1 --format=%H -G '^reviewed' -- <feature dir>/state.toml`, run in the checkout. No such commit means the full diff.
- **Collect reuse**: `collect::reusable` (`src/collect.rs:30-34`) keys on the head line only. A reused file from the same head was built with the same inputs, so nothing changes there.
- **Deliberate non-change**: prd §15 "Not handled: docs-only passes in their own section". The settled block is the same for every pass, with no per-pass selection.

## Tasks

### 1. The settled block — high — blocked by: —
**Source**: §7 FR-5: "`--feature NNN`… builds a settled block. It holds each decision in the feature's `log.jsonl` (id, title and its `**Chose:**` line), plus the PRD's Decisions log and Non-goals sections… capped at 32 KiB… A superseded AD is left out."

A new module (`src/settled.rs`, or beside `collect`) takes a feature directory and returns the block text. It has three parts:

- a decisions list, one line per AD: id, title and the `**Chose:**` text, with ADs some later entry `**Supersedes:**` dropped;
- the PRD's `Decisions log` section;
- the PRD's `Non-goals` section.

Cap the whole block at 32 KiB, cutting the Non-goals first, then the oldest decisions. Name the cut in one line.

**Acceptance:** unit tests:

- `a_superseded_decision_is_left_out`: the AD that a later `**Supersedes:** AD-1` names does not appear, and its successor does.
- `the_block_names_what_the_cap_cut`.
- `no_log_or_section_is_no_block_part`.

### 2. `--feature` puts it in every pass prompt — high — blocked by: 1
**Source**: §7 FR-5: "Every pass prompt gets that block, with one instruction: a finding that contradicts an entry must name it and say why the entry is wrong… Without `--feature`, the prompt is today's."

Add `--feature NNN` to `collect` and `second`. Resolve the feature dir in the checkout, and append the block plus the instruction to each pass prompt, the way `with_others` does. Doc mode ignores it.

**Acceptance:**

- `feature_puts_settled_decisions_in_every_pass_prompt`: a fixture feature with two decisions, one superseding the other, and a `prd.md` with both sections. Every recorded prompt holds the live AD's title and Chose line and the Non-goals text, and not the superseded AD.
- `no_feature_keeps_the_prompt_as_today`.

### 3. `--since` reviews only later commits — high — blocked by: —
**Source**: §8 FR-6: "`review collect --since <commit>` reviews `<commit>..HEAD` for the second opinion. The plan's `changed:` line names that range… Leaks still scans `<base>...HEAD`… The size row applies to the since diff."

Thread a range through `numstat`, `diff` and slice 02's part fetch. `collect` uses `Since` when given and `Base` otherwise, keeps leaks on `Base`, and words the plan line as above.

**Acceptance:**

- `collect_since_reviews_only_later_commits`: a fixture branch with commit `R` then two more files. Only those two files reach the stub.
- `collect_since_still_scans_leaks_on_the_whole_branch`: a leaks hit planted before `R` still shows in `## Leaks`.

### 4. The driver passes the feature and the last-reviewed commit — high — blocked by: 2, 3
**Source**: §8 FR-6: "The last-reviewed commit is the newest one that changed the `reviewed` line of the feature's `state.toml`. One Rust function finds it. The driver passes it as `--since` when it exists"; §7 FR-5: "The driver passes the run's feature".

One function, given a checkout and a feature, returns the last-reviewed commit or none. In `polish()`, pass `run.state().feature` and that commit into the collect options.

**Acceptance:**

- `last_reviewed_commit_is_the_one_that_set_reviewed`: a fixture whose `state.toml` gains `reviewed` in commit `R`, then an unrelated commit, returns `R`. One with no `reviewed` returns none.
- A run test asserts the driver's collect carried the feature: the stub's recorded prompt holds a fixture decision.

### 5. Verify — medium — blocked by: 4
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q`

**Acceptance:** inner loop green, one commit.

### 6. Prove — at the checkpoint — blocked by: 5
prd §13 rows **FR-5**: the Rust tests `feature_puts_settled_decisions_in_every_pass_prompt`, `a_superseded_decision_is_left_out` and `no_feature_keeps_the_prompt_as_today`. `polish_lists_five_settled_sources` is slice 05's. Rows **FR-6**: `collect_since_reviews_only_later_commits`, `collect_since_still_scans_leaks_on_the_whole_branch` and `last_reviewed_commit_is_the_one_that_set_reviewed`. They are proven below the UI: the checkpoint runs them and keeps the transcript.

**Acceptance:** after the checkpoint, the transcript is in Progress.
