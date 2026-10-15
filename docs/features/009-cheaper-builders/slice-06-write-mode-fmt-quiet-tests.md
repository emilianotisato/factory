# Slice 06 — Write-mode fmt and quiet tests

**Source**: [`prd.md`](prd.md) → §3 FR-9, and FR-10's inner loop and `/gate` halves. **Depends on**: — (independent of every other slice: only this one touches `skills/`, `Cargo.toml` and `docs/project.toml`. FR-10's contract line is slice 02's, which owns `assets/contract-slice.md`)
**Created**: 2026-10-15

**Progress**:
- **Built.** `/gate` round 3: the inner loop formats in write mode and the gate keeps check mode; a one-minute gate's inner loop is the gate's own tiers, formatter in write mode, tests quiet. A new round-3 bullet names the quiet flags: Rust `cargo test -q`, Python `pytest -q`, Kotlin `./gradlew test -q`, Pest `--compact`, Vitest `--reporter=dot`, Go quiet by default. The audit's inner-loop bullet checks the same rule. Every `| format |` row of `references/stacks.md` gives write / check. Bundle test `gate_inner_loop_formats_in_write_mode_and_runs_tests_quiet` pins both. `Cargo.toml` 0.7.0, `skills/bundle.lock` relocked at 0.7.0, `Cargo.lock` moved with it. `docs/project.toml` `inner` set through `factory docs gate set --inner`; `gate` untouched.
- **Inner loop:** 2 runs, 1 red, ~1.7 min. The red was `cargo fmt --check` on the new test's iterator chain.
- **Deviations:** none. The relock ran as `FACTORY_BUNDLE_WRITE=1 cargo test -q --test skills_bundle`, the one test target that writes the lock.
- **Left for the checkpoint:** the FR-9 and FR-10 (inner-loop half) transcripts.

**Slice**: The inner loop formats in write mode and runs tests quiet; the gate keeps check mode. `/gate` round 3 and its audit say so, round 3 names each stack's quiet test flag, and the stack table gives each stack's write form beside its check form. This repo's `inner` becomes `cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q`, and its `gate` stays byte-identical. The skill edit changes the bundle's hash, so this slice bumps to 0.7.0 and relocks.

## Current code / environment

- **`/gate` names no format mode**: round 3, `skills/gate/SKILL.md:80`: "**When the full gate runs in about a minute, the inner loop is the gate**: declare the same string for both. Otherwise … **format and lint are always in it**". The audit repeats the same-string rule (`:148`). `:132` (a tier red at checkpoints and never in the inner loop belongs in it) stays as it is.
- **The stack table** (`skills/gate/references/stacks.md`): one `| format |` row per stack. Only PHP gives both forms, `pint --parallel` / `pint --parallel --test` (`:16`). The others give the check form alone: Node `prettier --check .` (`:40`), Go `gofmt -l .` or `gofumpt -l .` (`:63`), Kotlin `./gradlew spotlessCheck` (`:82`), Python `ruff format --check .` (`:103`), Rust `cargo fmt --check` (`:123`).
- **The bundle**: `skills/gate/SKILL.md` and `skills/gate/references/stacks.md` are in it (`src/skills.rs:34-38`); `assets/` is not. `bundle_lock_matches` (`tests/skills_bundle.rs:179-184`) fails on any skill edit until `Cargo.toml:3` (0.6.0) moves and `FACTORY_BUNDLE_WRITE=1 cargo test` rewrites `skills/bundle.lock` (0.6.0 today).
- **`relock` refuses a second hash at one version**: `src/skills.rs:160-176` accepts a new hash only at a version above the lock's. *§3 FR-9: "Every `skills/` edit of this PRD (FR-9 and FR-10) lands in that one slice."*
- **Skill text is pinned by bundle tests**: `polish_fixers_run_one_after_another_with_no_lane_rule` (`tests/skills_bundle.rs:142`) cuts one section of a skill and asserts what it holds. That is the pattern for this slice's test.
- **This repo's config**: `docs/project.toml:3` `gate` and `:4` `inner` are the same string, `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`. `factory docs gate set --inner "<cmd>"` writes `inner`. `docs gate` is not one of the verbs `FACTORY_BOARD` sends to the main tree (`board_root`, `src/cli/docs.rs:253-265`: `set`, `retro`, `status`, `log add`), so in a lane it writes the lane's `docs/project.toml` and this slice's commit carries it. No protected file covers it (`protected_files`, `src/commit.rs:11-13`).
- **Tests never read this repo's live `docs/`** (CLAUDE.md). The new `inner` string is proven by a command transcript at the checkpoint, never by a test.
- **Deliberate non-change**: *prd §7: "Releasing the `/gate` edit. The bump and the relock ride the branch (§3 FR-9). The tag and `factory skills update` are the owner's, after the merge."* Do not tag.
- **Deliberate non-change**: *§3 FR-9 Accepted cost: "A stale skill line polish finds after the relock cannot be fixed on this branch; it goes to an idea."*

