# Slice 03 — Doctor reads the manifest, and skills update

**Source**: [`prd.md`](prd.md) → §5 FR-3 (the comparison, doctor's line, preflight's judge line), §6 FR-4 (`factory skills update`), and §4 FR-2's last acceptance bullet (`source checkout`). **Depends on**: 02 (it reads the manifest `init` writes, and its fixtures are repos `factory init` built; `skills::render` and `skills::source_checkout` are slice 02's).
**Created**: 2026-10-13

**Progress**: _(written as it lands — what was built, the gate numbers, what a review caught)_

**Slice**: After this slice a repo can tell whether its skills match the binary. `factory docs doctor` prints a `skills` line after `gate`: `current`, `none`, `behind`, `ahead`, `differs`, plus `hand-edited` and `missing` when files moved, or `source checkout` in this repo. `factory docs preflight` repeats any verdict but `current` and `source checkout` as a judge line and keeps its exit code. `factory skills update` re-renders a `behind`, `differs` or `missing` repo, deletes what the new bundle dropped, prints the commit line and commits nothing; it refuses `ahead` and `hand-edited` with exit 1. The `ahead` hint here is `factory upgrade` alone; slice 06 adds the from-source rule.

**Highest risk: two run tests count preflight's judge lines exactly** (`tests/run.rs:362-363`, `tests/loop.rs:596-597`: `k[..4]` are `judge_noted` and `k[4]` is `run_started`). A skills line that lands in `judge` for a healthy fixture makes five, and both go red.

## Current code / environment

- **Doctor**: `src/docs/doctor.rs:59-102` `pub fn run`. `Framework::V3` pushes `line("gate", …)` (`:70`), then `drift` (`:71-77`); `line(key, value)` pads the key to 15 columns (`:53-55`). `tests/docs_check.rs:67-70` asserts the v3 output with `starts_with("Framework      v3\n")` and `contains` for `gate` and `drift`, so one more line breaks nothing. `doctor_in` (`:49-60`) asserts doctor leaves `git status` unchanged: the verdict must only read.
- **Preflight**: `src/docs/preflight.rs:98-102` `pub fn run(root, reference)` collects `refuse`, `ask`, `judge`, `ok`; the exit code is `1` on any refuse, `2` on any ask, else `0` (`:229-235`), so a judge line never changes it. `pub fn cli` (`:366-399`) prints `✓` ok, `✗` refuse, `!` ask, `?` judge. The run reads the same `preflight::run` through `src/docs/mod.rs:91` and notes each judge line as a `judge_noted` event, never waiting on it (`tests/run.rs:351`).
- **Judge-line counts in tests**: `tests/run.rs:362-363` and `tests/loop.rs:596-597` assert exactly four `judge_noted` events before `run_started`. Every v3 fixture is built by `factory docs init` (`tests/harness/mod.rs:190`), so after slice 02 its skills are `current`. `tests/docs_lifecycle.rs:82` and `tests/docs.rs:40` assert `judge` is non-empty, and `:181` asserts `working tree clean` is in `ok`.
- **The CLI**: `src/main.rs:19-50` `enum Command`, dispatch `:54-66`, exit code through `u8::try_from(code)` (`:68`). Commands live in `src/cli/*.rs` (`src/cli/mod.rs:4-12`) and take the root from `paths::repo_root` (`src/cli/docs.rs:220`). `tests/cli.rs:12-14` `SUBCOMMANDS` holds the verbs `--help` must list.
- **Versions**: `Cargo.toml:3` is `0.1.0`. There is no semver crate in `[dependencies]`; slice 06 adds `axoupdater`, which re-exports one. The manifest's `version` is written from `env!("CARGO_PKG_VERSION")` by slice 02.
- **Deliberate non-changes** — *(prd §14 "Not handled: `factory docs check` warning on stale skills. … `check` is the pre-commit hook and a behind repo must still commit." §14 "Not handled: a run refusing or auto-updating on stale skills. … a judge line is the signal". §14 "Not handled: `update --force`." §14 "Not handled: `update --dry`." §14 "Not handled: `init` and `update` committing what they wrote." Do not add a `W-SKILLS-*` code, a refuse or ask line, a flag, or a commit.)*

## Tasks

