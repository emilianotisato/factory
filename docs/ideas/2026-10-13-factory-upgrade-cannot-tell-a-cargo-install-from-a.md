---
title: "factory upgrade cannot tell a cargo install from a release install in ~/.cargo/bin"
state: "open"
added: "2026-10-13"
origin: "PRD-004 polish"
---

**Found by:** PRD-004 polish, local review and the second opinion independently (2026-10-13).

`dist-workspace.toml` sets `install-path = "CARGO_HOME"`, so the release installer writes `~/.cargo/bin/factory` and a receipt whose `install_prefix` is `~/.cargo`. `cargo install --path .` writes the same file. `axoupdater` 0.10.2's `check_receipt_is_for_this_executable` (`receipt.rs:97-113`) drops the trailing `bin` and compares directories, so once the installer has run on a machine, a later source build still "matches" the receipt.

What that breaks (`src/cli/upgrade.rs` `updater`/`from_release`):
- FR-8's clarification promises "a `cargo install` binary beside a release install prints the source line". With one shared path there is no "beside": `factory upgrade` treats the source build as a release install, and doctor's `ahead` hint says `factory upgrade` instead of `cargo install --path <source>`.
- No downgrade happens: `upgrade` sets the running binary's version (`set_current_version`), so it replaces the build only with a strictly newer release. The cost is a misleading hint, and a dev build silently replaced by a newer release.

Options:
1. Treat the receipt as ours only when its `version` equals `CARGO_PKG_VERSION` (read `factory-receipt.json` ourselves; `axoupdater` has no getter). A same-version source build is then indistinguishable, but harmless.
2. Release into its own dir (`install-path = "~/.factory/bin"`), which changes PATH setup for users.
3. Accept and document in README `## Trying it`.
