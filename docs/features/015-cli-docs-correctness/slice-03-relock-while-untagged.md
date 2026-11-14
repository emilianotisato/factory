# Slice 03 — Relock while untagged

**Source**: [`prd.md`](prd.md) → §5 FR-3, §2, §12. **Depends on**: —. It is independent: it edits only `src/skills.rs` and `tests/skills_bundle.rs`, and no skill text, so no version bump.
**Created**: 2026-11-12

**Progress**:

- **Built**: `skills::Tagged` (`Yes`/`No`/`Unknown`) and `skills::tag_state(repo, version)`: a local `rev-parse -q --verify` tag answers alone, else `ls-remote --exit-code` maps 0 → tagged, 2 → untagged, anything else (128, git missing) → unknown; `GIT_TERMINAL_PROMPT=0` so it never waits on a prompt. `relock` takes the `Tagged` value and stays pure: a greater version relocks whatever the tag state, the same version relocks only when untagged, unknown refuses with "could not reach origin to prove v… is untagged; retry online or bump Cargo.toml past …". `lock()` in `tests/skills_bundle.rs` probes the repo root under `FACTORY_BUNDLE_WRITE` only, so plain `cargo test` (CI) never touches the network.
- **Tests**: the old same-version test split into `a_skill_edit_without_a_bump_fails_the_check` (the check message, unchanged) and `an_untagged_version_relocks_and_a_tagged_one_refuses`; `relock_refuses_when_origin_cannot_be_reached` builds fixture repos in a temp dir (missing-path origin, no origin, bare origin with and without the tag, local tag with no origin). Fixture tags point at an empty tree from `git mktree`, so no commit and no git identity is needed. The bump tests now pass every tag state.
- **Inner loop**: 1 run, 0 red, 2 min.
- **Deliberate reds**: `an_untagged_version_relocks_and_a_tagged_one_refuses` went red with the same-version-untagged arm removed (`Err(bump…)` where `Ok(0.2.0, bb)` was expected). `relock_refuses_when_origin_cannot_be_reached` went red with every non-zero `ls-remote` exit mapped to untagged, the `release.sh` trap (`No` where `Unknown` was expected).
- **Prove (checkpoint)**: FR-3 row, both `run` tests.
- **Deviations**: none.
- **Proven (checkpoint 1)**: FR-3 `an_untagged_version_relocks_and_a_tagged_one_refuses` and `relock_refuses_when_origin_cannot_be_reached` passed — `cp-1/fr3-tests.txt`

**Slice**: After this slice, `FACTORY_BUNDLE_WRITE=1 cargo test` relocks a changed skills bundle at the same version when `v<version>` is tagged neither locally nor on origin. It refuses once either holds the tag. When origin cannot be reached, it refuses and says to retry online or bump. The second-skill-edit trip that PRD-005, 007, 008, 012, 017 and 018 hit is gone.

## Current code / environment

- **`relock`**: `src/skills.rs:165-182`, `pub fn relock(old: &Lock, version: &str, hash: &str)`. It returns `Ok(new)` only when `parts(version)? > parts(&old.version)?`. Otherwise it returns "skills changed since {v}: bump Cargo.toml past {v} first". It is pure, and stays pure: give it the tag state as an argument (prd §5, "the caller passes whether the version is tagged").
- **Its only caller**: `lock()` in `tests/skills_bundle.rs:186-207`, under `FACTORY_BUNDLE_WRITE` (`:196-197`). CI runs plain `cargo test` (`.github/workflows/ci.yml:20`), which only checks (`bundle_lock_matches`, `:202`), so CI never probes the network.
- **The probe**: a small function with three outcomes (tagged, untagged, unknown):
  - local: `git rev-parse -q --verify refs/tags/v<version>`;
  - origin: `git ls-remote --exit-code --tags origin refs/tags/v<version>`.
  - It may live in `src/skills.rs` as a `pub fn`, so a test can drive it against fixture repos. Call it from `lock()`.
- **Trap**: `scripts/release.sh:31` pipes `ls-remote` to `/dev/null 2>&1`, which treats every non-zero exit alike. `--exit-code` exits 2 for "no matching ref". Any other non-zero exit (128: no `origin`, unreachable) is unknown, so refuse. Never map it to "untagged".
- **Tests**:
  - `a_skill_edit_without_a_bump_fails_and_cannot_relock` (`src/skills.rs:886`) asserts a same-version relock is an `Err`. It splits by tag state.
  - `a_skill_edit_with_a_bump_relocks` and `a_binary_only_bump_fails_the_check_and_relocks_freely` (`:904-926`) must still pass.
- **Deliberate non-change**: prd §5 rejected trusting local tags when origin is unreachable. No skill text mentions relock today, and none is added (prd §5).

## Tasks

### 1. Relock by tag state — high — blocked by: —
**Source**: §5 FR-3: "At the same version, relock replaces the hash when `v<version>` is a tag neither locally nor on origin. It keeps refusing once either has the tag… When origin cannot be reached (offline, or no `origin` remote), it refuses and says so."

In `src/skills.rs`, `relock` takes the tag state. Same version plus untagged gives the new lock. Tagged gives today's refusal. Unknown refuses with "could not reach origin to prove v… is untagged; retry online or bump". A greater version relocks as today, whatever the tag state.

**Acceptance:** `an_untagged_version_relocks_and_a_tagged_one_refuses`, a unit test replacing `:886`. The same version untagged relocks, and the same version tagged refuses with the bump message.

### 2. The probe tells "no tag" from "cannot reach" — high — blocked by: 1
**Source**: §5 FR-3 trap: "`ls-remote --exit-code` exits 2 for 'no such ref' and other codes (128) for 'cannot reach'. The probe must tell exit 2 apart from the rest."

Write the probe and call it from `tests/skills_bundle.rs` `lock()`.

**Acceptance:** `relock_refuses_when_origin_cannot_be_reached`, against fixture repos built in a temp dir:

- an origin pointing at a missing path gives unknown, and relock refuses with the online message;
- a bare origin holding the tag gives tagged;
- a bare origin without it gives untagged;
- a local tag gives tagged even with no origin.

### 3. Verify — medium — blocked by: 2
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q`

**Acceptance:** inner loop green, one commit.

### 4. Prove — at the checkpoint — blocked by: 3
prd §12 row **FR-3**: `run an_untagged_version_relocks_and_a_tagged_one_refuses` and `run relock_refuses_when_origin_cannot_be_reached`. It is proven below the UI.

**Acceptance:** after the checkpoint, the transcripts are in Progress.

**Code-review fixes (2026-11-13).**

- `tag_state` ran `git ls-remote origin` on every `FACTORY_BUNDLE_WRITE=1` run, even a bump or an unchanged bundle, and ssh can stall on it. `relock` now takes the probe as a closure and calls it only for a same-version hash change (AD-6). The bump test proves no probe runs.
- `check_lock`'s stale message still said to bump first. It now says to relock first, and to bump only once the version is tagged.
- The `tag_state` test's git helper read the user's global config, and `tag.gpgSign=true` turned its lightweight tags red. It now runs with `GIT_CONFIG_GLOBAL=/dev/null` and `GIT_CONFIG_NOSYSTEM=1`, like the other git tests.
