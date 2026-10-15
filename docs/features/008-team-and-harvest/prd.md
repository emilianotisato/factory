---
number: 008
slug: team-and-harvest
title: "A repo a team can share, and the harvest"
# type: feature | bugfix | enhancement | refactor
type: feature
created: 2026-10-15
author: "Emiliano Tisato (with Claude)"
summary: |
  A factory repo several people share from different machines: evidence stays in the run dir, the closeout files no friction, the retro compares tracked retros, docs check warns on home paths. Archive becomes a harvest (`factory docs harvest`, `/harvest`) that routes each lesson to the project, the framework or factory (`factory upstream`), and decisions.md trades DEC numbers for slug anchors.
  Agreed 2026-10-14: FR-1 to FR-8, FR-9 retired at the cut. A cross-vendor review corrected 13 claims inline.
from_idea: "2026-10-15-archive-harvests-what-a-feature-taught"
---

# PRD-008 — A repo a team can share, and the harvest

## From idea 2026-10-15-archive-harvests-what-a-feature-taught

`factory docs archive` only moves a folder, so nobody runs it: on 2026-10-14 factory had seven features done and none archived. Done features stay in `docs/features/` because their retros are worth reading together, and the owner does read them in batches to improve the project's harness and the framework. Give archive that meaning: **archive = the lessons are harvested.** Read a batch of done features, route every lesson to where it belongs, then archive the batch.

**Why agents need it.** A feature folder mixes records and instructions. A record (a log entry, a retro, "on 10-15 we chose X") stays true after the code moves. An instruction goes stale and still sounds sure: slice-04 of PRD-007 places `branch_guard` at `src/closeout.rs:1109-1139`, and one day later it starts at line 1203. A future agent, or a teammate's agent on another machine, searches the repo and trusts what reads as current. Slices are the worst case, and they are two things glued: the recon and the tasks are instructions, while the "Code-review fixes" and the plan-versus-actual story are exactly what the retro analysis reads. So slices stay until the harvest, then leave for git history.

