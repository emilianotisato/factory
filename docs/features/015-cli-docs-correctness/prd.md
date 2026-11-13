---
number: 015
slug: cli-docs-correctness
title: "CLI, docs and log correctness"
type: bugfix
created: 2026-11-05
author: Emiliano Tisato
summary: |
  factory's own tools stop misleading the agent:
  - in a lane, every docs verb reads and writes the board files (`state.toml`, `log.jsonl`) in the main tree, and says so;
  - every driver commit subject fits 50 characters, with the rest in the body;
  - relock replaces the hash of a version that has no tag yet;
  - the `skills update` and `init` commit lines commit only their own dirs;
  - from-idea PRDs keep working relative links;
  - docs verbs exit 2 on a file or TOML error;
  - the run log has one reader;
  - the board scan and `renumber` skip symlinks.
  Interviewed 2026-11-12.
---

# PRD-015 — CLI, docs and log correctness

## 1. Why now

These are places where an agent's own tool lies to it.

Bugs the owner's runs hit:

- **`docs set reviewed=` looked like a no-op** (2026-11-05). It printed "state.toml updated", but `docs check` in the lane kept warning `W-NOT-REVIEWED`. `log add` and `log show` split the same way in two client runs (icg-cloud PRD-019, arca-api PRD-025), and AD-14 duplicated AD-13 because of it.
- **Relock tripped every feature that edits a skill twice:** PRD-005, 007, 008, 012, 017 and 018. Both of 2026-11-12's runs restored the base lock by hand (PRD-017 retro `:22`, PRD-018 retro `:19`).
- **Slice commit subjects run 53 to 73 characters**, breaking the owner's 50-character rule on every run.
- **From-idea PRDs broke every relative link:** 8 PRDs in one downstream project, fixed by hand.

The rest are defects found by earlier polish reviews (PRD-002, PRD-012):

- an unscoped commit line;
- an exit code a hook cannot read;
- a second log reader;
- symlinks followed by the board scan and `renumber`.

**Deliberately left alone:** the two log races, and the two hand-rolled verbs (§14).

## 2. Blast radius

- **Workers in a lane.** FR-1 changes what `check`, `log show` and `status` read in a lane. Every worker runs them, so a worker now sees its own `docs set` and `log add` writes. Outside a lane (`FACTORY_BOARD` unset, or another repo) nothing changes.
- **Commit history.** FR-2 changes every driver commit subject longer than 50 characters. The landing code finds an already-landed commit by exact subject:
  - `src/scheduler.rs:993-994`;
  - `src/closeout.rs:579-582`;
  - `src/checkpoint.rs:751-754`.

  So the cut must be deterministic. A run resumed across the upgrade looks for the new subject. A slice that landed under the old long subject, before the upgrade, is not found and lands again. Accepted: upgrade between runs, as `CLAUDE.md` already says.
- **Releases.** FR-3 lets a skill hash change under an untagged version. Once `vX.Y.Z` is tagged, nothing changes: the released hash is still frozen.
- **No external contract.** No JSON, event or schema changes. The `commit:` lines that `skills update` and `init` print change shape. Their only readers are agents following `skills/_shared/stale-tooling.md:7`, and the new line still runs as one command.

## 3. FR-1 — In a lane, the board files come from the main tree

**Today.**

- `board_root` (`src/cli/docs.rs:253-264`) swaps the whole root to `$FACTORY_BOARD`, the main tree (`src/worker.rs:55,260`), for `set`, `retro`, `status` and `log add` (`:225`, `board_root(&root, args).unwrap_or(root)`). It does so only when that tree shares a git common dir with the cwd. Those verbs read everything from main: prose, `project.toml`, the git branch.
- Every other verb uses the lane, including `check` (`:292`, `check::run(root)`) and `log show` (`:436`, `log::entries(&f.dir)`).
- The driver's own in-process check, which gates a slice's `done`, is `check::run(&lane_dir)` (`src/scheduler.rs:2076`). It has no `FACTORY_BOARD`, and nothing here changes it.
- So the polish worker, which runs in a lane (`src/closeout.rs:197,230`), sets `reviewed` in main's `state.toml`. Its `docs check` then reads the lane's copy and warns `W-NOT-REVIEWED` (`src/docs/check.rs:537-541`).
- The message `docs/features/{name}/state.toml updated` (`src/cli/docs.rs:706`) does not say which tree.
- The driver commits the main tree's board files (`src/commit.rs:15`, `src/closeout.rs:573,685`), so main is the record.

