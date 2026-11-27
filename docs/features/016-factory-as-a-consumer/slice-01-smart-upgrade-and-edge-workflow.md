# Slice 01 — Smart factory upgrade and edge workflow

**Source**: [`prd.md`](prd.md) → §3 (FR-1). **Depends on**: — (independent; writes no files touched by Slices 02–04)
**Created**: 2026-11-27

**Progress**: _(written as it lands — what was built, the gate numbers, what a review caught)_

**Slice**: Lets the owner upgrade `factory` as a normal consumer — verifying receipt version against the running binary, allowing `factory upgrade` to install from GitHub Releases even when no receipt exists (when `[source]` is unset), adding `--edge` prerelease opt-in, adding a rolling `.github/workflows/edge.yml` prerelease workflow on `main` pushes, and retiring the `cargo install --path . --locked` post-merge rule from `CLAUDE.md` and `AGENTS.md`. Leaves home-path shapes (Slice 02) and `factory upstream` changes (Slices 03–04) to their own slices.

**Highest-risk**: `install_prefix_from_exe` in `src/cli/upgrade.rs` and `.github/workflows/edge.yml` version patching/tag lookup — getting `FACTORY_INSTALL_DIR` wrong would install into `~/.cargo/bin/bin/factory`, and omitting `fetch-depth: 0` in `edge.yml` would make `git rev-parse` miss existing release tags.

## Current code / environment

- **`src/cli/upgrade.rs:12-30`**: `UpgradeArgs` has only `--check`. `updater()` calls `u.load_receipt().ok()?` and checks `u.check_receipt_is_for_this_executable().unwrap_or(false)` without verifying the receipt's recorded `version` against `env!("CARGO_PKG_VERSION")`. Because `dist-workspace.toml:15` sets `install-path = "CARGO_HOME"`, a `cargo install` binary in `~/.cargo/bin/factory` collides with an older release receipt.
- **`src/cli/upgrade.rs:58-100`**: When `updater()` returns `None`, `upgrade()` always prints `installed from source — <source_line>` and exits `0`, even when `[source]` is unset in `config.toml`.
- **`axoupdater 0.10.2` & live `v0.18.0` `factory-installer.sh:595-600`**: Setting `FACTORY_INSTALL_DIR` / `CARGO_DIST_FORCE_INSTALL_DIR` forces `_install_layout="cargo-home"`, which **appends `/bin`**. When `AxoUpdater` has no loaded receipt (`current_version_installed_by == None`), `install_prefix_root()` does NOT strip `/bin` before passing `install_dir` to the installer. Therefore `install_prefix_from_exe(exe: &Path) -> PathBuf` must strip a trailing `"bin"` component from `exe.parent()` (`~/.cargo/bin/factory` → `~/.cargo`, `/usr/local/bin/factory` → `/usr/local`, `/opt/factory/factory` → `/opt/factory`).
- **`axoupdater 0.10.2` receipt search order**: `AXOUPDATER_CONFIG_WORKING_DIR`, `AXOUPDATER_CONFIG_PATH`, `XDG_CONFIG_HOME/factory`, `~/.config/factory` (filename `factory-receipt.json`, JSON field `"version"`).
- **`axoupdater 0.10.2` prereleases**: `u.configure_version_specifier(UpdateRequest::LatestMaybePrerelease)` queries `{api}/repos/{owner}/{name}/releases` (rather than `/releases/latest`) and parses `tag_name` as SemVer, filtering for assets starting with `factory-installer`.
- **`.github/workflows/release.yml:1-296`**: Tag-triggered (`on: push: tags: '**[0-9]+.[0-9]+.[0-9]+*'`), uses `cargo-dist` `0.32.0`, 4 Unix targets (`dist-workspace.toml:1-23`), `actions/checkout@v6`. Pushing a tag with `GITHUB_TOKEN` does NOT trigger `release.yml`, so `edge.yml` must build and publish directly.
- **`tests/upgrade.rs:1-187`**: Two tests are authorized in `prd.md` frontmatter (`tests/upgrade.rs::no_receipt_and_no_source_prints_the_installer` at `:135` and `tests/upgrade.rs::a_receipt_for_another_prefix_is_from_source` at `:148`). `no_receipt_prints_the_source_line_from_config` (`:123`), `check_prints_the_newer_release_and_installs_nothing` (`:160`), and `check_on_the_latest_release_is_already_current` (`:176`) are NOT authorized and must remain green untouched. **Do NOT edit shared helpers (`release_server`, `Env::receipt`, `Env::upgrade`) outside authorized test functions** — add new helper functions instead so `testguard` stays clean.
- **`tests/release.rs:62-72`**: `the_readme_offers_the_installer_before_cargo_install` asserts `README.md` contains both `releases/latest/download/factory-installer.sh` and `cargo install --path .` (with the installer appearing first).
- **`CLAUDE.md:6`, `AGENTS.md:6`, `README.md:107`**: Currently instruct `cargo install --path . --locked` after every merge and describe the receipt-confusion workaround.
- **Deliberate non-change**: Moving `dist-workspace.toml` `install-path` away from `CARGO_HOME` *(prd §10 Non-goals decided against it. Do not edit `dist-workspace.toml`.)*