**Lean: what archive keeps.** `prd.md` with a banner ("History. The code, `docs/reference` and `docs/decisions.md` win.": PRD-007's AD-6 already overrode FR-5 as written), `state.toml` (the folder keeps the next number countable), `log.jsonl` and `retro.md` (records). It drops the slices, and any evidence still in the tree (the team-repo idea says evidence should not be there at all). Agents outside a factory run get the same rule from one line in the convention block `factory init` writes into `CLAUDE.md`: `docs/features` and `docs/archive` are history; the code, `docs/reference` and `docs/decisions.md` win.

**Lean: where a lesson goes.** The brainstorm's three scopes, project → user → base:
- **The project**: the harness (gate, verify skill, lanes, `docs/ux-rubric.md`), `CLAUDE.md`, `docs/reference`, a decision. A diff in this repo.
- **The framework in this project's context**: a `docs/project.toml` knob, a project overlay of a skill.
- **Factory itself**: with a factory checkout on the machine, an idea file in its `docs/ideas` (half built: when `~/.config/factory/config.toml` names `[source] path`, every run's closeout copies the retro's `## Framework friction` section into that checkout's `docs/ideas/`, uncommitted, via `file_friction` in `src/closeout.rs`; `2026-10-15-friction-cody-agent-prd-026.md` came from a cody-agent run that way. It files at closeout, not at the harvest). Without one, for anyone else running factory, a script drafts a GitHub issue on `emilianotisato/factory` that the maintainers take or leave. ⚠ That repo is public: an insight from a client repo is rewritten in general terms (no client or customer names, domain data or local paths) and shown to the person before it posts, and it searches open issues first so a repeat becomes a +1, not a duplicate. The closeout's friction file already leaks one: it ends `From <absolute path of the retro>`, a client project's local path.

**Lean: decisions get compacted at the harvest.** Today `DEC-n` is monotonic and never reused, a reversed one is superseded but never deleted, and each heading names its origin (`PRD-011 slice-06`). The owner asked why number and source them at all. Piece by piece:
- **The origin** points into slices the harvest drops, and `git log -S` on `decisions.md` finds the PR anyway. The **Because** line carries the incident in plain words instead.
- **The number's** one real job is a stable link target: arca-api's architecture and operations docs link `decisions.md#dec-n` anchors 11 times. Logs and PR bodies that cite a number are records and may go stale. Named slugs (`#public-reads-are-pure`) keep the link, survive a rewrite, and cannot collide the way two developers' `DEC-22`s can. A merged entry keeps its old slug as an extra anchor.
- **Supersede-not-delete** keeps the road not taken, and the **Over** line already does that. Fold the loser into the winner's Over and delete the dead entry.

So the harvest may merge, rewrite, regroup by topic and delete an entry whose **Applies when** no longer exists in the code, as a diff the owner reviews. Two rules hold: an **Over** is never lost (it is what stops an agent re-proposing the rejected option), and a decision that is really a domain fact moves to the doc that owns the topic (arca-api's DEC-10, DEC-11, DEC-15 and DEC-16 read like ARCA rules, not engineering forks). The skills citing `DEC-n` change with it: polish (the lookup, "cite it", the entry template, the numbering rule), ux, gate.

**Relation to the learn loop.** The brainstorm planned an event-driven `learn` step (retros, overturned ADs, parks, repeated reds; never an unattended rule edit). The harvest is its human end: archive marks a batch whose events were read and routed.

Open: what makes a batch (a count, an age, the owner's call); the split between a verb (the mechanical moves, the slug anchors, the issue draft) and a skill (the routing judgment, with the owner).

## From idea 2026-10-13-decide-what-a-team-repo-commits-evidence-recording

Every feature folder commits run output as it came off the owner's machine, and nobody chose that for a team repo. A sweep on 2026-10-13 found local paths (`/home/<user>`, `/tmp/claude-…`, `/mnt/devcache/…`) in 57 tracked files under `docs/`: `evidence/` transcripts (24 in 001, 15 in 002), the `claude-p-*` recordings, the ctx-jev replay, three committed `__pycache__` files, and PRD text itself. No secrets turned up, but nothing stops one either: a recording or a gate log can carry an env dump, a token, a customer name.

To decide: what a feature folder holds when several devs share the repo. Lean: specs, the log and the retro are committed; evidence and recordings are run artifacts that live in the run dir (or one gitignored folder) and are linked from the PR, not the tree. Whatever is committed goes through a scrub step (home dir → `~`, scratch paths → `<scratch>`, a secret-pattern refusal) written once by the driver, not by each worker. Also: `docs/.gitignore` gets the obvious junk (`__pycache__`), and `docs check` warns on an absolute home path in a tracked doc. Applies to every repo on the framework, so it lands with PRD-004's installer at the latest.

**2026-10-14, after PRD-007.** The owner set the frame: weight does not matter; what matters is what agents read and what a team of people and their agents on different machines shares. A run starts and ends on one machine, and nothing hands it to another mid-way. So evidence has two readers, the run and the PR reviewer, and neither needs the tree. PRD-007's run committed 8 evidence files while its PR body said "Evidence lives on the machine that ran the build": the polish contract already assumes this model and the driver does not do it yet. The lean sharpens:
- **The run dir, not a gitignored folder.** Lanes are separate worktrees, so an ignored file never leaves the lane that wrote it. Evidence belongs beside `driver.log`.
- **Copied into the PR, not linked.** A teammate cannot open a path on the owner's machine, and the owner reads the PR from a phone. The driver already owns every GitHub call (DEC-2), so it puts the text evidence in the PR (a collapsed block per checkpoint, the body caps at 65,536 characters). Open: screenshots. As far as known, `gh` cannot attach an image to a PR; test an alternative before deciding (an orphan branch the driver pushes and links would need the pre-push guard to allow it).
- **Nothing is lost from the tree.** The log's checkpoint entries already sum up each evidence file in a line, and that is the part worth keeping.
- Slices stay committed until the feature is harvested: see `2026-10-15-archive-harvests-what-a-feature-taught`.

## From idea 2026-10-13-multi-dev-coherence-prove-several-developers-can-s

The framework was built for one developer and the assumption leaks everywhere: `models.md` carries one person's Cursor path and default model; feature folders commit evidence with local paths (see the team-repo idea); lane files may hold a developer's DB names and ports; `docs new` reserves numbers only across one machine's worktrees; the metrics store and run dirs are per machine; the retro compares against "the last three runs on this repo" meaning this machine's. Before the first repo with a second developer, review every one of these for one question: what is the repo's, what is the machine's, what is the person's. Lean: the repo commits specs, state, log and retro only; the machine holds runs, evidence and the store; the person holds model preferences and vendor paths in `~/.config/factory/` as user-scope overlays (PRD-004). Also decide what two developers running `factory run` on different features of the same repo at once need: number allocation across clones (`docs check` in CI catches the duplicate today), lane name collisions, and PR bodies that cite the other run. Retrigger: the first repo with a second developer, or when PRD-004's scopes land.

**2026-10-14, after PRD-007.** Three refinements:
- **The retro's comparison should read the tracked retros.** Every clone has every feature's `retro.md` and its numbers table; only this machine's runs are in the local store. Compare against the last three tracked retros, and keep the store as the source of the run's own row. That makes the tracked retros the team's shared metrics.
- **`DEC-n` collides like PRD numbers.** Two developers polishing at once both write `DEC-22`. The harvest idea (`2026-10-15-archive-harvests-what-a-feature-taught`) replaces the counter with named slugs.
- **The split, refined.** Repo: the PRD, state, log, retro and decisions, plus slices until the harvest. Machine: the run dir, evidence included (see the team-repo idea). Person: preferences. A team member's agent outside a run learns that `docs/features` is history from the `CLAUDE.md` convention line in the harvest idea.

> The three idea bodies above are kept as filed. Where they differ from the numbered sections below, the numbered sections win. Two reversals from the interview, 2026-10-14: evidence never reaches the PR (§3 FR-2), and the closeout files no friction at all (§3 FR-1).

## 1. Why now

**Bugs the owner hit:**

1. **The closeout files friction where nobody wants it.** `file_friction` (`src/closeout.rs:416-449`) runs after every polish worker (`:330`, `:2139`). It copies the retro's `## Framework friction` section into the `[source] path` checkout's `docs/ideas/`, uncommitted, one file per run, ending `From <absolute path of the retro>`. Three such ideas here carry the owner's home path (`docs/ideas/2026-10-14-friction-factory-prd-005.md:13`, `2026-10-14-friction-factory-prd-006.md:12`, `2026-10-15-friction-factory-prd-007.md:14`); the cody-agent one was made relative by hand. The owner rejected the mechanism on 2026-10-14 as ugly and inconvenient, and wants it fixed first.
2. **Run output is committed.** The checkpoint contract sends evidence to `{feature dir}/evidence/cp-{n}-*` (`assets/contract-checkpoint.md:15`), and the driver commits the lane with `git add -A` (`src/land.rs:47`). This repo tracks 118 evidence files (001: 31, 002: 17, 003: 7, 004: 22, 005: 21, 006: 12, 007: 8); arca-api tracks 120, 76 of them screenshots. PRD-007's run committed 8 while its PR body said "Evidence lives on the machine that ran the build" (`assets/pr-body.md:32`).
3. **Home paths in shared files.** 42 tracked files under `docs/` hold `/home/<user>`: 22 evidence files, 001's recordings and ctx-jev replay (3 of them committed `__pycache__` files), slices of 002 and 003, two qa-guides, 002's `prd.md`, 006's `research.md`, the three friction ideas, and 003's `log.jsonl`, where the driver itself wrote a BLOCK body with the run dir's absolute path.
4. **Archive never runs.** `factory docs archive` (`src/docs/ship.rs:91-110`) only moves the folder and sets the status. The PR body asks for it before merging (`assets/pr-body.md:10`) and nobody does it: here 7 features are shipped and 0 archived, in arca-api 6 and 0.
5. **Shipped slices read as current.** PRD-007 slice-04 places `branch_guard` at `src/closeout.rs:1109-1139`. It starts at `:1203` today. An agent that searches the repo, or a teammate's agent on another machine, trusts what reads as current.

**Improvements, with no incident yet:**

- The owner reads shipped retros in batches to evolve the project's harness and the framework. Nothing routes what he finds, so archive has no meaning to give.
- The retro's "Compared with" rows read only this machine's store (`src/retro.rs:204`, `src/db.rs:454-474`). A teammate's runs never show up.
- `DEC-n` is a counter that two developers polishing at once would both bump.

**Left alone:** PRD numbers (they stay, §7), lanes (local and never pushed), per-person config (already user-scope), and handing a run to another machine (a run starts and ends on one machine, settled 2026-10-14).

## 2. Blast radius

- **Whoever runs `/deliver`** (FR-1 to FR-4). Evidence lands in the run dir and never in a commit. Progress lines and checkpoint entries name `cp-{n}/<file>`. The PR's Proof cell says `CP-{n}` instead of a path. No friction file appears anywhere. BLOCK bodies write `~/` paths. The retro compares against tracked retros.
- **arca-api's harness** (FR-2). Its four verify-arca drive specs hardcode `const EVIDENCE = 'docs/features/024-admin-inspection-ux/evidence'`, and `features/provisioning.md:83` points there. They keep writing there, and the lane keeps committing it, until they point at `$FACTORY_EVIDENCE`. That change is arca's, and the owner's.
- **Every repo on the framework** (FR-1, FR-2, FR-5 to FR-7). The skills `polish`, `ux`, `gate`, `slice` and `prune` change, `harvest` is new, and so do the checkpoint, slice and merge contracts, `autonomy.md`, `pr-body.md` and the retro template. Each skill edit costs a crate bump (the bundle.lock test), a `v*` tag and `factory skills update` in each repo.
- **Links into `decisions.md`** (FR-7). arca-api's 9 anchored `decisions.md#dec-n` links (7 in `docs/architecture`, 2 in `docs/reference/pendings`) and the 93 link lines in its frozen archive keep resolving, because each `dec-n` stays an anchor. Its 2 links to the file with no anchor, in `docs/operations`, open the file, as today. Its plain `DEC-n` mentions (4, in architecture, operations and releases) stay records.

  > *Corrected 2026-10-14, adversarial pass:* an earlier draft counted 7 anchored links and missed the 2 in `docs/reference/pendings`.
- **`emilianotisato/factory` on GitHub** (FR-8). The repo is public. `factory upstream post` creates an issue or a comment there only after the scrub check passes and the person read the text, and never inside a run.
- **The CLI.** `factory docs harvest` and `factory upstream` are new. `factory docs archive` also banners and drops now (FR-5). `[source] path` stays, because `factory upgrade` reads it (`src/cli/upgrade.rs:32-39`). No API, webhook or URL changes.

## 3. FR-1 — The closeout files no friction

**Today.** `file_friction` (`src/closeout.rs:416-449`) is called when the run's polish worker settles (`:330`) and when a polish worker settles in the resumed path (`:2139`). Its helpers are `only_placeholder` (`:452-459`) and `friction()` (`:461-471`); tests pin them at `:2535-2560`. The section it copies is the retro template's `## Framework friction → skills repo` (`assets/retro.md:33`). The PR body's Score line already carries the single biggest friction (`assets/pr-body.md:48`).

**Decision, 2026-10-14.** Delete `file_friction` and its two calls. Keep the section reader only if FR-5 reuses it. The retro's Framework friction section is the only home of a run's friction until the owner harvests it (FR-6), and factory hears about it through `factory upstream` (FR-8). The heading becomes `## Framework friction → factory`: the skills live in this repo now. The harvest reads any heading that starts with `## Framework friction`, so arca-api's 10 retros with the old heading still read. `[source] path` stays in the config (`src/config.rs:19,25-27`) for `factory upgrade` and FR-8. The four friction ideas already filed stay ideas; the first harvest here rewrites their `From` lines by hand (FR-6 step 4).

**Accepted cost.** Friction from a client repo reaches factory only when someone harvests that repo. The owner reads retros in batches anyway, and a friction that waits loses nothing.

**Acceptance.** A stub run whose polish fills the Framework friction section, with `[source] path` set to a second checkout, leaves that checkout's `git status --porcelain` empty. `grep -rn file_friction src/` finds nothing.

## 3. FR-2 — Evidence stays in the run dir

**Today.** The checkpoint worker saves evidence "under the harness's artifacts or `{feature dir}/evidence/cp-{n}-*`" (`assets/contract-checkpoint.md:15`), appends the paths to each slice's Progress line (`:16`) and records `evidence: {paths}` in the checkpoint entry (`:17`). The lane commit takes everything not ignored (`src/land.rs:47`), and nothing ignores `evidence/`. No code reads an evidence file: the only mention in `src/` is a comment (`src/docs/check.rs:568`). The run's readers are agents. Polish caught an unproven FR twice because no checkpoint record named a proof for it: in 002 no checkpoint's **Prove** line named FR-20 (`docs/features/002-docs-port/retro.md:27`), and in 004 CP-2's Covers line left out slice 03, so FR-3 and FR-4 never ran (`docs/features/004-skills/retro.md:42`). Those records are the log and the Progress lines, which stay. *Corrected 2026-10-14, adversarial pass:* an earlier draft said both were a Progress line with no evidence path. People review the code (owner, 2026-10-14). Nothing makes a checkpoint look at its own screenshots: the harness standard only requires one to exist (`skills/harness/references/skill-template.md:83`).

**Decision, 2026-10-14.** The driver sets `FACTORY_EVIDENCE=<run dir>/evidence` for every worker, beside `FACTORY_REPO` and `FACTORY_BOARD` (`src/worker.rs:242-243`); the run dir is `<data_dir>/runs/<id>` (`Run::runs_dir`, `src/run.rs:219-220`, joined with the id at `:243`). *Corrected 2026-10-14, adversarial pass:* an earlier draft cited `src/paths.rs:63-72`, which only resolves the data dir. The checkpoint contract saves evidence under `$FACTORY_EVIDENCE/cp-{n}/`, and a harness that saves elsewhere copies what it wrote there. Nothing goes under `docs/`. Progress lines and the checkpoint entry name each file as `cp-{n}/<file>`, never an absolute path. Autonomy's "never record a Prove row without an artifact that exists on disk" (`assets/autonomy.md:57`) still holds: the disk is the run dir's. Nothing goes into the PR either. The Slices table's Proof cell (`assets/pr-body.md:30`) says `CP-{n}: {rows}`, or "no Prove rows: why", and `:32` says evidence stays in the run dir of the machine that ran the build. `skills/prune/SKILL.md:28` stops naming the feature folder. Outside a run, evidence goes to the harness's gitignored artifacts dir, as `skills/harness/SKILL.md:55` already says.

This reverses the brief's "the driver copies it into the PR" and the team-repo idea's "Copied into the PR, not linked". Screenshots need no upload, so `gh --attach` (gh 2.99+) is not used.

**Accepted cost.** Nobody off that machine can open a run's evidence, and it goes when the run dir is pruned. The checkpoint entry keeps what each row proved, in one line, in the tracked log.

**Acceptance.** A stub run whose checkpoint writes `$FACTORY_EVIDENCE/cp-1/row.txt` ends with the file at `<run dir>/evidence/cp-1/row.txt`, no `evidence/` path in `git ls-tree -r` of the feature branch, and a checkpoint entry in `docs log show` naming `cp-1/row.txt`. The stub writes a file for this; today it answers "evidence: none (stub)" (`tests/stubs/claude:277,283`).

## 3. FR-3 — No home paths in tracked docs

**Today.** 42 tracked files under `docs/` hold `/home/<user>` (§1). The leaks scan skips `docs/` by design (`src/leaks.rs:766`). The driver writes absolute run-dir paths into BLOCK bodies that land in the tracked `log.jsonl`: `src/checkpoint.rs:147` and `:228`, `src/closeout.rs:182`, `:1153` and `:1915`, `src/scheduler.rs:2019`. One is tracked so far (`docs/features/003-observatory/log.jsonl`). `docs/.gitignore` holds only `docs.lock`.

**Decision, 2026-10-14.**
- The driver writes any path under the home dir as `~/…` in a log body. The person answering a block is on the same machine, so `~/` still opens.
- `factory docs check` warns `W-HOME-PATH` for a tracked file under `docs/` that holds `/home/<name>/` or `/Users/<name>/`: one line per file, with the first hit's line number. `docs/archive/` is skipped, because it is frozen and cannot be fixed.
- `docs/.gitignore` gains `__pycache__/`.

**Accepted cost.** Today's 42 files warn until the first harvest drops the evidence and the slices; the few left (ideas, `research.md`) are fixed by hand then, in FR-6 step 4. It is a warning, not an error, so the run's preflight still starts (`src/docs/preflight.rs:97-111` refuses on errors only).

**Acceptance.** In a fixture, `factory docs check` reports `W-HOME-PATH` for `docs/features/001-x/slice-01-a.md` holding `/home/x/a`, and nothing for the same line under `docs/archive/`. A stub run blocked by a red gate writes a BLOCK body with `~/` and no `/home/`.

## 3. FR-4 — The retro compares against tracked retros

**Today.** `numbers` (`src/retro.rs:152-221`) builds "Compared with" from `earlier_runs` (`src/db.rs:454-474`), this machine's store, keyed by the repo's canonical path, three runs (`COHORT`, `src/retro.rs:16`). A teammate's runs never appear, and `earlier_runs` has no other caller but its test (`src/db.rs:1290`). Five tracked retros hold a `(this run)` row: 7 cells in 002, 004, 005 and 006 (run, feature, weighted, slices, per slice, lines changed, per line), 9 in 007 (plus comment share and id hits). 001's says "No earlier finished run in this repo." (`docs/features/001-driver/retro.md:264`), and 003's has no Numbers block. *Corrected 2026-10-14, adversarial pass:* an earlier draft said every tracked retro holds a row.

**Decision, 2026-10-14.** The "Compared with" rows are the `(this run)` rows of the three newest tracked retros under `docs/features/*/retro.md` and `docs/archive/*/retro.md` in the tree the retro is written in, this feature's own excluded, and a retro with no `(this run)` row skipped. Newest means the date in the run id (`007-20261015-c8fa`), then the feature number. Each row is copied as written, with `(this run)` dropped, and a 7-cell row gets `—` in the two missing cells. This run's own row still comes from the store (`compare_row`, `src/retro.rs:121`). With no tracked row, the line reads "No earlier retro in this repo." (replacing `NO_EARLIER`, `:15`). `earlier_runs` and its tests go (`src/db.rs:1290`, `src/retro.rs:371`).

**Accepted cost.** A copied row is the figure at its own run, not a recount: 004's own retro says 7,099,681, while the rows in 005, 006 and 007 say 7,369,085. The tracked figure is the one the team shares.

**Acceptance.** In a fixture whose store holds only the 008 run, with tracked retros for 005, 006 and 007, `factory retro <run>` rewrites 008's `retro.md`. Its Compared-with table holds the 008 row from the store, then exactly those three rows, newest first, 005 and 006 padded with `—`.

> *Corrected 2026-10-14, adversarial pass:* an earlier draft said `factory retro` prints the rows with an empty store. It needs the run in the store (`src/cli/retro.rs:23-25`), and it writes the rows into `retro.md`; stdout only says `updated` (`src/cli/retro.rs:39-46`).

## 3. FR-5 — `factory docs harvest`, and archived means harvested

**Today.** `docs archive` (`src/docs/ship.rs:91-110`) git-mvs the folder and sets `status = "archived"`, nothing else. The polish skill closes with `docs ship` or `docs archive` (`skills/polish/SKILL.md:197`), and so does the slice skill (`skills/slice/SKILL.md:155`). The driver ships and never archives (`docs/reference/skills-decisions.md:249`). The board defines archived as "nothing left to do" (`docs/index.md:22,61`, `assets/docs/index.md:22,59`). The convention block `factory init` writes has no line saying feature docs are history (`assets/docs/convention.md`).

**Decision, 2026-10-14.**
- **`factory docs harvest [NNN…]`** prints the batch's lessons, read-only. For each feature: its retro's sections under `# Retro`, without the Numbers block, then each idea in `docs/ideas/` whose `origin` names `PRD-NNN` (`"PRD-007 polish"`, `"factory PRD-007 run …"`), as its id and first sentence. With no numbers, the batch is every `shipped` feature; there is no threshold or nudge, the batch is the owner's call. A named feature that is not `shipped` is refused with its status.
- **`--done`** archives the batch. For each feature it refuses when the folder has uncommitted changes or `docs/archive/<dir>` exists. Then it puts one banner under `prd.md`'s frontmatter: `> **History.** The code, \`docs/reference\` and \`docs/decisions.md\` win over this document.` It rewrites `/home/<name>/` and `/Users/<name>/` to `~/` in the files it keeps. It `git rm`s every tracked file except `prd.md`, `state.toml`, `log.jsonl` and `retro.md`, git-mvs the folder to `docs/archive/`, sets `status = "archived"` and stages everything, so each kept file shows `R`, never `M`: `archive_frozen` (`src/docs/check.rs:694-719`) and the pre-commit hook (`src/docs/hook.rs:18-21`) refuse an `M` under `docs/archive/`. It prints what it dropped and leaves the commit to its caller.

  > *Corrected 2026-10-15, slice recon:* "each kept file shows `R`" is wrong for the check. `archive_frozen` runs `git status --porcelain -- docs/archive`, and under that pathspec a moved and edited file shows `A`, never `R`; without it, a small edited file can show `D` plus `A`. It is never `M` as long as staging comes after every edit, which is what the check and the hook need.
- **`factory docs archive NNN`** runs the same moves as `harvest NNN --done`, so an archived folder has one shape.
- **The closeout ships, never archives.** `skills/polish/SKILL.md:197` and `skills/slice/SKILL.md:155` say `factory docs ship`. `assets/pr-body.md:10` loses its archive sentence. `docs/index.md` and `assets/docs/index.md` define archived by what the folder holds: moved, records only, history banner on. The definition adds that `/harvest` routes the lessons first. No verb can check that the routing happened, so the definition does not promise it.

  > *Corrected 2026-10-14, adversarial pass:* an earlier draft defined archived as "lessons routed" while `docs archive` and `--done` move without routing. The definition now states what the verbs guarantee.
- **The convention line.** `assets/docs/convention.md` gains: "`docs/features` and `docs/archive` are history: the code, `docs/reference` and `docs/decisions.md` win." New repos get it from `factory init`. `convention()` appends only when the heading is absent (`src/docs/scaffold.rs:137-197`), so an existing repo gets the line from its first harvest (FR-6).

**Accepted cost.** Everything else in the folder goes to git history with the slices: qa-guides, `research.md` (006), 001's recordings and ctx-jev replay. The skill moves a file worth keeping to `docs/reference` first. A slice's plan-versus-actual story leaves the tree with it, and the retro analysis reads it before `--done`.

**Acceptance.** In a fixture with two shipped features, `factory docs harvest` prints both retros' sections and the ideas whose origin names them. `factory docs harvest 001 --done` leaves `docs/archive/001-x/` holding exactly the four files, the banner right after the frontmatter, no `/home/` in them, and status archived. Then `factory docs check` reports no `E-ARCHIVE-MODIFIED`, and `git commit` passes the hook `factory docs hook` installs.

## 3. FR-6 — The `/harvest` skill

**Today.** No step reads shipped features for lessons. The brainstorm's learn loop planned one and never built it (see the harvest idea above).

**Decision, 2026-10-14.** A new skill, `skills/harvest/SKILL.md`, in the bundle and in the convention block's skill list. It is attended only, with the owner present, and never a run role. Its steps:
1. `factory docs harvest [NNN…]` for the lessons.
2. Each lesson, with the owner, by `AskUserQuestion` with the recommendation first, goes to one of four places. **The project**: a diff here, in the harness, the gate, `CLAUDE.md`, `docs/reference`, `docs/ux-rubric.md` or a `decisions.md` entry. **The framework in this project**: a `docs/project.toml` knob. **Factory itself**: `factory upstream` (FR-8), or `factory docs idea add` when this repo is the factory checkout. **Nowhere**: dropped, said aloud.
3. Compact `docs/decisions.md` by FR-7's rules, as one diff the owner reviews.
4. On a repo's first harvest: add FR-5's convention line to `CLAUDE.md` and `AGENTS.md`, update `docs/index.md`'s archived definition, and rewrite by hand the home paths `W-HOME-PATH` still reports outside the batch (the friction ideas' `From` lines, `research.md`).
5. Move a record worth keeping to `docs/reference`, then run `factory docs harvest NNN… --done`.
6. Commit on a new `harvest/<date>` branch when on the default branch, else on the current one. Never push.

The first harvest is also the migration (owner, 2026-10-14). In this repo it runs after this feature merges, with the new binary. In arca-api it runs on the owner's go, after its verify-arca specs point at `$FACTORY_EVIDENCE` (§2).

**Accepted cost.** Lessons wait as long as the owner waits. The routing is judgment, so no run can do it.

**Acceptance.** The skill renders through `factory init` and passes the bundle test. The owner's first `/harvest` here is the Human row in §5.

## 3. FR-7 — Decisions without numbers

**Today.** `docs/decisions.md:7-11` rules that `DEC-n` is monotonic and a reversed entry is superseded, never deleted; its 3 entries are `### DEC-n — {question} · {date} · PRD-NNN polish` with no anchor line (`:15`, `:22`, `:29`). Polish writes them by the template at `skills/polish/SKILL.md:175-184` and the rule at `:186`, and cites `DEC-n` at `:22`, `:110-111`, `:147`, `:159` and `:196`; `:87`, `:163`, `:173` and `:210` say "DEC" or "`docs/decisions.md`" without a number. Other mentions: `skills/ux/SKILL.md:42` and `assets/autonomy.md:17,38` (`DEC-n`); `assets/contract-slice.md:17`, `assets/contract-merge.md:18` and `assets/pr-body.md:16` ("a DEC", "DEC candidates"); `skills/gate/SKILL.md:40` (the file only); `docs/reference/skills-decisions.md:3`. arca-api has 21 h2 entries, each under an `<a id="dec-n"></a>` line, and 15 mentions outside it and its features (§2).

> *Corrected 2026-10-14, adversarial pass:* an earlier draft listed `:163`, `:173`, `:210` and the two contracts as `DEC-n` citations, and counted 11 mentions in arca-api. All of them still change wording; only the counts were wrong. Its PRD-024 defines its own local DEC-1 to DEC-8 (`docs/features/024-admin-inspection-ux/prd.md:165-172`).

**Decision, 2026-10-14.**
- An entry is `## {Short name}` with **Chose**, **Over**, **Because** and **Applies when**, and no number, date or origin. Its link is the heading's own GitHub anchor, the slug.
- A renamed or merged entry keeps its old slugs and its `dec-n` as `<a id="…"></a>` lines above the heading, so every existing link resolves.
- A reversed or merged loser folds into the winner's **Over**.
- An entry whose **Applies when** no longer exists in the code stays whole. An **Over** is never lost. *Cut 2026-10-14:* a `## Retired` list that shrank a dead entry to one line; §7.
- A decision that is a domain fact moves to the doc that owns the topic, leaving its anchors on a one-line pointer.
- Polish writes new entries in this form and every skill cites `decisions.md#{slug}`. `autonomy.md`'s "does the PRD, the slice, or a `DEC-n` settle this" reads "or `docs/decisions.md`", and the PR body's "DEC candidates" becomes "Decision candidates".
- Records keep their ids: logs, PR bodies, retros, the archive and `skills-decisions.md`. The leaks rule keeps `DEC-\d+` (`src/leaks.rs:43,157`).

Converting an existing `decisions.md` is the harvest skill's step 3. Nothing outside `decisions.md` is rewritten, so arca-api's PRD-local DEC ids are never mismapped.

**Accepted cost.** A hand edit can still delete an anchor and break a link, and nothing checks it (§7). arca-api's `openapi/public_text_test.go:11` guard regex knows `\bDEC-\d` and not slugs; teaching it is arca's change.

**Acceptance.** No skill or asset tells an agent to write or cite `DEC-n`: a bundle test greps `skills/` and `assets/` for `DEC-` and allows only the leaks pattern and history lines. The polish template shows the new form. The first conversion here (3 entries, each keeping `dec-1` to `dec-3`) is part of the FR-6 Human row.

## 3. FR-8 — `factory upstream`

**Today.** The only path from a client repo to factory is `file_friction` (FR-1), which ignores that `emilianotisato/factory` is public.

**Decision, 2026-10-14.** The skill rewrites a lesson in general terms; the verb is the last check before anything leaves. The draft is a markdown file: its first `# ` line is the title, the rest the body.
- **`factory upstream check <draft>`** exits 1 with one line per hit for: an absolute home path; the current repo's directory name, or its `origin` remote's owner or name; an email address. It then lists up to five open issues on `emilianotisato/factory` matching the title (`gh issue list --state open --search`), so the person can comment on a match by hand instead of posting a repeat. A failed search (no `gh`, no login) prints one line and is not a hit.
- **`factory upstream post <draft>`** runs the check again and refuses on a hit. With `[source] path` set to a factory checkout, it adds the draft as an idea there and commits only that file. Otherwise it runs `gh issue create`. *Cut 2026-10-14:* `--on N`, a +1 comment on a matching issue; §7.
- Both refuse when `FACTORY_BOARD` is set (`src/worker.rs:55`): never inside a run, where the driver owns every GitHub call (DEC-2 in this repo's `decisions.md`).

**Accepted cost.** The name check is a floor. A customer name that is not in the repo or remote name passes it; the person reading the draft in step 2 of FR-6 is the real check.

**Acceptance.** Against the stub `gh` (`tests/stubs/gh`): a draft holding `/home/x/` or the repo name exits 1 naming the hit; a clean draft with no `[source]` calls `gh issue create --repo emilianotisato/factory`; with `[source]` set, one committed idea appears in that checkout and `gh` creates no issue; with `FACTORY_BOARD` set, both refuse.

> *Corrected 2026-10-14, adversarial pass:* an earlier draft said `gh` is never called with `[source]` set, while `post` reruns `check`, whose issue search calls `gh issue list`.

## 3. FR-9 — RETIRED (2026-10-14)

The owner's call at the cut, 2026-10-14: no pain asks for it, because the repo has one author so far. It proposed that `factory docs new` fill `author` from `git config user.name`; today it writes `author: _(name)_` (`src/docs/scaffold.rs:486` lists the key) and never fills it. The recon went to `docs/ideas/2026-10-15-fill-a-prd-s-author-from-git.md`, so a future PRD does not redo it. §7 has the Not handled line.

## 4. Decisions log

- ~~**OPEN-1** — What replaces `file_friction` at closeout?~~ **Nothing** (2026-10-14). The retro section is the only home until the harvest. FR-1.
- ~~**OPEN-2** — How do screenshots reach the PR?~~ **They don't** (2026-10-14). Evidence is for the run's agents; people review the code. FR-2.
- ~~**OPEN-3** — What makes a harvest batch?~~ **The owner's call**, no threshold or nudge: every shipped feature, or the numbers named (2026-10-14). FR-5.
- ~~**OPEN-4** — Verb or skill?~~ **The verb moves, the skill routes** (2026-10-14). FR-5, FR-6, FR-8.
- ~~**OPEN-5** — Where do the retro's comparison rows come from?~~ **The tracked retros** (2026-10-14). FR-4.
- ~~**OPEN-6** — PRD numbers across clones?~~ **Left out** (2026-10-14). §7.
- ~~**OPEN-7** — How do existing repos migrate?~~ **Their first harvest** (2026-10-14). FR-6.
- ~~**OPEN-8** — Warn on home paths in tracked docs?~~ **Yes, `W-HOME-PATH`** (2026-10-14). FR-3.
- ~~**OPEN-9** — Drop PRD numbers for slugs?~~ **No** (2026-10-14). §7.

## 5. QA plan

**Gate**: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` *(declared in project.toml)*
**Surface**: CLI (`factory docs harvest`, `factory upstream`, `factory docs check`, `factory retro`, a `factory run`), plus the skill texts.
**Harness**: the stub harness in `cargo test` (`tests/stubs/claude`, `tests/stubs/gh`, the v3 fixture). No UI.
**Seed state**: the test fixtures; none beyond them.

| FR | Proof on the real surface | Evidence | Human? |
|---|---|---|---|
| FR-1 | run: a stub run with a filled Framework friction section and `[source] path` set to a second checkout; `git status --porcelain` in that checkout | transcript | no |
| FR-2 | run: a stub run whose checkpoint writes `$FACTORY_EVIDENCE/cp-1/row.txt`; `ls` the run dir, `git ls-tree -r` the feature branch, `factory docs log show` | transcript | no |
| FR-3 | run: `factory docs check` on a fixture with a home path under `docs/features` and under `docs/archive`; a stub run blocked by a red gate, then `grep /home/` its log | transcript | no |
| FR-4 | run: `factory retro <run>` on a fixture whose store holds only that run, with three tracked retros (7 and 9 cells); read back the Compared-with table in `retro.md` | transcript | no |
| FR-5 | run: `factory docs harvest`, then `--done`, on a fixture with two shipped features; `factory docs check`; `git commit` through the installed hook | transcript | no |
| FR-6 | The owner's first `/harvest` here after the merge, over 001 to 007: the lessons routed, `decisions.md` converted, the convention line added, the batch archived | the `harvest/<date>` diff | **yes**: routing is judgment |
| FR-7 | run: the bundle test that no skill or asset asks for `DEC-n`; the conversion itself rides on the FR-6 row | transcript | no |
| FR-8 | run: `factory upstream check` and `post` against the stub `gh`, with and without `[source]`, and with `FACTORY_BOARD` set | transcript | no |
| FR-9 | retired | — | — |

**Who builds it.** A `factory` run builds every FR, and nothing needs a human mid-run. The `/harvest` skill is text the run writes; using it is attended by nature. `factory upstream` only ever meets the stub `gh` in the run.

**Build order.** The driver that builds this is the installed one, so this feature's own checkpoints still commit evidence under `docs/features/008-team-and-harvest/evidence/`. The first harvest after the merge drops it.

## 6. Slices

Eight slices on `feat/team-and-harvest`: one branch, one PR, one commit per slice, Prove at the checkpoints. `factory docs status` is the board.

- **01, no friction filing** (FR-1). The closeout writes nothing into the `[source]` checkout. The retro's Framework friction section, renamed `→ factory`, is where a run's friction waits for a harvest. First because the owner wants FR-1 fixed first (§1).
- **02, home paths** (FR-3). One matcher for `/home/<name>/` and `/Users/<name>/`. The driver writes `~/` in the log entries it adds, `factory docs check` warns `W-HOME-PATH`, and `docs/.gitignore` ignores `__pycache__/`.
- **03, evidence in the run dir** (FR-2). `FACTORY_EVIDENCE` for every worker, the checkpoint contract saving under it, and the PR body's Proof cell naming the checkpoint.
- **04, `factory docs harvest` lists a batch's lessons** (FR-5, the read). The verb, read-only: retro sections without the Numbers block, then the ideas whose origin names the feature.
- **05, `--done` and `docs archive`** (FR-5, the moves and texts). One function moves a batch to its archived shape, staged and uncommitted. The closeout ships, never archives, and the convention gains its history line. ⚠ The highest risk: the order of the moves. One edit after the staging and the hook refuses the commit.
- **06, `factory upstream`** (FR-8). `check` and `post`, the last gate before anything leaves a client repo for the public `emilianotisato/factory`.
- **07, the retro compares tracked retros** (FR-4). The Compared-with rows come from the tree, not this machine's store; `earlier_runs` goes.
- **08, the skills and 0.6.0** (FR-6, FR-7, and the skill lines of FR-2 and FR-5). The `/harvest` skill, decisions without numbers, the closeout and prune lines, the `DEC-` bundle test, and the bump. Every `skills/` edit of this feature sits here, so one bump and one relock cover them.

**Order.** The spine is 02 and 04: 02's matcher is what 05 rewrites kept files with and what 06 reports hits with, and 04's verb is what 05 adds `--done` to. 01, 02, 03, 04 and 07 depend on nothing; with `lanes = 4`, 01 to 04 start together and 07 takes the first free lane. 06 waits for 02 only, 05 for 02 and 04, and they run side by side. 08 waits for 05 and 06, because the skill runs both verbs, and goes last. Width 4, then 2 or 3, then 1.

**Shared files, in different hunks.** `tests/stubs/claude`: 01 at `:52-53,366`, 03 at `:241-290`. `src/closeout.rs` and `tests/closeout.rs`: 01 in the friction code and `source_repo`, 07 at `:996` and the Compared-with tests. `tests/checkpoints.rs`: 02 and 03 each add a test. `tests/docs_v3.rs`: 02 at `:212-215`, 04 at `:318-356`. `assets/pr-body.md`: 03 at `:30,32`, 05 at `:10`, 08 at `:16`. The rest is sequential by the graph: `src/docs/check.rs` 02 then 05, `src/cli/docs.rs` and `tests/docs_lifecycle.rs` 04 then 05, `assets/docs/convention.md` 05 then 08.

**Checkpoints** (`checkpoint_every = 2`). Four, each covering the next two slices to finish, the last after 08. 02 and 04 land early among the first four, so the matcher and the verb are proven before 05 and 06 build on them.

**The installed driver builds this run**, and it predates every part of this feature. Install the current `main` first (`cargo install --path .`, 0.5.0), for 007's polish. So this run still commits its checkpoint evidence under this folder's `evidence/` (§5 Build order). With `[source]` set on this machine, its closeout files one last friction idea into this checkout's `docs/ideas/`, untracked: the first `/harvest` routes or drops it. This repo's `.claude/skills` link into `skills/`, so the polish worker reads 08's polish text once 08 has landed. 0.6.0 is tagged after the merge, not in the run; every repo then needs `factory skills update`.

**`qa-guide.md`** holds the one Human row, FR-6: the owner's first `/harvest` here, after the merge, with the 0.6.0 binary.

## 7. Non-goals

- **PRD numbers stay.** Every verb already takes a slug or a folder name (`src/docs/board.rs:240-252`). The number orders the board, gives short cross-references ("PRD-007 AD-6"), and lets one leaks pattern catch plan ids in code (`src/leaks.rs:43`). Dropping it would touch 1,551 mentions here and 444 in arca-api. Decided 2026-10-14. Revisit if two developers collide in practice.
- **Not handled: PRD number allocation across clones.** Accepted 2026-10-14: no second developer yet, and `E-NUM-DUPLICATE` (`src/docs/check.rs:115-129`) plus the run's preflight refusal catch a duplicate. Known: `renumber` resolves the first folder with the number, and its text replace of `PRD-old` would rewrite the other feature's mentions too (`src/docs/renumber.rs:62-131`). Revisit at the first collision.
- **Lanes do not collide across machines.** They are local detached worktrees under `<data_dir>/lanes/`, and their `refs/factory/NNN/…` refs are never pushed; only `feat/<slug>` is.
- **Per-person settings are done.** `[review]` and `[source]` already live in `~/.config/factory/config.toml`, and `models.md` holds nothing personal.
- **No evidence in the PR**, text or image, and no `gh --attach` (2026-10-14).
- **Not handled: a checkpoint looking at its own screenshots.** Accepted 2026-10-14: the pass comes from assertions and the ARIA snapshot, and only the UX pass in polish judges images (`skills/ux/SKILL.md:18,56`). Revisit if a run proves a screen that was visibly wrong.
- **Not handled: the driver keeping a stray `evidence/` out of the lane commit.** Accepted 2026-10-14: after FR-2, arca-api's verify-arca specs are the only known writer, and the owner points them at `$FACTORY_EVIDENCE`. Revisit if a run commits an `evidence/` path after that.
- **Not handled: checking that links into `decisions.md` resolve.** Accepted 2026-10-14: FR-7 keeps every old anchor, and no broken link has been seen. Revisit on the first one.
- **Not handled: filling a PRD's author from git** (FR-9). Cut 2026-10-14: one author so far. Revisit when a second person writes a PRD.
- **Not handled: a +1 comment from `factory upstream`.** Cut 2026-10-14: no duplicate has been filed yet, and `check` lists the matches for the person to comment by hand. Revisit if a duplicate issue gets filed.
- **Not handled: retiring a dead decision.** Cut 2026-10-14: no entry is dead yet, so one stays whole until the owner decides. Revisit when the first harvest finds one.
- **Not handled: scratch paths** (`/tmp/claude-…`, `/mnt/devcache/…`) in tracked docs. `W-HOME-PATH` looks for home paths only.
- **No mid-run handoff** between machines or people (settled 2026-10-14).
- **Related, not in scope:** `2026-10-12-cross-repo-ideas-scanner`, `2026-10-13-mark-partial-figures-in-the-retro-s-numbers-table`, `2026-10-13-refresh-numbers-after-the-ci-rounds`.

## 8. Verification notes

Checked against `main` at `33ff80b`, 2026-10-14. **Re-verify at slice entry.**

- `src/closeout.rs:416-449` — `file_friction`; called at `:330` (polish settles in the run) and `:2139` (a polish worker settles); helpers `only_placeholder` `:452-459`, `friction()` `:461-471`; tests `:2535-2560`.
- `assets/retro.md:33` — `## Framework friction → skills repo`; also pinned by `tests/stubs/claude:53,366` and `src/closeout.rs:2543,2545`. `friction()` matches any line starting `## Framework friction` (`src/closeout.rs:464`). *Corrected 2026-10-14, adversarial pass:* an earlier draft also named `tests/closeout.rs`; it never holds the heading.
- `src/config.rs:19,25-27` — `[source]` is `SourceRepo { path }`; `src/cli/upgrade.rs:32-39` reads it.
- `docs/ideas/2026-10-14-friction-factory-prd-005.md:13`, `…-prd-006.md:12`, `2026-10-15-friction-factory-prd-007.md:14` — `From /home/emiliano/…` lines.
- `assets/contract-checkpoint.md:15-17,25` — evidence to `{feature dir}/evidence/cp-{n}-*`, paths into Progress and the checkpoint entry.
- `src/land.rs:47` — the lane commit is `git add -A`; nothing ignores `evidence/`.
- `src/docs/check.rs:568` — the only `evidence` mention in `src/`, a comment.
- `src/worker.rs:52,55` declare `FACTORY_REPO` and `FACTORY_BOARD`, `:242-243` set them for every worker; `:207` runs workers with `bypassPermissions`, so writing outside the lane works.
- `src/paths.rs:12-18,63-72` — the data dir, `~/.local/share/factory` by default; `src/run.rs:219-220,243` — run dir `<data_dir>/runs/<id>`.
- `tests/stubs/claude:277,283` — the stub checkpoint reports "evidence: none (stub)".
- `skills/harness/references/skill-template.md:83` — UI proof = ARIA snapshot plus a screenshot; nothing asks to judge the image.
- `assets/autonomy.md:57` — never record a Prove row without an artifact on disk.
- `assets/pr-body.md:10,16,30,32,48` — the archive sentence, DEC candidates, the Proof cell, the evidence line, the Score line.
- `src/leaks.rs:43,157` — the id pattern includes `PRD` and `DEC`; `:766` excludes `docs/` from the scan.
- `src/checkpoint.rs:147,228`, `src/closeout.rs:182,1153,1915`, `src/scheduler.rs:2019` — BLOCK bodies with absolute run-dir paths.
- `docs/.gitignore` — holds `docs.lock` only.
- `src/retro.rs:15,16,121,152-221,204` — `NO_EARLIER`, `COHORT = 3`, `compare_row`, `numbers`, the `earlier_runs` call; `src/db.rs:454-474` — `earlier_runs`, keyed by repo path, no other caller but its test (`:1290`); `src/cli/retro.rs:23-25,39` — `factory retro` needs the run in the store and writes `retro.md`.
- Tracked `(this run)` rows: 7 cells in 002, 004, 005, 006; 9 cells in 007 (`docs/features/007-polish-in-the-binary/retro.md:73`). 004's own row is 7,099,681; 005, 006 and 007 compare against 7,369,085.
- `src/docs/ship.rs:91-110` — `archive` moves and sets the status only.
- `src/docs/check.rs:694-719` — `E-ARCHIVE-MODIFIED` on `M` in either porcelain column; `R`, `A`, `??` pass.
- `src/docs/hook.rs:18-21` — the pre-commit hook refuses staged `M` under `docs/archive/`.
- `src/docs/scaffold.rs:137-197` — `convention()` appends only when the heading is absent; `:486` lists `author` among the frontmatter keys.
- `src/docs/ideas.rs:25,55,154` — ideas carry `origin`; in `docs/ideas/` it reads `"PRD-007 polish"` or `"factory PRD-007 run 007-20261015-c8fa"`.
- `src/docs/board.rs:240-252` — `resolve` takes a number, a slug or a folder name.
- `skills/polish/SKILL.md:22,87,110,111,147,159,163,173,175-184,186,196,197,210` — DEC lookups, citations, the template, the numbering rule, the closeout's ship-or-archive, the report.
- `skills/slice/SKILL.md:155`, `skills/ux/SKILL.md:42`, `skills/gate/SKILL.md:40`, `skills/prune/SKILL.md:28`, `assets/autonomy.md:17,23,38,45`, `assets/contract-slice.md:17`, `assets/contract-merge.md:18`.
- `docs/decisions.md:7-11,15,22,29` — the rules and three `### DEC-n` entries without anchor lines.
- `docs/index.md:22,61`, `assets/docs/index.md:22,59` — archived = nothing left to do.
- `docs/reference/skills-decisions.md:249` — "Ship, never archive" for the driver.
- `src/docs/check.rs:115-129` — `E-NUM-DUPLICATE`; `src/docs/preflight.rs:97-111` refuses on check errors; `src/docs/renumber.rs:62-131` — the first-match resolve and the text replace.
- `src/closeout.rs:1203` — `branch_guard` starts here; PRD-007 slice-04 says `:1109-1139`.
- arca-api (`~/Projects/Prokey/arca-api`): 120 tracked evidence files (017: 6, 019: 21, 022: 13, 024: 80); four verify-arca specs hardcode the 024 evidence path, and `features/provisioning.md:83` points there; `decisions.md` has 21 entries, each under `<a id="dec-n"></a>`; 9 anchored links (7 in `docs/architecture`, 2 in `docs/reference/pendings`), 2 unanchored links in `docs/operations`, 4 plain mentions; 93 link lines in `docs/archive`; `docs/features/024-admin-inspection-ux/prd.md:165-172` defines PRD-local DEC-1 to DEC-8; `openapi/public_text_test.go:11` guards `\bDEC-\d`; 6 shipped features unarchived; 10 retros with the old friction heading.
- `gh` 2.102.0 on this machine has `--attach` on `pr create`, `pr edit` and `pr comment`; unused (FR-2).
- The installed `factory` is 0.4.0; `Cargo.toml` is 0.5.0.
