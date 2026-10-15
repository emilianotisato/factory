# factory

The `factory` CLI, built and dogfooded in this repo. `.claude/skills/` links into `skills/`, so a skill edit here is live at once; releasing it takes a version bump, a tag and `factory skills update`.

- **Tests read fixtures, never this repo's live `docs/`.** A test that counts `docs/ideas` or the index goes red on any docs commit.
- **After a merge, run `cargo install --path . --locked`** before the next `factory run`. A lane runs the installed binary, so a stale one polishes without the newest verbs.

# Product Docs Convention

- **`docs/index.md` is the canonical map and feature lifecycle.** Read it before touching anything in `docs/`. A feature is `docs/features/{NNN}-{slug}/`: `prd.md`, `slice-NN-*.md` when big, and a `state.toml` only the `factory` binary writes.
- **PRD number `{NNN}` is identity, never reused. Priority is not the number**: a higher number can ship first, and the queue lives in `docs/index.md`.
- **Never edit `docs/archive/**`**: frozen history, move-only.
- **`docs/features` and `docs/archive` are history**: the code, `docs/reference` and `docs/decisions.md` win.
- **Slice, don't fatten** one doc past an agent's context budget. Loose ideas go one file each in `docs/ideas/` (`factory docs idea add`).
- **The gate is `gate` under `[docs]` in `docs/project.toml`** and `factory docs gate` prints it. It is the one command that proves a change is done. Every slice's Verify task runs it.
- **The skills** `/prd`, `/slice`, `/deliver`, `/polish`, `/gate`, `/harness`, `/harvest`, `/prune` and `/ux` live in `.claude/skills/`, written by `factory init`. `factory docs doctor` says whether they are current.
