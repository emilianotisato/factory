---
number: 011
slug: outline-reads
title: "Whole reads of Outline files go through"
type: enhancement
created: 2026-10-29
author: Emiliano Tisato
summary: |
  The worker hook stops denying a whole read of a file the pack only outlines; it denies a whole read of the slice file alone, which the pack carries in full. Agreed; no slices, built attended from this document, shipped as 0.10.0 and measured by hand on the next runs.
from_idea: "2026-10-20-the-outline-deny-costs-a-turn-and-the-builder-read"
---

# PRD-011 — Whole reads of Outline files go through

## 1. Why now

On 0.7.0 the hook's Outline deny was the builders' top denial: **9 of the 12** across the four runs read after PRD-009 merged (010-20261016-61d1 nycase, 026-20261017-f6ac arca-api, 012-20261017-3bb0 prokeycloud_onprem, 016-20261020-0bba prokeycloud; `docs/archive/009-cheaper-builders/retro.md` § After merge).

The pack's `# Outline` holds each cited symbol's definition (with its body when one definition matches) and the outline of each file the slice names or a definition sits in. It never holds a whole file. A builder that wants the rest of a file cannot get it from the pack. In all 9 cases the next call read the same file anyway, with `Read` plus `offset: 1` and a `limit`, or with `sed -n 1,80p`. The deny bought nothing and cost a turn each time, plus a re-issued multi-file `cat` in two cases: one Outline file in the list denies the whole call.

> *Corrected 2026-10-29 (adversarial pass): an earlier draft said the Outline holds "definitions and their lines, not its body". `Recon::of` prints a cited symbol's body when one definition matches (`src/pack.rs:753-756`), and `tests/loop.rs:373-376` pins it. The 9 re-reads stand either way: each wanted more of the file than the pack held.*

This is a bug the runs hit, not an improvement. It leaves alone the ctx grep deny (3 of the 12, each a bare-name grep that PRD-009 FR-4 still means to send to ctx) and the push guard.

## 2. Blast radius

- **Builders, the only workers on a slice pack.** They stop seeing the Outline hint; a whole read of an outlined file, or a `cat` that names one, goes on to the worker's permission rules. A checkpoint or polish pack is unchanged: `Held::section` already returns `None` for any pack not named `slice-*` (`src/cli/hook.rs:72-74`). So is a fix pack: it is `fix-*` with an empty file list (`src/pack.rs:300`).

  > *Corrected 2026-10-29 (adversarial pass): an earlier draft named fixers in this bullet. They never see this deny.*
- **Context cost.** A builder that reads a whole outlined file now pays its full size in one call instead of a denied call plus a ranged read of about the same size. The 9 cases show the read happens either way.
- **External contracts:** none. The pack header format (`<!-- factory-pack files: … -->`, `src/pack.rs:261`) stays as it is.

## 3. FR-1 — Deny a whole read of the slice file only

**Today.** `Held::section` (`src/cli/hook.rs:71-91`) labels a path by its basename and its place in the pack header; it never reads a heading. The first header entry is the slice file and answers `Slice` (`:84-86`). `prd.md` and `log.jsonl` answer `None` (`:80-82`, `:87-89`). Every other entry answers `Outline` (`:90`). That covers the files `Recon::of` outlined (`src/pack.rs:218-223`) and a block's source file when the header lists one (`:213-217`). A block found in the log has `log.jsonl` as its source, already `None`; only the fallback source `answers/<id>.json` (`src/scheduler.rs:826-828`) gets the Outline hint, though it is under `# Block and answer`. `decide` denies a whole `Read` of any file with a section, where whole means each of `offset` and `limit` is absent or null (`src/cli/hook.rs:103-105`).

> *Corrected 2026-10-29 (adversarial pass): an earlier draft said `section` maps a path to the section it sits under, and that every block source got a wrong Outline hint. Both were false as stated; the fallback `answers/<id>.json` is the one case.* `bash` denies a `cat` not piped onward when any argument has a section (`:154-162`).

**Decision (2026-10-29).** Only the slice file is held. A whole `Read` or a bare `cat` of the slice file is still denied with the `# Slice` hint. Every other file in the header goes through. The last line of `assets/lookup.md` (`:9`), which every slice pack quotes under `# Lookup` (`src/pack.rs:256`), today says a whole read of any file the pack holds is answered with its section. It changes to name only the slice file.

**Accepted cost.** A builder may now read a large outlined file whole where a ranged read would have done. The 9 recorded cases read from line 1 anyway, so no saving is lost that the runs showed. The alternative, keeping the Outline deny but letting a multi-file `cat` through, was rejected: it still costs a turn on every single-file `Read`, which was most of the 9. Dropping the slice deny too was rejected: the pack holds the slice in full, so a whole re-read is pure duplication, and keeping one deny costs nothing.

