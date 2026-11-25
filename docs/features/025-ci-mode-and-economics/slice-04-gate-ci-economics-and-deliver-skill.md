# Slice 04 — Gate CI economics guide and /deliver updates

**Source**: [`prd.md`](prd.md) → §6 (FR-4), plus the `skills/deliver/SKILL.md` updates from §4 (FR-2) and §5 (FR-3). **Depends on**: — _(independent: edits only `skills/`, `src/skills.rs`, `Cargo.toml`, `Cargo.lock`, `tests/skills_bundle.rs`, and `tests/cockpit.rs`, none of which slices 01–03 touch)_
**Created**: 2026-11-25

**Progress**:
- Checkpoint 1, FR-4 (`every_file_under_skills_is_in_the_bundle_byte_for_byte`, `bundle_lock_matches`, `gate_skill_carries_ci_economics_guidance`, `the_cockpit_skill_is_under_sixty_lines_and_names_the_five_commands` green): cp-1/FR-4/tests.txt
- Checkpoint 1, FR-3 (`deliver_skill_handles_local_ci_mode` green): cp-1/FR-3/tests.txt

**Slice**: Adds `skills/gate/references/ci.md` with the draft-silent PR CI design rule, split-tier CI guidance, and the 8-item CI cost audit checklist; updates `skills/gate/SKILL.md` (Round 1, §5 audit, §6 declare) and `skills/deliver/SKILL.md` (`--ci`, CI-mode `ASK-n`, §3 `[skip ci]` when `ci == "local"`) while keeping `deliver/SKILL.md` strictly under 60 lines; registers the new reference in `src/skills.rs`, bumps `Cargo.toml` to `0.15.0`, relocks `skills/bundle.lock`, and pins the new skill guidance with unique-passage tests.

## Current code / environment

- **`skills/gate/SKILL.md:70`**: Round 1 currently recommends *"both, same command, so they cannot drift"*. §5 audit (`:144`) checks only whether CI divergence is documented. §6 (`:161`) writes CI guidance.
- **`skills/deliver/SKILL.md`** (50 lines):
  - Line 13: `1. \`factory run NNN\` (add \`--budget <weighted tokens>\` or \`--hypothesis "<one line>"\` only when the owner gave one)…`
  - Line 28: `BLOCK-n …` or `ASK-n …` relay instructions.
  - Lines 45–46: §3 step 3 (`Push once.`) and step 4 (`Tell the owner the PR URL, that it is a draft, and that merging is theirs.`).
- **`tests/cockpit.rs:767-805`**: `the_cockpit_skill_is_under_sixty_lines_and_names_the_five_commands` asserts `lines < 60` and checks **18** exact unique passages (`:773-795`, including `"1. \`factory run NNN\` (add \`--budget"`). **Do not break any of those 18 passages and keep `deliver/SKILL.md` strictly under 60 lines (aim for ≤ 55).**
- **`src/skills.rs:25-93`**: `FILES` lists every bundled skill file by hand (`tests/skills_bundle.rs:34` fails on any file under `skills/` missing from `FILES`).
- **`Cargo.toml:3` & `skills/bundle.lock`**: `version = "0.14.0"` is **already tagged locally** (`.git/refs/tags/v0.14.0` → `24fd465c`), so `relock` (`src/skills.rs:198-227`) refuses to rewrite `skills/bundle.lock` until `Cargo.toml` is bumped past `0.14.0` (bump to `0.15.0`, run `FACTORY_BUNDLE_WRITE=1 cargo test`, and keep the updated `Cargo.lock`).
- **Deliberate non-changes**:
  - *prd §6 & §10*: skill-only guidance; do **not** add a binary `.github/workflows/*.yml` parser in `src/docs/preflight.rs`.
  - *prd §6*: never cite numbered `DEC-n` anchors (`tests/skills_bundle.rs:83`) and never put owner-specific paths or names (`/home/`, `Dropbox`, `emiliano`) in skills (`tests/cockpit.rs:801`).

## Tasks

### 1. Add `skills/gate/references/ci.md` and update `skills/gate/SKILL.md` — high — blocked by: —
**Source**: §6 FR-4: cost-aware CI design rule, draft-silent PR CI, split-tier CI, 8-item audit checklist, linked from `skills/gate/SKILL.md` Round 1, §5 audit, and §6 declare.

`skills/gate/references/ci.md`, `skills/gate/SKILL.md`, `src/skills.rs`.
- Create `skills/gate/references/ci.md` covering:
  1. The design rule: *what cloud CI proves that the local gate did not* (clean-environment reproducibility + merge check).
  2. **Draft-silent PR CI**: `on: pull_request: types: [opened, reopened, synchronize, ready_for_review]` with `if: github.event.pull_request.draft == false` so `factory`'s draft PRs burn 0 cloud minutes until marked ready for review.
  3. **Split-tier CI**: heavy local tiers (mutation testing, multi-shard browser suites, complexity/CRAP) stay local (or nightly); PR CI runs the fast clean-room core.
  4. **CI cost audit checklist** (all 8 items): (1) full suite running on draft PRs, (2) duplicate `push: [main]` runs after PR-gated merge (and deploy workflows chaining `workflow_run` on a redundant full verify), (3) missing `concurrency` with `cancel-in-progress: true`, (4) missing `paths-ignore` for `docs/**` and `**.md`, (5) missing dependency/build caches, (6) hardcoded local parallelism on 2-vCPU cloud runners (`$(nproc)` instead of fixed `--processes=10`), (7) unused coverage extensions (`coverage: xdebug` without `--coverage`), (8) heavy local-only tiers or daily cron baselines burning metered private minutes.
