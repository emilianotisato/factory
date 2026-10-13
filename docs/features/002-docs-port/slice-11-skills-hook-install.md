---
slice: 11
status: pending
depends_on: [8, 10]
---

# Slice 11 — Skills edit, hook verb, install (FR-17 rest)

**Source**: [`prd.md`](prd.md) → §7 FR-17 (the pre-commit hook body bullet, the skill files bullet, the hook and grep half of the acceptance), §5 FR-15 (the `hook [--here]` verb, the `--help` half of the acceptance), §10 FR-20 (the skills' ideas wording, carried in §7), §14 (the skills edit is live on this machine; `cargo install --path .`), §8 FR-18 (the harness bullet, for the `Fixture::v3` sweep). **Depends on**: 08 (the skills may say `factory docs idea add` only once `migrate` turns a repo's `ideas.md` into files, prd §14 last bullet), 10 (every verb and contract the skills and the hook name exists and is wired; the skills edit goes live on the machine the moment it is written, so it comes last).
**Created**: 2026-10-13 · **Status**: pending

**Progress**: _(written as it lands — what was built, the gate numbers, what a review caught)_

**Slice**: After this slice `factory docs hook [--here]` prints or installs a pre-commit body that calls `factory docs check`. It refuses archive edits, and it exits 0 when `factory` is not on `PATH`. `factory docs --help` lists exactly the eighteen v3 verbs. Every skill file under `~/.claude/skills` that told an agent to run `node …/docs.mjs` says `factory docs` instead, and the FR-20 skill lines name `factory docs idea add` and `new --from-idea`. The branch binary is installed with `cargo install --path .`, so the edited skills resolve on this machine. The test harness has one fixture constructor. The owner commits the skills repo and pastes the hook body into `~/.config/git/hooks/pre-commit` by hand, after the merge (the FR-17 human row, in [`qa-guide.md`](qa-guide.md)).

## Current code / environment

Line numbers verified 2026-10-13. The skills repo is outside this checkout and changes on the owner's hand, so re-verify every skill line at slice entry.

