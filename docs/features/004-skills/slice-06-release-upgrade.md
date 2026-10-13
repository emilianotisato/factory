# Slice 06 — Releases and factory upgrade

**Source**: [`prd.md`](prd.md) → §10 FR-8 (the release workflow, `factory upgrade`, the README), and §5 FR-3's `ahead` hint ("or the `cargo install --path <source>` line when the binary was not installed from a release, FR-8"). **Depends on**: 03 (it rewrites the `ahead` hint slice 03 prints, in `skills::verdict`), 05 (it reads `[source] path`, which slice 05 adds to the user config).
**Created**: 2026-10-13

**Progress**: `dist init` (cargo-dist 0.32.0, installed locally) wrote `.github/workflows/release.yml` and **`dist-workspace.toml`**, not `[workspace.metadata.dist]` (PRD FR-8 acceptance carries the dated correction); its "good defaults" added `x86_64-pc-windows-msvc`, removed by hand before `dist generate`. `Cargo.toml` gains `repository`, `[package.metadata.dist] dist = true` (without it `publish = false` leaves dist nothing to release) and `[profile.dist]`; `ci.yml` is byte-identical. **aarch64**: the plan's matrix builds `aarch64-unknown-linux-gnu` on a native `ubuntu-22.04-arm` runner (macOS on `macos-14` and `macos-15-intel`, x86_64 Linux on `ubuntu-22.04`), so nothing cross-compiles. **TLS**: `cargo tree -i aws-lc-sys` → `aws-lc-rs` → `rustls` → `reqwest 0.13.5` → `axoasset 2.0.1` → `axoupdater 0.10.2`; `cargo tree -i openssl-sys` matches no package. A C TLS crate on native runners only, so the stack stays. `factory upgrade [--check]` in `src/cli/upgrade.rs` (`from_release`, `source_line`, `ahead_hint`, `INSTALLER`), `axoupdater` with `blocking` and `tokio` (`rt`) for the local `--check` runtime; doctor, preflight and `skills update` print the `ahead` hint through `Verdict::lines`, which asks `ahead_hint` only on `ahead`. `tests/upgrade.rs` (5, a stub release on `127.0.0.1:0`): source line from `[source]`, the installer line without, a receipt for `/nonexistent`, `--check` → `0.2.0 → 999.0.0` with the binary's mtime unchanged, `--check` on `v0.2.0` → `already current`. `tests/release.rs` (3): the four targets, `installers = ["shell"]`, `install-updater = false`, no `pc-windows`, the tag pattern `'**[0-9]+.[0-9]+.[0-9]+*'`, the installer before `cargo install` in the README. `tests/skills_fence.rs` (9): `ahead` ends with the installer line, a new test with `[source] path = "/src/factory"` for doctor and `update` (exit 1); config and receipt dirs under the fixture's `.git`. `tests/harness/mod.rs` sets `AXOUPDATER_CONFIG_PATH`. Both fence and upgrade tests pass with a receipt for the test binary under `XDG_CONFIG_HOME`. Inner loop: 1 run, 0 red, 1.5 min. The fresh binary on this machine, no receipt: `installed from source — curl --proto '=https' --tlsv1.2 -LsSf https://github.com/emilianotisato/factory/releases/latest/download/factory-installer.sh | sh`, exit 0; with a receipt and no server, `factory: error sending request …`, exit 1. Release machinery files: AD-10. Prove rows (FR-8 machine row) left for the checkpoint.
CP-2 FR-8 machine row, `tests/release.rs` (3 passed) and `tests/upgrade.rs` (5 passed, `--check` against the stub release): `docs/features/004-skills/evidence/cp-2-fr8-release-upgrade-tests.txt`.
CP-2 FR-8 `dist plan` (cargo-dist 0.32.0, the four archives and `factory-installer.sh`, matching the block below): `docs/features/004-skills/evidence/cp-2-fr8-dist-plan.txt`.
CP-2 FR-8 no-receipt line, the fresh debug binary with no `~/.config/factory/`: `installed from source — curl … factory-installer.sh | sh`, exit 0: `docs/features/004-skills/evidence/cp-2-fr8-no-receipt.txt`.
CP-2 FR-8 human row (tag, green release, installer on a second machine, `already current`) is the owner's after the merge.

`dist plan`, 2026-10-13:

