# Product Docs Convention

- **`docs/index.md` is the canonical map and feature lifecycle.** Read it before touching anything in `docs/`. A feature is `docs/features/{NNN}-{slug}/`: `prd.md`, `slice-NN-*.md` when big, and a `state.toml` only the `factory` binary writes.
- **PRD number `{NNN}` is identity, never reused. Priority is not the number**: a higher number can ship first, and the queue lives in `docs/index.md`.
- **Never edit `docs/archive/**`**: frozen history, move-only.
- **Slice, don't fatten** one doc past an agent's context budget. Loose ideas go one file each in `docs/ideas/` (`factory docs idea add`).
- **The gate is `gate` under `[docs]` in `docs/project.toml`** and `factory docs gate` prints it. It is the one command that proves a change is done. Every slice's Verify task runs it.
- **The skills** `/prd`, `/slice`, `/deliver`, `/polish`, `/gate`, `/harness`, `/prune` and `/ux` live in `.claude/skills/`, written by `factory init`. `factory docs doctor` says whether they are current.