- **The v2 hook verb to port** (`~/.claude/skills/feature-docs/bin/docs.mjs`): `HOOK_MARKER = '# feature-docs hook'` `:1930`. `hookBody(root)` `:1932-1947`: marker, a comment line, `git diff --cached --name-only --diff-filter=M | grep -q "^docs/archive/"` then refuse with "docs: docs/archive/** is frozen — move folders, never edit files inside." and `exit 1`, then `git diff --cached --name-only | grep -q "^docs/"` then `node <bin> check || exit 1`. `hook(root, opts)` `:1949-1975`: "not a git repository" dies. A set `core.hooksPath` without `--here` prints that it is set and that `.git/hooks` is ignored. It then prints "Append this to <hooksPath>/pre-commit", the body, and the `--here` hint, and returns (exit 0). `--here` sets `core.hooksPath .git/hooks` first. It installs at `<git-common-dir>/hooks/pre-commit`, where `--git-common-dir` may be relative (`:1965`). An existing file with the marker prints "hook already installed". An existing file without it dies with "exists — append this by hand", plus the body. A new file gets `#!/bin/sh`, the body, and mode `0755`.
- **The owner's global hook, the model for the v3 body** (`~/.config/git/hooks/pre-commit`, which no worker touches, prd §2): `DOCS_TOOL=…` `:11`. `root=$(git rev-parse --show-toplevel) || exit 0` `:13`. `[ -f "$root/docs/index.md" ] && [ -d "$root/docs/features" ] || exit 0` `:14`. The archive refusal `:16-19`. `[ -f "$DOCS_TOOL" ] || exit 0` `:22`, then `(cd "$root" && node "$DOCS_TOOL" check) || exit 1` `:23`. The v3 body swaps `:22-23` for `command -v factory >/dev/null 2>&1 || exit 0` and `(cd "$root" && factory docs check) || exit 1`.
- **The fixture's git is isolated, so `hook` installs into `.git/hooks` in tests**: `tests/harness/mod.rs:454-455` sets `GIT_CONFIG_GLOBAL` to an empty file and `GIT_CONFIG_NOSYSTEM=1`. So the owner's global `core.hooksPath` never reaches a fixture. **The fixture's `PATH` has no `factory`**: `build_env` (`:420-450`) puts the stubs dir, the dirs of `node`, `git` and `sh`, `/usr/bin` and `/bin` on it. The "factory found" test must prepend `target/debug` (the dir of `env!("CARGO_BIN_EXE_factory")`). The "not on `PATH`" test must check that no `factory` is in the dirs it keeps. This machine has a second `factory` at `~/.local/share/mise/shims/factory`, besides `~/.cargo/bin/factory`.
- **The `--help` test today** (`tests/cli.rs:10-28`) checks the nine top-level subcommands only (`SUBCOMMANDS`, `:12-14`). The native `docs` help is slice 01's (`src/cli/docs.rs`, routing step "no args, `-h` or `--help` → native help"). Slices 02-07 each added a verb, and this slice adds `hook`, the eighteenth.
- **The skill lines, as found** (`command grep -rn 'docs\.mjs' ~/.claude/skills/ --include='*.md'`, minus `feature-docs/bin/`): **26 lines in 10 files**, not the PRD's 23 in 9. `deliver/SKILL.md:25,28,145`. `feature-docs/SKILL.md:20,180,183,326`. `feature-docs/migration.md:121,122`. `feature-docs/upgrades/v2.md:112,113` (not in the PRD's list). `guardrails/SKILL.md:24,133`. `polish/SKILL.md:17,120`. `prd/SKILL.md:20,37,102,164`. `prd-to-tasks/SKILL.md:20,30,124,168,169`. `verify-app/SKILL.md:17`. **`test-cleanup/SKILL.md` names no `docs.mjs`**; the line is `test-cleanup/references/adapters.md:11` ("**First**: `node ~/.claude/skills/feature-docs/bin/docs.mjs gate`"). `feature-docs/SKILL.md:180` is prose about the tool, not a command. `deliver/bin/measure.py` names it twice and stays (PRD-003 FR-25 retired it).
- **The search tool lies in this environment.** In a Claude Code Bash session, `grep` is a shell function that runs `ugrep` with `--ignore-files` (`type grep`). `grep -rn docs.mjs ~/.claude/skills --include=*.md`, the PRD's acceptance spelling, prints **1** line on this machine. `command grep -rn 'docs\.mjs' ~/.claude/skills/ --include='*.md'` prints all 26. Use `command grep` for every count in this slice.
- **Three swapped lines name v2-only verbs**: `prd/SKILL.md:164` and `feature-docs/migration.md:121` run `check --fix`, and `deliver/SKILL.md:145` runs `log merge`. In a v2 repo the shim passes them to node unchanged (prd §7). In a v3 repo `log merge` exits 2 with "removed in v3" (slice 01), and `--fix` is not a v3 flag (prd §5 "Drops… `--fix`").
- **The FR-20 ideas lines, as found**: `polish/SKILL.md:127` ("a line in `docs/ideas.md`") and `:129` ("an `ideas.md` line only"). `prd-to-tasks/SKILL.md:92` ("add an `ideas.md` line"). `feature-docs/autonomy.md:35` (the Park row) and `:43` (the May list, "lines in `docs/ideas.md`"). `prd/template.md:66` ("Moved to [`ideas.md`](../../ideas.md)"). `prd/SKILL.md:56` ("delete the line from `ideas.md`") and `:160` ("remove that line"). `deliver/pr-body.md:39` ("**Parked → `docs/ideas.md`**: {the line}"). **The trap**: `docs.mjs` has no `idea` verb. Its `default:` case (`:2107-2109`) prints the usage and exits 2. So in a v2 repo, `factory docs idea add` routes to node and fails, and a `/polish` Park step there loses the finding.
- **The skills repo has an owner edit in flight**: `git -C ~/.claude/skills status --short` shows ` M polish/SKILL.md` (the `FINDINGS` line at `:199`, PRD-003's) and `?? factory-deliver`. The worker edits `polish/SKILL.md` on top of that change and must not revert or stage it.
- **The edit is live the moment it is written** (prd §14): `~/.claude/skills` is a symlink to `~/Dropbox/dev/claude-code-skills`. The installed `factory` is `~/.cargo/bin/factory` (first on `PATH`). The running driver keeps its open binary. Every worker it spawns after the install resolves the new one: `paths::self_exe` (`src/paths.rs:40-47`) strips the ` (deleted)` suffix `/proc/self/exe` gets once `cargo install` replaces the file.
- **The alias to sweep**: slice 09 made `Fixture::new` build v3 and kept `Fixture::v3` as an alias, and kept `factory_docs` and `factory_docs_json` as aliases of `docs` and `docs_json` (slice-09 task 6). Slices 01-08 wrote their tests on `Fixture::v3`.
- **Deliberate non-changes** — *(prd §2 and §7: "The owner pastes it once: a bad edit there breaks every commit on the machine, so no worker touches it." prd §7: "`feature-docs/bin/docs.mjs` itself is not edited; the shim still execs it for v2 repos. `measure.py` is not edited." "The worker does not commit in that repo; the owner does, once, after the merge." prd §15: the skills' move into this repo and an installer verb are PRD-004. Do not edit `~/.config/git/hooks/pre-commit`, `docs.mjs` or `measure.py`. Do not run `git` in `~/.claude/skills` except `status` and `diff`.)*

## Tasks

### 1. `factory docs hook [--here]` — high — blocked by: —
**Source**: §5 FR-15: "**`hook [--here]`** installs or prints the pre-commit body of FR-17, with today's `core.hooksPath` behaviour (`docs.mjs:1949`)." §7 FR-17: "The pre-commit hook body printed and installed by `factory docs hook` becomes: refuse staged modifications under `docs/archive/`; when anything under `docs/` is staged and `docs/index.md` exists, run `factory docs check`, which routes; if the `factory` binary is not on `PATH`, exit 0 silently, as the owner's hook does today when `DOCS_TOOL` is missing (`pre-commit:22`)… `factory docs hook` prints it when `core.hooksPath` is set, as today."

`src/docs/hook.rs` (new) and one arm in `src/cli/docs.rs`. `pub fn body() -> String`: `# feature-docs hook` (today's marker, so a repo with the v2 hook reads as installed), then `root=$(git rev-parse --show-toplevel 2>/dev/null) || exit 0`. Then the archive refusal with today's message, on `--diff-filter=M`. Then, when `docs/` is staged and `"$root/docs/index.md"` exists: `command -v factory >/dev/null 2>&1 || exit 0` and `(cd "$root" && factory docs check) || exit 1`. `pub fn install(root, here) -> Result<Outcome>` ports `hook()` (`docs.mjs:1949-1975`) line for line: the `core.hooksPath` print and the `--here` hint, `--here` setting `core.hooksPath .git/hooks`, a relative `--git-common-dir`, "hook already installed", the refusal of a foreign `pre-commit` with the body printed (exit 2), and `#!/bin/sh` plus mode `0755`. `hook` runs in a v3 repo and natively. In a v2 repo the shim sends it to node as every other verb.

The trap: the body names `factory`, not the absolute path `factory_command` gives workers. It runs in the owner's shell at commit time, after the binary that printed it may have been rebuilt elsewhere. A missing `factory` must exit 0, so the absolute path is wrong here.

**Acceptance:** `tests/docs_v3.rs`: on `Fixture::new`, `factory docs hook` exits 0, prints `installed .git/hooks/pre-commit`, and the file is executable and starts with `#!/bin/sh\n# feature-docs hook\n`. A second run prints `hook already installed`. With `git config core.hooksPath /tmp/x` set in the fixture, it prints the body and installs nothing. With a foreign `pre-commit` present, it exits 2 and leaves the file unchanged. `--here` sets `core.hooksPath` to `.git/hooks`.

### 2. The hook body refuses an archive edit and a broken link — high — blocked by: 1
**Source**: §7 FR-17 Acceptance: "The hook body test stages a change under `docs/archive/` and the commit is refused; stages a `docs/` change in a v3 fixture with a broken link and the commit is refused by `factory docs check`."

`tests/docs_v3.rs`: install the hook in `Fixture::new` with `target/debug` first on the fixture's `PATH`. Case one: commit a file under `docs/archive/001-x/`, modify it, stage, `git commit`. Case two: append a markdown link to a missing `missing.md` to the feature's `prd.md`, stage, `git commit`. Case three: the same broken link, with a `PATH` that holds only the `git` and `sh` dirs.

The trap: case three passes for the wrong reason if a `factory` sits in a kept dir. On this machine `/usr/bin` holds none, but `~/.local/share/mise/shims` does. Assert `command -v factory` fails under that `PATH` before the commit.

**Acceptance:** case one exits non-zero with "docs/archive/** is frozen" on stderr, and `git log` is unchanged. Case two exits non-zero with `E-LINK-BROKEN` in its output, and `git log` is unchanged. Case three commits (exit 0).

### 3. `factory docs --help` lists the eighteen verbs — medium — blocked by: 1
**Source**: §5 FR-15: "verbs: `init`, `new`, `slice`, `status`, `preflight`, `gate`, `set` (FR-13), `log add|show` (FR-14), `idea add|list|drop` (FR-20), `ship`, `archive`, `renumber`, `lanes`, `check`, `doctor`, `hook`, `migrate` (FR-16), `retro` (FR-19)." Acceptance: "`factory docs --help` lists the eighteen verbs and nothing else."

`tests/docs_v3.rs` (or `tests/cli.rs` beside `help_lists_every_subcommand`, `:17`): a `DOCS_VERBS: [&str; 18]` constant. Run `factory docs --help` in a v3 fixture and in a directory with no `docs/` (help is native in both, slice 01 routing). Collect the first word of each verb line. If slice 01's help format makes "verb line" ambiguous, fix the format here: one verb per line, indented two spaces.

**Acceptance:** the collected set equals `DOCS_VERBS` in both runs. Neither output contains `render`, `vendor` or `merge`.

### 4. The skills say `factory docs` — high — blocked by: 3
**Source**: §7 FR-17: "in the nine files under `~/.claude/skills/` that name `docs.mjs` (23 lines: `deliver/SKILL.md`, `feature-docs/SKILL.md`, `feature-docs/migration.md`, `guardrails/SKILL.md`, `polish/SKILL.md`, `prd/SKILL.md`, `prd-to-tasks/SKILL.md`, `test-cleanup/SKILL.md`, `verify-app/SKILL.md`), swap `node ~/.claude/skills/feature-docs/bin/docs.mjs` for `factory docs`. `feature-docs/bin/docs.mjs` itself is not edited… The worker does not commit in that repo." Acceptance: "After the last slice, `grep -rn docs.mjs ~/.claude/skills --include=*.md` matches only prose inside `feature-docs` that describes the v2 tool and the shim, none of it a command to run."

Swap the string on the 25 command lines listed under Current code. That covers `test-cleanup/references/adapters.md:11` for the PRD's `test-cleanup/SKILL.md`, and `feature-docs/upgrades/v2.md:112-113`, which the PRD's list missed. `feature-docs/SKILL.md:180`: rewrite as prose about the shim: "`factory docs` does the bookkeeping a machine can decide; in a v2 repo it runs `bin/docs.mjs`". Use `sed -i` per file with the literal path, and diff each file after. Do not run `git add` or `git commit` in `~/.claude/skills`.

The trap: `prd/SKILL.md:164`, `feature-docs/migration.md:121` (`check --fix`) and `deliver/SKILL.md:145` (`log merge`) become `factory docs` calls that work only in a v2 repo. Swap them as the PRD says, and write one AD with `Review: decide` naming the three lines and their v3 exit. Do not reword them into v3 verbs: that is a skill behaviour change the PRD does not make.

**Acceptance:** `command grep -rn 'docs\.mjs' ~/.claude/skills/ --include='*.md' | command grep -v '/feature-docs/bin/'` prints only `feature-docs/SKILL.md:180`, and that line holds no `node`. `command grep -rn 'node ~/.claude/skills/feature-docs/bin/docs.mjs' ~/.claude/skills/ --include='*.md'` prints nothing. `git -C ~/.claude/skills diff --stat` lists only the 10 files, plus the owner's in-flight `polish/SKILL.md` change still present. `git -C ~/.claude/skills log -1` is the commit it was at slice entry.

### 5. The skills' ideas lines name the ideas verbs — high — blocked by: 4
**Source**: §7 FR-17: "**Ideas are more than a swap** (FR-20): `polish/SKILL.md:127-129`, `prd-to-tasks/SKILL.md:92`, `feature-docs/autonomy.md` (the Park row and the May list) and `prd/template.md:66` change "an `ideas.md` line" to `factory docs idea add`; `prd/SKILL.md:56,160` changes "delete the line" to `factory docs new … --from-idea <id>`; `deliver/pr-body.md` names the idea id."

Edit the lines listed under Current code. Use the same words as this repo's `assets/autonomy.md` and `assets/pr-body.md` after slice 10, so the two copies stay one text. `prd/SKILL.md:56` graduates with `factory docs new <slug> --from-idea <id>`, which deletes the idea file. `:160` then has nothing to remove: say the file went with `--from-idea`. `prd/template.md:66` links `docs/ideas/` and names `factory docs idea add`.

The trap: in a v2 repo `idea add` fails (`docs.mjs:2107-2109`, exit 2). Each edited line keeps a clause for v2 repos, "in a v2 repo (`factory docs doctor` says so), add the line to `docs/ideas.md` as before", or the `/polish` Park step in an unmigrated repo drops its findings. The PRD names no v2 fallback, so record it as an AD with `Review: decide`.

**Acceptance:** `command grep -rn 'ideas\.md' ~/.claude/skills/{polish,prd-to-tasks,prd,deliver}/ ~/.claude/skills/feature-docs/autonomy.md` matches only the v2-fallback clauses. `command grep -c 'idea add' ~/.claude/skills/feature-docs/autonomy.md` is at least 2. `command grep -n 'from-idea' ~/.claude/skills/prd/SKILL.md` matches `:56`. Run `factory docs idea add --title Probe --body-file -` (body from stdin) in a temp `factory docs init` repo, then `idea list --json` and `idea drop <id> --because probe`, all exit 0. That proves the spelling the skills now teach.

### 6. One fixture constructor — low — blocked by: —
**Source**: §8 FR-18: "The harness builds the v3 fixture with the binary: `factory docs init`, `factory docs new fixture --title Fixture`, `factory docs slice 001 a|b|c --depends 2`, `patch_prd`, no render; `Fixture::docs()` and `docs_json()` run `env!("CARGO_BIN_EXE_factory") docs …`."

`tests/`: replace every `Fixture::v3(` with `Fixture::new(` and every `factory_docs(` and `factory_docs_json(` with `docs(` and `docs_json(`. Then delete the three aliases from `tests/harness/mod.rs`. `Fixture::v2` (slice 09) stays.

**Acceptance:** `command grep -rn 'Fixture::v3\|factory_docs' tests` prints nothing. `cargo test` compiles with no `dead_code` warning from the harness.

### 7. Install the branch binary — high — blocked by: 1, 2, 3, 4, 5, 6
**Source**: §7 FR-17: "The slice ends with `cargo install --path .` so that `factory docs` exists on this machine the moment the skills say it (§14 states the cost)." §14: "the branch build becomes the installed `factory`, with the shim routing this v2 repo to node. The running driver is unaffected (Linux keeps the open binary), and the next worker it spawns runs the new one."

Run `cargo install --path .` after the inner loop is green. Run it in the foreground and time it. Then, in this repo (still v2), run `factory docs check` and `factory docs status 002 --json`. Run `factory docs --help` too.

The trap: the polish worker of this very run gets the new binary (Current code, `paths::self_exe`). If the install fails halfway, every skill's docs step on this machine fails with an unknown subcommand until it is rerun. A failed install is a block, not a skip.

**Acceptance:** `command -v factory` is `~/.cargo/bin/factory`. `factory docs --help` lists `hook`. `factory docs check` in this repo exits 0 through the shim. `factory docs status 002 --json` prints this feature's board.

### 8. Verify — medium — blocked by: 1, 2, 3, 4, 5, 6
`cargo test` (the inner loop; the full gate runs at this slice's checkpoint)

**Acceptance:** inner loop green, one commit (this repo only; the skills repo stays uncommitted).

### 9. Prove — at the checkpoint — blocked by: 7, 8
Rows from [`prd.md`](prd.md) § QA plan: the machine half of **FR-17**. The hook body refuses an archive edit and a broken link (task 2's transcript). After the skills slice, the `docs.mjs` grep over `~/.claude/skills` and `factory docs check` in this repo exit as §7 says: run the grep with `command grep`, and run the check with the installed binary. Also the `--help` half of **FR-15**: the `factory docs --help` output with its eighteen verbs. The human half of FR-17 is the owner's, in [`qa-guide.md`](qa-guide.md): run `/prd`, `/polish` and `/prd-to-tasks` once each in a v2 repo, commit the skills repo, and paste the `factory docs hook` body into the global hook. Evidence goes in this slice's Progress.

**Acceptance:** after the checkpoint, Progress holds the hook test transcript with both refusals, the grep output (one prose line), `factory docs check` with `exit 0`, the `--help` output and `git -C ~/.claude/skills diff --stat`.