## Tasks

### 1. `/gate` round 3 and the audit name the modes — high — blocked by: —
**Source**: §3 FR-9: "The inner loop runs the formatter in write mode, and the gate keeps check mode. The same-string rule becomes: when the full gate runs in about a minute, the inner loop is the gate's own tiers, with the formatter in write mode and the tests quiet (FR-10). Round 3 and the audit (`:80`, `:148`) say so." §3 FR-10: "`/gate` round 3 names each stack's quiet test flag for the inner loop, where one exists."

`skills/gate/SKILL.md:80` and `:148`. Round 3: the inner loop formats in write mode and the gate in check mode; when the gate runs in about a minute, the inner loop is the gate's own tiers with the formatter in write mode and the tests quiet; it names each stack's quiet test flag where one exists, such as Rust's `cargo test -q`. The audit's inner-loop bullet checks the same rule.

**Acceptance:** a bundle test cuts round 3 out of `skills/gate/SKILL.md` and asserts it names write mode for the inner loop, check mode for the gate and `cargo test -q`, and no longer says to declare the same string for both.

### 2. The stack table gives both format forms — high — blocked by: —
**Source**: §3 FR-9: "the stack table gives each stack's write form beside its check form."

`skills/gate/references/stacks.md`: each `| format |` row gives the write form, then the check form, as the PHP row does (`:16`). For example, Rust becomes `cargo fmt` / `cargo fmt --check`.

**Acceptance:** the bundle test of task 1 also asserts every `| format |` row of the stack table holds a write form and a check form, split by ` / `.

### 3. 0.7.0 and the relock — high — blocked by: 1, 2
**Source**: §3 FR-9: "The slice that edits the skill bumps `Cargo.toml` from 0.6.0 to 0.7.0 and relocks (`FACTORY_BUNDLE_WRITE=1 cargo test`)."

`Cargo.toml:3` to 0.7.0, then `FACTORY_BUNDLE_WRITE=1 cargo test`. Commit `Cargo.lock`, which the next build rewrites with the new version, and `skills/bundle.lock`.

**The trap:** `relock` refuses a second hash at 0.7.0 (`src/skills.rs:160-176`). Finish tasks 1 and 2 first. A skill edit after the relock needs the 0.6.0 lock restored from git and one more relock.

**Acceptance:** `bundle_lock_matches` passes at 0.7.0, and `skills/bundle.lock` says `version = "0.7.0"`.

### 4. This repo's inner loop — high — blocked by: —
**Source**: §3 FR-9: "This repo's `inner` becomes `cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q`, through `factory docs gate set --inner`. Its `gate` stays byte-identical." §3 FR-10: "This repo's inner loop runs `cargo test -q` (the FR-9 string)."

`factory docs gate set --inner "cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q"`. `docs/project.toml:3` stays as it is.

**Acceptance:** `factory docs gate --inner` prints the new string and `factory docs gate` prints the old one, read as a transcript at the checkpoint (task 6). `git diff main -- docs/project.toml` touches the `inner` line only.

### 5. Verify — medium — blocked by: 1, 2, 3, 4
`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 6. Prove — at the checkpoint — blocked by: 5
`prd.md` § QA plan rows, driven below the UI:
- **FR-9**: run `factory docs gate --inner` and `factory docs gate` in this repo, and the bundle test of task 1; keep the transcript.
- **FR-10**, its inner-loop half: the same `factory docs gate --inner` ends in `cargo test -q`. The Contract half is slice 02's.

**Acceptance:** after the checkpoint, each row has its transcript in this slice's **Progress**.
