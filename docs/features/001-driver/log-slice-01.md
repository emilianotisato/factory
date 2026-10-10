## Traps

- `tests/harness.rs` next to `tests/harness/mod.rs` is a rustc E0761 ambiguity; every test file includes the shared module with `#[path = "harness/mod.rs"] mod harness;` instead of `mod harness;`.
- `docs.mjs` resolves a `./gate.sh` gate by checking the file exists, so the fixture writes `gate.sh` before `docs new`; the other way round the gate reads as missing and preflight refuses.
- `docs preflight` refuses a repo with no `origin` remote (`docs.mjs:1731`); a fixture needs the bare `origin.git` the builder adds, or every preflight test fails on the wrong thing.
- `cargo test` swallows the timing line `tests/harness.rs` prints; `cargo test --test harness -- --nocapture` shows it.

## Blockers

_(none)_

## Decisions

### AD-101 — Does the fixture repo get a remote? · slice-01 · 2026-10-09

**Chose:** a bare `origin.git` beside the repo in the same temp dir, added as `origin`
**Over:** no remote, which `docs preflight` refuses (`docs.mjs:1731`), or a fake URL, which later slices' pushes could not reach
**Because:** FR-10 asks for a fixture preflight has nothing to raise on, and §12 names only the index keys; a local bare remote also gives slice 09 somewhere to push without network
**Undo:** two lines in `tests/harness/mod.rs`
**Review:** no

### AD-102 — Where do the gate scenario and its log live so a run never dirties the tree? · slice-01 · 2026-10-09

**Chose:** `scenario/` at the repo root, listed in the fixture's `.gitignore`
**Over:** a directory outside the repo passed by env, which `gate.sh` would have to read on every call
**Because:** FR-10 says the gate "reads a scenario file" and preflight refuses a dirty tree (`docs.mjs:1726`); ignoring the directory keeps both true
**Undo:** the `.gitignore` line and the path in `gate.sh`
**Review:** no

### AD-103 — Does the slice write a CI workflow when autonomy.md forbids it? · slice-01 · 2026-10-09

**Chose:** write `.github/workflows/ci.yml` as task 7 orders: ubuntu, Node 20, stable Rust with rustfmt and clippy, the gate string verbatim from `docs/index.md`
**Over:** leaving CI to the owner, which would leave §16 FR-10 without its named evidence (a CI run) and FR-6 with nothing to watch
**Because:** autonomy.md lists CI workflow files under Never, and says the PRD outranks that list for a file it names, with the entry marked `decide`
**Undo:** delete the file
**Review:** decide

### AD-104 — How do the test binaries share the fixture code? · slice-01 · 2026-10-09

**Chose:** `tests/harness/mod.rs` included by `#[path]` from each `tests/*.rs`, with `#![allow(dead_code)]` since no single test uses every helper
**Over:** a `[[test]]` with `harness = false`, or moving the fixture into `src/` behind a feature flag, which would ship test code in the binary
**Because:** the slice names `tests/harness/mod.rs` and `tests/harness.rs` without saying how they coexist (E0761)
**Undo:** rename the smoke test file
**Review:** no

### AD-105 — What does the stub claude replay? · slice-01 · 2026-10-09

**Chose:** `tests/stubs/stream.json`, four events cut from `recordings/claude-p-oneshot-2.1.280.jsonl` (init, one assistant, one user, result) with `{{PLACEHOLDERS}}` the stub fills per spawn; owner paths, plugins, skills, memory paths, MCP servers and hook events removed, usage numbers kept
**Over:** reading the recording at run time, which carries the owner's machine into every stream and would need the same stripping on every spawn
**Because:** FR-10 wants streams "cut from real recordings" and §19 wants nothing owner-specific outside `config.toml`
**Undo:** regenerate from the recording; the cut is a 20-line node script
**Review:** no

### AD-106 — What status does the fixture PRD carry? · slice-01 · 2026-10-09

**Chose:** `status: ready`, the prose Status line to match, `**Harness**: none`, and one `run` row per slice replacing the scaffold's `FR-1` placeholder
**Over:** leaving the scaffold's `draft`, which preflight accepts but which the driver's own preflight (slice 02) may not
**Because:** the slice asks for a QA plan "so `W-NO-QA-PLAN` does not fire", and that warning only checks ready or in-progress PRDs
**Undo:** three branches in `Fixture::patch_prd`
**Review:** no