- Update `skills/gate/SKILL.md`:
  - §3 Round 1 (`:70`): link `references/ci.md`; recommend draft-silent clean-room CI + full local gate for slow (>2 min) or metered private repos, keeping same-command-in-both for fast gates.
  - §5 Audit mode (`:144`): expand the CI bullet to audit `.github/workflows/*.yml` against `references/ci.md` and report each leak with its estimated cost.
  - §6 Declare and document (`:161`): link `references/ci.md`.
- Register `("gate/references/ci.md", include_str!("../skills/gate/references/ci.md"))` in `src/skills.rs:FILES` (`:38-46`, kept alphabetical).

**The trap**: Keep `tests/skills_bundle.rs:165` (`section("gate/SKILL.md", "**Round 3", "## 4.")`) intact — do not rename `**Round 3` or `## 4.` headings in `skills/gate/SKILL.md`.

**Acceptance:** `gate_skill_carries_ci_economics_guidance` in `tests/skills_bundle.rs` uses `assert_unique_passage` to verify `gate/references/ci.md` names draft-silent (`draft == false`), duplicate `push: [main]`, `cancel-in-progress: true`, `paths-ignore`, dependency caches, runner parallelism, `xdebug` without `--coverage`, and heavy local tiers/cron, and verifies `gate/SKILL.md` links `references/ci.md`. **Fails if removed**: deleting any checked item or the link fails `assert_unique_passage`.

### 2. Update `skills/deliver/SKILL.md` for `--ci`, CI-mode `ASK-n`, and §3 `[skip ci]` — high — blocked by: —
**Source**: §4 FR-2 & §5 FR-3: `--ci local|remote` on `factory run`, CI-mode `ASK-n` choices `local` / `remote` / `proceed`, and §3 checking `status --json` `ci == "local"` to stamp `[skip ci]` on the review commit and remind the owner to trigger merge CI.

`skills/deliver/SKILL.md`, `tests/cockpit.rs`.
- On line 13, add `--ci local|remote` beside `--budget` and `--hypothesis` (keeping `"1. \`factory run NNN\` (add \`--budget"` verbatim).
- In §2 line 28 (or one concise sub-line), note that when `ASK-n` is the CI-mode ask, present `local`, `remote`, and `proceed` so `factory answer` receives the exact token.
- In §3 steps 3–4 (lines 45–46), note: check `factory status <id> --json`'s `ci` field; when `"local"`, append `[skip ci]` to the owner-review commit subject before pushing and tell the owner remote CI was skipped so they know to mark the PR ready for review (or trigger CI once) before merging.
- Add `deliver_skill_handles_local_ci_mode` in `tests/cockpit.rs` using `assert_unique_passage` for the `--ci`, CI-mode ask (`local`/`remote`/`proceed`), and §3 `[skip ci]` passages.

**The trap**: `tests/cockpit.rs:771` enforces `lines < 60` on `skills/deliver/SKILL.md` (currently 50 lines) and `:773-795` pins 18 exact passages. Keep edits tight so total lines stay ≤ 55 and all 18 passages remain unique and intact.

**Acceptance:** `deliver_skill_handles_local_ci_mode` and `the_cockpit_skill_is_under_sixty_lines_and_names_the_five_commands` in `tests/cockpit.rs` both pass. **Fails if removed**: deleting the `--ci` or §3 `[skip ci]` wording fails `deliver_skill_handles_local_ci_mode`.

### 3. Bump `Cargo.toml` to `0.15.0` and relock `skills/bundle.lock` — high — blocked by: 1, 2
**Source**: §2 Blast radius & §6 FR-4: `v0.14.0` is tagged locally, so `Cargo.toml` bumps past `0.14.0` before `FACTORY_BUNDLE_WRITE=1 cargo test` relocks `skills/bundle.lock`.

`Cargo.toml`, `Cargo.lock`, `skills/bundle.lock`.
- Bump `version = "0.14.0"` → `version = "0.15.0"` in `Cargo.toml`.
- Run `FACTORY_BUNDLE_WRITE=1 cargo test --test skills_bundle` so `skills/bundle.lock` updates cleanly (and `Cargo.lock` picks up `0.15.0`), then re-run `cargo test --test skills_bundle` *without* `FACTORY_BUNDLE_WRITE` to prove the lock matches.

**The trap**: Never edit `skills/bundle.lock` by hand and never touch `bundle_lock_matches` or `check_lock` in `tests/skills_bundle.rs`.

**Acceptance:** `every_file_under_skills_is_in_the_bundle_byte_for_byte` and `bundle_lock_matches` in `tests/skills_bundle.rs` pass with `FACTORY_BUNDLE_WRITE` unset. **Fails if removed**: reverting `Cargo.toml` to `0.14.0` or reverting `skills/bundle.lock` fails `bundle_lock_matches`.

### 4. Verify — medium — blocked by: 3
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q`

**Acceptance:** inner loop green, one commit.

### 5. Prove — at the checkpoint — blocked by: 4
`prd.md` §8, the **FR-4** row and the `/deliver` part of **FR-3**: `every_file_under_skills_is_in_the_bundle_byte_for_byte`, `bundle_lock_matches`, `gate_skill_carries_ci_economics_guidance`, `the_cockpit_skill_is_under_sixty_lines_and_names_the_five_commands`, and `deliver_skill_handles_local_ci_mode`. All are `run` rows, proven below the UI: the checkpoint runs them and keeps the transcript.

**Acceptance:** after the checkpoint, the transcript shows all five tests green.
