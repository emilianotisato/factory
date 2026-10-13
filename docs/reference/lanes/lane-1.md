---
lane: 1
env: [CARGO_TARGET_DIR=target/lane-1]
setup: 
---

This lane has its own cargo build output, `target/lane-1`. Siblings never block
on its lock and never invalidate its incremental cache. No setup: every test in
this crate runs in its own temp dir and sets `FACTORY_DATA_DIR` and
`FACTORY_CONFIG_DIR` itself.

**What this lane does not isolate.** The source tree. All lanes edit the same
working tree and `cargo test` compiles the whole crate, so a sibling's
half-written file can fail your build or tests in code your slice does not
touch. When that happens, do not fix the sibling's file: wait a minute and run
again. Touch only the files your slice names. `Cargo.toml` and `Cargo.lock`
are shared too; add a dependency only when your slice requires it, and commit
nothing yourself, the driver commits by pathspec.