### 1. The verdict — high — blocked by: —
**Source**: §5 FR-3: "The comparison, in order: no manifest → `none → factory init`. Repo version < binary version → `behind (repo 0.1.0, binary 0.2.0) → factory skills update`. Repo version > binary version → `ahead (repo 0.3.0, binary 0.2.0) → factory upgrade` … Same version, different bundle hash → `differs (0.2.0 both, bundle differs: a dev build) → factory skills update`. Then, whatever the version verdict, any file whose on-disk hash differs from the manifest → `hand-edited: polish/SKILL.md, …`, and any manifest file that is missing → `missing: …`. The version line and the hand-edit line are both printed when both apply." "The healthy line is `current (0.2.0)`." §4 FR-2: "`doctor` prints `skills: source checkout` when `.claude/skills/prd` resolves inside `skills/`."

`src/skills.rs`: `pub fn verdict(root) -> Verdict` and `Verdict::lines() -> Vec<String>`.

- `source_checkout(root)` first → `source checkout`, nothing else.
- No `.claude/skills/factory.toml` → `none → factory init`.
- Otherwise parse the manifest and compare its `version` with `CARGO_PKG_VERSION` as three integers (`major.minor.patch`), not as strings: `"0.10.0" < "0.9.0"` as text. Then the bundle hash on equal versions.
- Then every `[files]` entry: absent on disk → `missing`; hash different → `hand-edited`. Sorted, comma-joined, relative to `.claude/skills/`.
- `current (<version>)` is printed only when no other line applies. A repo at the right version with one edited file prints `hand-edited: polish/SKILL.md` alone.

A manifest that does not parse, or whose version is not three integers, prints `unreadable .claude/skills/factory.toml: <error>` rather than a guessed verdict. That is the error path, not a new state.

**Acceptance:** unit tests in `src/skills.rs` on a temp dir rendered by `skills::render`: one per verdict, each asserting `lines()` exactly, including a `behind` repo with a hand edit (two lines, version first) and `0.10.0` against `0.9.0` comparing as greater.

### 2. Doctor's `skills` line — high — blocked by: 1
**Source**: §5 FR-3: "Doctor prints it". "Doctor's `skills` line is the only new output." Acceptance: "Seven fixtures, one per verdict …, each producing exactly the line above from `factory docs doctor`".

`src/docs/doctor.rs` `run`, in `Framework::V3`, right after the `gate` push (`:70`): one `line("skills", l)` per `verdict(root).lines()`. V3 only: a v2 repo's first line already says to migrate.

**Acceptance:** `tests/skills_fence.rs`, through the binary, on temp repos built by `factory init`: the seven fixtures print exactly these, each on a line starting `skills` padded to 15 columns:

| Fixture | How the test makes it | Line |
|---|---|---|
| current | `init` alone | `current (<CARGO_PKG_VERSION>)` |
| none | delete the manifest | `none → factory init` |
| behind | manifest `version = "0.0.0"` | `behind (repo 0.0.0, binary <v>) → factory skills update` |
| ahead | manifest `version = "999.0.0"` | `ahead (repo 999.0.0, binary <v>) → factory upgrade` |
| differs | manifest `bundle` set to 64 zeros | `differs (<v> both, bundle differs: a dev build) → factory skills update` |
| hand-edited | append a byte to `polish/SKILL.md` | `hand-edited: polish/SKILL.md` |
| missing | delete `ux/SKILL.md` | `missing: ux/SKILL.md` |

Build the versions from `env!("CARGO_PKG_VERSION")`, never a literal: slice 07 bumps it. A repo whose `.claude/skills/prd` links to `../../skills/prd` prints `source checkout`. Doctor leaves `git status` unchanged on each, as `doctor_in` checks today.

### 3. Preflight repeats it — high — blocked by: 1
**Source**: §5 FR-3: "preflight repeats it as a judge line and goes on." Acceptance: "`factory docs preflight` printing the same text as a judge line with its exit code unchanged". Clarified at slicing, 2026-10-13 (written into FR-3): `current` and `source checkout` go to `ok`; every other verdict line goes to `judge`.

`src/docs/preflight.rs` `run`, in the v3 arm next to `docs check` (`:107-123`): push `format!("skills {l}")` for each verdict line, to `ok` when the verdict is `current` or `source checkout`, else to `judge`.

