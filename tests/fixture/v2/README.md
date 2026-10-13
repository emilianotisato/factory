# The v2 fixture (PRD-002 slice 07)

A small v2 repo, made once with the vendored tool in `.docs/` and then edited by hand, for `tests/migrate.rs` and `Fixture::v2`.

Made with `node .docs/bin/docs.mjs` `init`, `new fixture --title Fixture`, `slice 001 a`, `slice 001 b`, `slice 001 c --depends 2`, the harness's index frontmatter and PRD edits, and `render --write`. Then by hand: `docs/project.toml` with `[run]` and `[workers.polish]`, slices 1-2 `done` and slice 3 `building` on lane 1, the PRD `in-progress` with a Notes cell, the `agent-log.md` entries (a trap, a resolved BLOCK, ADs out of order with Owner lines, a preflight AD, a polish AD, a checkpoint) and `retro.md`.

`node .docs/bin/docs.mjs check` must exit 0 on it: the migration test runs it first. Its one warning, `W-RETRO-FM` on `gate_minutes`, is v2 reading `3.25` as text; the decimal is there on purpose, as in real retros.