## Tasks

### 1. Receipt version check, receipt-less upgrade, and `--edge` — high — blocked by: —
**Source**: §3 FR-1: "Read `factory-receipt.json` from the same config search order `axoupdater` uses... require `receipt.version == env!(\"CARGO_PKG_VERSION\")` alongside `u.check_receipt_is_for_this_executable().unwrap_or(false)` inside `from_release()` / `matching_receipt()`. Add pure helper `pub fn install_prefix_from_exe(exe: &Path) -> PathBuf`... Add `#[arg(long)] pub edge: bool` to `UpgradeArgs`... If `cfg.source.is_some()` AND `!from_release()` AND `!args.edge`... keep printing `installed from source — cargo install --path <dir>`... Otherwise... use that `AxoUpdater`; else build `AxoUpdater::new_for(\"factory\")`, configure `ReleaseSource`... and call `u.set_install_dir(install_prefix_from_exe(&paths::self_exe()?))`."

`src/cli/upgrade.rs`, `tests/upgrade.rs`.
- Read `factory-receipt.json` from the first existing directory in: `AXOUPDATER_CONFIG_WORKING_DIR`, `AXOUPDATER_CONFIG_PATH`, `XDG_CONFIG_HOME/factory`, `$HOME/.config/factory`. Parse its `"version"` string field and require it to equal `env!("CARGO_PKG_VERSION")` inside `updater()` (so `from_release()` and `ahead_hint()` return false / fall back to `[source]` when a `cargo install` build has a different version from the receipt).
- Add `pub fn install_prefix_from_exe(exe: &Path) -> PathBuf`: take `exe.parent().unwrap_or(Path::new("/"))`, and if `parent.file_name() == Some(OsStr::new("bin"))`, return `parent.parent().unwrap_or(parent).to_path_buf()`, else `parent.to_path_buf()`.
- Add `#[arg(long)] pub edge: bool` to `UpgradeArgs`.
- Update `upgrade(args, out)`:
  1. Load `cfg = config::load(&paths::config_dir())?.config`.
  2. Resolve `matching = updater()`.
  3. If `cfg.source.is_some() && matching.is_none() && !args.edge`, print `installed from source — cargo install --path <dir>` (via `source_line(&cfg)`) and return `Ok(0)`.
  4. Otherwise obtain `mut u`: use `matching` if `Some`, else build `AxoUpdater::new_for("factory")`, call `u.set_release_source(ReleaseSource { release_type: ReleaseSourceType::GitHub, owner: "emilianotisato".into(), name: "factory".into(), app_name: "factory".into() })`, and call `u.set_install_dir(&install_prefix_from_exe(&paths::self_exe()?))` (use the exact `axoupdater 0.10.2` `set_install_dir` signature — check compiler types; `set_install_dir(&str)` takes `&str`).
  5. Set `current_version` to `env!("CARGO_PKG_VERSION")` and `u.configure_version_specifier(if args.edge { UpdateRequest::LatestMaybePrerelease } else { UpdateRequest::Latest })`.
  6. Keep `--check` and `run_sync()` output format identical (`{current} → {v}` / `already current`).
- **The trap**: Do NOT touch `release_server`, `Env::receipt`, or `Env::upgrade` in `tests/upgrade.rs` (unauthorized shared code). In the two authorized tests (`no_receipt_and_no_source_prints_the_installer` and `a_receipt_for_another_prefix_is_from_source`), pass `Some(&release_server("v999.0.0"))` with `--check` (NEVER run unstubbed `&[]` against real `github.com`!). Add new test helpers (e.g., `write_receipt_with_version`, `releases_server_with_prerelease` matching `/api/v3/repos/emilianotisato/factory/releases` by path prefix so query strings like `?per_page=...` still match) and new tests for: mismatched-version receipt making `from_release()` false / respecting `[source]`, receipt-less `--check` returning `already current` on `v{V}`, and `--check --edge` discovering `999.0.0-edge.42` while plain `--check` ignores prereleases.