**Acceptance.**
- A whole `Read` and a bare `cat` of the slice file in a slice pack are denied with `pack_hint("Slice")`.
- A whole `Read` and a bare `cat` of an outlined file, and of a fallback block source `answers/<id>.json`, get no decision: `factory hook` prints nothing. The asserts that pin the Outline deny today flip to this: `src/cli/hook.rs:1241`, `:1262-1263`, `:1267`, `:1287-1290`, `:1306`, and `tests/cli.rs:131-135`.
- A `cat` that names the slice file among other files is still denied with the `# Slice` hint, pinned by a new assert (no test covers the multi-file case today).
- `only_the_repo_copy_of_a_held_file_is_held` (`src/cli/hook.rs:1284`) moves to the slice path, so the repo-copy matching stays pinned now that `src/helper.rs` is no longer held.
- The checkpoint pack still holds nothing (`src/cli/hook.rs:1270-1280` unchanged).
- The `# Lookup` section of a slice pack names only the slice file as denied whole.
- The gate is green with the flipped asserts.

## 4. Decisions log

- ~~**OPEN-1** — Deny shape?~~ Settled 2026-10-29: slice file only (§3).
- ~~**OPEN-2** — A denials figure in `factory report`?~~ Settled 2026-10-29: no, measured by hand like PRD-009 (§7).
- ~~**OPEN-3** — Who builds it?~~ Settled 2026-10-29: attended, in the session that wrote this PRD, no slices.

## 5. QA plan

**Gate**: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` *(declared in project.toml)*
**Surface**: CLI, `factory hook` reading a PreToolUse JSON on stdin with `FACTORY_PACK` and the repo env set.
**Harness**: gate-only plus one command transcript; the hook is a CLI with no UI.
**Seed state**: a slice pack file whose header lists a slice file and an outlined file; the hook tests' `held()` fixture is the shape.

| FR | Proof on the real surface | Evidence | Human? |
|---|---|---|---|
| FR-1 | run the installed `factory hook` with a pack whose header lists a slice file and an outlined file: a whole `Read` of the slice prints a deny naming `# Slice`; a whole `Read` of the outlined file and a `cat` of it print nothing. The hook tests that pin a whole read of a held file, once flipped, cover the same verdicts. | command transcript | no |
| FR-1 after merge | an agent session reads the worker streams of the next runs on 0.10.0, as PRD-009's After merge did. Pass: zero `# Outline` deny reasons. Reported, no pass mark: turns per builder against the same repo's last 0.7.0+ run. | a § After merge note in this feature's archived records at harvest | no |

## 6. Slices

No slices — build straight from this document.

## 7. Non-goals

- **The ctx grep deny.** Its 3 denials are the ones PRD-009 FR-4 meant; idea `2026-10-15-deny-a-bare-name-grep-only-where-ctx-can-search` holds any change to it.
- **The pack header format.** Outlined files stay listed; `src/pack.rs` writes the list and only `Held::parse` (`src/cli/hook.rs:42-58`) reads it, so trimming it is churn.
- **Not handled: a denials figure in `factory report`.** Accepted 2026-10-29: two hand counts (PRD-009, this one) are not yet evidence that a stored figure pays for a schema change. Revisit if a third PRD needs the same hand count.

## 8. Verification notes

Checked against `main` at `d308529` (0.9.0) on 2026-10-29. **Re-verify at slice entry.**

- `src/cli/hook.rs:71-91` — `Held::section`: index 0 is `Slice`, `prd.md` and `log.jsonl` are `None`, everything else `Outline`.
- `src/cli/hook.rs:72-74` — a pack not named `slice-*` holds nothing.
- `src/cli/hook.rs:103-105` — `decide`: a `Read` is whole when each of `offset` and `limit` is absent or null.
- `src/cli/hook.rs:154-162` — a `cat` not piped onward is denied on the first held argument.
- `src/pack.rs:212-223` — header order: slice, `prd.md`, block source, then outlined files.
- `src/pack.rs:255`, `:261` — `# Outline` holds `recon.text`; the header is the pack's first line.
- `src/pack.rs:753-756` — a cited symbol's definition is printed with its body; `tests/loop.rs:373-376` pins it.
- `src/pack.rs:300` — a fix pack is `fix-*` with an empty header list.
- `src/scheduler.rs:811-828` — a block's source is `log.jsonl`, or `answers/<id>.json` as the fallback.
- `assets/lookup.md:9` — the builder-facing sentence about whole-file reads.
- `src/cli/hook.rs:1237`, `:1284` and `tests/cli.rs:131-135` — the tests that pin Outline denies today; `src/cli/hook.rs:1004` is the fixture header.
- Nothing in `skills/` names the Outline deny; `src/pack.rs` writes the header and `Held::parse` reads it.
