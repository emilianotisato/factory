---
lane: 2
env: [CARGO_TARGET_DIR=target/lane-2]
setup: 
---

This lane has its own cargo build output, `target/lane-2`. Siblings never block
on its lock and never invalidate its incremental cache. No setup: every test in
this crate runs in its own temp dir and sets `FACTORY_DATA_DIR` and
`FACTORY_CONFIG_DIR` itself.
