# Slice 05 — Polish skill and autonomy text

**Source**: [`prd.md`](prd.md) → §3 FR-1, §7 FR-5 (the skill half), §8 FR-6 (the skill half), §9 FR-7, §10 FR-8, §2, §13. **Depends on**: — (independent: text only. It names `--feature` and `--since`, which slice 03 builds, but nothing compiles against them, and both land on the same branch and PR.)
**Created**: 2026-11-12

**Progress**:

- **Built.** In `/polish` §1: the security brief names `git diff <base>...HEAD`, tells the subagent not to take its range from the built-in skill, and reports the range, with an empty review reported as not run. `/ux` starts first, and no test or fix runs in the checkout until it returns. The collect gains `--feature <NNN>` and `--since <commit>` with its `git log -G '^reviewed'` command. The last resort gains `--feature <NNN>`. The fallback has a brief template. §2 lists five settled sources. §5 opens with "Fixes start after the UX pass returns". `assets/autonomy.md` gains the overturn rule outside the builder cut and the safe-contract Ask row in the Mapping. Four tests in `tests/skills_bundle.rs` pin these passages.
- **Inner loop.** 3 runs, 1 red, about 7 min. The red run was a stale `skills/bundle.lock` after a second skill edit under the version this slice had already bumped. Restoring the lock and rewriting it once fixed it.
- **Deviation.** The skill edit needed a version bump to 0.13.3 (`bundle_lock_matches`), and that bump also moves the cockpit mod's three version pins (`cockpit_mod`). This is AD-1 (fyi).
- **Prove.** The FR-1, FR-5 (skill row), FR-7 and FR-8 rows are left for the checkpoint.
- **Evidence (checkpoint 1):** FR-1 → `cp-1/fr-1-security-range.txt` (1 passed).
- **Evidence (checkpoint 1):** FR-5 (skill row) → `cp-1/fr-5-skill-sources.txt` (1 passed).
- **Evidence (checkpoint 1):** FR-7 → `cp-1/fr-7-ux-first.txt` (1 passed).
- **Evidence (checkpoint 1):** FR-8 → `cp-1/fr-8-autonomy-rows.txt` (1 passed).

**Slice**: After this slice, `/polish` §1:

- briefs the security subagent with `<base>...HEAD`;
- starts `/ux` first and runs no test or fix in the checkout until it returns;
- passes `--feature NNN`, and `--since` with the last-reviewed commit, to an attended collect and to the last resort;
- carries a fallback brief template.

`/polish` §2 treats five sources as settled. `assets/autonomy.md` gains the overturn row and the safe-contract Ask row. Skill-text tests pin each passage.

## Current code / environment

- **The security paragraph**: `skills/polish/SKILL.md:55`. It names `security-review`, lists the lens (auth, tenancy, input parsing, … logging), and says "Run it as its own subagent". The built-in skill's prompt diffs `origin/HEAD...` (prd §3, Claude Code 2.1.316). The brief must name `<base>...HEAD` and tell the subagent not to take its range from the built-in skill's git commands.
- **The collector order**: `:49` local review, `:53` leaks, `:55` security, `:57-59` `/ux`, `:61-63` second opinion. Fixes are §5 (`:119-128`), where "Each fixer runs the tests it touches". prd §9: start `/ux` first, run no test and no fix in that checkout until it returns, and run the read-only reviewers meanwhile.
- **The read-only rule**: `:47` "Every reviewer is read-only on the checkout". It stays, and the UX ordering sits beside it.
- **The attended collect command**: `:33` `factory review collect --base <base-branch> --out <scratchpad>/collect.md`. The last resort: `:68` `factory review second --diff <base-branch>`. Both gain `--feature <NNN>` on a feature branch (prd §7). The collect also gains `--since <commit>` when the feature's `state.toml` has `reviewed` set. The commit comes from `git log -1 --format=%H -G '^reviewed' -- docs/features/<NNN>-*/state.toml`, and the full diff is used when there is none or the owner asks (prd §8).
- **The fallback**: `:70` says to brief a fresh agent per pass, but gives no template. prd §7: a template holding the pass's files, the range and the same settled block (the feature's ADs, the PRD's Decisions log and Non-goals).
- **The settled rule**: `:89` counts "a settled `OPEN-n` or a decision in `docs/decisions.md`". prd §7: add slice task text, earlier ADs and the PRD's Non-goals, five sources in all.
- **Trap — numbered decisions**: `no_skill_or_asset_asks_for_a_numbered_decision` (`tests/skills_bundle.rs:76`) fails on any line containing the uppercase `DEC-` prefix. The source ideas say "DEC-n". Write "a decision in `docs/decisions.md`" instead.
- **autonomy.md is embedded, and builders see a cut copy**: `include_str!("../assets/autonomy.md")` (`src/pack.rs:17`). `builder_autonomy()` (`:37-38`) drops everything between `<!-- builder: cut -->` (`assets/autonomy.md:29`) and `<!-- builder: end -->` (`:42`), which is the Mapping table (`:30-40`).
  - **The overturn row** applies to any phase's AD, builders included, so it goes outside the cut, after the test table (`:17-25`) or beside the `Review:` paragraph (`:27`).
  - **The safe-contract Ask row** is a polish/ux Ask, so it goes in the Mapping inside the cut.
