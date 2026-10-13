# Slice 01 — The family lives here, embedded

**Source**: [`prd.md`](prd.md) → §3 FR-1 (every decision and the three acceptance bullets except the `models.md` trim, which is slice 04's), §4 FR-2 decision "This repo is the one exception" (the source-checkout links only; the detector is slice 03's). **Depends on**: — (the spine: every other slice but 05 reads the bundle this slice builds).
**Created**: 2026-10-13

**Progress**:

- **Built.** `skills/` now holds the seven Dropbox folders from `git archive 33796be` plus `_shared/models.md` (`git show 33796be:feature-docs/models.md`, `cmp` identical). `src/skills.rs` holds `NAMES`, `FILES` (19 entries, byte-sorted, so `_shared/…` first and `SKILL.md` before `references/`), `file_hash`, `hash_of` and `bundle_hash`; the hash definition is in `hash_of`'s doc comment. `tests/skills_bundle.rs` walks `skills/` and checks the nine `.claude/skills/` links. Nine relative symlinks under `.claude/skills/`. `README.md:36` and `:60` and `skills/README.md` rewritten; the `ln -s` install line is kept for `tests/cockpit.rs`.
- **Task 1 evidence.** `grep -rn feature-docs skills/` prints nothing. `diff -r "$P" skills/ -x deliver -x _shared -x README.md` against the `33796be` archive shows exactly eight changed line pairs, each a link rewrite:
  - `harness/references/maintain.md` `19c19`: target `../../feature-docs/models.md` → `../../_shared/models.md`, text `feature-docs/models.md` → `_shared/models.md`
  - `polish/SKILL.md` `17c17`, `52c52`; `prd/SKILL.md` `15c15`, `99c99`; `slice/SKILL.md` `17c17`; `ux/SKILL.md` `18c18`: target `../feature-docs/models.md` → `../_shared/models.md`, text `feature-docs/models.md` → `_shared/models.md`
  - `polish/SKILL.md` `60c60`: target `../feature-docs/models.md` → `../_shared/models.md`, text `models.md` unchanged
- **Inner loop.** 2 runs, 1 red, about 2 min. The red was `cargo fmt --check` on the two new files; `cargo fmt` fixed it. The green run: fmt, clippy `-D warnings`, `cargo test` all passing (220 lib tests plus every integration binary).
- **A unit test beyond the slice's list:** `a_path_and_content_cannot_trade_bytes` pins the `\0` separators in `hash_of`.
- **Left for the checkpoint (Task 6).** The FR-1 row: the `cargo test --test skills_bundle` transcript, the empty grep, and the red run on a stray `skills/prd/references/x.md`.
- **Checkpoint 1, FR-1 row.** Green: `docs/features/004-skills/evidence/cp-1-fr1-bundle-green.txt` (3 passed). `grep -rn feature-docs skills/` empty: `docs/features/004-skills/evidence/cp-1-fr1-grep.txt`. Red on a stray file, naming `prd/references/x.md`, then removed: `docs/features/004-skills/evidence/cp-1-fr1-bundle-red.txt`.
- **Checkpoint 1, fixed.** Commit 9eed991 carried no `skills/` file but `README.md` and none of the nine `.claude/skills/` links, so HEAD did not compile: `docs/features/004-skills/evidence/cp-1-head-build.txt`. The checkpoint commit carries them; the staged tree builds and passes alone: `docs/features/004-skills/evidence/cp-1-index-build.txt`.

**Slice**: After this slice the eight family skills and `_shared/models.md` live in `skills/`, ported from Dropbox `33796be` with only the eight `models.md` links changed, and the binary carries them as one embedded bundle with a hash. A gate test fails the moment a file under `skills/` is missing from the bundle or differs from it. This repo's `.claude/skills/<name>` are committed relative symlinks into `skills/`. Nothing renders the bundle yet (slice 02), nothing compares it (slice 03), and `models.md` keeps its Cursor block until slice 04 moves it into the binary.

