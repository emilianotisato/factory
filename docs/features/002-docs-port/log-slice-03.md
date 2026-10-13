## Decisions

### AD-301 — A live feature with no prd.md or no state.toml · slice-03 · 2026-10-13
**Chose:** a folder under `features/` with no `prd.md` keeps v2's `E-NO-PRD`. One with no `state.toml` is `E-STATE-INVALID`, and the message names `factory docs set NNN status=draft`.
**Over:** dropping both cases silently, or adding a new code for the missing state file.
**Because:** the slice lists the codes for a file that does not parse, and is silent on a file that is absent. Both are errors in v2, and a v3 feature without state has no status at all.
**Undo:** two arms in `src/docs/check.rs` `feature`.
**Review:** fyi

### AD-302 — check reads features/ and archive/ itself · slice-03 · 2026-10-13
**Chose:** `board::scan_folder` is now `pub`, and `check::run` calls it once per folder instead of calling `board::scan`.
**Over:** `board::scan`, which the slice names.
**Because:** `board::scan` fails on a `project.toml` that does not parse, which would hide every other finding. The slice's trap asks check to keep going.
**Undo:** make `scan_folder` private again and catch `scan`'s error.
**Review:** no

### AD-303 — A v3 repo whose project.toml does not parse never reaches native check · slice-03 · 2026-10-13
**Chose:** routing and `doctor::detect` stay on `project::is_v3`, as slice 01 and the slice spec state. `E-PROJECT-INVALID` for a parse failure is unit-tested. Through the CLI, only the type errors in `[docs]` reach it.
**Over:** routing on "docs/project.toml exists", which would send a broken v3 file to native check.
**Because:** routing is slice 01's decision, and the slice names `is_v3` for detect. Today a broken `project.toml` next to a `docs/index.md` goes to `docs.mjs`, and doctor reports `v1` or `v2`.
**Undo:** test `project::path(root).is_file()` in `route` and `detect` instead of `is_v3`.
**Review:** fyi

### AD-304 — doctor's drift line in a v2 or v1 repo · slice-03 · 2026-10-13
**Chose:** `clean` when `docs.mjs check` says `docs check: clean`. Otherwise it shows its `N errors, M warnings` line, or `docs.mjs check exited N` when it printed none, and the tool's other lines follow. When there is no `docs.mjs`, it shows `unknown: no docs.mjs: …`.
**Over:** passing the tool's output through without a drift line.
**Because:** the slice asks for the same `drift` line in all three formats. Slice 11 rewrites the skills against these lines.
**Undo:** `src/docs/doctor.rs` `run`, the `V2 | V1` arm.
**Review:** no

### AD-305 — HEAD does not compile without earlier slices' uncommitted files · slice-03 · 2026-10-13
**Chose:** leave them as they are, and name them here for the driver.
**Over:** staging files this slice did not write.
**Because:** HEAD (4730e99) holds both `src/docs.rs` and `src/docs/mod.rs`. The working tree stages the deletion of `src/docs.rs`. These are slice 01's and are not in HEAD:
- `src/cli/mod.rs` (`pub mod docs`)
- `src/main.rs` (the `Docs` command)
- `src/config.rs` (`docs` in `PROJECT_TOP_KEYS`)
- `tests/fixture/stub-docs/docs.mjs`
- `tests/snapshots/docs-log/show.md`

The inner loop is green only with all of them. A slice-03 commit without them fails to build.
**Undo:** commit them with this slice, or in a fix commit before it.
**Review:** decide

## Traps

- `scaffold::new` allocates past every number in `features/` and `archive/`. A test that writes `docs/archive/005-old` and then calls `new` gets 006, not 002. To get a known folder, write it by hand.

## Untracked leftovers

- src/cli/docs.rs
- src/docs/mod.rs