**Acceptance:** Unit tests in `src/cli/upgrade.rs` prove `install_prefix_from_exe` maps `~/.cargo/bin/factory` → `~/.cargo`, `/usr/local/bin/factory` → `/usr/local`, and `/opt/factory/factory` → `/opt/factory` (fails if `/bin` stripping is removed). Integration tests in `tests/upgrade.rs` prove: (a) no receipt + no `[source]` + `--check` against stub server prints `{V} → 999.0.0` (fails if receipt-less upgrade is reverted to printing `installed from source`), (b) mismatched-prefix receipt + no `[source]` + `--check` prints `{V} → 999.0.0` (fails if receipt-less fallback is removed), (c) mismatched-version receipt + `[source]` prints `installed from source — cargo install --path /src/factory` (fails if receipt version comparison is removed), (d) `--check --edge` prints `{V} → 999.0.0-edge.42` while plain `--check` ignores the prerelease (fails if `UpdateRequest::LatestMaybePrerelease` is removed).

### 2. Rolling `edge` prerelease workflow — high — blocked by: —
**Source**: §3 FR-1: "Rolling `edge` prerelease workflow (`.github/workflows/edge.yml`): Triggers on `push` to `main` (with `paths-ignore: ['docs/**', 'README.md', 'CLAUDE.md', 'AGENTS.md']`) and `workflow_dispatch`... `plan` job uses `actions/checkout@v6` with `fetch-depth: 0`... reads `version` from `Cargo.toml` (`X.Y.Z`), checks `git rev-parse -q --verify \"refs/tags/v${VER}\"` (if that tag already exists, increments patch to `X.Y.(Z+1)`...), sets `EDGE_VER=\"${BASE}-edge.${GITHUB_RUN_NUMBER}\"`, and exports `edge_ver` and `edge_tag=\"v${EDGE_VER}\"`... Every build job... checks out `main` and runs the same version-patch step updating `Cargo.toml` and `Cargo.lock` in the CI workspace (never committed) to `EDGE_VER` before invoking `dist build --tag=\"${EDGE_TAG}\" ...`. Publish job runs `gh release create \"${EDGE_TAG}\" --prerelease ...`, then deletes older `*-edge.*` prereleases (and their tags)."

`.github/workflows/edge.yml`, `tests/release.rs`.
- Create `.github/workflows/edge.yml` modeled on `.github/workflows/release.yml` (same `cargo-dist` `0.32.0`, same 4 Unix targets via `dist plan` matrix, `actions/checkout@v6`, `actions/upload-artifact@v7`, `actions/download-artifact@v8`):
  - `on`: `push: branches: [main]` with `paths-ignore: ['docs/**', 'README.md', 'CLAUDE.md', 'AGENTS.md']`, plus `workflow_dispatch:`.
  - `permissions: { contents: write }`.
  - `concurrency: { group: edge-release, cancel-in-progress: true }`.
  - `plan` job: `actions/checkout@v6` with `fetch-depth: 0`, compute `BASE` from `Cargo.toml` `version` (if `git rev-parse -q --verify "refs/tags/v${VER}"` succeeds, bump patch `Z` → `Z+1`), set `EDGE_VER="${BASE}-edge.${GITHUB_RUN_NUMBER}"` and `EDGE_TAG="v${EDGE_VER}"`, patch `Cargo.toml` and `Cargo.lock` in workspace to `EDGE_VER`, install `cargo-dist` `0.32.0`, run `dist plan --tag="${EDGE_TAG}" --output-format=json` (or `dist host --steps=create` equivalent without creating a release prematurely — verify `dist plan --tag=...` matches `release.yml`), and export `manifest`, `edge-ver`, `edge-tag`.
  - `build-local-artifacts` and `build-global-artifacts`: each checks out the commit, runs the exact same workspace-only `Cargo.toml` + `Cargo.lock` version patch to `${{ needs.plan.outputs.edge-ver }}` before `dist build --tag="${{ needs.plan.outputs.edge-tag }}" ...`, and uploads artifacts.
  - `publish` job: downloads all build artifacts, removes granular `*-dist-manifest.json` files, runs `gh release create "${EDGE_TAG}" --prerelease --target "${GITHUB_SHA}" --title "factory ${EDGE_TAG}" --notes "Rolling edge build from ${GITHUB_SHA}." artifacts/*`, then lists existing prereleases matching `*-edge.*` (excluding `${EDGE_TAG}`) via `gh release list --json tagName,isPrerelease` and deletes each older one with `gh release delete "$tag" --yes --cleanup-tag`.