**Highest risk: the port must not change a skill's meaning.** Until the owner deletes the Dropbox folders after the merge, every session on this machine, this feature's own workers included, reads the Dropbox copies (prd §2). A port that drifts from `33796be` ships text nobody reviewed.

## Current code / environment

- **The one family skill already here**: `skills/deliver/SKILL.md`, symlinked out as `~/.claude/skills/deliver`. `skills/README.md` is four lines plus a one-row table (`deliver`) and the install line `ln -s "$(pwd)/skills/deliver" ~/.claude/skills/deliver`. `tests/cockpit.rs:582-586` asserts that exact install line, so **keep it in this slice**; slice 02 replaces it with `factory init` and updates the test. `tests/cockpit.rs:550-580` asserts the deliver skill's content and forbids `/home/`, `Dropbox` and `emiliano` in it. Leave `deliver/SKILL.md` untouched.
- **The Dropbox source**: `~/Dropbox/dev/claude-code-skills` (git, no remote), clean at `33796be`. The seven folders: `prd/SKILL.md`, `slice/SKILL.md`, `polish/SKILL.md`, `gate/{SKILL.md, references/stacks.md, references/tiers.md}`, `harness/{SKILL.md, references/drivers.md, references/feature-map.md, references/maintain.md, references/skill-template.md}`, `prune/{SKILL.md, references/adapters.md, references/rules.md}`, `ux/{SKILL.md, references/inspection.md, references/rubric.md}`: 17 files. Plus `feature-docs/models.md`. With `deliver/SKILL.md` the bundle is **19 files**.
- **The eight links, exactly** (verified 2026-10-13 at `33796be`; `grep -rn feature-docs` over the seven folders finds these and nothing else):
  - `prd/SKILL.md:15` and `:99`, `slice/SKILL.md:17`, `polish/SKILL.md:17` and `:52`, `ux/SKILL.md:18` link the text `feature-docs/models.md` to the target `../feature-docs/models.md`;
  - `polish/SKILL.md:60` links the text `models.md` to the target `../feature-docs/models.md`;
  - `harness/references/maintain.md:19` links the text `feature-docs/models.md` to the target `../../feature-docs/models.md`.
- **The embedding pattern**: `src/pack.rs:23-35`, `pub const X: &str = include_str!("../assets/…");`, one const per asset. `src/docs/scaffold.rs:16-20` does the same. `src/metrics.rs:196-203` has `pub fn sha256_hex(bytes: &[u8]) -> String`; reuse it, `sha2` is already a dependency (`Cargo.toml`).
- **The crate is a lib plus a bin**, both named `factory` (`Cargo.toml:8-13`), so an integration test can `use factory::skills`. Modules are listed in `src/lib.rs:10-37`.
- **This repo has no `.claude/` dir, no `CLAUDE.md` and no `AGENTS.md`.** `.gitignore` is `/target`; `git check-ignore -v .claude/skills/prd` exits 1 (not ignored, checked 2026-10-13).
- **Skill precedence**: "Enterprise over personal, and personal over project" (prd §2, verified at slicing). The links this slice commits are inert on this machine until the owner removes the Dropbox folders. Do not test them by invoking a skill.
- **`README.md:36`** says "installed by `factory` into each project: `/prd`, …, `/ux`, `/lanes`. Project skills are rendered from a profile plus overlays, never copied, and drift is detected by hash. Three scopes with promotion: project, user, base." **`README.md:60`** says "The skills still live outside this repo; moving them in with an installer is PRD-004."
- **Deliberate non-changes** — *(prd §1: "Left alone, deliberately. Stack profiles and overlays …, the learn loop, Windows, `/lanes`, and every non-family skill in Dropbox (`caveman`, `codex`, `self-learning`, …)". prd §3: "`assets/autonomy.md` is the only autonomy copy. The Dropbox `feature-docs/autonomy.md` is not shipped." prd §14: "Stack profiles and overlays … this PRD renders byte-identical files"; "`/lanes`. A README promise; it ships with the isolated-lanes PRD." Do not port `self-learning`, `feature-docs/autonomy.md` or anything else from Dropbox.)*
- **Not this slice's edits**: `models.md`'s `## Second opinion, specifically` block and the `polish`/`prd` second-opinion paragraphs change in slice 04 (FR-6). Port them verbatim here. `skills/bundle.lock` is slice 07's.

