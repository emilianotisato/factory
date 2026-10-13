---
number: 004
slug: skills
title: "The skill family ships from this repo"
type: feature
created: 2026-10-13
author: Emiliano Tisato
summary: |
  **Agreed 2026-10-13** (three interview rounds, eleven forks; Cursor's auth had expired, so the adversarial pass was a fresh Claude agent: 12 findings, all corrected inline, four of them breaking). The eight family skills (`prd`, `slice`, `deliver`, `polish`, `gate`, `harness`, `prune`, `ux`) plus one shared `models.md` move from the Dropbox repo into `skills/` here and are embedded in the binary. `factory init` renders them byte-identical into a repo's `.claude/skills/` (Codex reads them through `.agents/skills/` symlinks, Cursor reads `.claude/skills/` directly), writes a manifest with the crate version and a sha256 per file, appends the convention block to `CLAUDE.md`/`AGENTS.md` and prints the commit line. `docs doctor` and run preflight print one of `current`, `none`, `behind`, `ahead`, `differs`, plus `hand-edited` or `missing` files; `factory skills update` re-renders, refuses to downgrade and refuses over a hand edit. A gate test forces a version bump on every skill change. Two retro pains become binary commands: `factory review second` (the cross-vendor second opinion, auth probed first, ~600-line passes, streamed so a timeout keeps its findings) and closeout filing each retro's `Framework friction` section as an idea in this repo. `cargo-dist` builds Linux and macOS releases on a tag and `factory upgrade` installs them. No overlays, no Windows, no store row for reviews. The cut (2026-10-13) kept the convention block, the Codex symlinks, the bump test, `--doc`, `--others`, the release workflow and `upgrade --check`; it retired the collision refusal, `init`/`update` committing, `--force`, `--dry` and the unfiled-friction line. **Sliced 2026-10-13, seven slices:** 01 the port and the embedded bundle (spine) and 05 friction to an idea (independent) ▲ · 02 `factory init` and 04 `factory review second` (live run at this checkpoint) ▲ · 03 doctor, preflight and `skills update`, and 07 the bundle lock and the bump to 0.2.0 ▲ · 06 releases and `factory upgrade` ▲. Branches 02 → 03 → 06 and 04 → 07. `qa-guide.md` holds the FR-6, FR-8 and FR-2 owner checks; after the merge the owner tags `v0.2.0`.
from_idea: "2026-10-13-the-skill-family-lives-in-this-repo-factory-skills"
---

# PRD-004 — The skill family ships from this repo

Graduated from the idea `2026-10-13-the-skill-family-lives-in-this-repo-factory-skills` (its last text is in git at `39c8ad4`, folded in below). The per-skill review that preceded it ran 2026-10-13 and ended with all eight names live in `~/.claude/skills`; `docs/reference/skills-decisions.md` holds the older decisions.

## 1. Why now

**Bugs the owner hit.**

- **The retro-to-skill loop has no destination.** Three retros (`001-driver/retro.md`, `002-docs-port/retro.md`, `003-observatory/retro.md`, each under `## Framework friction → skills repo`) proposed about twelve concrete skill edits. None landed in the Dropbox repo until the owner ran a review by hand on 2026-10-13. The section exists in the template (`assets/retro.md:33`) and nothing reads it.
- **The second opinion fails silently.** 001: three Cursor passes of 1,000 to 1,400 lines timed out at 900 s with nothing. 002: Cursor's auth had expired and the only signal was exit 1 after the review. 003: a 31-file, 3.8k-line branch produced nothing before the kill. The skills carry a prose stopgap (`polish/SKILL.md:54-56`: 600-line passes, probe `cursor-agent --list-models` first) that a model re-derives every run.
- **A stale binary writes stale docs with no signal.** Today, in this session: `~/.cargo/bin/factory` was built at 09:36; `assets/docs/prd.scaffold.md` changed at 13:29 (`248c5df`, `/slice`) and 13:35 (`141e27c`, `## 2. Blast radius`); `factory docs new skills` scaffolded this PRD with `## 2. Users affected` and `/prd-to-tasks`. Nothing said the binary was behind the repo. The skills will have the same problem the day they live in repos: a repo rendered by one binary, read by another.
- **A folder that was eliminated is still load-bearing.** `feature-docs/` was retired on 2026-10-13 (Dropbox `014e28f`), yet `models.md` inside it is linked eight times from five family files (`prd/SKILL.md:15,99`, `slice/SKILL.md:17`, `polish/SKILL.md:17,52,60`, `harness/references/maintain.md:19`, `ux/SKILL.md:18`), and the stale `feature-docs/autonomy.md` differs from `assets/autonomy.md` (one row: the harness major-upgrade block).

**Improvements.**

- Another developer, or a second machine, gets the family by installing the binary, not by cloning a Dropbox folder with no remote and symlinking it. Nobody needs Rust installed to use `factory`.
- Codex and Cursor sessions in a repo read the same skills Claude Code does.

**Left alone, deliberately.** Stack profiles and overlays (the README's "rendered from a profile plus overlays" is a promise this PRD corrects, §3), the learn loop, Windows, `/lanes`, and every non-family skill in Dropbox (`caveman`, `codex`, `self-learning`, `deploying-laravel-cloud`, the Omarchy ones).

## 2. Blast radius

- **Every Claude Code session in an owner repo** that types `/prd`, `/slice`, `/polish`, `/gate`, `/harness`, `/ux`, `/prune` or `/deliver`. Today those resolve from `~/.claude/skills` → `~/Dropbox/dev/claude-code-skills`. After this ships they resolve from the repo's `.claude/skills/`. Until this repo's own merge, sessions here keep reading the Dropbox copies, so the port must not change a skill's meaning (FR-1).
- **The polish worker.** `src/pack.rs:484` spawns the prompt `/polish`, which Claude Code resolves from the skills dirs inside `-p` (`src/pack.rs:412`). After `factory init` in a repo the project copy is what the worker reads. Verify at slice entry which copy wins when a user-dir skill has the same name; the owner's machine will have none after the migration, so the question only matters for a third party. *Verified at slicing, 2026-10-13 (`code.claude.com/docs/en/skills`): "Enterprise over personal, and personal over project." Until the owner deletes the eight Dropbox folders, every session on this machine, this feature's own polish worker included, reads the Dropbox copies, not `skills/` or a rendered `.claude/skills/`. A third party with a same-named user skill keeps theirs. §12's FR-6 live row moved off the polish worker for this reason.*
- **`factory docs init` callers.** `/prd` preflight and `/gate` route a repo off the framework to `factory docs init`; its output gains the skills lines and the convention block (FR-2). `src/cli/docs.rs:240` routes an uninitialised repo to it; unchanged.
- **The harness detector** (`src/docs/gate.rs:411-418`) globs `.claude/skills/verify-*/SKILL.md`. The family adds sibling folders; no family name starts with `verify-`, so it is unaffected.
- **`~/.config/factory/config.toml`** gains `[source]` and `[review]` tables (FR-6, FR-7, FR-8). `src/config.rs:1-3`: an unknown key is a warning, never an error, so an older binary tolerates the new keys.
- **GitHub.** A `release.yml` workflow and `v*` tags on the public repo `emilianotisato/factory`; macOS runner minutes per tag. The existing `ci.yml` is untouched.
- **The owner's machine after the merge.** Eight folders leave the Dropbox repo and `~/.claude/skills/deliver` (a symlink into this repo) goes; the `~/.claude/skills` symlink itself stays for the other skills. Outside an initialised repo `/prd` no longer exists; that is the design (§7).

## 3. FR-1 — The family lives in `skills/` here and is embedded in the binary

**Today.** `skills/deliver/SKILL.md` is the one family skill in this repo, symlinked out as `~/.claude/skills/deliver`. The other seven live in `~/Dropbox/dev/claude-code-skills` (git, no remote): `prd/SKILL.md`, `slice/SKILL.md`, `polish/SKILL.md`, `gate/SKILL.md` + `references/{stacks,tiers}.md`, `harness/SKILL.md` + `references/{drivers,feature-map,maintain,skill-template}.md`, `prune/SKILL.md` + `references/{adapters,rules}.md`, `ux/SKILL.md` + `references/{inspection,rubric}.md`, 163 KB in 17 files (`wc -c`, 2026-10-13), plus `feature-docs/models.md` (8 KB). The binary already embeds its assets one by one with `include_str!` (`src/docs/scaffold.rs:16-20`, `src/pack.rs:23-35`).

**Decisions, 2026-10-13.**

- **Members: the eight plus `_shared/models.md`.** Chosen over "the eight plus `self-learning`" (it carries Dropbox-era paths and no retro ever touched it) and over inlining `models.md` into each skill (eight copies of the roles table the review just deduplicated). `/lanes` is a README promise with no skill behind it; it joins when its PRD lands.
- **Layout: `skills/<name>/…` and `skills/_shared/models.md`.** The eight links `../feature-docs/models.md` become `../_shared/models.md` (`../../_shared/models.md` from `harness/references/maintain.md`). A folder without `SKILL.md` is not a skill to Claude Code, Codex or Cursor, so `_shared` renders into `.claude/skills/_shared/` harmlessly.
- **`models.md` shrinks to the roles.** The `## Second opinion, specifically` invocation block, the Cursor path, the default model and the rate-limit note move into FR-6 and `config.toml`; the file keeps the four roles, the bar rule and the preconditions. Nothing personal is left in the family, which is what makes FR-2 byte-identical rendering possible.
- **Embedded as a bundle.** A const list of `(relative path, content)` for every file under `skills/`, generated by `build.rs` or listed by hand; the slicer picks. Either way a gate test walks `skills/` and fails when a file on disk is missing from the bundle, so a new reference file cannot ship unembedded. *Clarified at slicing, 2026-10-13: the bundle is every file under the eight skill folders and `skills/_shared/`. `skills/README.md` describes the source folder and `skills/bundle.lock` (FR-5) records the bundle's own hash, so neither is bundled, rendered or hashed; the walk skips both by name. The slicer picked the hand-written list (`include_str!`, as `src/pack.rs:23-35` does) over `build.rs`.*
- **`assets/autonomy.md` is the only autonomy copy.** The Dropbox `feature-docs/autonomy.md` is not shipped; packs embed the asset (`src/pack.rs:30`).
- **The Dropbox repo stops being the source for these eight.** The port copies all seven folders at Dropbox HEAD `33796be`. *An earlier draft listed a per-skill sha (the commit each review landed in: `432005f` polish, `2b29e14` prd, `e85e4a4` slice, …); the adversarial pass on 2026-10-13 showed the later renames (`1c4f47b`, `3596fc2`, `33796be`) touched polish, prd and slice after those commits, so copying at the per-skill sha would ship `/guardrails` and `/test-cleanup` names. HEAD is the reviewed text.* Only the three edits this PRD requires change meaning: the `models.md` links, the second-opinion paragraphs that now call FR-6, and the README tables.
- **README corrections.** `README.md:36` says skills are "rendered from a profile plus overlays" and lists `/lanes`; after this PRD it says byte-identical rendering with a manifest, and `/lanes` is marked planned. `skills/README.md` lists the eight.

**Accepted cost.** A skill edit now costs a commit here, a version bump (FR-5), a release (FR-8) and `factory skills update` in each repo, where it used to be one file save in Dropbox. That is the price of a copy per repo that a binary can check.

**Acceptance.**

- `skills/` holds the eight folders and `_shared/models.md`; `grep -r feature-docs skills/` is empty.
- `cargo test` fails when a file under `skills/` is absent from the embedded bundle.
- `models.md` contains no shell invocation, no binary path and no third-party model name (`grok-…`, `gpt-…`). The Claude tier words in the roles table (`opus`, `fable`, `sonnet`, `haiku`) stay: they are roles, not a machine's setup. *Corrected 2026-10-13 after the adversarial pass: the earlier line said "no model name", which the kept roles table fails.*

## 4. FR-2 — `factory init` renders the family into a repo and commits it

**Today.** `factory docs init` (`src/docs/scaffold.rs:47-95`) writes `docs/project.toml`, `docs/index.md`, `docs/{features,ideas,archive}/`, the ideas README and a `.gitignore` line, never overwriting, and prints `next: declare the verify gate … or run /gate`. It does not touch `CLAUDE.md` or `AGENTS.md`; the v2 tool printed a reminder to append the convention block by hand (Dropbox `014e28f^:feature-docs/bin/docs.mjs:1239`) and the block lived in the eliminated skill (`014e28f^:feature-docs/SKILL.md:317-326`). The top-level verbs are `run wait answer status stop report retro ctx hook docs daemon` (`src/main.rs:19-50`).

**Decisions, 2026-10-13.**

- **Target: a committed copy in the repo.** Chosen over user-dir-only (a repo cannot pin, a second machine gets nothing, workers depend on machine state), over `.agents/skills/` as the one copy (Claude Code reading it is unverified), and over a repo opt-in (two live paths). Cursor's documentation lists `.claude/skills/` and `.agents/skills/` among the project dirs it reads; Codex reads `.agents/skills/`; Claude Code reads `.claude/skills/`. So the files go to `.claude/skills/<name>/…` once, and `.agents/skills/<name>` is a relative symlink `../../.claude/skills/<name>`, committed.
- **Verb: `factory init`, with `factory docs init` as its alias.** Both do the whole thing: the layout, the skills, the manifest (FR-3), the convention block, the commit, then the setup order. Chosen over a separate `factory skills install` namespace (two setup commands for a new repo) and over a `factory sync` that creates and overwrites in one verb. Every existing pointer to `factory docs init` stays valid.
- **Byte-identical, never rendered through a template.** No overlays: the family has no personal fact left (FR-1), so the file on disk equals the embedded file and a hash says whether a human touched it. Chosen over a user-scope overlay appended under `## Local` (it would be committed into every repo, so not private, and nothing needs it) and over a project overlay file (a second file per skill the hash must ignore). A repo that must change a skill forks it in place; `update` refuses over the fork (FR-4) and git shows it.
- **Collisions are not special-cased.** *Cut 2026-10-13: an earlier draft refused `init` over an existing `.claude/skills/<name>` the manifest did not record; no repo has such a name today, so the owner cut it.* `init` renders the bundle over whatever is there; git shows what it replaced. See §14.
- **Convention block.** Appended to `CLAUDE.md` and `AGENTS.md`, created if absent; when one is a symlink to the other, or their contents are identical, written once and mirrored. Idempotent on its heading `# Product Docs Convention`. Its text is the v2 block corrected for v3: the gate is `gate` under `[docs]` in `docs/project.toml`, ideas are one file each under `docs/ideas/`, and one line names the eight skills. Kept to about ten lines: the binary teaches the rest.
- **It does not commit.** `init` ends by printing the paths it wrote and the one line to commit them (`git add … && git commit -m "chore: factory init (skills <version>)"`). *Cut 2026-10-13: an earlier draft had `init` and `update` commit by pathspec, as `factory docs idea drop` does; the owner cut it, so the commit is the caller's, as every other write in `factory docs` is.*
- **This repo is the one exception.** Here `.claude/skills/<name>` are relative symlinks to `../../skills/<name>`, committed by hand, no manifest, so a session in this repo reads the live source and a skill edit is live without a bump. `doctor` prints `skills: source checkout` when `.claude/skills/prd` resolves inside `skills/`. Cost: one special case in the detector; it is what makes dogfooding the skills possible. *Clarified at slicing, 2026-10-13: `init` on a source checkout (`.claude/skills/prd` resolves inside `<root>/skills/`) leaves the skills alone, writing nothing under `.claude/skills/` or `.agents/skills/`; the layout and the convention block follow the second-`init` rule below.*
- **Setup order printed last:** gate (`factory docs gate set "<cmd>"` or `/gate`), harness (`/harness`), UX rubric (`/ux rubric`), as `/prd` preflight already says in prose.
- **What a second `init` does.** *Clarified at slicing, 2026-10-13, from the acceptance below and FR-4.* `init` writes whatever is missing, as `docs init` does today: the layout parts, the convention block when its heading is absent, and the skills when `.claude/skills/factory.toml` is absent. With a manifest present the skills are `update`'s job (FR-4), so `init` leaves `.claude/skills/` and `.agents/skills/` alone. Nothing written → `already initialised`, exit 0. `factory init` routes exactly as `factory docs init` does (`src/cli/docs.rs:220-245`): a v2 repo still reaches `docs.mjs`, and gets the skills from the `init` it runs after `docs migrate`.

**Accepted cost.** About 170 KB of markdown in every repo, and one re-render commit per repo per family release. The owner chose that over a machine-wide copy that no repo could pin.

**Acceptance.**

- On an empty git repo, `factory init` creates the layout, `.claude/skills/{prd,slice,deliver,polish,gate,harness,prune,ux,_shared}`, `.agents/skills/<name>` symlinks for the eight, `.claude/skills/factory.toml`, the block in both `CLAUDE.md` and `AGENTS.md`, and prints the commit line, committing nothing; a second `factory init` prints `already initialised`, changes nothing and exits 0.
- Every rendered file's sha256 equals the embedded file's.
- A pre-existing `.claude/skills/polish/SKILL.md` is replaced by the bundle's and recorded in the manifest.
- `CLAUDE.md` a symlink to `AGENTS.md` → the block appears once in the target.
- In this repo, `factory docs doctor` prints `skills: source checkout`.

## 5. FR-3 — The manifest, and what `doctor` and preflight say

**Today.** `factory docs doctor` (`src/docs/doctor.rs:59-104`) prints `Framework`, `gate` and `drift`. Run preflight (`src/docs/preflight.rs:227-250`) sorts its lines into `refuse`, `ask`, `judge` and `ok`; exit 1 refuses, 2 asks. The crate version is `0.1.0` (`Cargo.toml:3`) and has never been bumped. `sha2` is already a dependency.

**Decisions, 2026-10-13.**

- **Manifest at `.claude/skills/factory.toml`:** `version = "<crate version>"`, `bundle = "<sha256 of the embedded bundle>"`, and a `[files]` table `"<relative path>" = "<sha256>"` for every rendered file (symlinks listed under `[links]` with their targets). A loose file at the skills root is not a skill to any of the three agents. Chosen over a table in `docs/project.toml`: that file is written under a lock by `factory docs set` and an idea already asks to protect it from workers; seventeen hashes do not belong there.
- **The comparison, in order:** no manifest → `none → factory init`. Repo version < binary version → `behind (repo 0.1.0, binary 0.2.0) → factory skills update`. Repo version > binary version → `ahead (repo 0.3.0, binary 0.2.0) → factory upgrade` (or the `cargo install --path <source>` line when the binary was not installed from a release, FR-8). Same version, different bundle hash → `differs (0.2.0 both, bundle differs: a dev build) → factory skills update`. Then, whatever the version verdict, any file whose on-disk hash differs from the manifest → `hand-edited: polish/SKILL.md, …`, and any manifest file that is missing → `missing: …`. The version line and the hand-edit line are both printed when both apply.
- **Doctor prints it; preflight repeats it as a judge line and goes on.** Chosen over a run refusing on stale skills (a one-line skill tweak would stop a run the owner wanted now) and over the run re-rendering and committing on its own (a commit nobody asked for, on a branch a worker is about to build on). The owner reads the line in `factory run`'s preflight and in `/deliver`. *Clarified at slicing, 2026-10-13: `current` and `source checkout` are preflight `ok` lines; every other verdict line is a judge line.*
- **Doctor's `skills` line is the only new output.** `factory docs check` gets no `W-SKILLS-*` code: `check` runs as a pre-commit hook and a behind repo must still be able to commit.

**Accepted cost.** A stale `/polish` can run inside a run if the owner ignores the line. Accepted: the line is the signal the three retros never had.

- **The healthy line is `current (0.2.0)`.** *Added 2026-10-13: the adversarial pass noted the draft never named what a clean repo prints.*

**Acceptance.** Seven fixtures, one per verdict (`current`, `none`, `behind`, `ahead`, `differs`, `hand-edited`, `missing`), each producing exactly the line above from `factory docs doctor`, and `factory docs preflight` printing the same text as a judge line with its exit code unchanged.

## 6. FR-4 — `factory skills update`

**Today.** Nothing re-renders a repo's skills; the Dropbox symlink made the question moot.

**Decisions, 2026-10-13.**

- **`behind` or `differs` → re-render.** Every bundle file is written, files the manifest lists but the bundle no longer holds are deleted (a renamed reference, a retired skill, with its `.agents` symlink), the manifest is rewritten, and the commit line `chore: skills <version>` is printed; nothing is committed (same as `init`, FR-2).
- **`ahead` → exit 1** with the `factory upgrade` hint. Chosen over treating ahead like behind (a teammate's upgrade reverted by the next person's `update`).
- **`hand-edited` → exit 1** listing the files. The way through is git: `git checkout -- .claude/skills/<file>` restores the manifest's text, then `update` runs.
- **`current` → `already current`, exit 0.**
- *Clarified at slicing, 2026-10-13, for the verdicts the list above leaves out:* `missing` re-renders, as `behind` does. `none` → exit 1 with the `factory init` hint. A source checkout → exit 1, since its skills are read live from `skills/` (FR-2).
- *Cut 2026-10-13: `--force` (render over `ahead` or `hand-edited`), `--dry`, and a collision refusal for a new family name. The owner cut all three at the cut question; none had evidence. See §14.*

**Accepted cost.** A repo that forked a skill on purpose has to revert its fork in git before every family release and re-apply it after. That is the cost of refusing to lose a hand edit silently; a sanctioned per-repo customisation is a Not handled line (§14).

**Acceptance.** On the `behind` fixture, `update` leaves every file equal to the bundle, a manifest at the new version, the commit line printed and nothing committed. On `ahead` and `hand-edited`, exit 1, no file changed. A file the old manifest lists and the new bundle lacks is gone after `update`.

## 7. FR-5 — A skill change bumps the crate version, enforced by the gate

**Today.** `Cargo.toml:3` is `0.1.0` through three shipped PRDs; the binary has no way to say which skills it carries.

**Decisions, 2026-10-13.**

- **The crate version is the family version.** Chosen over a `build.rs` git-sha stamp (shas do not order, so `ahead` and `behind` collapse into `differs`, and an install from a dirty tree stamps a sha that matches nothing) and over convention alone (the first forgotten bump makes two bundles claim one version).
- **A gate test enforces it.** `skills/bundle.lock` records `version` and `sha256` of the bundle. The test passes only when **both** the recomputed hash equals the lock's hash **and** `Cargo.toml`'s version equals the lock's version; otherwise it fails naming the lock and the two lines to run (bump `Cargo.toml`, then `FACTORY_BUNDLE_WRITE=1 cargo test`). The write mode rewrites the lock from `Cargo.toml` and `skills/`, but **refuses when the hash changed and `Cargo.toml`'s version is not greater than the lock's**: a skill edit cannot re-lock under the old version. A version bump with an unchanged bundle re-locks freely: that is a binary-only release. *Rewritten 2026-10-13: the adversarial pass showed the earlier wording ("different → pass when the version is greater and the lock was rewritten") collapsed to "hash equal → pass", so a rewrite without a bump passed, the very case the FR exists to block.*
- **Release ritual** (FR-8): bump, rewrite the lock, commit, tag `v<version>`, push the tag.

**Accepted cost.** A version bump in every commit that touches `skills/`, and a lock file that merges by hand when two branches both touch skills. Small; the alternative is `doctor` lying.

**Acceptance.** Editing one byte under `skills/` without a bump makes `cargo test` fail naming the lock, and `FACTORY_BUNDLE_WRITE=1 cargo test` refuses to re-lock; bumping and rewriting the lock makes it pass; the hash the lock records equals the `bundle` value FR-3 writes into a manifest.

## 8. FR-6 — `factory review second`: the cross-vendor second opinion as a command

**Today.** `/polish` §1 (`polish/SKILL.md:52-64`) and `/prd` §4 (`prd/SKILL.md:99`) run Cursor from the invocation in `models.md` § Second opinion: `timeout 900 cursor-agent -p --trust --mode ask --output-format stream-json --model grok-4.7-high "<prompt>"` with the diff written to a file, read back with two `jq` lines. The three failures in §1 are all on this path; the stopgap in `polish/SKILL.md:54-56` is prose. No `~/.config/factory/config.toml` exists on the owner's machine today; `src/config.rs:21-28` reads `budget_weighted_tokens` and `[workers.<role>]` from it when it does.

**Decisions, 2026-10-13.**

- **Two modes.** `factory review second --diff <base> [-- <paths>]` for `/polish`; `factory review second --doc <file> --prompt "<text>"` for `/prd`. Chosen over diff-only (the PRD pass keeps the same failure modes) and over also recording a `reviews` row (a schema bump no retro asked for; Not handled).
- **Config, `[review]` in `~/.config/factory/config.toml`:** `agent = "cursor-agent"` (name or path), `model = "grok-4.7-high"`, `timeout_secs = 900`, `pass_lines = 600`. Defaults are embedded, so a missing file or key works; the owner's personal facts (the account's rate limit on `gpt-5.6-sol-xhigh`, the renamed binary) live in this file and the family carries none of them.
- **Probe first, spend nothing on a dead auth.** `<agent> --list-models`: non-zero exit, a login prompt, or the configured model absent from the list → stdout `unreviewed: <reason>`, exit 2, no pass started. The skills' fallback (a fresh Claude agent, never a fork) starts on exit 2.
- **Diff mode splits into passes.** `git diff <base>...HEAD --numstat -- <paths>` sizes each file; files are grouped in order into passes of at most `pass_lines` changed lines, a file never split, a file larger than the cap alone in its pass and said so. Each pass writes `review-<n>.diff` and runs one invocation with the four load-bearing flags (`--trust`, `--mode ask`, `--output-format stream-json`, `--stream-partial-output`) under the timeout, logging to `<data dir>/reviews/<timestamp>/review-<n>.jsonl` (`src/paths.rs` data dir).
- **A timeout keeps what it found.** The stream's assistant text is accumulated as it arrives; on a kill the partial text is printed under a `(timed out after N s, partial)` header instead of being thrown away. *Corrected 2026-10-13: the earlier draft named three flags and left "does `stream-json` carry deltas" for slice entry. The adversarial pass answered it from `cursor-agent --help` (2026.10.01): deltas arrive only with `--stream-partial-output`, "only works with --print and stream-json format". Without it a kill keeps nothing, which is the 001 and 003 failure unchanged. The flag is now part of the invocation.*
- **`--others <file>`** passes the other reviewers' one-line summaries with the question "what would fixing those still miss", the rule `polish/SKILL.md:58` already states.
- **Output is markdown on stdout for the skill to triage:** one `## Pass n — <files> (<lines> lines, <seconds> s, <model>)` per pass, the findings text under it, `unreviewed` for an empty pass. Exit 0 when at least one pass returned text, 2 when none did, 1 on a usage error. The log path is printed last.
- **The skills call it.** `polish/SKILL.md` §1 and `prd/SKILL.md` §4 replace the invocation paragraphs with the command, its exit codes and the fallback; `models.md` loses the invocation block (FR-1).

**Accepted cost.** Cursor's flags and stream format now live in Rust, so a Cursor change is a release, not a file edit. Accepted over prose a model retypes from memory, which the retros show losing a flag or a pass size.

**Acceptance.**

- A stub `cursor-agent` in `tests/stubs/` (beside the stub `claude` and `gh`) that replays a recorded stream: a 1,500-line fixture diff across five files yields three passes, none above 600 lines except a single 700-line file that is alone and flagged.
- The stub exiting 1 on `--list-models` → `unreviewed: …`, exit 2, zero review invocations recorded.
- The stub sleeping past a 2 s test timeout after emitting two assistant chunks → the chunks are printed under the partial header, exit 0.
- Doc mode on `prd.md` with a prompt → one pass, the prompt and the path in the recorded argv.

## 9. FR-7 — Closeout files the retro's friction section as an idea here

**Today.** The polish worker writes `retro.md` from the template (`assets/retro.md`, whose `:33` heading is `## Framework friction → skills repo`); closeout commits it (`src/closeout.rs:343-400`, `commit_polish`) and nothing reads the section. `factory docs idea add --title --body-file - [--origin]` (`src/docs/ideas.rs`) writes one idea file and does not commit.

**Decisions, 2026-10-13.**

- **Destination: an idea in this repo, triaged by the owner.** After the polish commit, closeout reads the section; when it holds more than the template's placeholder and `[source] path` in `config.toml` names a checkout with `docs/ideas/`, it writes an idea there in process (`idea add` with root = the source path): title `friction: <repo> PRD-NNN`, origin `<repo> PRD-NNN run <id>`, body the section verbatim plus the retro's path. The owner keeps it (graduates or edits it) or drops it with `factory docs idea drop`; an untouched idea is the owner's backlog, not the run's problem. Chosen over printing an issue link (the copy-paste step that failed three times) and over `gh issue create` (a public issue carrying a private project's names).
- **Never committed by the run.** The source checkout may be on any branch and dirty; the file lands and the owner commits it. When the source checkout is the run's own repo (a run here), the file is written after the polish commit and left untracked for the owner, listed in the run's output.
- **No source path, or the write fails** → nothing is filed and the run goes on; the section stays in `retro.md` where it always was. *Cut 2026-10-13: an earlier draft printed a `friction unfiled` line with the issues URL; the owner cut it (one machine today, no evidence). See §14.*

**Accepted cost.** One idea per run for the owner to triage, and a config key. The alternative was the status quo, which lost twelve edits.

**Acceptance.** The stub harness's polish scenario writes a retro with a two-bullet friction section; with `[source] path` pointing at a temp v3 repo, closeout leaves `docs/ideas/<date>-friction-<repo>-prd-NNN.md` there with the two bullets, origin set, uncommitted; with no `[source]`, no file is written and the run output carries no friction line. A retro with only the template placeholder files nothing.

## 10. FR-8 — Releases for Linux and macOS, and `factory upgrade`

**Today.** The only install is `cargo install --path .` (`README.md:69`), which needs Rust. There are no tags and no releases on `emilianotisato/factory` (public). CI is one `ci.yml` job running the gate on `main` pushes and PRs. The driver is Unix-only: `src/daemon.rs:8` imports `std::os::unix::process::CommandExt` for the `setsid` re-exec, ~~and `src/worker.rs` is the only other file with `os::unix` code~~. *An earlier draft also named `src/cli/hook.rs` and `src/cli/run.rs`; they only mention `setsid` in a string and a comment (adversarial pass, 2026-10-13).* *Corrected at slicing, 2026-10-13: `std::os::unix` is imported in `src/worker.rs:33-34`, `src/paths.rs:9-10`, `src/run.rs:15`, `src/measure.rs:504`, `src/docs/hook.rs:6` and `src/cli/docs.rs:8` too. The conclusion stands: the driver is Unix-only.*

**Decisions, 2026-10-13.**

- **Linux and macOS, in this PRD.** `cargo-dist` (`dist init`) generates `.github/workflows/release.yml` on `v*` tags for `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`, `x86_64-apple-darwin` and `aarch64-apple-darwin`: tarballs, checksums and `factory-installer.sh` on a GitHub release. Chosen over all three OSes (Windows is a port of the daemon, the hook and the symlinks, weeks not a slice) and over its own PRD (every other dev needs Rust until then; the owner said no).
- **`factory upgrade`** uses the `axoupdater` library: reads the install receipt the installer wrote under `~/.config/factory/`, fetches the latest release, replaces the binary, prints `0.1.0 → 0.2.0` or `already current`; `--check` prints without installing. A binary with no receipt (a `cargo install` build) prints `installed from source — cargo install --path <source>` using `[source] path`, or the installer URL when there is none. Doctor's `ahead` hint (FR-3) uses the same rule. *Clarified at slicing, 2026-10-13: "not installed from a release" is no receipt, or a receipt whose `install_prefix` is not the running binary's directory (`axoupdater`'s `check_receipt_is_for_this_executable`). A `cargo install` binary beside a release install therefore prints the source line instead of upgrading the other copy.*
- **The tag is the owner's.** Bump (FR-5), commit, `git tag v0.2.0`, push the tag from a plain terminal: the global pre-push hook refuses `main` under Claude Code and the SSH key needs a passphrase, as every merge here already does.
- **README** `## Trying it` gets the installer line first and `cargo install` second.

**Accepted cost.** macOS runner minutes per tag, a `[workspace.metadata.dist]` table in `Cargo.toml`, and the release workflow is unproven until the first tag, which no test can push. Verify at slice entry how `cargo-dist` builds `aarch64-unknown-linux-gnu` (native runner or cross) and the current `axoupdater` version.

**Acceptance.**

- `cargo test` reads `[workspace.metadata.dist]` in `Cargo.toml` and `.github/workflows/release.yml` and finds the four targets and the installer; the builder runs `dist plan` once at slice time (installing `cargo-dist` locally) and pastes its output into the slice's Progress. *Corrected 2026-10-13: `cargo-dist` is not installed here and the gate cannot depend on it, so the earlier "`dist plan` lists the targets" had no surface inside the gate.* *Clarified at slicing, 2026-10-13: recent `cargo-dist` may write its config to `dist-workspace.toml` instead of `Cargo.toml`; the test reads whichever file `dist init` writes, and slice 06's Progress says which.*
- `factory upgrade --check` against a stub release server prints the newer version; with no receipt it prints the source line and exits 0.
- Human (qa-guide): the owner tags `v0.2.0`, the release workflow goes green with four archives and the installer, `factory upgrade` on a second machine replaces the binary and `factory --version` says `0.2.0`. *Corrected at slicing, 2026-10-13: no release exists below `v0.2.0` (`gh release list` is empty), so no machine can hold a receipt-installed older binary for `upgrade` to replace. The second machine installs `v0.2.0` with `factory-installer.sh`, `factory --version` says `0.2.0`, the receipt lands in `~/.config/factory/`, and `factory upgrade --check` prints `already current`. The replace path is first observable at the next tag. The branch carries the bump to `0.2.0` (slice 07), so the owner's ritual after the merge is the tag alone.*
- *Recon at slicing, 2026-10-13, `axoupdater` 0.10.2 source:* the receipt is `<dir>/factory-receipt.json`, where `<dir>` is `AXOUPDATER_CONFIG_PATH` when set, else `$XDG_CONFIG_HOME/factory` when it exists, else `~/.config/factory` (`src/receipt.rs:123-160`). `FACTORY_INSTALLER_GHE_BASE_URL=http://127.0.0.1:<port>` sends the release lookup to `<url>/api/v3/repos/<owner>/<name>/releases/latest` (`src/release/github.rs:14-43`, `:80`), so a test can serve a stub release from a local socket. The sync calls need the crate's `blocking` feature. `is_update_needed` returns false unless the receipt's `install_prefix` is the running binary's directory (`src/receipt.rs:91-118`), so a test receipt names the test binary's directory.

## 11. Decisions log

No `OPEN-n`. Every fork was closed in the interview on 2026-10-13; the FR sections carry each decision with its cost. The forks, for the record: target (repo copy), verb (`init` + `doctor` + `update`), members (the eight + `models.md`), fence (manifest, refuse, no overlays), behind (warn), ahead (refuse to downgrade), bump rule (gate test), releases (Linux + macOS here), feedback (idea here, owner triages), review command (diff and doc modes), proof (gate + stub + human rows), builder (`factory run`, owner migrates after the merge).

Facts to re-verify at slice entry, not decisions: Claude Code's precedence between a project skill and a user skill of the same name; whether Cursor lists a family skill twice when it reaches it from both `.agents/skills/` and `.claude/skills/` (its docs walk every root and say nothing about deduping); `cargo-dist`'s aarch64 Linux build path; `axoupdater`'s receipt format (0.10.2 and `cargo-dist` 0.32.0 are current on crates.io, 2026-10-13).

Settled at slicing on 2026-10-13: the precedence (personal over project, §2), and `axoupdater`'s receipt path and release-URL override (FR-8 Acceptance). Still open, and where each is answered: Cursor's double listing, by the owner in the FR-2 human row, because Cursor's auth had expired; `cargo-dist`'s aarch64 Linux path, by slice 06's builder when `dist init` writes the workflow.

Settled by the adversarial pass on 2026-10-13: Codex follows a symlinked skill folder (`codex debug prompt-input` in a temp repo listed a skill reached only through `.agents/skills/x → ../../.claude/skills/x`); Cursor's `stream-json` emits deltas only with `--stream-partial-output` (FR-6).

## 12. QA plan

**Gate**: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` *(declared in project.toml)*
**Surface**: CLI (`factory init`, `factory docs doctor`, `factory skills update`, `factory review second`, `factory upgrade`), plus the three agents' skill lists in an initialised repo.
**Harness**: gate-only. The stub harness (`tests/stubs/claude`, `tests/stubs/gh`, `tests/harness/mod.rs`) gains a stub `cursor-agent`; every FR but the live parts is a `cargo test` transcript. A `verify-factory` harness was offered and declined: the proof is already a transcript.
**Seed state**: a temp git repo per test; the `behind`/`ahead`/`differs`/`hand-edited` fixtures are manifests written by the test; a fixture diff and a recorded Cursor stream for FR-6; a temp v3 repo as the `[source]` for FR-7.

| FR | Proof on the real surface | Evidence | Human? |
|---|---|---|---|
| FR-1 | run `cargo test`: the bundle test walks `skills/` and finds every file embedded; `grep -r feature-docs skills/` empty | test transcript, grep output | no |
| FR-2 | run `factory init` on a temp repo: layout, nine skill folders, eight symlinks, manifest, block in both files, the commit line printed and nothing committed; run it twice; run it over a pre-existing `polish` | test transcript | no |
| FR-2 | after the merge: `factory init` in `arca-api`, then `/prd` appears in Claude Code's skill list, `cursor-agent` and `codex` list it in that repo, and Cursor lists it once, not twice | the owner's read-back | yes |
| FR-3 | run `factory docs doctor` on each of the seven fixtures *(six before 2026-10-13's `current` fixture; corrected at slicing)*; `factory docs preflight` shows the judge line | test transcript | no |
| FR-4 | run `factory skills update` on `behind` (renders, prints the commit line), `ahead` and `hand-edited` (exit 1, nothing changed), a dropped file is gone | test transcript | no |
| FR-5 | run `cargo test` after a one-byte skill edit without a bump (fails naming the lock), then with the bump and lock rewrite (passes) | test transcript | no |
| FR-6 | run `factory review second --diff` against the stub: three passes, the oversized file alone; `--list-models` failing → exit 2; a timeout keeps the partial text; `--doc` records the prompt | test transcript | no |
| FR-6 | ~~one real `factory review second --diff main` on this feature's branch during its own polish~~ *corrected at slicing, 2026-10-13: the polish worker reads the Dropbox `/polish` (§2), which never calls the command.* The checkpoint that covers slice 04 runs `factory review second --diff main -- src/review.rs src/cli/review.rs` once on this branch with the owner's real `cursor-agent` *(scoped to the slice's own source at slicing, 2026-10-13, so one pass stays under the cap)*: findings and the log path, or `unreviewed: <reason>` with exit 2 when the auth is gone | the checkpoint's transcript | no |
| FR-6 | after `cursor-agent login`: one `factory review second --diff main` prints findings; one more with `timeout_secs = 60` under `[review]` prints text under the partial header. The live delta shape of `--stream-partial-output` was never recorded, so only a live run proves the partial path | the owner's read-back | yes |
| FR-7 | run the stub polish scenario with a friction section and a temp `[source]`: the idea file lands uncommitted; without `[source]`, no file and no line | test transcript | no |
| FR-8 | run `factory upgrade --check` against a stub release; `cargo test` finds the four targets in the dist metadata and the release workflow | test transcript | no |
| FR-8 | the owner tags `v0.2.0`; the release is green with four archives and the installer; ~~`factory upgrade` on a second machine prints `0.1.0 → 0.2.0`~~ the installer on a second machine (or a throwaway `HOME`) installs `0.2.0` with a receipt, and `factory upgrade --check` prints `already current` *(corrected at slicing, 2026-10-13: no older release exists to upgrade from; FR-8 Acceptance)* | release page, the owner's read-back | yes |

The three `yes` rows are what `qa-guide.md` holds.

## 13. Slices

Seven slices, built on `feat/skills`: one branch, one PR, one commit per slice, Prove at the checkpoints. `factory docs status` is the board.

- **01, the spine: the family moves in** (FR-1). The seven Dropbox folders and `models.md` are ported at `33796be` into `skills/`, embedded as one bundle with one hash, and this repo's `.claude/skills/` link into `skills/`. Every slice but 05 reads the bundle, so it goes first.
- **02, `factory init`** (FR-2): the bundle, the manifest and the convention block rendered into a repo, the commit line printed.
- **03, the fence** (FR-3, FR-4): doctor's and preflight's `skills` line, and `factory skills update`. It reads the manifest 02 writes.
- **04, `factory review second`** (FR-6, and FR-1's `models.md` trim): the command, its stub, `[review]`, and the skill edits that call it. It needs only the ported skills.
- **05, friction to an idea** (FR-7): `[source] path`, and closeout filing the retro's friction section. Independent of the rest.
- **06, releases and `factory upgrade`** (FR-8). It rewrites 03's `ahead` hint and reads 05's `[source] path`.
- **07, the bundle lock and the bump to `0.2.0`** (FR-5). It follows 04, the last skill edit, so the lock is written once, at the version the owner tags.

**Order.** 01 is the spine and 05 builds beside it. Then two branches: 02 → 03 → 06, and 04 → 07. The board is two wide at every step: 01 and 05, then 02 and 04, then 03 and 07, then 06.

**Checkpoints** (`checkpoint_every = 2`) fall after the second, fourth and sixth done slices, and at the end. In the planned order the first covers 01 and 05, so the bundle and its hash are proven before anything builds on them. The second covers 02 and 04, and runs the one live `factory review second`. The third covers 03 and 07; the last covers 06.

**Risks each slice names at its top**: 02 changes what every harness fixture's `docs init` writes; 03 adds preflight lines that `tests/run.rs` and `tests/loop.rs` count; 04's live partial stream was never recorded; 06's release workflow cannot run before the tag; 07 makes every later skill edit a version bump, polish included.

**Human proof** lives in [`qa-guide.md`](qa-guide.md): the three Human rows of §12. FR-6's needs only `cursor-agent login`; FR-2's and FR-8's come after the merge, when the owner tags `v0.2.0` (the bump is on the branch) and runs `factory init` in `arca-api`.

## 14. Non-goals

- **Stack profiles and overlays.** `README.md:36` promised "rendered from a profile plus overlays"; this PRD renders byte-identical files and corrects the README. Profiles are the stack-plugin idea in the README's roadmap.
- **Windows.** The daemon, the hook and the symlinks are Unix; a Windows build is a port with its own PRD.
- **`/lanes`.** A README promise; it ships with the isolated-lanes PRD.
- **Non-family skills** (`caveman`, `codex`, `self-learning`, `deploying-laravel-cloud`, Omarchy's) stay in Dropbox and the user dir.
- **The learn loop.** Friction ideas are its raw material; ranking them is a later PRD.
- **Not handled: a user-scope install.** Accepted 2026-10-13: outside an initialised repo `/prd` does not exist; a new repo starts with `factory init` from a terminal. Revisit if the owner finds themself wanting `/prd` in a repo before `init`, more than once.
- **Not handled: a sanctioned per-repo customisation of a skill.** Accepted 2026-10-13: no overlay exists; a fork is a hand edit that `update` refuses. Revisit when a repo needs a lasting change to one skill that the family should not carry.
- **Not handled: `init --no-skills`.** Accepted 2026-10-13: no repo asked for the board without the skills.
- **Not handled: a run refusing or auto-updating on stale skills.** Accepted 2026-10-13: a judge line is the signal; the owner chose it over a blocker or an unasked commit. Revisit if a run builds with a skill a release had already fixed.
- **Not handled: a `reviews` row in the metrics store.** Accepted 2026-10-13: no retro asked for the number. Revisit when a retro wants review duration or findings per pass compared across runs.
- **Not handled: `gh issue create` for friction.** Accepted 2026-10-13: private project names in a public issue. ~~The unfiled line carries the URL instead.~~ *Corrected at slicing, 2026-10-13: the cut retired the unfiled line (last bullet); with no `[source]` nothing is filed.*
- **Not handled: `factory docs check` warning on stale skills.** Accepted 2026-10-13: `check` is the pre-commit hook and a behind repo must still commit.
- **Not handled: `init` refusing over a project skill with a family name.** Cut 2026-10-13: no repo has one; `init` renders over it and git shows the replacement. Revisit when a repo loses a skill this way.
- **Not handled: `init` and `update` committing what they wrote.** Cut 2026-10-13: both print the commit line; the commit is the caller's, as every other `factory docs` write is. Revisit if uncommitted skills dirs show up in `doctor` output more than once.
- **Not handled: `update --force`.** Cut 2026-10-13: an `ahead` repo upgrades the binary, a `hand-edited` repo reverts the edit in git, then `update` runs. Revisit when a repo keeps a deliberate fork across releases.
- **Not handled: `update --dry`.** Cut 2026-10-13: `doctor` already prints the verdict and `update` touches only `.claude/skills/` and `.agents/skills/`, which git undoes.
- **Not handled: a `friction unfiled` line with the issues URL.** Cut 2026-10-13: one machine, with `[source]` set. Revisit when a second contributor runs the binary without a checkout of this repo.

## 15. Verification notes

Checked against `main` at `39c8ad4` and Dropbox `33796be`, 2026-10-13; corrected the same day after the adversarial pass (the Dropbox shas, the unix files, the drift commit, the size, the polish line range; each marked where it sits). **Re-verify at slice entry.**

- `src/main.rs:19-50` — the top-level verbs; `init`, `skills`, `review`, `upgrade` are new.
- `src/docs/scaffold.rs:47-95` — `docs init` writes the layout, never overwrites, prints the gate line; no `CLAUDE.md` handling.
- `src/cli/docs.rs:240` — an uninitialised repo routes only `init` natively; `:303-306` still resolves v2's `docs.mjs` from `~/.claude/skills/feature-docs/bin/`, which is v2 routing and stays.
- `src/docs/doctor.rs:59-104` — doctor's three lines; the `skills` line is added after `gate`.
- `src/docs/preflight.rs:227-250` — the `refuse`/`ask`/`judge`/`ok` lists and the exit code; the skills verdict is a judge line.
- `src/docs/gate.rs:411-418` — the `verify-*` glob over `.claude/skills`; `:305` reads `CLAUDE.md`/`AGENTS.md` for gate hints only.
- `src/pack.rs:412-416,484` — the polish pack's prompt is `/polish`; the skill body is not in the pack, so the project copy is what the worker reads.
- `src/pack.rs:23-35`, `src/docs/scaffold.rs:16-20` — assets embedded with `include_str!`; the bundle follows the pattern.
- `src/config.rs:1-3,21-28` — the user config file, its keys, unknown keys warn.
- `src/paths.rs:76` — `~/.config/factory/`; no `config.toml` exists on the owner's machine today.
- `src/closeout.rs:343-400` — `commit_polish` writes a missing retro from the template and commits by pathspec; the friction read goes after it.
- `src/docs/ideas.rs` and `factory docs idea add --help` — `--title`, `--body-file -`, `--origin`; `add` does not commit, `drop` does.
- `src/daemon.rs:8` — `std::os::unix`; Windows is out.
- `Cargo.toml:3` — `version = "0.1.0"`; `sha2` already a dependency; no `[workspace.metadata.dist]`.
- `.github/workflows/ci.yml` — one gate job; no release workflow, no tags, no releases (`gh release list` empty); repo is public.
- `assets/retro.md:33` — `## Framework friction → skills repo`; the three shipped retros each filled it.
- `assets/autonomy.md` vs Dropbox `feature-docs/autonomy.md` — one row differs (`/harness` major-upgrade block); the asset is current.
- Dropbox family: 17 files in seven folders, ~~about 200 KB~~ 163 KB (FR-1; this line missed the 2026-10-13 correction, fixed at slicing); `feature-docs/models.md` 8 KB linked from `prd/SKILL.md:15,99`, `slice/SKILL.md:17`, `polish/SKILL.md:17,52,60`, `harness/references/maintain.md:19`, `ux/SKILL.md:18`.
- `polish/SKILL.md:52-64` — the second-opinion paragraphs FR-6 replaces; `:54-56` the prose stopgap.
- `~/.claude/skills` → `~/Dropbox/dev/claude-code-skills`; `~/.codex/skills` → `~/Dropbox/dev/codex-skills`; `~/.agents/skills` and `~/.cursor/skills` exist with non-family skills. The only project skill in an owner repo today is `herdr-plugins/.claude/skills/verify-herdr-plugins`.
- Cursor docs (`cursor.com/docs/skills`): project dirs `.agents/skills/`, `.cursor/skills/`, and `.claude/skills/`, `.codex/skills/` for compatibility. Codex (`developers.openai.com/codex/skills`): `.agents/skills/` in the repo and `~/.agents/skills/`. Claude Code: `.claude/skills/`.
- Live drift seen 2026-10-13: binary built 09:36, scaffold asset changed 13:35 (`141e27c`); `docs new` wrote `## 2. Users affected` and `/prd-to-tasks` into this PRD, corrected by hand.
- Dead code: none found on these paths.
- Re-verified at slicing, 2026-10-13, on `feat/skills` at `bfd5807`: each slice's Current code section carries the facts its builder needs; corrections made here are marked *at slicing* where they sit.