- **The `**Supersedes:** AD-n` line**: prd §10 names its exact form, and slice 03's settled block parses it. Write it exactly so.
- **The skills bundle**: any edit under `skills/` changes `skills/bundle.lock`. Run `FACTORY_BUNDLE_WRITE=1 cargo test` once (`tests/skills_bundle.rs:190`), then commit the lock with the skill. `assets/autonomy.md` is not in the bundle.
- **Test pattern**: `polish_collect_starts_from_the_collected_file_with_the_last_resort_beside_it` (`tests/skills_bundle.rs:127-140`) cuts `## 1.`…`## 2.` and asserts substrings. Its three kept strings must survive.
- **Deliberate non-change**: prd §15 "Not handled: a UX brief that starts from checkpoint evidence and lists states up front" (owner cut 2026-11-12). Do not edit `skills/ux/SKILL.md`. Also "Not handled: the UX pass's own worktree and database". The ordering is the whole fix.

## Tasks

### 1. Security pass reviews the base range — high — blocked by: —
**Source**: §3 FR-1: "The subagent reviews `git diff <base>...HEAD`, with the security lens the paragraph already lists. It does not rely on the built-in skill's own git commands. The prompt says which range it reviewed."

Edit `skills/polish/SKILL.md:55` to say:

- the subagent's prompt names the range and keeps the lens;
- it does not take its range from the built-in skill;
- it reports the range it reviewed, and one that reviewed nothing says so, so the pass is reported as not run.

**Acceptance:** `polish_security_pass_reviews_the_base_range` asserts that §1's security paragraph contains `<base>...HEAD` and the instruction not to take the built-in's range.

### 2. UX first, no tests until it returns — high — blocked by: —
**Source**: §9 FR-7: "Start `/ux` first, before anything that runs tests. Run no test and no fix in that checkout until the UX pass returns. The local review, security pass and second opinion are read-only, so they run meanwhile. Fixes (§5) start after the UX pass returns."

Edit §1, where it orders the collectors, and §5's opening, to carry this.

**Acceptance:** `polish_starts_ux_first_and_holds_tests` asserts §1 says to start `/ux` first and forbids tests in the checkout until it returns, and that §5 says fixes start after it.

### 3. Settled sources, flags and the fallback template — high — blocked by: —
**Source**: §7 FR-5: "§2 treats slice task text, earlier ADs and the PRD's Non-goals as settled… §1 gains a fallback brief template"; §8 FR-6: "Polish §1 does the same attended, and the skill names the git command."

The edits:

- `:33` and `:68` gain `--feature <NNN>`.
- `:33` gains the `--since` rule and its git command.
- `:70` gains a short fallback brief template: the files, the range, the settled block, and "a finding that contradicts an entry names it".
- `:89` lists the five settled sources.

**Acceptance:** `polish_lists_five_settled_sources` asserts §2 names `OPEN-n`, `docs/decisions.md`, slice task text, earlier ADs and Non-goals. The existing §1 test still passes. A substring check asserts §1 holds `--feature` and `--since`.

### 4. Two autonomy rows — high — blocked by: —
**Source**: §10 FR-8: "An AD that overturns an earlier AD is allowed. Its body carries a `**Supersedes:** AD-n` line… A contract-changing Ask whose current behavior is safe is parked with `factory docs idea add` for the owner, and never blocks the run."

`assets/autonomy.md` gets two additions:

- **The overturn rule**, outside the builder cut: allowed, cite the overturned AD, and write `**Supersedes:** AD-n` in the new AD's body.
- **A Mapping row** inside the cut: a `/polish` or `/ux` Ask whose fix changes a contract while today's behavior is safe to ship → `factory docs idea add`, no block.

**Acceptance:** `autonomy_has_the_overturn_and_safe_contract_rows` asserts both texts exist. The overturn rule must survive `pack::builder_autonomy()` and name `**Supersedes:**`. The contract row must sit inside the Mapping.

### 5. Verify — medium — blocked by: 1, 2, 3, 4
`FACTORY_BUNDLE_WRITE=1 cargo test -q` once to rewrite `skills/bundle.lock`, then `cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q`

**Acceptance:** inner loop green, one commit holding the skill, the asset, the lock and the tests.

### 6. Prove — at the checkpoint — blocked by: 5
prd §13 rows:

- **FR-1:** `polish_security_pass_reviews_the_base_range`.
- **FR-5:** the skill row, `polish_lists_five_settled_sources`.
- **FR-7:** `polish_starts_ux_first_and_holds_tests`.
- **FR-8:** `autonomy_has_the_overturn_and_safe_contract_rows`.

They are proven below the UI: the checkpoint runs them and keeps the transcript.

**Acceptance:** after the checkpoint, the transcript is in Progress.

**Code-review fixes (2026-11-12).**

- Polish §8 still said to set `reviewed` in the PRD's frontmatter, which nothing reads, so an attended round left `--since` with no commit to find. It now says `factory docs set <NNN> reviewed=<date>`.
- The new safe-contract Ask row and the general Ask row both matched one case. The row now says it wins over the Ask row.
- The overturn rule let a `fyi` AD overturn a `decide` one with no owner question. Such an AD is now `decide` itself (AD-8). The bundle lock was rewritten under 0.13.3.