## Tasks

### 1. Port the seven folders and `models.md` at `33796be` — high — blocked by: —
**Source**: §3 FR-1: "Members: the eight plus `_shared/models.md`." "Layout: `skills/<name>/…` and `skills/_shared/models.md`. The eight links `../feature-docs/models.md` become `../_shared/models.md` (`../../_shared/models.md` from `harness/references/maintain.md`)." "The port copies all seven folders at Dropbox HEAD `33796be`."

`skills/`. Copy from git, not from the working tree, so the text is exactly the reviewed commit:

```
git -C ~/Dropbox/dev/claude-code-skills archive 33796be prd slice polish gate harness prune ux | tar -x -C skills/
mkdir -p skills/_shared
git -C ~/Dropbox/dev/claude-code-skills show 33796be:feature-docs/models.md > skills/_shared/models.md
```

Then rewrite the eight links and nothing else: `../feature-docs/models.md` → `../_shared/models.md` in the six top-level `SKILL.md` lines, and `../../feature-docs/models.md` → `../../_shared/models.md` in `harness/references/maintain.md:19`. Keep the link text as it is (`feature-docs/models.md` as visible text becomes `_shared/models.md`, so the grep below comes back empty). Leave `models.md` verbatim.

The trap: these files were created by `tar`, not by Edit or Write, so a `factory run` commit by pathspec does not see them (the workers commit the paths their Edit/Write calls named). `git add skills/` by name before the commit.

**Acceptance:** `grep -rn feature-docs skills/` prints nothing. A diff against the archive shows only the eight link lines:

```
P=$(mktemp -d) && git -C ~/Dropbox/dev/claude-code-skills archive 33796be prd slice polish gate harness prune ux | tar -x -C "$P"
diff -r "$P" skills/ -x deliver -x _shared -x README.md
```

prints eight changed line pairs, all link rewrites. Paste both outputs into Progress.

### 2. The embedded bundle — high — blocked by: 1
**Source**: §3 FR-1: "Embedded as a bundle. A const list of `(relative path, content)` for every file under `skills/` … listed by hand … a gate test walks `skills/` and fails when a file on disk is missing from the bundle". Clarified at slicing: "the bundle is every file under the eight skill folders and `skills/_shared/`. `skills/README.md` … and `skills/bundle.lock` … neither is bundled, rendered or hashed; the walk skips both by name."

`src/skills.rs` (new, `pub mod skills;` in `src/lib.rs`):

- `pub const NAMES: [&str; 8] = ["prd", "slice", "deliver", "polish", "gate", "harness", "prune", "ux"];` (the eight that get `.agents/skills` links in slice 02; `_shared` is not a skill).
- `pub const FILES: &[(&str, &str)]`, 19 entries, sorted by path, paths relative to `skills/` with `/` separators: `("_shared/models.md", include_str!("../skills/_shared/models.md"))`, `("deliver/SKILL.md", …)`, … `("ux/references/rubric.md", …)`.
- `pub fn file_hash(content: &str) -> String`, `metrics::sha256_hex` of the bytes.
- `pub fn hash_of(files: &[(&str, &str)]) -> String`: sha256 over the entries sorted by path, each fed as `path`, `\0`, `content`, `\0`. Pure, so a test can hash a modified list.
- `pub fn bundle_hash() -> String`, `hash_of(FILES)`.