**Decision (2026-11-12; narrowed the same day after the adversarial pass).** An earlier draft said every docs verb would read only the board files from main and everything else from the lane. That was wrong twice:

- the four listed verbs already swap the whole root;
- making `status` read prose from the lane breaks `tests/docs_v3.rs:637`, whose JSON carries the git branch, and the detached lane says `HEAD`.

The rule as built:

- **`log show` joins the whole-root list.** It reads only the log, so a whole-root swap is exact.
- **`check` keeps the lane as its root,** so it still validates the prose the worker is editing. But when `board_root` would resolve for it, it reads each feature's `state.toml` and `log.jsonl` from the matching folder in the main tree.
- **A write's message names the main tree's path** when it is not the cwd: `set`, `log add`, and `retro --set`.
- **Unchanged:**
  - `docs new` and `docs slice`, which write the cwd's `state.toml` as today. No run has created a feature inside a lane.
  - The driver's in-process `check::run(&lane_dir)`.

- **Accepted cost:** in a lane, `check` mixes two trees, the lane's prose and main's board files. It can now disagree with the driver's in-process check of the same lane on a warning such as `W-NOT-REVIEWED`. The driver's check gates on errors, and the lane gets main's board files at its next reset.
- **Rejected:** keeping the split and fixing only the message. `check` would still warn `W-NOT-REVIEWED` after polish set it.
- **Trap:** the same-repo check stays (`:261-263`). A fixture repo inside `cargo test` in a worker must never read the worker's board.

**Acceptance.**

- In a lane with `FACTORY_BOARD` set:
  - `docs set N reviewed=<date>` followed by `docs check` shows no `W-NOT-REVIEWED`;
  - `docs log add` followed by `docs log show` shows the new entry.
- `set`'s message names the main tree's path.
- `a_lane_writes_the_main_trees_board_and_ignores_another_repos` (`tests/docs_v3.rs`) still passes, including the `status` JSON at `:637`, the `retro` read at `:646` and the other-repo half.

## 4. FR-2 — Every driver commit subject fits 50 characters

**Today.**

- `commit::subject` (`src/commit.rs:149-151`) is `format!("{kind}({:03}): {text}", …)`, with no cap.
- Slice commits pass `slice-{NN} {title}` (`src/scheduler.rs:988`). The 20-character prefix leaves 30 for the title, and real titles run 53 to 73 in all.
- `fix(NNN): CI {check}` (`src/closeout.rs:482`) carries an arbitrary check name.
- Both commit helpers pass a single `-m` (`src/commit.rs:73`, `src/land.rs:71`). The held-commit path already passes a full `%B` message, body included, as that one `-m` (`src/scheduler.rs:1021-1024`). An earlier draft said neither helper could write a body (corrected 2026-11-12). So a body is the subject plus a blank line plus the body, in the same `-m`.

**Decision (2026-11-12).** `commit::subject` returns at most 50 characters.

- When the text overflows, it is cut at the last space that keeps the subject within 50 **characters** (chars, never bytes: titles hold "—"), with no ellipsis.
  - When no space after the `kind(NNN): ` prefix fits, it is cut at 50 characters, on a char boundary.
  - The full text becomes the body's first line.
- The cut is a pure function of the text, so the landing lookups by `%s` (§2) still match.
- The commit helpers gain an optional body, passed as a second `-m`.
- The limit is fixed at 50, with no knob.

