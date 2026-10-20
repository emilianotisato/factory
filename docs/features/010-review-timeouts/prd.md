---
number: 010
slug: review-timeouts
title: "Second-opinion passes that finish"
type: enhancement
created: 2026-10-20
author: Emiliano Tisato
summary: |
  The second opinion's default timeout goes from 900 s to 1800 s, because half of all stored passes were killed while still reviewing. Eight one-line skill fixes (seven harvest slips and the `/gate` anecdote) ride the same version bump (0.8.0). Agreed; small enough to build from one slice.
from_idea: "2026-10-15-a-timed-out-review-pass-with-only-opening-chatter"
---

# PRD-010 — Second-opinion passes that finish

## 1. Why now

**The bug.** Every polish since PRD-001 lost second-opinion passes to the timeout. On 2026-10-20, all 31 stored review dirs under `~/.local/share/factory/reviews/` were measured: **90 passes, 44 killed at 900 s, 37 done, 9 exited with no text.** Every pass ran `grok-4.7-high`. The 2026-10-20 run on another repo lost 4 of 4 passes; the 2026-10-17 run lost 5 of 5. Each lost pass costs a fallback agent, or leaves its files unreviewed.

**The timeout is too short, not the passes too big.** Done diff passes took 400 to 873 s, with a median near 700 s: 900 s sits in the tail of a normal pass. Diff size does not predict a kill. Passes of 281 and 325 lines were killed, while passes of 1909 and 1200 lines finished.

**The killed passes were reviewing.** They made 32 to 186 tool calls and left only 250 to 900 chars of visible text, but 72k to 86k chars of `thinking`. One (`20261017-051010/review-3`) thought "the review is finalized with six file:line defects" seconds before the kill.

**The improvement.** Seven skill-text slips the 2026-10-15 and 2026-10-20 harvests filed, plus the `/gate` anecdote idea: eight line fixes. Each was a guess a worker had to make, and none ships without a version bump, so they ride this one.

**Left alone.** How a killed pass is marked (§8). The idea this PRD graduated from proposed a `— partial` mark; the owner chose the timeout alone on 2026-10-20.

## 2. Blast radius

- **Every polish** waits longer when a pass hangs: up to 30 minutes instead of 15. Passes run in parallel (`run_passes`, `src/review.rs`), so it is one wait per collect, not one per pass. A `factory run`'s closeout runs the collect synchronously before the polish worker (`closeout.rs:354`), with no outer timer to trip.
- **The `/prd` adversarial pass** (`factory review second --doc`) shares the knob. Four doc passes ran out the 900 s too (`040706`, `185843`, `200129`, `232152`).
- **An owner's `[review] timeout_secs`** in `~/.config/factory/config.toml` still wins. This machine sets none.
- **The skill bundle** goes to 0.8.0. Every project picks the text up on `factory skills update`; no doc shape changes.

## 3. FR-1 — The default review timeout is 1800 s

Today `Review::default()` sets `timeout_secs: 900` (`src/config.rs:71`), and `config.rs:528` pins it in a test. `run_pass` kills the pass's process group at that timeout (`src/review.rs:380`, `run_logged` at `:224`). The probe caps itself separately at `PROBE_SECS` (`review.rs:268`) and is unaffected.

**Decision (2026-10-20): 1800 s,** about twice the slowest done pass (873 s). **Accepted cost:** a pass that truly hangs now costs 30 minutes before the fallback starts. It lost to a smaller raise (1200 to 1500 s) because the killed passes were still mid-review at 900 s, and nothing shows how much longer they needed. It lost to salvaging the `thinking` stream because thinking holds rejected hypotheses ("the mass-update bug is not a defect"), and step 2 would verify noise.

**Acceptance:** with no `[review]` in the config, `Review::default().timeout_secs` is 1800, pinned by the config defaults test. After the merge and `cargo install --path . --locked`, the next real polish's review dir is counted as killed, done and no-text, attended, and appended to this feature's `retro.md` as `## After merge`: fewer than half killed is the fix holding. *Corrected 2026-10-20: an earlier draft had the run's own polish record it. That polish runs the installed, pre-merge binary at 900 s and writes the retro before the merge (`assets/contract-polish.md:14-15`), so it cannot.*

## 4. FR-2 — The skill-text slips, in one bump

Each line is a fix already written in its idea, checked against the skill text on 2026-10-20.