```
announcing v0.2.0
  factory 0.2.0
    source.tar.gz
      [checksum] source.tar.gz.sha256
    factory-installer.sh
    sha256.sum
    factory-aarch64-apple-darwin.tar.xz
      [bin] factory
      [misc] README.md
      [checksum] factory-aarch64-apple-darwin.tar.xz.sha256
    factory-aarch64-unknown-linux-gnu.tar.xz
      [bin] factory
      [misc] README.md
      [checksum] factory-aarch64-unknown-linux-gnu.tar.xz.sha256
    factory-x86_64-apple-darwin.tar.xz
      [bin] factory
      [misc] README.md
      [checksum] factory-x86_64-apple-darwin.tar.xz.sha256
    factory-x86_64-unknown-linux-gnu.tar.xz
      [bin] factory
      [misc] README.md
      [checksum] factory-x86_64-unknown-linux-gnu.tar.xz.sha256
```

**Slice**: After this slice a developer without Rust can install `factory`, and an installed binary can update itself. `dist init` writes a release workflow that, on a `v*` tag, builds four Linux and macOS archives, checksums and `factory-installer.sh` onto a GitHub release. `factory upgrade` reads the installer's receipt through `axoupdater`, replaces the binary and prints `<old> → <new>` or `already current`; `--check` only prints. A binary not installed from a release prints the `cargo install --path <source>` line, or the installer line when no `[source]` is set, and doctor's `ahead` hint follows the same rule. The tag itself, and so the first real release, is the owner's after the merge (qa-guide).

**Highest risk: the release workflow cannot run before the tag**, so the gate only proves its text. Two facts decide whether the first tag goes green, and both are the builder's to check at `dist init`: how cargo-dist builds `aarch64-unknown-linux-gnu`, and what TLS stack `axoupdater` pulls in, since a C-compiled TLS crate is what breaks a cross build.

## Current code / environment

- **No release machinery**: `.github/workflows/` holds only `ci.yml`, one job running the gate on `main` pushes and PRs. No tags, no releases (`gh release list` empty, 2026-10-13); the repo `emilianotisato/factory` is public. `cargo-dist` is not installed here (`which dist` fails); 0.32.0 is current on crates.io.
- **`Cargo.toml`**: `version = "0.1.0"` (`:3`), lib and bin both named `factory` (`:8-14`), no `[workspace.metadata.dist]`, no `[profile.dist]`. Recent cargo-dist versions may write their config to `dist-workspace.toml` instead of `Cargo.toml`; the PRD names `[workspace.metadata.dist]`, so whichever file `dist init` writes is what the test reads, and a different one is a dated correction in the PRD.
- **Unix only**: `std::os::unix` is imported across `src/` (`daemon.rs:8`, `worker.rs:33-34`, `paths.rs:9-10`, `run.rs:15`, `cli/docs.rs:8`, `docs/hook.rs:6`). Windows stays out (§14).
- **`axoupdater` 0.10.2** (read from its crate source, 2026-10-13):
  - Features: `default = ["github_releases"]`, `blocking = ["tokio"]`. `github_releases` pulls `axoasset/remote`, which is `reqwest`; its lockfile resolves `rustls` with both `ring` and `aws-lc-sys`. `aws-lc-sys` compiles C.
  - API: `AxoUpdater::new_for("factory")` (`src/lib.rs:148`), `load_receipt()` (`src/receipt.rs:56`) errors when there is none, `check_receipt_is_for_this_executable()` (`:91-118`) compares the receipt's `install_prefix` with the running exe's canonical dir (a trailing `bin` stripped), `set_current_version` (`src/lib.rs:216`), `disable_installer_output` (`:272`), `is_update_needed_sync` (`:370`), `run_sync` (`:618`). `query_new_version` (`:629`) is async only.
  - The receipt is `<dir>/factory-receipt.json`: `<dir>` is `AXOUPDATER_CONFIG_PATH`, else `$XDG_CONFIG_HOME/factory` when it exists, else `~/.config/factory` (`src/receipt.rs:123-160`). Its fields: `install_prefix`, `binaries`, `source { release_type, owner, name, app_name }`, `version`, `provider { source, version }` (`:18-39`).
  - `FACTORY_INSTALLER_GHE_BASE_URL=http://127.0.0.1:<port>` sends the lookup to `<url>/api/v3/repos/<owner>/<name>/releases/latest` (`src/release/github.rs:14-43` joins `api/v3` onto the base, `:80`). A release counts only when an asset `name` starts with `factory-installer`.
