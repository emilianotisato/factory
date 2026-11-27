---
title: "lanes: isolate CARGO_TARGET_DIR per lane so parallel builds do not overwrite test binaries"
state: "open"
added: "2026-11-27"
origin: "harvest 2026-11-27"
---

In PRD-029 (slice-03 log note), parallel lanes shared a single `CARGO_TARGET_DIR` (`/mnt/devcache/.../cargo-target`), so another lane's `cargo build` replaced `debug/factory` while an integration test was shelling out to the binary, causing spurious test failures.

**Shape:** give each lane its own `CARGO_TARGET_DIR` (or per-lane bin output) in `docs/reference/lanes/lane-N.md` or rebuild `cargo build --bin factory` inside lane-isolated target dirs so concurrent lanes never race on the compiled binary.
