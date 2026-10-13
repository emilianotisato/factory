# Slice 07 — Bundle lock and the 0.2.0 bump

**Source**: [`prd.md`](prd.md) → §7 FR-5 (every decision and the acceptance), §10 FR-8's human row ("the owner tags `v0.2.0` … installs `0.2.0`"), which needs the crate at `0.2.0` before the tag. **Depends on**: 02 (the acceptance compares the lock with the `bundle` value `skills::render` writes into a manifest), 04 (it is the last slice that edits `skills/`; locking before it would cost a second bump inside this PRD).
**Created**: 2026-10-13

**Progress**: built 2026-10-13.
CP-2 FR-5 in a throwaway worktree of HEAD plus the staged index, one space appended to `skills/ux/references/rubric.md`:
1. red, `skills/bundle.lock is stale: bump version in Cargo.toml`, exit 101: `docs/features/004-skills/evidence/cp-2-fr5-1-red.txt`
2. refused, `FACTORY_BUNDLE_WRITE=1` at 0.2.0, `skills changed since 0.2.0: bump Cargo.toml past 0.2.0 first`, exit 101: `docs/features/004-skills/evidence/cp-2-fr5-2-refused.txt`
3. rewrite at `0.2.1`, 5 passed, the lock now `version = "0.2.1"`: `docs/features/004-skills/evidence/cp-2-fr5-3-rewrite.txt`
4. green, 5 passed: `docs/features/004-skills/evidence/cp-2-fr5-4-green.txt`
Worktree removed; the branch's `skills/bundle.lock` still says `0.2.0`.