- **Tests inherit the real `HOME`**: `tests/harness/mod.rs:517-563` `build_env` sets `PATH`, `FACTORY_DATA_DIR`, `FACTORY_CONFIG_DIR` and others but never clears the environment, and the fence tests in `tests/skills_fence.rs` run the binary directly. Once the owner installs a release, `~/.config/factory/factory-receipt.json` exists on this machine.
- **The `ahead` hint today** (slice 03): `ahead (repo <a>, binary <b>) → factory upgrade`, asserted in `tests/skills_fence.rs`.
- **README**: `## Trying it` (`README.md:64-75`) opens with "You need Rust" and `cargo install --path .`; slice 02 already turned `factory docs init` into `factory init` there.
- **Deliberate non-changes** — *(prd §14 "Windows. The daemon, the hook and the symlinks are Unix; a Windows build is a port with its own PRD." §10 FR-8: "The tag is the owner's." Do not add Windows targets, and do not tag, push a tag or create a release.)*

## Tasks

### 1. cargo-dist writes the release workflow — high — blocked by: —
**Source**: §10 FR-8: "`cargo-dist` (`dist init`) generates `.github/workflows/release.yml` on `v*` tags for `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`, `x86_64-apple-darwin` and `aarch64-apple-darwin`: tarballs, checksums and `factory-installer.sh` on a GitHub release." "Verify at slice entry how `cargo-dist` builds `aarch64-unknown-linux-gnu` (native runner or cross)". Acceptance: "the builder runs `dist plan` once at slice time (installing `cargo-dist` locally) and pastes its output into the slice's Progress."

Install `cargo-dist` 0.32.0 (`cargo install cargo-dist --version 0.32.0 --locked`). Run `dist init` non-interactively with exactly the four targets, the `shell` installer, GitHub CI and the `github` host; set `install-updater = false`, since `factory upgrade` is the updater. Run `dist plan` and paste it into Progress.

Then answer the two open facts in Progress: how the workflow builds `aarch64-unknown-linux-gnu` (a native ARM runner, or cross), and `cargo tree -i aws-lc-sys` and `cargo tree -i openssl-sys` after task 2. A cross build plus `aws-lc-sys` is the known way the first tag goes red. If so, prefer a native `ubuntu-24.04-arm` runner in the dist config over changing the TLS stack.

The trap: `dist init` also edits `Cargo.toml` (a `[profile.dist]`) and may add or rewrite `ci.yml`. Keep `ci.yml` as it is; it is the gate. And commit `release.yml` and any `dist-workspace.toml` by name: the run's commit takes the paths a worker wrote with Edit or Write, not what a tool generated.

**Acceptance:** `tests/release.rs` reads `.github/workflows/release.yml` and the dist config (`[workspace.metadata.dist]` in `Cargo.toml`, or `dist-workspace.toml`) and asserts: the four target triples and no `windows`; installers `["shell"]`; the workflow triggers on tags matching `v*` or the cargo-dist equivalent pattern. If dist wrote `dist-workspace.toml`, Progress says so and the PRD's FR-8 bullet gets a dated correction.

### 2. `factory upgrade` — high — blocked by: —
**Source**: §10 FR-8: "`factory upgrade` uses the `axoupdater` library: reads the install receipt the installer wrote under `~/.config/factory/`, fetches the latest release, replaces the binary, prints `0.1.0 → 0.2.0` or `already current`; `--check` prints without installing. A binary with no receipt (a `cargo install` build) prints `installed from source — cargo install --path <source>` using `[source] path`, or the installer URL when there is none."

`Cargo.toml`: `axoupdater = { version = "0.10", features = ["blocking"] }`. `src/main.rs`: `/// Replace this binary with the latest release` `Upgrade(UpgradeArgs)` with `--check`. `src/cli/upgrade.rs` (new). `tests/cli.rs:12-14`: add `"upgrade"`.

- `new_for("factory")`, then `load_receipt()`. An error, or `check_receipt_is_for_this_executable()` false, means "not installed from a release": print `installed from source — cargo install --path <path>` when `[source] path` is set, else `installed from source — curl --proto '=https' --tlsv1.2 -LsSf https://github.com/emilianotisato/factory/releases/latest/download/factory-installer.sh | sh`. Exit 0.
- `set_current_version(CARGO_PKG_VERSION)`: the running binary is the truth, not the receipt's `version`.
- `--check`: a current-thread tokio runtime `block_on(query_new_version())`. Newer → `<current> → <new>`; else `already current`. Exit 0. Network errors → `factory: <error>`, exit 1.
- Without `--check`: `disable_installer_output()`, `run_sync()`. `Some(result)` → `<old> → <new>`; `None` → `already current`.
- `pub fn from_release() -> bool` and `pub fn source_line(cfg) -> String` in the same module, for task 3.