Slice 03 compares this hash with a manifest's `bundle`, and slice 07's `skills/bundle.lock` records it, so its definition is fixed here. Write it in the doc comment.

**Acceptance:** `tests/skills_bundle.rs` walks `skills/` recursively from `env!("CARGO_MANIFEST_DIR")`, skipping the root `README.md` and `bundle.lock` by name. It asserts the set of relative paths equals the set in `FILES` (a file on disk and not in the bundle fails, naming it), and each file's bytes equal its entry. Unit tests in `src/skills.rs`: changing one byte of one entry changes `hash_of`; reversing the input order does not; `FILES` is sorted and has no duplicates; every name in `NAMES` has a `<name>/SKILL.md` entry.

### 3. This repo reads the live source — high — blocked by: 1
**Source**: §4 FR-2: "This repo is the one exception. Here `.claude/skills/<name>` are relative symlinks to `../../skills/<name>`, committed by hand, no manifest, so a session in this repo reads the live source and a skill edit is live without a bump."

`.claude/skills/`. Nine links, the eight plus `_shared` (the skills' `../_shared/models.md` links resolve against the path the agent read, so `.claude/skills/_shared` must exist too):

```
mkdir -p .claude/skills
for n in prd slice deliver polish gate harness prune ux _shared; do ln -s "../../skills/$n" ".claude/skills/$n"; done
git add .claude/skills
```

No manifest here, and no `.agents/skills/` links: FR-2 names only `.claude/skills/` for the exception.

The trap: a link created with `ln -s` is invisible to the run's pathspec commit, like task 1's files. `git add .claude/skills` by name. Git stores a symlink as a link, so the commit diff shows `-> ../../skills/prd`, not the file.

**Acceptance:** a test in `tests/skills_bundle.rs` reads `std::fs::read_link` on each of the nine `.claude/skills/<name>` paths and asserts the target is exactly `../../skills/<name>` and that it resolves to a directory holding `SKILL.md` (or `models.md` for `_shared`).

### 4. The READMEs say what is true — medium — blocked by: 1
**Source**: §3 FR-1: "README corrections. `README.md:36` says skills are 'rendered from a profile plus overlays' and lists `/lanes`; after this PRD it says byte-identical rendering with a manifest, and `/lanes` is marked planned. `skills/README.md` lists the eight."

`README.md:36`: keep the first sentence's list, mark `/lanes` as planned, and replace the overlays and scopes sentences with what FR-2 and FR-3 build: each repo gets byte-identical copies in `.claude/skills/` from `factory init`, with a manifest of hashes so `factory docs doctor` says when they are behind, ahead or hand-edited. `README.md:60`: the skills now live in `skills/` and ship inside the binary. Leave `## Trying it` alone; slices 02 and 06 rewrite it.

`skills/README.md`: one row per skill for the eight (what each does, one line, taken from each skill's frontmatter `description`), plus one line on `_shared/models.md` (the model roles the skills share). Keep the `ln -s` install line as it is (`tests/cockpit.rs:582-586`).

**Acceptance:** `tests/cockpit.rs` still passes; `skills/README.md` names all eight with a link to each `SKILL.md`; `README.md` no longer contains "overlays" or "Three scopes".

### 5. Verify — medium — blocked by: 1, 2, 3, 4
`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 6. Prove — at the checkpoint — blocked by: 5
Rows from [`prd.md`](prd.md) §12 this slice proves, below the UI: the **FR-1 row**: run `cargo test --test skills_bundle` and keep its transcript; run `grep -rn feature-docs skills/` (empty); and, to show the test bites, add a stray `skills/prd/references/x.md`, run the test (red, naming the file), delete it. The FR-1 `models.md` bullet is slice 04's proof.

**Acceptance:** after the checkpoint, Progress holds the green transcript, the empty grep, and the red run naming `prd/references/x.md`.
