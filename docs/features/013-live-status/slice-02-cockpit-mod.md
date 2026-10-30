# Slice 02 — The cockpit mod and its gate

**Source**: [`prd.md`](prd.md) → §6 FR-4, §8 FR-6. **Depends on**: — (the mod reads the envelope §3 defines, from fixture JSON in its own tests, so it does not wait for slice 01's code)
**Created**: 2026-10-30

**Highest risk in this PRD**: the first mod in this repo, on a young API. Read the types before writing a line; the 2.1.312 types file is the spec, not this slice.

**Progress**: _(written as it lands — what was built, the gate numbers, what a review caught)_

**Slice**: Adds the Claude Code mod at `mods/cockpit/` and puts it under the gate. In a cockpit session it polls `factory status --here --json` every 10 s and draws the status line, the band when the owner is needed, and the `/factory-board` pane. In a worker it does nothing. The gate gains `claude plugin validate` and `claude plugin test`, CI installs a pinned `claude`, and a cargo test pins the mod's version to the crate's. Embedding the mod in the binary and installing it is slice 03's.

## Current code / environment

- **No mod exists**: there is no `mods/` folder. The plugin is `factory-cockpit`. Its files are `.claude-plugin/plugin.json`, `hooks/hooks.json` (`{ "modules": ["./register.tsx"] }`), `hooks/register.tsx`, its `*.test.tsx`, and `types/index.d.ts` when it keeps `$.state` (prd §6). The pane's open flag and the last JSON are read while drawing, so expect `$.state` atoms and the contract file.
- **The API**: Claude Code 2.1.312. `$.clock.every`, `$.process.run(argv, init)`, `$.env.get(name)`, `$.ui.status(text)` (undefined clears), a `ui.render` hook on `{ component: 'AbovePrompt' }`, `$.ui.open({ id, title })` with a `ui.render` hook on `{ component: 'Pane', requestId }`, and `$.command.register` in `session.start` answered by `command.run`. Grep the types file for each (`'process.run'`, `Pane: {`, `AbovePrompt`). The engine writes `.claude-plugin/types/` into a loaded mod; that folder is not source, so gitignore it.
- **Testing a mod**: `claude plugin test <dir>` runs every `*.test.ts(x)`, importing `test`, `expect`, `mock` from `claude-code/testing`. Nothing sits beneath the plugin: the test answers each `$` call with a hook (`on('process.run', () => ({ value: … }))`, `on('env.get', …)`), and the clock is mocked, so a test advances it to fire the poll.
- **Workers**: they run `claude -p` with `FACTORY_REPO` set (`src/worker.rs:259`), in lane worktrees. A user-scope plugin loads there too (prd §2), so the guard is the whole defence.
- **The JSON it reads**: prd §3. `{ schema: 1, factory: "<version>", runs: [{ id, feature, phase, line, owner: [], board: [] }] }`. An unknown `schema` counts as stale (prd §6).
- **The gate**: `docs/project.toml:3` is the cargo gate alone; `inner` (`:4`) stays cargo only, because §8 names the gate.
- **CI**: `.github/workflows/ci.yml:20` runs the cargo gate. Node 20 is already set up (`:13-15`), so `npm install -g @anthropic-ai/claude-code@2.1.312` is the install step, pinned at the version the probe ran (prd §8). CI runs on a push to `main` and on pull requests only.
- **The version**: `Cargo.toml:3` is `0.11.0`. The precedent for a test reading this repo's own source files is `tests/skills_bundle.rs:9-10` (`CARGO_MANIFEST_DIR`).
- **Deliberate non-changes**: no native notifications (prd §12: "Not handled: native notifications from the mod"). No poll-interval knob and no slower poll with no live run (§12). No `factory docs doctor` line for the mod (§12). The `/deliver` skill is unchanged (§12).

## Tasks

### 1. The mod: guard, poll and status line — high — blocked by: —
**Source**: §6 FR-4: "Idle in workers. `$.env.get("FACTORY_REPO")` is set → the mod registers nothing that polls or draws." "Poll. Every 10 s, `factory status --here --json`, run in the session's directory." "Status line. Every run's `line`, joined with `  |  `, through `$.ui.status`. With `runs` empty it is cleared (`undefined`), unless the mod is stale." The stale and failure bullets.

`mods/cockpit/`. The mod's own version is one constant in `register.tsx`, equal to `Cargo.toml`'s. Stale means the JSON's `factory` differs from it, or `schema` is not 1. Stale adds `· mod stale: factory cockpit install, then /reload-plugins`; stale with no run shows `factory: mod stale: factory cockpit install, then /reload-plugins`. Failure (`factory` missing, a non-zero exit, unparsable JSON) shows `factory: ` plus the first line of the error, and the next poll tries again.

**Acceptance:** `claude plugin test mods/cockpit` passes tests that assert: with `FACTORY_REPO` set, no `process.run` call is ever made; a two-run fixture sets the status line to both lines joined by `  |  `; empty `runs` clears it; a different `factory` version adds the stale suffix; empty `runs` with a different version shows the suffix alone; a non-zero exit shows `factory: ` and the error's first line.

### 2. The band and the pane — high — blocked by: 1
**Source**: §6 FR-4: "Band above the prompt. Drawn only while some run's `owner` is non-empty, one row per `owner` string, each prefixed with the feature number. Otherwise the hook answers `next(e)` and draws nothing." "Pane. `/factory-board` toggles a pane titled `factory` that shows every run's `board`, one run after another. With no live run it says `no live run in this repo`."

`mods/cockpit/hooks/register.tsx`, `mods/cockpit/types/index.d.ts`. Follow the shapes of the plugin-authoring examples `band.tsx` and `pane.tsx`.

**Acceptance:** tests assert that a fixture with one BLOCK in `owner` draws exactly one band row starting with the feature number, and that an empty `owner` draws none. `/factory-board` opens the pane, which shows the fixture's board lines, and a second `/factory-board` closes it. With no run the pane says `no live run in this repo`.

### 3. The gate, CI and the version pin — high — blocked by: 1
**Source**: §8 FR-6: "The gate in `docs/project.toml` gains `claude plugin validate mods/cockpit && claude plugin test mods/cockpit`. CI gains a step that installs the `claude` CLI at a pinned version, before the gate step. A cargo test pins `mods/cockpit/.claude-plugin/plugin.json`'s `version` and the mod's own version constant to `CARGO_PKG_VERSION`."

`docs/project.toml` (the `gate` line), `.github/workflows/ci.yml` (an install step after Node, and the gate step updated to the new gate verbatim), and a new cargo test under `tests/`.

**The trap**: the version constant lives in TypeScript, so the cargo test reads `register.tsx` as text. Pin it with one exact, greppable declaration, and make the test fail when that declaration is missing, not only when it differs.

**Acceptance:** `the_mod_carries_the_crates_version` fails when `plugin.json`'s `version` or the constant differs from `CARGO_PKG_VERSION`, and when the constant is missing. `factory docs gate` prints the new gate. With `register.tsx` deliberately broken, the gate exits non-zero; the transcript shows it.

### 4. Verify — medium — blocked by: 2, 3
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q` _(the inner loop; the full gate runs at this slice's checkpoint)_. Also run `claude plugin validate mods/cockpit && claude plugin test mods/cockpit`; the inner loop does not cover the mod.

**Acceptance:** inner loop and both `claude plugin` commands green, one commit.

### 5. Prove — at the checkpoint — blocked by: 4
§10 QA plan rows:

- FR-4: run `claude plugin test mods/cockpit`; keep the transcript.
- FR-6: run the gate with a broken `register.tsx` and see it red; keep the gate transcript. The CI half ("read the PR's CI log for the two `claude plugin` steps") exists only once the PR's CI has run, so it is read at polish, not at this checkpoint.
- FR-4, Human: the owner's look in a live cockpit. It needs slice 03's install, so it is the owner's, at review (§10 "Who builds it").

**Acceptance:** after the checkpoint, the FR-4 test transcript and the red-gate transcript are in this slice's Progress.