- **Accepted cost:** a long slice title reads cut in `git log --oneline`. The body holds it whole.
- **Rejected:**
  - a configurable limit, a knob nobody asked for;
  - dropping the title, which makes history read worse.

**Acceptance.** For both a 73-character slice title and a 60-character CI check name:

- the subject is at most 50 characters;
- it ends at a word boundary;
- the body's first line is the full text;
- a second call on the same input returns the same subject.

The landing tests in `tests/commits.rs`, `tests/merge.rs` and `tests/checkpoints.rs` still pass. Their one-letter titles are unchanged.

## 5. FR-3 — Relock replaces the hash of an untagged version

**Today.**

- `relock` (`src/skills.rs:165-182`) accepts a new hash only when the version is strictly greater than the lock's. Otherwise: "skills changed since {v}: bump Cargo.toml past {v} first".
- Its only caller is `lock()` in `tests/skills_bundle.rs:186-207`, under `FACTORY_BUNDLE_WRITE`. CI only checks (`bundle_lock_matches`).
- So a second skill edit on a feature branch, after a slice already bumped and relocked, cannot relock without a second bump. PRD-005, 007, 008, 012, 017 and 018 each worked around it.
- `scripts/release.sh:30-31` already probes tags:
  - locally: `git rev-parse -q --verify refs/tags/$tag`;
  - on origin: `git ls-remote --exit-code --tags origin refs/tags/$tag`.
- **Trap** (adversarial pass, 2026-11-12): `release.sh:31` treats every non-zero exit as "no tag". `ls-remote --exit-code` exits 2 for "no such ref" and other codes (128) for "cannot reach". The probe must tell exit 2 apart from the rest, or an offline relock replaces a released hash: the case this FR rejects.

**Decision (2026-11-12).**

- At the same version, relock replaces the hash when `v<version>` is a tag neither locally nor on origin. It keeps refusing once either has the tag: that hash is what users installed.
- When origin cannot be reached (offline, or no `origin` remote), it refuses and says so ("could not reach origin to prove v… is untagged; retry online or bump").
- `relock` stays pure: the caller passes whether the version is tagged. The probe runs in the test-side caller, the way `release.sh` does it.

- **Accepted cost:** relock at the same version needs the network, and an offline relock refuses.
- **Rejected:** trusting local tags when origin is unreachable. A tag pushed from another machine and not fetched would let a released hash be replaced.

**Acceptance.**

- `a_skill_edit_without_a_bump_fails_and_cannot_relock` (`src/skills.rs:886`) splits:
  - an untagged same-version relock succeeds;
  - a tagged one refuses;
  - an unknown one (origin unreachable) refuses with the online message.
- The bump cases at `:904-926` still pass.

## 6. FR-4 — The printed commit lines commit only their own dirs

**Today.**

- `factory skills update` prints `commit: git add <dirs> && git commit -m "chore: skills <v>"` (`src/cli/skills.rs:94`).
- `factory init` prints the same unscoped form (`src/docs/scaffold.rs:131`).
- `git commit` with no pathspec takes the whole index. `skills/_shared/stale-tooling.md:7` has `/prd`, `/slice`, `/deliver` and `/lanes` run the update line without asking, so anything already staged rides along (PRD-012 polish, grok).

**Decision (2026-11-12).** Both lines end `git commit -m "…" -- <dirs>`. The paths are tracked after the `add`, so new files are included.

- **Accepted cost:** none.

**Acceptance.** With `src/x.rs` staged, running the printed line commits only the listed dirs and leaves `src/x.rs` staged. `tests/init.rs:380` compares the whole line and moves with it. `tests/skills_fence.rs:286,403` and `tests/init.rs:217` match a prefix and pass unchanged (an earlier draft said all four move; corrected 2026-11-12).

## 7. FR-5 — From-idea PRDs keep working relative links

**Today.**