- **Built**: `src/skills.rs` gains `LOCK`, `Lock` (`parse`, `text`), `version_parts` (the one integer parser; slice 03's `verdict_for` calls it too, AD-6), `check_lock` and `relock`, with six unit tests covering every case task 1 lists. `tests/skills_bundle.rs` gains `bundle_lock_matches` and `the_lock_records_what_a_manifest_carries`, both reading one `OnceLock` result so a write run never races a reader (AD-7). `Cargo.toml` and `Cargo.lock` move to `0.2.0`; `skills/bundle.lock` is written at `0.2.0`, sha256 `8845b9b6…2619`. `tests/knobs.rs` builds its expected tail from `CARGO_PKG_VERSION`.
- **Lock written**: `cargo test --test skills_bundle` with no lock went red twice with `no skills/bundle.lock: run FACTORY_BUNDLE_WRITE=1 cargo test`. The write run passed 5 of 5, and the plain run after it passed 5 of 5.
- **Inner loop**: 2 runs, 1 red, 1.2 min. The red run was `cargo fmt --check` on the new code. The green run took 71 s.
- **Commit**: `Cargo.toml`, `Cargo.lock`, `skills/bundle.lock`, `tests/skills_bundle.rs` and `tests/knobs.rs` are staged by name. `src/skills.rs` also holds slice 03's uncommitted hunks (AD-6).
- **Prove**: the FR-5 row (the four worktree transcripts) is left for the checkpoint.

**Slice**: After this slice a skill edit cannot ship under an old version number. `skills/bundle.lock` records the crate version and the bundle hash, and a gate test fails whenever either one disagrees with `Cargo.toml` and the embedded bundle, naming the lock and the two lines that fix it. `FACTORY_BUNDLE_WRITE=1 cargo test` rewrites the lock, but refuses when the bundle changed and the version did not go up. The crate moves to `0.2.0`, the first version that carries the family, and the lock is written at it. Tagging `v0.2.0` is the owner's, after the merge (qa-guide).

**Highest risk: none of the code; the trap is timing.** From this commit on, every edit under `skills/` fails the gate until the version goes up. A polish or review fix to a skill after this slice means `0.2.1`, and then the owner tags whatever `Cargo.toml` says.

## Current code / environment

- **The version**: `Cargo.toml:3` is `version = "0.1.0"` and has never moved; `Cargo.lock` is committed and carries the package's own version, so the bump changes both.
- **The bundle hash** (slice 01): `skills::bundle_hash()` is `hash_of(FILES)`, a sha256 over the 19 embedded entries sorted by path, each fed as `path`, `\0`, `content`, `\0`. `FILES` is `include_str!`, so cargo rebuilds when a file under `skills/` changes, and slice 01's walk in `tests/skills_bundle.rs` already fails when disk and bundle disagree. That walk skips `skills/README.md` and `skills/bundle.lock` by name (PRD §3, clarified at slicing), so the lock never hashes itself.
- **The manifest** (slice 02): `skills::render` writes `bundle = bundle_hash()` and `version = env!("CARGO_PKG_VERSION")` into `.claude/skills/factory.toml`.
- **Version comparison** (slice 03): `skills::verdict` compares versions as three integers, never as strings. Reuse that parser here; do not write a second one.
- **Tests that pin the version**: `tests/knobs.rs:149` asserts the knobs line `ends_with("lanes 1 · factory 0.1.0 · claude 2.1.296")`; the binary prints its own `CARGO_PKG_VERSION` there (`src/config.rs:206`). `src/cli/report.rs:1120-1134` also says `0.1.0`, but as fixed JSON input to `knobs_text`, so the bump leaves it green. The slice 03 fence tests build their versions from `env!("CARGO_PKG_VERSION")` already.
- **Integration tests see the version**: `env!("CARGO_PKG_VERSION")` and `env!("CARGO_MANIFEST_DIR")` are set for `tests/*.rs`, so the test needs no `Cargo.toml` parsing.
- **Deliberate non-changes** — *(prd §7 FR-5: "Chosen over a `build.rs` git-sha stamp … and over convention alone." prd §7: "A version bump with an unchanged bundle re-locks freely: that is a binary-only release." Do not add a `build.rs`, a git hook, or a refusal for a version bump without a skill change.)*

## Tasks

### 1. The lock check and the rewrite rule — high — blocked by: —
**Source**: §7 FR-5: "`skills/bundle.lock` records `version` and `sha256` of the bundle. The test passes only when **both** the recomputed hash equals the lock's hash **and** `Cargo.toml`'s version equals the lock's version; otherwise it fails naming the lock and the two lines to run (bump `Cargo.toml`, then `FACTORY_BUNDLE_WRITE=1 cargo test`). The write mode rewrites the lock from `Cargo.toml` and `skills/`, but **refuses when the hash changed and `Cargo.toml`'s version is not greater than the lock's**".

`src/skills.rs`, two pure functions so the rule is tested without touching the real lock:

- `pub struct Lock { version: String, sha256: String }`, serde, TOML: `version = "0.2.0"` and `sha256 = "<hex>"`. `LOCK: &str = "skills/bundle.lock"`.
- `pub fn check_lock(lock: &Lock, version: &str, hash: &str) -> Result<(), String>`: `Ok` only when both match. The error names `skills/bundle.lock`, says which side differs (`bundle changed: <lock> → <now>`, `version: lock <a>, Cargo.toml <b>`), and ends with the two lines: `bump version in Cargo.toml` and `FACTORY_BUNDLE_WRITE=1 cargo test`.
- `pub fn relock(old: Option<&Lock>, version: &str, hash: &str) -> Result<Lock, String>`: refuses when `old` exists, its `sha256` differs from `hash`, and `version` is not greater than `old.version` by the integer comparison. The refusal says `skills changed since <old.version>: bump Cargo.toml past <old.version> first`. Otherwise it returns the new lock. No old lock is the first write.

**Acceptance:** unit tests in `src/skills.rs`: both equal → `check_lock` ok; hash differs at the same version → `check_lock` errs naming the lock and both lines, `relock` refuses; hash differs and version `0.2.1` over `0.2.0` → `relock` ok; same hash and a greater version → `check_lock` errs, `relock` ok; `0.10.0` over `0.9.0` counts as greater.

### 2. The gate test, and the write mode — high — blocked by: 1
**Source**: §7 FR-5: "A gate test enforces it." "The write mode rewrites the lock from `Cargo.toml` and `skills/`". Acceptance: "Editing one byte under `skills/` without a bump makes `cargo test` fail naming the lock, and `FACTORY_BUNDLE_WRITE=1 cargo test` refuses to re-lock".

`tests/skills_bundle.rs`, one test `bundle_lock_matches`. It reads `<CARGO_MANIFEST_DIR>/skills/bundle.lock`. When `FACTORY_BUNDLE_WRITE` is set, it calls `relock(old, env!("CARGO_PKG_VERSION"), &bundle_hash())`: on `Ok`, it writes the lock and passes; on `Err`, it panics with the refusal. When it is not set, it panics with `check_lock`'s message on `Err`, and with "no skills/bundle.lock: run FACTORY_BUNDLE_WRITE=1 cargo test" when the file is missing.

The trap: the write mode runs inside `cargo test`, so every other test in the run also executes. Only this one test reads the variable. Nothing else may write under `skills/`, or a write run would rehash its own output.

**Acceptance:** with the lock committed, `cargo test --test skills_bundle` passes; the task-1 unit tests carry the failing cases, since the integration test reads the real lock.

### 3. Bump to 0.2.0 and write the lock — high — blocked by: 2
**Source**: §7 FR-5: "The crate version is the family version." "Release ritual (FR-8): bump, rewrite the lock, commit, tag `v<version>`, push the tag." §12 FR-8 human row: "the owner tags `v0.2.0` … installs `0.2.0`". Acceptance: "the hash the lock records equals the `bundle` value FR-3 writes into a manifest".

`Cargo.toml:3` → `version = "0.2.0"`. Build once so `Cargo.lock` follows, then `FACTORY_BUNDLE_WRITE=1 cargo test --test skills_bundle` writes `skills/bundle.lock`. Commit `Cargo.toml`, `Cargo.lock` and `skills/bundle.lock` together. Then `tests/knobs.rs:149`: build the expected tail with `format!("lanes 1 · factory {} · claude 2.1.296", env!("CARGO_PKG_VERSION"))`.

The trap: a run commits the paths its worker wrote with Edit or Write. `Cargo.lock` and `skills/bundle.lock` are written by `cargo`, so name all three in the commit by hand.

**Acceptance:** `tests/skills_bundle.rs` gains a test that renders a temp git repo with `skills::render` and asserts the manifest's `bundle` equals the lock's `sha256` and its `version` equals the lock's `version`. `tests/knobs.rs` passes at `0.2.0`.

### 4. Verify — medium — blocked by: 1-3
`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 5. Prove — at the checkpoint — blocked by: 4
Rows from [`prd.md`](prd.md) §12 this slice proves, below the UI: the **FR-5 row**, run `cargo test` after a one-byte skill edit without a bump (fails naming the lock), then with the bump and lock rewrite (passes).

Do it in a throwaway worktree, never in the branch's own tree: `git worktree add <scratch>/fr5 HEAD`. There, append one space to `skills/ux/references/rubric.md` and run `cargo test --test skills_bundle` (red, naming `skills/bundle.lock`). Run `FACTORY_BUNDLE_WRITE=1 cargo test --test skills_bundle` (refused). Set `Cargo.toml` to `0.2.1`, run the write again (passes), then `cargo test --test skills_bundle` (green). Remove the worktree with `git worktree remove --force <scratch>/fr5`.

**Acceptance:** after the checkpoint, Progress holds the four transcripts in order: red naming the lock, the refusal, the rewrite, green. The branch's `skills/bundle.lock` still says `0.2.0`.
