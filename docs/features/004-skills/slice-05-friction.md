# Slice 05 — Closeout files the retro's friction as an idea

**Source**: [`prd.md`](prd.md) → §9 FR-7. **Depends on**: — (independent: it touches closeout, the user config and the stub `claude`, none of which slices 01-04 change; its only shared lines are the `TOP_KEYS` entry slice 04 also edits).
**Created**: 2026-10-13

**Progress**: _(written as it lands — what was built, the gate numbers, what a review caught)_

**Slice**: After this slice a run's retro friction reaches the skills repo without a copy-paste step. Right after the polish commit, closeout reads the retro's `## Framework friction → skills repo` section. When it holds more than the template's placeholder and `[source] path` in `~/.config/factory/config.toml` names a checkout with `docs/ideas/`, closeout writes one idea there: `friction: <repo> PRD-NNN`, the section verbatim and the retro's path. Nothing is committed, and the run prints the idea's path. With no `[source]`, a placeholder-only section, or any failure, nothing is filed, nothing is printed, and the run goes on. Slice 06 reads the same `[source] path` for its from-source upgrade line.

## Current code / environment

- **Where the hook goes**: `src/closeout.rs:163-169` `fn polish(run, docs, cfg: &Config, board, out)`; on the success path `:325` calls `commit_polish(run, board, row_id, &exit.summary, &template, &before, out)?`, then emits `PolishDone` (`:326-329`) and returns `Ok(None)` (`:330`). `cfg` is already in scope, so the source path needs no new plumbing.
- **The retro**: `commit_polish` (`:343-400`) computes `retro_rel = format!("{}/retro.md", board.dir)` (`:354`) under `run.state().repo`, and writes it from the template when the worker left none (`:356-364`). `run.state().feature` is the number; `run.id()` is the run id.
- **The template's section**: `assets/retro.md:33-35`, the heading `## Framework friction → skills repo` and the placeholder line `A step, rule or template in the framework itself. Name the skill and the section.`, then `## Score` (`:37`).
- **Writing an idea in process**: `src/docs/ideas.rs:186` `pub fn add(root, idea: &Idea) -> Result<String>` writes `docs/ideas/<added>-<slug>.md` with `create_new`, suffixing `-2`, `-3` when the name is taken, and never commits. `Idea` (`:25-33`) is `{ title, state, added, origin, retrigger, priority, body }`; `STATES` are `open` and `owed` (`:20`). `slug` (`:73-90`) lowercases and kebabs, so `friction: arca-api PRD-012` becomes `friction-arca-api-prd-012`. `clock::today()` (`src/clock.rs:59`) gives the date.
- **Output**: `scheduler::say(out, line)` (`src/scheduler.rs:1729`).
- **What later commits sweep**: `ship` commits only the feature's own dir (`src/closeout.rs:600`, `commit::expand(&repo, [board.dir])`). A CI-fix round snapshots the tree before its worker (`:1591`, `scheduler::snapshot`), so an idea written after polish is in the baseline and its pathspec commit leaves it out. A run in this repo therefore leaves the idea untracked, as FR-7 says.
- **Config**: `src/config.rs:75` already has `pub enum Source` (where a setting came from), so the new table's struct needs another name. `TOP_KEYS` (`:103`) lists the user file's top-level keys and warns on others; slice 04 adds `"review"`.
- **The stub polish worker**: `tests/stubs/claude:294-350`. Its retro is a fixed string at `:340`, `# Retro — PRD-NNN`, `## Cost vs catch`, `## Score`, with no friction section. Steps take `touch`, `skip`, `slices`, `subagents` and `outcome` (`:287-292`).
- **Closeout tests**: `tests/closeout.rs` builds plans with `plan_with` (`:205`), `done()` (`:186`) and `step()` (`:190`), starts a run with `start_with(fx, env)` (`:326`) and waits with `wait_phase` (`:133`). The fixture's config dir is `<base>/config` (`FACTORY_CONFIG_DIR`, `tests/harness/mod.rs:517-563`).
- **Deliberate non-changes** — *(prd §14 "Not handled: a `friction unfiled` line with the issues URL. Cut 2026-10-13: one machine, with `[source]` set." §14 "Not handled: `gh issue create` for friction. … private project names in a public issue." §14 "The learn loop. Friction ideas are its raw material; ranking them is a later PRD." §9: "Never committed by the run." Do not print a line when nothing is filed, call `gh`, rank or dedupe ideas, or commit the idea.)*

## Tasks

### 1. `[source] path` in the user config — high — blocked by: —
**Source**: §9 FR-7: "when it holds more than the template's placeholder and `[source] path` in `config.toml` names a checkout with `docs/ideas/`". §10 FR-8 reads the same key for its from-source line.