- `with_idea` (`src/docs/scaffold.rs:349-370`) pastes the idea's body verbatim under `## From idea {id}` (`:363`).
- The idea lived in `docs/ideas/`, and the PRD lives one folder deeper, so `../reference/x.md` now points at `docs/features/reference/x.md`.
- `check` already flags the result as `E-LINK-BROKEN` (`src/docs/check.rs:791-822`). That is how the owner found 8 of them, all fixed by hand.
- `src/docs/migrate.rs:1242-1328` has a link rewriter (`edit_links`) that skips URLs, `#` anchors and images, but not code fences.

**Decision (2026-11-12).**

- `with_idea` rewrites each relative markdown link target from the idea's folder to the PRD's folder, reusing `migrate.rs`'s link helpers.
- URLs, `#` anchors and links inside fenced code blocks stay as written.
- **Accepted cost:** none.

**Acceptance.** For an idea whose body holds markdown links to `../reference/x.md`, `./other-idea.md`, `https://x` and `#a`, plus a fenced block holding a link to `../y.md`, the new PRD:

- resolves the first two to the same files as from the idea's folder;
- leaves the URL, the anchor and the fenced block byte-for-byte unchanged.

The same idea without the fenced block passes `docs check` with no `E-LINK-BROKEN`. `check`'s link scan (`src/docs/check.rs:791-862`) does not skip fences, so a fenced link is still flagged. An earlier draft claimed the whole body would pass check. Corrected 2026-11-12.

## 8. FR-6 — Docs verbs exit 2 on a file or TOML error

**Today.**

- `cli::docs::docs` maps `Error::Msg` to exit 2 (`src/cli/docs.rs:213-221`), so a `state.toml` that does not parse already exits 2 (`src/docs/state.rs:74-76`).
- `Io`, `Toml` (from `project.toml`, `src/docs/project.rs:58,108`), `Json` and `Sql` fall through to `src/main.rs:109-113`, which exits 1.
- `check` uses 1 for "errors found" (`src/cli/docs.rs:290-297`), so a hook cannot tell a broken `project.toml` from a failed check.

**Decision (2026-11-12).** Under `factory docs`, every `Err` exits 2: the `Io`, `Toml`, `Json` and `Sql` variants join `Msg` in `cli::docs::docs`. Exits 1 that a verb returns on purpose as `Ok(1)` are unchanged:

- `check`'s errors;
- `retro` with no row (`:574`);
- `gate` with no gate (`src/cli/gate.rs:481,519`);
- preflight's (`src/preflight.rs:221,418`).

- **Accepted cost:** none.

**Acceptance.** `docs log show 001` where `log.jsonl` is a directory exits 2. On v0.13.4 it exits 1, checked 2026-11-12. A `check` with errors still exits 1. An earlier draft tested an unparseable `project.toml`, which already exits 2 through the "not on the framework" path (`src/cli/docs.rs:246-249`). Corrected 2026-11-12.

## 9. FR-7 — The run log has one reader

**Today.** `answers::entries` (`src/answers.rs:150-160`) parses `log.jsonl` with its own `serde_json::from_str` loop. `docs/log.rs` already has the reader:

- `parse` at `:131` is private and drops line numbers;
- `entries` at `:154` needs the 1-based line.

**Decision (2026-11-12).** `docs::log` exposes one reader that returns each entry with its 1-based line. `answers::entries` uses it, so the two cannot drift on a format change.

- **Accepted cost:** none.
- The two races (§14) stay out.

**Acceptance.**

- `answers::entries` has no JSON parsing of its own.
- `a_block_is_resolved_by_an_answer_and_its_line_stays` (`src/answers.rs:272`) still passes.
- A log holding a line of invalid UTF-8 no longer makes `answers::entries` fail. Today it fails with `read_to_string` (`src/answers.rs:152-155`), while the docs reader reads lossily (`src/docs/log.rs:134`). Its other lines are read with the same line numbers.
- An earlier draft's acceptance, an unparseable line skipped the same way by both, already held. Corrected 2026-11-12.