The trap: a `current` line in `judge` would be a fifth `judge_noted` on every healthy fixture, and `tests/run.rs:362-363` and `tests/loop.rs:596-597` assert four. And a manifest edited in a test leaves the tree dirty, which preflight judges on its own (`tests/docs_lifecycle.rs:161-181`), so commit the fixture's edit before calling preflight.

**Acceptance:** `tests/skills_fence.rs` on the harness fixture (`tests/harness/mod.rs`, one feature): `factory docs preflight 001 --json` has `skills current (<v>)` in `ok` and no `skills` line in `judge`. After committing a `behind` manifest, `judge` holds `skills behind (repo 0.0.0, binary <v>) → factory skills update` and the exit code equals the clean fixture's. `tests/run.rs` and `tests/loop.rs` stay green unchanged.

### 4. `factory skills update` — high — blocked by: 1
**Source**: §6 FR-4: "`behind` or `differs` → re-render. Every bundle file is written, files the manifest lists but the bundle no longer holds are deleted (a renamed reference, a retired skill, with its `.agents` symlink), the manifest is rewritten, and the commit line `chore: skills <version>` is printed; nothing is committed". "`ahead` → exit 1 with the `factory upgrade` hint." "`hand-edited` → exit 1 listing the files. The way through is git: `git checkout -- .claude/skills/<file>` restores the manifest's text, then `update` runs." "`current` → `already current`, exit 0." Clarified at slicing, 2026-10-13 (written into FR-4): `missing` re-renders like `behind`; `none` exits 1 with the `factory init` hint; a source checkout exits 1, since its skills are read live from `skills/`.

`src/main.rs`: `/// Bring this repo's skills to the binary's bundle` `Skills { #[command(subcommand)] verb: SkillsVerb }` with one verb `Update`. `src/cli/skills.rs` (new, listed in `src/cli/mod.rs`). `tests/cli.rs:12-14`: add `"skills"`.

The order: `none`, source checkout, `ahead`, `hand-edited` (any version) each exit 1 and write nothing. `hand-edited` prints the files and one `git checkout -- .claude/skills/<file>` line per file. Else `behind`, `differs` or `missing` → `skills::render(root)`, then delete every old-manifest `[files]` path the new manifest lacks and every old `[links]` path the new one lacks, then remove directories left empty under `.claude/skills/`. Print what changed and `commit: git add .claude/skills .agents/skills && git commit -m "chore: skills <version>"`. Else `already current`, exit 0.

The trap: `render` rewrites the manifest, so read the old one before calling it, or the dropped files are invisible. And `git add` of a deleted path must still be in the printed line: `git add .claude/skills .agents/skills` stages deletions under both dirs, so name the dirs, not the files.

**Acceptance:** `tests/skills_fence.rs`:

- On `behind` with an extra old-manifest entry `old/SKILL.md` (written on disk with its hash) and `[links]` entry `.agents/skills/old`: exit 0, every bundle file equals the embedded text, the manifest's `version` is `<v>`, `old/` and `.agents/skills/old` are gone, stdout holds the commit line, and `git log` gains nothing.
- On `ahead`: exit 1, stdout names `factory upgrade`, and a snapshot of `.claude/skills` and `.agents/skills` is unchanged.
- On `hand-edited`: exit 1, the `git checkout -- .claude/skills/polish/SKILL.md` line, nothing changed; after running that checkout, `update` on the same repo prints `already current`.
- On `missing`: `ux/SKILL.md` is back. On `current`: `already current`, exit 0. On `none` and on a source checkout: exit 1, nothing written.

### 5. Verify — medium — blocked by: 1-4
`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 6. Prove — at the checkpoint — blocked by: 5
Rows from [`prd.md`](prd.md) §12 this slice proves, below the UI:

- **FR-3**: run `factory docs doctor` on each of the seven fixtures; `factory docs preflight` shows the judge line. Drive it with the real binary in a scratch repo: `factory init`, commit, then each edit from task 2's table, `factory docs doctor` after each, and `factory docs preflight 001` on the `behind` one.
- **FR-4**: run `factory skills update` on `behind` (renders, prints the commit line), `ahead` and `hand-edited` (exit 1, nothing changed), a dropped file is gone.
- **FR-2, last bullet**: `factory docs doctor` in this repo prints `skills` and `source checkout`.

**Acceptance:** after the checkpoint, Progress holds the seven doctor lines, the preflight output with its exit code, the three `update` transcripts with exit codes and `git status --porcelain` after each, and this repo's doctor output.