- **The trap**: Patch BOTH `Cargo.toml` (only the `[package]` `version = "..."` line) and `Cargo.lock` (only the `name = "factory"` package's `version = "..."` line) in every job that invokes `dist`, never running `git commit`. Never mention `pc-windows` in the workflow.
- Add a test in `tests/release.rs` (`the_edge_workflow_builds_and_prunes_rolling_prereleases_on_main`) verifying `.github/workflows/edge.yml` triggers on `push` to `main` (with `paths-ignore` covering `docs/**`), checks out with `fetch-depth: 0`, checks `refs/tags/v`, patches `Cargo.toml` and `Cargo.lock`, uses `dist build`, creates a `--prerelease` with `gh release create`, and prunes older `-edge.` prereleases.

**Acceptance:** `cargo test --test release` passes with `the_edge_workflow_builds_and_prunes_rolling_prereleases_on_main` asserting every workflow invariant above (fails if `.github/workflows/edge.yml` is deleted or any required trigger/`fetch-depth: 0`/lock-patch/`--prerelease`/prune step is removed).

### 3. Retire the `cargo install` post-merge rule from repo docs — medium — blocked by: 1
**Source**: §3 FR-1: "Replace the `cargo install --path . --locked` rule in `CLAUDE.md:6` and `AGENTS.md:6` with consumer upgrade guidance (`factory upgrade` / `factory upgrade --edge` once a release builds; `cargo install --path . --locked` only when testing local unmerged builds). Replace the receipt-confusion workaround at `README.md:107` with how `factory upgrade` and `factory upgrade --edge` work, keeping a mention of `cargo install --path .` after the installer URL so `tests/release.rs:62-72` stays green."

`CLAUDE.md`, `AGENTS.md`, `README.md`, `tests/release.rs`.
- Update `CLAUDE.md:6` and `AGENTS.md:6` identically: replace the post-merge `cargo install --path . --locked` mandate with consumer upgrade guidance (`factory upgrade` for tagged releases, `factory upgrade --edge` for rolling `main` builds; use `cargo install --path . --locked` only when testing unmerged local changes).
- Update `README.md:80` and `README.md:107`: document `factory upgrade` (latest tagged release) and `factory upgrade --edge` (latest rolling `main` prerelease), replacing the stale receipt-confusion paragraph at `:107` while preserving `cargo install --path .` after the installer URL so `tests/release.rs:62-72` stays green.
- Add an assertion in `tests/release.rs` proving `CLAUDE.md` and `AGENTS.md` mention `factory upgrade --edge` instead of mandating `cargo install --path . --locked` after a merge, and `README.md` mentions `factory upgrade --edge` and drops the stale "takes the source build for a release install" sentence.

**Acceptance:** `cargo test --test release` passes, proving `the_readme_offers_the_installer_before_cargo_install` stays green and the updated guidance assertions pass (fails if the old `After a merge, run cargo install` text is restored or `--edge` guidance is removed).

### 4. Verify — medium — blocked by: 1, 2, 3
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 5. Prove — at the checkpoint — blocked by: 4
**Source**: §8 QA plan FR-1 row: "run `cargo test --test upgrade` and `cargo test --test release` (plus post-merge observation of live `.github/workflows/edge.yml` publishing a prerelease on `main`)".

At Checkpoint 1, run `cargo test --test upgrade` and `cargo test --test release` alongside the full gate and record the transcript in **Progress**. (The post-merge observation of live `.github/workflows/edge.yml` is recorded in [`qa-guide.md`](qa-guide.md) for the owner after merge.)

**Acceptance:** Checkpoint transcript in **Progress** confirms every test in `tests/upgrade.rs` and `tests/release.rs` passes (would fail if receipt version check, receipt-less upgrade, `--edge`, `edge.yml`, or doc guidance were broken).