`src/config.rs`: `pub source: Option<SourceRepo>` on `Config`, `SourceRepo { path: PathBuf }`, `#[serde(default)]`. `TOP_KEYS` gains `"source"`. A leading `~/` expands against `paths::home()` when the path is read.

**Acceptance:** unit tests: no `[source]` → `None`; `[source] path = "/x"` → `/x`; `"~/p"` → `<home>/p`; no unknown-key warning for `[source]`.

### 2. Read the section and file the idea — high — blocked by: 1
**Source**: §9 FR-7: "After the polish commit, closeout reads the section; when it holds more than the template's placeholder and `[source] path` in `config.toml` names a checkout with `docs/ideas/`, it writes an idea there in process (`idea add` with root = the source path): title `friction: <repo> PRD-NNN`, origin `<repo> PRD-NNN run <id>`, body the section verbatim plus the retro's path." "Never committed by the run. … When the source checkout is the run's own repo (a run here), the file is written after the polish commit and left untracked for the owner, listed in the run's output." "No source path, or the write fails → nothing is filed and the run goes on".

`src/closeout.rs`: a `fn file_friction(run, board, cfg, out)` called right after `commit_polish` at `:325`, returning nothing.

- The section is the text after the line starting `## Framework friction` up to the next line starting `## `, trimmed. Empty, or equal to the template's placeholder line, → return.
- `cfg.source` unset, or `<path>/docs/ideas/` not a directory → return.
- `ideas::add(<path>, &Idea { title: "friction: <repo> PRD-NNN", state: "open", added: clock::today(), origin: Some("<repo> PRD-NNN run <id>"), body: "<section>\n\nFrom <repo dir>/<retro_rel>\n", .. })`, where `<repo>` is the run repo's directory name.
- On `Ok(id)`, `say(out, "friction: filed <path>/docs/ideas/<id>.md")`. Any error → return silently.

The trap: `file_friction` must never return an error into `polish`. A `?` inside it would turn an unwritable source dir into a failed closeout, which is the opposite of "the run goes on". Match on every fallible call.

**Acceptance:** unit tests on the section reader: the template's own text → nothing; a two-bullet section → the two bullets; a section that is last in the file → read to the end.

### 3. The stub writes a friction section — high — blocked by: —
**Source**: §9 FR-7 Acceptance: "The stub harness's polish scenario writes a retro with a two-bullet friction section". §12 Seed state: "a temp v3 repo as the `[source]` for FR-7".

`tests/stubs/claude:340`: when `step.friction` is a string, the retro gets `## Framework friction → skills repo\n\n<step.friction>\n\n` before `## Score`. Document the option in the comment at `:287-292`.

**Acceptance:** the closeout tests below see the section in the committed `retro.md`.

### 4. The closeout scenarios — high — blocked by: 2, 3
**Source**: §9 FR-7 Acceptance: "with `[source] path` pointing at a temp v3 repo, closeout leaves `docs/ideas/<date>-friction-<repo>-prd-NNN.md` there with the two bullets, origin set, uncommitted; with no `[source]`, no file is written and the run output carries no friction line. A retro with only the template placeholder files nothing."

`tests/closeout.rs`, three tests on the stub harness, each running a plan through polish with `step.friction` set on the polish step:

- **With a source**: a second temp dir, `git init` and `factory docs init`, named in `<base>/config/config.toml` as `[source] path`. After the run, `<source>/docs/ideas/<today>-friction-<fixture repo name>-prd-001.md` exists, holds both bullets, the origin line `<repo> PRD-001 run <id>` and the retro's path; `git -C <source> status --porcelain` lists it untracked; the run output has `friction: filed`.
- **No source**: same plan, no config file. No file under any `docs/ideas/` besides the README, and no `friction:` in the output.
- **Placeholder only**: `step.friction` is the template's placeholder line, with a source set. Nothing filed.

The trap: the fixture repo's own `docs/ideas/` exists too. Point `[source]` at the second dir, and assert the fixture repo gained no idea file, so a wrong root is caught.

**Acceptance:** the three tests pass, and the existing `tests/closeout.rs` tests pass unchanged.

### 5. Verify — medium — blocked by: 1-4
`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 6. Prove — at the checkpoint — blocked by: 5
Rows from [`prd.md`](prd.md) §12 this slice proves, below the UI: the **FR-7 row**, run the stub polish scenario with a friction section and a temp `[source]`: the idea file lands uncommitted; without `[source]`, no file and no line. The checkpoint runs the three task-4 tests by name with `--nocapture` and keeps the transcript; the with-source test prints the idea file's text and the source repo's `git status --porcelain` before its temp dirs go.

**Acceptance:** after the checkpoint, Progress holds the test transcript, the idea file's text, and the porcelain line showing it untracked.