## 10. FR-8 — The board scan and `renumber` skip symlinks

**Today.**

- `src/docs/board.rs:160` filters feature folders with `is_dir()`, which follows links.
- `:190-199` reads any `slice-NN-*.md`, link or not.
- `src/docs/renumber.rs:22-33` recurses with `is_dir()`, following links. A link to an ancestor stops only at the kernel's 40-link limit, so an earlier draft's "never ends" was wrong (corrected 2026-11-12). It then rewrites every `.md` it found, through the link (`:96-110`).
- `src/docs/check.rs:732-733` already uses `fs::symlink_metadata` against FIFOs and `/dev/zero` (PRD-002 security pass).

**Decision (2026-11-12).**

- The board scan and `renumber` use `symlink_metadata`.
- They skip a symlinked feature folder or slice file, and `renumber` never follows a link down.
- **Accepted cost:** a symlinked feature is invisible to the board.

**Acceptance.** In a fixture with a symlinked feature folder, a symlinked slice file and a link from `docs/features/x/loop` to `docs/`:

- the board (`docs status --json`) has no feature for the linked folder;
- no slice's `file` points at the linked slice file;
- `docs renumber` rewrites no file reached through a link.

Out of scope: `check`'s own `walk_md` (`src/docs/check.rs:777`) and `migrate.rs:1332` follow directory links the same way. Nothing seen depends on it.

## 11. Decisions log

- ~~**Lane docs rule**~~: the board files always come from the main tree (2026-11-12), §3.
- ~~**Subject shape**~~: cut the title at a word, full text in the body, fixed at 50 (2026-11-12), §4.
- ~~**Cap scope**~~: every driver subject (2026-11-12), §4.
- ~~**Relock**~~: replace while untagged locally and on origin; refuse when origin cannot be reached (2026-11-12), §5.
- ~~**Code-review fixes verb**~~: left out (2026-11-12), §14.
- ~~**Log races**~~: left out; one reader only (2026-11-12), §9, §14.
- ~~**Superseded AD in `log show`**~~: left out (2026-11-12), §14.

## 12. QA plan

- **Gate:** `factory docs gate`.
- **Surface:** the `factory` CLI and the driver's commits. No UI and no harness: every row is proven below the UI by a test whose transcript is the evidence.
- **Seed:** the existing fixtures (`tests/harness/mod.rs`, a lane fixture as in `tests/docs_v3.rs`).
- **Human rows:** none, so there is no `qa-guide.md`.

| FR | Proof | Human? |
|---|---|---|
| FR-1 | run `a_lane_reads_the_board_files_it_wrote` | no |
| FR-2 | run `a_long_subject_is_cut_at_a_word_with_the_rest_in_the_body` and `a_long_slice_title_lands_under_a_short_subject` | no |
| FR-3 | run `an_untagged_version_relocks_and_a_tagged_one_refuses` and `relock_refuses_when_origin_cannot_be_reached` | no |
| FR-4 | run `the_printed_commit_line_leaves_other_staged_files_alone` | no |
| FR-5 | run `a_from_idea_prd_keeps_its_relative_links` | no |
| FR-6 | run `a_docs_verb_exits_2_on_an_unreadable_project_toml` | no |
| FR-7 | run `answers_read_the_log_through_the_docs_reader` | no |
| FR-8 | run `the_board_and_renumber_skip_symlinks` | no |

## 13. Slices

_Not sliced yet: `/slice` fills this._

## 14. Non-goals

