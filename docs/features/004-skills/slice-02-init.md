# Slice 02 — factory init renders the family

**Source**: [`prd.md`](prd.md) → §4 FR-2 (every decision and acceptance bullet but the last, which is slice 03's doctor line), §5 FR-3 decision "Manifest at `.claude/skills/factory.toml`" (the writer; the reader is slice 03). **Depends on**: 01 (it renders `skills::FILES` and hashes with `skills::file_hash` and `skills::bundle_hash`; and the source-checkout guard reads the links slice 01 committed).
**Created**: 2026-10-13

**Progress**: _(written as it lands — what was built, the gate numbers, what a review caught)_

**Slice**: After this slice `factory init` (and its alias `factory docs init`) sets a repo up in one command: the v3 docs layout as today, the nine skill folders written byte-identical into `.claude/skills/`, eight relative `.agents/skills/<name>` links for Codex, a manifest with the crate version and a sha256 per file, and the convention block in `CLAUDE.md` and `AGENTS.md`. It prints what it wrote, the one commit line, and the setup order, and commits nothing. A second `init` writes only what is missing and leaves an existing skills install to `update`. Reading the manifest (doctor, preflight, `skills update`) is slice 03's.

**Highest risk: every harness fixture runs `docs init`** (`tests/harness/mod.rs:190`), so from this slice on every fixture repo carries the skills, the links and both convention files in its base commit. The whole suite is this slice's test, not only `tests/init.rs`.

## Current code / environment

- **`init` today**: `src/docs/scaffold.rs:47-95` `pub fn init(root, out) -> Result<()>` writes `docs/project.toml`, `docs/index.md`, `docs/{features,ideas,archive}/`, `docs/ideas/README.md` and the `docs/.gitignore` line, each only when missing, collecting names in `made`. Nothing made → `say(out, "already initialised")` (`:88`). Else `created <list>` and `next: declare the verify gate under [docs] in docs/project.toml (...), or run /gate` (`:93`). No test asserts the `next:` line (grep, 2026-10-13).
- **Routing**: `src/cli/docs.rs:219-247` `route()`. `migrate`/`doctor` or a v3 repo → `native()`; a v2 repo (`docs/index.md` without `[docs]`) → `exec` of `docs.mjs`; no board and verb `init` → `native()` (`:240`). `native()` dispatches `Verb::Init => scaffold::init(root, out)` (`:345`). The root is `paths::repo_root(".")`.
- **The CLI**: `src/main.rs:19-50` `enum Command` (`Run … Docs { args } … Daemon`), dispatched at `:54-66`; `Command::Docs { args } => cli::docs::docs(&args, &mut stdout(), &mut stderr())` (`:65`). `tests/cli.rs:12-14` `SUBCOMMANDS: [&str; 9]` is checked against `--help`.
- **Other `init` callers**: `tests/harness/mod.rs:190` (`build_v3`, then `git add -A` and `commit` at `:221-222`), `tests/docs_v3.rs:203`, `:231` and `:243`, `tests/docs_ideas.rs:350` and `:354`, `src/docs/check.rs:879` (in-process). `tests/docs_v3.rs:217-234` snapshots three `docs/` files across a second init and asserts `already initialised`: still true after this slice.
- **The gate hint scan reads `CLAUDE.md` and `AGENTS.md`**: `src/docs/gate.rs:304-316`, through `verify_hint` (`:261-271`), which flags any line where the word "verify" sits between the line's first and last backtick. The convention block goes into exactly those files, so a block line with "verify" inside a backtick span becomes a bogus gate candidate in every repo without a gate.
- **The v2 block**, Dropbox `014e28f^:feature-docs/SKILL.md:317-326`: heading `# Product Docs Convention`, five bullets (index.md is the map, number is identity and priority is not, never edit `docs/archive/**`, slice instead of fattening and loose ideas in `docs/ideas.md`, the gate is `gate:` in `docs/index.md` frontmatter). Two bullets are v2-wrong today.
- **`skills/README.md`** keeps `ln -s "$(pwd)/skills/deliver" ~/.claude/skills/deliver` from slice 01, asserted by `tests/cockpit.rs:582-586`. **`README.md:64-77`** `## Trying it` runs `cargo install --path .` then `factory docs init`.
- **This repo is a source checkout** after slice 01: `.claude/skills/prd` → `../../skills/prd`, no manifest. FR-2 clarified at slicing: "`init` on a source checkout (`.claude/skills/prd` resolves inside `<root>/skills/`) leaves the skills alone".
- **Deliberate non-changes** — *(prd §4: "It does not commit." and §14 "Not handled: `init` and `update` committing what they wrote. Cut 2026-10-13 … the commit is the caller's". §14 "Not handled: `init` refusing over a project skill with a family name. Cut 2026-10-13: … `init` renders over it and git shows the replacement." §14 "Not handled: `init --no-skills`." §14 "Not handled: a user-scope install. … outside an initialised repo `/prd` does not exist". §4 "Byte-identical, never rendered through a template. No overlays". Do not add a commit, a collision refusal, a flag, a `~/.claude/skills` write or a template step.)*

## Tasks

### 1. `factory init`, the alias that routes as `docs init` does — high — blocked by: —
**Source**: §4 FR-2: "Verb: `factory init`, with `factory docs init` as its alias. Both do the whole thing … Every existing pointer to `factory docs init` stays valid." Clarified: "`factory init` routes exactly as `factory docs init` does (`src/cli/docs.rs:220-245`): a v2 repo still reaches `docs.mjs`".

`src/main.rs`: a `/// Set a repo up: the docs layout, the skill family, the convention block` `Init` variant, dispatched as `cli::docs::docs(&["init".to_string()], &mut stdout(), &mut stderr())`, so there is one code path. `tests/cli.rs:12-14`: add `"init"` (the array becomes `[&str; 10]`).

**Acceptance:** `factory --help` lists `init` (the existing `help_lists_every_subcommand`); in `tests/init.rs`, `factory init` and `factory docs init` on two empty repos produce the same file tree and the same stdout.

### 2. Render the bundle and write the manifest — high — blocked by: —
**Source**: §4 FR-2: "the files go to `.claude/skills/<name>/…` once, and `.agents/skills/<name>` is a relative symlink `../../.claude/skills/<name>`, committed." "Byte-identical, never rendered through a template." §5 FR-3: "Manifest at `.claude/skills/factory.toml`: `version = "<crate version>"`, `bundle = "<sha256 of the embedded bundle>"`, and a `[files]` table `"<relative path>" = "<sha256>"` for every rendered file (symlinks listed under `[links]` with their targets)."

`src/skills.rs`:

- `pub struct Manifest { version: String, bundle: String, files: BTreeMap<String, String>, links: BTreeMap<String, String> }` with serde, read and written through `toml`. `files` keys are relative to `.claude/skills/` (`polish/SKILL.md`); `links` keys are repo-relative (`.agents/skills/polish`) with the target as value (`../../.claude/skills/polish`). BTreeMap keeps the file stable across renders. `MANIFEST: &str = ".claude/skills/factory.toml"`.
- `pub fn render(root) -> Result<Rendered>`: write every `FILES` entry to `.claude/skills/<path>` (create parents, replace what is there), then each `NAMES` link at `.agents/skills/<name>`, then the manifest with `version = env!("CARGO_PKG_VERSION")` and `bundle = bundle_hash()`. `Rendered` lists the files and links written, for the output.
- `pub fn source_checkout(root) -> bool`: `.claude/skills/prd` canonicalises to a path under `root/skills/` canonicalised. Slice 03's doctor uses the same function.

The traps:

- **A symlinked skill folder writes through.** A repo whose `.claude/skills/polish` is a link (to Dropbox, say) would have the bundle written into the link's target. Before writing under `.claude/skills/<name>`, if that path is a symlink, remove the link and create a real directory. `symlink_metadata`, not `metadata`.
- **An existing `.agents/skills/<name>`**: a link is removed and recreated; a file is removed; a real directory is removed with `remove_dir_all`. FR-2 renders over whatever is there and git shows it (non-goal above). `std::os::unix::fs::symlink`; Unix only, as the daemon already is (`src/daemon.rs:8`).

**Acceptance:** `tests/init.rs` on an empty git repo: every `FILES` entry exists under `.claude/skills/` and its sha256 equals `skills::file_hash` of the embedded text; `.agents/skills/<name>` for each of the eight is a symlink whose `read_link` is `../../.claude/skills/<name>` and that resolves to a directory holding `SKILL.md`; there is no `.agents/skills/_shared`; the manifest parses, `version` is `CARGO_PKG_VERSION`, `bundle` equals `skills::bundle_hash()`, `files` has 19 entries matching the on-disk hashes, `links` has 8. A pre-existing `.claude/skills/polish/SKILL.md` holding `mine\n` is replaced by the bundle's text and recorded in the manifest. A pre-existing `.claude/skills/ux` symlink to a temp dir outside the repo: after `init` it is a real directory and the temp dir is untouched.

### 3. The convention block — high — blocked by: —
**Source**: §4 FR-2: "Convention block. Appended to `CLAUDE.md` and `AGENTS.md`, created if absent; when one is a symlink to the other, or their contents are identical, written once and mirrored. Idempotent on its heading `# Product Docs Convention`. Its text is the v2 block corrected for v3: the gate is `gate` under `[docs]` in `docs/project.toml`, ideas are one file each under `docs/ideas/`, and one line names the eight skills. Kept to about ten lines".

`assets/docs/convention.md` (new, `include_str!` beside the other `scaffold.rs` assets at `:16-20`), this text:

```markdown
# Product Docs Convention

- **`docs/index.md` is the canonical map and feature lifecycle.** Read it before touching anything in `docs/`. A feature is `docs/features/{NNN}-{slug}/`: `prd.md`, `slice-NN-*.md` when big, and a `state.toml` only the `factory` binary writes.
- **PRD number `{NNN}` is identity, never reused. Priority is not the number**: a higher number can ship first, and the queue lives in `docs/index.md`.
- **Never edit `docs/archive/**`**: frozen history, move-only.
- **Slice, don't fatten** one doc past an agent's context budget. Loose ideas go one file each in `docs/ideas/` (`factory docs idea add`).
- **The gate is `gate` under `[docs]` in `docs/project.toml`** and `factory docs gate` prints it. It is the one command that proves a change is done. Every slice's Verify task runs it.
- **The skills** `/prd`, `/slice`, `/deliver`, `/polish`, `/gate`, `/harness`, `/prune` and `/ux` live in `.claude/skills/`, written by `factory init`. `factory docs doctor` says whether they are current.
```

In `scaffold::init`: for `CLAUDE.md` and `AGENTS.md`, skip a file whose text already holds a line equal to `# Product Docs Convention`. Otherwise append the block (a blank line first when the file is not empty), creating the file when absent. Dedupe the pair before writing: when one path is a symlink resolving to the other (canonicalise both), write the target once; when both exist with identical contents, append to both, so they stay identical.

The trap: `fs::OpenOptions::append` on `CLAUDE.md` follows a symlink, so appending to both names of a linked pair writes the block twice into one file. And keep the word "verify" out of every backtick span (`src/docs/gate.rs:261-271`).

**Acceptance:** `tests/init.rs`: on an empty repo both files hold the block exactly once; a second `init` leaves both byte-identical; `CLAUDE.md` as a symlink to `AGENTS.md` (which holds `# mine\n`) → `AGENTS.md` is `# mine\n`, a blank line and the block, once, and `CLAUDE.md` is still a link; two identical pre-existing files both gain the block once; `factory docs gate` on an initialised repo with no gate declared lists no `CLAUDE.md:` or `AGENTS.md:` hint.

### 4. What `init` writes the second time, and the source checkout — high — blocked by: 2, 3
**Source**: §4 FR-2, clarified at slicing: "`init` writes whatever is missing … the convention block when its heading is absent, and the skills when `.claude/skills/factory.toml` is absent. With a manifest present the skills are `update`'s job (FR-4), so `init` leaves `.claude/skills/` and `.agents/skills/` alone. Nothing written → `already initialised`, exit 0." Clarified at slicing (FR-2): "`init` on a source checkout … leaves the skills alone, since this repo has no manifest."

`src/docs/scaffold.rs` `init`: after the layout, call `skills::render(root)` only when `.claude/skills/factory.toml` is absent and `skills::source_checkout(root)` is false; then the convention block. Every write adds to `made`; nothing made → `already initialised`, as at `:88`.

**Acceptance:** `tests/init.rs`: a second `factory init` prints `already initialised`, exits 0, and a snapshot of every file and link under the repo (excluding `.git`) is unchanged; with the manifest present and `.claude/skills/prd/SKILL.md` edited by hand, `init` leaves the edit; a repo whose `.claude/skills/prd` is a link to `../../skills/prd` with a real `skills/prd/SKILL.md` gets no manifest, no `.agents/skills/` and no rendered files from `init`.

### 5. The output: what was written, the commit line, the setup order — high — blocked by: 4
**Source**: §4 FR-2: "`init` ends by printing the paths it wrote and the one line to commit them (`git add … && git commit -m "chore: factory init (skills <version>)"`)." "Setup order printed last: gate (`factory docs gate set "<cmd>"` or `/gate`), harness (`/harness`), UX rubric (`/ux rubric`)". Acceptance: "prints the commit line, committing nothing".

`src/docs/scaffold.rs` `init`, replacing the `next:` line at `:91-94`:

```
created docs/project.toml, docs/index.md, docs/features/, docs/ideas/, docs/archive/, docs/ideas/README.md, docs/.gitignore
skills 0.1.0: .claude/skills/ (19 files, factory.toml), .agents/skills/ (8 links)
convention block: CLAUDE.md, AGENTS.md
commit: git add docs .claude/skills .agents/skills CLAUDE.md AGENTS.md && git commit -m "chore: factory init (skills 0.1.0)"
next, in order: the gate (factory docs gate set "<cmd>", or /gate), the harness (/harness), the UX rubric (/ux rubric)
```

Each line appears only when its part was written, and `git add` names only the top-level paths written this time. The version is `CARGO_PKG_VERSION`; the counts come from `Rendered`.

**Acceptance:** `tests/init.rs` on an empty repo: stdout holds the `commit:` line with exactly the five paths and the version, and the `next, in order:` line; afterwards `git rev-parse HEAD` still fails (nothing committed) and `git status --porcelain` lists the five paths untracked. Running the printed `git add … && git commit …` line commits them all with that subject.

### 6. The install line says `factory init` — medium — blocked by: 1
**Source**: §4 FR-2: "Verb: `factory init`". §1 Improvements: "Another developer, or a second machine, gets the family by installing the binary, not by cloning a Dropbox folder … and symlinking it."

`skills/README.md`: replace the `ln -s` install paragraph with "`factory init` in a repo writes these into `.claude/skills/`", and one sentence on this repo's own links (a session here reads `skills/` live). `tests/cockpit.rs:582-586`: assert `factory init` in the README instead of the `ln -s` line. `README.md` `## Trying it` (`:64-77`): `factory init` replaces `factory docs init`, its comment naming the skills and the convention block. Slice 06 adds the installer line above `cargo install`.

**Acceptance:** `tests/cockpit.rs` passes with the new assertion; `grep -n "docs init" README.md` finds no line in `## Trying it`.

### 7. Verify — medium — blocked by: 1-6
`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` _(the inner loop; the full gate runs at this slice's checkpoint)_

The trap: the harness fixtures now carry about 170 KB of skills each (`tests/harness/mod.rs:190`). A test that walks the fixture tree, counts untracked files, or matches `CLAUDE.md` may go red here for a reason that is not a bug in `init`. Fix the test's assumption, and write each such fix in Progress.

**Acceptance:** inner loop green, one commit.

### 8. Prove — at the checkpoint — blocked by: 7
Rows from [`prd.md`](prd.md) §12 this slice proves, below the UI: the **FR-2 machine row**: run `factory init` on a temp repo (layout, nine skill folders, eight links, manifest, block in both files, the commit line printed and nothing committed), run it twice, run it over a pre-existing `polish`. Drive it with the real binary in a scratch repo, not only through `cargo test`: keep the stdout of both runs, `ls -la .agents/skills`, `git status --porcelain`, and the manifest's first lines. The FR-2 human row (Claude Code, `cursor-agent` and `codex` list `/prd` in `arca-api`, Cursor once) is the owner's, after the merge, in `qa-guide.md`. FR-2's last bullet (`skills: source checkout`) is slice 03's.

**Acceptance:** after the checkpoint, Progress holds both stdouts, the link listing, the porcelain output, and the replaced `polish/SKILL.md` hash equal to the manifest's.
