---
number: 022
slug: guards-a-branch-cant-loosen
title: "Guards a branch can't loosen"
# type: feature | bugfix | enhancement | refactor
type: feature
created: 2026-11-05
author: Emiliano Tisato
summary: |
  The branch under build cannot edit the rules it is judged by: slice workers cannot commit docs/project.toml, the leaks allowlist is read from the base, every caller resolves the base the same way, and leaks catches plan ids inside strings. Draft from the 2026-11-05 idea triage, not interviewed.
---

# PRD-022 — Guards a branch can't loosen

> **Draft from the 2026-11-05 idea triage.** The owner chose what is in and out, group by group; nothing here is interviewed or verified yet. `/prd` continues this draft: interview, recon, FRs, QA plan. `file:line` facts are from `main` at `ca128f7`; re-verify them.

## 1. Why now

`protected_files` still names v2's `docs/index.md`, not `project.toml` where the gate now lives (`src/commit.rs:11-12`), so a builder could lower a floor to go green. `allowlist(repo)` reads the scanned worktree (`src/leaks.rs:788`). Five callers resolve the base their own way, and `default_branch` falls back to `main` without trying `origin/` (`src/closeout.rs:2282`). On icg-cloud, leaks missed `fr123-7` in a string (`src/leaks.rs:42,63-67`).

## 2. Candidate FRs

Kept by the owner at the triage. `/prd` turns each into an FR or cuts it.

- **C-1 — Slice workers cannot commit `docs/project.toml`**, keeping the preflight worker's one allowed write (`inner`).
- **C-2 — The leaks allowlist is read from the base.**
- **C-3 — One base-ref resolver** for pack, closeout, leaks, review and collect.
- **C-4 — The leaks plan-id rule catches `fr123-7`.**

## 3. Not in scope (decided 2026-11-05)

- **Leaks: a multi-line splitter, scan cost, tuning candidates** — dropped: big work, one hit across the last two features.
- **A pre-push guard for the driver's own git calls; worker pushes to a remote with its own `pushurl`** — dropped: the deny rules and the hook still hold.

## 4. Source ideas

The full text of each idea this draft absorbed; the idea files are deleted.

### Protect `docs/project.toml` from slice workers in v3

_added 2026-10-13_

`src/commit.rs` `protected_files` still names `docs/index.md`, which held the gate in v2; in v3 the gate and lanes live in `project.toml`, which a worker can now commit. Blocking it outright would also stop the preflight worker's one allowed write (`inner`, autonomy.md § May), so this needs a rule: protected for slices, allowed at preflight. PRD-002 polish review, 2026-10-13.

**Recon, PRD-005 (2026-10-13).** The premise above is out of date: v3 has no preflight worker (job kinds at `src/worker.rs:192-210`), and autonomy § May (`assets/autonomy.md:43`) no longer mentions `inner`, so protecting `project.toml` for every worker needs no exception. PRD-005 makes the risk real, though: its landing commits the lane with `git add -A`, so any worker edit to `project.toml` would land. The fix drafted there was one more entry in `protected_files`, dropped at landing per role like the others. The owner cut it at PRD-005's final review for want of an incident (PRD-005 §10, §16). Retrigger: a landed commit that changes the gate, the inner loop or the lane count.

### leaks: allowlist trust, scan cost, tuning candidates

_added 2026-10-14, origin: PRD-006 polish_

Three trust and cost gaps in `factory leaks`, found by the PRD-006 polish security pass and the fallback second-opinion agents.

- **The allowlist is read from the worktree being scanned.** A checkpoint worker can add `[leaks] allow` in `docs/project.toml` and its own hits vanish from the next `# Leaks` section. The Autonomy contract forbids it, but nothing checks it. Option: read the allowlist from the base ref (`git show <base>:docs/project.toml`) when `--base` is given, and print allowlist changes on the branch as their own section.
- **Cost.** `docs doctor` and `docs preflight` run `--all` over the whole tree each time (3.5 s on cody-agent, 4.2 s on arca-api with a debug build). There is no cap on file size or count; lockfiles and large json/yaml are read and count in the comment-share denominator. Option: skip lockfiles and files over a size cap, keep data formats out of the share, and cache the `--all` verdict by tree hash.
- **Tuning candidates seen in the field but not fixed:** `C99`/`C11` (language standards), "task 1" in prose, `block1`/`cp1` as Bézier names, `Dec-2026` dates. Each needs a non-hit unit test before a pattern change.

### One base-ref resolver for pack, closeout and leaks

_added 2026-10-14, origin: PRD-006 polish_

Three callers each work out "the base branch" their own way: `pack.rs` (around line 459), `closeout.rs` (around line 485, `default_branch`) and the `changed_lines` helper. PRD-006 added `leaks::scan(Input::Base(..))`, called from the checkpoint driver through `closeout::default_branch`. None handles a repo whose default branch exists only as `origin/<name>` with no local branch: the scan errors and the checkpoint pack says `not run`.

Shape: one `base_ref(repo) -> Result<String>` that tries the local default branch, then `origin/<default>`, then `merge-base` against it, used by all four. Found in the PRD-006 polish review; pre-existing, so not fixed in that PR.

### Also from

friction-icg-cloud PRD-019, its leaks bullet (C-4): full text in *CLI, docs and log correctness*.
