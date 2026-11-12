# Slice 02 — Polish and prune text

**Source**: [`prd.md`](prd.md) → §3 FR-1, §5 FR-3, §7 FR-5, §10 FR-8, §2, §15. **Depends on**: 01. Its tests call slice 01's helper and go in `tests/skills_bundle.rs`, which 01 rewrites.
**Created**: 2026-11-12

**Progress**: _(written as it lands — what was built, the gate numbers, what a review caught)_

**Slice**: After this slice:

- polish's closeout names the PRD `summary` instead of a dead Notes cell;
- polish's UX override and skip passages name an API-only change;
- prune counts polish's own edits as under review;
- prune lets a cited test name go, mapping old → new in the commit body or the report;
- prune's PR mode names the whole-suite branch and reuses polish's fresh gate.

This slice also bumps the version to 0.13.4, because it is the first skill edit on the branch. The UTC date sentences, polish's included, are slice 04's.

## Current code / environment

- **FR-1**: `skills/polish/SKILL.md:214`, in §9's closeout list: "- the index **Notes** cell — decisions taken, what remains, the PR number". v3's index has no Notes cell (`assets/docs/index.md`). `skills/prd/SKILL.md:129` says `summary` "replaces any notes cell".
- **FR-5**: two passages.
  - `skills/polish/SKILL.md:45`: "Two rules override the table. … **Anything user-visible gets the UX pass**, however small — a one-line CSS change is exactly where a state breaks."
  - `:61`: "Skip it only when nothing user-visible changed, and say so. No harness … A Claude Code mod is the exception: …"
  - prd §7 corrected: `:45` must carry the exception too, or it overrides `:61`.
  - `polish_starts_ux_first_and_holds_tests` (`tests/skills_bundle.rs:300`) pins `:47`, so keep that passage intact.
- **FR-3**: `skills/prune/SKILL.md:24`, precondition 1: "…Working tree must be clean, or dirty only with the changes under review … Inside a `factory` run the uncommitted polish fixes are the changes under review."
  - Polish calls prune at §7 (`skills/polish/SKILL.md:155-159`).
  - Before that, §5 fixes code and writes log entries, and §6 writes `docs/ideas/` files.
- **FR-8**: prune's precondition 5 and PR mode.
  - Precondition 5, `skills/prune/SKILL.md:28`: "…A consolidation that renames one leaves those as they are: do not edit a transcript, and do not rewrite a Progress line or a log entry. Prefer keeping the name. When a rename is still right, say the old and new names in the commit body or your report. One run renamed a method a slice's Progress named and had to put it back."
  - PR mode, `:71-79`: step 3 is "Baseline: run the scoped test files 3 times (cheap at this scope)…". Step 6 runs the verify gate.
  - `:61`: "Inside a `factory` run the driver commits everything at once, so the per-unit rationale goes in your report instead." That is why the old → new map goes "in the commit body, or in its report inside a `factory` run" (prd §10, corrected).
  - The anecdote "had to put it back" argued for keeping names. With the new rule it is the case the commit-body map covers. Reword it or drop it, but do not keep "Prefer keeping the name".
- **The gate reuse condition** (prd §10): "right after a green gate, with no code or test change since (polish §6's `docs/ideas/` files do not count)".
- **Version bump** (prd §2): `bundle_lock_matches` (`tests/skills_bundle.rs:202`) refuses a skill edit under 0.13.3. Bump to 0.13.4 in:
  - `Cargo.toml` and `Cargo.lock`;
  - the cockpit mod's three pins, which `tests/cockpit_mod.rs:19-40` checks: `mods/cockpit/.claude-plugin/plugin.json:3`, plus the pins in `mods/cockpit/hooks/` and `mods/cockpit/tests/cockpit.test.tsx`. Grep for `0.13.3`.
  - Then run `FACTORY_BUNDLE_WRITE=1 cargo test -q --test skills_bundle` once to rewrite `skills/bundle.lock`.
- **The relock trap** (`src/skills.rs:165-182`): a second skill edit under the same, already-relocked version refuses ("bump Cargo.toml past 0.13.4 first"). Restore the lock from the base first, with `git show main:skills/bundle.lock > skills/bundle.lock`, then relock once. PRD-017's run logged the same workaround. prd §2 keeps the real fix in PRD-015.
- **Deliberate non-changes**:
  - prd §17 "Not handled: an Ask that reverses an AD a checkpoint already proved". Do not touch polish §4 or the autonomy mapping row.
  - Polish §8's `reviewed=<date>` verb is already done (prd §17). Its UTC sentence is slice 04's.

## Tasks

### 1. Polish's closeout and the API-only UX case — high — blocked by: —
**Source**: §3 FR-1: "The bullet becomes: keep the PRD's `summary` frontmatter current, saying what landed and what remains. No PR number goes in it." §7 FR-5: "`:45`'s override rule … excepts a change whose only surface is an API. `:61` says what to do then: skip `/ux`, say so, and point at the QA plan's Human row if there is one."

Edit `skills/polish/SKILL.md:214`, `:45` and `:61`.

**Acceptance:**

- `polish_closeout_keeps_the_summary_current`: §9 holds the `summary` passage exactly once, and the file no longer holds "the index **Notes** cell".
- `polish_skips_ux_on_an_api_only_change`: the override sentence and the skip paragraph each hold their API-only passage exactly once.

Both go in `tests/skills_bundle.rs` through the helper.

### 2. Prune's preconditions and PR mode — high — blocked by: —
**Source**: §5 FR-3: "when polish invokes prune, attended or not, polish's own uncommitted edits count as changes under review: fixes, slice docs, `docs/ideas/` files and the feature's log." §10 FR-8: "A name cited only in frozen records … does not bind. Prune may rename or merge freely… names old → new in the commit body, or in its report inside a `factory` run"; "When the scoped tests are most of the suite, prune takes full mode's coverage baseline. When polish invokes prune right after a green gate, with no code or test change since …, step 3's three repeat runs are replaced by that gate result, cited in the report."

Edit `skills/prune/SKILL.md:24`, `:28` and PR mode `:71-79`.

**Acceptance:**

- `prune_counts_polish_own_edits_as_under_review`: precondition 1 names polish's docs and idea edits and is not limited to a `factory` run.
- `prune_lets_a_cited_name_go`: precondition 5 holds the frozen-records passage once and no longer holds "Prefer keeping the name".
- `prune_pr_mode_reuses_the_polish_gate`: PR mode holds the whole-suite passage and the gate-reuse passage, once each.

### 3. Bump and relock — high — blocked by: 1, 2
**Source**: §2: "A skill edit needs a version bump past 0.13.3 (`bundle_lock_matches`), and the bump moves the cockpit mod's three version pins."

Bump to 0.13.4 as described under Current code, then relock once.

**Acceptance:** `bundle_lock_matches` and `tests/cockpit_mod.rs` pass.

### 4. Verify — medium — blocked by: 3
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q`

**Acceptance:** inner loop green, one commit.

### 5. Prove — at the checkpoint — blocked by: 4
prd §15 rows:

- **FR-1:** `polish_closeout_keeps_the_summary_current`.
- **FR-3:** `prune_counts_polish_own_edits_as_under_review`.
- **FR-5:** `polish_skips_ux_on_an_api_only_change`.
- **FR-8:** `prune_lets_a_cited_name_go` and `prune_pr_mode_reuses_the_polish_gate`.

They are proven below the UI: the checkpoint runs them and keeps the transcript.

**Acceptance:** after the checkpoint, the transcripts are in Progress.