The trap: `query_new_version` has no sync variant. A `#[tokio::main]` would make every command async; build a `tokio::runtime::Builder::new_current_thread().enable_all()` runtime locally, as `is_update_needed_sync` does inside the crate.

**Acceptance:** `tests/upgrade.rs`, through the binary, every case with `AXOUPDATER_CONFIG_PATH` set to a temp dir:

- **No receipt**, `[source] path = "/src/factory"` in a temp `FACTORY_CONFIG_DIR`: stdout `installed from source — cargo install --path /src/factory`, exit 0. With no config: the installer line, exit 0.
- **Receipt for this binary** (`install_prefix` = the canonical dir of `env!("CARGO_BIN_EXE_factory")`, `source` = `github`/`emilianotisato`/`factory`/`factory`), and a `TcpListener` on `127.0.0.1:0` serving `/api/v3/repos/emilianotisato/factory/releases/latest` with `tag_name: "v999.0.0"`, `prerelease: false` and an asset named `factory-installer.sh`, via `FACTORY_INSTALLER_GHE_BASE_URL`: `factory upgrade --check` prints `<CARGO_PKG_VERSION> → 999.0.0`, exit 0, and the binary is untouched (same mtime).
- Same, serving `v<CARGO_PKG_VERSION>`: `already current`.
- **Receipt for another prefix** (`/nonexistent`): the from-source line.

The plain `factory upgrade` install path is not tested: it would replace the test binary. The owner's qa-guide row covers it.

### 3. Doctor's `ahead` hint follows the rule — high — blocked by: 2
**Source**: §5 FR-3: "`ahead (repo 0.3.0, binary 0.2.0) → factory upgrade` (or the `cargo install --path <source>` line when the binary was not installed from a release, FR-8)". §10 FR-8: "Doctor's `ahead` hint (FR-3) uses the same rule." §6 FR-4: "`ahead` → exit 1 with the `factory upgrade` hint."

`src/skills.rs` `verdict`: the `ahead` line ends `→ factory upgrade` when `upgrade::from_release()`, else `→ cargo install --path <source>` or `→ <the installer line>`. `factory skills update` on `ahead` prints the same hint. Both load the user config through `config::load(&paths::config_dir())` for `[source]`.

The trap: from here on, any test that reaches the `ahead` verdict reads a receipt, and without `AXOUPDATER_CONFIG_PATH` that is the owner's real `~/.config/factory/factory-receipt.json`. Set `AXOUPDATER_CONFIG_PATH` to an empty temp dir in `tests/harness/mod.rs` `build_env` and in every `tests/skills_fence.rs` and `tests/upgrade.rs` command. Then a test binary is always "from source".

**Acceptance:** `tests/skills_fence.rs`: the `ahead` fixture's doctor line becomes `ahead (repo 999.0.0, binary <v>) → cargo install --path <p>` with `[source] path = <p>` in the test's config dir, and ends with the installer line without one; `skills update` on `ahead` prints the same hint and exits 1. The test passes with and without a real receipt in `~/.config/factory/`.

### 4. README: the installer first — medium — blocked by: 1
**Source**: §10 FR-8: "README `## Trying it` gets the installer line first and `cargo install` second."

`README.md` `## Trying it` (`:64-75`): "You need the `claude` CLI logged in and `gh` authenticated." Then the installer line, then "or, from a checkout, with Rust: `cargo install --path .`", then `factory init` and the gate line as slice 02 left them. One sentence after it: `factory upgrade` updates an installed binary.

**Acceptance:** `tests/release.rs` asserts the installer URL appears in `README.md` before `cargo install --path .`.

### 5. Verify — medium — blocked by: 1-4
`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 6. Prove — at the checkpoint — blocked by: 5
Rows from [`prd.md`](prd.md) §12 this slice proves, below the UI: the **FR-8 machine row**, run `factory upgrade --check` against a stub release; `cargo test` finds the four targets in the dist metadata and the release workflow. Also run the fresh binary's `factory upgrade` with no receipt on this machine and keep its from-source line. The tag, the green release, the installer on a second machine and `upgrade --check` saying `already current` are the owner's FR-8 human row, after the merge, in `qa-guide.md`.

**Acceptance:** after the checkpoint, Progress holds the `dist plan` output, the aarch64 and TLS answers, the `tests/release.rs` and `tests/upgrade.rs` transcript, and the no-receipt line.