1. **polish §1, the leaks verdict** (`skills/polish/SKILL.md:53`). It says "`n fix hits` is hits", while `count` (`src/collect.rs:84-90`) prints `1 fix hit` for one. Write "`n fix hit(s)`".
2. **polish §2, read the leak hits** (`:72-74`). §2 never says to read the `## Leaks` part's hits. Add at its top: "A leaks verdict with hits → read the `## Leaks` part too: each hit sits under its fix line."
3. **polish §8, cross-slice fixes** (`:148`). It already sends feature-wide fixes "in the last slice touched". Add that a fix goes to the slice that owns the code it changes.
4. **polish §8, where the fixes section goes** (`:148`). Say "at the end of the slice file". *Corrected 2026-10-20: the harvest's version said slices have no Progress section now. They do: a `**Progress**:` line near the top, which `pack.rs:468-471` lifts into the pack. The fix is only the placement rule.*
5. **polish §9 in a factory run** (`:190`). The contract (`:13`) replaces every closeout step, but §9 still lists them. Add one line at its top: "In a factory run, skip to Report."
6. **prune §5, the log names tests too** (`skills/prune/SKILL.md:28`). It protects test names in evidence and Progress lines. Extend it to the feature's `log.jsonl`, whose Undo lines quote them.
7. **/slice, a file two FRs write** (near `skills/slice/SKILL.md:53`, the dependencies bullet). PRD-008's FR-5 and FR-6 both changed `docs/index.md`, so one slice built half and the QA guide claimed both. Add: flag a file two FRs write, and put both edits in one slice or name their order.
8. **/gate round 3, the anecdote** (`skills/gate/SKILL.md:80-81`). The "PRD-001 of the factory ran `cargo test` alone…" sentence ends the quiet-tests bullet, where it argues nothing. Move it back to the end of the inner-loop bullet (`:80`).

**The bump.** `Cargo.toml` goes to `0.8.0`, and `FACTORY_BUNDLE_WRITE=1 cargo test` rewrites `skills/bundle.lock`. `relock` refuses a second hash at 0.7.0 (`src/skills.rs:161`).

**Acceptance:** each of the eight lines reads as fixed in the skill diff, and the gate is green with the relocked bundle at 0.8.0.

## 5. Decisions log

No open questions.

- ~~Which shape ends the lost passes?~~ 2026-10-20: raise the timeout only. A `— partial` mark, a prompt line asking for defects as they are found, and salvaging thinking were all left out (§8).
- ~~Bundle the slips?~~ 2026-10-20: yes, plus the `/gate` anecdote idea, since its retrigger was "the next skill bundle bump".

## 6. QA plan

**Gate**: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` *(declared in project.toml)*
**Surface**: CLI (`factory review second`, `factory review collect`) and the skill text.
**Harness**: gate-only. Nothing user-visible changes beyond a longer wait and the skill text.
**Seed state**: none needed.

| FR | Proof on the real surface | Evidence | Human? |
|---|---|---|---|
| FR-1 | run `cargo test -q` with the defaults test pinning 1800; after merge and install, attended: the next real polish's review dir counted as killed / done / no-text | test transcript; the counts in `retro.md` § After merge | no |
| FR-2 | run the gate with the relocked bundle; read each of the eight lines in `git diff -- skills/` | gate transcript; the diff | no |

## 7. Slices

_(Filled by `/slice`.)_

## 8. Non-goals

- **Not handled: a killed pass with only opening chatter still counts** (2026-10-20). `body` (`src/review.rs:509-526`) counts any partial on a kill, which is how PRD-007's two empty passes read as reviewed. The owner left the `— partial` mark out; with fewer kills it should matter less. Reopen when a pass killed at 1800 s comes back `— counted` with no `file:line` in its text.
- **Not handled: thinking is not salvaged** on a kill (2026-10-20). It mixes findings with rejected hypotheses. Reopen if 1800 s still kills passes whose thinking holds finished findings.
- **Not handled: no prompt line asking for each defect as found** (2026-10-20). Whether grok obeys mid-run is unproven. Reopen with the same signal as above.
- **Not handled: diff passes that exit with no text** at 5 to 532 s (5 of 90), cause unknown. Reopen when one is reproduced.
- **Not here: model or effort changes.** `[review] model` is the owner's config, not this repo's.
- **Not here: `factory stop` killing a collect's review**, the idea `2026-10-15-factory-stop-should-kill-a-polish-collect-s-review`. A longer timeout makes an orphaned review live longer, which that idea's evidence will show.

## 9. Verification notes

Checked against `main` at `bf89434`, 2026-10-20. Re-verify at slice entry.

- `src/config.rs:71`: `timeout_secs: 900` in `Review::default()`; `:528` pins `(…, 900, 600)`; `:131` lists the `[review]` keys.
- `src/review.rs:380`: `run_logged(…, Duration::from_secs(cfg.timeout_secs))`; `:268` caps the probe at `PROBE_SECS`.
- `src/review.rs:332-359`: `parse_stream` keeps the `result`, an error result's text (`:339-340`) and `assistant` items of type `text` (`:349-352`); `thinking` is dropped. *Corrected 2026-10-20: an earlier note said result and assistant text only, missing the error.*
- `src/review.rs:508-526`: `body` counts a kill's partial, marked `(timed out after N s, partial)`.
- `src/collect.rs:139-147`: a pass's `counted` / `unreviewed` mark comes from `pass_text`.
- `src/closeout.rs:354`: the run's closeout calls `collect::collect` synchronously, with no outer timeout.
- `src/collect.rs:84-90`: `count` prints `1 fix hit` / `n fix hits`.
- `src/pack.rs:468-471`: the pack lifts a slice's `**Progress**:` line.
- `~/.config/factory/config.toml` has no `[review]` section on this machine.
- Review-dir counts from a scratch script over `~/.local/share/factory/reviews/*/review-N.jsonl`: 90 passes, 44 killed, 37 done, 9 no-text.
- `assets/contract-polish.md:14-15`: the polish worker writes `retro.md` in its last turn, before the driver commits and ships.
