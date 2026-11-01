---
lane: 1
env: [CARGO_TARGET_DIR=target/lane-1]
setup: if [ -d "$CARGO_TARGET_DIR" ] && [ "$(du -sm "$CARGO_TARGET_DIR" | cut -f1)" -gt 15000 ]; then rm -rf "$CARGO_TARGET_DIR"; fi
---

This lane has its own cargo build output, `target/lane-1`. Siblings never block
on its lock and never invalidate its incremental cache. Nothing else trims it, so
the setup wipes it at run start once it passes 15 GB: one cold build beats a
lane worktree that grows past 50 GB. Every test in this crate runs in its own
temp dir and sets `FACTORY_DATA_DIR` and `FACTORY_CONFIG_DIR` itself.