- **Not handled: two `factory answer` calls racing** (2026-11-12, owner cut). `resolve` never re-checks "still open" under `docs.lock` (`src/answers.rs`, `src/docs/log.rs:195-228`), so both calls can append. It needs two terminals, and it has never happened. Reopen on a duplicate answer.
- **Not handled: `answer`'s slice status read, then set** (`src/cli/answer.rs:204-208`). A driver write in between is overwritten. Same cut and same signal.
- **Not handled: `settle_dead`'s two appends** (`src/scheduler.rs:2040` then `:1923-1931`). A crash between them leaves a block with no note, though the block body already says what happened. Reopen on a block with no note.
- **Not handled: a verb that appends a slice's Code-review fixes** (2026-11-12). Polish's hand edit (`skills/polish/SKILL.md:163`) has caused no reported defect. Reopen if one does.
- **Not handled: `log show` marking an AD a later `**Supersedes:**` line overturned** (2026-11-12). The review passes already drop it (`src/settled.rs:48-100`), and no run has been misled. The old `log merge` rename is moot: v3 has no `log merge` (`src/cli/docs.rs:209`).
- **Polish §8 names the `docs set reviewed=` verb**: done in PRD-017 (`skills/polish/SKILL.md:168`).
- **The `.push(` hook false positive**: PRD-019.
- **One landing pipeline** (PRD-020 C-8) also edits `land_lane`, but not `commit::subject`. Whichever ships second rebases.

## 15. Verification notes

Checked 2026-11-12 on `feat/cli-docs-correctness` at main `820d21b` (v0.13.4). Re-verify at slice entry.

- `board_root` is at `src/cli/docs.rs:253-264`, unchanged since PRD-005 (`611dcd1`). `FACTORY_BOARD` is set at `src/worker.rs:55,260`. Polish runs in a lane (`src/closeout.rs:197,230`). The `set` message is at `src/cli/docs.rs:706`.
- `commit::subject` is at `src/commit.rs:149-151`. The slice subject is at `src/scheduler.rs:988`, and the CI subject at `src/closeout.rs:482`. The lookups by subject are at `src/scheduler.rs:993-994`, `src/closeout.rs:579-582` and `src/checkpoint.rs:751-754`. The single `-m` is at `src/commit.rs:73` and `src/land.rs:71`.
- `relock` is at `src/skills.rs:165-182`; its test is at `:886`, with bump tests at `:904-926`. The caller is `tests/skills_bundle.rs:186-207`, and the tag probes are at `scripts/release.sh:30-31`. No skill mentions relock today.
- The commit lines are at `src/cli/skills.rs:94` and `src/docs/scaffold.rs:131`. Their tests are at `tests/skills_fence.rs:286,403` and `tests/init.rs:217,380`. The draft named `src/skills.rs` and `tests/skills_bundle.rs` tests; there are none.
- `with_idea` is at `src/docs/scaffold.rs:349-370`, and the link helpers at `src/docs/migrate.rs:1242-1328`. `E-LINK-BROKEN` is at `src/docs/check.rs:791-822`.
- The exit-2 mapping is at `src/cli/docs.rs:213-221`, and the fall-through to exit 1 at `src/main.rs:109-113`. The error variants are at `src/error.rs:7-28`.
- `answers::entries` is at `src/answers.rs:150-160`. The log reader's `parse` is at `src/docs/log.rs:131` and `entries` at `:154`.
- The symlink-following scans are at `src/docs/board.rs:160,190-199` and `src/docs/renumber.rs:22-33,96-110`.
- **Adversarial pass (2026-11-12):** a fresh Opus agent, since Cursor's quota is out until 2026-11-24 (`factory review second` exit 2). It made 9 findings, all applied above as dated corrections. FR-1 was narrowed: `log show` swaps the whole root, and `check` reads only the board files from main. The acceptances of FR-5, FR-6, FR-7 and FR-8 were made able to fail. FR-2's cut was defined in chars, and FR-3's probe trap named.
- No other draft touches `answers.rs`, `cli/docs.rs`, `skills.rs`, `cli/skills.rs`, `board.rs`, `renumber.rs` or `scaffold.rs`. PRD-022 edits `src/commit.rs` `protected_files`, a different function. PRD-021 waits on FR-3.

## 16. Source ideas

The full text of the ideas this PRD absorbed is in the 2026-11-05 triage draft, `git show 820d21b:docs/features/015-cli-docs-correctness/prd.md` §4.
