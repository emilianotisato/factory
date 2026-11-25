# Slice 02 — Portable cockpit skills and bundle relock

**Source**: [`prd.md`](prd.md) → §2, §7 (FR-5), §9, §11. **Depends on**: — (independent of slice 01; touches only `src/skills.rs`, `.agents/skills/`, `skills/`, `Cargo.toml`, `Cargo.lock`, `tests/skills_bundle.rs`, and `tests/cockpit.rs`)
**Created**: 2026-11-25

**Progress**:
- **Built:** `LINK_DIRS` (10 `NAMES` + `_shared`) drives `skills::render`'s `.agents/skills/` loop, so `.agents/skills/_shared -> ../../.claude/skills/_shared` is linked and recorded in `[links]`; this checkout gains `.agents/skills/<name>` and `_shared` links to `../../skills/<name>`. Bundled skills are harness-neutral: `models.md` and `polish` route the second opinion through `[review]` / `factory setup`, and `models.md`, `polish` and `prd` tell `disabled:` (no fallback) apart from `unreviewed:` (fresh local subagent). `deliver` drops `AskUserQuestion`/`PushNotification`/Skill tool/`/clear` and adds the adversarial-reviewer `ASK-n` sentence (50 lines). Version `0.16.0`; `skills/bundle.lock` relocked.
- **Inner loop:** 2 runs, 1 red, ~2.5 min. The red caught `tests/cockpit_mod.rs` requiring the cockpit mod's three version strings to match `Cargo.toml` (AD-2).
- **Seen red once:** `skills_render_links_shared_into_agents_skills` (render looping `NAMES`: `.agents/skills/_shared` NotFound) · `this_repos_claude_skills_link_into_the_source` (`.agents/skills/_shared` link removed: NotFound) · `every_file_under_skills_is_in_the_bundle_byte_for_byte` (stray `skills/stray.md`: "on disk, not in the bundle") · `bundle_lock_matches` (lock back at 0.15.0, and a reverted `models.md` line: "bundle.lock is stale") · `skills_are_harness_neutral_and_distinguish_disabled_review` (reverted `models.md` fallback line: passage occurs 0 times) · `models_md_holds_no_invocation_and_the_skills_call_review_second` (`cursor-agent -p` in models.md) · `the_cockpit_skill_is_under_sixty_lines_and_names_the_five_commands` ("Skill tool" back in deliver) · `deliver_skill_handles_local_ci_mode` (CI-mode sentence altered: 0 times). Each green after restore.
- **Deviations:** `tests/init.rs` repinned to the new `_shared` link and 11 links (AD-1); cockpit mod versions bumped (AD-2); `/prd`'s Skill tool needle repinned (AD-3); `gate/SKILL.md` and `ux/references/rubric.md` left with `AskUserQuestion`, outside the file list (AD-4).
- **Prove (CP-1) FR-5:** all six tests pass — `cp-1/FR-5/skills_render_links_shared_into_agents_skills.txt`, `cp-1/FR-5/skills_bundle.txt`, `cp-1/FR-5/cockpit.txt`; checkout links, 50-line `deliver`, no "unconditional", `disabled`/`unreviewed` split — `cp-1/FR-5/checkout-surface.txt`.

**Slice**: Fixes `.agents/skills/_shared` symlinking in `skills::render` and in `factory`'s own source checkout, replaces Claude-only and Cursor-only phrasing across the bundled skills (including distinguishing `disabled` vs `unreviewed` second opinion in `/polish` and `/prd` and keeping `deliver/SKILL.md` under 60 lines), bumps `Cargo.toml` to `0.16.0`, relocks `skills/bundle.lock`, and updates the pinned tests in `tests/skills_bundle.rs` and `tests/cockpit.rs`. Leaves Rust provider/setup/review code to slices 01, 03, and 04.

## Current code / environment

- **Skills renderer**: `src/skills.rs:14-23` defines `SKILLS_DIR = ".claude/skills"`, `LINKS_DIR = ".agents/skills"`, and `NAMES` (10 skills, excluding `_shared`). `render()` (`:379-393`) symlinks only `NAMES` into `.agents/skills/`, so relative links like `../_shared/models.md` break when a harness resolves relative to `.agents/skills/<name>/` without canonicalizing first. `verdict_for()` (`:493-501`) and `remove_dropped()` (`:600-611`) already read `m.links` dynamically!
- **Source checkout links**: `.claude/skills/` links all 10 `NAMES` + `_shared` to `../../skills/<name>` (`tests/skills_bundle.rs:58-71`), but `.agents/skills/` does not exist in this repo yet.
- **Claude/Cursor-coupled skills**:
  - `skills/_shared/models.md:28-32` says `"Cursor CLI is the cross-vendor path, unconditionally"` and falls back to `"a fresh Claude agent"`. Note `tests/skills_bundle.rs:107-120` rejects `` ```bash ``, `grok-`, `gpt-`, `gemini-`, `~/.local/bin`, `--trust`, and `cursor-agent -p` in `models.md`.
  - `skills/polish/SKILL.md:63-72` says `"Cursor CLI, unconditionally. Do not probe for any other vendor's CLI"`, `"checks --list-models"`, and treats exit `2` unconditionally as owing a fallback. Note `tests/skills_bundle.rs:140-161` pins `"factory review collect"`, `"factory leaks --base"`, `"**`factory review second --diff <base-branch>`**"`, and §5 fixer wording.
  - `skills/prd/SKILL.md:52, :99, :119, :165-166` names `AskUserQuestion`, `Skill tool`, and treats exit `2` unconditionally as unreviewed.
  - `skills/deliver/SKILL.md:1-50` (currently 50 lines) names `AskUserQuestion`, `PushNotification`, `Skill tool`, `/clear`. `tests/cockpit.rs:767-820` pins `< 60` lines and 18+ verbatim passages (PRD-026 §2 & §7 explicitly authorize updating the Claude-only tool-name passages in `tests/cockpit.rs:778-779` to the harness-neutral phrasing).
  - `skills/harvest/SKILL.md`, `skills/lanes/SKILL.md`, `skills/harness/SKILL.md`, `skills/_shared/stale-tooling.md`, `skills/README.md` also contain `AskUserQuestion` / `Claude Code skills` wording.
- **Bundle lock & version**: `Cargo.toml:3` is `0.15.0` and `.git/refs/tags/v0.15.0` exists locally (`e61273d`). `src/skills.rs:175-188` (`tag_state`) returns `Tagged::Yes` at `0.15.0`, so `Cargo.toml` must bump to `0.16.0` before `FACTORY_BUNDLE_WRITE=1 cargo test` relocks `skills/bundle.lock`.
- **Deliberate non-changes (PRD-026 §11)**:
  - No user-global `~/.agents/skills/` installer outside repos.
  - No replacement TUI for `mods/cockpit`.

## Tasks

### 1. Symlink `_shared` into `.agents/skills/` in `src/skills.rs` and add source-checkout `.agents/skills/` links — high — blocked by: —
**Source**: §7 FR-5: "Add `pub const LINK_DIRS: [&str; 11]` (all 10 `NAMES` plus `"_shared"`) and symlink `.agents/skills/_shared -> ../../.claude/skills/_shared` in `skills::render`... Add `.agents/skills/<name>` and `.agents/skills/_shared` symlinks in `factory`'s own repo pointing to `../../skills/<name>`".

`src/skills.rs`, `.agents/skills/*`, `tests/skills_bundle.rs`.
- In `src/skills.rs`, add `pub const LINK_DIRS: [&str; 11]` containing all 10 `NAMES` plus `"_shared"`, and iterate `LINK_DIRS` in `render()` (`:380-393`) when `!shared` so `.agents/skills/_shared -> ../../.claude/skills/_shared` is created and recorded in `manifest.links`.
- Verify any existing unit tests in `src/skills.rs` that count links (e.g. `10 links` → `11 links`) are updated cleanly.
- Create `.agents/skills/{prd,slice,deliver,polish,gate,harness,harvest,lanes,prune,ux,_shared}` symlinks in this repo pointing to `../../skills/<name>`.
- Update `this_repos_claude_skills_link_into_the_source` (`tests/skills_bundle.rs:58-71`) to verify both `.claude/skills/<name>` and `.agents/skills/<name>` resolve to `../../skills/<name>`.

**The trap**: Do NOT touch `NAMES` (`src/skills.rs:21`) — `NAMES` explicitly lists the 10 runnable skills that each hold a `SKILL.md` (`src/skills.rs:20`: "`_shared` is not one: it has no `SKILL.md`"). Keep `NAMES` at 10 and add `LINK_DIRS` for the symlink loop!

**Acceptance:** Unit test `skills_render_links_shared_into_agents_skills` renders into a temp repo, asserts `.agents/skills/_shared` links to `../../.claude/skills/_shared`, asserts reading `.agents/skills/deliver/../_shared/models.md` succeeds without canonicalizing `.agents/skills/deliver` first, and asserts `verdict` reports `missing: .agents/skills/_shared` if that link is removed (fails if `_shared` is omitted from `render()`). `this_repos_claude_skills_link_into_the_source` fails if any `.agents/skills/<name>` link in this checkout is missing or wrong.

### 2. Make bundled skills harness-neutral, bump version to `0.16.0`, relock `skills/bundle.lock`, and update pinned tests — high — blocked by: 1
**Source**: §7 FR-5: "Harness-neutral skill wording: `skills/_shared/models.md`, `skills/polish/SKILL.md`, `skills/prd/SKILL.md`, `skills/deliver/SKILL.md`, `skills/harvest/SKILL.md`, `skills/lanes/SKILL.md`, `skills/harness/SKILL.md`, `skills/_shared/stale-tooling.md`, `skills/README.md`... bump `Cargo.toml` to `0.16.0` and relock `skills/bundle.lock`".

`skills/_shared/models.md`, `skills/_shared/stale-tooling.md`, `skills/deliver/SKILL.md`, `skills/polish/SKILL.md`, `skills/prd/SKILL.md`, `skills/harvest/SKILL.md`, `skills/lanes/SKILL.md`, `skills/harness/SKILL.md`, `skills/README.md`, `Cargo.toml`, `Cargo.lock`, `skills/bundle.lock`, `tests/skills_bundle.rs`, `tests/cockpit.rs`.
- `skills/_shared/models.md`:
  - Replace `:28` ("Cursor CLI is the cross-vendor path, unconditionally...") with `factory review second` using `[review]` in `~/.config/factory/config.toml` (configured via `factory setup`).
  - Replace `:32` ("Cursor unavailable, or the run fails → a fresh Claude agent...") with "Second opinion unavailable or unreviewed → a fresh subagent on the `judgment` model with **no inherited context**", noting that an explicitly disabled second opinion (`disabled:`) skips the fallback.
  - Keep free of every token forbidden by `tests/skills_bundle.rs:110-119`.
- `skills/polish/SKILL.md`:
  - Replace `:63` ("Cursor CLI, unconditionally...") with the configured cross-vendor provider in `[review]` (`factory setup review`), noting cross-vendor is the point.
  - Update `:65` and `:70-72` so `skipped (disabled in config)` / `disabled:` means the owner opted out (no fallback owed), while `fallback owed` / `unreviewed:` triggers the fresh local subagent fallback; remove Cursor-specific `"checks --list-models"` in `:70`. Keep all passages pinned by `tests/skills_bundle.rs:140-161` intact.
- `skills/prd/SKILL.md`:
  - Update `:99` so exit `2` with `disabled:` skips the fallback while `unreviewed:` runs the fresh local subagent fallback.
  - Generalize `AskUserQuestion` and `Skill tool` wording (`:52, :119, :165-166`) to harness-neutral structured choice / skill invocation wording.
- `skills/deliver/SKILL.md` (MUST stay strictly `< 60` lines!):
  - Generalize Claude-only tool names: structured choice prompt instead of bare `AskUserQuestion`, notify the owner if your harness supports push notifications instead of bare `PushNotification`, invoke `/slice NNN from /deliver` without requiring `Skill tool`, start a fresh session instead of `/clear`.
  - Add one concise sentence in §2 covering the adversarial-reviewer `ASK-n` (`<provider>`, `none`, or `proceed`), alongside the CI-mode `ASK-n` sentence, keeping all passages pinned by `tests/cockpit.rs:809-820` (`deliver_skill_handles_local_ci_mode`) verbatim!
  - Update `tests/cockpit.rs:767-806` to pin the updated harness-neutral passages (replacing the Claude-only `"then `AskUserQuestion` with the question's text"` and `"PushNotification"` needles while keeping every command/behavior needle and `< 60` lines check).
- `skills/harvest/SKILL.md`, `skills/lanes/SKILL.md`, `skills/harness/SKILL.md`, `skills/_shared/stale-tooling.md`, `skills/README.md`: replace `"Claude Code skills"` → `"Agent skills"` and bare `AskUserQuestion` / `Skill tool` references with harness-neutral wording (preserving `skills/README.md`'s `` `factory init` `` passage pinned at `tests/cockpit.rs:805`).
- Bump `Cargo.toml` version to `"0.16.0"`, refresh `Cargo.lock`, and run `FACTORY_BUNDLE_WRITE=1 cargo test --test skills_bundle` to update `skills/bundle.lock`.
- Add `skills_are_harness_neutral_and_distinguish_disabled_review` in `tests/skills_bundle.rs` asserting via `assert_unique_passage` that `models.md`, `polish/SKILL.md`, and `prd/SKILL.md` name `factory setup` / `[review]` and distinguish `disabled` from `unreviewed`, and asserting neither `models.md` nor `polish/SKILL.md` contains `"Cursor CLI, unconditionally"` or `"Cursor CLI is the cross-vendor path"`.

**The trap**: Check `tests/skills_bundle.rs:188-250` and `tests/cockpit.rs:767-820` carefully — `deliver/SKILL.md` must stay `< 60` lines, and `tests/skills_bundle.rs:140-161` pins exact passages in `polish/SKILL.md` §1 and §5. Do not touch any unrelated `assert_unique_passage` needles.

**Acceptance:** `every_file_under_skills_is_in_the_bundle_byte_for_byte`, `bundle_lock_matches`, `models_md_holds_no_invocation_and_the_skills_call_review_second`, `skills_are_harness_neutral_and_distinguish_disabled_review`, `the_cockpit_skill_is_under_sixty_lines_and_names_the_five_commands`, and `deliver_skill_handles_local_ci_mode` all pass (each fails if its guarded wording or bundle lock is reverted).

### 3. Verify — medium — blocked by: 1, 2
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q`

**Acceptance:** inner loop green, one commit.

### 4. Prove — at the checkpoint — blocked by: 3
Drive this row from [`prd.md`](prd.md) §9 QA plan at CP-1:
- **FR-5**: run `cargo test skills_render_links_shared_into_agents_skills`, `cargo test this_repos_claude_skills_link_into_the_source`, `cargo test every_file_under_skills_is_in_the_bundle_byte_for_byte`, `cargo test bundle_lock_matches`, `cargo test skills_are_harness_neutral_and_distinguish_disabled_review`, and `cargo test the_cockpit_skill_is_under_sixty_lines_and_names_the_five_commands`.

**Acceptance:** FR-5 row has passing test transcripts captured in checkpoint evidence and recorded in **Progress**.
